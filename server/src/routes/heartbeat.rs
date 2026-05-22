use crate::session::AppState;
use axum::{extract::State, http::StatusCode, Json};
use shared::protocol::{HeartbeatRequest, HeartbeatResponse};
use std::sync::Arc;

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HeartbeatRequest>,
) -> (StatusCode, Json<HeartbeatResponse>) {
    // Rate limiting
    {
        let (limit, window_secs) = {
            if let Ok(cfg) = state.config.read() {
                (cfg.rate_limit_count, cfg.rate_limit_window_secs)
            } else {
                (5, 10)
            }
        };
        let mut rl = state.rate_limiter.lock().await;
        if !rl.check(&payload.session_id, limit, window_secs) {
            tracing::debug!("Rate limit hit: {}", payload.session_id);
            return (
                StatusCode::OK,
                Json(HeartbeatResponse {
                    status: "ok".into(),
                    next_salt: None,
                }),
            );
        }
    }

    let config = {
        if let Ok(cfg) = state.config.read() {
            cfg.clone()
        } else {
            crate::config::Config::default()
        }
    };
    let conn = match state.db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Db pool error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(HeartbeatResponse {
                    status: "error".into(),
                    next_salt: None,
                }),
            );
        }
    };
    match crate::session::verify_heartbeat(&conn, &config, &payload) {
        Ok(next_salt) => (
            StatusCode::OK,
            Json(HeartbeatResponse {
                status: "ok".into(),
                next_salt: Some(next_salt),
            }),
        ),
        Err(e) => {
            tracing::warn!("Heartbeat failed for {}: {}", payload.session_id, e);
            (
                StatusCode::OK,
                Json(HeartbeatResponse {
                    status: "ok".into(),
                    next_salt: None,
                }),
            )
        }
    }
}
