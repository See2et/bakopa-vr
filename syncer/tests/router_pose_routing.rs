mod common;

use bloom_core::ParticipantId;
use common::sample_pose;
use syncer::{
    messages::ChatMessage, participant_table::ParticipantTable, OutboundPayload, Router, StreamKind,
    SyncerEvent, TracingContext,
};
use bloom_core::RoomId;

#[test]
fn route_pose_sends_to_other_participants_only() {
    let sender = ParticipantId::new();
    let receiver = ParticipantId::new();

    let mut table = ParticipantTable::new();
    table.apply_join(sender.clone());
    table.apply_join(receiver.clone());

    let router = Router::new();
    let outbound = router.route_pose(&sender, sample_pose(), &table);

    assert_eq!(outbound.len(), 1, "expected exactly one outbound packet");
    assert_eq!(
        outbound[0].to, receiver,
        "should target the other participant only"
    );
    assert_eq!(
        outbound[0].stream_kind,
        StreamKind::Pose,
        "pose routing must set stream_kind=pose"
    );
}

#[test]
fn route_pose_returns_empty_when_only_sender_present() {
    let sender = ParticipantId::new();
    let mut table = ParticipantTable::new();
    table.apply_join(sender.clone());

    let router = Router::new();
    let outbound = router.route_pose(&sender, sample_pose(), &table);

    assert!(outbound.is_empty(), "should not send pose back to self");
}

#[test]
fn route_pose_excludes_left_participants_and_still_reaches_remaining() {
    let sender = ParticipantId::new();
    let left = ParticipantId::new();
    let still_here = ParticipantId::new();

    let mut table = ParticipantTable::new();
    table.apply_join(sender.clone());
    table.apply_join(left.clone());
    table.apply_leave(left.clone());

    let router = Router::new();
    let after_leave = router.route_pose(&sender, sample_pose(), &table);
    assert!(
        after_leave.is_empty(),
        "should not deliver to participants that have already left"
    );

    table.apply_join(still_here.clone());
    let after_rejoin = router.route_pose(&sender, sample_pose(), &table);
    assert_eq!(
        after_rejoin.len(),
        1,
        "should deliver to remaining participants after leave"
    );
    assert_eq!(
        after_rejoin[0].to, still_here,
        "delivery should target newly joined participant"
    );
}

#[test]
fn route_chat_mirrors_pose_routing_and_sets_stream_kind_chat() {
    let sender = ParticipantId::new();
    let receiver = ParticipantId::new();

    let mut table = ParticipantTable::new();
    table.apply_join(sender.clone());
    table.apply_join(receiver.clone());

    let chat = sample_chat(&sender);
    let router = Router::new();
    let outbound = router.route_chat(&sender, chat.clone(), &table);

    assert_eq!(outbound.len(), 1, "expected one chat outbound");
    let packet = &outbound[0];
    assert_eq!(
        packet.to, receiver,
        "chat should target the other participant"
    );
    assert_eq!(
        packet.stream_kind,
        StreamKind::Chat,
        "chat routing must set stream_kind=chat"
    );
    assert!(
        matches!(&packet.payload, OutboundPayload::Chat(m) if *m == chat),
        "payload should carry the original chat message"
    );
}

#[test]
fn outbound_to_event_fills_tracing_context_with_stream_kind_pose_and_chat() {
    let room = RoomId::new();
    let sender = ParticipantId::new();
    let receiver = ParticipantId::new();

    let mut table = ParticipantTable::new();
    table.apply_join(sender.clone());
    table.apply_join(receiver.clone());

    let router = Router::new();

    // Pose path
    let pose = sample_pose();
    let pose_outbound = router
        .route_pose(&sender, pose.clone(), &table)
        .pop()
        .expect("pose outbound expected");
    let pose_event = pose_outbound.into_event(&sender, &room);
    assert!(matches!(
        pose_event,
        SyncerEvent::PoseReceived {
            ctx: TracingContext { stream_kind: StreamKind::Pose, room_id, participant_id },
            from,
            pose: _
        } if room_id == room && from == sender && participant_id == sender
    ));

    // Chat path
    let chat = sample_chat(&sender);
    let chat_outbound = router
        .route_chat(&sender, chat.clone(), &table)
        .pop()
        .expect("chat outbound expected");
    let chat_event = chat_outbound.into_event(&sender, &room);
    assert!(matches!(
        chat_event,
        SyncerEvent::ChatReceived {
            ctx: TracingContext { stream_kind: StreamKind::Chat, room_id, participant_id },
            chat: _,
        } if room_id == room && participant_id == sender
    ));
}

fn sample_chat(sender: &ParticipantId) -> ChatMessage {
    ChatMessage {
        version: 1,
        timestamp_micros: 0,
        sequence_id: 1,
        sender: sender.to_string(),
        message: "hello".to_string(),
    }
}
