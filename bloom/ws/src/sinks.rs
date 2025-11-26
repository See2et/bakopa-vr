use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bloom_api::ServerToClient;
use bloom_core::ParticipantId;

/// Outgoing sink abstraction (e.g., a WebSocket sender).
pub trait OutSink {
    fn send(&mut self, message: ServerToClient);
}

/// Broadcast sink that can deliver messages to specific participants.
pub trait BroadcastSink {
    fn send_to(&mut self, to: &ParticipantId, message: ServerToClient);
}

/// Test helper sink that records messages.
#[derive(Default, Debug)]
pub struct RecordingSink {
    pub sent: Vec<ServerToClient>,
}

impl OutSink for RecordingSink {
    fn send(&mut self, message: ServerToClient) {
        self.sent.push(message);
    }
}

/// Test helper broadcast sink that records messages per participant.
#[derive(Default, Debug)]
pub struct RecordingBroadcastSink {
    pub sent: HashMap<ParticipantId, Vec<ServerToClient>>,
}

impl RecordingBroadcastSink {
    pub fn messages_for(&self, participant: &ParticipantId) -> Option<&[ServerToClient]> {
        self.sent.get(participant).map(Vec::as_slice)
    }
}

impl BroadcastSink for RecordingBroadcastSink {
    fn send_to(&mut self, to: &ParticipantId, message: ServerToClient) {
        self.sent.entry(to.clone()).or_default().push(message);
    }
}

/// Shared broadcast sink for simulating multiple connections sharing delivery.
#[derive(Clone, Default)]
pub struct SharedBroadcastSink {
    inner: Arc<Mutex<HashMap<ParticipantId, Vec<ServerToClient>>>>,
}

impl SharedBroadcastSink {
    pub fn messages_for(&self, participant: &ParticipantId) -> Option<Vec<ServerToClient>> {
        self.inner
            .lock()
            .ok()
            .and_then(|m| m.get(participant).cloned())
    }
}

impl BroadcastSink for SharedBroadcastSink {
    fn send_to(&mut self, to: &ParticipantId, message: ServerToClient) {
        if let Ok(mut map) = self.inner.lock() {
            map.entry(to.clone()).or_default().push(message);
        }
    }
}

/// No-op broadcast sink for cases where we do not care about broadcast.
#[derive(Default)]
pub struct NoopBroadcastSink;

impl BroadcastSink for NoopBroadcastSink {
    fn send_to(&mut self, _to: &ParticipantId, _message: ServerToClient) {}
}
