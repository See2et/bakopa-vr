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

    pub fn from_pose(message: PoseMessage) -> Result<Self, SyncMessageError> {
        if message.version != 1 {
            return Err(SyncMessageError::UnsupportedVersion {
                received: message.version,
            });
        }

        let body =
            serde_json::to_value(&message).map_err(|_| SyncMessageError::SchemaViolation {
                kind: "pose".to_string(),
                reason: "serialize_failed",
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
                reason: "serialize_failed",
            })?;

        Ok(SyncMessageEnvelope {
            version: 1,
            kind: StreamKind::Chat,
            body,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoseMessage {
    pub version: u32,
    pub timestamp_micros: u64,
    pub head: PoseTransform,
    #[serde(default)]
    pub hand_l: Option<PoseTransform>,
    #[serde(default)]
    pub hand_r: Option<PoseTransform>,
}

impl PoseMessage {
    pub fn from_json_body(value: &JsonValue) -> Result<Self, SyncMessageError> {
        if !value.is_object() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "pose".to_string(),
                reason: "body_not_object",
            });
        }

        let obj = value.as_object().expect("checked is_object");
        if !obj.contains_key("head") {
            return Err(SyncMessageError::SchemaViolation {
                kind: "pose".to_string(),
                reason: "missing_head",
            });
        }

        let pose: PoseMessage = serde_json::from_value(value.clone()).map_err(|_| {
            SyncMessageError::SchemaViolation {
                kind: "pose".to_string(),
                reason: "invalid_pose",
            }
        })?;

        if pose.version != 1 {
            return Err(SyncMessageError::UnsupportedVersion {
                received: pose.version,
            });
        }

        Ok(pose)
    }
}

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
                reason: "body_not_object",
            });
        }

        let msg: ChatMessage = serde_json::from_value(value.clone()).map_err(|_| {
            SyncMessageError::SchemaViolation {
                kind: "chat".to_string(),
                reason: "invalid_chat",
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
                reason: "missing_sender",
            });
        }

        if self.message.is_empty() || self.message.chars().count() > Self::MAX_MESSAGE_LEN {
            return Err(SyncMessageError::SchemaViolation {
                kind: "chat".to_string(),
                reason: "message_length",
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoseTransform {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

impl TryFrom<SyncMessageEnvelope> for PoseMessage {
    type Error = SyncMessageError;

    fn try_from(envelope: SyncMessageEnvelope) -> Result<Self, Self::Error> {
        if envelope.kind != StreamKind::Pose {
            return Err(SyncMessageError::SchemaViolation {
                kind: "pose".to_string(),
                reason: "kind_mismatch",
            });
        }

        PoseMessage::from_json_body(&envelope.body)
    }
}

impl TryFrom<SyncMessageEnvelope> for ChatMessage {
    type Error = SyncMessageError;

    fn try_from(envelope: SyncMessageEnvelope) -> Result<Self, Self::Error> {
        if envelope.kind != StreamKind::Chat {
            return Err(SyncMessageError::SchemaViolation {
                kind: "chat".to_string(),
                reason: "kind_mismatch",
            });
        }

        ChatMessage::from_json_body(&envelope.body)
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
