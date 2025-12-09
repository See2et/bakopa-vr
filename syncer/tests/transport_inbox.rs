mod common;

use bloom_core::{ParticipantId, RoomId};
use common::{sample_chat, sample_pose};
use syncer::{
    participant_table::ParticipantTable, Outbound, OutboundPayload, StreamKind, SyncerEvent,
    TransportEvent, TransportInbox,
};

fn setup_participants() -> (RoomId, ParticipantId, ParticipantId, ParticipantTable) {
    let room_id = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let mut table = ParticipantTable::new();
    table.apply_join(a.clone());
    table.apply_join(b.clone());

    (room_id, a, b, table)
}

#[test]
fn pose_received_is_parsed_with_tracing_context() {
    let (room_id, from, to, participants) = setup_participants();

    let outbound = Outbound {
        from: from.clone(),
        to,
        stream_kind: StreamKind::Pose,
        payload: OutboundPayload::Pose(sample_pose()),
    };

    let payload = outbound
        .into_transport_payload()
        .expect("serialize outbound");

    let events = TransportInbox::from_events(vec![TransportEvent::Received { from: from.clone(), payload }])
        .drain_into_events(&room_id, &participants);

    let pose = events
        .into_iter()
        .find_map(|e| match e {
            SyncerEvent::PoseReceived { from: sender, ctx, .. } => Some((sender, ctx)),
            _ => None,
        })
        .expect("expected PoseReceived event");

    assert_eq!(pose.0, from);
    assert_eq!(pose.1.room_id, room_id);
    assert_eq!(pose.1.participant_id, pose.0);
    assert_eq!(pose.1.stream_kind, StreamKind::Pose);
}

#[test]
fn chat_received_is_parsed_with_tracing_context() {
    let (room_id, from, to, participants) = setup_participants();

    let outbound = Outbound {
        from: from.clone(),
        to,
        stream_kind: StreamKind::Chat,
        payload: OutboundPayload::Chat(sample_chat(&from)),
    };

    let payload = outbound
        .into_transport_payload()
        .expect("serialize outbound");

    let events = TransportInbox::from_events(vec![TransportEvent::Received { from: from.clone(), payload }])
        .drain_into_events(&room_id, &participants);

    let chat = events
        .into_iter()
        .find_map(|e| match e {
            SyncerEvent::ChatReceived { chat, ctx } => Some((chat, ctx)),
            _ => None,
        })
        .expect("expected ChatReceived event");

    assert_eq!(chat.0.sender, from.to_string());
    assert_eq!(chat.1.room_id, room_id);
    assert_eq!(chat.1.participant_id, from);
    assert_eq!(chat.1.stream_kind, StreamKind::Chat);
}

#[test]
fn invalid_payload_is_reported_as_error_event() {
    let (room_id, from, _to, participants) = setup_participants();

    let invalid_payload = syncer::TransportPayload::Bytes(vec![]);

    let events = TransportInbox::from_events(vec![TransportEvent::Received { from, payload: invalid_payload }])
        .drain_into_events(&room_id, &participants);

    assert!(events
        .iter()
        .any(|e| matches!(e, SyncerEvent::Error { .. })), "expected Error event for invalid payload");
}
