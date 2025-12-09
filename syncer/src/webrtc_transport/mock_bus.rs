use std::cell::RefCell;
use std::rc::Rc;

use bloom_core::ParticipantId;

use crate::{TransportEvent, TransportPayload};

#[derive(Default)]
pub struct BusState {
    pub messages: Vec<(ParticipantId, ParticipantId, TransportPayload)>, // (to, from, payload)
}

#[derive(Clone)]
pub struct MockBus {
    state: Rc<RefCell<BusState>>,
}

impl MockBus {
    pub fn new_shared() -> (Self, Self) {
        let state = Rc::new(RefCell::new(BusState::default()));
        (Self { state: state.clone() }, Self { state })
    }

    pub fn push(&self, to: ParticipantId, from: ParticipantId, payload: TransportPayload) {
        self.state.borrow_mut().messages.push((to, from, payload));
    }

    pub fn drain_for(&self, me: &ParticipantId) -> Vec<TransportEvent> {
        let mut out = Vec::new();
        let mut state = self.state.borrow_mut();
        let mut i = 0;
        while i < state.messages.len() {
            if state.messages[i].0 == *me {
                let (_to, from, payload) = state.messages.remove(i);
                out.push(TransportEvent::Received { from, payload });
            } else {
                i += 1;
            }
        }
        out
    }
}
