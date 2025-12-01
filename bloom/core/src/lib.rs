pub mod id;
pub mod room;
pub mod signaling;

pub use id::{ParticipantId, RoomId};
pub use room::{CreateRoomResult, JoinRoomError, ParticipantList, RoomManager};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_room_id_is_valid_uuid() {
        let room_id = RoomId::new();

        assert_ne!(
            *room_id.as_uuid(),
            uuid::Uuid::nil(),
            "生成されたRoomIdはnil UUIDではない"
        );
    }

    #[test]
    fn generated_participant_id_is_valid_uuid() {
        let participant_id = ParticipantId::new();

        assert_ne!(
            *participant_id.as_uuid(),
            uuid::Uuid::nil(),
            "生成されたParticipantIdはnil UUIDではない"
        );
    }

    #[test]
    fn create_room_returns_ids_and_self_is_only_participant() {
        let mut manager = RoomManager::new();
        let room_owner_id = ParticipantId::new();

        let result = manager.create_room(room_owner_id);

        assert!(
            *result.room_id.as_uuid() != uuid::Uuid::nil(),
            "room_idはnilであってはならない"
        );
        assert!(
            *result.self_id.as_uuid() != uuid::Uuid::nil(),
            "self_idはnilであってはならない"
        );
        assert_eq!(
            result.participants,
            vec![result.self_id.clone()],
            "作成直後は作成者のみが参加者"
        );
    }

    #[test]
    fn join_adds_unique_participant() {
        let mut manager = RoomManager::new();
        let owner = ParticipantId::new();
        let create = manager.create_room(owner.clone());
        let room_id = create.room_id.clone();

        let new_participant = ParticipantId::new();
        let joined = manager
            .join_room(&room_id, new_participant.clone())
            .expect("room should exist")
            .expect("should join without error");
        assert_eq!(joined.len(), 2, "オーナーと新規参加者の2名になるはず");
        assert!(joined.contains(&owner));
        assert!(joined.contains(&new_participant));

        // 同じ参加者が再度joinしても重複しない
        let joined_again = manager
            .join_room(&room_id, new_participant.clone())
            .expect("room should exist")
            .expect("should allow duplicate join as no-op");
        assert_eq!(joined_again.len(), 2);
    }

    #[test]
    fn join_rejects_when_full() {
        let mut manager = RoomManager::new();
        let owner = ParticipantId::new();
        let create = manager.create_room(owner);
        let room_id = create.room_id.clone();

        for _ in 0..6 {
            let _ = manager
                .join_room(&room_id, ParticipantId::new())
                .expect("room exists")
                .expect("should not be full yet");
        }
        let _ = manager
            .join_room(&room_id, ParticipantId::new())
            .expect("room exists")
            .expect("8人目までは許容");

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
    fn leave_removes_participant_and_keeps_others() {
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

        assert!(!after_leave.contains(&p2));
        assert!(after_leave.contains(&owner));
        assert!(after_leave.contains(&p3));
        assert_eq!(after_leave.len(), 2);
    }

    #[test]
    fn leave_removes_room_when_empty() {
        let mut manager = RoomManager::new();
        let owner = ParticipantId::new();
        let create = manager.create_room(owner.clone());
        let room_id = create.room_id.clone();

        let after_leave = manager
            .leave_room(&room_id, &owner)
            .expect("room exists for leave");
        assert!(after_leave.is_empty());

        let join_after_empty = manager.join_room(&room_id, ParticipantId::new());
        assert!(join_after_empty.is_none());
    }

    #[test]
    fn order_preserved_on_join_and_leave() {
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

        let after_leave_p2 = manager
            .leave_room(&room_id, &p2)
            .expect("room exists after leave");
        assert_eq!(after_leave_p2, vec![owner.clone(), p3.clone()]);

        let p4 = ParticipantId::new();
        let after_p4 = manager
            .join_room(&room_id, p4.clone())
            .expect("room exists")
            .expect("join p4 ok");
        assert_eq!(after_p4, vec![owner, p3, p4]);
    }

    #[test]
    fn smoke_sequence_reflects_state() {
        let mut manager = RoomManager::new();
        let owner = ParticipantId::new();
        let create = manager.create_room(owner.clone());
        let room_id = create.room_id.clone();

        let p2 = ParticipantId::new();
        let p3 = ParticipantId::new();
        let p4 = ParticipantId::new();

        let _ = manager
            .join_room(&room_id, p2.clone())
            .expect("room exists")
            .expect("join p2 ok");
        let after_p3 = manager
            .join_room(&room_id, p3.clone())
            .expect("room exists")
            .expect("join p3 ok");
        assert_eq!(after_p3, vec![owner.clone(), p2.clone(), p3.clone()]);

        let after_leave_p2 = manager
            .leave_room(&room_id, &p2)
            .expect("room exists after leave");
        assert_eq!(after_leave_p2, vec![owner.clone(), p3.clone()]);

        let after_p4 = manager
            .join_room(&room_id, p4.clone())
            .expect("room exists")
            .expect("join p4 ok");
        assert_eq!(after_p4, vec![owner.clone(), p3.clone(), p4.clone()]);

        let final_list = manager
            .leave_room(&room_id, &owner)
            .expect("room exists after owner leaves");
        assert_eq!(final_list, vec![p3.clone(), p4.clone()]);
    }
}
