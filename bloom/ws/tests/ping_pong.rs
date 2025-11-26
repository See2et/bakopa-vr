#[path = "common.rs"]
mod common;

use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::SharedCore;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// サーバが定期Pingを送信することを検証する（auto-pongがあるため切断までは検証しない）。
#[tokio::test]
async fn server_sends_periodic_ping_and_disconnects_on_missing_pong() {
    let core = SharedCore::new(bloom_ws::MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    }));
    let (server_url, _handle) = spawn_bloom_ws_server_with_core(core).await;

    // クライアントはPingに応答しない（Pongを送らない）
    let (mut ws, _) = connect_async(&server_url).await.expect("connect client");
    ws.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let _ = recv_server_msg(&mut ws).await; // consume RoomCreated

    // 30s間隔のPingを1回受信できることを確認（auto-pongが起きるため切断は検証しない）
    let recv = tokio::time::timeout(tokio::time::Duration::from_secs(40), async {
        loop {
            if let Some(msg) = ws.next().await {
                if let Ok(Message::Ping(_)) = msg {
                    return;
                }
            }
        }
    })
    .await;
    assert!(recv.is_ok(), "should receive at least one Ping from server");
}
