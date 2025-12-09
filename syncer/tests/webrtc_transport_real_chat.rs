use bloom_core::ParticipantId;
use syncer::{Transport, TransportEvent, TransportPayload};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_roundtrip_over_real_datachannel() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (mut ta, mut tb) = syncer::webrtc_transport::RealWebrtcTransport::pair_for_tests(a, b.clone());

    // sutera-data チャンネルがopenしている前提（現状スタブ）
    assert!(ta.has_data_channel_open("sutera-data"));
    assert!(tb.has_data_channel_open("sutera-data"));

    // A -> B へバイトを送信
    ta.send(
        b,
        TransportPayload::Bytes(br#"{"v":1,"kind":"chat","body":{"message":"hi"}}"#.to_vec()),
        syncer::TransportSendParams::for_stream(syncer::StreamKind::Chat),
    );

    // B が受信できることを確認
    let events = tb.poll();
    assert!(matches!(events.as_slice(), [TransportEvent::Received { .. }]));
}
