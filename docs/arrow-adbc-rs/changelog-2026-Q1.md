# arrow-adbc-rs Changelog -- 2026 Q1

## Week 2026-W12 (Mar 16 - Mar 22)

This week focused on completing FlightSQL catalog support, fixing bound-parameter
propagation across all three non-SQLite drivers, and a formatting pass.

### Highlights

**FlightSQL and cross-driver bound-parameter fixes** (`13e3554`, `5085900`)

The FlightSQL driver gained `get_table_schema` support (fetching the IPC schema
from the server). Bound parameters — previously only wired up in the SQLite driver
— are now correctly forwarded to `execute` in the PostgreSQL, MySQL, and FlightSQL
drivers. This closes a behavioral gap where prepared statements with parameters
would silently ignore the bound values on non-SQLite backends.

Stale documentation and minor code-quality issues were also cleaned up as part of
the same commit (`13e3554`).

**Code formatting** (`e615169`)

`cargo fmt` applied workspace-wide; no logic changes.

### Issues

- Opened: None
- Closed: None

### Releases

None

---
