use serde_json::json;
use syncer::messages::{ControlMessage, ControlPayload, SyncMessageError};

#[test]
fn control_join_round_trip() {
    let control = ControlMessage::Join(ControlPayload {
        participant_id: "participant-alice".into(),
        reconnect_token: Some("token-123".into()),
        reason: None,
    });

    let value = serde_json::to_value(&control).expect("serialize control join");
    let decoded: ControlMessage = serde_json::from_value(value).expect("deserialize control join");

    assert_eq!(decoded, control);
}

#[test]
fn control_leave_round_trip() {
    let control = ControlMessage::Leave(ControlPayload {
        participant_id: "participant-bob".into(),
        reconnect_token: None,
        reason: Some("timeout".into()),
    });

    let value = serde_json::to_value(&control).expect("serialize control leave");
    let decoded: ControlMessage = serde_json::from_value(value).expect("deserialize control leave");

    assert_eq!(decoded, control);
}

#[test]
fn unsupported_control_kind_returns_error() {
    let raw = json!({
        "kind": "control.promote",
        "participantId": "participant-carol",
    });

    let err = ControlMessage::from_json_body(&raw)
        .expect_err("unsupported control kind should be invalid");

    assert!(matches!(
        err,
        SyncMessageError::SchemaViolation { kind, reason }
        if kind == "control" && reason == "unsupported_kind"
    ));
}
