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
fn windows_progress_independently_per_session() {
    // Aはt=0で上限到達、Bはt=0.5sで上限到達。
    // t=1.1s時点ではAのウィンドウはリセット済みでAllowed、BはまだRateLimitedのまま。
    let start = Instant::now();
    let mut clock = FakeClock { now: start };
    let shared_clock = clock.clone();
    let mut limiter = RateLimiter::with_clock(20, Duration::from_secs(1), shared_clock);

    let session_a = "ipc-session-a";
    let session_b = "ipc-session-b";

    // A: t=0 で20件消費し、21件目でRateLimited
    for _ in 0..20 {
        assert_eq!(
            limiter.check_and_record_with_clock(session_a, StreamKind::Pose, clock.clone()),
            RateLimitDecision::Allowed
        );
    }
    assert_eq!(
        limiter.check_and_record_with_clock(session_a, StreamKind::Pose, clock.clone()),
        RateLimitDecision::RateLimited {
            stream_kind: StreamKind::Pose
        }
    );

    // 時刻を0.5秒進め、Bがここで上限到達
    clock.advance(Duration::from_millis(500));
    for _ in 0..20 {
        assert_eq!(
            limiter.check_and_record_with_clock(session_b, StreamKind::Chat, clock.clone()),
            RateLimitDecision::Allowed
        );
    }
    assert_eq!(
        limiter.check_and_record_with_clock(session_b, StreamKind::Chat, clock.clone()),
        RateLimitDecision::RateLimited {
            stream_kind: StreamKind::Chat
        }
    );

    // t=1.1s（開始から1.1秒）へ進める: Aのウィンドウはリセット済み、Bはまだ1秒未満なのでRateLimitedのまま。
    clock.advance(Duration::from_millis(600)); // 累計1.1秒経過

    assert_eq!(
        limiter.check_and_record_with_clock(session_a, StreamKind::Pose, clock.clone()),
        RateLimitDecision::Allowed,
        "session A should be reset after its 1s window"
    );

    assert_eq!(
        limiter.check_and_record_with_clock(session_b, StreamKind::Chat, clock.clone()),
        RateLimitDecision::RateLimited {
            stream_kind: StreamKind::Chat
        },
        "session B should still be within its window and remain rate limited"
    );
}
