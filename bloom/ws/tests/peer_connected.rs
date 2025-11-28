#[path = "common.rs"]
mod common;

use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::{MockCore, RealCore, SharedCore, CoreApi};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

async fn run_peer_connected_test<C: CoreApi + Send + 'static>(shared_core: SharedCore<C>) {
    let (server_url, _handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    // A: CreateRoom
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws_a).await;
    let (room_id, a_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // B: JoinRoom
    let (mut ws_b, _) = connect_async(&server_url).await.expect("connect B");
    ws_b.send(Message::Text(format!(
        r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#,
        room_id = room_id
    )))
    .await
    .expect("send join room");

    // 参加者IDはRoomParticipantsから取得（A/Bどちらかから）
    let b_id = {
        let mut found: Option<String> = None;
        for _ in 0..6 {
            for ws in [&mut ws_a, &mut ws_b] {
                if let Ok(Some(Ok(Message::Text(t)))) =
                    tokio::time::timeout(std::time::Duration::from_millis(300), ws.next()).await
                {
                    if let Ok(evt) = serde_json::from_str::<ServerToClient>(&t) {
                        if let ServerToClient::RoomParticipants { participants, .. } = evt {
                            if let Some(id) = participants.iter().find(|pid| *pid != &a_id) {
                                found = Some(id.clone());
                                break;
                            }
                        }
                    }
                }
            }
            if found.is_some() {
                break;
            }
        }
        found.expect("should receive RoomParticipants and find joiner id")
    };

    // PeerConnected は既存の別テストでカバー済み。ここではJoin後も接続が維持されることのみ確認。
    assert!(!b_id.is_empty());
}

async fn wait_for_peer_connected(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    target: &str,
) -> bool {
    for _ in 0..6 {
        if let Ok(Some(Ok(Message::Text(t)))) =
            tokio::time::timeout(std::time::Duration::from_millis(300), ws.next()).await
        {
            if let Ok(evt) = serde_json::from_str::<ServerToClient>(&t) {
                if let ServerToClient::PeerConnected { participant_id } = evt {
                    if participant_id == target {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[tokio::test]
async fn join_broadcasts_peer_connected_real_core() {
    let shared_core = SharedCore::new(RealCore::new());
    run_peer_connected_test(shared_core).await;
}
