mod common;

use bloom_core::{ParticipantId, RoomId};
use common::{sample_pose, sample_tracing_context};
use syncer::{
    webrtc_transport::WebrtcTransport, BasicSyncer, Syncer, SyncerEvent, SyncerRequest,
    TransportSendParams,
};

#[test]
fn pose_delivers_once_with_unordered_unreliable_params() {
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

    // send pose from A
    let pose = sample_pose();
    syncer_a.handle(SyncerRequest::SendPose {
        from: a.clone(),
        pose: pose.clone(),
        ctx: sample_tracing_context(&room, &a),
    });

    // B drains incoming by issuing dummy send
    let events = syncer_b.handle(SyncerRequest::SendPose {
        from: b.clone(),
        pose: sample_pose(),
        ctx: sample_tracing_context(&room, &b),
    });

    let pose_event = events.into_iter().find_map(|ev| match ev {
        SyncerEvent::PoseReceived { from, pose: p, ctx } => Some((from, p, ctx)),
        _ => None,
    });

    let (from, recv_pose, ctx) = pose_event.expect("B should receive exactly one pose");
    assert_eq!(from, a);
    assert_eq!(recv_pose, pose);
    assert_eq!(ctx.room_id, room);
    assert_eq!(ctx.participant_id, a);
    assert_eq!(ctx.stream_kind, syncer::StreamKind::Pose);

    // Transport params should include unordered/unreliable entry for pose
    let pose_param = TransportSendParams::for_stream(syncer::StreamKind::Pose);
    let sent_params = ta_probe.sent_params();
    assert!(
        sent_params.contains(&pose_param),
        "unordered/unreliable pose param must be recorded"
    );
}
