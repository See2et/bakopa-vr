#[path = "common.rs"]
mod common;

use bloom_api::{ErrorCode, ServerToClient};
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::SharedCore;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// レート制御: 1秒間に21件送信で21件目がRateLimitedになることを検証する。
#[tokio::test]
async fn rate_limit_drops_21st_message_and_returns_error() {
    let core = SharedCore::new(bloom_ws::MockCore::new(CreateRoomResult {
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

    for _ in 0..21 {
        ws.send(Message::Text(
            format!(
                r#"{{"type":"Offer","to":"{to}","sdp":"v=0 offer","room_id":"{room_id}"}}"#,
                to = self_id,
                room_id = room_id
            )
            .into(),
        ))
        .await
        .expect("send offer");
    }

    let mut got_rate_limited = false;
    for _ in 0..30 {
        match tokio::time::timeout(std::time::Duration::from_millis(200), ws.next()).await {
            Ok(Some(Ok(Message::Text(txt)))) => {
                if let Ok(msg) = serde_json::from_str::<ServerToClient>(&txt) {
                    if let ServerToClient::Error { code, .. } = msg {
                        if code == ErrorCode::RateLimited {
                            got_rate_limited = true;
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    assert!(got_rate_limited, "21st message should return RateLimited");

    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    ws.send(Message::Text(
        format!(
            r#"{{"type":"Offer","to":"{to}","sdp":"v=0 offer","room_id":"{room_id}"}}"#,
            to = self_id,
            room_id = room_id
        )
        .into(),
    ))
    .await
    .expect("send after cooldown");

    handle.shutdown().await;
}
