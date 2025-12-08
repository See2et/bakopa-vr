use bloom_core::ParticipantId;

use syncer::{participant_table::ParticipantTable, SyncerEvent};

#[test]
fn apply_leave_emits_peer_left_once_and_removes_participant() {
    let mut table = ParticipantTable::new();
    let alice = ParticipantId::new();

    // Precondition: alice is registered via join
    table.apply_join(alice.clone());

    let events = table.apply_leave(alice.clone());

    assert!(
        events.iter().any(|event| matches!(
            event,
            SyncerEvent::PeerLeft { participant_id } if participant_id == &alice
        )),
        "expected PeerLeft event for alice"
    );

    assert!(
        !table.is_registered(&alice),
        "alice should be removed from table after leave"
    );
}
