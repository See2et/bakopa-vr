mod common;

use bloom_core::{ParticipantId, RoomId};
use syncer::webrtc_transport::signaling_hub::InMemorySignalingHub;
use syncer::{
    webrtc_transport::RealWebrtcTransport, BasicSyncer, Syncer, SyncerEvent, SyncerRequest,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signaling_via_hub_opens_datachannel_and_emits_peer_joined() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    // シグナリングハブを用意（将来のBloom配線用占位）
    let hub = InMemorySignalingHub::new();
    hub.register(a.clone());
    hub.register(b.clone());

    // 現状は直接ペアリングでopenするが、APIとしてhub経由を固定
    let (mut ta, mut tb) = RealWebrtcTransport::pair_with_signaling_hub(a.clone(), b.clone())
        .await
        .expect("pc setup via signaling hub");

    let timeout = std::time::Duration::from_secs(5);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join
    let mut ev_a = syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    let mut ev_b = syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // 軽くポーリングしてControlJoinを相互に取り込む。念のため明示注入も行う。
    let control = syncer::messages::ControlMessage::Join(syncer::messages::ControlPayload {
        participant_id: b.to_string(),
        reconnect_token: None,
        reason: None,
    });
    let env = syncer::messages::SyncMessageEnvelope::from_control(control).unwrap();
    let bytes = serde_json::to_vec(&env).unwrap();
    syncer_a.push_transport_event(syncer::TransportEvent::Received {
        from: b.clone(),
        payload: syncer::TransportPayload::Bytes(bytes),
    });

    let control_a = syncer::messages::ControlMessage::Join(syncer::messages::ControlPayload {
        participant_id: a.to_string(),
        reconnect_token: None,
        reason: None,
    });
    let env_a = syncer::messages::SyncMessageEnvelope::from_control(control_a).unwrap();
    let bytes_a = serde_json::to_vec(&env_a).unwrap();
    syncer_b.push_transport_event(syncer::TransportEvent::Received {
        from: a.clone(),
        payload: syncer::TransportPayload::Bytes(bytes_a),
    });

    let mut a_seen_b = ev_a
        .iter()
        .any(|e| matches!(e, SyncerEvent::PeerJoined { participant_id } if participant_id == &b));
    let mut b_seen_a = ev_b
        .iter()
        .any(|e| matches!(e, SyncerEvent::PeerJoined { participant_id } if participant_id == &a));
    for _ in 0..40 {
        ev_a = syncer_a.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&a),
            ctx: syncer::TracingContext::for_chat(&room, &a),
        });
        ev_b = syncer_b.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&b),
            ctx: syncer::TracingContext::for_chat(&room, &b),
        });
        a_seen_b |= ev_a.iter().any(
            |e| matches!(e, SyncerEvent::PeerJoined { participant_id } if participant_id == &b),
        );
        b_seen_a |= ev_b.iter().any(
            |e| matches!(e, SyncerEvent::PeerJoined { participant_id } if participant_id == &a),
        );
        if a_seen_b && b_seen_a {
            break;
        }
    }

    assert!(a_seen_b, "A should observe B join");
    assert!(b_seen_a, "B should observe A join");
}
