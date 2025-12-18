use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::convert::TryFrom;

use crate::StreamKind;

use super::envelope::SyncMessageEnvelope;
use super::error::reason;
use super::error::SyncMessageError;

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
                reason: reason::BODY_NOT_OBJECT,
            });
        }

        let obj = value.as_object().expect("checked is_object");
        if !obj.contains_key("head") {
            return Err(SyncMessageError::SchemaViolation {
                kind: "pose".to_string(),
                reason: reason::MISSING_HEAD,
            });
        }

        let pose: PoseMessage = serde_json::from_value(value.clone()).map_err(|_| {
            SyncMessageError::SchemaViolation {
                kind: "pose".to_string(),
                reason: reason::INVALID_POSE,
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
                reason: reason::KIND_MISMATCH,
            });
        }

        PoseMessage::from_json_body(&envelope.body)
    }
}
