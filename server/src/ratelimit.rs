use std::collections::HashMap;
use std::time::Instant;

pub struct RateLimiter {
    buckets: HashMap<String, (u32, Instant)>,
    limit: u32,
    window_secs: u64,
}

impl RateLimiter {
    pub fn new(limit: u32, window_secs: u64) -> Self {
        Self { buckets: HashMap::new(), limit, window_secs }
    }

    pub fn check(&mut self, key: &str) -> bool {
        let now = Instant::now();
        let entry = self.buckets.entry(key.to_string()).or_insert((0, now));
        if now.duration_since(entry.1).as_secs() >= self.window_secs {
            *entry = (1, now);
            true
        } else if entry.0 >= self.limit {
            false
        } else {
            entry.0 += 1;
            true
        }
    }

    /// Remove entries whose rate-limit window has fully elapsed.
    /// Call this periodically (e.g. from the cleanup loop) to bound memory usage.
    pub fn evict_stale(&mut self) {
        let window = self.window_secs;
        let now = Instant::now();
        self.buckets
            .retain(|_, (_, ts)| now.duration_since(*ts).as_secs() < window);
    }
}