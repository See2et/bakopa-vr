use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ErrorCode {
    RoomFull,
    RoomNotFound,
    InvalidPayload,
    ParticipantNotFound,
    RateLimited,
    Internal,
}
