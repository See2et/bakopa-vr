use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::convert::TryFrom;

use crate::StreamKind;

use super::envelope::SyncMessageEnvelope;
use super::error::reason;
use super::error::SyncMessageError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub version: u32,
    pub timestamp_micros: u64,
    pub sequence_id: u64,
    pub sender: String,
    pub message: String,
}

impl ChatMessage {
    pub const MAX_MESSAGE_LEN: usize = 2048;

    pub fn from_json_body(value: &JsonValue) -> Result<Self, SyncMessageError> {
        if !value.is_object() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "chat".to_string(),
                reason: reason::BODY_NOT_OBJECT,
            });
        }

        let msg: ChatMessage = serde_json::from_value(value.clone()).map_err(|_| {
            SyncMessageError::SchemaViolation {
                kind: "chat".to_string(),
                reason: reason::INVALID_CHAT,
            }
        })?;

        msg.validate()?;
        Ok(msg)
    }

    pub fn validate(&self) -> Result<(), SyncMessageError> {
        if self.version != 1 {
            return Err(SyncMessageError::UnsupportedVersion {
                received: self.version,
            });
        }

        if self.sender.is_empty() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "chat".to_string(),
                reason: reason::MISSING_SENDER,
            });
        }

        if self.message.is_empty() || self.message.chars().count() > Self::MAX_MESSAGE_LEN {
            return Err(SyncMessageError::SchemaViolation {
                kind: "chat".to_string(),
                reason: reason::MESSAGE_LENGTH,
            });
        }

        Ok(())
    }
}

impl TryFrom<SyncMessageEnvelope> for ChatMessage {
    type Error = SyncMessageError;

    fn try_from(envelope: SyncMessageEnvelope) -> Result<Self, Self::Error> {
        if envelope.kind != StreamKind::Chat {
            return Err(SyncMessageError::SchemaViolation {
                kind: "chat".to_string(),
                reason: reason::KIND_MISMATCH,
            });
        }

        ChatMessage::from_json_body(&envelope.body)
    }
}
