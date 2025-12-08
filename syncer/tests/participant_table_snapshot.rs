use std::collections::HashSet;

use bloom_core::ParticipantId;

use syncer::participant_table::ParticipantTable;

#[test]
fn participants_returns_current_members_after_join_and_leave() {
    let mut table = ParticipantTable::new();
    let alice = ParticipantId::new();
    let bob = ParticipantId::new();

    table.apply_join(alice.clone());
    table.apply_join(bob.clone());
    table.apply_leave(alice.clone());

    let snapshot: HashSet<_> = table.participants().into_iter().collect();

    assert!(
        !snapshot.contains(&alice),
        "alice should be absent after leave"
    );

    assert!(snapshot.contains(&bob), "bob should remain in snapshot");
    assert_eq!(snapshot.len(), 1, "only bob should remain registered");
}
