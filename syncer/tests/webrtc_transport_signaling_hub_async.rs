use bloom_core::ParticipantId;
use syncer::webrtc_transport::signaling_hub::{InMemorySignalingHub, SignalKind, SignalMessage};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_signaling_exchange_round_trip() {
    let hub = InMemorySignalingHub::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    hub.register(a.clone());
    hub.register(b.clone());

    let offer = SignalMessage {
        from: a.clone(),
        to: b.clone(),
        kind: SignalKind::Offer,
        payload: "offer_sdp".into(),
    };

    // send from multiple tasks to check Send+Sync
    let hub_clone = hub.clone();
    let sender = tokio::spawn(async move {
        hub_clone.send(offer);
    });

    sender.await.unwrap();

    let b_clone = b.clone();
    let msgs = tokio::task::spawn_blocking(move || hub.drain_for(&b_clone))
        .await
        .unwrap();
    assert!(msgs
        .iter()
        .any(|m| m.kind == SignalKind::Offer && m.from == a && m.to == b));
}
