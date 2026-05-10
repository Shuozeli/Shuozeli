# pidx Tasks

## Completed

- [x] Phase 0: Skeleton (Cargo, clap CLI, config, DB schema, migrations)
- [x] Phase 1: Dark launch (3 repos: sync, status, commit classifier)
- [x] Phase 2: Core features (activity, health scoring, release fetching, sync event log)
- [x] Phase 3: Docs pipeline (export + ingest)
- [x] Phase 4: Reporting (table + markdown, LLM insights blending)

## Pending

- [ ] Phase 5: Full launch (all 13 repos, parallel sync, pagination)
- [ ] Phase 6: Polish (incremental sync, rate limit handling, --json output)
- [ ] Truncate long LLM status in status table for better terminal display
- [ ] Add --repo filter to status and report commands
- [ ] Weekly changelog generation (hybrid pidx + agent approach) — _superseded by the LLM doc pipeline below_
  - **Output:** `docs/{repo}/changelog-{YYYY}-Q{N}.md` per tracked repo, one file per quarter, weekly entries newest-first
  - **Workflow:**
    1. `pidx sync` to refresh GitHub data
    2. `pidx docs export` to produce structured markdown per repo
    3. Agent reads pidx exports + each repo's docs (tasks.md, CHANGELOG.md, design docs) to produce a contextual weekly summary
    4. Agent writes/appends to `index/docs/{repo}/changelog-{YYYY}-Q{N}.md`
  - **Content per week:** commits with context, milestones reached, phases completed, issues opened/closed, releases. Repos with no activity get a one-liner "No updates this week."
  - **Cadence:** Weekly, covering Mon-Sun

## LLM Doc Pipeline (`pidx changelog`)

Map-reduce fanout pipeline that updates per-repo `CHANGELOG.md`,
`architecture.md`, and the description override in `pidx.toml`.
Full design in [`llm-doc-pipeline.md`](llm-doc-pipeline.md).

- [ ] **Phase 0 — Schema + skeleton.** Add `commit_classifications` and `doc_reducer_outputs` tables; `repos.last_processed_sha` column; migration. Add `LlmClient` trait + `AnthropicCompatibleClient` skeleton (no real calls). Wire `pidx changelog --dry-run --repo <name>` to discover commits in `(last_processed_sha, HEAD]` and print the count.
- [ ] **Phase 1 — Map (classify).** Implement `enrich` (git diff fetcher, capped at `[llm.classify].diff_lines_per_file`). Implement `classify_commit` against MiniMax. Cache results in `commit_classifications`. `--dry-run` prints classifications without writing docs. Bounded concurrency (`[llm].max_concurrent_requests`).
- [ ] **Phase 2 — Reduce (changelog).** Per-week reducer reads cached classifications, renders Keep-a-Changelog markdown, writes `docs/<repo>/CHANGELOG.md`. Idempotent (re-render with same inputs ⇒ byte-identical). Reducer output cached in `doc_reducer_outputs` keyed by `input_hash`.
- [ ] **Phase 3 — Reduce (architecture).** Whole-repo reducer reads classifications + a directory snapshot (top-level files + first heading of each `.md`). Writes `docs/<repo>/architecture.md`. Cache invalidation same as changelog.
- [ ] **Phase 4 — Reduce (description).** One-line summary; writes a proposed diff into `pidx.toml`'s `[[repos]] description` field. `--apply-descriptions` flag commits the toml edit (still no git commit).
- [ ] **Phase 5 — Fanout.** `--all` parallelizes across configured repos (default 2 concurrent); `--since 7d` skips repos with no commits in window. Daily token budget enforcement with warn-at-pct logging. Retry-with-backoff on 429.
- [ ] **Phase 6 — Polish.** Doubao adapter (different `base_url`, same trait). Structured JSON logging. Integration tests against a fixture git repo with N synthetic commits. `--repo <path>` resolution: configured name first, then arbitrary local git path.

### Design tradeoffs (locked)

- Per-commit classification call (vs per-batch) — cache hit rate first
- Diff-stat + 40 lines per file (vs full diffs) — token budget
- Write-only (vs auto-commit) — hallucinations stay reviewable
- Trait-based LLM abstraction (vs hardcoded provider) — MiniMax + Doubao + Anthropic share wire format
- State in pidx.db (vs filesystem cache) — atomic invalidation when prompts change

### Out of scope (v1)

- Auto-commit
- Cross-repo summaries
- PR-level granularity
- Issue/PR body generation
- Provider failover chain
