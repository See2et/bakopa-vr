mod common;

use common::sample_pose;
use bloom_core::ParticipantId;
use syncer::{participant_table::ParticipantTable, Router, StreamKind};

#[test]
fn route_pose_sends_to_other_participants_only() {
    let sender = ParticipantId::new();
    let receiver = ParticipantId::new();

    let mut table = ParticipantTable::new();
    table.apply_join(sender.clone());
    table.apply_join(receiver.clone());

    let router = Router::new();
    let outbound = router.route_pose(&sender, sample_pose(), &table);

    assert_eq!(outbound.len(), 1, "expected exactly one outbound packet");
    assert_eq!(
        outbound[0].to, receiver,
        "should target the other participant only"
    );
    assert_eq!(
        outbound[0].stream_kind,
        StreamKind::Pose,
        "pose routing must set stream_kind=pose"
    );
}

#[test]
fn route_pose_returns_empty_when_only_sender_present() {
    let sender = ParticipantId::new();
    let mut table = ParticipantTable::new();
    table.apply_join(sender.clone());

    let router = Router::new();
    let outbound = router.route_pose(&sender, sample_pose(), &table);

    assert!(outbound.is_empty(), "should not send pose back to self");
}

#[test]
fn route_pose_excludes_left_participants_and_still_reaches_remaining() {
    let sender = ParticipantId::new();
    let left = ParticipantId::new();
    let still_here = ParticipantId::new();

    let mut table = ParticipantTable::new();
    table.apply_join(sender.clone());
    table.apply_join(left.clone());
    table.apply_leave(left.clone());

    let router = Router::new();
    let after_leave = router.route_pose(&sender, sample_pose(), &table);
    assert!(
        after_leave.is_empty(),
        "should not deliver to participants that have already left"
    );

    table.apply_join(still_here.clone());
    let after_rejoin = router.route_pose(&sender, sample_pose(), &table);
    assert_eq!(
        after_rejoin.len(),
        1,
        "should deliver to remaining participants after leave"
    );
    assert_eq!(
        after_rejoin[0].to, still_here,
        "delivery should target newly joined participant"
    );
}
