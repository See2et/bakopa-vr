mod common;

use bloom_core::{ParticipantId, RoomId};
use common::bus_transport::{new_bus, BusTransport};
use syncer::{BasicSyncer, Syncer, SyncerEvent, SyncerRequest};


#[test]
fn audio_frame_is_emitted_as_voice_event_with_context() {
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

    // A sends audio frame via official API
    syncer_a.handle(SyncerRequest::SendVoiceFrame {
        frame: vec![1, 2, 3],
        ctx: common::sample_voice_context(&room, &a),
    });

    // B processes incoming; expect a VoiceFrameReceived
    let events = syncer_b.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&b),
        ctx: common::sample_tracing_context(&room, &b),
    });

    let voice_event = events
        .into_iter()
        .find_map(|e| match e {
            SyncerEvent::VoiceFrameReceived { from, frame, ctx } => Some((from, frame, ctx)),
            _ => None,
        })
        .expect("expected voice frame event");

    assert_eq!(voice_event.0, a);
    assert_eq!(voice_event.1, vec![1, 2, 3]);
    assert_eq!(voice_event.2.room_id, room);
    assert_eq!(voice_event.2.participant_id, a);
    assert_eq!(voice_event.2.stream_kind, syncer::StreamKind::Voice);
}
