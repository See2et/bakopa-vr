mod common;

use bloom_core::{ParticipantId, RoomId};
use syncer::{
    webrtc_transport::WebrtcTransport, BasicSyncer, StreamKind, Syncer, SyncerRequest,
    TransportSendParams, TracingContext,
};

fn params_for(kind: StreamKind) -> TransportSendParams {
    TransportSendParams::for_stream(kind)
}

#[test]
fn pose_and_chat_use_expected_channel_params() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (ta, tb) = WebrtcTransport::pair(a.clone(), b.clone());

    // clone to observe send側の記録（shared state想定）
    let ta_probe = ta.clone();

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join both
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // send pose then chat from A
    syncer_a.handle(SyncerRequest::SendPose {
        from: a.clone(),
        pose: common::sample_pose(),
        ctx: TracingContext {
            room_id: room.clone(),
            participant_id: a.clone(),
            stream_kind: StreamKind::Pose,
        },
    });

    syncer_a.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&a),
        ctx: TracingContext::for_chat(&room, &a),
    });

    // 送信時に使われたパラメータを観測（ControlJoinブロードキャストが先に積まれるためフィルタする）
    let sent = ta_probe.sent_params();
    let pose_then_chat: Vec<_> = sent
        .into_iter()
        .filter(|p| matches!(p, TransportSendParams::DataChannel { .. } | TransportSendParams::AudioTrack))
        .filter(|p| p == &params_for(StreamKind::Pose) || p == &params_for(StreamKind::Chat))
        .collect();

    // PoseとChatがそれぞれ1回以上送られ、順序も Pose -> Chat で記録されていることを確認
    assert!(
        pose_then_chat.windows(2).any(|w| w[0] == params_for(StreamKind::Pose) && w[1] == params_for(StreamKind::Chat)),
        "pose send should use unordered/unreliable params and precede chat params"
    );
}
