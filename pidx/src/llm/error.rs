use std::time::Duration;

use thiserror::Error;

/// Errors surfaced by [`crate::llm::LlmClient`] implementations.
///
/// Phase 0 only needs `NotImplemented`; the rest of the variants are
/// pre-declared so that Phase 1+ doesn't churn the public surface when
/// real provider calls land.
#[derive(Debug, Error)]
pub enum LlmError {
    /// The trait method has no implementation in this build (Phase 0
    /// skeleton). Callers should treat this as a programmer error,
    /// not a recoverable runtime condition.
    #[error("LLM operation not implemented (Phase 0 skeleton)")]
    NotImplemented,

    /// Transport-level failure (connect, TLS, body read, decode).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// 401 / 403 / missing api key env var. The string carries a short
    /// human-readable reason; we deliberately do not leak the key.
    #[error("authentication failure: {0}")]
    Auth(String),

    /// 429 from the provider. `retry_after` mirrors the `Retry-After`
    /// header when present; `None` means the provider didn't tell us.
    #[error("rate limited (retry after {retry_after:?})")]
    RateLimit { retry_after: Option<Duration> },

    /// Catch-all for anything that doesn't fit the above. Use sparingly;
    /// add a typed variant if you find yourself reaching for this in
    /// hot paths.
    #[error("LLM error: {0}")]
    Other(#[from] anyhow::Error),
}
