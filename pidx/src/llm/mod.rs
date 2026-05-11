//! LLM client abstraction for the doc pipeline.
//!
//! Phase 1 wires the classify path against MiniMax. Reducer methods
//! still return [`LlmError::NotImplemented`] until Phase 2+.

// Reducer types are re-exported now so callers (changelog command,
// future fanout driver) can compile against the final shapes; the
// `unused_imports` allow keeps the re-exports from warning until
// reducers land.
#![allow(dead_code, unused_imports)]

mod client;
pub mod enrich;
mod error;
pub mod snapshot;
mod types;

pub use client::AnthropicCompatibleClient;
pub use enrich::{EnrichError, EnrichedCommit, FileDiff, enrich_commit, render_for_prompt};
pub use error::LlmError;
pub use snapshot::{DirEntry, DirKind, MarkdownHeading, RepoSnapshot, RootFileEntry};
pub use types::{
    ArchitectureClassificationContext, Classification, ClassifyRequest, CommitCategory,
    CommitImpact, ReduceArchitectureRequest, ReduceChangelogRequest,
    ReduceChangelogWeekClassification, ReduceDescriptionRequest,
};

use std::future::Future;
use std::pin::Pin;

/// Version of the `classify_commit` system prompt + JSON schema.
///
/// **Bump this whenever the prompt text or the response schema
/// changes.** The `commit_classifications` cache is keyed on
/// `(repo_id, sha, prompt_version)`, so a bump cleanly invalidates
/// every existing row on the next run. Forgetting to bump means the
/// cache will serve stale rows under the new prompt — which is exactly
/// the bug this constant exists to prevent.
pub const CLASSIFY_PROMPT_VERSION: u32 = 1;

/// Version of the `reduce_changelog` system prompt + output format.
///
/// **Bump this whenever the reducer prompt text or expected markdown
/// shape changes.** The `doc_reducer_outputs.input_hash` mixes this
/// constant in alongside the sorted classification SHAs, so a bump
/// invalidates every cached weekly fragment on the next run. Without
/// the bump, the reducer would happily serve stale prose under the new
/// prompt.
pub const REDUCE_CHANGELOG_PROMPT_VERSION: u32 = 1;

/// Version of the `reduce_architecture` system prompt + output format.
///
/// **Bump this whenever the architecture reducer prompt text, snapshot
/// shape, or expected markdown structure changes.** The
/// `doc_reducer_outputs.input_hash` for `kind="architecture"` mixes
/// this constant in alongside the sorted classification SHAs and the
/// snapshot hash, so a bump invalidates every cached architecture
/// document on the next run. Without the bump, the reducer would serve
/// stale prose under the new prompt.
pub const REDUCE_ARCHITECTURE_PROMPT_VERSION: u32 = 1;

/// Maximum number of recent classifications passed to the architecture
/// reducer. Architecture wants enough recent context to understand the
/// current shape; older history is noise and bloats the token budget.
pub const ARCHITECTURE_CLASSIFICATION_LIMIT: usize = 200;

/// Result alias for trait methods. Boxed so the trait stays object-safe
/// without depending on `async_trait` (we don't need dyn dispatch in
/// Phase 0, but we want the same shape it would take when we do).
pub type LlmFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, LlmError>> + Send + 'a>>;

/// Abstraction over LLM providers that speak the Anthropic Messages
/// wire format (MiniMax, Doubao, Anthropic itself).
pub trait LlmClient: Send + Sync {
    fn classify_commit<'a>(
        &'a self,
        req: ClassifyRequest,
    ) -> LlmFuture<'a, Classification>;

    fn reduce_changelog<'a>(
        &'a self,
        req: ReduceChangelogRequest,
    ) -> LlmFuture<'a, String>;

    fn reduce_architecture<'a>(
        &'a self,
        req: ReduceArchitectureRequest,
    ) -> LlmFuture<'a, String>;

    fn reduce_description<'a>(
        &'a self,
        req: ReduceDescriptionRequest,
    ) -> LlmFuture<'a, String>;
}
