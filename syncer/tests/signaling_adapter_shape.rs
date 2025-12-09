use bloom_core::ParticipantId;
use syncer::messages::{SignalingAnswer, SignalingIce, SignalingOffer};
use syncer::SignalingAdapter;

#[derive(Default)]
struct RecordingSignalingAdapter {
    calls: Vec<Call>,
}

#[derive(Debug, PartialEq)]
enum Call {
    Offer { to: ParticipantId, payload: SignalingOffer },
    Answer { to: ParticipantId, payload: SignalingAnswer },
    Ice { to: ParticipantId, payload: SignalingIce },
}

impl SignalingAdapter for RecordingSignalingAdapter {
    fn send_offer(&mut self, to: ParticipantId, payload: SignalingOffer) {
        self.calls.push(Call::Offer { to, payload });
    }

    fn send_answer(&mut self, to: ParticipantId, payload: SignalingAnswer) {
        self.calls.push(Call::Answer { to, payload });
    }

    fn send_ice(&mut self, to: ParticipantId, payload: SignalingIce) {
        self.calls.push(Call::Ice { to, payload });
    }
}

#[test]
fn signaling_adapter_records_calls_in_order() {
    let mut adapter = RecordingSignalingAdapter::default();

    let alice = ParticipantId::new();
    let bob = ParticipantId::new();
    let carol = ParticipantId::new();

    let offer = SignalingOffer {
        version: 1,
        room_id: "room-123".into(),
        participant_id: "alice".into(),
        auth_token: "INSECURE_DEV".into(),
        ice_policy: "default".into(),
        sdp: "v=0\n".into(),
    };

    let answer = SignalingAnswer {
        version: 1,
        room_id: "room-123".into(),
        participant_id: "bob".into(),
        auth_token: "INSECURE_DEV".into(),
        sdp: "v=0\n".into(),
    };

    let ice = SignalingIce {
        version: 1,
        room_id: "room-123".into(),
        participant_id: "carol".into(),
        auth_token: "INSECURE_DEV".into(),
        candidate: "candidate:1 1 udp 2122260223 192.0.2.1 54400 typ host".into(),
        sdp_mid: Some("0".into()),
        sdp_mline_index: Some(0),
    };

    adapter.send_offer(alice.clone(), offer.clone());
    adapter.send_answer(bob.clone(), answer.clone());
    adapter.send_ice(carol.clone(), ice.clone());

    assert_eq!(
        adapter.calls,
        vec![
            Call::Offer {
                to: alice,
                payload: offer,
            },
            Call::Answer {
                to: bob,
                payload: answer,
            },
            Call::Ice {
                to: carol,
                payload: ice,
            },
        ]
    );
}
