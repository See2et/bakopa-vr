use bloom_core::ParticipantId;

use syncer::{participant_table::ParticipantTable, SyncerEvent};

#[test]
fn apply_join_emits_peer_joined_and_registers_participant() {
    let mut table = ParticipantTable::new();
    let alice = ParticipantId::new();

    let events = table.apply_join(alice.clone());

    assert!(
        events.iter().any(|event| matches!(
            event,
            SyncerEvent::PeerJoined { participant_id } if participant_id == &alice
        )),
        "expected PeerJoined event for alice"
    );

    assert!(
        table.is_registered(&alice),
        "alice should be registered in the table after join"
    );
}
