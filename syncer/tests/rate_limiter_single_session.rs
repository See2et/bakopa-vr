use std::time::Duration;

use syncer::{
    rate_limiter::{RateLimitDecision, RateLimiter},
    StreamKind,
};

#[test]
fn returns_rate_limited_on_first_overflow_for_single_session() {
    // 20件までは許容し、21件目で初回のRateLimitedを返すことを期待する。
    let mut limiter = RateLimiter::new(20, Duration::from_secs(1));
    let session_id = "ipc-session-1";

    for i in 0..20 {
        let decision = limiter.check_and_record(session_id, StreamKind::Pose);
        assert_eq!(
            decision,
            RateLimitDecision::Allowed,
            "message {} should be allowed before hitting the limit",
            i + 1
        );
    }

    let decision = limiter.check_and_record(session_id, StreamKind::Pose);
    assert_eq!(
        decision,
        RateLimitDecision::RateLimited {
            stream_kind: StreamKind::Pose
        },
        "21st message should trigger RateLimited for the session"
    );
}
