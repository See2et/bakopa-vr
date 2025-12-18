use bloom_core::ParticipantId;
use syncer::webrtc_transport::RealWebrtcTransport;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn datachannel_opens_with_real_peer_connections() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (mut ta, mut tb) = RealWebrtcTransport::pair_with_datachannel_real(a, b)
        .await
        .expect("pc init");

    let timeout = std::time::Duration::from_secs(3);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");

    ta.shutdown().await;
    tb.shutdown().await;
}
