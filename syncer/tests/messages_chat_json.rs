use serde_json::json;
use syncer::messages::{ChatMessage, SyncMessageError};

#[test]
fn chat_message_round_trip_with_unicode_at_max_length() {
    let max_len = ChatMessage::MAX_MESSAGE_LEN;
    let content = "あ🙂".repeat(max_len / 2);

    let message = ChatMessage {
        version: 1,
        timestamp_micros: 987_654,
        sequence_id: 42,
        sender: "participant-alice".into(),
        message: content.clone(),
    };

    let json_value = serde_json::to_value(&message).expect("serialize chat message");
    let decoded: ChatMessage =
        serde_json::from_value(json_value).expect("deserialize chat message");

    assert_eq!(decoded, message);
    assert_eq!(decoded.message, content);
}

#[test]
fn chat_message_version_mismatch_is_invalid_payload() {
    let raw = json!({
        "version": 2,
        "timestampMicros": 1,
        "sequenceId": 9,
        "sender": "participant-bob",
        "message": "こんにちは"
    });

    let err = ChatMessage::from_json_body(&raw).expect_err("version 2 should be unsupported");

    assert!(matches!(
        err,
        SyncMessageError::UnsupportedVersion { received } if received == 2
    ));
}
