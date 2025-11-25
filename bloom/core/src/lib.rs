//! Core domain types and room management skeleton for Bloom signaling.
//! 現時点では仕様書 2-0 準備段階の骨組みのみを提供する。

/// ルームを一意に識別するID。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RoomId(String);

impl RoomId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 参加者を一意に識別するID。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParticipantId(String);

impl ParticipantId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// ルーム・参加者管理を担うコンポーネントの骨組み。
#[derive(Default)]
pub struct RoomManager;

impl RoomManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新規Roomを作成し、作成者自身を最初の参加者として登録する。
    /// TODO: 実装はこれから。現段階ではRedテストを発火させるためのスタブ。
    pub fn create_room(&mut self) -> CreateRoomResult {
        unimplemented!("create_room is not implemented yet");
    }
}

/// Room作成時の戻り値。
#[derive(Clone, Debug)]
pub struct CreateRoomResult {
    pub room_id: RoomId,
    pub self_id: ParticipantId,
    pub participants: Vec<ParticipantId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_room_returns_ids_and_self_is_only_participant() {
        let mut manager = RoomManager::new();

        let result = manager.create_room();

        assert!(
            !result.room_id.as_str().is_empty(),
            "room_idは空であってはならない"
        );
        assert!(
            !result.self_id.as_str().is_empty(),
            "self_idは空であってはならない"
        );
        assert_eq!(
            result.participants,
            vec![result.self_id.clone()],
            "作成直後は作成者のみが参加者"
        );
    }
}
