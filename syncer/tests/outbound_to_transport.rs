mod common;

use bloom_core::ParticipantId;
use common::{sample_chat, sample_pose};
use syncer::{Outbound, OutboundPayload, StreamKind};

#[test]
fn pose_outbound_serializes_with_kind_and_version() {
    let pose = sample_pose();
    let from = ParticipantId::new();
    let to = ParticipantId::new();
    let outbound = Outbound {
        from: from.clone(),
        to,
        stream_kind: StreamKind::Pose,
        payload: OutboundPayload::Pose(pose.clone()),
    };

    let payload = outbound
        .into_transport_payload()
        .expect("serialize pose outbound");

    let envelope = payload
        .parse_envelope()
        .expect("payload should parse to envelope");

    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.kind, StreamKind::Pose);
    assert_eq!(envelope.body, serde_json::to_value(pose).unwrap());
}

#[test]
fn chat_outbound_serializes_with_kind_and_version() {
    let from = ParticipantId::new();
    let chat = sample_chat(&from);
    let outbound = Outbound {
        from: from.clone(),
        to: ParticipantId::new(),
        stream_kind: StreamKind::Chat,
        payload: OutboundPayload::Chat(chat.clone()),
    };

    let payload = outbound
        .into_transport_payload()
        .expect("serialize chat outbound");

    let envelope = payload
        .parse_envelope()
        .expect("payload should parse to envelope");

    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.kind, StreamKind::Chat);
    assert_eq!(envelope.body, serde_json::to_value(chat).unwrap());
}
