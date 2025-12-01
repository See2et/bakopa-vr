#[path = "common.rs"]
mod common;

use std::str::FromStr;

use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::MockCore;
use futures_util::{SinkExt, StreamExt};
use tokio::time::{self, Duration};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message};

use common::*;

/// 異常切断でleaveが1回だけ呼ばれ、残存参加者へPeerDisconnected/RoomParticipantsが送られることを検証する。
#[tokio::test]
async fn abnormal_close_triggers_single_leave_and_broadcasts() {
    time::pause();

    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_join_result(Some(Ok(vec![])))
    .with_leave_result(Some(vec![ParticipantId::new()]));
    let core_arc = std::sync::Arc::new(std::sync::Mutex::new(mock_core));
    let shared_core = bloom_ws::SharedCore::from_arc(core_arc.clone());

    let (server_url, _handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    // A: CreateRoom
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws_a).await;
    let (room_id_str, a_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // B: JoinRoom
    let (mut ws_b, _) = connect_async(&server_url).await.expect("connect B");
    ws_b.send(Message::Text(format!(
        r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#,
        room_id = room_id_str
    )))
    .await
    .expect("send join room");
    // Join時に流れるPeerConnected/RoomParticipantsをすべて消費しておく
    for _ in 0..3 {
        match tokio::time::timeout(std::time::Duration::from_millis(100), ws_b.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                let _parsed: ServerToClient = serde_json::from_str(&t).expect("parse server msg");
                // 2つ程度のメッセージを想定（PeerConnected, RoomParticipants）。超過しても無視。
            }
            _ => break,
        }
    }

    // Bのparticipant_id取得
    let b_id = {
        let core = core_arc.lock().expect("lock core");
        core.join_room_calls
            .last()
            .map(|(_, p)| p.to_string())
            .expect("b id recorded")
    };

    // leave_room_result をBのみ残る形でセット
    {
        let mut core = core_arc.lock().expect("lock core");
        core.leave_room_result = Some(vec![ParticipantId::from_str(&b_id).expect("parse b id")]);
    }

    // A異常切断（Closeフレーム送信だが猶予適用経路を期待）
    ws_a.close(None).await.expect("close ws_a");

    // 接続終了を検知させる
    tokio::task::yield_now().await;
    time::advance(Duration::from_millis(10)).await;
    tokio::task::yield_now().await;

    // 猶予期間を経過させる
    time::advance(bloom_ws::ABNORMAL_DISCONNECT_GRACE + Duration::from_millis(200)).await;
    time::advance(Duration::from_millis(1)).await;
    tokio::task::yield_now().await;

    // leave_roomが呼ばれているか先に確認（猶予後+余裕分）
    {
        let core = core_arc.lock().expect("lock core");
        assert_eq!(core.leave_room_calls.len(), 1);
    }

    // BがPeerDisconnectedとRoomParticipantsを受信
    let mut received_peer_disconnected = false;
    let mut received_room_participants = false;
    for _ in 0..10 {
        match tokio::time::timeout(std::time::Duration::from_millis(200), ws_b.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                let evt: ServerToClient = serde_json::from_str(&t).expect("parse server msg");
                match evt {
                    ServerToClient::PeerDisconnected { participant_id } => {
                        if participant_id == a_id {
                            received_peer_disconnected = true;
                        }
                    }
                    ServerToClient::RoomParticipants { participants, .. } => {
                        if !participants.contains(&a_id) {
                            received_room_participants = true;
                        }
                    }
                    _ => {}
                }
                if received_peer_disconnected && received_room_participants {
                    break;
                }
            }
            _ => {}
        }
    }

    assert!(
        received_peer_disconnected,
        "B must receive PeerDisconnected for A"
    );
    assert!(
        received_room_participants,
        "B must receive updated RoomParticipants"
    );

    let core = core_arc.lock().expect("lock core");
    assert_eq!(core.leave_room_calls.len(), 1);
}

/// Close(Normal) は異常扱いせず、leave_room を呼ばないことを検証する。
#[tokio::test]
async fn normal_close_does_not_trigger_abnormal_leave() {
    time::pause();

    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_join_result(Some(Ok(vec![])))
    .with_leave_result(Some(vec![ParticipantId::new()]));
    let core_arc = std::sync::Arc::new(std::sync::Mutex::new(mock_core));
    let shared_core = bloom_ws::SharedCore::from_arc(core_arc.clone());

    let (server_url, _handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    // A: CreateRoom
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a
        .send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws_a).await;
    let (room_id_str, _) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // B: JoinRoom
    let (mut ws_b, _) = connect_async(&server_url).await.expect("connect B");
    ws_b
        .send(Message::Text(format!(
            r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#,
            room_id = room_id_str
        )))
        .await
        .expect("send join room");
    // 初期通知を捨てる
    for _ in 0..3 {
        match tokio::time::timeout(std::time::Duration::from_millis(50), ws_b.next()).await {
            Ok(Some(Ok(Message::Text(_)))) => {}
            _ => break,
        }
    }

    // A 正常Close（CloseCode::Normal）
    ws_a
        .close(Some(CloseFrame {
            code: CloseCode::Normal,
            reason: "".into(),
        }))
        .await
        .expect("close ws_a with Normal");

    tokio::task::yield_now().await;
    time::advance(Duration::from_millis(20)).await;
    tokio::task::yield_now().await;

    // 猶予を過ぎても leave_room は呼ばれない
    time::advance(bloom_ws::ABNORMAL_DISCONNECT_GRACE + Duration::from_millis(50)).await;
    tokio::task::yield_now().await;

    let core = core_arc.lock().expect("lock core");
    assert_eq!(core.leave_room_calls.len(), 0);
}

/// 異常切断時にleave_roomを即時呼ばず、猶予時間経過後に呼ぶことを検証する。
#[tokio::test]
async fn abnormal_close_waits_grace_before_leave() {
    time::pause();

    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_join_result(Some(Ok(vec![])))
    .with_leave_result(Some(vec![ParticipantId::new()]));
    let core_arc = std::sync::Arc::new(std::sync::Mutex::new(mock_core));
    let shared_core = bloom_ws::SharedCore::from_arc(core_arc.clone());

    let (server_url, _handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    // A: CreateRoom
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws_a).await;
    let (room_id_str, _) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // B: JoinRoom
    let (mut ws_b, _) = connect_async(&server_url).await.expect("connect B");
    ws_b.send(Message::Text(format!(
        r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#,
        room_id = room_id_str
    )))
    .await
    .expect("send join room");
    // 初期通知を捨てる
    for _ in 0..3 {
        match tokio::time::timeout(std::time::Duration::from_millis(50), ws_b.next()).await {
            Ok(Some(Ok(Message::Text(_)))) => {}
            _ => break,
        }
    }

    // A異常切断（ソケットドロップ＝予期せぬ切断を模擬）
    drop(ws_a);

    // 接続終了を検知させる
    tokio::task::yield_now().await;
    time::advance(Duration::from_millis(10)).await;
    tokio::task::yield_now().await;

    // 100ms待ってもleave_roomが呼ばれないことを期待
    time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;
    {
        let core = core_arc.lock().expect("lock core");
        assert_eq!(
            core.leave_room_calls.len(),
            0,
            "should not call leave_room before grace period"
        );
    }

    // 猶予経過後にleave_roomが1回呼ばれること
    time::advance(bloom_ws::ABNORMAL_DISCONNECT_GRACE).await;
    time::advance(Duration::from_millis(1)).await; // タスクに実行機会を与える
    tokio::task::yield_now().await;
    {
        let core = core_arc.lock().expect("lock core");
        assert_eq!(
            core.leave_room_calls.len(),
            1,
            "should call leave_room after grace period"
        );
    }
}
