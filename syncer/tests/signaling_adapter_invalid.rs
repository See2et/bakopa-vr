use bloom_api::payload::RelaySdp;
use bloom_api::ServerToClient;
use bloom_core::{ParticipantId, RoomId};
use syncer::messages::SyncMessageError;
use syncer::signaling_adapter::{
    BloomSignalingAdapter, ClientToServerSender, PeerConnectionCloser, SignalingContext,
};
use syncer::{SyncerError, SyncerEvent};

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
fn invalid_offer_emits_invalid_payload_and_peer_left_then_closes_pc() {
    let room_id = RoomId::new();
    let ctx = SignalingContext {
        room_id: room_id.to_string(),
        auth_token: "INSECURE_DEV".to_string(),
        ice_policy: "default".to_string(),
    };

    let closer = RecordingCloser::default();
    let mut adapter = BloomSignalingAdapter::with_context_and_closer(NoopSender, closer, ctx);

    let remote = ParticipantId::new();
    let valid_offer = ServerToClient::Offer {
        from: remote.to_string(),
        payload: RelaySdp {
            sdp: "v=0\n".into(),
        },
    };
    adapter.push_incoming(valid_offer);
    let poll = adapter.poll();
    let events = poll.events;
    assert!(
        events.is_empty(),
        "first valid offer should not emit errors"
    );

    // invalid offer with empty SDP
    let invalid_offer = ServerToClient::Offer {
        from: remote.to_string(),
        payload: RelaySdp { sdp: String::new() },
    };
    adapter.push_incoming(invalid_offer);

    let poll = adapter.poll();
    assert!(
        poll.payloads.is_empty(),
        "invalid payload should not be forwarded"
    );
    let events = poll.events;

    // expect one InvalidPayload error and one PeerLeft for cleanup
    assert!(events.contains(&SyncerEvent::Error {
        kind: SyncerError::InvalidPayload(SyncMessageError::SchemaViolation {
            kind: "signaling".to_string(),
            reason: syncer::messages::reason::MISSING_SDP,
        }),
    }));

    assert!(events.contains(&SyncerEvent::PeerLeft {
        participant_id: remote.clone(),
    }));

    let closed = adapter.into_inner_closer().closed;
    assert_eq!(
        closed,
        vec![remote],
        "close must be called exactly once on invalid re-offer"
    );
}
