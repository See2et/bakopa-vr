use bloom_core::{ParticipantId, RoomId};
use syncer::{StubSyncer, Syncer, SyncerEvent, SyncerRequest};

/// Join が複数イベント（SelfJoined + PeerJoined）を返すことを期待
#[test]
fn join_returns_self_and_peer_joined_events() {
    let mut syncer = StubSyncer;

    let room_id = RoomId::new();
    let alice = ParticipantId::new();
    let bob = ParticipantId::new();

    // まず Alice が参加（既存参加者を作る）
    let _ = syncer.handle(SyncerRequest::Join {
        room_id: room_id.clone(),
        participant_id: alice.clone(),
    });

    // Bob 参加時に SelfJoined(Bob) と PeerJoined(Alice) が一度に返ることを期待
    let events = syncer.handle(SyncerRequest::Join {
        room_id,
        participant_id: bob.clone(),
    });

    assert!(
        events.iter().any(|e| matches!(
            e,
            SyncerEvent::SelfJoined { participant_id, .. } if participant_id == &bob
        )),
        "expected SelfJoined for bob"
    );

    assert!(
        events.iter().any(|e| matches!(
            e,
            SyncerEvent::PeerJoined { participant_id } if participant_id == &alice
        )),
        "expected PeerJoined for alice"
    );
}
