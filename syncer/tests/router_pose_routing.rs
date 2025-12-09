use bloom_core::ParticipantId;

mod common;

use common::sample_pose;
use syncer::{Router, StreamKind};

#[test]
fn route_pose_sends_to_other_participants_only() {
    let sender = ParticipantId::new();
    let receiver = ParticipantId::new();
    let participants = vec![sender.clone(), receiver.clone()];

    let router = Router::new();
    let outbound = router.route_pose(&sender, sample_pose(), &participants);

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
