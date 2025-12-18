use std::cell::RefCell;
use std::rc::Rc;

use bloom_api::payload::RelaySdp;
use bloom_api::ServerToClient;
use bloom_core::ParticipantId;

use syncer::signaling_adapter::{
    BloomSignalingAdapter, ClientToServerSender, PeerConnectionCloser, SignalingContext,
};

#[derive(Default)]
struct NoopSender;
impl ClientToServerSender for NoopSender {
    fn send(&mut self, _message: bloom_api::ClientToServer) {}
}

#[derive(Clone)]
struct MockCloser {
    calls: Rc<RefCell<u32>>,
}

impl MockCloser {
    fn new(counter: Rc<RefCell<u32>>) -> Self {
        Self { calls: counter }
    }
}

impl Default for MockCloser {
    fn default() -> Self {
        Self::new(Rc::new(RefCell::new(0)))
    }
}

impl PeerConnectionCloser for MockCloser {
    fn close(&mut self, _participant: &ParticipantId) {
        *self.calls.borrow_mut() += 1;
    }
}

#[test]
fn closer_invoked_once_on_reoffer() {
    let room = "room-x".to_string();
    let auth = "token-x".to_string();
    let ctx = SignalingContext {
        room_id: room,
        auth_token: auth,
        ice_policy: "default".to_string(),
    };

    let counter = Rc::new(RefCell::new(0));
    let closer = MockCloser::new(counter.clone());

    let mut adapter = BloomSignalingAdapter::with_context_and_closer(NoopSender, closer, ctx);

    let pid = ParticipantId::new().to_string();

    // 1st offer (new participant) -> should not close
    adapter.push_incoming(ServerToClient::Offer {
        from: pid.clone(),
        payload: RelaySdp {
            sdp: "v=0".to_string(),
        },
    });
    adapter.poll();
    assert_eq!(*counter.borrow(), 0, "first offer should not close");

    // 2nd offer (re-offer) -> should close once
    adapter.push_incoming(ServerToClient::Offer {
        from: pid.clone(),
        payload: RelaySdp {
            sdp: "v=0".to_string(),
        },
    });
    adapter.poll();
    assert_eq!(
        *counter.borrow(),
        1,
        "re-offer should invoke closer exactly once"
    );
}
