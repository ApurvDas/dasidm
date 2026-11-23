//! Token-bucket rate limiter. Live-adjustable; a rate of 0 means unlimited.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub struct RateLimiter {
    rate: AtomicU64,
    state: Mutex<Bucket>,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(bytes_per_sec: u64) -> Self {
        Self {
            rate: AtomicU64::new(bytes_per_sec),
            state: Mutex::new(Bucket { tokens: 0.0, last: Instant::now() }),
        }
    }

    pub fn unlimited() -> Self {
        Self::new(0)
    }

    pub fn rate(&self) -> u64 {
        self.rate.load(Ordering::Relaxed)
    }

    pub fn set_rate(&self, bytes_per_sec: u64) {
        self.rate.store(bytes_per_sec, Ordering::Relaxed);
        let mut s = self.state.lock();
        s.tokens = 0.0;
        s.last = Instant::now();
    }

    /// Take `n` bytes from the bucket, sleeping if the bucket is in debt.
    pub async fn acquire(&self, n: usize) {
        let rate = self.rate();
        if rate == 0 {
            return;
        }
        let wait = {
            let mut s = self.state.lock();
            let now = Instant::now();
            let elapsed = now.duration_since(s.last).as_secs_f64();
            s.last = now;
            // Allow a burst of a quarter second, at least 64 KiB.
            let burst = (rate as f64 / 4.0).max(64.0 * 1024.0);
            s.tokens = (s.tokens + elapsed * rate as f64).min(burst);
            s.tokens -= n as f64;
            if s.tokens < 0.0 {
                Duration::from_secs_f64(-s.tokens / rate as f64)
            } else {
                Duration::ZERO
            }
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
}
