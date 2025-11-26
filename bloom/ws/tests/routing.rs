#[path = "common.rs"]
mod common;

use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::MockCore;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// Offerが宛先のみに届くことを検証する。
#[tokio::test]
async fn offer_is_delivered_only_to_target_participant() {
    let (_layer, _guard) = setup_tracing();

    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_join_result(Some(Ok(vec![])));
    let core_arc = std::sync::Arc::new(std::sync::Mutex::new(mock_core));
    let shared_core = bloom_ws::SharedCore::from_arc(core_arc.clone());

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

    // クライアントB: JoinRoom
    let (mut ws_b, _) = connect_async(&server_url).await.expect("connect B");
    ws_b.send(Message::Text(format!(
        r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#,
        room_id = room_id_str
    )))
    .await
    .expect("send join room");
    // join処理完了を待つ
    let _ = recv_server_msg(&mut ws_b).await;

    // 参加者IDをコア呼び出しから取得
    let b_id = {
        let core = core_arc.lock().expect("lock core");
        core.join_room_calls
            .last()
            .map(|(_, p)| p.to_string())
            .expect("b id recorded")
    };

    // A -> B へ Offer
    let offer_json = format!(
        r#"{{"type":"Offer","to":"{to}","sdp":"v=0 offer"}}"#,
        to = b_id
    );
    ws_a.send(Message::Text(offer_json.into()))
        .await
        .expect("send offer");

    let b_msg = recv_server_msg(&mut ws_b).await;
    match b_msg {
        ServerToClient::Offer { from, .. } => assert_eq!(from, a_id),
        other => panic!("expected Offer on B, got {:?}", other),
    }

    // AにはOfferが届かない
    for _ in 0..5 {
        match tokio::time::timeout(std::time::Duration::from_millis(50), ws_a.next()).await {
            Ok(Some(Ok(Message::Text(txt)))) => {
                let msg: ServerToClient =
                    serde_json::from_str(&txt).expect("parse server message on A");
                if matches!(msg, ServerToClient::Offer { .. }) {
                    panic!("A should not receive offer, got {:?}", msg);
                }
            }
            _ => break,
        }
    }

    let core = core_arc.lock().expect("lock core");
    assert_eq!(core.relay_offer_calls.len(), 1);
    assert_eq!(core.relay_offer_calls[0].1.to_string(), a_id);
    assert_eq!(core.relay_offer_calls[0].2.to_string(), b_id);

    handle.shutdown().await;
}
