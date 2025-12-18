use std::cell::RefCell;
use std::rc::Rc;

use bloom_core::ParticipantId;

use syncer::{Transport, TransportEvent, TransportPayload};

#[allow(dead_code)]
#[derive(Default)]
pub struct BusState {
    pub registered: Vec<ParticipantId>,
    pub messages: Vec<(ParticipantId, ParticipantId, TransportPayload)>,
}

#[allow(dead_code)]
pub struct BusTransport {
    me: ParticipantId,
    bus: Rc<RefCell<BusState>>,
}

#[allow(dead_code)]
impl BusTransport {
    pub fn new(me: ParticipantId, bus: Rc<RefCell<BusState>>) -> Self {
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

    fn send(
        &mut self,
        to: ParticipantId,
        payload: TransportPayload,
        _params: syncer::TransportSendParams,
    ) {
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

        let _ = to; // signature維持（将来宛先制御する場合のため保持）
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

#[allow(dead_code)]
pub fn new_bus() -> Rc<RefCell<BusState>> {
    Rc::new(RefCell::new(BusState::default()))
}
