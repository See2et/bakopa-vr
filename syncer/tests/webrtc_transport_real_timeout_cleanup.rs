use bloom_core::ParticipantId;
use syncer::{Transport, TransportEvent};

/// RED: openせずタイムアウトしたらFailureを積み、リソースをクローズすることを期待。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connection_timeout_emits_failure_and_cleans_up() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    // fail-fast設定で、timeout内にopenしないようにする
    let (mut ta, tb) =
        syncer::webrtc_transport::RealWebrtcTransport::pair_with_datachannel_real_failfast(
            a.clone(),
            b.clone(),
        )
        .await
        .expect("pc setup");

    // 非常に短いタイムアウトでopen待ち
    let timeout = std::time::Duration::from_millis(50);
    let _ = ta.wait_data_channel_open(timeout).await; // 失敗を期待

    // Failureイベントが積まれていること
    let events = ta.poll();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TransportEvent::Failure { .. })),
        "failure should be emitted on timeout"
    );

    ta.shutdown().await;
    tb.shutdown().await;
}
