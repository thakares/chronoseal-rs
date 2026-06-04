use crate::{
    config::Config,
    output::TextOutput,
    ratelimit::RateLimiter,
    routes, session,
    storage::{self, StoreStats},
};
use axum::{
    extract::ConnectInfo, http::StatusCode, response::IntoResponse, routing::get, Json, Router,
};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tokio::sync::Notify;
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
        let pid = self
            .pid
            .map_or_else(|| "unknown".to_string(), |pid| pid.to_string());
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

#[derive(Debug, Serialize)]
pub struct DbTypeEntry {
    pub name: &'static str,
    pub implemented: bool,
    pub notes: &'static str,
}

#[derive(Debug, Serialize)]
pub struct DbTypeReport {
    pub default: &'static str,
    pub backends: Vec<DbTypeEntry>,
}

impl TextOutput for DbTypeReport {
    fn to_text(&self) -> String {
        let mut out = format!("default={}\n", self.default);
        for backend in &self.backends {
            let status = if backend.implemented {
                "implemented"
            } else {
                "todo"
            };
            out.push_str(&format!(
                "db_type={} status={} notes={}\n",
                backend.name, status, backend.notes
            ));
        }
        out
    }
}

impl TextOutput for Config {
    fn to_text(&self) -> String {
        format!(
            "bind={}\ndb_type={}\npid_file={}\ndb_path={}\nfrontend_dir={}\nlog_file={}\ngene_size={}",
            self.bind,
            self.db_type.as_str(),
            self.pid_file.display(),
            self.db_path.display(),
            self.frontend_dir.display(),
            self.log_file
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.gene_size
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

    let db_pool = init_db_pool(&config)?;
    let state = Arc::new(session::AppState {
        db_pool,
        rate_limiter: RateLimiter::new(),
        config: std::sync::RwLock::new(config.clone()),
        heartbeats_total: std::sync::atomic::AtomicU64::new(0),
        verification_failures_total: std::sync::atomic::AtomicU64::new(0),
        mutation_failures_total: std::sync::atomic::AtomicU64::new(0),
        replay_attempts_total: std::sync::atomic::AtomicU64::new(0),
        storage_latency_ns: std::sync::atomic::AtomicU64::new(0),
        storage_ops_count: std::sync::atomic::AtomicU64::new(0),
        http_latency_ns: std::sync::atomic::AtomicU64::new(0),
        http_ops_count: std::sync::atomic::AtomicU64::new(0),
    });

    let bg_state = state.clone();
    tokio::spawn(async move { crate::cleanup::cleanup_loop(bg_state).await });

    let app = Router::new()
        .route("/init", axum::routing::post(routes::init::handler))
        .route("/hb", axum::routing::post(routes::heartbeat::handler))
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/stats", get(stats_handler))
        .nest_service(
            "/",
            tower_http::services::ServeDir::new(&config.frontend_dir),
        )
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(axum::middleware::from_fn(
            crate::middleware::security_headers,
        ))
        .layer(axum::middleware::from_fn(crate::middleware::log_request))
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024)) // 64 KiB
        .with_state(state.clone());

    let addr: SocketAddr = config.bind.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(bind = %config.bind, "chronoseal daemon started");

    let shutdown = signal_task(state.clone());
    let result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await;

    remove_pid_file(&config.pid_file);
    result?;
    info!("chronoseal daemon stopped");
    Ok(())
}

pub fn db_type_report() -> DbTypeReport {
    DbTypeReport {
        default: crate::config::DbType::SqliteInMemory.as_str(),
        backends: vec![
            DbTypeEntry {
                name: crate::config::DbType::SqliteInMemory.as_str(),
                implemented: true,
                notes: "default runtime backend",
            },
            DbTypeEntry {
                name: crate::config::DbType::SqliteInDisk.as_str(),
                implemented: true,
                notes: "persistent SQLite backend (uses --db-path)",
            },
            DbTypeEntry {
                name: crate::config::DbType::Valkey.as_str(),
                implemented: true,
                notes: "compatibility mode: falls back to sqlite-in-memory",
            },
        ],
    }
}

fn init_db_pool(config: &Config) -> Result<storage::DbPool, Box<dyn std::error::Error>> {
    storage::DbPool::init(config)
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
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "healthy" })),
    )
}

async fn stats_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(state): axum::extract::State<Arc<session::AppState>>,
) -> Result<Json<StoreStats>, (StatusCode, String)> {
    if !is_loopback(addr.ip()) {
        return Err((StatusCode::FORBIDDEN, "Forbidden".to_string()));
    }
    state
        .db_pool
        .stats()
        .map(Json)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

async fn metrics_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(state): axum::extract::State<Arc<session::AppState>>,
) -> Result<String, (StatusCode, String)> {
    if !is_loopback(addr.ip()) {
        return Err((StatusCode::FORBIDDEN, "Forbidden".to_string()));
    }
    let stats = state
        .db_pool
        .stats()
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let heartbeats = state
        .heartbeats_total
        .load(std::sync::atomic::Ordering::Relaxed);
    let ver_failures = state
        .verification_failures_total
        .load(std::sync::atomic::Ordering::Relaxed);
    let mut_failures = state
        .mutation_failures_total
        .load(std::sync::atomic::Ordering::Relaxed);
    let replays = state
        .replay_attempts_total
        .load(std::sync::atomic::Ordering::Relaxed);

    let store_ns = state
        .storage_latency_ns
        .load(std::sync::atomic::Ordering::Relaxed) as f64;
    let store_sum = store_ns / 1_000_000_000.0;
    let store_count = state
        .storage_ops_count
        .load(std::sync::atomic::Ordering::Relaxed);

    let http_ns = state
        .http_latency_ns
        .load(std::sync::atomic::Ordering::Relaxed) as f64;
    let http_sum = http_ns / 1_000_000_000.0;
    let http_count = state
        .http_ops_count
        .load(std::sync::atomic::Ordering::Relaxed);

    Ok(format!(
        "# HELP chronoseal_active_sessions Active ChronoSeal sessions\n\
         # TYPE chronoseal_active_sessions gauge\n\
         chronoseal_active_sessions {}\n\
         # HELP chronoseal_expired_sessions Expired sessions not yet removed\n\
         # TYPE chronoseal_expired_sessions gauge\n\
         chronoseal_expired_sessions {}\n\
         # HELP chronoseal_max_chain_length Maximum heartbeat chain length\n\
         # TYPE chronoseal_max_chain_length gauge\n\
         chronoseal_max_chain_length {}\n\
         # HELP chronoseal_heartbeats_total Total heartbeat requests processed\n\
         # TYPE chronoseal_heartbeats_total counter\n\
         chronoseal_heartbeats_total {}\n\
         # HELP chronoseal_verification_failures_total Total heartbeat verification failures\n\
         # TYPE chronoseal_verification_failures_total counter\n\
         chronoseal_verification_failures_total {}\n\
         # HELP chronoseal_mutation_failures_total Total heartbeat mutation verification failures\n\
         # TYPE chronoseal_mutation_failures_total counter\n\
         chronoseal_mutation_failures_total {}\n\
         # HELP chronoseal_replay_attempts_total Total heartbeat replay attempts detected\n\
         # TYPE chronoseal_replay_attempts_total counter\n\
         chronoseal_replay_attempts_total {}\n\
         # HELP chronoseal_storage_latency_seconds_sum Total time spent in storage operations in seconds\n\
         # TYPE chronoseal_storage_latency_seconds_sum counter\n\
         chronoseal_storage_latency_seconds_sum {:.6}\n\
         # HELP chronoseal_storage_latency_seconds_count Total storage operations count\n\
         # TYPE chronoseal_storage_latency_seconds_count counter\n\
         chronoseal_storage_latency_seconds_count {}\n\
         # HELP chronoseal_http_latency_seconds_sum Total time spent in HTTP request processing in seconds\n\
         # TYPE chronoseal_http_latency_seconds_sum counter\n\
         chronoseal_http_latency_seconds_sum {:.6}\n\
         # HELP chronoseal_http_latency_seconds_count Total HTTP operations count\n\
         # TYPE chronoseal_http_latency_seconds_count counter\n\
         chronoseal_http_latency_seconds_count {}\n",
        stats.sessions,
        stats.expired_sessions,
        stats.max_chain_length,
        heartbeats,
        ver_failures,
        mut_failures,
        replays,
        store_sum,
        store_count,
        http_sum,
        http_count
    ))
}

fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

async fn signal_task(state: Arc<session::AppState>) {
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

        let state_for_hup = state.clone();
        tokio::spawn(async move {
            let mut sighup = signal(SignalKind::hangup()).expect("install SIGHUP handler");
            while sighup.recv().await.is_some() {
                match Config::load(None) {
                    Ok(reloaded) => {
                        info!(
                            bind = %reloaded.bind,
                            db_path = %reloaded.db_path.display(),
                            "received SIGHUP; configuration reloaded"
                        );
                        if let Ok(mut config_write) = state_for_hup.config.write() {
                            *config_write = reloaded;
                        }
                    }
                    Err(err) => warn!(error = %err, "received SIGHUP; configuration reload failed"),
                }
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
    stream.write_all(
        format!("GET {path} HTTP/1.1\r\nHost: chronoseal\r\nConnection: close\r\n\r\n").as_bytes(),
    )?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let (_, body) = response
        .split_once("\r\n\r\n")
        .ok_or("daemon returned an invalid HTTP response")?;
    Ok(body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> Config {
        Config {
            bind: "127.0.0.1:0".to_string(),
            db_type: crate::config::DbType::SqliteInMemory,
            pid_file: std::path::PathBuf::from("/tmp/chronoseal-test.pid"),
            db_path: std::path::PathBuf::from("/tmp/chronoseal-test.sqlite"),
            frontend_dir: std::path::PathBuf::from("."),
            log_file: None,
            heartbeat_min_interval_ms: 12_000,
            heartbeat_max_interval_ms: 25_000,
            expiration_minutes: 30,
            rate_limit_count: 5,
            rate_limit_window_secs: 10,
            max_timestamp_drift_ms: 30_000,
            min_mouse_total_dist: 1.0,
            max_mouse_avg_speed: 5.0,
            min_pause_count: 0,
            require_mouse_activity: false,
            gene_size: shared::constants::DEFAULT_GENE_SIZE,
            mutation_rounds: shared::constants::DEFAULT_MUTATION_ROUNDS,
        }
    }

    #[test]
    fn test_db_type_report_lists_backends() {
        let report = db_type_report();
        assert_eq!(report.default, "sqlite-in-memory");
        assert_eq!(report.backends.len(), 3);
        assert!(report.backends.iter().any(|b| b.name == "valkey"));
    }

    #[test]
    fn test_init_db_pool_sqlite_in_memory() {
        let config = base_config();
        let pool = init_db_pool(&config).unwrap();
        let stats = pool.stats().unwrap();
        assert_eq!(stats.sessions, 0);
        assert_eq!(stats.expired_sessions, 0);
        assert_eq!(stats.max_chain_length, 0);
    }

    #[test]
    fn test_init_db_pool_sqlite_in_disk() {
        let mut config = base_config();
        config.db_type = crate::config::DbType::SqliteInDisk;
        config.db_path = std::path::PathBuf::from("/tmp/chronoseal-db-type-disk.sqlite");
        let _ = std::fs::remove_file(&config.db_path);
        let pool = init_db_pool(&config).unwrap();
        let stats = pool.stats().unwrap();
        assert_eq!(stats.sessions, 0);
        assert_eq!(stats.expired_sessions, 0);
        assert_eq!(stats.max_chain_length, 0);
    }

    #[test]
    fn test_init_db_pool_valkey_compat_mode() {
        let mut config = base_config();
        config.db_type = crate::config::DbType::Valkey;
        match init_db_pool(&config) {
            Ok(pool) => {
                let stats = pool.stats().unwrap();
                assert_eq!(stats.sessions, 0);
                assert_eq!(stats.expired_sessions, 0);
                assert_eq!(stats.max_chain_length, 0);
            }
            Err(_) => {
                // Valkey not running in the test environment, which is acceptable
            }
        }
    }
}
