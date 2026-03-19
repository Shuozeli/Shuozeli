# pidx Architecture

Last updated: 2026-03-19

## Overview

pidx is a director-level project index CLI that syncs GitHub data into local SQLite, generates structured docs for LLM consumption, and ingests LLM-produced analysis back. It provides a hybrid human+LLM dashboard for tracking progress, velocity, and health across multiple repositories.

## Data Flow

```
pidx sync  -->  GitHub API  -->  SQLite (raw data)
pidx docs export  -->  per-project markdown  -->  ~/.pidx/docs/{repo}/
LLM processes docs  -->  writes analysis back  -->  ~/.pidx/docs/{repo}/llm_summary.md
pidx docs ingest  -->  reads LLM output  -->  SQLite (llm_summaries table)
pidx status / report  -->  combines raw data + LLM summaries
pidx index  -->  reads SQLite + config  -->  regenerates root README.md
```

## Module Structure

```
src/
  main.rs              -- clap CLI, command dispatch
  config.rs            -- TOML config loading (fail-fast), includes index_path and categories
  classify.rs          -- CommitCategory enum (7 variants) from message prefixes
  health.rs            -- Health score computation (recency + velocity + issues)
  db/                  -- SQLite via rusqlite, all access in transactions
    mod.rs             -- Database struct, migrations, tx() wrapper
    schema.rs          -- CREATE TABLE DDL (6 tables: repos, commits, issues, releases, llm_summaries, sync_events)
    repo_store.rs      -- upsert_repo, get_all_repos, get_repo_by_name
    commit_store.rs    -- upsert_commit, get_commits_since, count_commits_since, get_all_commits_for_repo
    issue_store.rs     -- upsert_issue, get_open_issues, get_issues_by_state, get_all_issues_for_repo
    release_store.rs   -- upsert_release, get_releases_for_repo
    llm_summary_store.rs -- insert_llm_summary, get_latest_summary
    sync_log_store.rs  -- log_sync_event (writes to sync_events table)
  github/              -- GitHub API client via reqwest
    mod.rs             -- GithubClient (shared HTTP client with auth)
    types.rs           -- API response deserialization structs (GithubRepo, GithubCommit, GithubIssue, GithubRelease)
    repo_fetcher.rs    -- fetch_repo
    commit_fetcher.rs  -- fetch_commits(repo, per_page)
    issue_fetcher.rs   -- fetch_issues(repo, state) -- filters out pull requests
    release_fetcher.rs -- fetch_releases(repo)
  commands/            -- One module per CLI command
    sync_command.rs    -- Pulls data from GitHub, stores in SQLite
    status_command.rs  -- Table overview with health scores
    activity_command.rs-- Recent commits grouped by day
    report_command.rs  -- Digest with category breakdown
    docs_command.rs    -- Export markdown / ingest LLM summaries
    index_command.rs   -- Regenerate root README.md project catalog from DB + config
  display/             -- Output rendering
    table_renderer.rs  -- comfy-table terminal tables (status + activity)
    markdown_renderer.rs -- Markdown report generation
```

## Design Decisions

- **Allowlist model**: Config is manually edited TOML. No auto-discovery.
- **All DB access in transactions**: Including reads, via `db.tx()`.
- **Fail-fast**: Missing config or env vars cause immediate errors.
- **Commit classification**: Prefix-based. 7 categories: Feature (feat:, feat(, "add "), Bugfix (fix:, fix(), Docs (docs:, docs(), Refactor (refactor:, refactor(), Test (test:, test(), Sync (sync:, sync(), and Chore (default).
- **Health score**: Weighted composite of recency (40%), velocity (40%), issues (20%). Labels: Active (>=80), Healthy (>=60), Moderate (>=40), Stale (>=20), Dormant (<20).
