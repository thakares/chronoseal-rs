use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Yaml,
}

#[derive(Debug, Parser)]
#[command(
    name = "chronoseal",
    version,
    about = "Linux-native cryptographic browser attestation service",
    long_about = "ChronoSeal runs as a composable Unix service. The CLI is the source of truth for daemon operation, health checks, configuration validation, metrics, and shell integration.",
    after_help = "Examples:\n  chronoseal\n  chronoseal run --bind 127.0.0.1:3000\n  chronoseal status --format json\n  chronoseal health --config /etc/chronoseal/config.toml\n  chronoseal config check --output yaml\n  chronoseal generate keypair\n  chronoseal completion bash > /etc/bash_completion.d/chronoseal\n\nConfiguration precedence:\n  CLI flags > CHRONOSEAL_* environment variables > config file > built-in defaults\n\nDefault config discovery:\n  /etc/chronoseal/config.toml, then $XDG_CONFIG_HOME/chronoseal/config.toml, then ~/.config/chronoseal/config.toml"
)]
pub struct Cli {
    #[command(flatten)]
    pub globals: GlobalArgs,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Args)]
pub struct GlobalArgs {
    /// Path to config file.
    #[arg(long, env = "CHRONOSEAL_CONFIG", global = true)]
    pub config: Option<PathBuf>,

    /// Output format for machine-readable commands.
    #[arg(long, short = 'f', value_enum, default_value = "text", global = true)]
    pub format: OutputFormat,

    /// Alias for --format, provided for Unix tool compatibility.
    #[arg(long, value_enum, global = true)]
    pub output: Option<OutputFormat>,

    /// Override the logging filter, for example info, chronoseal=debug.
    #[arg(long, env = "CHRONOSEAL_LOG", global = true)]
    pub log: Option<String>,
}

impl GlobalArgs {
    pub fn output_format(&self) -> OutputFormat {
        self.output.unwrap_or(self.format)
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the ChronoSeal daemon.
    #[command(after_help = "Examples:\n  chronoseal run\n  chronoseal run --bind 127.0.0.1:3000 --frontend-dir /srv/chronoseal/frontend\n  CHRONOSEAL_BIND=0.0.0.0:3000 chronoseal run")]
    Run(RunArgs),

    /// Report whether the configured daemon is reachable and which PID file is present.
    #[command(after_help = "Examples:\n  chronoseal status\n  chronoseal status --format json\n  chronoseal status --pid-file /run/chronoseal.pid")]
    Status(RuntimeArgs),

    /// Perform a daemon health probe.
    #[command(after_help = "Examples:\n  chronoseal health\n  chronoseal health --format json\n  chronoseal health --bind 127.0.0.1:3000")]
    Health(RuntimeArgs),

    /// Validate and print effective configuration.
    #[command(subcommand)]
    Config(ConfigCommand),

    /// Generate operational material.
    #[command(subcommand)]
    Generate(GenerateCommand),

    /// Print version and build information.
    #[command(after_help = "Examples:\n  chronoseal version\n  chronoseal version --format json")]
    Version,

    /// Print Prometheus metrics from the running daemon.
    #[command(after_help = "Examples:\n  chronoseal metrics\n  chronoseal metrics --bind 127.0.0.1:3000")]
    Metrics(RuntimeArgs),

    /// Print service statistics from the running daemon.
    #[command(after_help = "Examples:\n  chronoseal stats\n  chronoseal stats --format json")]
    Stats(RuntimeArgs),

    /// Generate shell completions.
    #[command(after_help = "Examples:\n  chronoseal completion bash\n  chronoseal completion zsh > ~/.zfunc/_chronoseal")]
    Completion { shell: clap_complete::Shell },
}

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    #[command(flatten)]
    pub runtime: RuntimeArgs,

    /// SQLite database path. Use ':memory:' for ephemeral state.
    #[arg(long, env = "CHRONOSEAL_DB_PATH")]
    pub db_path: Option<PathBuf>,

    /// Static frontend directory served at /.
    #[arg(long, env = "CHRONOSEAL_FRONTEND_DIR")]
    pub frontend_dir: Option<PathBuf>,

    /// Optional structured JSON log file.
    #[arg(long, env = "CHRONOSEAL_LOG_FILE")]
    pub log_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct RuntimeArgs {
    /// Socket address the daemon binds to, or that CLI probes connect to.
    #[arg(long, env = "CHRONOSEAL_BIND")]
    pub bind: Option<String>,

    /// PID file path.
    #[arg(long, env = "CHRONOSEAL_PID_FILE")]
    pub pid_file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Validate configuration and print the effective values.
    #[command(after_help = "Examples:\n  chronoseal config check\n  chronoseal config check --config /etc/chronoseal/config.toml\n  chronoseal config check --output json")]
    Check(RuntimeArgs),
}

#[derive(Debug, Subcommand)]
pub enum GenerateCommand {
    /// Generate an Ed25519 keypair as hex-encoded JSON/YAML/text.
    #[command(after_help = "Examples:\n  chronoseal generate keypair\n  chronoseal generate keypair --format json")]
    Keypair,
}
