use serde_json::json;
use syncer::messages::{SignalingIce, SignalingMessage, SyncMessageError};

fn sample_ice() -> SignalingIce {
    SignalingIce {
        version: 1,
        room_id: "room-xyz".into(),
        participant_id: "alice".into(),
        auth_token: "INSECURE_DEV".into(),
        candidate: "candidate:0 1 UDP 2122252543 192.0.2.1 3478 typ host".into(),
        sdp_mid: Some("0".into()),
        sdp_mline_index: Some(0),
    }
}

#[test]
fn ice_message_round_trip() {
    let ice = SignalingMessage::Ice(sample_ice());

    let value = serde_json::to_value(&ice).expect("serialize ice");
    let decoded: SignalingMessage = serde_json::from_value(value).expect("deserialize ice");

    assert_eq!(decoded, ice);
}

#[test]
fn ice_message_with_old_version_is_rejected() {
    let raw = json!({
        "type": "ice",
        "version": 0,
        "roomId": "room-xyz",
        "participantId": "alice",
        "authToken": "INSECURE_DEV",
        "candidate": "candidate:0 1 UDP 2122252543 192.0.2.1 3478 typ host",
        "sdpMid": "0",
        "sdpMLineIndex": 0
    });

    let err = SignalingMessage::from_json_body(&raw).expect_err("version 0 should be unsupported");

    assert!(matches!(
        err,
        SyncMessageError::UnsupportedVersion { received } if received == 0
    ));
}
