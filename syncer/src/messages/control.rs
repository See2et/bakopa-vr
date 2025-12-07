use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::convert::TryFrom;

use crate::StreamKind;

use super::envelope::SyncMessageEnvelope;
use super::error::reason;
use super::error::SyncMessageError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ControlMessage {
    Join(ControlPayload),
    Leave(ControlPayload),
}

impl ControlMessage {
    pub fn from_json_body(value: &JsonValue) -> Result<Self, SyncMessageError> {
        if !value.is_object() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "control".to_string(),
                reason: reason::BODY_NOT_OBJECT,
            });
        }

        serde_json::from_value(value.clone()).map_err(|_| SyncMessageError::SchemaViolation {
            kind: "control".to_string(),
            reason: reason::UNSUPPORTED_KIND,
        })
    }

    pub fn kind_stream(&self) -> StreamKind {
        match self {
            ControlMessage::Join(_) => StreamKind::ControlJoin,
            ControlMessage::Leave(_) => StreamKind::ControlLeave,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlPayload {
    pub participant_id: String,
    #[serde(default)]
    pub reconnect_token: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

impl TryFrom<SyncMessageEnvelope> for ControlMessage {
    type Error = SyncMessageError;

    fn try_from(envelope: SyncMessageEnvelope) -> Result<Self, Self::Error> {
        match envelope.kind {
            StreamKind::ControlJoin | StreamKind::ControlLeave => {
                ControlMessage::from_json_body(&envelope.body)
            }
            _ => Err(SyncMessageError::SchemaViolation {
                kind: "control".to_string(),
                reason: reason::KIND_MISMATCH,
            }),
        }
    }
}
