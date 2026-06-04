use crate::errors::SessionError;
use crate::session::AppState;
use axum::{extract::State, Json};
use shared::protocol::{InitRequest, InitResponse};
use std::sync::Arc;

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InitRequest>,
) -> Result<Json<InitResponse>, SessionError> {
    let start_http = std::time::Instant::now();
    let config = state.get_config();

    // Rate limit session creation by public key to prevent storage exhaustion.
    if !state.rate_limiter.check(
        &payload.public_key,
        config.rate_limit_count,
        config.rate_limit_window_secs,
    ) {
        return Err(SessionError::RateLimited);
    }

    let start_db = std::time::Instant::now();
    let resp = crate::session::create_session(&state.db_pool, &config, &payload.public_key);
    let db_dur = start_db.elapsed().as_nanos() as u64;
    state
        .storage_latency_ns
        .fetch_add(db_dur, std::sync::atomic::Ordering::Relaxed);
    state
        .storage_ops_count
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let resp = resp?;

    let http_dur = start_http.elapsed().as_nanos() as u64;
    state
        .http_latency_ns
        .fetch_add(http_dur, std::sync::atomic::Ordering::Relaxed);
    state
        .http_ops_count
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    Ok(Json(resp))
}
