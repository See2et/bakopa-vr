use bloom_core::ParticipantId;

use syncer::messages::ControlPayload;
use syncer::{
    participant_table::ParticipantTable, ControlMessage, PendingPeerEvent, SyncerError, SyncerEvent,
};

fn leave_event_for(participant: &ParticipantId) -> PendingPeerEvent {
    leave_event_with_raw(&participant.to_string())
}

fn join_event_for(participant: &ParticipantId) -> PendingPeerEvent {
    join_event_with_raw(&participant.to_string())
}

fn leave_event_with_raw(raw: &str) -> PendingPeerEvent {
    let payload = ControlPayload {
        participant_id: raw.to_string(),
        reconnect_token: None,
        reason: None,
    };

    PendingPeerEvent::from(ControlMessage::Leave(payload))
}

fn join_event_with_raw(raw: &str) -> PendingPeerEvent {
    let payload = ControlPayload {
        participant_id: raw.to_string(),
        reconnect_token: None,
        reason: None,
    };

    PendingPeerEvent::from(ControlMessage::Join(payload))
}

#[test]
fn duplicate_control_leave_events_are_idempotent() {
    let mut table = ParticipantTable::new();
    let alice = ParticipantId::new();

    let join_events = table.apply_pending_peer_event(join_event_for(&alice));
    assert!(
        join_events.iter().any(|event| matches!(
            event,
            SyncerEvent::PeerJoined { participant_id } if participant_id == &alice
        )),
        "setup join must register alice"
    );

    let first_leave = table.apply_pending_peer_event(leave_event_for(&alice));
    assert_eq!(
        first_leave.len(),
        1,
        "first leave should emit exactly one event"
    );
    assert!(matches!(
        &first_leave[0],
        SyncerEvent::PeerLeft { participant_id } if participant_id == &alice
    ));

    let second_leave = table.apply_pending_peer_event(leave_event_for(&alice));
    assert!(
        second_leave.is_empty(),
        "duplicate leave events should not emit PeerLeft twice"
    );
}

#[test]
fn invalid_participant_id_emits_error_event() {
    let mut table = ParticipantTable::new();
    let invalid_raw = "not-a-valid-participant-id";

    let events = table.apply_pending_peer_event(join_event_with_raw(invalid_raw));

    assert!(
        events.iter().any(|event| matches!(
            event,
            SyncerEvent::Error { kind: SyncerError::InvalidParticipantId { raw_value } }
                if raw_value == invalid_raw
        )),
        "invalid participant id should emit error event"
    );
}
