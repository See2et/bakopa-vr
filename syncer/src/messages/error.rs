#[derive(Debug, Clone, PartialEq)]
pub enum SyncMessageError {
    MissingVersion,
    UnsupportedVersion { received: u32 },
    UnknownKind { value: String },
    BodyTooLarge { bytes: usize },
    BodyJsonMalformed,
    SchemaViolation { kind: String, reason: &'static str },
}

pub mod reason {
    pub const BODY_NOT_OBJECT: &str = "body_not_object";
    pub const VERSION_NOT_U32: &str = "version_not_u32";
    pub const MISSING_KIND: &str = "missing_kind";
    pub const KIND_NOT_STRING: &str = "kind_not_string";
    pub const MISSING_BODY: &str = "missing_body";
    pub const SERIALIZE_FAILED: &str = "serialize_failed";
    pub const MISSING_HEAD: &str = "missing_head";
    pub const INVALID_POSE: &str = "invalid_pose";
    pub const MISSING_SENDER: &str = "missing_sender";
    pub const MESSAGE_LENGTH: &str = "message_length";
    pub const INVALID_CHAT: &str = "invalid_chat";
    pub const UNSUPPORTED_KIND: &str = "unsupported_kind";
    pub const KIND_MISMATCH: &str = "kind_mismatch";
    pub const MISSING_TYPE: &str = "missing_type";
    pub const MISSING_ROOM_ID: &str = "missing_room_id";
    pub const MISSING_AUTH_TOKEN: &str = "missing_auth_token";
    pub const MISSING_ICE_POLICY: &str = "missing_ice_policy";
    pub const MISSING_SDP: &str = "missing_sdp";
    pub const INVALID_OFFER: &str = "invalid_offer";
    pub const INVALID_ANSWER: &str = "invalid_answer";
    pub const INVALID_ICE: &str = "invalid_ice";
    pub const MISSING_CANDIDATE: &str = "missing_candidate";
    pub const INVALID_CANDIDATE: &str = "invalid_candidate";
}
