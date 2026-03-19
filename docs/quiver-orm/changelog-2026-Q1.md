# quiver-orm Changelog -- 2026 Q1

## Week 2026-W12 (Mar 16 - Mar 22)

This week delivered a driver-layer refactor that reduced duplication across
driver crates, switched async return types to `BoxFuture`, and expanded
PostgreSQL and MySQL test coverage.

### Highlights

**Driver deduplication and BoxFuture migration** (`bd4ccb6`, `38549bb`)

The driver crates (`quiver-driver-sqlite`, `quiver-driver-postgres`,
`quiver-driver-mysql`) were refactored to eliminate copy-pasted codegen
patterns. Async trait methods that previously returned `impl Future` or
`Pin<Box<dyn Future>>` were unified to `BoxFuture` for consistency with the
rest of the stack. PostgreSQL and MySQL integration tests were added in
the same pass, covering the core transaction and query paths that had
previously only been exercised by the SQLite driver.

**Documentation fix** (`1da4767`)

A broken `PoolGuard` rustdoc link in the SQLite pool module was corrected.

### Issues

- Opened: None
- Closed: None

### Releases

None

---
