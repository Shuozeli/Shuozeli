//! LLM client abstraction for the doc pipeline.
//!
//! Phase 0 ships only the trait, request/response shapes, and a
//! provider-agnostic skeleton client. Every method returns
//! [`LlmError::NotImplemented`] until Phase 1 wires the wire-format
//! adapter.

// The trait, request/response types, and error variants are wired in
// Phase 1+. The skeleton is exported now so callers (changelog command,
// future fanout driver) can compile against the final shapes; the
// `unused_imports` allow keeps the re-exports from warning until
// Phase 1 wires them in.
#![allow(dead_code, unused_imports)]

mod client;
mod error;
mod types;

pub use client::AnthropicCompatibleClient;
pub use error::LlmError;
pub use types::{
    Classification, ClassifyRequest, CommitCategory, CommitImpact,
    ReduceArchitectureRequest, ReduceChangelogRequest, ReduceDescriptionRequest,
};

use std::future::Future;
use std::pin::Pin;

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
