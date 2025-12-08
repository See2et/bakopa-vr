use std::collections::HashMap;

use bloom_core::ParticipantId;

use crate::SyncerEvent;

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
