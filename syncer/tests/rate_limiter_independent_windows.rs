use std::time::{Duration, Instant};

mod common;

use syncer::{
    rate_limiter::{RateLimitDecision, RateLimiter},
    StreamKind,
};

use crate::common::fake_clock::FakeClock;

#[test]
fn windows_progress_independently_per_session() {
    // Aはt=0で上限到達、Bはt=0.5sで上限到達。
    // t=1.1s時点ではAのウィンドウはリセット済みでAllowed、BはまだRateLimitedのまま。
    let clock = FakeClock::new(Instant::now());
    let mut limiter = RateLimiter::with_clock(20, Duration::from_secs(1), clock.clone());

    let session_a = "ipc-session-a";
    let session_b = "ipc-session-b";

    // A: t=0 で20件消費し、21件目でRateLimited
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
        }
    );

    // 時刻を0.5秒進め、Bがここで上限到達
    clock.advance(Duration::from_millis(500));
    for _ in 0..20 {
        assert_eq!(
            limiter.check_and_record(session_b, StreamKind::Chat),
            RateLimitDecision::Allowed
        );
    }
    assert_eq!(
        limiter.check_and_record(session_b, StreamKind::Chat),
        RateLimitDecision::RateLimited {
            stream_kind: StreamKind::Chat
        }
    );

    // t=1.1s（開始から1.1秒）へ進める: Aのウィンドウはリセット済み、Bはまだ1秒未満なのでRateLimitedのまま。
    clock.advance(Duration::from_millis(600)); // 累計1.1秒経過

    assert_eq!(
        limiter.check_and_record(session_a, StreamKind::Pose),
        RateLimitDecision::Allowed,
        "session A should be reset after its 1s window"
    );

    assert_eq!(
        limiter.check_and_record(session_b, StreamKind::Chat),
        RateLimitDecision::RateLimited {
            stream_kind: StreamKind::Chat
        },
        "session B should still be within its window and remain rate limited"
    );
}
