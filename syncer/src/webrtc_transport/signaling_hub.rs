use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use bloom_core::ParticipantId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalKind {
    Offer,
    Answer,
    Ice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalMessage {
    pub from: ParticipantId,
    pub to: ParticipantId,
    pub kind: SignalKind,
    pub payload: String,
}

/// シンプルなin-memoryシグナリングハブ。登録済みピア宛てにメッセージをキューする。
#[derive(Default, Debug, Clone)]
pub struct InMemorySignalingHub {
    inner: Arc<Mutex<HashMap<ParticipantId, VecDeque<SignalMessage>>>>,
}

impl InMemorySignalingHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, participant: ParticipantId) {
        let mut guard = self.inner.lock().unwrap();
        guard.entry(participant).or_default();
    }

    pub fn send(&self, msg: SignalMessage) {
        let mut guard = self.inner.lock().unwrap();
        if let Some(queue) = guard.get_mut(&msg.to) {
            queue.push_back(msg);
        }
    }

    pub fn drain_for(&self, participant: &ParticipantId) -> Vec<SignalMessage> {
        let mut guard = self.inner.lock().unwrap();
        if let Some(queue) = guard.get_mut(participant) {
            return queue.drain(..).collect();
        }
        Vec::new()
    }
}
