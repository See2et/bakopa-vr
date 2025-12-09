use std::time::{Duration, Instant};

mod common;

use syncer::{
    rate_limiter::{RateLimitDecision, RateLimiter},
    StreamKind,
};

use crate::common::fake_clock::FakeClock;

#[test]
fn resets_after_window_elapsed_on_same_session() {
    // 20件まで許容 → 21件目でRateLimited → 1秒経過後に再び許容されること。
    let clock = FakeClock::new(Instant::now());
    let mut limiter = RateLimiter::with_clock(20, Duration::from_secs(1), clock.clone());
    let session_id = "ipc-session-1";

    for _ in 0..20 {
        assert_eq!(
            limiter.check_and_record(session_id, StreamKind::Pose),
            RateLimitDecision::Allowed
        );
    }

    assert_eq!(
        limiter.check_and_record(session_id, StreamKind::Pose),
        RateLimitDecision::RateLimited {
            stream_kind: StreamKind::Pose
        }
    );

    clock.advance(Duration::from_millis(1100));

    assert_eq!(
        limiter.check_and_record(session_id, StreamKind::Pose),
        RateLimitDecision::Allowed,
        "after window elapses, counter should reset and allow again"
    );
}
