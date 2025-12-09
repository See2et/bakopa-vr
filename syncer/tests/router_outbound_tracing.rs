use bloom_core::{ParticipantId, RoomId};

mod common;
use common::{sample_chat, sample_pose};
use syncer::{
    participant_table::ParticipantTable, Router, StreamKind, SyncerEvent, TracingContext,
};

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
    let pose_outbound = router
        .route_pose(&sender, sample_pose(), &table)
        .pop()
        .expect("pose outbound expected");
    let pose_event = pose_outbound.into_event(&room);
    assert!(matches!(
        pose_event,
        SyncerEvent::PoseReceived {
            ctx: TracingContext {
                stream_kind: StreamKind::Pose,
                room_id,
                participant_id
            },
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
    let chat_event = chat_outbound.into_event(&room);
    assert!(matches!(
        chat_event,
        SyncerEvent::ChatReceived {
            ctx: TracingContext {
                stream_kind: StreamKind::Chat,
                room_id,
                participant_id
            },
            chat: _,
        } if room_id == room && participant_id == sender
    ));
}
