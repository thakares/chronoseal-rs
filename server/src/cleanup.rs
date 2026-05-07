use std::sync::Arc;
use crate::session::AppState;

pub async fn cleanup_loop(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        let db = state.db.lock().await; // this is infallible
        let now = crate::storage::current_time_ms();
        let _ = db.execute(
            "DELETE FROM sessions WHERE expires_at < ?1",
            rusqlite::params![now],
        );
    }
}