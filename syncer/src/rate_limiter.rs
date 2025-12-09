use std::{
    collections::HashMap,
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

/// デフォルトで `RealClock` を用いるが、テストでは型パラメータでクロックを差し替えられる。
#[derive(Debug)]
pub struct RateLimiter<C: Clock = RealClock> {
    limit: u32,
    window: Duration,
    sessions: HashMap<String, WindowCounter>,
    clock: C,
}

impl RateLimiter<RealClock> {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            sessions: HashMap::new(),
            clock: RealClock,
        }
    }
}

impl<C: Clock> RateLimiter<C> {
    pub fn with_clock(limit: u32, window: Duration, clock: C) -> Self {
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

    /// テスト用: 呼び出し側でクロックを明示できるAPI。
    pub fn check_and_record_with_clock(
        &mut self,
        session_id: impl AsRef<str>,
        stream_kind: StreamKind,
        clock: C,
    ) -> RateLimitDecision {
        self.check_and_record_inner(session_id.as_ref(), stream_kind, clock.now())
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

        // ウィンドウ経過でリセット
        if now.duration_since(counter.window_start) >= self.window {
            counter.window_start = now;
            counter.count = 0;
        }

        if counter.count < self.limit {
            counter.count += 1;
            RateLimitDecision::Allowed
        } else {
            RateLimitDecision::RateLimited { stream_kind }
        }
    }
}
