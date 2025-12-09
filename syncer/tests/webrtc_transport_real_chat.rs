use bloom_core::ParticipantId;
use syncer::{Transport, TransportEvent, TransportPayload};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_roundtrip_over_real_datachannel() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (mut ta, mut tb) = syncer::webrtc_transport::RealWebrtcTransport::pair_with_datachannel_real(a, b.clone())
        .await
        .expect("pc setup");

    let timeout = std::time::Duration::from_secs(5);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");

    assert!(ta.has_data_channel_open("sutera-data"));
    assert!(tb.has_data_channel_open("sutera-data"));

    // A -> B へバイトを送信
    ta.send(
        b,
        TransportPayload::Bytes(br#"{"v":1,"kind":"chat","body":{"message":"hi"}}"#.to_vec()),
        syncer::TransportSendParams::for_stream(syncer::StreamKind::Chat),
    );

    // B が受信できることを確認（データチャネル経路で到達するまでポーリング）
    let mut received = false;
    for _ in 0..30 {
        let events = tb.poll();
        if matches!(events.as_slice(), [TransportEvent::Received { .. }]) {
            received = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(received, "chat should arrive over real datachannel");
}
