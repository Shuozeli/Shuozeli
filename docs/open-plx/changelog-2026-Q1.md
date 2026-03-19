# open-plx Changelog -- 2026 Q1

## Week 2026-W12 (Mar 16 - Mar 22)

All six v1 phases were completed this week, taking open-plx from an empty
repository to a fully working server-driven dashboard platform with a
verified end-to-end stack. This matters because the two-phase rendering
protocol, Flight SQL data path, dashboard variables, auth, observability,
and dark mode are all now in production-ready shape.

### Highlights

**Foundation (Phase 0)**
- `0cc8df1` -- Initial project setup establishing the Rust crate structure
  (`open-plx-core`, `open-plx-config`, `open-plx-auth`, `open-plx-server`),
  proto-first layout, and frontend scaffolding.

**Arrow Flight data path and widget rendering (Phase 1)**
- `e6ab4b3` -- Arrow Flight service for static data with tonic 0.14 upgrade,
  establishing the gRPC-native data transport layer.
- `f16958b` -- `WidgetDataService` and frontend data-fetching wiring the
  two-phase render protocol (layout push, per-widget data pull).
- `fb88dea` -- G2 chart mapper and widget rendering, completing the semantic
  proto spec -> G2 translation layer for all chart types.

**Flight SQL, variables, auth, observability, and dark mode (Phases 2-5)**
- `70e69bc` -- Flight SQL client integration (via `arrow-adbc-rs`), dashboard
  variables with topological resolution, pluggable auth (`AuthProvider` trait
  with dev-mode and API-key providers), structured event-log observability,
  and dark mode theme support, completing all remaining v1 scope in a single
  compound commit.

**Code quality**
- `8b6a86c` -- Resolved all clippy and rustdoc warnings to keep the workspace
  clean for CI.

**End-to-end test suite**
- `ba6fbaa` -- Initial E2E smoke tests confirming full-stack operation.
- `c96e229` -- 42 tests across 8 suites covering core flows.
- `e530caa` -- State-based E2E framework with 54 tests and no screenshot
  dependencies, improving suite determinism.
- `4638e9e` -- 96 E2E tests providing full widget type coverage, navigation,
  and error state verification.

### Issues
- Opened: None
- Closed: None

### Releases
None

---
