use bloom_core::ParticipantId;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn datachannel_opens_between_two_real_webrtc_transports() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (mut ta, mut tb) =
        syncer::webrtc_transport::RealWebrtcTransport::pair_with_datachannel_real(a, b)
            .await
            .expect("pc setup");

    let timeout = Duration::from_secs(5);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");

    assert!(ta.has_data_channel_open("sutera-data"));
    assert!(tb.has_data_channel_open("sutera-data"));

    ta.shutdown().await;
    tb.shutdown().await;
}
