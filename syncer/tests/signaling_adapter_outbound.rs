use bloom_api::payload::{RelayIce, RelaySdp};
use bloom_api::ClientToServer;
use bloom_core::ParticipantId;
use syncer::messages::{SignalingAnswer, SignalingIce, SignalingOffer};
use syncer::signaling_adapter::{BloomSignalingAdapter, ClientToServerSender};
use syncer::SignalingAdapter;

#[derive(Default)]
struct RecordingSender {
    outbox: Vec<ClientToServer>,
}

impl ClientToServerSender for RecordingSender {
    fn send(&mut self, message: ClientToServer) {
        self.outbox.push(message);
    }
}

#[test]
fn outbound_signaling_is_transcoded_to_bloom_messages() {
    let sink = RecordingSender::default();
    let mut adapter = BloomSignalingAdapter::new(sink);

    let to_offer = ParticipantId::new();
    let to_answer = ParticipantId::new();
    let to_ice = ParticipantId::new();

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

    adapter.send_offer(to_offer.clone(), offer.clone());
    adapter.send_answer(to_answer.clone(), answer.clone());
    adapter.send_ice(to_ice.clone(), ice.clone());

    let outbox = adapter.into_inner().outbox;

    assert_eq!(
        outbox,
        vec![
            ClientToServer::Offer {
                to: to_offer.to_string(),
                payload: RelaySdp {
                    sdp: offer.sdp,
                },
            },
            ClientToServer::Answer {
                to: to_answer.to_string(),
                payload: RelaySdp {
                    sdp: answer.sdp,
                },
            },
            ClientToServer::IceCandidate {
                to: to_ice.to_string(),
                payload: RelayIce {
                    candidate: ice.candidate,
                },
            },
        ]
    );
}
