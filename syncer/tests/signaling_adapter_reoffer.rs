use bloom_api::payload::RelaySdp;
use bloom_api::ServerToClient;
use bloom_core::{ParticipantId, RoomId};
use syncer::signaling_adapter::{BloomSignalingAdapter, ClientToServerSender, PeerConnectionCloser, SignalingContext};
use syncer::SyncerEvent;

#[derive(Default)]
struct RecordingCloser {
    closed: Vec<ParticipantId>,
}

impl PeerConnectionCloser for RecordingCloser {
    fn close(&mut self, participant: &ParticipantId) {
        self.closed.push(participant.clone());
    }
}

#[derive(Default)]
struct NoopSender;

impl ClientToServerSender for NoopSender {
    fn send(&mut self, _message: bloom_api::ClientToServer) {}
}

#[test]
fn reoffer_closes_previous_peer_connection_and_emits_peer_left() {
    let room_id = RoomId::new();
    let ctx = SignalingContext {
        room_id: room_id.to_string(),
        auth_token: "INSECURE_DEV".to_string(),
        ice_policy: "default".to_string(),
    };

    let closer = RecordingCloser::default();
    let mut adapter = BloomSignalingAdapter::with_context_and_closer(NoopSender::default(), closer, ctx);

    let remote = ParticipantId::new();

    let offer = |sdp: &str| ServerToClient::Offer {
        from: remote.to_string(),
        payload: RelaySdp { sdp: sdp.to_string() },
    };

    // 1回目のOffer
    adapter.push_incoming(offer("v=0\no=- 0 0 IN IP4 127.0.0.1\n"));
    let (_payloads, events) = adapter.poll();
    assert!(events.is_empty(), "initial offer should not emit PeerLeft");

    // 2回目のOffer（再接続）
    adapter.push_incoming(offer("v=0\no=- 1 1 IN IP4 127.0.0.1\n"));
    let (_payloads, events) = adapter.poll();

    assert!(
        events.contains(&SyncerEvent::PeerLeft {
            participant_id: remote.clone(),
        }),
        "re-offer must emit PeerLeft for the old session"
    );

    let closed = adapter.into_inner_closer().closed;
    assert_eq!(closed, vec![remote], "close should be called exactly once for the participant");
}
