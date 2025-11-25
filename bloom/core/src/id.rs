use std::fmt;

use uuid::Uuid;

/// ルームを一意に識別するID。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RoomId(Uuid);

impl RoomId {
    /// UUID v4 を生成してRoomIdを作る。
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 参加者を一意に識別するID。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParticipantId(Uuid);

impl ParticipantId {
    /// UUID v4 を生成してParticipantIdを作る。
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for ParticipantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
