use serde_json::json;
use syncer::messages::{SyncMessageEnvelope, SyncMessageError};
use syncer::StreamKind;

#[test]
fn envelope_round_trip_through_serde_json() {
    let envelope = SyncMessageEnvelope {
        version: 1,
        kind: StreamKind::Pose,
        body: json!({
            "head": {
                "position": [0.0, 1.0, 0.5],
                "rotation": [0.0, 0.0, 0.0, 1.0]
            }
        }),
    };

    let serialized = serde_json::to_string(&envelope).expect("serialize envelope");
    let decoded: SyncMessageEnvelope =
        serde_json::from_str(&serialized).expect("deserialize envelope");

    assert_eq!(decoded, envelope);
}

#[test]
fn missing_version_field_is_reported_as_invalid_payload() {
    let raw = r#"{"kind":"pose","body":{}}"#;

    let err = SyncMessageEnvelope::from_slice(raw.as_bytes())
        .expect_err("missing version should be invalid");

    assert!(matches!(err, SyncMessageError::MissingVersion));
}
