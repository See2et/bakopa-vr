use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ErrorCode {
    RoomFull,
    InvalidPayload,
    ParticipantNotFound,
    RateLimited,
    Internal,
}
