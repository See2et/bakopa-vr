use bloom_core::ParticipantId;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn datachannel_opens_between_two_real_webrtc_transports() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    // シグナリングは後で実装。ここでは同一プロセスで直接呼び合わせる想定。
    let mut ta =
        syncer::webrtc_transport::RealWebrtcTransport::new(a, vec![]).expect("create transport a");
    let mut tb =
        syncer::webrtc_transport::RealWebrtcTransport::new(b, vec![]).expect("create transport b");

    // TODO: offer/answer/ice交換を実装したらここに呼び出しを入れる。

    // sutera-data が open になるまで短時間ポーリング
    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    let mut opened = false;
    while tokio::time::Instant::now() < deadline {
        if ta.has_data_channel_open("sutera-data") && tb.has_data_channel_open("sutera-data") {
            opened = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert!(opened, "sutera-data channel should open between peers");

    // mute unused for now
    let _ = (&mut ta, &mut tb);
}
