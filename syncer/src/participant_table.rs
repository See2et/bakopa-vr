use std::collections::HashMap;
use std::str::FromStr;

use bloom_core::ParticipantId;

use crate::{PendingPeerEvent, PendingPeerEventKind, SyncerError, SyncerEvent};

#[derive(Default)]
pub struct ParticipantTable {
    sessions: HashMap<ParticipantId, SessionId>,
    next_session: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(u64);

impl ParticipantTable {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_session: 1,
        }
    }

    /// Apply a join event and return emitted SyncerEvents.
    pub fn apply_join(&mut self, participant: ParticipantId) -> Vec<SyncerEvent> {
        let mut events = Vec::new();

        if self.sessions.contains_key(&participant) {
            events.push(SyncerEvent::PeerLeft {
                participant_id: participant.clone(),
            });
        }

        let session = self.allocate_session();
        self.sessions.insert(participant.clone(), session);
        events.push(SyncerEvent::PeerJoined {
            participant_id: participant,
        });

        events
    }

    /// Apply a leave event and return emitted SyncerEvents.
    pub fn apply_leave(&mut self, participant: ParticipantId) -> Vec<SyncerEvent> {
        match self.sessions.remove(&participant) {
            Some(_session) => vec![SyncerEvent::PeerLeft {
                participant_id: participant,
            }],
            None => Vec::new(),
        }
    }

    /// Apply a PendingPeerEvent originating from ControlMessage notifications.
    pub fn apply_pending_peer_event(&mut self, event: PendingPeerEvent) -> Vec<SyncerEvent> {
        let participant_id = match ParticipantId::from_str(&event.participant_id) {
            Ok(participant_id) => participant_id,
            Err(_) => {
                return vec![SyncerEvent::Error {
                    kind: SyncerError::InvalidParticipantId {
                        raw_value: event.participant_id,
                    },
                }]
            }
        };

        match event.kind {
            PendingPeerEventKind::Joined => self.apply_join(participant_id),
            PendingPeerEventKind::Left => self.apply_leave(participant_id),
        }
    }

    /// Returns true if the participant is currently registered in the table.
    pub fn is_registered(&self, participant: &ParticipantId) -> bool {
        self.sessions.contains_key(participant)
    }

    /// Returns a snapshot of the registered participants.
    pub fn participants(&self) -> Vec<ParticipantId> {
        self.sessions.keys().cloned().collect()
    }

    fn allocate_session(&mut self) -> SessionId {
        let session = SessionId(self.next_session);
        self.next_session += 1;
        session
    }
}
