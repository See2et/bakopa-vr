mod common;

use bloom_core::ParticipantId;
use syncer::{Transport, TransportPayload};

use common::bus_transport::{new_bus, BusTransport};

#[test]
fn send_reaches_everyone_except_sender() {
    let bus = new_bus();
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
    let bus = new_bus();
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
