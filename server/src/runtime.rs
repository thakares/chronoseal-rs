use crate::{
    config::Config,
    output::TextOutput,
    ratelimit::RateLimiter,
    routes, session,
    storage::{self, StoreStats},
};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tokio::sync::{Mutex, Notify};
use tracing::{error, info, warn};

#[derive(Debug, Serialize)]
pub struct HealthReport {
    pub status: &'static str,
    pub bind: String,
}

impl TextOutput for HealthReport {
    fn to_text(&self) -> String {
        format!("{}\nbind={}", self.status, self.bind)
    }
}

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub running: bool,
    pub healthy: bool,
    pub bind: String,
    pub pid_file: String,
    pub pid: Option<u32>,
}

impl TextOutput for StatusReport {
    fn to_text(&self) -> String {
        let pid = self.pid.map_or_else(|| "unknown".to_string(), |pid| pid.to_string());
        format!(
            "running={}\nhealthy={}\nbind={}\npid_file={}\npid={}",
            self.running, self.healthy, self.bind, self.pid_file, pid
        )
    }
}

#[derive(Debug, Serialize)]
pub struct VersionReport {
    pub name: &'static str,
    pub version: &'static str,
    pub target: &'static str,
}

impl TextOutput for VersionReport {
    fn to_text(&self) -> String {
        format!("{} {}", self.name, self.version)
    }
}

#[derive(Debug, Serialize)]
pub struct KeypairReport {
    pub algorithm: &'static str,
    pub public_key_hex: String,
    pub private_key_hex: String,
}

impl TextOutput for KeypairReport {
    fn to_text(&self) -> String {
        format!(
            "algorithm={}\npublic_key_hex={}\nprivate_key_hex={}",
            self.algorithm, self.public_key_hex, self.private_key_hex
        )
    }
}

impl TextOutput for Config {
    fn to_text(&self) -> String {
        format!(
            "bind={}\npid_file={}\ndb_path={}\nfrontend_dir={}\nlog_file={}",
            self.bind,
            self.pid_file.display(),
            self.db_path.display(),
            self.frontend_dir.display(),
            self.log_file
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string())
        )
    }
}

impl TextOutput for StoreStats {
    fn to_text(&self) -> String {
        format!(
            "sessions={}\nexpired_sessions={}\nmax_chain_length={}",
            self.sessions, self.expired_sessions, self.max_chain_length
        )
    }
}

pub async fn run_daemon(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    install_pid_file(&config.pid_file)?;

    let conn = storage::init_db(&config.db_path)?;
    let state = Arc::new(session::AppState {
        db: Mutex::new(conn),
        rate_limiter: Mutex::new(RateLimiter::new(
            shared::constants::RATE_LIMIT_COUNT,
            shared::constants::RATE_LIMIT_WINDOW_SECS,
        )),
    });

    let bg_state = state.clone();
    tokio::spawn(async move { crate::cleanup::cleanup_loop(bg_state).await });

    let app = Router::new()
        .route("/init", axum::routing::post(routes::init::handler))
        .route("/hb", axum::routing::post(routes::heartbeat::handler))
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/stats", get(stats_handler))
        .nest_service("/", tower_http::services::ServeDir::new(&config.frontend_dir))
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(axum::middleware::from_fn(crate::middleware::log_request))
        .with_state(state);

    let addr: SocketAddr = config.bind.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(bind = %config.bind, "chronoseal daemon started");

    let shutdown = signal_task(config.clone());
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await;

    remove_pid_file(&config.pid_file);
    result?;
    info!("chronoseal daemon stopped");
    Ok(())
}

pub fn probe_health(config: &Config) -> HealthReport {
    if http_get(&config.bind, "/health").is_ok() {
        HealthReport {
            status: "healthy",
            bind: config.bind.clone(),
        }
    } else {
        HealthReport {
            status: "unreachable",
            bind: config.bind.clone(),
        }
    }
}

pub fn probe_status(config: &Config) -> StatusReport {
    let pid = read_pid(&config.pid_file);
    let healthy = http_get(&config.bind, "/health").is_ok();
    StatusReport {
        running: pid.is_some() || healthy,
        healthy,
        bind: config.bind.clone(),
        pid_file: config.pid_file.display().to_string(),
        pid,
    }
}

pub fn fetch_metrics(config: &Config) -> Result<String, Box<dyn std::error::Error>> {
    http_get(&config.bind, "/metrics")
}

pub fn fetch_stats(config: &Config) -> Result<StoreStats, Box<dyn std::error::Error>> {
    let body = http_get(&config.bind, "/stats")?;
    Ok(serde_json::from_str(&body)?)
}

pub fn generate_keypair() -> KeypairReport {
    let private_key = rand::random::<[u8; 32]>();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&private_key);
    let verifying_key = signing_key.verifying_key();
    KeypairReport {
        algorithm: "ed25519",
        public_key_hex: hex::encode(verifying_key.to_bytes()),
        private_key_hex: hex::encode(private_key),
    }
}

pub fn version() -> VersionReport {
    VersionReport {
        name: "chronoseal",
        version: env!("CARGO_PKG_VERSION"),
        target: std::env::consts::ARCH,
    }
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "healthy" })))
}

async fn stats_handler(
    axum::extract::State(state): axum::extract::State<Arc<session::AppState>>,
) -> Result<Json<StoreStats>, (StatusCode, String)> {
    let db = state.db.lock().await;
    storage::stats(&db)
        .map(Json)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

async fn metrics_handler(
    axum::extract::State(state): axum::extract::State<Arc<session::AppState>>,
) -> Result<String, (StatusCode, String)> {
    let db = state.db.lock().await;
    storage::stats(&db)
        .map(|stats| {
            format!(
                "# HELP chronoseal_sessions Active ChronoSeal sessions\n# TYPE chronoseal_sessions gauge\nchronoseal_sessions {}\n# HELP chronoseal_expired_sessions Expired sessions not yet removed\n# TYPE chronoseal_expired_sessions gauge\nchronoseal_expired_sessions {}\n# HELP chronoseal_max_chain_length Maximum heartbeat chain length\n# TYPE chronoseal_max_chain_length gauge\nchronoseal_max_chain_length {}\n",
                stats.sessions, stats.expired_sessions, stats.max_chain_length
            )
        })
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

async fn signal_task(config: Config) {
    let shutdown = Arc::new(Notify::new());

    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let shutdown_term = shutdown.clone();
        tokio::spawn(async move {
            let mut sigterm = signal(SignalKind::terminate()).expect("install SIGTERM handler");
            sigterm.recv().await;
            info!("received SIGTERM; shutting down gracefully");
            shutdown_term.notify_one();
        });

        let shutdown_int = shutdown.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                info!("received interrupt; shutting down gracefully");
                shutdown_int.notify_one();
            }
        });

        let hup_config = config.clone();
        tokio::spawn(async move {
            let mut sighup = signal(SignalKind::hangup()).expect("install SIGHUP handler");
            while sighup.recv().await.is_some() {
                match Config::load(None) {
                    Ok(reloaded) => info!(
                        bind = %reloaded.bind,
                        db_path = %reloaded.db_path.display(),
                        "received SIGHUP; configuration reloaded"
                    ),
                    Err(err) => warn!(error = %err, "received SIGHUP; configuration reload failed"),
                }
                let _ = &hup_config;
            }
        });

        tokio::spawn(async move {
            let mut sigusr1 = signal(SignalKind::user_defined1()).expect("install SIGUSR1 handler");
            while sigusr1.recv().await.is_some() {
                info!("received SIGUSR1; stats are available via chronoseal stats or /stats");
            }
        });
    }

    #[cfg(not(unix))]
    {
        if tokio::signal::ctrl_c().await.is_ok() {
            shutdown.notify_one();
        }
    }

    shutdown.notified().await;
}

fn install_pid_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            warn!(path = %parent.display(), error = %err, "could not create PID directory");
        }
    }
    match fs::write(path, std::process::id().to_string()) {
        Ok(()) => Ok(()),
        Err(err) => {
            warn!(path = %path.display(), error = %err, "could not write PID file");
            Ok(())
        }
    }
}

fn remove_pid_file(path: &Path) {
    if let Err(err) = fs::remove_file(path) {
        if err.kind() != std::io::ErrorKind::NotFound {
            error!(path = %path.display(), error = %err, "could not remove PID file");
        }
    }
}

fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn http_get(bind: &str, path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect_timeout(&bind.parse()?, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(format!("GET {path} HTTP/1.1\r\nHost: chronoseal\r\nConnection: close\r\n\r\n").as_bytes())?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let (_, body) = response
        .split_once("\r\n\r\n")
        .ok_or("daemon returned an invalid HTTP response")?;
    Ok(body.to_string())
}
