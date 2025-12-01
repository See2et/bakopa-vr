#[path = "common.rs"]
mod common;

use std::sync::{Arc, Mutex};

use bloom_api::{RelayIce, ServerToClient};
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::MockCore;
use futures_util::{SinkExt, StreamExt};
use tokio::net::UdpSocket;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// TURN候補を含むIceCandidateが改変されずに宛先へ届くことを検証する。
#[tokio::test]
async fn ice_candidate_is_forwarded_without_mutation() {
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_join_result(Some(Ok(vec![])));
    let core_arc = Arc::new(Mutex::new(mock_core));
    let shared_core = bloom_ws::SharedCore::from_arc(core_arc.clone());

    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared_core.clone()).await;
    let (mut ws_a, mut ws_b, _room_id_str, a_id, b_id) =
        setup_room_with_two_clients(&server_url, &core_arc).await;

    // join時のPeerConnected/RoomParticipantsをB側で捨てておく
    for _ in 0..3 {
        match tokio::time::timeout(std::time::Duration::from_millis(100), ws_b.next()).await {
            Ok(Some(Ok(Message::Text(_)))) => {}
            _ => break,
        }
    }

    // A -> B へ TURN候補を含む IceCandidate
    let candidate =
        "candidate:842163049 1 udp 1677729535 192.0.2.3 54400 typ relay raddr 0.0.0.0 rport 0";
    let ice_json = format!(
        r#"{{"type":"IceCandidate","to":"{to}","candidate":"{candidate}"}}"#,
        to = b_id,
        candidate = candidate
    );
    ws_a.send(Message::Text(ice_json.into()))
        .await
        .expect("send ice candidate");

    let b_msg = recv_server_msg(&mut ws_b).await;
    match b_msg {
        ServerToClient::IceCandidate { from, payload } => {
            assert_eq!(from, a_id);
            assert_eq!(payload.candidate, candidate);
        }
        other => panic!("expected IceCandidate on B, got {:?}", other),
    }

    // コアに渡されたペイロードも改変されていないことを確認
    let core = core_arc.lock().expect("lock core");
    assert_eq!(core.relay_ice_calls.len(), 1);
    let (_room, from, to, RelayIce { candidate: relayed }) = core.relay_ice_calls[0].clone();
    assert_eq!(from.to_string(), a_id);
    assert_eq!(to.to_string(), b_id);
    assert_eq!(relayed, candidate);

    handle.shutdown().await;
}

/// バイナリフレームを送ってもメディア扱いせず、1003 Closeで拒否しRoomParticipantsを変化させない。
#[tokio::test]
async fn binary_frame_is_rejected_without_room_participants_change() {
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    })
    .with_join_result(Some(Ok(vec![])));
    let core_arc = Arc::new(Mutex::new(mock_core));
    let shared_core = bloom_ws::SharedCore::from_arc(core_arc.clone());

    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared_core.clone()).await;
    let (mut ws_a, mut ws_b, _room_id_str, _a_id, _b_id) =
        setup_room_with_two_clients(&server_url, &core_arc).await;

    // A側に溜まっている（Joinによる）RoomParticipantsを捨てておく
    while let Ok(Some(Ok(Message::Text(_)))) =
        tokio::time::timeout(std::time::Duration::from_millis(50), ws_a.next()).await
    {
        // drain
    }
    // B側もJoin時の通知を捨てる
    while let Ok(Some(Ok(Message::Text(_)))) =
        tokio::time::timeout(std::time::Duration::from_millis(50), ws_b.next()).await
    {
        // drain
    }

    // Bがバイナリ（疑似RTP）を送信
    ws_b.send(Message::Binary(vec![0u8, 1, 2, 3, 4]))
        .await
        .expect("send binary frame");

    // サーバから1003 Closeを受信すること
    let mut close_ok = false;
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(200) {
        if let Some(msg) = ws_b.next().await {
            match msg {
                Ok(Message::Close(Some(frame))) => {
                    assert_eq!(frame.code, CloseCode::Unsupported);
                    close_ok = true;
                    break;
                }
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
                Ok(other) => panic!("unexpected message before close: {:?}", other),
                Err(e) => panic!("ws error before close: {:?}", e),
            }
        } else {
            break;
        }
    }
    assert!(close_ok, "server should send Close(1003) for binary frame");

    // A側はRoomParticipantsを受け取らないことを確認（短時間待機）
    if let Ok(Some(Ok(Message::Text(txt)))) =
        tokio::time::timeout(std::time::Duration::from_millis(100), ws_a.next()).await
    {
        if let Ok(parsed) = serde_json::from_str::<ServerToClient>(&txt) {
            if matches!(parsed, ServerToClient::RoomParticipants { .. }) {
                panic!("RoomParticipants should not be broadcast on binary rejection");
            }
        }
    }

    // core.leave_roomが呼ばれていない（参加者状態を変化させない）
    let core = core_arc.lock().expect("lock core");
    assert!(
        core.leave_room_calls.is_empty(),
        "binary rejection should not invoke leave_room"
    );

    handle.shutdown().await;
}

/// WSポートをBloom自身がUDPで占有していないことを確認する（同ポートでUDPバインドできる）。
#[tokio::test]
async fn udp_socket_bind_on_ws_port_succeeds() {
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    });
    let shared_core = bloom_ws::SharedCore::new(mock_core);

    let (_server_url, handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    let udp_bind = UdpSocket::bind(handle.addr).await;
    assert!(
        udp_bind.is_ok(),
        "UDP bind on ws port should succeed because Bloom does not open UDP sockets"
    );

    handle.shutdown().await;
}
