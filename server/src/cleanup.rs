use std::sync::Arc;
use crate::session::AppState;

pub async fn cleanup_loop(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        // Evict expired sessions from SQLite.
        {
            let db = state.db.lock().await;
            let now = crate::storage::current_time_ms();
            let _ = db.execute(
                "DELETE FROM sessions WHERE expires_at < ?1",
                rusqlite::params![now],
            );
        }

        // Evict stale rate-limiter entries to prevent unbounded HashMap growth.
        {
            let mut rl = state.rate_limiter.lock().await;
            rl.evict_stale();
        }
    }
}