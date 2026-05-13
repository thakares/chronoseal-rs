mod cleanup;
mod cli;
mod config;
mod crypto;
mod fingerprint;
mod middleware;
mod output;
mod ratelimit;
mod routes;
mod runtime;
mod session;
mod storage;
mod trust;
mod vm;

use clap::{CommandFactory, Parser};
use cli::{Cli, Command, ConfigCommand, GenerateCommand};
use config::Config;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    if let Err(err) = try_main().await {
        eprintln!("chronoseal: {err}");
        std::process::exit(1);
    }
}

async fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let log_filter = cli.globals.log.as_deref().unwrap_or("info");
    let log_file = log_file_for_command(&cli);
    let _log_guard = init_logging(log_filter, log_file)?;

    match &cli.command {
        None | Some(Command::Run(_)) => {
            let mut config = Config::load(cli.globals.config.as_deref())?;
            if let Some(Command::Run(args)) = &cli.command {
                config.apply_run_args(args);
                config.validate()?;
            }
            runtime::run_daemon(config).await?;
        }
        Some(Command::Status(args)) => {
            let mut config = Config::load(cli.globals.config.as_deref())?;
            config.apply_runtime_args(args);
            config.validate()?;
            output::print(cli.globals.output_format(), &runtime::probe_status(&config))?;
        }
        Some(Command::Health(args)) => {
            let mut config = Config::load(cli.globals.config.as_deref())?;
            config.apply_runtime_args(args);
            config.validate()?;
            let report = runtime::probe_health(&config);
            let healthy = report.status == "healthy";
            output::print(cli.globals.output_format(), &report)?;
            if !healthy {
                std::process::exit(2);
            }
        }
        Some(Command::Config(ConfigCommand::Check(args))) => {
            let mut config = Config::load(cli.globals.config.as_deref())?;
            config.apply_runtime_args(args);
            config.validate()?;
            output::print(cli.globals.output_format(), &config)?;
        }
        Some(Command::Generate(GenerateCommand::Keypair)) => {
            output::print(cli.globals.output_format(), &runtime::generate_keypair())?;
        }
        Some(Command::Version) => {
            output::print(cli.globals.output_format(), &runtime::version())?;
        }
        Some(Command::Metrics(args)) => {
            let mut config = Config::load(cli.globals.config.as_deref())?;
            config.apply_runtime_args(args);
            config.validate()?;
            print!("{}", runtime::fetch_metrics(&config)?);
        }
        Some(Command::Stats(args)) => {
            let mut config = Config::load(cli.globals.config.as_deref())?;
            config.apply_runtime_args(args);
            config.validate()?;
            output::print(cli.globals.output_format(), &runtime::fetch_stats(&config)?)?;
        }
        Some(Command::Completion { shell }) => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            clap_complete::generate(*shell, &mut command, name, &mut std::io::stdout());
        }
    }
    Ok(())
}

fn log_file_for_command(cli: &Cli) -> Option<PathBuf> {
    match &cli.command {
        None => Config::load(cli.globals.config.as_deref())
            .ok()
            .and_then(|config| config.log_file),
        Some(Command::Run(args)) => {
            let mut config = Config::load(cli.globals.config.as_deref()).ok()?;
            config.apply_run_args(args);
            config.log_file
        }
        _ => None,
    }
}

fn init_logging(
    filter: &str,
    log_file: Option<PathBuf>,
) -> Result<Option<tracing_appender::non_blocking::WorkerGuard>, Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_new(filter)?;
    if let Some(path) = log_file {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let directory = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("chronoseal.jsonl");
        let appender = tracing_appender::rolling::never(directory, file_name);
        let (writer, guard) = tracing_appender::non_blocking(appender);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().with_target(false))
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_target(false)
                    .with_writer(writer),
            )
            .init();
        Ok(Some(guard))
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(false)
            .init();
        Ok(None)
    }
}
