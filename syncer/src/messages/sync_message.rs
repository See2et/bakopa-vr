use crate::StreamKind;

use super::chat::ChatMessage;
use super::control::ControlMessage;
use super::envelope::SyncMessageEnvelope;
use super::error::SyncMessageError;
use super::pose::PoseMessage;
use super::signaling::SignalingMessage;

#[derive(Debug, Clone, PartialEq)]
pub enum SyncMessage {
    Pose(PoseMessage),
    Chat(ChatMessage),
    Control(ControlMessage),
    Signaling(SignalingMessage),
}

impl SyncMessage {
    pub fn into_envelope(self) -> Result<SyncMessageEnvelope, SyncMessageError> {
        match self {
            SyncMessage::Pose(pose) => SyncMessageEnvelope::from_pose(pose),
            SyncMessage::Chat(chat) => SyncMessageEnvelope::from_chat(chat),
            SyncMessage::Control(control) => SyncMessageEnvelope::from_control(control),
            SyncMessage::Signaling(signaling) => SyncMessageEnvelope::from_signaling(signaling),
        }
    }

    pub fn from_envelope(envelope: SyncMessageEnvelope) -> Result<Self, SyncMessageError> {
        match envelope.kind {
            StreamKind::Pose => PoseMessage::try_from(envelope).map(SyncMessage::Pose),
            StreamKind::Chat => ChatMessage::try_from(envelope).map(SyncMessage::Chat),
            StreamKind::ControlJoin | StreamKind::ControlLeave => {
                ControlMessage::try_from(envelope).map(SyncMessage::Control)
            }
            StreamKind::SignalingOffer | StreamKind::SignalingAnswer | StreamKind::SignalingIce => {
                SignalingMessage::try_from(envelope).map(SyncMessage::Signaling)
            }
            other => Err(SyncMessageError::UnknownKind {
                value: other.as_str().to_string(),
            }),
        }
    }
}
