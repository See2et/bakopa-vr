use crate::StreamKind;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::convert::TryFrom;

use super::chat::ChatMessage;
use super::control::ControlMessage;
use super::error::reason;
use super::error::SyncMessageError;
use super::pose::PoseMessage;
use super::signaling::SignalingMessage;

pub const MAX_ENVELOPE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncMessageEnvelope {
    #[serde(rename = "v")]
    pub version: u32,
    #[serde(rename = "kind")]
    pub kind: StreamKind,
    #[serde(rename = "body")]
    pub body: JsonValue,
}

impl SyncMessageEnvelope {
    pub const MAX_BYTES: usize = MAX_ENVELOPE_BYTES;

    pub fn from_slice(bytes: &[u8]) -> Result<Self, SyncMessageError> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(SyncMessageError::BodyTooLarge { bytes: bytes.len() });
        }

        let raw: JsonValue =
            serde_json::from_slice(bytes).map_err(|_| SyncMessageError::BodyJsonMalformed)?;

        let envelope = raw
            .as_object()
            .ok_or_else(|| SyncMessageError::SchemaViolation {
                kind: "envelope".to_string(),
                reason: reason::BODY_NOT_OBJECT,
            })?;

        let version_value = envelope.get("v").ok_or(SyncMessageError::MissingVersion)?;
        let version_u64 =
            version_value
                .as_u64()
                .ok_or_else(|| SyncMessageError::SchemaViolation {
                    kind: "envelope".to_string(),
                    reason: reason::VERSION_NOT_U32,
                })?;
        let version =
            u32::try_from(version_u64).map_err(|_| SyncMessageError::SchemaViolation {
                kind: "envelope".to_string(),
                reason: reason::VERSION_NOT_U32,
            })?;
        if version != 1 {
            return Err(SyncMessageError::UnsupportedVersion { received: version });
        }

        let kind_value = envelope
            .get("kind")
            .ok_or_else(|| SyncMessageError::SchemaViolation {
                kind: "envelope".to_string(),
                reason: reason::MISSING_KIND,
            })?;
        let kind_str = kind_value
            .as_str()
            .ok_or_else(|| SyncMessageError::SchemaViolation {
                kind: "envelope".to_string(),
                reason: reason::KIND_NOT_STRING,
            })?;
        let kind = StreamKind::parse(kind_str)?;

        let body_value = envelope
            .get("body")
            .ok_or_else(|| SyncMessageError::SchemaViolation {
                kind: kind.as_str().to_string(),
                reason: reason::MISSING_BODY,
            })?;
        if !body_value.is_object() {
            return Err(SyncMessageError::SchemaViolation {
                kind: kind.as_str().to_string(),
                reason: reason::BODY_NOT_OBJECT,
            });
        }

        Ok(SyncMessageEnvelope {
            version,
            kind,
            body: body_value.clone(),
        })
    }

    pub fn from_pose(message: PoseMessage) -> Result<Self, SyncMessageError> {
        if message.version != 1 {
            return Err(SyncMessageError::UnsupportedVersion {
                received: message.version,
            });
        }

        let body =
            serde_json::to_value(&message).map_err(|_| SyncMessageError::SchemaViolation {
                kind: "pose".to_string(),
                reason: reason::SERIALIZE_FAILED,
            })?;

        Ok(SyncMessageEnvelope {
            version: 1,
            kind: StreamKind::Pose,
            body,
        })
    }

    pub fn from_chat(message: ChatMessage) -> Result<Self, SyncMessageError> {
        message.validate()?;

        let body =
            serde_json::to_value(&message).map_err(|_| SyncMessageError::SchemaViolation {
                kind: "chat".to_string(),
                reason: reason::SERIALIZE_FAILED,
            })?;

        Ok(SyncMessageEnvelope {
            version: 1,
            kind: StreamKind::Chat,
            body,
        })
    }

    pub fn from_control(message: ControlMessage) -> Result<Self, SyncMessageError> {
        let body =
            serde_json::to_value(&message).map_err(|_| SyncMessageError::SchemaViolation {
                kind: "control".to_string(),
                reason: reason::SERIALIZE_FAILED,
            })?;

        Ok(SyncMessageEnvelope {
            version: 1,
            kind: message.kind_stream(),
            body,
        })
    }

    pub fn from_signaling(message: SignalingMessage) -> Result<Self, SyncMessageError> {
        message.validate()?;

        let body =
            serde_json::to_value(&message).map_err(|_| SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::SERIALIZE_FAILED,
            })?;

        Ok(SyncMessageEnvelope {
            version: 1,
            kind: message.kind_stream(),
            body,
        })
    }
}
