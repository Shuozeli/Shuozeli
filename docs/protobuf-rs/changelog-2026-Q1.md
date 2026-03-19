# protobuf-rs Changelog -- 2026 Q1

## Week 2026-W12 (Mar 16 - Mar 22)

This week introduced optional gRPC service code generation and completed a
code-quality sweep that resolved most of the open findings tracked in
`docs/code-quality-findings.md`.

### Highlights

**Optional gRPC service generation** (`1a337b2`)

A new `grpc` Cargo feature gates gRPC service stub generation. When enabled,
the codegen pipeline emits server and client trait/impl skeletons alongside the
existing message types. This mirrors the `feature = "grpc"` pattern used in the
rest of the stack and keeps the default build free of gRPC dependencies.

**Code-quality cleanup** (`cd0eefc`, `ee5bac5`)

Two refactor commits addressed previously tracked findings: duplicate logic
was deduplicated, boilerplate reduced, and all outstanding `cargo clippy`
warnings resolved. The `rust_field_type` deduplication and shared `test-utils`
helpers noted in `docs/code-quality-findings.md` are among the items now
confirmed resolved.

**Formatting** (`2168f32`)

`rustfmt` applied workspace-wide.

### Issues

- Opened: None
- Closed: None

### Releases

None

---
