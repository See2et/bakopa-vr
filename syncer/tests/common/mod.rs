use bloom_core::{ParticipantId, RoomId};
use syncer::{Pose, PoseTransform, StreamKind, TracingContext};

pub fn sample_pose() -> Pose {
    Pose {
        version: 1,
        timestamp_micros: 0,
        head: PoseTransform {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        },
        hand_l: None,
        hand_r: None,
    }
}

pub fn sample_tracing_context(room_id: &RoomId, participant_id: &ParticipantId) -> TracingContext {
    TracingContext {
        room_id: room_id.clone(),
        participant_id: participant_id.clone(),
        stream_kind: StreamKind::Pose,
    }
}
