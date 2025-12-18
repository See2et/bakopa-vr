use bloom_core::{ParticipantId, RoomId};
use syncer::{StubSyncer, Syncer, SyncerRequest};

#[test]
fn join_request_returns_events_vector() {
    let mut syncer = StubSyncer;
    let room_id = RoomId::new();
    let participant_id = ParticipantId::new();

    let _ = syncer.handle(SyncerRequest::Join {
        room_id,
        participant_id,
    });
}
