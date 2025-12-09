use std::{
    collections::HashMap,
    num::NonZeroU32,
    time::{Duration, Instant},
};

use crate::StreamKind;

/// 時刻取得を抽象化するためのトレイト。テストでフェイククロックを差し替える。
pub trait Clock: Clone {
    fn now(&self) -> Instant;
}

/// 実際のシステム時計。
#[derive(Clone, Copy, Debug)]
pub struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allowed,
    RateLimited { stream_kind: StreamKind },
}

#[derive(Debug)]
struct WindowCounter {
    window_start: Instant,
    count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    pub limit_per_window: NonZeroU32,
    pub window: Duration,
}

/// デフォルトで `RealClock` を用いるが、テストでは型パラメータでクロックを差し替えられる。
#[derive(Debug)]
pub struct RateLimiter<C: Clock = RealClock> {
    limit: NonZeroU32,
    window: Duration,
    sessions: HashMap<String, WindowCounter>,
    clock: C,
}

impl RateLimiter<RealClock> {
    pub fn new(limit: u32, window: Duration) -> Self {
        let limit = NonZeroU32::new(limit).expect("limit must be greater than zero");
        Self {
            limit,
            window,
            sessions: HashMap::new(),
            clock: RealClock,
        }
    }

    pub fn from_config(config: RateLimitConfig) -> Self {
        Self {
            limit: config.limit_per_window,
            window: config.window,
            sessions: HashMap::new(),
            clock: RealClock,
        }
    }

    pub fn config(&self) -> RateLimitConfig {
        RateLimitConfig {
            limit_per_window: self.limit,
            window: self.window,
        }
    }
}

impl<C: Clock> RateLimiter<C> {
    pub fn with_clock(limit: u32, window: Duration, clock: C) -> Self {
        let limit = NonZeroU32::new(limit).expect("limit must be greater than zero");
        Self {
            limit,
            window,
            sessions: HashMap::new(),
            clock,
        }
    }

    pub fn check_and_record(
        &mut self,
        session_id: impl AsRef<str>,
        stream_kind: StreamKind,
    ) -> RateLimitDecision {
        self.check_and_record_inner(session_id.as_ref(), stream_kind, self.clock.now())
    }

    fn check_and_record_inner(
        &mut self,
        session_id: &str,
        stream_kind: StreamKind,
        now: Instant,
    ) -> RateLimitDecision {
        let counter = self
            .sessions
            .entry(session_id.to_owned())
            .or_insert_with(|| WindowCounter {
                window_start: now,
                count: 0,
            });

        if Self::should_reset(counter, now, self.window) {
            counter.window_start = now;
            counter.count = 0;
        }

        if counter.count < self.limit.get() {
            counter.count += 1;
            RateLimitDecision::Allowed
        } else {
            RateLimitDecision::RateLimited { stream_kind }
        }
    }

    fn should_reset(counter: &WindowCounter, now: Instant, window: Duration) -> bool {
        now.duration_since(counter.window_start) >= window
    }

    /// オプション: 一定時間経過したセッションのカウンタをクリーンアップする。
    /// windowを超えてカウントが0のものを削除することでメモリ膨張を防ぐ。
    pub fn purge_inactive(&mut self, now: Instant) {
        self.sessions.retain(|_, counter| {
            !Self::should_reset(counter, now, self.window) || counter.count > 0
        });
    }
}
