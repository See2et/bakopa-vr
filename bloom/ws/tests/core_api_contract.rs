use bloom_core::{ParticipantId, RoomId};
use bloom_ws::{CoreApi, MockCore, RealCore, SharedCore};

fn run_contract<C: CoreApi>(mut core: C) {
    let owner = ParticipantId::new();
    let create = core.create_room(owner.clone());
    assert_eq!(create.participants.len(), 1);
    assert!(create.participants.contains(&owner));

    let room_id = create.room_id.clone();
    let joiner = ParticipantId::new();
    let joined = core
        .join_room(&room_id, joiner.clone())
        .expect("room exists")
        .expect("join ok");
    assert!(joined.contains(&owner));
    assert!(joined.contains(&joiner));

    let left = core
        .leave_room(&room_id, &joiner)
        .expect("room exists after leave");
    assert!(!left.contains(&joiner));
}

#[test]
fn contract_real_core() {
    let core = RealCore::new();
    run_contract(core);
}

#[test]
fn contract_mock_core() {
    let room_id = RoomId::new();
    let owner = ParticipantId::new();
    let joiner = ParticipantId::new();
    let mut mock = MockCore::new(bloom_core::CreateRoomResult {
        room_id: room_id.clone(),
        self_id: owner.clone(),
        participants: vec![owner.clone()],
    })
    .with_join_result(Some(Ok(vec![owner.clone(), joiner.clone()])))
    .with_leave_result(Some(vec![owner.clone()]))
    .with_participants(room_id.clone(), vec![owner.clone(), joiner.clone()]);

    // 手動で簡易チェック（MockCoreは状態整合性より記録優先のため緩め）
    let create = mock.create_room(owner.clone());
    assert_eq!(create.room_id, room_id);

    let joined = mock
        .join_room(&room_id, joiner.clone())
        .expect("room exists")
        .expect("join ok");
    assert_eq!(joined.len(), 2);

    let left = mock
        .leave_room(&room_id, &joiner)
        .expect("room exists after leave");
    assert_eq!(left.len(), 1);
}
