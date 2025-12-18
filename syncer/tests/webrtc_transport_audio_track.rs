mod common;

use bloom_core::{ParticipantId, RoomId};
use syncer::{
    webrtc_transport::WebrtcTransport, BasicSyncer, StreamKind, Syncer, SyncerEvent, SyncerRequest,
    TracingContext, TransportSendParams,
};

#[test]
fn audio_frame_traverses_transport_and_records_audio_params() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (ta, tb) = WebrtcTransport::pair(a.clone(), b.clone());
    let ta_probe = ta.clone();

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

    // A から B へ音声フレームを送信（正式API）
    syncer_a.handle(SyncerRequest::SendVoiceFrame {
        frame: vec![7, 8, 9],
        ctx: TracingContext {
            room_id: room.clone(),
            participant_id: a.clone(),
            stream_kind: StreamKind::Voice,
        },
    });

    // B 側で受信処理を促すため、適当なリクエストを投げてイベントをドレイン
    let events = syncer_b.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&b),
        ctx: TracingContext {
            room_id: room.clone(),
            participant_id: b.clone(),
            stream_kind: StreamKind::Chat,
        },
    });

    let voice_event = events.into_iter().find_map(|e| match e {
        SyncerEvent::VoiceFrameReceived { from, frame, ctx } => Some((from, frame, ctx)),
        _ => None,
    });

    let (from, frame, _ctx) =
        voice_event.expect("voice frame should be delivered as VoiceFrameReceived");
    assert_eq!(from, a);
    assert_eq!(frame, vec![7, 8, 9]);

    // 送信側で AudioTrack パラメータが記録されていることも確認
    let sent_params = ta_probe.sent_params();
    // ControlJoinのブロードキャストが先に1件積まれるため、AudioTrackが少なくとも1件含まれることを確認する。
    let audio_count = sent_params
        .iter()
        .filter(|p| matches!(p, TransportSendParams::AudioTrack))
        .count();
    assert!(
        audio_count >= 1,
        "audio track send params should be recorded at least once"
    );
}
