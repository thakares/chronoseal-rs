use crate::session::AppState;
use std::sync::Arc;

/// Runs an infinite background loop that periodically cleans up database and memory resources.
///
/// Every 60 seconds, this loop performs two tasks:
/// 1. Evicts expired session records from the configured database storage backend.
/// 2. Evicts stale rate-limiter entries that have outlived the current rate-limiting window.
///
/// # Arguments
/// * `state` - Shared reference to the server application state.
pub async fn cleanup_loop(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        // Evict expired sessions from the configured storage backend.
        {
            if let Err(err) = state.db_pool.delete_expired_sessions() {
                tracing::error!("Failed to evict expired sessions: {}", err);
            }
        }

        // Evict stale rate-limiter entries to prevent unbounded HashMap growth.
        {
            let window_secs = state.get_config().rate_limit_window_secs;
            let mut rl = state.rate_limiter.lock().await;
            rl.evict_stale(window_secs);
        }
    }
}

