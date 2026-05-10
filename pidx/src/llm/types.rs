//! Request/response shapes for [`crate::llm::LlmClient`].
//!
//! These mirror the design doc verbatim; serde derives are present so
//! that Phase 1+ can serialize/deserialize them straight into provider
//! payloads without an intermediate struct.

use serde::{Deserialize, Serialize};

/// Categories used in the changelog reducer (Keep-a-Changelog flavored).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CommitCategory {
    Added,
    Changed,
    Fixed,
    Removed,
    Internal,
}

/// SemVer-flavored impact estimate for a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommitImpact {
    Minor,
    Major,
    Breaking,
}

/// Inputs for a single per-commit classification call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyRequest {
    pub repo_name: String,
    pub sha: String,
    pub commit_subject: String,
    pub commit_body: String,
    /// `git show --stat --diff-filter=ACDMR -p` output, capped at
    /// `[llm.classify].diff_lines_per_file` per file.
    pub diff_excerpt: String,
    /// Bumped when the classification prompt changes — invalidates the
    /// `(repo_id, sha, prompt_version)` cache.
    pub prompt_version: u32,
}

/// Output of a single classification call. Persisted in the
/// `commit_classifications` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    pub category: CommitCategory,
    /// One-line, present-tense imperative summary.
    pub summary: String,
    pub impact: CommitImpact,
}

/// Inputs for the per-week (or per-tag) changelog reducer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReduceChangelogRequest {
    pub repo_name: String,
    /// e.g. `"week-2026-W18"` or `"v0.3.0"`.
    pub scope_key: String,
    pub classifications: Vec<Classification>,
    pub prompt_version: u32,
}

/// Inputs for the whole-repo architecture reducer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReduceArchitectureRequest {
    pub repo_name: String,
    /// Recent classifications, oldest-first, used to bias the rewrite
    /// toward what changed lately.
    pub classifications: Vec<Classification>,
    /// Top-level files + first heading of each `.md` — see design doc.
    pub directory_snapshot: String,
    pub prompt_version: u32,
}

/// Inputs for the one-line description reducer (writes back into
/// `pidx.toml` `[[repos]] description` override).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReduceDescriptionRequest {
    pub repo_name: String,
    pub classifications: Vec<Classification>,
    pub directory_snapshot: String,
    pub prompt_version: u32,
}
