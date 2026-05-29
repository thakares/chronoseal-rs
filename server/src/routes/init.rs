use crate::errors::SessionError;
use crate::session::AppState;
use axum::{extract::State, Json};
use shared::protocol::{InitRequest, InitResponse};
use std::sync::Arc;

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InitRequest>,
) -> Result<Json<InitResponse>, SessionError> {
    let config = state.get_config();
    let resp = crate::session::create_session(&state.db_pool, &config, &payload.public_key)?;
    Ok(Json(resp))
}
