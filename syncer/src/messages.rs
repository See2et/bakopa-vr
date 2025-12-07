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
    pub kind: SyncMessageKind,
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
        let kind = SyncMessageKind::from_str(kind_str)?;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMessageKind {
    #[serde(rename = "pose")]
    Pose,
    #[serde(rename = "chat")]
    Chat,
    #[serde(rename = "control.join")]
    ControlJoin,
    #[serde(rename = "control.leave")]
    ControlLeave,
    #[serde(rename = "signaling.offer")]
    SignalingOffer,
    #[serde(rename = "signaling.answer")]
    SignalingAnswer,
    #[serde(rename = "signaling.ice")]
    SignalingIce,
}

impl SyncMessageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SyncMessageKind::Pose => "pose",
            SyncMessageKind::Chat => "chat",
            SyncMessageKind::ControlJoin => "control.join",
            SyncMessageKind::ControlLeave => "control.leave",
            SyncMessageKind::SignalingOffer => "signaling.offer",
            SyncMessageKind::SignalingAnswer => "signaling.answer",
            SyncMessageKind::SignalingIce => "signaling.ice",
        }
    }
}

impl FromStr for SyncMessageKind {
    type Err = SyncMessageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pose" => Ok(SyncMessageKind::Pose),
            "chat" => Ok(SyncMessageKind::Chat),
            "control.join" => Ok(SyncMessageKind::ControlJoin),
            "control.leave" => Ok(SyncMessageKind::ControlLeave),
            "signaling.offer" => Ok(SyncMessageKind::SignalingOffer),
            "signaling.answer" => Ok(SyncMessageKind::SignalingAnswer),
            "signaling.ice" => Ok(SyncMessageKind::SignalingIce),
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
