mod common;

use bloom_core::{ParticipantId, RoomId};
use common::bus_transport::{new_bus, BusTransport};
use common::{sample_chat, sample_pose, sample_tracing_context};
use syncer::{BasicSyncer, Syncer, SyncerEvent, SyncerRequest};

#[test]
fn pose_flow_delivers_once_to_peer() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let bus = new_bus();

    let ta = BusTransport::new(a.clone(), bus.clone());
    let tb = BusTransport::new(b.clone(), bus.clone());

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // A sends pose
    syncer_a.handle(SyncerRequest::SendPose {
        from: a.clone(),
        pose: sample_pose(),
        ctx: sample_tracing_context(&room, &a),
    });

    // B processes incoming events (no additional send)
    let events = syncer_b.handle(SyncerRequest::SendPose {
        from: b.clone(),
        pose: sample_pose(),
        ctx: sample_tracing_context(&room, &b),
    });

    let pose_count = events
        .iter()
        .filter(|e| matches!(e, SyncerEvent::PoseReceived { .. }))
        .count();
    assert_eq!(pose_count, 1, "B should receive exactly one pose");

    let ctx = events
        .into_iter()
        .find_map(|e| match e {
            SyncerEvent::PoseReceived { ctx, from, .. } => Some((ctx, from)),
            _ => None,
        })
        .expect("expected pose event");

    assert_eq!(ctx.0.room_id, room);
    assert_eq!(ctx.0.participant_id, a);
    assert_eq!(ctx.0.stream_kind, syncer::StreamKind::Pose);
    assert_eq!(ctx.1, a);
}

#[test]
fn chat_flow_delivers_once_to_peer_with_tracing() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let bus = new_bus();

    let ta = BusTransport::new(a.clone(), bus.clone());
    let tb = BusTransport::new(b.clone(), bus.clone());

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    let chat = sample_chat(&a);

    syncer_a.handle(SyncerRequest::SendChat {
        chat: chat.clone(),
        ctx: sample_tracing_context(&room, &a),
    });

    let events = syncer_b.handle(SyncerRequest::SendChat {
        chat: sample_chat(&b),
        ctx: sample_tracing_context(&room, &b),
    });

    let chat_event = events
        .into_iter()
        .find_map(|e| match e {
            SyncerEvent::ChatReceived { chat, ctx } => Some((chat, ctx)),
            _ => None,
        })
        .expect("expected chat event");

    assert_eq!(chat_event.0.sender, a.to_string());
    assert_eq!(chat_event.1.room_id, room);
    assert_eq!(chat_event.1.participant_id, a);
    assert_eq!(chat_event.1.stream_kind, syncer::StreamKind::Chat);
}
