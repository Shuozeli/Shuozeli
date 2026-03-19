# flatbuffers-rs Changelog -- 2026 Q1

## Week 2026-W12 (Mar 16 - Mar 22)

This week advanced code quality and internal architecture, completing a comprehensive
audit remediation pass and landing the gRPC service generation feature behind a compile-time
feature flag -- both of which move the project toward broader production adoption.

### Highlights

**Code quality audit and remediation**

A thorough audit pass resolved 17 distinct code quality issues across the codebase
(`3844642`). Separately, Clippy and rustfmt CI failures that had been blocking clean
builds were fixed (`8912459`).

**gRPC service stub generation (Phase F2)**

Optional gRPC service stub generation was added, compiled in via `--features grpc`
on the `flatc-rs-codegen` crate (`bd3a9fc`). This generates server traits and client
stubs from `rpc_service` declarations in `.fbs` schemas, completing ROADMAP item F2.

**Schema type separation**

Parsed and resolved schema types were separated into distinct representations
(`8791685`), clarifying the boundary between the parser and semantic analyzer stages
and reducing implicit coupling in the internal data model.

**Documentation alignment**

Stale documentation was updated to match the current state of the codebase (`570d306`),
keeping in sync with the significant audit and feature work completed since the last
doc pass.

### Issues

- Opened: None
- Closed: None

### Releases

None

---
