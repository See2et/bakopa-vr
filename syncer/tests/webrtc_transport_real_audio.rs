use bloom_core::ParticipantId;
use syncer::{Transport, TransportEvent, TransportPayload};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audio_frame_delivered_over_real_webrtc_transport() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (mut ta, mut tb) =
        syncer::webrtc_transport::RealWebrtcTransport::pair_with_datachannel_real(a, b)
            .await
            .expect("pc setup");

    let timeout = std::time::Duration::from_secs(5);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");

    ta.add_dummy_audio_track()
        .await
        .expect("should add dummy audio track");

    let payload = vec![0u8; 160];
    ta.send_dummy_audio_frame(payload)
        .await
        .expect("should send dummy frame");

    // 受信側でAudioFrameイベントを待つ
    let mut received = false;
    for _ in 0..30 {
        let events = tb.poll();
        if events.iter().any(|e| {
            matches!(
                e,
                TransportEvent::Received {
                    payload: TransportPayload::AudioFrame(_),
                    ..
                }
            )
        }) {
            received = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(received, "audio frame should be delivered");

    ta.shutdown().await;
    tb.shutdown().await;
}
