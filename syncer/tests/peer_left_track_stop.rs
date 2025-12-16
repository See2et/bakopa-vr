use bloom_core::{ParticipantId, RoomId};
use syncer::{BasicSyncer, Syncer, SyncerEvent, SyncerRequest, Transport, TransportEvent, TransportPayload, TransportSendParams};

/// Transport that emits two Failure events across two polls (simulating track停止や二重通知)。
#[derive(Debug)]
struct DualFailureTransport {
    peer: ParticipantId,
    poll_count: u8,
}

impl DualFailureTransport {
    fn new(peer: ParticipantId) -> Self {
        Self { peer, poll_count: 0 }
    }
}

impl Transport for DualFailureTransport {
    fn register_participant(&mut self, _participant: ParticipantId) {}

    fn send(&mut self, _to: ParticipantId, _payload: TransportPayload, _params: TransportSendParams) {}

    fn poll(&mut self) -> Vec<TransportEvent> {
        self.poll_count += 1;
        match self.poll_count {
            1 => vec![TransportEvent::Failure { peer: self.peer.clone() }],
            2 => vec![TransportEvent::Failure { peer: self.peer.clone() }],
            _ => Vec::new(),
        }
    }
}

/// フェーズ3: track停止などでFailureが2度届いてもPeerLeftは1回だけになることを確認。
#[test]
fn peer_left_emitted_once_when_failure_repeats() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let transport = DualFailureTransport::new(b.clone());
    let mut syncer = BasicSyncer::new(a.clone(), transport);

    // self join
    let mut events = syncer.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });

    // 1回目のpoll（Failure発火）
    events.extend(syncer.handle(SyncerRequest::SendChat {
        chat: syncer::messages::ChatMessage {
            version: 1,
            timestamp_micros: 0,
            sequence_id: 1,
            sender: a.to_string(),
            message: "hello".into(),
        },
        ctx: syncer::TracingContext::for_chat(&room, &a),
    }));

    // 2回目のpoll（同じFailure再発）
    events.extend(syncer.handle(SyncerRequest::SendChat {
        chat: syncer::messages::ChatMessage {
            version: 1,
            timestamp_micros: 0,
            sequence_id: 2,
            sender: a.to_string(),
            message: "hello again".into(),
        },
        ctx: syncer::TracingContext::for_chat(&room, &a),
    }));

    let peer_left_count = events
        .iter()
        .filter(|ev| matches!(ev, SyncerEvent::PeerLeft { participant_id } if participant_id == &b))
        .count();

    assert_eq!(
        peer_left_count, 1,
        "PeerLeft for the same participant should be emitted only once even with repeated failures"
    );
}
