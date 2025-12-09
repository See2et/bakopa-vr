use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::convert::TryFrom;

use crate::StreamKind;

use super::envelope::SyncMessageEnvelope;
use super::error::reason;
use super::error::SyncMessageError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SignalingMessage {
    Offer(SignalingOffer),
    Answer(SignalingAnswer),
    Ice(SignalingIce),
}

impl SignalingMessage {
    pub fn from_json_body(value: &JsonValue) -> Result<Self, SyncMessageError> {
        if !value.is_object() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::BODY_NOT_OBJECT,
            });
        }

        let obj = value.as_object().expect("checked is_object");
        let signaling_type = obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
            SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::MISSING_TYPE,
            }
        })?;

        let message: SignalingMessage = match signaling_type {
            "offer" => {
                Self::ensure_field(obj, "roomId", reason::MISSING_ROOM_ID)?;
                Self::ensure_field(obj, "authToken", reason::MISSING_AUTH_TOKEN)?;
                Self::ensure_field(obj, "icePolicy", reason::MISSING_ICE_POLICY)?;
                Self::ensure_field(obj, "sdp", reason::MISSING_SDP)?;
                serde_json::from_value(value.clone()).map_err(|_| {
                    SyncMessageError::SchemaViolation {
                        kind: "signaling".to_string(),
                        reason: reason::INVALID_OFFER,
                    }
                })?
            }
            "answer" => {
                Self::ensure_field(obj, "roomId", reason::MISSING_ROOM_ID)?;
                Self::ensure_field(obj, "authToken", reason::MISSING_AUTH_TOKEN)?;
                Self::ensure_field(obj, "sdp", reason::MISSING_SDP)?;
                serde_json::from_value(value.clone()).map_err(|_| {
                    SyncMessageError::SchemaViolation {
                        kind: "signaling".to_string(),
                        reason: reason::INVALID_ANSWER,
                    }
                })?
            }
            "ice" => {
                Self::ensure_field(obj, "roomId", reason::MISSING_ROOM_ID)?;
                Self::ensure_field(obj, "authToken", reason::MISSING_AUTH_TOKEN)?;
                Self::ensure_field(obj, "candidate", reason::MISSING_CANDIDATE)?;
                serde_json::from_value(value.clone()).map_err(|_| {
                    SyncMessageError::SchemaViolation {
                        kind: "signaling".to_string(),
                        reason: reason::INVALID_ICE,
                    }
                })?
            }
            _ => {
                return Err(SyncMessageError::SchemaViolation {
                    kind: "signaling".to_string(),
                    reason: reason::UNSUPPORTED_KIND,
                })
            }
        };

        message.validate()?;
        Ok(message)
    }

    fn ensure_field(
        obj: &JsonMap<String, JsonValue>,
        key: &str,
        reason_code: &'static str,
    ) -> Result<(), SyncMessageError> {
        if !obj.contains_key(key) {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason_code,
            });
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), SyncMessageError> {
        match self {
            SignalingMessage::Offer(offer) => offer.validate(),
            SignalingMessage::Answer(answer) => answer.validate(),
            SignalingMessage::Ice(ice) => ice.validate(),
        }
    }

    pub fn kind_stream(&self) -> StreamKind {
        match self {
            SignalingMessage::Offer(_) => StreamKind::SignalingOffer,
            SignalingMessage::Answer(_) => StreamKind::SignalingAnswer,
            SignalingMessage::Ice(_) => StreamKind::SignalingIce,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalingOffer {
    pub version: u32,
    pub room_id: String,
    pub participant_id: String,
    pub auth_token: String,
    pub ice_policy: String,
    pub sdp: String,
}

impl SignalingOffer {
    fn validate(&self) -> Result<(), SyncMessageError> {
        if self.version != 1 {
            return Err(SyncMessageError::UnsupportedVersion {
                received: self.version,
            });
        }
        if self.room_id.is_empty() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::MISSING_ROOM_ID,
            });
        }
        if self.auth_token.is_empty() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::MISSING_AUTH_TOKEN,
            });
        }
        if self.sdp.is_empty() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::MISSING_SDP,
            });
        }
        if self.ice_policy.is_empty() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::MISSING_ICE_POLICY,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalingAnswer {
    pub version: u32,
    pub room_id: String,
    pub participant_id: String,
    pub auth_token: String,
    pub sdp: String,
}

impl SignalingAnswer {
    fn validate(&self) -> Result<(), SyncMessageError> {
        if self.version != 1 {
            return Err(SyncMessageError::UnsupportedVersion {
                received: self.version,
            });
        }
        if self.room_id.is_empty() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::MISSING_ROOM_ID,
            });
        }
        if self.auth_token.is_empty() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::MISSING_AUTH_TOKEN,
            });
        }
        if self.sdp.is_empty() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::MISSING_SDP,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalingIce {
    pub version: u32,
    pub room_id: String,
    pub participant_id: String,
    pub auth_token: String,
    pub candidate: String,
    #[serde(default)]
    pub sdp_mid: Option<String>,
    #[serde(default)]
    pub sdp_mline_index: Option<u16>,
}

impl SignalingIce {
    const MAX_CANDIDATE_LEN: usize = 1024;

    fn validate(&self) -> Result<(), SyncMessageError> {
        if self.version != 1 {
            return Err(SyncMessageError::UnsupportedVersion {
                received: self.version,
            });
        }
        if self.room_id.is_empty() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::MISSING_ROOM_ID,
            });
        }
        if self.auth_token.is_empty() {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::MISSING_AUTH_TOKEN,
            });
        }
        if self.candidate.is_empty() || self.candidate.len() > Self::MAX_CANDIDATE_LEN {
            return Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::INVALID_CANDIDATE,
            });
        }
        Ok(())
    }
}

impl TryFrom<SyncMessageEnvelope> for SignalingMessage {
    type Error = SyncMessageError;

    fn try_from(envelope: SyncMessageEnvelope) -> Result<Self, Self::Error> {
        match envelope.kind {
            StreamKind::SignalingOffer | StreamKind::SignalingAnswer | StreamKind::SignalingIce => {
                SignalingMessage::from_json_body(&envelope.body)
            }
            _ => Err(SyncMessageError::SchemaViolation {
                kind: "signaling".to_string(),
                reason: reason::KIND_MISMATCH,
            }),
        }
    }
}
