use bloom_core::ParticipantId;
use syncer::webrtc_transport::signaling_hub::{InMemorySignalingHub, SignalKind, SignalMessage};

#[test]
fn signaling_hub_exchanges_offer_answer_ice() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let mut hub = InMemorySignalingHub::new();
    hub.register(a.clone());
    hub.register(b.clone());

    let offer = SignalMessage {
        from: a.clone(),
        to: b.clone(),
        kind: SignalKind::Offer,
        payload: "offer_sdp".into(),
    };
    let answer = SignalMessage {
        from: b.clone(),
        to: a.clone(),
        kind: SignalKind::Answer,
        payload: "answer_sdp".into(),
    };
    let ice = SignalMessage {
        from: a.clone(),
        to: b.clone(),
        kind: SignalKind::Ice,
        payload: "candidate".into(),
    };

    hub.send(offer.clone());
    hub.send(answer.clone());
    hub.send(ice.clone());

    let msgs_for_b = hub.drain_for(&b);
    assert!(msgs_for_b.contains(&offer));
    assert!(msgs_for_b.contains(&ice));

    let msgs_for_a = hub.drain_for(&a);
    assert!(msgs_for_a.contains(&answer));
}
