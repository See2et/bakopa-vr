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
fn resets_after_window_elapsed_on_same_session() {
    // 20件まで許容 → 21件目でRateLimited → 1秒経過後に再び許容されること。
    let start = Instant::now();
    let mut clock = FakeClock { now: start };
    let shared_clock = clock.clone();
    let mut limiter = RateLimiter::with_clock(20, Duration::from_secs(1), shared_clock);
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
        limiter.check_and_record_with_clock(session_id, StreamKind::Pose, clock.clone()),
        RateLimitDecision::Allowed,
        "after window elapses, counter should reset and allow again"
    );
}
