# beu Changelog -- 2026 Q1

## Week 2026-W12 (Mar 16 - Mar 22)

Single bugfix: `beu init` now defaults to Claude Code and `.agents`-only skill
installation rather than installing rules for all supported agent runtimes.

### Highlights

**`beu init` default scope narrowed** (`64ea9b0`)

Previously `beu init` would install skill rules into all agent rule directories
it could find. The fix restricts the default target to Claude Code and the
`.agents` directory, reducing noise for projects that only use one agent runtime.

### Issues

- Opened: None
- Closed: None

### Releases

None

---
