use serde_json::json;
use syncer::messages::{SignalingAnswer, SignalingMessage, SignalingOffer, SyncMessageError};

fn sample_offer() -> SignalingOffer {
    SignalingOffer {
        version: 1,
        room_id: "room-123".into(),
        participant_id: "alice".into(),
        auth_token: "INSECURE_DEV".into(),
        ice_policy: "default".into(),
        sdp: "v=0\no=- 0 0 IN IP4 127.0.0.1\n".into(),
    }
}

fn sample_answer() -> SignalingAnswer {
    SignalingAnswer {
        version: 1,
        room_id: "room-123".into(),
        participant_id: "bob".into(),
        auth_token: "INSECURE_DEV".into(),
        sdp: "v=0\no=- 0 0 IN IP4 127.0.0.1\n".into(),
    }
}

#[test]
fn signaling_offer_round_trip() {
    let offer = SignalingMessage::Offer(sample_offer());

    let value = serde_json::to_value(&offer).expect("serialize offer");
    let decoded: SignalingMessage = serde_json::from_value(value).expect("deserialize offer");

    assert_eq!(decoded, offer);
}

#[test]
fn signaling_answer_round_trip() {
    let answer = SignalingMessage::Answer(sample_answer());

    let value = serde_json::to_value(&answer).expect("serialize answer");
    let decoded: SignalingMessage = serde_json::from_value(value).expect("deserialize answer");

    assert_eq!(decoded, answer);
}

#[test]
fn missing_room_id_causes_schema_violation() {
    let raw = json!({
        "type": "offer",
        "version": 1,
        "participantId": "alice",
        "authToken": "INSECURE_DEV",
        "icePolicy": "default",
        "sdp": "v=0\n"
    });

    let err = SignalingMessage::from_json_body(&raw).expect_err("room_id is required");

    assert!(matches!(
        err,
        SyncMessageError::SchemaViolation { kind, reason }
        if kind == "signaling" && reason == "missing_room_id"
    ));
}

#[test]
fn missing_auth_token_causes_schema_violation() {
    let raw = json!({
        "type": "answer",
        "version": 1,
        "participantId": "bob",
        "roomId": "room-123",
        "sdp": "v=0\n"
    });

    let err = SignalingMessage::from_json_body(&raw).expect_err("auth token is required");

    assert!(matches!(
        err,
        SyncMessageError::SchemaViolation { kind, reason }
        if kind == "signaling" && reason == "missing_auth_token"
    ));
}
