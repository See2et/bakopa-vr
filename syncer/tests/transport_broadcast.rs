use std::cell::RefCell;
use std::rc::Rc;

use bloom_core::ParticipantId;
use syncer::{Transport, TransportEvent, TransportPayload};

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
        let bus = self.bus.borrow();
        let recipients: Vec<ParticipantId> = bus
            .registered
            .iter()
            .filter(|p| *p != &self.me) // 自分以外
            .cloned()
            .collect();
        drop(bus);

        let mut bus = self.bus.borrow_mut();
        for r in recipients {
            bus.messages.push((r, self.me.clone(), payload.clone()));
        }

        let _ = to; // signature維持
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
fn send_reaches_everyone_except_sender() {
    let bus = Rc::new(RefCell::new(Bus::default()));
    let a = ParticipantId::new();
    let b = ParticipantId::new();
    let c = ParticipantId::new();

    let mut ta = BusTransport::new(a.clone(), bus.clone());
    let mut tb = BusTransport::new(b.clone(), bus.clone());
    let mut tc = BusTransport::new(c.clone(), bus.clone());

    ta.register_participant(a.clone());
    tb.register_participant(b.clone());
    tc.register_participant(c.clone());

    // A sends once
    ta.send(b.clone(), TransportPayload::Bytes(vec![1]));

    // B and C should receive, A should not
    let received_b = tb.poll();
    let received_c = tc.poll();

    assert_eq!(received_b.len(), 1, "B should receive one message");
    assert_eq!(received_c.len(), 1, "C should receive one message");

    // The sender should not receive its own message
    let received_a = ta.poll();
    assert!(received_a.is_empty(), "A should not receive its own send");
}

#[test]
fn nothing_delivered_before_registration() {
    let bus = Rc::new(RefCell::new(Bus::default()));
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let mut ta = BusTransport::new(a.clone(), bus.clone());
    let mut tb = BusTransport::new(b.clone(), bus.clone());

    // send before register -> should be dropped
    ta.send(b.clone(), TransportPayload::Bytes(vec![1]));

    let received_b = tb.poll();
    assert!(received_b.is_empty(), "unregistered peers should not receive");

    // after register, message should flow
    tb.register_participant(b.clone());
    ta.register_participant(a.clone());
    ta.send(b.clone(), TransportPayload::Bytes(vec![2]));

    let received_b = tb.poll();
    assert_eq!(received_b.len(), 1, "registered peer should receive after register");
}
