use crate::session::AppState;
use std::sync::Arc;

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
