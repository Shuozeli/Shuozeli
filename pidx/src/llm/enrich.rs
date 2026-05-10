//! Enrich a commit with the per-file diff data needed to feed the
//! classification prompt.
//!
//! Phase 1 reads from the local submodule clone at
//! `~/projects/Shuozeli/docs/<repo>/`. We never shell out to `gh` and
//! never clone — if the submodule is missing the caller sees a
//! [`EnrichError::CheckoutMissing`] with the exact `git submodule
//! update` command they need to run.

use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

/// Per-file diff included in [`EnrichedCommit`]. Capped at the
/// `[llm.classify].diff_lines_per_file` limit; longer hunks are
/// truncated with a `... <N more lines>` marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    /// `'A'` added, `'M'` modified, `'D'` deleted, `'R'` renamed.
    pub status: char,
    pub patch: String,
}

/// Output of [`enrich_commit`]: everything the classifier needs from
/// a single commit, with diffs already capped.
#[derive(Debug, Clone)]
pub struct EnrichedCommit {
    pub sha: String,
    /// Commit subject line (first line of the message).
    pub subject: String,
    /// Commit body (everything after the subject + blank line). `None`
    /// if the commit has only a subject.
    pub body: Option<String>,
    pub author: String,
    /// Unix seconds.
    pub timestamp: i64,
    pub diffs: Vec<FileDiff>,
}

/// Errors specific to the enrich step. Distinct from [`super::LlmError`]
/// because the failure modes are local-git specific (checkout layout,
/// `git` invocation), not provider-side.
#[derive(Debug, Error)]
pub enum EnrichError {
    /// The submodule at `docs/<repo>/` doesn't exist or has no `.git`.
    /// Carries the actionable command the user needs to run.
    #[error(
        "checkout missing for repo '{repo}' at {path}. Run \
         `git submodule update --init --recursive` from the Shuozeli \
         superproject root to populate it."
    )]
    CheckoutMissing { repo: String, path: PathBuf },

    /// `git` command itself failed (binary missing, repo corrupt, sha
    /// not present in the local clone, etc.).
    #[error("git invocation failed for sha {sha}: {message}")]
    GitFailed { sha: String, message: String },

    /// I/O error spawning `git` or reading its output.
    #[error("io error running git: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolve the on-disk path for a Shuozeli submodule clone.
///
/// Layout is fixed: `~/projects/Shuozeli/docs/<repo>/`. We don't honor
/// env vars or alternate roots in Phase 1; the submodule layout is
/// part of the project contract.
pub fn checkout_path(repo: &str) -> PathBuf {
    let home =
        std::env::var("HOME").unwrap_or_else(|_| "/home/cyuan".to_string());
    PathBuf::from(home)
        .join("projects/Shuozeli/docs")
        .join(repo)
}

/// Verify that `path` is a populated git checkout (has a `.git` entry,
/// either directory or gitlink file). Returns
/// [`EnrichError::CheckoutMissing`] otherwise.
fn ensure_checkout(repo: &str, path: &Path) -> Result<(), EnrichError> {
    if !path.join(".git").exists() {
        return Err(EnrichError::CheckoutMissing {
            repo: repo.to_string(),
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

/// Run `git` with the supplied args inside `repo_path`. Returns stdout
/// as a UTF-8 string. Non-UTF-8 bytes are replaced (defensive — patch
/// content can contain anything, but we're feeding the LLM text not
/// bytes).
fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, EnrichError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(EnrichError::GitFailed {
            sha: args.last().unwrap_or(&"<unknown>").to_string(),
            message: stderr.trim().to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Cap a multi-line patch at `max_lines` lines, appending a
/// `... <N more lines>` marker when truncated.
///
/// `max_lines == 0` disables capping (returns input unchanged); we
/// never encounter that in practice (config default is 40) but being
/// explicit avoids a divide-by-zero or surprising "everything is one
/// line" behavior.
pub fn truncate_patch(patch: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return patch.to_string();
    }
    let total = patch.lines().count();
    if total <= max_lines {
        return patch.to_string();
    }
    let kept: Vec<&str> = patch.lines().take(max_lines).collect();
    let dropped = total - max_lines;
    let mut out = kept.join("\n");
    out.push_str(&format!("\n... <{dropped} more lines>"));
    out
}

/// Parse the `--name-status -z` output from `git show` into a list of
/// `(status_char, path)` pairs. Format is NUL-separated:
///
/// - For most statuses: `STATUS\0PATH\0`
/// - For renames: `R<score>\0OLD_PATH\0NEW_PATH\0`
fn parse_name_status_z(raw: &str) -> Vec<(char, String)> {
    // The leading newline that `git show --format=` emits before the
    // file list isn't part of the NUL stream, so we trim it first.
    let stream = raw.trim_start_matches(['\n', '\r']);
    let mut out = Vec::new();
    let mut iter = stream.split('\0').filter(|s| !s.is_empty());
    while let Some(status_field) = iter.next() {
        let status_char = status_field
            .chars()
            .next()
            .unwrap_or('M')
            .to_ascii_uppercase();
        let Some(path) = iter.next() else { break };
        if status_char == 'R' || status_char == 'C' {
            // Renames carry old + new path. Use the new path for the
            // diff; classifier doesn't need the old name.
            let Some(new_path) = iter.next() else { break };
            out.push((status_char, new_path.to_string()));
            // unused; keep `path` swallowed.
            let _ = path;
        } else {
            out.push((status_char, path.to_string()));
        }
    }
    out
}

/// Fetch commit metadata + per-file diffs from a local clone.
///
/// `diff_lines_per_file` matches the `[llm.classify].diff_lines_per_file`
/// config knob. Each file's patch is independently capped.
pub fn enrich_commit(
    repo_name: &str,
    sha: &str,
    diff_lines_per_file: usize,
) -> Result<EnrichedCommit, EnrichError> {
    let path = checkout_path(repo_name);
    ensure_checkout(repo_name, &path)?;

    // Metadata: subject\nbody\n<unix-ts>\n<author> — `%x1f` (Unit
    // Separator) keeps fields parseable without the LLM-y "%n" risk.
    // Using `git show -s` for headers only.
    let meta_format = "%s%x1f%b%x1f%at%x1f%an";
    let meta = run_git(
        &path,
        &["show", "-s", &format!("--format={meta_format}"), sha],
    )?;
    let meta = meta.trim_end_matches('\n');
    let mut fields = meta.splitn(4, '\x1f');
    let subject = fields.next().unwrap_or("").to_string();
    let body_raw = fields.next().unwrap_or("").to_string();
    let timestamp: i64 = fields
        .next()
        .unwrap_or("0")
        .trim()
        .parse()
        .map_err(|_| EnrichError::GitFailed {
            sha: sha.to_string(),
            message: "could not parse author timestamp from git show".into(),
        })?;
    let author = fields.next().unwrap_or("").to_string();
    let body = if body_raw.trim().is_empty() {
        None
    } else {
        Some(body_raw.trim_end_matches('\n').to_string())
    };

    // File list: `--name-status -z` for unambiguous parsing of paths
    // with spaces / newlines.
    let names = run_git(
        &path,
        &[
            "show",
            "--format=",
            "--name-status",
            "--diff-filter=ACMRD",
            "-z",
            sha,
        ],
    )?;
    let entries = parse_name_status_z(&names);

    // Per-file patches. We invoke `git show ... -- <path>` once per file
    // so the diff is scoped and we can independently truncate. This is
    // O(N) processes per commit, which is fine for the commit volumes
    // we care about (50 commits × ~5 files = 250 git invocations,
    // sub-second on local disk).
    let mut diffs = Vec::with_capacity(entries.len());
    for (status, path_str) in entries {
        let raw = run_git(
            &path,
            &[
                "show",
                "--format=",
                "--diff-filter=ACMRD",
                sha,
                "--",
                &path_str,
            ],
        )?;
        let patch = truncate_patch(raw.trim_start_matches('\n'), diff_lines_per_file);
        diffs.push(FileDiff {
            path: path_str,
            status,
            patch,
        });
    }

    Ok(EnrichedCommit {
        sha: sha.to_string(),
        subject,
        body,
        author,
        timestamp,
        diffs,
    })
}

/// Render an [`EnrichedCommit`] into the text form fed to the LLM.
/// Kept here (not in `client.rs`) so tests can pin the format.
pub fn render_for_prompt(c: &EnrichedCommit) -> String {
    let mut out = String::new();
    out.push_str(&format!("Commit: {}\n", c.sha));
    out.push_str(&format!("Author: {}\n", c.author));
    out.push_str(&format!("Subject: {}\n", c.subject));
    if let Some(body) = &c.body {
        out.push_str("Body:\n");
        out.push_str(body);
        out.push('\n');
    }
    out.push_str("\nFiles changed:\n");
    for d in &c.diffs {
        out.push_str(&format!("- [{}] {}\n", d.status, d.path));
    }
    out.push_str("\nDiffs (per-file, capped):\n");
    for d in &c.diffs {
        out.push_str(&format!("\n--- {} ({}) ---\n", d.path, d.status));
        out.push_str(&d.patch);
        if !d.patch.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_patch_keeps_short_input_unchanged() {
        // Arrange
        let patch = "line1\nline2\nline3";

        // Act
        let out = truncate_patch(patch, 40);

        // Assert
        assert_eq!(out, patch);
    }

    #[test]
    fn truncate_patch_caps_at_limit_and_appends_marker() {
        // Arrange — 5 lines total, cap at 2.
        let patch = "a\nb\nc\nd\ne";

        // Act
        let out = truncate_patch(patch, 2);

        // Assert — first two kept, marker reports remaining 3.
        assert_eq!(out, "a\nb\n... <3 more lines>");
    }

    #[test]
    fn truncate_patch_at_exact_boundary_does_not_truncate() {
        // Arrange — 3 lines, cap at 3.
        let patch = "a\nb\nc";

        // Act
        let out = truncate_patch(patch, 3);

        // Assert — no marker.
        assert_eq!(out, "a\nb\nc");
    }

    #[test]
    fn parse_name_status_z_handles_modified_and_added() {
        // Arrange — synthetic NUL-separated output from
        // `git show --name-status -z`.
        let raw = "M\0src/main.rs\0A\0src/new.rs\0";

        // Act
        let parsed = parse_name_status_z(raw);

        // Assert
        assert_eq!(
            parsed,
            vec![('M', "src/main.rs".into()), ('A', "src/new.rs".into())]
        );
    }

    #[test]
    fn parse_name_status_z_handles_renames() {
        // Arrange — rename carries score + old + new path.
        let raw = "R100\0old/path.rs\0new/path.rs\0M\0src/lib.rs\0";

        // Act
        let parsed = parse_name_status_z(raw);

        // Assert — rename surfaces the new path; modify follows.
        assert_eq!(
            parsed,
            vec![('R', "new/path.rs".into()), ('M', "src/lib.rs".into())]
        );
    }

    #[test]
    fn checkout_path_is_under_shuozeli_docs() {
        // Arrange — pin to a known repo name.
        let repo = "taskq-rs";

        // Act
        let path = checkout_path(repo);

        // Assert
        assert!(path.ends_with("projects/Shuozeli/docs/taskq-rs"));
    }

    #[test]
    fn ensure_checkout_errors_when_dot_git_missing() {
        // Arrange — temp dir with no .git.
        let tmp = std::env::temp_dir().join(format!(
            "pidx-test-no-git-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        // Act
        let result = ensure_checkout("phantom-repo", &tmp);

        // Assert
        match result {
            Err(EnrichError::CheckoutMissing { repo, .. }) => {
                assert_eq!(repo, "phantom-repo");
            }
            other => panic!("expected CheckoutMissing, got {other:?}"),
        }

        std::fs::remove_dir_all(&tmp).ok();
    }
}
