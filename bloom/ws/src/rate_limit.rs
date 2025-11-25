use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Clock abstraction to allow deterministic tests.
pub trait Clock: Clone {
    fn now(&self) -> Instant;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitDecision {
    pub allowed: bool,
    /// true のとき、このメッセージはハンドラでドロップすべき。
    pub should_drop: bool,
}

/// Simple per-connection rate limiter (window + limit) for signaling messages.
pub struct RateLimiter<C: Clock> {
    clock: C,
    limit_per_window: u32,
    window: Duration,
    timestamps: VecDeque<Instant>,
    cooldown_until: Option<Instant>,
}

impl<C: Clock> RateLimiter<C> {
    pub fn new(clock: C, limit_per_window: u32, window: Duration) -> Self {
        Self {
            clock,
            limit_per_window,
            window,
            timestamps: VecDeque::new(),
            cooldown_until: None,
        }
    }

    pub fn check(&mut self) -> RateLimitDecision {
        let now = self.clock.now();

        // Cooldown中ならドロップのみ返す
        if let Some(until) = self.cooldown_until {
            if now < until {
                return RateLimitDecision {
                    allowed: false,
                    should_drop: true,
                };
            } else {
                // クールダウンを抜けたら状態リセット
                self.cooldown_until = None;
                self.timestamps.clear();
            }
        }

        // 古いタイムスタンプを掃除
        while let Some(&front) = self.timestamps.front() {
            if now.duration_since(front) > self.window {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }

        if (self.timestamps.len() as u32) >= self.limit_per_window {
            // 上限超過: 以降 window 長のクールダウンに入る
            self.cooldown_until = Some(now + self.window);
            return RateLimitDecision {
                allowed: false,
                should_drop: true,
            };
        }

        // 許可して記録
        self.timestamps.push_back(now);
        RateLimitDecision {
            allowed: true,
            should_drop: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockClock {
        now: Arc<Mutex<Instant>>,
    }

    impl MockClock {
        fn start_at(now: Instant) -> Self {
            Self {
                now: Arc::new(Mutex::new(now)),
            }
        }

        fn advance(&self, duration: Duration) {
            if let Ok(mut guard) = self.now.lock() {
                *guard = *guard + duration;
            }
        }
    }

    impl Clock for MockClock {
        fn now(&self) -> Instant {
            self
                .now
                .lock()
                .map(|t| *t)
                .unwrap_or_else(|_| Instant::now())
        }
    }

    fn new_limiter(limit: u32) -> (RateLimiter<MockClock>, MockClock) {
        let clock = MockClock::start_at(Instant::now());
        let limiter = RateLimiter::new(clock.clone(), limit, Duration::from_secs(1));
        (limiter, clock)
    }

    #[test]
    fn allows_twenty_then_limits_twenty_first() {
        let (mut limiter, _clock) = new_limiter(20);

        for _ in 0..20 {
            let decision = limiter.check();
            assert!(decision.allowed, "first 20 should be allowed");
            assert!(!decision.should_drop);
        }

        let decision = limiter.check();
        assert!(!decision.allowed, "21st should be rate limited");
        assert!(
            decision.should_drop,
            "rate-limited messages should be dropped"
        );
    }

    #[test]
    fn resets_after_one_second_window() {
        let (mut limiter, clock) = new_limiter(20);

        for _ in 0..21 {
            let _ = limiter.check();
        }

        clock.advance(Duration::from_secs(1));

        for _ in 0..20 {
            let decision = limiter.check();
            assert!(decision.allowed, "after 1s, counter should reset");
        }
    }

    #[test]
    fn counts_are_isolated_per_instance() {
        let clock = MockClock::start_at(Instant::now());
        let mut limiter_a = RateLimiter::new(clock.clone(), 20, Duration::from_secs(1));
        let mut limiter_b = RateLimiter::new(clock.clone(), 20, Duration::from_secs(1));

        for _ in 0..20 {
            let _ = limiter_a.check();
        }

        let decision_a = limiter_a.check();
        assert!(!decision_a.allowed, "limiter A hits its own limit");

        let decision_b = limiter_b.check();
        assert!(
            decision_b.allowed,
            "limiter B should not be affected by limiter A"
        );
    }

    #[test]
    fn rejected_decision_sets_drop_flag() {
        let (mut limiter, _clock) = new_limiter(1);

        let _ = limiter.check();
        let decision = limiter.check();

        assert!(!decision.allowed);
        assert!(decision.should_drop);
    }
}
