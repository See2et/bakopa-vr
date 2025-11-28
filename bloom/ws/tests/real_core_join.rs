#[path = "common.rs"]
mod common;

use bloom_api::ServerToClient;
use bloom_ws::{RealCore, SharedCore};
use futures_util::SinkExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// RealCore 経由で JoinRoom したとき、RoomParticipants が全員に届くことを検証する。
#[tokio::test]
async fn join_broadcasts_room_participants_with_real_core() {
    let shared = SharedCore::new(RealCore::new());
    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared).await;

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
        r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#
    )))
    .await
    .expect("send join room");

    // 最初にPeerConnectedが来るので読み飛ばす
    let _ = recv_server_msg(&mut ws_a).await;
    let _ = recv_server_msg(&mut ws_b).await;

    // 次に RoomParticipants が A と B の両方に届くことを確認
    let participants_a = recv_server_msg(&mut ws_a).await;
    let participants_b = recv_server_msg(&mut ws_b).await;

    for msg in [participants_a, participants_b] {
        match msg {
            ServerToClient::RoomParticipants {
                room_id: rid,
                participants,
            } => {
                assert_eq!(rid, room_id);
                assert_eq!(participants.len(), 2);
                assert!(participants.contains(&a_id));
                // B の ID はcore内部で払い出されるため2件であることのみ確認
            }
            other => panic!("expected RoomParticipants, got {:?}", other),
        }
    }

    handle.shutdown().await;
}
