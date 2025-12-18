use std::time::Duration;

use syncer::{
    rate_limiter::{RateLimitDecision, RateLimiter},
    StreamKind,
};

#[test]
fn separate_sessions_do_not_share_counters() {
    // セッションAは上限に達して制限されるが、同時刻のセッションBは許容されること。
    let mut limiter = RateLimiter::new(20, Duration::from_secs(1));
    let session_a = "ipc-session-a";
    let session_b = "ipc-session-b";

    for _ in 0..20 {
        assert_eq!(
            limiter.check_and_record(session_a, StreamKind::Pose),
            RateLimitDecision::Allowed
        );
    }

    assert_eq!(
        limiter.check_and_record(session_a, StreamKind::Pose),
        RateLimitDecision::RateLimited {
            stream_kind: StreamKind::Pose
        },
        "session A should be rate limited after hitting the threshold"
    );

    // Bはまだカウントゼロのはずなので許容される
    assert_eq!(
        limiter.check_and_record(session_b, StreamKind::Chat),
        RateLimitDecision::Allowed,
        "session B should not be affected by session A being limited"
    );
}
