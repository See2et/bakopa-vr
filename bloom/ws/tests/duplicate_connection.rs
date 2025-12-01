#[path = "common.rs"]
mod common;

use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::{MockCore, SharedCore};
use futures_util::{SinkExt, StreamExt};
use std::env;
use std::sync::OnceLock;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

static ENV_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

async fn env_lock() -> tokio::sync::MutexGuard<'static, ()> {
    ENV_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
}

use common::*;

/// 同一participant_idで新規接続したとき旧接続が優先的に切断されることを検証する
#[tokio::test]
async fn duplicate_participant_connection_disconnects_old_session() {
    let _env_guard = env_lock().await;
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

/// 旧接続が切断された後も新接続がブロードキャスト先として登録され続けることを検証する。
#[tokio::test]
async fn duplicate_participant_keeps_new_session_registered_for_broadcast() {
    let _env_guard = env_lock().await;
    let fixed_id = ParticipantId::new().to_string();
    env::set_var("BLOOM_TEST_PARTICIPANT_ID", &fixed_id);

    // 実Coreで参加者リストを自然に管理させる
    let shared_core = SharedCore::new(bloom_ws::RealCore::new());
    let (server_url, _handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    // 1本目の接続（旧）
    let (mut ws_old, _) = connect_async(&server_url).await.expect("connect old");

    // 2本目の接続（新）— この時点で旧接続へのCloseが飛ぶ
    let (mut ws_new, _) = connect_async(&server_url).await.expect("connect new");

    // 旧接続がCloseされるのを確認しておく（その後の残存処理を完了させるため）
    let _ = tokio::time::timeout(std::time::Duration::from_millis(200), async {
        loop {
            match ws_old.next().await {
                Some(Ok(Message::Close(_))) => break,
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => continue,
                Some(Ok(_)) => continue,
                Some(Err(_)) => break,
                None => break,
            }
        }
    })
    .await;

    // 新接続でRoomを作成
    ws_new
        .send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room on new");
    let room_created = recv_server_msg(&mut ws_new).await;
    let room_id = match room_created {
        ServerToClient::RoomCreated { room_id, .. } => room_id,
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // joinerは別participantにするため、固定ID設定を解除する
    env::remove_var("BLOOM_TEST_PARTICIPANT_ID");

    // 別participantでJoinし、オーナー（新接続）へブロードキャストされることを期待
    let (mut ws_joiner, _) = connect_async(&server_url).await.expect("connect joiner");
    ws_joiner
        .send(Message::Text(format!(
            r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#
        )))
        .await
        .expect("send join");
    let _ = recv_server_msg(&mut ws_joiner).await; // joiner側の初回応答を消費

    // オーナー側でPeerConnectedまたはRoomParticipantsを受信できるか確認
    let received = tokio::time::timeout(std::time::Duration::from_millis(500), async {
        loop {
            match ws_new.next().await {
                Some(Ok(Message::Text(t))) => {
                    if let Ok(evt) = serde_json::from_str::<ServerToClient>(&t) {
                        match evt {
                            ServerToClient::PeerConnected { .. } => return true,
                            ServerToClient::RoomParticipants { participants, .. } => {
                                if participants.len() >= 2 {
                                    return true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => break,
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    assert!(
        received,
        "new session should stay registered and receive broadcast after duplicate close"
    );

    env::remove_var("BLOOM_TEST_PARTICIPANT_ID");
}
