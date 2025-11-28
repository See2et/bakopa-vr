#[path = "common.rs"]
mod common;

use std::sync::{Arc, Mutex};

use bloom_api::{RelayIce, ServerToClient};
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::MockCore;
use futures_util::SinkExt;
use tokio_tungstenite::connect_async;
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
    // join処理完了を待つ
    let _ = recv_server_msg(&mut ws_b).await;

    // 参加者IDをコア呼び出しから取得
    let b_id = {
        let core = core_arc.lock().expect("lock core");
        core.join_room_calls
            .last()
            .map(|(_, p)| p.to_string())
            .expect("b id recorded")
    };

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
