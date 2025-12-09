use std::collections::{HashMap, VecDeque};

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
#[derive(Default, Debug)]
pub struct InMemorySignalingHub {
    queues: HashMap<ParticipantId, VecDeque<SignalMessage>>,
}

impl InMemorySignalingHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, participant: ParticipantId) {
        self.queues.entry(participant).or_default();
    }

    pub fn send(&mut self, msg: SignalMessage) {
        if let Some(queue) = self.queues.get_mut(&msg.to) {
            queue.push_back(msg);
        }
    }

    pub fn drain_for(&mut self, participant: &ParticipantId) -> Vec<SignalMessage> {
        if let Some(queue) = self.queues.get_mut(participant) {
            return queue.drain(..).collect();
        }
        Vec::new()
    }
}
