use std::time::{Duration, Instant};

mod common;

use syncer::{
    rate_limiter::{RateLimitDecision, RateLimiter},
    StreamKind,
};

use crate::common::fake_clock::FakeClock;

#[test]
fn stays_rate_limited_until_window_elapses() {
    // 21件目以降はウィンドウが切れるまで連続してRateLimitedを返し、ウィンドウ経過後に再びAllowedに戻ること。
    let clock = FakeClock::new(Instant::now());
    let mut limiter = RateLimiter::with_clock(20, Duration::from_secs(1), clock.clone());
    let session_id = "ipc-session-1";

    for _ in 0..20 {
        assert_eq!(
            limiter.check_and_record(session_id, StreamKind::Pose),
            RateLimitDecision::Allowed
        );
    }

    // 21〜25件目はすべてRateLimitedのまま。
    for attempt in 21..=25 {
        assert_eq!(
            limiter.check_and_record(session_id, StreamKind::Pose),
            RateLimitDecision::RateLimited {
                stream_kind: StreamKind::Pose
            },
            "attempt {} should remain rate limited until the window resets",
            attempt
        );
    }

    // ウィンドウ経過後に許容へ戻る。
    clock.advance(Duration::from_millis(1100));
    assert_eq!(
        limiter.check_and_record(session_id, StreamKind::Pose),
        RateLimitDecision::Allowed,
        "after window elapses, counter should reset and allow again"
    );
}
