use bloom_api::payload::RelaySdp;
use bloom_api::ServerToClient;
use bloom_core::{ParticipantId, RoomId};
use syncer::signaling_adapter::{
    BloomSignalingAdapter, ClientToServerSender, PeerConnectionCloser, SignalingContext,
};
use syncer::{
    BasicSyncer, Syncer, SyncerEvent, SyncerRequest, Transport, TransportEvent, TransportPayload,
    TransportSendParams,
};

/// テスト用: 送信は捨て、キューに積んだ受信イベントをpollで返すだけのフェイクTransport。
#[derive(Default)]
struct QueueTransport {
    events: std::collections::VecDeque<TransportEvent>,
}

impl QueueTransport {
    fn with_event(ev: TransportEvent) -> Self {
        let mut q = std::collections::VecDeque::new();
        q.push_back(ev);
        Self { events: q }
    }
}

impl Transport for QueueTransport {
    fn register_participant(&mut self, _participant: ParticipantId) {}

    fn send(
        &mut self,
        _to: ParticipantId,
        _payload: TransportPayload,
        _params: TransportSendParams,
    ) {
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        self.events.drain(..).collect()
    }
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

#[derive(Default)]
struct NoopSender;

impl ClientToServerSender for NoopSender {
    fn send(&mut self, _message: bloom_api::ClientToServer) {}
}

#[test]
fn reoffer_emits_peer_left_before_peer_joined_in_order() {
    let room_id = RoomId::new();
    let remote = ParticipantId::new();
    let local = ParticipantId::new();

    let ctx = SignalingContext {
        room_id: room_id.to_string(),
        auth_token: "INSECURE_DEV".to_string(),
        ice_policy: "default".to_string(),
    };

    let closer = RecordingCloser::default();
    let mut adapter =
        BloomSignalingAdapter::with_context_and_closer(NoopSender::default(), closer, ctx);

    let offer = |sdp: &str| ServerToClient::Offer {
        from: remote.to_string(),
        payload: RelaySdp {
            sdp: sdp.to_string(),
        },
    };

    // 1回目のOffer（初回接続）: イベントなし
    adapter.push_incoming(offer("v=0\no=- 0 0 IN IP4 127.0.0.1\n"));
    let poll1 = adapter.poll();
    assert!(
        poll1.events.is_empty(),
        "first offer should not emit events"
    );

    // 2回目のOffer（再接続）: PeerLeft が発火し、ペイロードも返るはず
    adapter.push_incoming(offer("v=0\no=- 1 1 IN IP4 127.0.0.1\n"));
    let poll2 = adapter.poll();

    let mut events = poll2.events;
    assert!(
        events.iter().any(
            |e| matches!(e, SyncerEvent::PeerLeft { participant_id } if participant_id == &remote)
        ),
        "reoffer must emit PeerLeft for the old session"
    );

    // 生成されたペイロードを QueueTransport 経由で BasicSyncer に流し込む
    let payload = poll2
        .payloads
        .into_iter()
        .next()
        .expect("payload from reoffer");
    let transport = QueueTransport::with_event(TransportEvent::Received {
        from: remote.clone(),
        payload,
    });

    let mut syncer = BasicSyncer::new(local.clone(), transport);
    let mut syncer_events = syncer.handle(SyncerRequest::Join {
        room_id: room_id.clone(),
        participant_id: local.clone(),
    });

    // 再Offer後の join 完了を想定して、イベントをまとめて順序を検証
    events.append(&mut syncer_events);

    // 期待: 先頭が PeerLeft(remote)、直後のイベントが PeerJoined(remote)
    let mut iter = events.into_iter();
    let first = iter.next().expect("events should not be empty");
    let second = iter.next().unwrap_or(SyncerEvent::Error {
        kind: syncer::SyncerError::InvalidPayload(
            syncer::messages::SyncMessageError::UnknownKind {
                value: "placeholder".into(),
            },
        ),
    });

    assert!(
        matches!(first, SyncerEvent::PeerLeft { ref participant_id } if participant_id == &remote),
        "first event should be PeerLeft of remote"
    );
    assert!(
        matches!(second, SyncerEvent::PeerJoined { ref participant_id } if participant_id == &remote),
        "second event should be PeerJoined of remote (new session)"
    );

    // closer が1回だけ close を呼んでいることを確認
    let closer = adapter.into_inner_closer();
    assert_eq!(
        closer.closed,
        vec![remote],
        "closer should be called once for remote"
    );
}
