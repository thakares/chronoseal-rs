mod cleanup;
mod crypto;
mod fingerprint;
mod middleware;
mod ratelimit;
mod routes;
mod session;
mod storage;
mod trust;
mod vm;

use axum::Router;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use session::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let conn = storage::init_db().expect("DB init");
    let state = Arc::new(AppState {
        db: Mutex::new(conn),
        rate_limiter: Mutex::new(ratelimit::RateLimiter::new(
            shared::constants::RATE_LIMIT_COUNT,
            shared::constants::RATE_LIMIT_WINDOW_SECS,
        )),
    });

    // Periodic cleanup
    let bg_state = state.clone();
    tokio::spawn(async move { cleanup::cleanup_loop(bg_state).await });

    let app = Router::new()
        .route("/init", axum::routing::post(routes::init::handler))
        .route("/hb", axum::routing::post(routes::heartbeat::handler))
        .nest_service("/", tower_http::services::ServeDir::new("../frontend"))
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(axum::middleware::from_fn(middleware::log_request))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Server running on :3000");
    axum::serve(listener, app).await.unwrap();
}