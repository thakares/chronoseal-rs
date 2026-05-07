use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;
use shared::protocol::{HeartbeatRequest, HeartbeatResponse};
use crate::session::AppState;

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HeartbeatRequest>,
) -> (StatusCode, Json<HeartbeatResponse>) {
    // Rate limiting
    {
        let mut rl = state.rate_limiter.lock().await;
        if !rl.check(&payload.session_id) {
            tracing::debug!("Rate limit hit: {}", payload.session_id);
            return (StatusCode::OK, Json(HeartbeatResponse { status: "ok".into(), next_salt: None }));
        }
    }

    let db = state.db.lock().await;
    match crate::session::verify_heartbeat(&db, &payload) {
        Ok(next_salt) => (
            StatusCode::OK,
            Json(HeartbeatResponse { status: "ok".into(), next_salt: Some(next_salt) }),
        ),
        Err(e) => {
            tracing::warn!("Heartbeat failed for {}: {}", payload.session_id, e);
            (StatusCode::OK, Json(HeartbeatResponse { status: "ok".into(), next_salt: None }))
        }
    }
}