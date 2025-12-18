use serde_json::json;
use syncer::messages::{PoseMessage, PoseTransform, SyncMessageError};

fn sample_transform(x: f32, y: f32, z: f32) -> PoseTransform {
    PoseTransform {
        position: [x, y, z],
        rotation: [0.0, 0.0, 0.0, 1.0],
    }
}

#[test]
fn pose_message_round_trip_via_json() {
    let pose = PoseMessage {
        version: 1,
        timestamp_micros: 123_456,
        head: sample_transform(0.0, 1.0, 2.0),
        hand_l: Some(sample_transform(-0.5, 0.8, 0.2)),
        hand_r: None,
    };

    let json_value = serde_json::to_value(&pose).expect("serialize pose message");
    let decoded: PoseMessage =
        serde_json::from_value(json_value).expect("deserialize pose message");

    assert_eq!(decoded, pose);
}

#[test]
fn missing_head_field_is_invalid_payload() {
    let raw = json!({
        "version": 1,
        "timestampMicros": 42,
        "handL": {
            "position": [0.0, 0.0, 0.0],
            "rotation": [0.0, 0.0, 0.0, 1.0]
        }
    });

    let err = PoseMessage::from_json_body(&raw)
        .expect_err("missing head should be treated as invalid payload");

    assert!(matches!(
        err,
        SyncMessageError::SchemaViolation { kind, reason }
        if kind == "pose" && reason == "missing_head"
    ));
}
