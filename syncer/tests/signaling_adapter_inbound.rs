use bloom_api::payload::{RelayIce, RelaySdp};
use bloom_api::ServerToClient;
use bloom_core::{ParticipantId, RoomId};
use syncer::messages::{SignalingMessage, SyncMessage};
use syncer::signaling_adapter::{BloomSignalingAdapter, ClientToServerSender, SignalingContext};

#[derive(Default)]
struct NoopSender;

impl ClientToServerSender for NoopSender {
    fn send(&mut self, _message: bloom_api::ClientToServer) {}
}

#[test]
fn inbound_offer_is_wrapped_into_sync_message_envelope() {
    let room_id = RoomId::new();
    let ctx = SignalingContext {
        room_id: room_id.to_string(),
        auth_token: "INSECURE_DEV".to_string(),
        ice_policy: "default".to_string(),
    };

    let mut adapter: BloomSignalingAdapter<NoopSender> =
        BloomSignalingAdapter::with_context(NoopSender::default(), ctx);

    let remote = ParticipantId::new();
    let raw = ServerToClient::Offer {
        from: remote.to_string(),
        payload: RelaySdp {
            sdp: "v=0\n".into(),
        },
    };

    adapter.push_incoming(raw);

    let poll = adapter.poll();
    let mut polled = poll.payloads;
    let events = poll.events;
    assert!(events.is_empty());
    assert_eq!(polled.len(), 1, "one payload should be produced");

    let payload = polled.pop().unwrap();
    let parsed = payload.parse_sync_message().expect("parse sync message");

    match parsed {
        SyncMessage::Signaling(SignalingMessage::Offer(offer)) => {
            assert_eq!(offer.version, 1);
            assert_eq!(offer.room_id, room_id.to_string());
            assert_eq!(offer.participant_id, remote.to_string());
            assert_eq!(offer.auth_token, "INSECURE_DEV");
            assert_eq!(offer.ice_policy, "default");
            assert_eq!(offer.sdp, "v=0\n");
        }
        other => panic!("unexpected message: {other:?}"),
    }
}

#[test]
fn inbound_answer_and_ice_are_also_wrapped() {
    let room_id = RoomId::new();
    let ctx = SignalingContext {
        room_id: room_id.to_string(),
        auth_token: "INSECURE_DEV".to_string(),
        ice_policy: "default".to_string(),
    };

    let mut adapter: BloomSignalingAdapter<NoopSender> =
        BloomSignalingAdapter::with_context(NoopSender::default(), ctx);

    let remote = ParticipantId::new();

    adapter.push_incoming(ServerToClient::Answer {
        from: remote.to_string(),
        payload: RelaySdp {
            sdp: "v=0\n".into(),
        },
    });

    adapter.push_incoming(ServerToClient::IceCandidate {
        from: remote.to_string(),
        payload: RelayIce {
            candidate: "candidate:1 1 udp 2122260223 192.0.2.1 54400 typ host".into(),
        },
    });

    let poll = adapter.poll();
    let polled = poll.payloads;
    let events = poll.events;
    assert!(events.is_empty());
    assert_eq!(polled.len(), 2);

    let messages: Vec<_> = polled
        .into_iter()
        .map(|p| p.parse_sync_message().unwrap())
        .collect();

    assert!(matches!(messages[0], SyncMessage::Signaling(SignalingMessage::Answer(_))));
    assert!(matches!(messages[1], SyncMessage::Signaling(SignalingMessage::Ice(_))));
}
