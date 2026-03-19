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
- [ ] Weekly changelog generation (hybrid pidx + agent approach)
  - **Output:** `docs/{repo}/changelog-{YYYY}-Q{N}.md` per tracked repo, one file per quarter, weekly entries newest-first
  - **Workflow:**
    1. `pidx sync` to refresh GitHub data
    2. `pidx docs export` to produce structured markdown per repo
    3. Agent reads pidx exports + each repo's docs (tasks.md, CHANGELOG.md, design docs) to produce a contextual weekly summary
    4. Agent writes/appends to `index/docs/{repo}/changelog-{YYYY}-Q{N}.md`
  - **Content per week:** commits with context, milestones reached, phases completed, issues opened/closed, releases. Repos with no activity get a one-liner "No updates this week."
  - **Cadence:** Weekly, covering Mon-Sun
