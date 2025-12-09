use std::time::{Duration, Instant};

use syncer::{
    rate_limiter::{Clock, RateLimitDecision, RateLimiter},
    StreamKind,
};

#[derive(Clone)]
struct FakeClock {
    now: Instant,
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.now
    }
}

impl FakeClock {
    fn advance(&mut self, delta: Duration) {
        self.now += delta;
    }
}

#[test]
fn stays_rate_limited_until_window_elapses() {
    // 21件目以降はウィンドウが切れるまで連続してRateLimitedを返し、ウィンドウ経過後に再びAllowedに戻ること。
    let start = Instant::now();
    let mut clock = FakeClock { now: start };
    let shared_clock = clock.clone();
    let mut limiter = RateLimiter::with_clock(20, Duration::from_secs(1), shared_clock);
    let session_id = "ipc-session-1";

    for _ in 0..20 {
        assert_eq!(
            limiter.check_and_record_with_clock(session_id, StreamKind::Pose, clock.clone()),
            RateLimitDecision::Allowed
        );
    }

    // 21〜25件目はすべてRateLimitedのまま。
    for attempt in 21..=25 {
        assert_eq!(
            limiter.check_and_record_with_clock(session_id, StreamKind::Pose, clock.clone()),
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
        limiter.check_and_record_with_clock(session_id, StreamKind::Pose, clock.clone()),
        RateLimitDecision::Allowed,
        "after window elapses, counter should reset and allow again"
    );
}
