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

/// ルーム・参加者管理を担うコンポーネントの骨組み。
#[derive(Default)]
pub struct RoomManager;

impl RoomManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新規Roomを作成し、作成者自身を最初の参加者として登録する。
    pub fn create_room(&mut self, room_owner: ParticipantId) -> CreateRoomResult {
        let room_id = RoomId::new();
        let self_id = room_owner;
        let participants = vec![self_id.clone()];

        CreateRoomResult {
            room_id,
            self_id,
            participants,
        }
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
    fn generated_room_id_is_valid_uuid() {
        let room_id = RoomId::new();

        assert_ne!(
            *room_id.as_uuid(),
            Uuid::nil(),
            "生成されたRoomIdはnil UUIDではない"
        );
    }

    #[test]
    fn generated_participant_id_is_valid_uuid() {
        let participant_id = ParticipantId::new();

        assert_ne!(
            *participant_id.as_uuid(),
            Uuid::nil(),
            "生成されたParticipantIdはnil UUIDではない"
        );
    }

    #[test]
    fn create_room_returns_ids_and_self_is_only_participant() {
        let mut manager = RoomManager::new();
        let room_owner_id = ParticipantId::new();

        let result = manager.create_room(room_owner_id);

        assert!(
            *result.room_id.as_uuid() != Uuid::nil(),
            "room_idはnilであってはならない"
        );
        assert!(
            *result.self_id.as_uuid() != Uuid::nil(),
            "self_idはnilであってはならない"
        );
        assert_eq!(
            result.participants,
            vec![result.self_id.clone()],
            "作成直後は作成者のみが参加者"
        );
    }
}
