use std::collections::HashMap;
use std::time::Instant;

/// A simple, in-memory sliding-window rate limiter for tracking client heartbeat frequency.
pub struct RateLimiter {
    /// Maps session identifiers to request counts and window start timestamps.
    buckets: HashMap<String, (u32, Instant)>,
}

impl RateLimiter {
    /// Creates a new, empty `RateLimiter`.
    pub fn new() -> Self {
        Self {
            buckets: HashMap::new(),
        }
    }

    /// Evaluates if a request conforms to the rate limit.
    ///
    /// Returns `true` if allowed, or `false` if the rate limit is exceeded.
    ///
    /// # Arguments
    /// * `key` - The unique identifier to rate-limit (e.g., session ID).
    /// * `limit` - The maximum number of allowed requests per window.
    /// * `window_secs` - The length of the sliding-window in seconds.
    pub fn check(&mut self, key: &str, limit: u32, window_secs: u64) -> bool {
        let now = Instant::now();
        let entry = self.buckets.entry(key.to_string()).or_insert((0, now));
        if now.duration_since(entry.1).as_secs() >= window_secs {
            *entry = (1, now);
            true
        } else if entry.0 >= limit {
            false
        } else {
            entry.0 += 1;
            true
        }
    }

    /// Evicts expired rate-limit entries whose time windows have fully elapsed.
    ///
    /// Intended to be called periodically to bound in-memory map growth.
    ///
    /// # Arguments
    /// * `window_secs` - The active rate-limiting window duration in seconds.
    pub fn evict_stale(&mut self, window_secs: u64) {
        let now = Instant::now();
        self.buckets
            .retain(|_, (_, ts)| now.duration_since(*ts).as_secs() < window_secs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_rate_limiter() {
        let mut rl = RateLimiter::new();
        // Limit of 2 requests per 1 second window
        assert!(rl.check("user1", 2, 1));
        assert!(rl.check("user1", 2, 1));
        assert!(!rl.check("user1", 2, 1)); // 3rd fails

        assert!(rl.check("user2", 2, 1)); // different key succeeds

        thread::sleep(Duration::from_millis(1100));
        assert!(rl.check("user1", 2, 1)); // succeeds after time window
    }

    #[test]
    fn test_rate_limiter_eviction() {
        let mut rl = RateLimiter::new();
        assert!(rl.check("user1", 1, 1));
        assert_eq!(rl.buckets.len(), 1);

        rl.evict_stale(1);
        assert_eq!(rl.buckets.len(), 1); // not stale yet

        thread::sleep(Duration::from_millis(1100));
        rl.evict_stale(1);
        assert_eq!(rl.buckets.len(), 0); // evicted
    }
}
