#[path = "common.rs"]
mod common;

use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::MockCore;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// Join成功時にPeerConnectedがroom内全員（既存参加者とJoinした本人）へブロードキャストされることを検証する。
#[tokio::test]
async fn join_broadcasts_peer_connected_to_all_members() {
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_join_result(Some(Ok(vec![]))); // Join時は内部でowner + joinerを返すデフォルト分岐を使う

    let core_arc = std::sync::Arc::new(std::sync::Mutex::new(mock_core));
    let shared_core = bloom_ws::SharedCore::from_arc(core_arc.clone());
    let (server_url, _handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    // A: CreateRoom
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws_a).await;
    let (room_id, _a_id) = match room_created {
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

    // join_room_calls から参加者IDを取得
    let b_id = loop {
        if let Some((_, p)) = core_arc
            .lock()
            .expect("lock core")
            .join_room_calls
            .last()
            .cloned()
        {
            break p.to_string();
        }
        // 少し待って再度確認（非同期でjoinが届くため）
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };

    // AがPeerConnected(new B)を受信すること
    let mut got_peer_connected_a = false;
    for _ in 0..6 {
        if let Ok(Some(Ok(Message::Text(t)))) =
            tokio::time::timeout(std::time::Duration::from_millis(200), ws_a.next()).await
        {
            if let Ok(evt) = serde_json::from_str::<ServerToClient>(&t) {
                if let ServerToClient::PeerConnected { participant_id } = evt {
                    if participant_id == b_id {
                        got_peer_connected_a = true;
                        break;
                    }
                }
            }
        }
    }

    // B自身もPeerConnected(B)を受信すること（room内全員通知）
    let mut got_peer_connected_b = false;
    for _ in 0..6 {
        if let Ok(Some(Ok(Message::Text(t)))) =
            tokio::time::timeout(std::time::Duration::from_millis(200), ws_b.next()).await
        {
            if let Ok(evt) = serde_json::from_str::<ServerToClient>(&t) {
                match evt {
                    ServerToClient::PeerConnected { participant_id } if participant_id == b_id => {
                        got_peer_connected_b = true;
                        break;
                    }
                    // RoomParticipantsなど他イベントは無視して継続
                    _ => {}
                }
            }
        }
    }

    assert!(
        got_peer_connected_a,
        "creator should receive PeerConnected for new joiner"
    );
    assert!(
        got_peer_connected_b,
        "joiner should also receive PeerConnected for themselves"
    );
}
