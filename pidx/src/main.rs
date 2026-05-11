mod classify;
mod commands;
mod config;
mod db;
mod display;
mod github;
mod health;
mod llm;

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

    /// LLM doc pipeline — discover unprocessed commits and (eventually)
    /// regenerate per-repo CHANGELOG.md / architecture.md / description.
    /// Phase 1: `--dry-run` (no LLM) or `--classify` (run map step,
    /// cache results). Subcommand `export` retains the legacy
    /// structured-markdown export.
    Changelog {
        #[command(subcommand)]
        action: Option<ChangelogAction>,

        /// Repo to operate on (matches `[[repos]] name` in pidx.toml).
        #[arg(long)]
        repo: Option<String>,

        /// Discover commits and print the plan; do not call any LLM
        /// or write any files.
        #[arg(long)]
        dry_run: bool,

        /// Run the Phase 1 map pipeline: enrich commits with diffs,
        /// classify each via the LLM (parallel, cached), upsert
        /// results into `commit_classifications`. Does NOT advance
        /// `last_processed_sha` (Phase 2's job) and does NOT write
        /// any docs (Phase 2+).
        #[arg(long)]
        classify: bool,

        /// Run the Phase 2 reduce pipeline: group cached
        /// classifications by ISO week, ask the LLM to compose the
        /// per-week Keep-a-Changelog fragment (cached in
        /// `doc_reducer_outputs`), render the merged file, and write
        /// `docs/<repo>/CHANGELOG.md`. Implies `--classify` so a
        /// fresh-DB run does both phases in one command. Advances
        /// `last_processed_sha` on success.
        #[arg(long)]
        reduce: bool,

        /// Run the Phase 3 architecture reducer ONLY: read existing
        /// cached classifications + a fresh directory snapshot of the
        /// submodule, ask the LLM to compose `architecture.md` (cached
        /// in `doc_reducer_outputs` as `kind=architecture`,
        /// `scope_key=all`), and write
        /// `docs/<repo>/architecture.md`. Does NOT classify and does
        /// NOT advance `last_processed_sha`; fails clearly if there
        /// are no cached classifications.
        #[arg(long = "reduce-arch")]
        reduce_arch: bool,

        /// Bypass the cache and re-classify every commit. Still
        /// respects `prompt_version` for INSERT OR REPLACE. With
        /// `--reduce` or `--reduce-arch`, also bypasses the reducer
        /// cache.
        #[arg(long)]
        force: bool,
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
        Commands::Changelog { action, repo, dry_run, classify, reduce, reduce_arch, force } => match action {
            Some(ChangelogAction::Export { week, repo: subcmd_repo }) => {
                commands::changelog_command::export(
                    &config,
                    week.as_deref(),
                    subcmd_repo.as_deref(),
                )?;
            }
            None => {
                commands::changelog_command::run(
                    &config,
                    repo.as_deref(),
                    dry_run,
                    classify,
                    reduce,
                    reduce_arch,
                    force,
                )
                .await?;
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
