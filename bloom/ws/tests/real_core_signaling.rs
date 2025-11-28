#[path = "common.rs"]
mod common;

use bloom_api::{ErrorCode, ServerToClient};
use bloom_ws::{RealCore, SharedCore};
use futures_util::SinkExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// Offer/Answer/ICE が RealCore 経由で宛先に届くことを検証する。
#[tokio::test]
async fn signaling_is_delivered_to_target_via_real_core() {
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

    // skip PeerConnected / RoomParticipants
    let _ = recv_server_msg(&mut ws_a).await;
    let _ = recv_server_msg(&mut ws_a).await;
    let _ = recv_server_msg(&mut ws_b).await;

    // Bのparticipant_idを取得（join呼び出し記録から）
    let b_id = {
        // reuse helper by peeking at participants broadcast to B
        if let ServerToClient::RoomParticipants { participants, .. } =
            recv_server_msg(&mut ws_b).await
        {
            participants
                .into_iter()
                .find(|pid| pid != &a_id)
                .expect("should contain b id")
        } else {
            panic!("expected RoomParticipants for b id")
        }
    };

    // A -> B Offer
    let offer = format!(
        r#"{{"type":"Offer","to":"{to}","sdp":"v=0 offer"}}"#,
        to = b_id
    );
    ws_a.send(Message::Text(offer.into()))
        .await
        .expect("send offer");
    let recv_b = recv_server_msg(&mut ws_b).await;
    match recv_b {
        ServerToClient::Offer { from, payload } => {
            assert_eq!(from, a_id);
            assert_eq!(payload.sdp, "v=0 offer");
        }
        other => panic!("expected Offer on B, got {:?}", other),
    }

    // B -> A Answer
    let answer = format!(
        r#"{{"type":"Answer","to":"{to}","sdp":"v=0 answer"}}"#,
        to = a_id
    );
    ws_b.send(Message::Text(answer.into()))
        .await
        .expect("send answer");
    let recv_a = recv_server_msg(&mut ws_a).await;
    match recv_a {
        ServerToClient::Answer { from, payload } => {
            assert_eq!(from, b_id);
            assert_eq!(payload.sdp, "v=0 answer");
        }
        other => panic!("expected Answer on A, got {:?}", other),
    }

    // A -> B ICE
    let ice = format!(
        r#"{{"type":"IceCandidate","to":"{to}","candidate":"cand1"}}"#,
        to = b_id
    );
    ws_a.send(Message::Text(ice.into()))
        .await
        .expect("send ice");
    let recv_b2 = recv_server_msg(&mut ws_b).await;
    match recv_b2 {
        ServerToClient::IceCandidate { from, payload } => {
            assert_eq!(from, a_id);
            assert_eq!(payload.candidate, "cand1");
        }
        other => panic!("expected IceCandidate on B, got {:?}", other),
    }

    handle.shutdown().await;
}

/// 宛先不在なら ParticipantNotFound を返すことを確認（RealCore）。
#[tokio::test]
async fn signaling_to_missing_participant_returns_error_real_core() {
    let shared = SharedCore::new(RealCore::new());
    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared).await;

    // A: CreateRoom
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let _ = recv_server_msg(&mut ws_a).await;

    // 宛先不在のOffer
    let missing_id = uuid::Uuid::new_v4().to_string();
    let offer = format!(
        r#"{{"type":"Offer","to":"{to}","sdp":"v=0 offer"}}"#,
        to = missing_id
    );
    ws_a.send(Message::Text(offer.into()))
        .await
        .expect("send offer");

    let err = recv_server_msg(&mut ws_a).await;
    match err {
        ServerToClient::Error { code, .. } => assert_eq!(code, ErrorCode::ParticipantNotFound),
        other => panic!("expected Error ParticipantNotFound, got {:?}", other),
    }

    handle.shutdown().await;
}
