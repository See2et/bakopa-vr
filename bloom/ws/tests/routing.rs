#[path = "common.rs"]
mod common;

use bloom_api::{ErrorCode, ServerToClient};
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::{CoreApi, MockCore, RealCore, SharedCore};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

async fn run_offer_routing_test<C: CoreApi + Send + 'static>(shared_core: SharedCore<C>) {
    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared_core.clone()).await;

    // クライアントA: CreateRoom
    let (mut ws_a, _) = connect_async(&server_url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws_a).await;
    let (room_id_str, a_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // クライアントB: JoinRoom
    let (mut ws_b, _) = connect_async(&server_url).await.expect("connect B");
    ws_b.send(Message::Text(format!(
        r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#,
        room_id = room_id_str
    )))
    .await
    .expect("send join room");
    // join時に流れるPeerConnected/RoomParticipantsを捨てておく
    for _ in 0..3 {
        match tokio::time::timeout(std::time::Duration::from_millis(100), ws_b.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                let _parsed: ServerToClient = serde_json::from_str(&t).expect("parse server msg");
            }
            _ => break,
        }
    }

    // RoomParticipantsからBのIDを取得（A/Bどちらでも可）
    let b_id = {
        let mut found: Option<String> = None;
        for _ in 0..10 {
            for ws in [&mut ws_a, &mut ws_b] {
                if let Ok(Some(Ok(Message::Text(t)))) =
                    tokio::time::timeout(std::time::Duration::from_millis(200), ws.next()).await
                {
                    if let Ok(ServerToClient::RoomParticipants { participants, .. }) =
                        serde_json::from_str::<ServerToClient>(&t)
                    {
                        if let Some(id) = participants.iter().find(|pid| *pid != &a_id) {
                            found = Some(id.clone());
                            break;
                        }
                    }
                }
            }
            if found.is_some() {
                break;
            }
        }
        found.expect("participants include b")
    };

    // A -> B へ Offer
    let offer_json = format!(
        r#"{{"type":"Offer","to":"{to}","sdp":"v=0 offer"}}"#,
        to = b_id
    );
    ws_a.send(Message::Text(offer_json))
        .await
        .expect("send offer");

    let b_msg = recv_server_msg(&mut ws_b).await;
    match b_msg {
        ServerToClient::Offer { from, .. } => assert_eq!(from, a_id),
        other => panic!("expected Offer on B, got {:?}", other),
    }

    // AにはOfferが届かない
    for _ in 0..5 {
        match tokio::time::timeout(std::time::Duration::from_millis(50), ws_a.next()).await {
            Ok(Some(Ok(Message::Text(txt)))) => {
                let msg: ServerToClient =
                    serde_json::from_str(&txt).expect("parse server message on A");
                if matches!(msg, ServerToClient::Offer { .. }) {
                    panic!("A should not receive offer, got {:?}", msg);
                }
            }
            _ => break,
        }
    }

    handle.shutdown().await;
}

async fn run_missing_participant_error<C: CoreApi + Send + 'static>(shared_core: SharedCore<C>) {
    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    let (mut ws, _) = connect_async(&server_url).await.expect("connect A");
    ws.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let _ = recv_server_msg(&mut ws).await;

    let missing_id = uuid::Uuid::new_v4().to_string();
    let offer = format!(
        r#"{{"type":"Offer","to":"{to}","sdp":"v=0 offer"}}"#,
        to = missing_id
    );
    ws.send(Message::Text(offer))
        .await
        .expect("send missing offer");
    let resp = recv_server_msg(&mut ws).await;
    match resp {
        ServerToClient::Error { code, .. } => assert_eq!(code, ErrorCode::ParticipantNotFound),
        other => panic!("expected ParticipantNotFound, got {:?}", other),
    }

    handle.shutdown().await;
}

/// Offerが宛先のみに届く（MockCore）
#[tokio::test]
async fn offer_is_delivered_only_to_target_participant_mock() {
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_join_result(Some(Ok(vec![])));
    let core_arc = std::sync::Arc::new(std::sync::Mutex::new(mock_core));
    let shared_core = bloom_ws::SharedCore::from_arc(core_arc);
    run_offer_routing_test(shared_core).await;
}

/// Offerが宛先のみに届く（RealCore）
#[tokio::test]
async fn offer_is_delivered_only_to_target_participant_real_core() {
    let shared_core = SharedCore::new(RealCore::new());
    run_offer_routing_test(shared_core).await;
}

#[tokio::test]
async fn offer_to_missing_participant_returns_error_mock() {
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_relay_offer_result(Err(ErrorCode::ParticipantNotFound));
    let shared_core = bloom_ws::SharedCore::new(mock_core);
    run_missing_participant_error(shared_core).await;
}

#[tokio::test]
async fn offer_to_missing_participant_returns_error_real_core() {
    let shared_core = SharedCore::new(RealCore::new());
    run_missing_participant_error(shared_core).await;
}
