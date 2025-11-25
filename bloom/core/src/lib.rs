use std::collections::HashMap;

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
pub struct RoomManager {
    rooms: HashMap<RoomId, RoomState>,
}

#[derive(Clone, Debug)]
struct RoomState {
    participants: Vec<ParticipantId>,
}

impl RoomManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新規Roomを作成し、作成者自身を最初の参加者として登録する。
    pub fn create_room(&mut self, room_owner: ParticipantId) -> CreateRoomResult {
        let room_id = RoomId::new();
        let self_id = room_owner;
        let participants = vec![self_id.clone()];

        let state = RoomState {
            participants: participants.clone(),
        };
        self.rooms.insert(room_id.clone(), state);

        CreateRoomResult {
            room_id,
            self_id,
            participants,
        }
    }

    /// 既存Roomに参加者を追加し、最新の参加者リストを返す。
    pub fn join_room(
        &mut self,
        room_id: &RoomId,
        participant: ParticipantId,
    ) -> Option<Vec<ParticipantId>> {
        if let Some(room) = self.rooms.get_mut(room_id) {
            if !room.participants.contains(&participant) {
                room.participants.push(participant);
            }
            Some(room.participants.clone())
        } else {
            None
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

    #[test]
    fn join_room_adds_new_participant_and_returns_deduped_list() {
        let mut manager = RoomManager::new();
        let owner = ParticipantId::new();
        let create = manager.create_room(owner.clone());
        let room_id = create.room_id.clone();

        let new_participant = ParticipantId::new();
        let joined = manager
            .join_room(&room_id, new_participant.clone())
            .expect("room should exist");

        assert_eq!(joined.len(), 2, "オーナーと新規参加者の2名になるはず");
        assert!(
            joined.contains(&owner),
            "参加者リストにオーナーが残っていること"
        );
        assert!(
            joined.contains(&new_participant),
            "参加者リストに新規参加者が含まれること"
        );

        // 同じ参加者が再度joinしても重複しない
        let joined_again = manager
            .join_room(&room_id, new_participant.clone())
            .expect("room should exist");
        assert_eq!(
            joined_again.len(),
            2,
            "同一参加者で二重に増えないこと（重複防止）"
        );
    }
}
