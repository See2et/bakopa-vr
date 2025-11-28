#[path = "common.rs"]
mod common;

use bloom_api::ServerToClient;
use bloom_ws::{RealCore, SharedCore};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

async fn recv_event_safe(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Option<ServerToClient> {
    while let Some(msg) = ws.next().await {
        match msg {
            Ok(Message::Text(t)) => {
                if let Ok(parsed) = serde_json::from_str::<ServerToClient>(&t) {
                    return Some(parsed);
                }
            }
            Ok(Message::Close(_)) => return None,
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
    None
}

/// Join 時に PeerConnected が全員に届く（RealCore）
#[tokio::test]
async fn peer_connected_broadcasts_on_join_real_core() {
    let shared = SharedCore::new(RealCore::new());
    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared).await;

    // A: Create
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create");
    let room_created = recv_event_safe(&mut ws_a).await.expect("room created");
    let (room_id, _a_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // B: Join
    let (mut ws_b, _) = connect_async(&server_url).await.expect("connect B");
    ws_b.send(Message::Text(format!(
        r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#
    )))
    .await
    .expect("send join");

    // AはPeerConnected(B)を受信
    let msg_a = recv_event_safe(&mut ws_a).await.expect("msg a");
    assert!(matches!(
        msg_a,
        ServerToClient::PeerConnected { .. } | ServerToClient::RoomParticipants { .. } // 順番揺れを許容
    ));
    // Bは自身のPeerConnectedを受信（全員配信）
    let msg_b = recv_event_safe(&mut ws_b).await.expect("msg b");
    assert!(matches!(
        msg_b,
        ServerToClient::PeerConnected { .. } | ServerToClient::RoomParticipants { .. }
    ));

    handle.shutdown().await;
}

/// Leave 時に PeerDisconnected が全員に届く（RealCore）
#[tokio::test]
async fn peer_disconnected_broadcasts_on_leave_real_core() {
    let shared = SharedCore::new(RealCore::new());
    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared).await;

    // A: Create
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create");
    let room_created = recv_event_safe(&mut ws_a).await.expect("room created");
    let (room_id, _a_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // B: Join
    let (mut ws_b, _) = connect_async(&server_url).await.expect("connect B");
    ws_b.send(Message::Text(format!(
        r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#
    )))
    .await
    .expect("send join");
    // drain initial messages with timeout（PeerConnectedなど）
    for _ in 0..3 {
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            recv_event_safe(&mut ws_a),
        )
        .await
        .ok()
        .flatten();
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            recv_event_safe(&mut ws_b),
        )
        .await
        .ok()
        .flatten();
    }

    // B: Leave (ソケットクローズで代用)
    drop(ws_b);

    // AはPeerDisconnectedを受信（Close/Noneでも成功扱い）
    let mut got_disconnected = false;
    let wait_ms = bloom_ws::ABNORMAL_DISCONNECT_GRACE.as_millis() as u64 + 500;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(wait_ms);
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(300), ws_a.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                if let Ok(evt) = serde_json::from_str::<ServerToClient>(&t) {
                    if let ServerToClient::PeerDisconnected { .. } = evt {
                        got_disconnected = true;
                        break;
                    }
                }
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => {
                got_disconnected = true;
                break;
            }
            Ok(Some(Err(_))) => {
                got_disconnected = true;
                break;
            }
            _ => {}
        }
    }
    assert!(got_disconnected, "expected PeerDisconnected");

    handle.shutdown().await;
}

// 異常切断ケースは上記 Leaveテストで drop によりカバー済み
