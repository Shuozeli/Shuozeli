<!-- agent-updated: 2026-05-09T22:45:00Z -->

# LLM Doc Pipeline

A map-reduce pipeline that reads each repo's commit history and updates
**three kinds of docs** in `Shuozeli/Shuozeli/docs/<repo>/`:

| Doc kind        | File                       | Reducer scope            |
|-----------------|----------------------------|--------------------------|
| Changelog       | `CHANGELOG.md`             | Per-week (or per-tag)    |
| Architecture    | `architecture.md`          | Whole-repo, refreshed    |
| Description     | (override in `pidx.toml`)  | One-line summary         |

The pipeline is **incremental**, **idempotent**, **resumable**, and
**write-only** (never auto-commits). State lives in `~/.pidx/pidx.db`;
LLM classifications are cached per `(repo, commit_sha, prompt_version)`
so re-runs after a single new commit are nearly free.

## Architecture

### Phase shape (map-reduce fanout)

```
                                                 ┌─→ classify(commit_1) ─┐
discover(repo) ──→ enrich(commits) ──── map ────┤   classify(commit_2)   │── reduce ──→ render(kind) ──→ write(file)
   │                  │                         └─→ classify(commit_N) ─┘                      │
   │                  │                                                                         │
   ▼                  ▼                                                                         ▼
 db: last_           git: diff_                                                          docs/<repo>/<file>
 processed_sha       stat + N-line                                                        (writes only;
                     head per file                                                         caller commits)
```

- **discover**: query `~/.pidx/pidx.db` for commits in `(last_processed_sha, HEAD]`. Cheap.
- **enrich**: for each commit, fetch `git show --stat --diff-filter=ACDMR -p` capped at N lines per file (default 40). Cheap; bounded.
- **map**: per-commit LLM classify in parallel (bounded concurrency, default 4 in-flight). Each result cached on `(repo_id, sha, prompt_version)`. Cache hit ⇒ no LLM call.
- **reduce**: group cached classifications by reducer scope (week / tag / whole-repo) and ask the LLM to compose the doc fragment.
- **render**: merge reducer output with the existing file (idempotent — re-running with the same inputs produces byte-identical output).
- **write**: overwrite `docs/<repo>/<file>`. Print path and diff summary; do **not** stage or commit.

### Fan-out across repos × kinds

```
pidx changelog --all --kinds changelog,architecture,description
   │
   └──► repo₁ ──┬─→ changelog reducer ──→ docs/repo₁/CHANGELOG.md
        │       ├─→ architecture reducer → docs/repo₁/architecture.md
        │       └─→ description reducer ──→ pidx.toml override
        repo₂ ──┬─→ changelog
        │       ├─→ architecture
        │       └─→ description
        ⋮
```

Outer loop parallelizes repos (default 2 concurrent). Inner loop
parallelizes the per-commit map (default 4 concurrent per repo).
**Total in-flight LLM calls capped** by `[llm].max_concurrent_requests`
(default 8). Reducer calls are serialized per repo so retry semantics
stay simple.

## Schema additions

```sql
-- New table: cached per-commit classification.
CREATE TABLE commit_classifications (
    repo_id          INTEGER NOT NULL,
    sha              TEXT    NOT NULL,
    prompt_version   INTEGER NOT NULL,
    category         TEXT    NOT NULL,   -- Added | Changed | Fixed | Removed | Internal
    summary          TEXT    NOT NULL,   -- one-line, present-tense imperative
    impact           TEXT    NOT NULL,   -- minor | major | breaking
    llm_provider     TEXT    NOT NULL,
    llm_model        TEXT    NOT NULL,
    classified_at    INTEGER NOT NULL,
    PRIMARY KEY (repo_id, sha, prompt_version),
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

-- New table: per-repo per-kind reducer output cache.
CREATE TABLE doc_reducer_outputs (
    repo_id          INTEGER NOT NULL,
    kind             TEXT    NOT NULL,   -- changelog | architecture | description
    scope_key        TEXT    NOT NULL,   -- "week-2026-W18" | "all" | "all"
    input_hash       TEXT    NOT NULL,   -- BLAKE3 of (sorted classification SHAs + prompt_version)
    output           TEXT    NOT NULL,
    llm_provider     TEXT    NOT NULL,
    llm_model        TEXT    NOT NULL,
    rendered_at      INTEGER NOT NULL,
    PRIMARY KEY (repo_id, kind, scope_key)
);

-- Add to existing repos table:
ALTER TABLE repos ADD COLUMN last_processed_sha TEXT;
```

`prompt_version` bumps when the classification prompt changes — cache is
invalidated automatically. `input_hash` for the reducer cache short-
circuits when no underlying classifications have changed.

## LLM provider abstraction

Trait-based. Anthropic Messages API on the wire (MiniMax, Doubao, and
Anthropic itself all speak the same JSON shape — provider differences
are confined to base URL + auth header):

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn classify_commit(&self, req: ClassifyRequest) -> Result<Classification>;
    async fn reduce_changelog(&self, req: ReduceChangelogRequest) -> Result<String>;
    async fn reduce_architecture(&self, req: ReduceArchitectureRequest) -> Result<String>;
    async fn reduce_description(&self, req: ReduceDescriptionRequest) -> Result<String>;
}

pub struct AnthropicCompatibleClient {
    base_url: Url,
    api_key: SecretString,
    model: String,
    http: reqwest::Client,
}
```

Config (extends `pidx.toml`):

```toml
[llm]
provider = "minimax"          # minimax | doubao | anthropic
model = "MiniMax-M2"
api_key_env = "MINIMAX_API_KEY"
base_url = "https://api.minimax.chat/anthropic"   # provider-specific
max_concurrent_requests = 8
classify_max_tokens = 400
reduce_max_tokens = 2000

[llm.classify]
diff_lines_per_file = 40      # cap per-file diff size in classify input

[llm.budget]
daily_token_limit = 5_000_000  # hard stop on cost
warn_at_pct = 80               # log a warning at 80% utilization
```

## CLI surface

```
pidx changelog --repo <name>                    # one repo, all kinds
pidx changelog --all                            # all configured repos
pidx changelog --since 7d                       # only repos with commits in window
pidx changelog --repo <name> --kinds changelog  # subset of doc kinds
pidx changelog --repo <path/to/git>             # arbitrary local repo (not in config)
pidx changelog --force --since-sha <sha>        # bypass cache, reprocess range
pidx changelog --dry-run                        # discover + map + reduce, but skip write
```

`--repo <path>` resolves first against `[[repos]]` config; if no
match, treats it as a local git path. This is the "general tool"
escape hatch — pipeline works on any repo, not just Shuozeli's.

## Output format

### `docs/<repo>/CHANGELOG.md`

[Keep a Changelog](https://keepachangelog.com) format, weekly grouping
under `## [Unreleased]`. Tagged releases freeze a week's contents:

```markdown
# Changelog

<!-- pidx-managed: do not hand-edit content below; add notes above the marker -->

## [Unreleased]

### Week of 2026-05-04

#### Added
- W3C parent-context decoder for trace continuity
- per-handler deadline auto-wrap

#### Fixed
- abandoned acquire loops on drain (closes replay-races flake)

### Week of 2026-04-27

#### Changed
- ...

## [0.3.0] - 2026-04-20
...
```

### `docs/<repo>/architecture.md`

LLM-rewritten on each run from the current state of the codebase
(reducer reads the latest classifications + a directory snapshot).
Diff stays small in steady state because the input changes slowly.

### Description

Short one-liner; written into `pidx.toml` `[[repos]] description = "…"`
override (so `pidx index` picks it up on the next render). The reducer
surfaces a proposed diff; user runs `pidx changelog --apply-descriptions`
to commit the toml change.

## Cost model

Rough back-of-napkin for MiniMax M2 pricing (assumed ~$0.30/M input,
$1.20/M output as of 2026):

- **Per-commit classify**: ~500 input + 100 output tokens = ~$0.0003
- **Weekly reduce (changelog)**: ~3000 input + 800 output = ~$0.0019
- **Architecture reduce**: ~5000 input + 1500 output = ~$0.0033
- **Description reduce**: ~1000 input + 50 output = ~$0.0004

For 25 repos × 50 commits/week:
- Map: 1250 calls × $0.0003 = **$0.38/wk**
- Reduce: 25 × 3 = 75 calls × ~$0.002 = **$0.15/wk**
- **~$0.53/week** at full coverage. Cache hits drop this near zero on re-runs.

## Phase plan

1. **Phase 0: Schema + skeleton.** New tables, trait + stub adapter, `pidx changelog --dry-run` discovers + prints commit count.
2. **Phase 1: Map.** Implement the classify path against MiniMax, cache results in `commit_classifications`. `--dry-run` prints classifications.
3. **Phase 2: Reduce changelog.** Wire the changelog reducer + render + write. Manual review.
4. **Phase 3: Reduce architecture.** Same shape, different prompt + reducer scope.
5. **Phase 4: Reduce description.** Toml override + `--apply-descriptions` flag.
6. **Phase 5: Fan-out.** `--all` parallelizes across repos, budget enforcement, retry-with-backoff on 429.
7. **Phase 6: Polish.** Doubao adapter, daily-token-limit warnings, structured logging, integration tests against a fixture repo.

## Tradeoffs we accepted

- **Per-commit classification call** (vs per-batch). Cache hit rate stays high on re-runs; cost is ~2× per first run but break-even is one re-run.
- **Diff-stat + 40 lines per file** (vs full diffs). Loses some signal on big refactors; saves ~10× tokens on the average commit.
- **Write-only** (vs auto-commit). Hallucinations stay reviewable. `git diff docs/<repo>/CHANGELOG.md` after every run.
- **Trait-based LLM abstraction** (vs hardcoded provider). MiniMax + Doubao + Anthropic all share the wire format, so abstraction cost is one trait + three configs.
- **State in pidx.db** (vs filesystem cache). Atomic commits, transactional cache invalidation when prompts change.

## Out of scope (v1)

- Auto-commit (write only; user reviews and commits)
- Cross-repo summaries (per-repo only)
- PR-level granularity (commit-level only)
- Issue/PR body generation (future `pidx issue` command)
- Provider failover chain (single provider; switch via config)
