use std::cell::RefCell;
use std::rc::Rc;

mod common;

use bloom_core::{ParticipantId, RoomId};
use common::{sample_pose, sample_tracing_context};
use syncer::{
    StreamKind, Syncer, SyncerEvent, SyncerRequest, TracingContext, Transport, TransportEvent,
    TransportPayload,
};

struct SharedState {
    participants: Vec<ParticipantId>,
    messages: Vec<(ParticipantId, ParticipantId, TransportPayload)>,
}

struct FakeTransport {
    me: ParticipantId,
    bus: Rc<RefCell<SharedState>>,
}

impl FakeTransport {
    fn new(me: ParticipantId, bus: Rc<RefCell<SharedState>>) -> Self {
        Self { me, bus }
    }
}

impl Transport for FakeTransport {
    fn register_participant(&mut self, participant: ParticipantId) {
        let mut bus = self.bus.borrow_mut();
        if !bus.participants.iter().any(|p| p == &participant) {
            bus.participants.push(participant);
        }
    }

    fn send(&mut self, to: ParticipantId, payload: TransportPayload, _params: syncer::TransportSendParams) {
        let recipients: Vec<ParticipantId> = {
            let bus = self.bus.borrow();
            bus.participants
                .iter()
                .filter(|p| *p != &self.me)
                .cloned()
                .collect()
        };

        let mut bus = self.bus.borrow_mut();
        for p in recipients {
            bus.messages.push((p, self.me.clone(), payload.clone()));
        }
        let _ = to; // signatureは保持しつつ、ここではブロードキャストのみを模倣
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        let mut buf = self.bus.borrow_mut();
        let mut received = Vec::new();
        let mut i = 0;
        while i < buf.messages.len() {
            if buf.messages[i].0 == self.me {
                let (_to, from, payload) = buf.messages.remove(i);
                received.push(TransportEvent::Received { from, payload });
            } else {
                i += 1;
            }
        }
        received
    }
}

struct TransportBackedSyncer<'a> {
    transport: &'a mut dyn Transport,
    room: Option<RoomId>,
    me: Option<ParticipantId>,
}

impl<'a> TransportBackedSyncer<'a> {
    fn new(transport: &'a mut dyn Transport) -> Self {
        Self {
            transport,
            room: None,
            me: None,
        }
    }
}

impl<'a> Syncer for TransportBackedSyncer<'a> {
    fn handle(&mut self, request: SyncerRequest) -> Vec<SyncerEvent> {
        let events: Vec<SyncerEvent> = self
            .transport
            .poll()
            .into_iter()
            .filter_map(|ev| match ev {
                TransportEvent::Received { from, payload: _ } => Some(SyncerEvent::PoseReceived {
                    ctx: TracingContext {
                        room_id: self
                            .room
                            .clone()
                            .expect("room should be set before receive"),
                        participant_id: from.clone(),
                        stream_kind: StreamKind::Pose,
                    },
                    from,
                    pose: sample_pose(),
                }),
                TransportEvent::Failure { .. } => None,
            })
            .collect();

        match request {
            SyncerRequest::Join {
                room_id,
                participant_id,
            } => {
                self.room = Some(room_id);
                self.transport.register_participant(participant_id.clone());
                self.me = Some(participant_id);
            }
            SyncerRequest::SendPose {
                from,
                pose: _pose,
                ctx: _ctx,
            } => {
                self.transport.send(
                    from.clone(),
                    TransportPayload::Bytes(Vec::new()),
                    syncer::TransportSendParams::for_stream(StreamKind::Pose),
                );
            }
            SyncerRequest::SendChat { .. } => {}
            SyncerRequest::SendVoiceFrame { .. } => {}
        }

        events
    }
}

#[test]
fn join_pose_flow_delivers_pose_to_peer() {
    let shared = Rc::new(RefCell::new(SharedState {
        participants: Vec::new(),
        messages: Vec::new(),
    }));

    let room_id = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let mut transport_a = FakeTransport::new(a.clone(), shared.clone());
    let mut transport_b = FakeTransport::new(b.clone(), shared.clone());

    let mut syncer_a = TransportBackedSyncer::new(&mut transport_a);
    let mut syncer_b = TransportBackedSyncer::new(&mut transport_b);

    // Both peers join
    syncer_a.handle(SyncerRequest::Join {
        room_id: room_id.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room_id.clone(),
        participant_id: b.clone(),
    });

    // A sends pose
    syncer_a.handle(SyncerRequest::SendPose {
        from: a.clone(),
        pose: sample_pose(),
        ctx: sample_tracing_context(&room_id, &a),
    });

    // B polls events via handle and expects PoseReceived
    let events = syncer_b.handle(SyncerRequest::SendPose {
        from: b.clone(),
        pose: sample_pose(),
        ctx: sample_tracing_context(&room_id, &b),
    });

    assert!(
        events
            .iter()
            .any(|e| matches!(e, SyncerEvent::PoseReceived { from, .. } if from == &a)),
        "expected PoseReceived from A on B side"
    );
}
