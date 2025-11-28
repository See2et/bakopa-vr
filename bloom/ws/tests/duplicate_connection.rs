#[path = "common.rs"]
mod common;

use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::MockCore;
use futures_util::{SinkExt, StreamExt};
use std::env;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// 同一participant_idで新規接続したとき旧接続が優先的に切断されることを検証する
#[tokio::test]
async fn duplicate_participant_connection_disconnects_old_session() {
    let fixed_id = ParticipantId::new().to_string();
    env::set_var("BLOOM_TEST_PARTICIPANT_ID", &fixed_id);

    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    });
    let core_arc = std::sync::Arc::new(std::sync::Mutex::new(mock_core));
    let shared_core = bloom_ws::SharedCore::from_arc(core_arc);

    let (server_url, _handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    // 1本目の接続（旧接続）
    let (mut ws_old, _) = connect_async(&server_url).await.expect("connect old");

    // 2本目の接続（新接続）
    let (mut ws_new, _) = connect_async(&server_url).await.expect("connect new");

    // 仕様どおりなら、旧接続は優先的にCloseされるはず。
    // 一定時間内に旧接続でCloseを受信することを期待する。
    let old_closed = tokio::time::timeout(std::time::Duration::from_millis(200), async {
        loop {
            match ws_old.next().await {
                Some(Ok(Message::Close(_))) => return true,
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => continue,
                Some(Ok(_)) => continue,
                Some(Err(_)) => return true,
                None => return true,
            }
        }
    })
    .await
    .unwrap_or(false);

    assert!(
        old_closed,
        "old connection should be closed when a duplicate participant connects"
    );

    // 新接続はまだ開いていることを確認（何か1メッセージ送って応答がない程度で良い）
    ws_new
        .send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room on new");
    let reply = recv_server_msg(&mut ws_new).await;
    match reply {
        ServerToClient::RoomCreated { .. } => {}
        other => panic!("new connection should remain active, got {:?}", other),
    }

    env::remove_var("BLOOM_TEST_PARTICIPANT_ID");
}
