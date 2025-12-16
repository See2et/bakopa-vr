mod common;

use bloom_core::{ParticipantId, RoomId};
use syncer::{
    webrtc_transport::RealWebrtcTransport, BasicSyncer, StreamKind, Syncer, SyncerEvent,
    SyncerRequest, TracingContext,
};

/// GREEN target: 実PeerConnection経路で SendVoiceFrame が相手に届き、TracingContext が一致する。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn voice_frame_delivered_over_real_webrtc_syncer() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (mut ta, mut tb) =
        RealWebrtcTransport::pair_with_datachannel_real(a.clone(), b.clone())
            .await
            .expect("pc setup");

    let timeout = std::time::Duration::from_secs(5);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");

    // 送信側にダミーAudioTrackを追加しておく（本番では音声初期化相当）
    ta.add_dummy_audio_track()
        .await
        .expect("should add dummy audio track");

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join both peers
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // 送信
    let frame = vec![9u8; 160];
    syncer_a.handle(SyncerRequest::SendVoiceFrame {
        frame: frame.clone(),
        ctx: TracingContext {
            room_id: room.clone(),
            participant_id: a.clone(),
            stream_kind: StreamKind::Voice,
        },
    });

    // 受信側でイベントをポーリング
    let mut received = None;
    for _ in 0..40 {
        let events = syncer_b.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&b),
            ctx: TracingContext::for_chat(&room, &b),
        });
        received = events.into_iter().find_map(|ev| match ev {
            SyncerEvent::VoiceFrameReceived { from, frame: f, ctx } => Some((from, f, ctx)),
            _ => None,
        });
        if received.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let (from, recv_frame, ctx) =
        received.expect("voice frame should be delivered over real webrtc path");
    assert_eq!(from, a);
    assert_eq!(recv_frame, frame);
    assert_eq!(ctx.room_id, room);
    assert_eq!(ctx.participant_id, a);
    assert_eq!(ctx.stream_kind, StreamKind::Voice);
}
