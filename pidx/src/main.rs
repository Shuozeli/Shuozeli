mod classify;
mod commands;
mod config;
mod db;
mod display;
mod github;
mod health;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "pidx", about = "Director-level project index CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Pull commits, issues, releases from GitHub API
    Sync {
        /// Sync only this repo
        #[arg(long)]
        repo: Option<String>,
    },

    /// Table overview of all repos
    Status,

    /// Recent commits grouped by day with category tags
    Activity {
        /// Filter to a specific repo
        #[arg(long)]
        repo: Option<String>,

        /// Time range (e.g. 7d, 2w)
        #[arg(long, default_value = "7d")]
        since: String,
    },

    /// Regenerate the project index README.md
    Index,

    /// Weekly digest with velocity and health
    Report {
        /// Output format: table or md
        #[arg(long, default_value = "table")]
        format: String,

        /// Time period (e.g. 7d, 2w)
        #[arg(long, default_value = "7d")]
        period: String,
    },

    /// Generate or ingest documentation
    Docs {
        #[command(subcommand)]
        action: DocsAction,
    },

    /// Export weekly changelog data for agent consumption
    Changelog {
        #[command(subcommand)]
        action: ChangelogAction,
    },

    /// Display current config
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ChangelogAction {
    /// Export raw weekly data as structured markdown per repo
    Export {
        /// ISO week (e.g. 2026-W12). Defaults to current week.
        #[arg(long)]
        week: Option<String>,

        /// Export only this repo
        #[arg(long)]
        repo: Option<String>,
    },
}

#[derive(Subcommand)]
enum DocsAction {
    /// Export per-project markdown docs for LLM consumption
    Export {
        /// Export only this repo
        #[arg(long)]
        repo: Option<String>,
    },

    /// Ingest LLM-produced analysis back into SQLite
    Ingest {
        /// Ingest only this repo
        #[arg(long)]
        repo: Option<String>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("pidx=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    let config = config::Config::load()?;

    match cli.command {
        Commands::Sync { repo } => {
            commands::sync_command::run(&config, repo.as_deref()).await?;
        }
        Commands::Status => {
            commands::status_command::run(&config)?;
        }
        Commands::Index => {
            commands::index_command::run(&config)?;
        }
        Commands::Activity { repo, since } => {
            commands::activity_command::run(&config, repo.as_deref(), &since)?;
        }
        Commands::Report { format, period } => {
            commands::report_command::run(&config, &format, &period)?;
        }
        Commands::Docs { action } => match action {
            DocsAction::Export { repo } => {
                commands::docs_command::export(&config, repo.as_deref())?;
            }
            DocsAction::Ingest { repo } => {
                commands::docs_command::ingest(&config, repo.as_deref())?;
            }
        },
        Commands::Changelog { action } => match action {
            ChangelogAction::Export { week, repo } => {
                commands::changelog_command::export(&config, week.as_deref(), repo.as_deref())?;
            }
        },
        Commands::Config { action } => match action {
            ConfigAction::Show => {
                println!("Config path: {}", config::Config::config_path().display());
                println!("Owner: {}", config.owner);
                println!("DB path: {}", config.db_path().display());
                println!(
                    "Token env: {} ({})",
                    config.sync.github_token_env,
                    if config.github_token().is_ok() {
                        "set"
                    } else {
                        "NOT SET"
                    }
                );
                println!("Commits per sync: {}", config.sync.commits_per_sync);
                println!("\nRepos ({}):", config.repos.len());
                for repo in &config.repos {
                    println!("  - {} [{}]", repo.name, repo.category);
                }
            }
        },
    }

    Ok(())
}
