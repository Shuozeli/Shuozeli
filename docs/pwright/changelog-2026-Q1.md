# pwright Changelog -- 2026 Q1

## Week 2026-W12 (Mar 16 - Mar 22)

This week focused on two parallel tracks: expanding CLI surface with new commands and integration tests, and a systematic code quality audit that eliminated stringly-typed APIs, deduplication issues, and fail-fast violations inherited from AI-generated code.

### Highlights

**New CLI Commands and Integration Tests**

Four missing interaction commands were added -- `check`, `uncheck`, `scroll`, and `text` -- alongside integration test coverage (`be31c10`, `0dd832f`). Positional pointer commands `click-at`, `hover-at`, and `dblclick` were also introduced with codegen and integration tests (`03b3de9`).

**Network Capture**

A new network capture subsystem landed with three CLI commands (`network-listen`, `network-list`, `network-get`), a design doc, integration tests with a CI check rule, and recipe examples (`7157788`, `cbd5d08`, `c22c39c`, `ca23855`).

**CDP Codegen and Protocol Typing**

Typed domain params via CDP protocol codegen were added, along with a `FromEvalResult` trait and a chrome-devtools-mcp feature comparison (`448e430`, `c73702c`).

**Code Quality Audit and Tech Debt Reduction**

A `tech-debt.md` catalog was introduced (`4f0f201`) and systematically worked down: 21 items resolved by end of week (`d8fbf98`). Stringly-typed APIs were replaced with enums (`feca92a`, `d8150dc`), fail-fast error handling was enforced (`7758300`), the `SelectorKind` enum and delegation macro were added (`3c3dec1`), and a code quality discipline rule was codified in `CLAUDE.md` (`573fda1`).

**Documentation Cleanup**

Stale doc counts (domain, action, RPC) were corrected and `todo.md` was reorganized to reflect completed work (`a49539a`, `bab5749`, `da628e1`).

### Issues
- Opened: None
- Closed: None

### Releases
None

---
