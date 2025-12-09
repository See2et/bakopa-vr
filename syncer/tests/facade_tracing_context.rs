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

    fn send(&mut self, to: ParticipantId, payload: TransportPayload) {
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
        let _ = to;
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        let mut bus = self.bus.borrow_mut();
        let mut received = Vec::new();
        let mut i = 0;
        while i < bus.messages.len() {
            if bus.messages[i].0 == self.me {
                let (_to, from, payload) = bus.messages.remove(i);
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
            .map(|ev| match ev {
                TransportEvent::Received { from, payload: _ } => SyncerEvent::PoseReceived {
                    ctx: TracingContext {
                        room_id: self.room.clone().unwrap(),
                        participant_id: from.clone(),
                        stream_kind: StreamKind::Pose,
                    },
                    from,
                    pose: sample_pose(),
                },
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
                pose: _,
                ctx: _,
            } => {
                self.transport
                    .send(from.clone(), TransportPayload::Bytes(Vec::new()));
            }
            SyncerRequest::SendChat { .. } => {}
        }

        events
    }
}

#[test]
fn pose_received_carries_tracing_context() {
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

    syncer_a.handle(SyncerRequest::Join {
        room_id: room_id.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room_id.clone(),
        participant_id: b.clone(),
    });

    syncer_a.handle(SyncerRequest::SendPose {
        from: a.clone(),
        pose: sample_pose(),
        ctx: sample_tracing_context(&room_id, &a),
    });

    let events = syncer_b.handle(SyncerRequest::SendPose {
        from: b.clone(),
        pose: sample_pose(),
        ctx: sample_tracing_context(&room_id, &b),
    });

    let pose_event = events
        .into_iter()
        .find_map(|e| match e {
            SyncerEvent::PoseReceived { ctx, .. } => Some(ctx),
            _ => None,
        })
        .expect("PoseReceived event expected");

    assert_eq!(pose_event.room_id, room_id);
    assert_eq!(pose_event.participant_id, a);
    assert_eq!(pose_event.stream_kind, StreamKind::Pose);
}
