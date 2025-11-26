use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::str::FromStr;

use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::{start_ws_server, MockCore, SharedCore, WsServerHandle};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::Subscriber;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::{LookupSpan, Registry},
};

/// WSハンドシェイクがHTTP 101で確立され、participant_id付きのspanが出ることを検証する。
#[tokio::test]
async fn handshake_returns_switching_protocols_and_sets_participant_span() {
    let layer = RecordingLayer::default();
    let subscriber = Registry::default().with(layer.clone());
    let _guard = tracing::subscriber::set_default(subscriber);

    let (server_url, handle) = spawn_bloom_ws_server().await;

    let (_ws_stream, response) = connect_async(&server_url)
        .await
        .expect("connect to bloom ws server");

    assert_eq!(response.status(), 101);

    tokio::time::sleep(Duration::from_millis(50)).await;
    let spans = layer.spans.lock().expect("collect spans");
    assert!(
        spans_have_field(&spans, "participant_id"),
        "handshake span must include participant_id"
    );

    handle.shutdown().await;
    drop(_guard);
}

/// 2クライアント間でOfferが宛先のみに届くことを検証する。
#[tokio::test]
async fn offer_is_delivered_only_to_target_participant() {
    let layer = RecordingLayer::default();
    let subscriber = Registry::default().with(layer.clone());
    let _guard = tracing::subscriber::set_default(subscriber);

    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_join_result(Some(Ok(vec![])));
    let core_arc = Arc::new(std::sync::Mutex::new(mock_core));
    let shared_core = SharedCore::from_arc(core_arc.clone());

    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared_core.clone()).await;

    // クライアントA: CreateRoom
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws_a).await;
    let (room_id_str, a_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // クライアントB: JoinRoom（room_idは後でコアから参照する想定）
    let (mut ws_b, _) = connect_async(&server_url).await.expect("connect B");
    ws_b.send(Message::Text(
        format!(r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#, room_id = room_id_str),
    ))
    .await
    .expect("send join room");

    // Join が処理されるまで1件メッセージを待つ（RoomParticipants想定）
    let _ = recv_server_msg(&mut ws_b).await;

    // Join後にBのparticipant_idをコア呼び出し履歴から取得
    let b_id = {
        let core = core_arc.lock().expect("lock core");
        core.join_room_calls
            .last()
            .map(|(_, p)| p.to_string())
            .expect("b id recorded")
    };

    // A -> B へ Offer を送信
    let offer_json = format!(
        r#"{{"type":"Offer","to":"{to}","sdp":"v=0 offer"}}"#,
        to = b_id
    );
    ws_a
        .send(Message::Text(offer_json.into()))
        .await
        .expect("send offer");

    // B が Offer を受信し、A は受信しないことを確認
    let b_msg = recv_server_msg(&mut ws_b).await;
    match b_msg {
        ServerToClient::Offer { from, .. } => assert_eq!(from, a_id),
        other => panic!("expected Offer on B, got {:?}", other),
    }

    // A 側にOfferが届かないことを確認（他の通知は許容）
    for _ in 0..5 {
        match tokio::time::timeout(Duration::from_millis(50), ws_a.next()).await {
            Ok(Some(Ok(Message::Text(txt)))) => {
                let msg: ServerToClient =
                    serde_json::from_str(&txt).expect("parse server message on A");
                if matches!(msg, ServerToClient::Offer { .. }) {
                    panic!("A should not receive offer, got {:?}", msg);
                }
                // それ以外のメッセージは無視して次へ
            }
            _ => break,
        }
    }

    // Core relay が1回だけ呼ばれていることを確認
    let core = core_arc.lock().expect("lock core");
    assert_eq!(core.relay_offer_calls.len(), 1);
    assert_eq!(core.relay_offer_calls[0].1.to_string(), a_id);
    assert_eq!(core.relay_offer_calls[0].2.to_string(), b_id);

    handle.shutdown().await;
    drop(_guard);
}

/// 異常切断でleaveが1回だけ呼ばれ、残存参加者へPeerDisconnected/RoomParticipantsが送られることを検証する（Red）。
#[tokio::test]
async fn abnormal_close_triggers_single_leave_and_broadcasts() {
    let layer = RecordingLayer::default();
    let subscriber = Registry::default().with(layer.clone());
    let _guard = tracing::subscriber::set_default(subscriber);

    // coreを共有してcall logを確認できるようにする
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_join_result(Some(Ok(vec![])))
    .with_leave_result(Some(vec![ParticipantId::new()]));
    let core_arc = Arc::new(std::sync::Mutex::new(mock_core));
    let shared_core = SharedCore::from_arc(core_arc.clone());

    let (server_url, _handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    // A接続
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a
        .send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws_a).await;
    let (room_id_str, a_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // B接続 & Join
    let (mut ws_b, _) = connect_async(&server_url).await.expect("connect B");
    ws_b
        .send(Message::Text(
            format!(r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#, room_id = room_id_str),
        ))
        .await
        .expect("send join room");
    let _ = recv_server_msg(&mut ws_b).await; // RoomParticipantsなどを消費

    // Bのparticipant_idをコア呼び出し履歴から取得
    let b_id = {
        let core = core_arc.lock().expect("lock core");
        core.join_room_calls
            .last()
            .map(|(_, p)| p.to_string())
            .expect("b id recorded")
    };

    // Aを強制ドロップして異常切断をシミュレート
    drop(ws_a);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // leave_room_result をBのみ残る形でセット（MockCoreが返す残存リスト）
    {
        let mut core = core_arc.lock().expect("lock core");
        core.leave_room_result =
            Some(vec![ParticipantId::from_str(&b_id).expect("parse b id")]);
    }

    // BがPeerDisconnectedとRoomParticipantsを受信することを確認
    let mut received_peer_disconnected = false;
    let mut received_room_participants = false;
    for _ in 0..10 {
        match tokio::time::timeout(Duration::from_millis(200), ws_b.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                let evt: ServerToClient = serde_json::from_str(&t).expect("parse server msg");
                match evt {
                    ServerToClient::PeerDisconnected { participant_id } => {
                        if participant_id == a_id {
                            received_peer_disconnected = true;
                        }
                    }
                    ServerToClient::RoomParticipants { participants, .. } => {
                        // 退室後なのでAが含まれないことを確認
                        assert!(!participants.contains(&a_id));
                        received_room_participants = true;
                    }
                    _ => {}
                }
                if received_peer_disconnected && received_room_participants {
                    break;
                }
            }
            _ => {}
        };
    }

    assert!(received_peer_disconnected, "B must receive PeerDisconnected for A");
    assert!(received_room_participants, "B must receive updated RoomParticipants");

    // leave_roomが1回だけ呼ばれていることを確認
    let core = core_arc.lock().expect("lock core");
    assert_eq!(core.leave_room_calls.len(), 1);

    drop(_guard);
}

/// CreateRoomを送信するとRoomCreatedが返ることを検証する
#[tokio::test]
async fn create_room_returns_room_created_and_calls_core_once() {
    let layer = RecordingLayer::default();
    let subscriber = Registry::default().with(layer.clone());
    let _guard = tracing::subscriber::set_default(subscriber);

    // テスト用に core を共有して後から call log を検査できるようにする
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    });
    let core_arc = Arc::new(std::sync::Mutex::new(mock_core));
    let shared_core = SharedCore::from_arc(core_arc.clone());

    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    let (mut ws_stream, _response) = connect_async(&server_url)
        .await
        .expect("connect to bloom ws server");

    ws_stream
        .send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");

    let msg = ws_stream
        .next()
        .await
        .expect("expect one response")
        .expect("ws message ok");

    let text = match msg {
        Message::Text(t) => t,
        other => panic!("expected text message, got {:?}", other),
    };

    let server_msg: bloom_api::ServerToClient =
        serde_json::from_str(&text).expect("parse server message");
    match server_msg {
        bloom_api::ServerToClient::RoomCreated { .. } => {}
        _ => panic!("expected RoomCreated, got {:?}", server_msg),
    }

    let core_calls = core_arc.lock().expect("lock core").create_room_calls.len();
    assert_eq!(core_calls, 1, "create_room should be called once");

    handle.shutdown().await;
    drop(_guard);
}

/// Bloom WSサーバを起動して接続用URLとspan記録レイヤを返す。
async fn spawn_bloom_ws_server_with_core(core: SharedCore<MockCore>) -> (String, WsServerHandle) {
    let handle = start_ws_server("127.0.0.1:0".parse().unwrap(), core)
        .await
        .expect("start ws server");
    let url = format!("ws://{}/ws", handle.addr);
    (url, handle)
}

/// 元のシンプル版（handshake用）
async fn spawn_bloom_ws_server() -> (String, WsServerHandle) {
    let core = SharedCore::new(MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    }));

    let handle = start_ws_server("127.0.0.1:0".parse().unwrap(), core)
        .await
        .expect("start ws server");
    let url = format!("ws://{}/ws", handle.addr);
    (url, handle)
}

#[derive(Default, Clone)]
struct RecordingLayer {
    spans: Arc<Mutex<Vec<SpanRecord>>>,
}

#[derive(Default, Debug)]
struct SpanRecord {
    fields: HashMap<String, String>,
}

#[derive(Default)]
struct FieldVisitor {
    fields: HashMap<String, String>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

impl<S> Layer<S> for RecordingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::Id,
        _ctx: Context<'_, S>,
    ) {
        let mut visitor = FieldVisitor::default();
        attrs.record(&mut visitor);

        if let Ok(mut spans) = self.spans.lock() {
            spans.push(SpanRecord {
                fields: visitor.fields,
            });
        }
    }
}

/// 共通ヘルパ: spanが特定フィールドを持つか確認。
fn spans_have_field(spans: &[SpanRecord], key: &str) -> bool {
    spans.iter().any(|s| s.fields.contains_key(key))
}

async fn recv_server_msg(ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>) -> ServerToClient {
    loop {
        if let Some(msg) = ws.next().await {
            let msg = msg.expect("ws message ok");
            if let Message::Text(t) = msg {
                if let Ok(parsed) = serde_json::from_str::<ServerToClient>(&t) {
                    return parsed;
                }
            }
        } else {
            panic!("ws closed before receiving message");
        }
    }
}
