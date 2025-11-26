#[path = "common.rs"]
mod common;

use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::{MockCore, SharedCore};
use futures_util::SinkExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// ログ目的のハンドシェイクspan（participant_idフィールド）を再確認する。
#[tokio::test]
async fn handshake_emits_span_with_participant_id_field_over_ws() {
    let (layer, _guard) = setup_tracing();
    let (server_url, handle) = spawn_bloom_ws_server().await;

    let (_ws_stream, _response) = connect_async(&server_url)
        .await
        .expect("connect to bloom ws server");

    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    let spans = layer.spans.lock().expect("collect spans");
    assert!(spans_have_field(&spans, "participant_id"));

    handle.shutdown().await;
}

/// Offer処理のspanにparticipant_idとroom_idが含まれることの補助確認。
#[tokio::test]
async fn offer_span_includes_participant_and_room_over_ws_again() {
    let (layer, _guard) = setup_tracing();
    let core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    });
    let (server_url, handle) = spawn_bloom_ws_server_with_core(SharedCore::new(core)).await;

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
