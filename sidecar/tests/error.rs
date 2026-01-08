use syncer::messages::SyncMessageError;
use syncer::SyncerError;

#[test]
fn syncer_error_maps_to_transport_error() {
    let err = SyncerError::InvalidPayload(SyncMessageError::UnknownKind {
        value: "bogus".to_string(),
    });
    let payload = sidecar::app::syncer_error_payload(&err);
    let value: serde_json::Value = serde_json::from_str(&payload).expect("parse payload");
    assert_eq!(value.get("type").and_then(|v| v.as_str()), Some("Error"));
    assert_eq!(
        value.get("kind").and_then(|v| v.as_str()),
        Some("TransportError")
    );
}
