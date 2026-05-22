use crate::cli::{RunArgs, RuntimeArgs};
use serde::{Deserialize, Serialize};
use std::{
    env, fs, io,
    net::SocketAddr,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub bind: String,
    pub pid_file: PathBuf,
    pub db_path: PathBuf,
    pub frontend_dir: PathBuf,
    pub log_file: Option<PathBuf>,
    pub heartbeat_min_interval_ms: u64,
    pub heartbeat_max_interval_ms: u64,
    pub expiration_minutes: i64,
    pub rate_limit_count: u32,
    pub rate_limit_window_secs: u64,
    pub max_timestamp_drift_ms: i64,
    pub min_mouse_total_dist: f64,
    pub max_mouse_avg_speed: f64,
    pub min_pause_count: u32,
    pub require_mouse_activity: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:3000".to_string(),
            pid_file: PathBuf::from("/run/chronoseal.pid"),
            db_path: default_state_dir().join("chronoseal.sqlite"),
            frontend_dir: PathBuf::from("/usr/share/chronoseal/frontend"),
            log_file: None,
            heartbeat_min_interval_ms: 12_000,
            heartbeat_max_interval_ms: 25_000,
            expiration_minutes: 30,
            rate_limit_count: 5,
            rate_limit_window_secs: 10,
            max_timestamp_drift_ms: 30_000,
            min_mouse_total_dist: 10.0,
            max_mouse_avg_speed: 2.0,
            min_pause_count: 1,
            require_mouse_activity: true,
        }
    }
}

impl Config {
    pub fn load(config_path: Option<&Path>) -> Result<Self, ConfigError> {
        let mut config = Self::default();

        if let Some(path) = config_path
            .map(Path::to_path_buf)
            .or_else(discover_config_path)
        {
            let raw = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
                path: path.clone(),
                source,
            })?;
            config = toml::from_str(&raw).map_err(|source| ConfigError::Parse {
                path: path.clone(),
                source,
            })?;
        }

        config.apply_env();
        config.validate()?;
        Ok(config)
    }

    pub fn apply_runtime_args(&mut self, args: &RuntimeArgs) {
        if let Some(bind) = &args.bind {
            self.bind.clone_from(bind);
        }
        if let Some(pid_file) = &args.pid_file {
            self.pid_file = pid_file.clone();
        }
    }

    pub fn apply_run_args(&mut self, args: &RunArgs) {
        self.apply_runtime_args(&args.runtime);
        if let Some(db_path) = &args.db_path {
            self.db_path = db_path.clone();
        }
        if let Some(frontend_dir) = &args.frontend_dir {
            self.frontend_dir = frontend_dir.clone();
        }
        if let Some(log_file) = &args.log_file {
            self.log_file = Some(log_file.clone());
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.bind
            .parse::<SocketAddr>()
            .map_err(|source| ConfigError::InvalidBind {
                bind: self.bind.clone(),
                source,
            })?;
        Ok(())
    }

    fn apply_env(&mut self) {
        if let Ok(value) = env::var("CHRONOSEAL_BIND") {
            self.bind = value;
        }
        if let Ok(value) = env::var("CHRONOSEAL_PID_FILE") {
            self.pid_file = PathBuf::from(value);
        }
        if let Ok(value) = env::var("CHRONOSEAL_DB_PATH") {
            self.db_path = PathBuf::from(value);
        }
        if let Ok(value) = env::var("CHRONOSEAL_FRONTEND_DIR") {
            self.frontend_dir = PathBuf::from(value);
        }
        if let Ok(value) = env::var("CHRONOSEAL_LOG_FILE") {
            self.log_file = Some(PathBuf::from(value));
        }
        if let Ok(value) = env::var("CHRONOSEAL_HEARTBEAT_MIN_INTERVAL_MS") {
            if let Ok(val) = value.parse() {
                self.heartbeat_min_interval_ms = val;
            }
        }
        if let Ok(value) = env::var("CHRONOSEAL_HEARTBEAT_MAX_INTERVAL_MS") {
            if let Ok(val) = value.parse() {
                self.heartbeat_max_interval_ms = val;
            }
        }
        if let Ok(value) = env::var("CHRONOSEAL_EXPIRATION_MINUTES") {
            if let Ok(val) = value.parse() {
                self.expiration_minutes = val;
            }
        }
        if let Ok(value) = env::var("CHRONOSEAL_RATE_LIMIT_COUNT") {
            if let Ok(val) = value.parse() {
                self.rate_limit_count = val;
            }
        }
        if let Ok(value) = env::var("CHRONOSEAL_RATE_LIMIT_WINDOW_SECS") {
            if let Ok(val) = value.parse() {
                self.rate_limit_window_secs = val;
            }
        }
        if let Ok(value) = env::var("CHRONOSEAL_MAX_TIMESTAMP_DRIFT_MS") {
            if let Ok(val) = value.parse() {
                self.max_timestamp_drift_ms = val;
            }
        }
        if let Ok(value) = env::var("CHRONOSEAL_MIN_MOUSE_TOTAL_DIST") {
            if let Ok(val) = value.parse() {
                self.min_mouse_total_dist = val;
            }
        }
        if let Ok(value) = env::var("CHRONOSEAL_MAX_MOUSE_AVG_SPEED") {
            if let Ok(val) = value.parse() {
                self.max_mouse_avg_speed = val;
            }
        }
        if let Ok(value) = env::var("CHRONOSEAL_MIN_PAUSE_COUNT") {
            if let Ok(val) = value.parse() {
                self.min_pause_count = val;
            }
        }
        if let Ok(value) = env::var("CHRONOSEAL_REQUIRE_MOUSE_ACTIVITY") {
            if let Ok(val) = value.parse() {
                self.require_mouse_activity = val;
            }
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    InvalidBind {
        bind: String,
        source: std::net::AddrParseError,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, source } => write!(f, "failed to read {}: {source}", path.display()),
            Self::Parse { path, source } => {
                write!(f, "failed to parse {} as TOML: {source}", path.display())
            }
            Self::InvalidBind { bind, source } => {
                write!(f, "invalid bind address {bind}: {source}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

fn discover_config_path() -> Option<PathBuf> {
    if let Ok(path) = env::var("CHRONOSEAL_CONFIG") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }
    user_config_candidates()
        .into_iter()
        .find(|candidate| candidate.is_file())
}

pub fn user_config_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("/etc/chronoseal/config.toml")];
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        candidates.push(PathBuf::from(xdg).join("chronoseal/config.toml"));
    } else if let Ok(home) = env::var("HOME") {
        candidates.push(PathBuf::from(home).join(".config/chronoseal/config.toml"));
    }
    candidates
}

fn default_state_dir() -> PathBuf {
    if let Ok(value) = env::var("CHRONOSEAL_STATE_DIR") {
        return PathBuf::from(value);
    }
    if let Ok(value) = env::var("XDG_STATE_HOME") {
        return PathBuf::from(value).join("chronoseal");
    }
    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home).join(".local/state/chronoseal");
    }
    PathBuf::from("/var/lib/chronoseal")
}
