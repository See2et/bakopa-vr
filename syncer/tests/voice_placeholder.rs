mod common;

use bloom_core::{ParticipantId, RoomId};
use syncer::{
    BasicSyncer, Syncer, SyncerEvent, SyncerRequest, Transport, TransportEvent, TransportPayload,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct Bus {
    registered: Vec<ParticipantId>,
    messages: Vec<(ParticipantId, ParticipantId, TransportPayload)>,
}

struct BusTransport {
    me: ParticipantId,
    bus: Rc<RefCell<Bus>>,
}

impl BusTransport {
    fn new(me: ParticipantId, bus: Rc<RefCell<Bus>>) -> Self {
        Self { me, bus }
    }
}

impl Transport for BusTransport {
    fn register_participant(&mut self, participant: ParticipantId) {
        let mut bus = self.bus.borrow_mut();
        if !bus.registered.iter().any(|p| p == &participant) {
            bus.registered.push(participant);
        }
    }

    fn send(&mut self, to: ParticipantId, payload: TransportPayload) {
        let recipients: Vec<ParticipantId> = {
            let bus = self.bus.borrow();
            bus.registered
                .iter()
                .filter(|p| *p != &self.me)
                .cloned()
                .collect()
        };

        let mut bus = self.bus.borrow_mut();
        for r in recipients {
            bus.messages.push((r, self.me.clone(), payload.clone()));
        }

        let _ = to;
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        let mut bus = self.bus.borrow_mut();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bus.messages.len() {
            if bus.messages[i].0 == self.me {
                let (_to, from, payload) = bus.messages.remove(i);
                out.push(TransportEvent::Received { from, payload });
            } else {
                i += 1;
            }
        }
        out
    }
}

#[test]
fn audio_frame_is_emitted_as_voice_event_with_context() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let bus = Rc::new(RefCell::new(Bus::default()));

    let ta = BusTransport::new(a.clone(), bus.clone());
    let tb = BusTransport::new(b.clone(), bus.clone());

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // A sends raw audio frame via transport directly (placeholder)
    syncer_a.send_transport_payload(b.clone(), TransportPayload::AudioFrame(vec![1, 2, 3]));

    // B processes incoming; expect a VoiceFrameReceived
    let events = syncer_b.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&b),
        ctx: common::sample_tracing_context(&room, &b),
    });

    let voice_event = events
        .into_iter()
        .find_map(|e| match e {
            SyncerEvent::VoiceFrameReceived { from, frame, ctx } => Some((from, frame, ctx)),
            _ => None,
        })
        .expect("expected voice frame event");

    assert_eq!(voice_event.0, a);
    assert_eq!(voice_event.1, vec![1, 2, 3]);
    assert_eq!(voice_event.2.room_id, room);
    assert_eq!(voice_event.2.participant_id, a);
    assert_eq!(voice_event.2.stream_kind, syncer::StreamKind::Voice);
}
