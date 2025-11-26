#[path = "common.rs"]
mod common;
#[path = "logging_common.rs"]
mod logging_common;

use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::{MockCore, SharedCore};
use futures_util::SinkExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;
use logging_common::*;

fn shared_core_with_arc() -> (
    SharedCore<MockCore>,
    std::sync::Arc<std::sync::Mutex<MockCore>>,
) {
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    });
    let core_arc = std::sync::Arc::new(std::sync::Mutex::new(mock_core));
    let shared = SharedCore::from_arc(core_arc.clone());
    (shared, core_arc)
}

/// WSハンドシェイクがHTTP 101で確立され、participant_id付きのspanが出ることを検証する。
#[tokio::test]
async fn handshake_returns_switching_protocols_and_sets_participant_span() {
    let (layer, _guard) = setup_tracing();
    let core = SharedCore::new(MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    }));
    let (server_url, handle) = spawn_bloom_ws_server_with_core(core).await;

    let (_ws_stream, response) = connect_async(&server_url)
        .await
        .expect("connect to bloom ws server");

    assert_eq!(response.status(), 101);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let spans = layer.spans.lock().expect("collect spans");
    assert!(spans_have_field_value(&spans, "participant_id", ""));

    handle.shutdown().await;
}

/// CreateRoomを送信するとRoomCreatedが返り、coreが一度呼ばれることを検証。
#[tokio::test]
async fn create_room_returns_room_created_and_calls_core_once() {
    let (_layer, _guard) = setup_tracing();
    let (shared_core, core_arc) = shared_core_with_arc();
    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    let (mut ws_stream, _response) = connect_async(&server_url)
        .await
        .expect("connect to bloom ws server");

    ws_stream
        .send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");

    let msg = recv_server_msg(&mut ws_stream).await;
    match msg {
        ServerToClient::RoomCreated { .. } => {}
        _ => panic!("expected RoomCreated, got {:?}", msg),
    }

    let core_calls = core_arc.lock().expect("lock core").create_room_calls.len();
    assert_eq!(core_calls, 1);

    handle.shutdown().await;
}

/// tracingにparticipant_idとroom_idが載ることを統合経路で確認する。
#[tokio::test]
async fn offer_span_includes_participant_and_room_over_ws() {
    let (layer, _guard) = setup_tracing();
    let core = SharedCore::new(MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    }));
    let (server_url, handle) = spawn_bloom_ws_server_with_core(core).await;

    let (mut ws, _) = connect_async(&server_url).await.expect("connect client");
    ws.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws).await;
    let (room_id, self_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    let offer_json = format!(
        r#"{{"type":"Offer","to":"{to}","sdp":"v=0 offer","room_id":"{room_id}"}}"#,
        to = self_id,
        room_id = room_id
    );
    ws.send(Message::Text(offer_json.into()))
        .await
        .expect("send offer");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let spans = layer.spans.lock().expect("collect spans");
    assert!(spans_have_field_value(&spans, "participant_id", &self_id));
    assert!(spans_have_field_value(&spans, "room_id", &room_id));

    handle.shutdown().await;
}
