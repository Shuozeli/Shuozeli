# pidx

Last updated: 2026-03-19

Director-level project index CLI. Syncs GitHub data into local SQLite, generates structured docs for LLM analysis, and presents health/velocity dashboards.

## Setup

1. Create config at `~/.pidx/pidx.toml` (see `pidx.toml.example`)
2. Set `GITHUB_TOKEN` environment variable
3. `cargo build --release`

## Configuration

The config file at `~/.pidx/pidx.toml` supports:

- `owner` -- GitHub owner/org name.
- `index_path` -- Path to the root `README.md` that `pidx index` regenerates.
- `[sync]` -- Section with `github_token_env`, `commits_per_sync`, `db_path`.
- `[[categories]]` -- Define category keys and display titles (e.g., `key = "devtools"`, `title = "Developer Tools"`).
- `[[repos]]` -- Each entry has `name`, `category`, and optional `description` (overrides the GitHub description in the generated index).

## Usage

```bash
pidx sync [--repo <name>]                   # Pull data from GitHub
pidx status                                 # Table overview with health scores
pidx activity [--repo <name>] [--since 7d]  # Recent commits (default: 7d)
pidx report [--format table] [--period 7d]  # Digest (format: table or md; default: table, 7d)
pidx index                                  # Regenerate root README.md project catalog
pidx docs export [--repo <name>]            # Generate per-repo markdown for LLM
pidx docs ingest [--repo <name>]            # Read LLM analysis back into SQLite
pidx config show                            # Display config
```

## LLM Integration

1. Run `pidx docs export` to generate per-repo markdown at `~/.pidx/docs/{repo}/`
   - Exported files: `overview.md`, `changelog.md`, `issues.md`, `releases.md`, `health.md`
2. Feed the docs to an LLM (e.g., Gemini)
3. LLM writes `llm_summary.md` back to each repo's doc dir in this format:
   ```
   ---
   analyzed_at: 2026-03-19T12:00:00Z
   model: gemini-2.0
   ---

   ## Project Status
   <summary text>

   ## Key Risks
   <risks text>

   ## Recommendations
   <recommendations text>
   ```
4. Run `pidx docs ingest` to store in SQLite
5. `pidx status` and `pidx report` now include LLM insights
