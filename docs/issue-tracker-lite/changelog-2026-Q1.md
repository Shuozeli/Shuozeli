# issue-tracker-lite Changelog -- 2026 Q1

## Week 2026-W12 (Mar 16 - Mar 22)

This week established the foundation of the issue-tracker-lite repository, covering the initial commit, full CI setup, documentation, and a focused round of security hardening on ACL and hotlist endpoints that closes impersonation and privilege-escalation vectors.

### Highlights

#### Project Bootstrap
- `381462c` -- Initial commit: Issue Tracker Lite, establishing the core project structure.
- `4f3177d` -- Added usage guide, codelabs, API reference, tasks, and changelog documentation.

#### CI and Code Quality
- `57a1550` -- Added CI pipeline, pre-commit hooks, and resolved all outstanding clippy warnings.
- `7b47b0c` -- Fixed CI: installed `protobuf-compiler` required for proto codegen.
- `7ca515f` -- Refactored shared test helpers into a dedicated `test-utils` crate to reduce duplication.

#### Security Hardening
- `e11cfe1` -- Added authentication enforcement to ACL and hotlist endpoints, preventing unauthenticated access and identity impersonation.

#### Documentation / Bug Fixes
- `d629f91` -- Escaped regex pattern in a doc comment to fix a broken rustdoc link.

### Issues
- Opened: None
- Closed: None

### Releases
None

---
