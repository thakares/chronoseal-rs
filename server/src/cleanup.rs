use crate::session::AppState;
use std::sync::Arc;

pub async fn cleanup_loop(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        // Evict expired sessions from SQLite.
        {
            if let Ok(conn) = state.db_pool.get() {
                let now = crate::storage::current_time_ms();
                let _ = conn.execute(
                    "DELETE FROM sessions WHERE expires_at < ?1",
                    rusqlite::params![now],
                );
            } else {
                tracing::error!("Failed to get database connection from pool for cleanup");
            }
        }

        // Evict stale rate-limiter entries to prevent unbounded HashMap growth.
        {
            let window_secs = {
                if let Ok(config) = state.config.read() {
                    config.rate_limit_window_secs
                } else {
                    10 // fallback default
                }
            };
            let mut rl = state.rate_limiter.lock().await;
            rl.evict_stale(window_secs);
        }
    }
}
