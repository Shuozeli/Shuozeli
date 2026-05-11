//! Request/response shapes for [`crate::llm::LlmClient`].
//!
//! These mirror the design doc verbatim; serde derives are present so
//! that Phase 1+ can serialize/deserialize them straight into provider
//! payloads without an intermediate struct.

use serde::{Deserialize, Serialize};

use super::snapshot::RepoSnapshot;

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

/// One classification entry for the reducer's per-week input. Carries
/// the short SHA so the prompt can list `<sha7> <summary>` per bullet
/// and the LLM has a stable trace back to the source commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReduceChangelogWeekClassification {
    pub sha: String,
    pub category: CommitCategory,
    pub summary: String,
    pub impact: CommitImpact,
}

/// Inputs for the per-week (or per-tag) changelog reducer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReduceChangelogRequest {
    pub repo_name: String,
    /// e.g. `"week-2026-W18"` or `"v0.3.0"`.
    pub scope_key: String,
    /// Human-readable date label for the week (e.g. `"2026-04-27"`,
    /// the Monday of the ISO week). The reducer prompt embeds this in
    /// the `### Week of <date>` heading it emits.
    pub week_label: String,
    /// ISO date for the Monday that starts this week (YYYY-MM-DD).
    pub week_start: String,
    /// ISO date for the Sunday that ends this week (YYYY-MM-DD).
    pub week_end: String,
    pub classifications: Vec<ReduceChangelogWeekClassification>,
    pub prompt_version: u32,
}

/// One classification entry passed to the architecture reducer.
/// Carries enough context for the model to spot recurring themes
/// without needing the full diffs (which would blow the token budget).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureClassificationContext {
    pub sha: String,
    /// "Added" / "Changed" / "Fixed" / "Removed" / "Internal".
    pub category: String,
    pub summary: String,
}

/// Inputs for the whole-repo architecture reducer.
///
/// Phase 3 wire shape: pidx hands the model a structured snapshot of
/// the repo's current shape PLUS the most-recent `N` classifications
/// (sorted by commit date desc, capped by `ARCHITECTURE_CLASSIFICATION_LIMIT`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReduceArchitectureRequest {
    pub repo_name: String,
    /// One-line description if known (e.g. from `pidx.toml` override
    /// or a README's first paragraph). `None` when pidx has no
    /// description for this repo.
    pub repo_description: Option<String>,
    pub snapshot: RepoSnapshot,
    /// Most-recent classifications, oldest-first within the truncated
    /// window. The reducer prompt instructs the model to mine these
    /// for "Notable Design Decisions".
    pub recent_classifications: Vec<ArchitectureClassificationContext>,
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
