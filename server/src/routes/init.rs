use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;
use shared::protocol::{InitRequest, InitResponse};
use crate::session::AppState;

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InitRequest>,
) -> Result<Json<InitResponse>, (StatusCode, String)> {
    let db = state.db.lock().await;
    crate::session::create_session(&db, &payload.public_key)
        .map(Json)
        .map_err(|e| {
            tracing::error!("Init error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal".into())
        })
}