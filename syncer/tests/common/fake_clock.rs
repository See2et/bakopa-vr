use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use syncer::rate_limiter::Clock;

/// テスト用に共有可能なフェイククロック。
#[derive(Clone)]
#[allow(dead_code)]
pub struct FakeClock {
    now: Arc<Mutex<Instant>>,
}

impl FakeClock {
    #[allow(dead_code)]
    pub fn new(start: Instant) -> Self {
        Self {
            now: Arc::new(Mutex::new(start)),
        }
    }

    #[allow(dead_code)]
    pub fn advance(&self, delta: Duration) {
        let mut guard = self.now.lock().expect("clock poisoned");
        *guard += delta;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        *self.now.lock().expect("clock poisoned")
    }
}
