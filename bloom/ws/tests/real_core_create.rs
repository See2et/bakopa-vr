#[path = "common.rs"]
mod common;

use bloom_api::ServerToClient;
use bloom_ws::SharedCore;
use futures_util::SinkExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// RealCoreをstart_ws_serverに渡してCreateRoomが成功すること
#[tokio::test]
async fn create_room_with_real_core_returns_room_created() {
    let real_core = bloom_ws::RealCore::new();
    let shared = SharedCore::new(real_core);

    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared).await;

    let (mut ws, _) = connect_async(&server_url).await.expect("connect client");

    ws.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");

    let msg = recv_server_msg(&mut ws).await;
    match msg {
        ServerToClient::RoomCreated { room_id, self_id } => {
            assert!(!room_id.is_empty());
            assert!(!self_id.is_empty());
        }
        other => panic!("expected RoomCreated, got {:?}", other),
    }

    handle.shutdown().await;
}
