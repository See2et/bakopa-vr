use bloom_core::ParticipantId;
use syncer::{Transport, TransportEvent};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failure_event_emitted_on_ice_or_dtls_error() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (mut ta, _tb) =
        syncer::webrtc_transport::RealWebrtcTransport::pair_with_datachannel_real_failfast(
            a.clone(),
            b.clone(),
        )
        .await
        .expect("pc setup");

    // 短時間ポーリングしてFailureが発火することを期待
    let mut got = false;
    for _ in 0..20 {
        let events = ta.poll();
        if events
            .iter()
            .any(|e| matches!(e, TransportEvent::Failure { .. }))
        {
            got = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    assert!(got, "expected failure event when ICE/DTLS fails");
}
