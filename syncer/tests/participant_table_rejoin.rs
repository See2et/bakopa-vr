use bloom_core::ParticipantId;

use syncer::{participant_table::ParticipantTable, SyncerEvent};

#[test]
fn rejoin_emits_peer_left_then_peer_joined_in_order() {
    let mut table = ParticipantTable::new();
    let alice = ParticipantId::new();

    table.apply_join(alice.clone());

    let events = table.apply_join(alice.clone());

    assert_eq!(
        events.len(),
        2,
        "rejoin should emit two events (PeerLeft then PeerJoined)"
    );

    assert!(
        matches!(
            &events[0],
            SyncerEvent::PeerLeft { participant_id } if participant_id == &alice
        ),
        "first event must be PeerLeft for alice"
    );

    assert!(
        matches!(
            &events[1],
            SyncerEvent::PeerJoined { participant_id } if participant_id == &alice
        ),
        "second event must be PeerJoined for alice"
    );

    assert!(
        table.is_registered(&alice),
        "alice should remain registered after rejoin"
    );
}
