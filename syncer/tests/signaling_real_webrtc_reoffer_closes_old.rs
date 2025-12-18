use bloom_api::payload::RelaySdp;
use bloom_api::ServerToClient;
use bloom_core::ParticipantId;
use std::str::FromStr;
use syncer::signaling_adapter::{
    BloomSignalingAdapter, ClientToServerSender, PeerConnectionCloser, SignalingContext,
};
use syncer::SyncerEvent;

#[derive(Default)]
struct NoopSender;
impl ClientToServerSender for NoopSender {
    fn send(&mut self, _message: bloom_api::ClientToServer) {}
}

#[derive(Default)]
struct RecordingCloser {
    closed: Vec<ParticipantId>,
}

impl PeerConnectionCloser for RecordingCloser {
    fn close(&mut self, participant: &ParticipantId) {
        self.closed.push(participant.clone());
    }
}

/// Red→Green: 同一participantの再Offerで旧PCをcloseし、PeerLeft→PeerJoinedが1セットだけ出る。
#[test]
fn reoffer_closes_old_pc_and_emits_left_then_joined() {
    let ctx = SignalingContext {
        room_id: "room-x".into(),
        auth_token: "INSECURE_DEV".into(),
        ice_policy: "default".into(),
    };

    let mut adapter = BloomSignalingAdapter::with_context_and_closer(
        NoopSender::default(),
        RecordingCloser::default(),
        ctx,
    );

    let pid = ParticipantId::new().to_string();

    let offer = |sdp: &str| ServerToClient::Offer {
        from: pid.clone(),
        payload: RelaySdp {
            sdp: sdp.to_string(),
        },
    };

    // 初回Offer: イベントなし
    adapter.push_incoming(offer("v=0\no=- 0 0 IN IP4 127.0.0.1\n"));
    let poll1 = adapter.poll();
    assert!(
        poll1.events.is_empty(),
        "first offer should not emit peer events"
    );

    // 再Offer: PeerLeft→PeerJoinedを期待
    adapter.push_incoming(offer("v=0\no=- 1 1 IN IP4 127.0.0.1\n"));
    let poll2 = adapter.poll();

    let events = poll2.events;
    assert!(
        events.len() >= 2,
        "re-offer should emit at least two events (PeerLeft, PeerJoined)"
    );
    assert!(
        matches!(events[0], SyncerEvent::PeerLeft { ref participant_id } if participant_id == &ParticipantId::from_str(&pid).unwrap()),
        "first event should be PeerLeft for the participant"
    );
    assert!(
        matches!(events[1], SyncerEvent::PeerJoined { ref participant_id } if participant_id == &ParticipantId::from_str(&pid).unwrap()),
        "second event should be PeerJoined for the participant"
    );

    // closer が一度だけ呼ばれていること
    let closer = adapter.into_inner_closer();
    assert_eq!(
        closer.closed,
        vec![ParticipantId::from_str(&pid).unwrap()],
        "closer should be called once for the participant"
    );
}
