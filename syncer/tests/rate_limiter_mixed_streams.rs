use std::time::Duration;

use syncer::{
    rate_limiter::{RateLimitDecision, RateLimiter},
    StreamKind,
};

#[test]
fn counts_mixed_streams_in_same_session() {
    // PoseとChatを合算して20件までは許容し、21件目のControlでRateLimitedになる。
    let mut limiter = RateLimiter::new(20, Duration::from_secs(1));
    let session_id = "ipc-session-mixed";

    for _ in 0..19 {
        assert_eq!(
            limiter.check_and_record(session_id, StreamKind::Pose),
            RateLimitDecision::Allowed
        );
    }

    assert_eq!(
        limiter.check_and_record(session_id, StreamKind::Chat),
        RateLimitDecision::Allowed,
        "20th message (Chat) should still be allowed"
    );

    assert_eq!(
        limiter.check_and_record(session_id, StreamKind::ControlJoin),
        RateLimitDecision::RateLimited {
            stream_kind: StreamKind::ControlJoin
        },
        "21st message (Control) should trigger RateLimited after mixed streams reach the limit"
    );
}
