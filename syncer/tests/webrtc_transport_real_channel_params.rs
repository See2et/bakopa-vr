use bloom_core::ParticipantId;
use syncer::{
    webrtc_transport::{test_helpers, RealWebrtcTransport},
    Transport, TransportPayload, TransportSendParams,
};

/// DataChannel configがStreamKindに対応していることを検証（Red→Green用）。現状は未反映で失敗する想定。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn datachannel_params_reflect_stream_kind() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (mut ta, mut tb) = RealWebrtcTransport::pair_with_datachannel_real(a, b.clone())
        .await
        .expect("pc setup");

    let timeout = std::time::Duration::from_secs(3);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");

    ta.send(
        b.clone(),
        TransportPayload::Bytes(vec![0u8]),
        TransportSendParams::for_stream(syncer::StreamKind::Pose),
    );
    ta.send(
        b,
        TransportPayload::Bytes(vec![1u8]),
        TransportSendParams::for_stream(syncer::StreamKind::Chat),
    );

    let params = test_helpers::created_params(&ta);
    assert!(
        params.iter().any(|p| matches!(
            p,
            TransportSendParams::DataChannel {
                ordered: false,
                reliable: false,
                ..
            }
        )),
        "pose should be unordered/unreliable"
    );
    assert!(
        params.iter().any(|p| matches!(
            p,
            TransportSendParams::DataChannel {
                ordered: true,
                reliable: true,
                ..
            }
        )),
        "chat should be ordered/reliable"
    );

    ta.shutdown().await;
    tb.shutdown().await;
}
