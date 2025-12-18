use bloom_core::{ParticipantId, RoomId};
use syncer::{
    BasicSyncer, Syncer, SyncerEvent, SyncerRequest, Transport, TransportEvent, TransportPayload,
    TransportSendParams,
};

/// テスト用: pollの最初の呼び出しで同じpeerのFailureを2回返すTransport。
#[derive(Debug)]
struct FailingTransport {
    peer: ParticipantId,
    emitted: bool,
}

impl FailingTransport {
    fn new(peer: ParticipantId) -> Self {
        Self {
            peer,
            emitted: false,
        }
    }
}

impl Transport for FailingTransport {
    fn register_participant(&mut self, _participant: ParticipantId) {}

    fn send(
        &mut self,
        _to: ParticipantId,
        _payload: TransportPayload,
        _params: TransportSendParams,
    ) {
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        self.emitted = true;
        vec![
            TransportEvent::Failure {
                peer: self.peer.clone(),
            },
            TransportEvent::Failure {
                peer: self.peer.clone(),
            },
        ]
    }
}

#[test]
fn failure_events_are_deduped_to_single_peer_left() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let transport = FailingTransport::new(b.clone());
    let mut syncer = BasicSyncer::new(a.clone(), transport);

    // join self
    let mut all_events = syncer.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });

    // 任意リクエストでpollを走らせ、Failure x2 を取り込む
    let events = syncer.handle(SyncerRequest::SendChat {
        chat: syncer::messages::ChatMessage {
            version: 1,
            timestamp_micros: 0,
            sequence_id: 1,
            sender: a.to_string(),
            message: "hello".into(),
        },
        ctx: syncer::TracingContext::for_chat(&room, &a),
    });
    all_events.extend(events);

    let peer_left_count = all_events
        .iter()
        .filter(|ev| matches!(ev, SyncerEvent::PeerLeft { participant_id } if participant_id == &b))
        .count();

    assert_eq!(
        peer_left_count, 1,
        "PeerLeft for the same participant should be emitted only once"
    );
}
