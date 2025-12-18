use bloom_core::{ParticipantId, RoomId};
use syncer::{messages::ChatMessage, Pose, PoseTransform, StreamKind, TracingContext};

pub mod bus_transport;
pub mod fake_clock;
pub mod timeouts;

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn sample_tracing_context(room_id: &RoomId, participant_id: &ParticipantId) -> TracingContext {
    TracingContext {
        room_id: room_id.clone(),
        participant_id: participant_id.clone(),
        stream_kind: StreamKind::Pose,
    }
}

#[allow(dead_code)]
pub fn sample_voice_context(room_id: &RoomId, participant_id: &ParticipantId) -> TracingContext {
    TracingContext {
        room_id: room_id.clone(),
        participant_id: participant_id.clone(),
        stream_kind: StreamKind::Voice,
    }
}

#[allow(dead_code)]
pub fn sample_chat(sender: &ParticipantId) -> ChatMessage {
    ChatMessage {
        version: 1,
        timestamp_micros: 0,
        sequence_id: 1,
        sender: sender.to_string(),
        message: "hello".to_string(),
    }
}
