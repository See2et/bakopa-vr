use crate::StreamKind;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value as JsonValue};
use std::convert::TryFrom;
use std::str::FromStr;

const MAX_ENVELOPE_BYTES: usize = 64 * 1024;

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
                reason: "not_object",
            })?;

        let version_value = envelope.get("v").ok_or(SyncMessageError::MissingVersion)?;
        let version_u64 =
            version_value
                .as_u64()
                .ok_or_else(|| SyncMessageError::SchemaViolation {
                    kind: "envelope".to_string(),
                    reason: "version_not_u32",
                })?;
        let version =
            u32::try_from(version_u64).map_err(|_| SyncMessageError::SchemaViolation {
                kind: "envelope".to_string(),
                reason: "version_not_u32",
            })?;
        if version != 1 {
            return Err(SyncMessageError::UnsupportedVersion { received: version });
        }

        let kind_value = envelope
            .get("kind")
            .ok_or_else(|| SyncMessageError::SchemaViolation {
                kind: "envelope".to_string(),
                reason: "missing_kind",
            })?;
        let kind_str = kind_value
            .as_str()
            .ok_or_else(|| SyncMessageError::SchemaViolation {
                kind: "envelope".to_string(),
                reason: "kind_not_string",
            })?;
        let kind = StreamKind::from_str(kind_str)?;

        let body_value = envelope
            .get("body")
            .ok_or_else(|| SyncMessageError::SchemaViolation {
                kind: kind.as_str().to_string(),
                reason: "missing_body",
            })?;
        if !body_value.is_object() {
            return Err(SyncMessageError::SchemaViolation {
                kind: kind.as_str().to_string(),
                reason: "body_not_object",
            });
        }

        Ok(SyncMessageEnvelope {
            version,
            kind,
            body: body_value.clone(),
        })
    }
}

impl FromStr for StreamKind {
    type Err = SyncMessageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pose" => Ok(StreamKind::Pose),
            "chat" => Ok(StreamKind::Chat),
            "voice" => Ok(StreamKind::Voice),
            "control.join" => Ok(StreamKind::ControlJoin),
            "control.leave" => Ok(StreamKind::ControlLeave),
            "signaling.offer" => Ok(StreamKind::SignalingOffer),
            "signaling.answer" => Ok(StreamKind::SignalingAnswer),
            "signaling.ice" => Ok(StreamKind::SignalingIce),
            other => Err(SyncMessageError::UnknownKind {
                value: other.to_string(),
            }),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum SyncMessageError {
    MissingVersion,
    UnsupportedVersion { received: u32 },
    UnknownKind { value: String },
    BodyTooLarge { bytes: usize },
    BodyJsonMalformed,
    SchemaViolation { kind: String, reason: &'static str },
}
