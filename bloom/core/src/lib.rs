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

const MAX_PARTICIPANTS: usize = 8;

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
    ) -> Option<Result<Vec<ParticipantId>, JoinRoomError>> {
        if let Some(room) = self.rooms.get_mut(room_id) {
            if room.participants.len() >= MAX_PARTICIPANTS
                && !room.participants.contains(&participant)
            {
                return Some(Err(JoinRoomError::RoomFull));
            }
            if !room.participants.contains(&participant) {
                room.participants.push(participant);
            }
            Some(Ok(room.participants.clone()))
        } else {
            None
        }
    }

    /// 指定参加者をRoomから離脱させ、最新の参加者リストを返す。
    pub fn leave_room(
        &mut self,
        room_id: &RoomId,
        participant: &ParticipantId,
    ) -> Option<Vec<ParticipantId>> {
        if let Some(room) = self.rooms.get_mut(room_id) {
            room.participants.retain(|p| p != participant);
            if room.participants.is_empty() {
                self.rooms.remove(room_id);
                return Some(vec![]);
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinRoomError {
    RoomFull,
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

        let joined = joined.expect("should join without error");
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
        let joined_again = joined_again.expect("should allow duplicate join as no-op");
        assert_eq!(
            joined_again.len(),
            2,
            "同一参加者で二重に増えないこと（重複防止）"
        );
    }

    #[test]
    fn join_room_returns_room_full_when_exceeding_capacity() {
        let mut manager = RoomManager::new();
        let owner = ParticipantId::new();
        let create = manager.create_room(owner);
        let room_id = create.room_id.clone();

        // 7人追加で合計8人までは成功
        for _ in 0..6 {
            let _ = manager
                .join_room(&room_id, ParticipantId::new())
                .expect("room exists")
                .expect("should not be full yet");
        }
        // 8人目はまだ許容される
        let _ = manager
            .join_room(&room_id, ParticipantId::new())
            .expect("room exists")
            .expect("8人目までは許容");

        // 9人目でRoomFullエラー
        let ninth_result = manager
            .join_room(&room_id, ParticipantId::new())
            .expect("room exists");

        match ninth_result {
            Err(JoinRoomError::RoomFull) => {}
            Ok(list) => panic!(
                "9人目は受け入れずRoomFullを返すべきだが {:?} を返した",
                list
            ),
        }
    }

    #[test]
    fn leave_room_removes_participant_and_keeps_others() {
        let mut manager = RoomManager::new();
        let owner = ParticipantId::new();
        let create = manager.create_room(owner.clone());
        let room_id = create.room_id.clone();

        let p2 = ParticipantId::new();
        let p3 = ParticipantId::new();
        let _ = manager
            .join_room(&room_id, p2.clone())
            .expect("room exists")
            .expect("join p2 ok");
        let _ = manager
            .join_room(&room_id, p3.clone())
            .expect("room exists")
            .expect("join p3 ok");

        let after_leave = manager
            .leave_room(&room_id, &p2)
            .expect("room exists for leave");

        assert!(
            !after_leave.contains(&p2),
            "離脱した参加者はリストから除去される"
        );
        assert!(after_leave.contains(&owner), "他の参加者は残る（オーナー）");
        assert!(after_leave.contains(&p3), "他の参加者は残る（p3）");
        assert_eq!(after_leave.len(), 2, "残り2名のはず");
    }

    #[test]
    fn leave_room_removes_room_when_empty() {
        let mut manager = RoomManager::new();
        let owner = ParticipantId::new();
        let create = manager.create_room(owner.clone());
        let room_id = create.room_id.clone();

        let after_leave = manager
            .leave_room(&room_id, &owner)
            .expect("room exists for leave");
        assert!(
            after_leave.is_empty(),
            "最後の参加者が抜ければリストは空になる"
        );

        // 部屋が削除されているため、再joinはroomなしとして扱われることを期待
        let join_after_empty = manager.join_room(&room_id, ParticipantId::new());
        assert!(
            join_after_empty.is_none(),
            "空になった部屋は削除され、join不可であるべき"
        );
    }

    #[test]
    fn participant_list_preserves_join_order() {
        let mut manager = RoomManager::new();
        let owner = ParticipantId::new();
        let create = manager.create_room(owner.clone());
        let room_id = create.room_id.clone();

        let p2 = ParticipantId::new();
        let p3 = ParticipantId::new();

        let after_p2 = manager
            .join_room(&room_id, p2.clone())
            .expect("room exists")
            .expect("join p2 ok");
        assert_eq!(after_p2, vec![owner.clone(), p2.clone()]);

        let after_p3 = manager
            .join_room(&room_id, p3.clone())
            .expect("room exists")
            .expect("join p3 ok");
        assert_eq!(after_p3, vec![owner.clone(), p2.clone(), p3.clone()]);

        // p2 leaves; order of remaining should keep insertion order of the survivors
        let after_leave_p2 = manager
            .leave_room(&room_id, &p2)
            .expect("room exists after leave");
        assert_eq!(after_leave_p2, vec![owner.clone(), p3.clone()]);

        // new participant joins; should append to the end
        let p4 = ParticipantId::new();
        let after_p4 = manager
            .join_room(&room_id, p4.clone())
            .expect("room exists")
            .expect("join p4 ok");
        assert_eq!(after_p4, vec![owner, p3, p4]);
    }
}
