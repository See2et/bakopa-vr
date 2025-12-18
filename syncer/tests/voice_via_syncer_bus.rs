mod common;

use bloom_core::{ParticipantId, RoomId};
use syncer::{
    webrtc_transport::WebrtcTransport, BasicSyncer, StreamKind, Syncer, SyncerEvent, SyncerRequest,
    TracingContext, TransportSendParams,
};

#[test]
fn voice_frame_delivered_via_syncer_uses_audio_track_params() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    // in-process WebRTC風Transportを用いて、送信パラメータも観測できるようにしておく。
    let (ta, tb) = WebrtcTransport::pair(a.clone(), b.clone());
    let ta_probe = ta.clone();

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join both peers so filtering transport is active
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // 送信側: 音声フレームを送信
    let frame = vec![1u8, 2, 3, 4];
    let voice_ctx = TracingContext {
        room_id: room.clone(),
        participant_id: a.clone(),
        stream_kind: StreamKind::Voice,
    };

    syncer_a.handle(SyncerRequest::SendVoiceFrame {
        frame: frame.clone(),
        ctx: voice_ctx.clone(),
    });

    // 受信側: 適当なリクエストを流しつつイベントをドレイン
    let mut received = None;
    for _ in 0..5 {
        let events = syncer_b.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&b),
            ctx: TracingContext {
                room_id: room.clone(),
                participant_id: b.clone(),
                stream_kind: StreamKind::Chat,
            },
        });

        received = events.into_iter().find_map(|ev| match ev {
            SyncerEvent::VoiceFrameReceived {
                from,
                frame: f,
                ctx,
            } => Some((from, f, ctx)),
            _ => None,
        });

        if received.is_some() {
            break;
        }
    }

    let (from, recv_frame, ctx) =
        received.expect("voice frame should be delivered exactly once to the peer");

    assert_eq!(from, a);
    assert_eq!(recv_frame, frame);
    assert_eq!(ctx.room_id, room);
    assert_eq!(ctx.participant_id, a);
    assert_eq!(ctx.stream_kind, StreamKind::Voice);

    // 送信側でAudioTrackが使われたことを確認
    let sent_params = ta_probe.sent_params();
    assert!(
        sent_params
            .iter()
            .any(|p| matches!(p, TransportSendParams::AudioTrack)),
        "voice should use audio track send params"
    );
}
