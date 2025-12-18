use bloom_core::ParticipantId;

/// Smoke: 実PeerConnectionが作られ、sutera-data DataChannel をオープンできること。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_webrtc_transport_initializes_peer_connection() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (mut ta, mut tb) =
        syncer::webrtc_transport::RealWebrtcTransport::pair_with_datachannel_real(a, b)
            .await
            .expect("pc setup");

    assert!(ta.has_peer_connection());
    assert!(tb.has_peer_connection());

    let timeout = std::time::Duration::from_secs(5);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");
    assert!(ta.has_data_channel_open("sutera-data"));
    assert!(tb.has_data_channel_open("sutera-data"));

    ta.shutdown().await;
    tb.shutdown().await;
}
