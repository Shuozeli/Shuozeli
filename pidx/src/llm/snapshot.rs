//! Build a structured snapshot of a repo's working tree for the
//! architecture reducer.
//!
//! The snapshot is a STRUCTURED INPUT to the LLM — not the model's job
//! to figure out the layout. We hand it: top-level files (with sizes),
//! top-level directories (with file counts and a kind hint), and the
//! first H1/H2 of every Markdown file (capped at 100, sorted by path).
//!
//! Idempotent: same submodule state ⇒ identical [`RepoSnapshot`]
//! (verified by a unit test that diffs `serde_json::to_string` of two
//! consecutive snapshots).

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Hard cap on the number of markdown headings included in the
/// snapshot. The reducer doesn't need every README in a 1000-file repo,
/// and bigger inputs blow the token budget.
pub const MARKDOWN_HEADING_CAP: usize = 100;

/// File names we exclude from the snapshot because pidx writes them
/// itself (and we don't want a feedback loop where re-running the
/// reducer changes the snapshot hash and busts the cache forever).
const PIDX_MANAGED_FILES: &[&str] = &["architecture.md", "CHANGELOG.md"];

/// Top-level directory names we never descend into. Keeps the snapshot
/// fast and avoids ballooning the file count for vendor / build dirs.
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".claude",
    "target",
    "node_modules",
    "dist",
    "build",
    "__pycache__",
    ".venv",
    "venv",
    ".pytest_cache",
    ".mypy_cache",
    ".idea",
    ".vscode",
    ".gradle",
    ".cache",
];

/// Coarse classification of a top-level directory based on its name.
/// Heuristics are intentionally name-only — we don't peek at file
/// contents because the reducer prompt instructs the model to use the
/// kind as a hint, not a contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum DirKind {
    Source,
    Tests,
    Docs,
    Examples,
    Config,
    Build,
    Other,
}

impl DirKind {
    fn classify(name: &str) -> Self {
        match name {
            "src" | "crates" | "lib" | "libs" | "packages" | "app" | "apps" => Self::Source,
            "tests" | "test" | "it" | "integration" | "e2e" | "spec" => Self::Tests,
            "docs" | "doc" | "book" | "guide" | "guides" => Self::Docs,
            "examples" | "example" | "samples" | "sample" | "demo" | "demos" => {
                Self::Examples
            }
            "config" | "etc" | "configs" | "conf" => Self::Config,
            "target" | "build" | "dist" | "out" | "bin" => Self::Build,
            _ => Self::Other,
        }
    }
}

/// One top-level file (depth=1) listed in the snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootFileEntry {
    pub path: String,
    pub size_bytes: u64,
}

/// One top-level directory listed in the snapshot. `file_count` is a
/// recursive count (skips ignored dirs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    pub path: String,
    pub file_count: usize,
    pub kind: DirKind,
}

/// First non-empty H1 or H2 of a markdown file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownHeading {
    pub path: String,
    pub heading: String,
}

/// Structured snapshot fed to the architecture reducer.
///
/// `serde_json::to_string` of this struct produces deterministic bytes
/// because we sort each `Vec` before assembly — that's the property the
/// `input_hash` relies on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSnapshot {
    pub root_files: Vec<RootFileEntry>,
    pub directories: Vec<DirEntry>,
    pub markdown_headings: Vec<MarkdownHeading>,
}

impl RepoSnapshot {
    /// Build a snapshot from the working tree at `root`.
    ///
    /// Errors only on I/O failures reading the root itself. Per-file
    /// I/O failures (e.g. heading extraction on a binary file
    /// mislabeled `.md`) are silently skipped — losing one heading
    /// entry is preferable to failing the whole reducer.
    pub fn from_path(root: &Path) -> std::io::Result<Self> {
        let mut root_files: Vec<RootFileEntry> = Vec::new();
        let mut directories: Vec<DirEntry> = Vec::new();

        // Top-level pass: classify each entry as a file or a directory.
        // Hidden files (leading dot) at the root are skipped — pidx
        // doesn't need to know about `.gitignore` content for the
        // architecture summary. `.github` is the one allowed exception
        // (CI directories carry signal) but we treat it as a regular
        // dir entry, not a hidden one.
        let skip: HashSet<&str> = SKIP_DIRS.iter().copied().collect();
        let managed: HashSet<&str> = PIDX_MANAGED_FILES.iter().copied().collect();

        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let name = match entry.file_name().into_string() {
                Ok(s) => s,
                Err(_) => continue, // non-UTF-8 names ignored
            };
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Hidden entries skipped except .github.
            let is_hidden = name.starts_with('.');
            if is_hidden && name != ".github" {
                continue;
            }

            if metadata.is_dir() {
                if skip.contains(name.as_str()) {
                    continue;
                }
                let file_count = count_files_recursive(&path, &skip);
                directories.push(DirEntry {
                    path: name.clone(),
                    file_count,
                    kind: DirKind::classify(&name),
                });
            } else if metadata.is_file() {
                if managed.contains(name.as_str()) {
                    continue;
                }
                root_files.push(RootFileEntry {
                    path: name.clone(),
                    size_bytes: metadata.len(),
                });
            }
        }

        // Markdown headings: walk the whole tree, collect every .md,
        // sort, cap, extract. Exclude pidx-managed files at every depth
        // (top-level architecture.md/CHANGELOG.md) so re-running doesn't
        // change the snapshot hash via its own outputs.
        let mut md_paths = collect_markdown_paths(root, &skip, &managed);
        md_paths.sort();
        let mut markdown_headings = Vec::new();
        for path in md_paths.iter().take(MARKDOWN_HEADING_CAP) {
            if let Some(h) = extract_first_heading(path) {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");
                markdown_headings.push(MarkdownHeading {
                    path: rel,
                    heading: h,
                });
            }
        }

        // Determinism: sort all output vectors. (Markdown headings
        // already arrive in sorted-by-path order from the walk above,
        // but re-sorting is cheap and pins the contract.)
        root_files.sort_by(|a, b| a.path.cmp(&b.path));
        directories.sort_by(|a, b| a.path.cmp(&b.path));
        markdown_headings.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(Self {
            root_files,
            directories,
            markdown_headings,
        })
    }
}

/// Recursively count regular files under `dir`, skipping any
/// descendant whose name appears in `skip`. Symlinks counted as files
/// only when their target resolves to a file (we don't follow into
/// loops because [`fs::metadata`] follows the link, but we don't
/// recurse into symlinked dirs).
fn count_files_recursive(dir: &Path, skip: &HashSet<&str>) -> usize {
    let mut count = 0usize;
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    for entry in entries.flatten() {
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };
        // Skip both the global skip-list and hidden dirs (except
        // .github at top-level — but inside a top-level dir, treat
        // hidden as skip).
        if skip.contains(name.as_str()) || name.starts_with('.') {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.is_dir() {
            // Don't follow symlinked directories — protects against
            // loops without needing to track visited inodes.
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if file_type.is_symlink() {
                continue;
            }
            count += count_files_recursive(&entry.path(), skip);
        } else if metadata.is_file() {
            count += 1;
        }
    }
    count
}

/// Recursively collect every `.md` file path under `root`, skipping
/// directories named in `skip` and skipping any top-level file named
/// in `managed` (pidx's own outputs).
fn collect_markdown_paths(
    root: &Path,
    skip: &HashSet<&str>,
    managed: &HashSet<&str>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_for_markdown(root, root, skip, managed, &mut out);
    out
}

fn walk_for_markdown(
    root: &Path,
    dir: &Path,
    skip: &HashSet<&str>,
    managed: &HashSet<&str>,
    out: &mut Vec<PathBuf>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };
        // Hidden + skipped dirs are pruned at every depth (we don't
        // want `.git/refs/heads/.../foo.md` showing up).
        if skip.contains(name.as_str()) {
            continue;
        }
        if name.starts_with('.') && name != ".github" {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.is_dir() {
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if file_type.is_symlink() {
                continue;
            }
            walk_for_markdown(root, &entry.path(), skip, managed, out);
        } else if metadata.is_file() && name.to_ascii_lowercase().ends_with(".md") {
            // Top-level pidx-managed files (architecture.md /
            // CHANGELOG.md) are excluded so the snapshot doesn't
            // include the reducer's own output. Files of the same
            // name nested deeper in the tree are kept (they're
            // user content, not pidx outputs).
            if dir == root && managed.contains(name.as_str()) {
                continue;
            }
            out.push(entry.path());
        }
    }
}

/// Extract the first non-empty `# H1` or `## H2` from `path`. Returns
/// the heading text with leading `#`s and surrounding whitespace
/// stripped. Reads at most ~256 lines so we don't slurp megabyte-sized
/// files.
fn extract_first_heading(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    for (i, line) in reader.lines().enumerate() {
        if i >= 256 {
            break;
        }
        let line = line.ok()?;
        let trimmed = line.trim_start();
        // We accept H1 (# ) and H2 (## ); skip H3+ because they're
        // section sub-headings, not document titles. Skip lines that
        // look like horizontal rules (`---`).
        if let Some(rest) = trimmed.strip_prefix("# ").or_else(|| trimmed.strip_prefix("## ")) {
            let heading = rest.trim().to_string();
            if !heading.is_empty() {
                return Some(heading);
            }
        }
        // Setext-style underlined headings (===== / -----) are rare in
        // modern markdown; skip them on purpose to keep the parser
        // dead-simple.
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Make a temp dir under `std::env::temp_dir()` keyed by test name
    /// + pid so parallel test runs don't collide.
    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pidx-snapshot-test-{}-{}",
            label,
            std::process::id()
        ));
        // Clean any leftover from a prior aborted run.
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dir_kind_heuristics_classify_common_names() {
        // Arrange / Act / Assert
        assert_eq!(DirKind::classify("src"), DirKind::Source);
        assert_eq!(DirKind::classify("crates"), DirKind::Source);
        assert_eq!(DirKind::classify("tests"), DirKind::Tests);
        assert_eq!(DirKind::classify("e2e"), DirKind::Tests);
        assert_eq!(DirKind::classify("docs"), DirKind::Docs);
        assert_eq!(DirKind::classify("examples"), DirKind::Examples);
        assert_eq!(DirKind::classify("config"), DirKind::Config);
        assert_eq!(DirKind::classify("target"), DirKind::Build);
        assert_eq!(DirKind::classify("random_unknown"), DirKind::Other);
    }

    #[test]
    fn extract_first_heading_returns_first_h1_only() {
        // Arrange — file with H1, H2, then another H1.
        let dir = temp_dir("extract-first-heading");
        let path = dir.join("doc.md");
        fs::write(&path, "# Title\n## Subtitle\n# Other\n").unwrap();

        // Act
        let h = extract_first_heading(&path);

        // Assert — the first heading wins regardless of level.
        assert_eq!(h.as_deref(), Some("Title"));

        cleanup(&dir);
    }

    #[test]
    fn extract_first_heading_skips_blank_h1() {
        // Arrange — `# ` with empty body should be skipped.
        let dir = temp_dir("extract-blank");
        let path = dir.join("doc.md");
        fs::write(&path, "# \n## Real\n").unwrap();

        // Act
        let h = extract_first_heading(&path);

        // Assert
        assert_eq!(h.as_deref(), Some("Real"));

        cleanup(&dir);
    }

    #[test]
    fn snapshot_excludes_target_and_includes_known_dirs() {
        // Arrange — synthetic tree with src/, tests/, docs/,
        // examples/, target/. `target/` must NOT appear in the output.
        let dir = temp_dir("dir-classifier");
        for d in ["src", "tests", "docs", "examples", "target"] {
            fs::create_dir(dir.join(d)).unwrap();
            fs::write(dir.join(d).join("placeholder.txt"), "x").unwrap();
        }
        fs::write(dir.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();

        // Act
        let snap = RepoSnapshot::from_path(&dir).unwrap();

        // Assert — target excluded; src/tests/docs/examples present.
        let names: Vec<&str> = snap.directories.iter().map(|d| d.path.as_str()).collect();
        assert!(!names.contains(&"target"), "target must be excluded");
        assert!(names.contains(&"src"));
        assert!(names.contains(&"tests"));
        assert!(names.contains(&"docs"));
        assert!(names.contains(&"examples"));
        let kinds: std::collections::HashMap<_, _> = snap
            .directories
            .iter()
            .map(|d| (d.path.as_str(), d.kind))
            .collect();
        assert_eq!(kinds["src"], DirKind::Source);
        assert_eq!(kinds["tests"], DirKind::Tests);
        assert_eq!(kinds["docs"], DirKind::Docs);
        assert_eq!(kinds["examples"], DirKind::Examples);
        // root file picked up
        assert!(snap.root_files.iter().any(|f| f.path == "Cargo.toml"));

        cleanup(&dir);
    }

    #[test]
    fn snapshot_is_deterministic_across_consecutive_runs() {
        // Arrange — same tree.
        let dir = temp_dir("determinism");
        fs::write(dir.join("README.md"), "# Hello\n").unwrap();
        fs::create_dir(dir.join("src")).unwrap();
        fs::write(dir.join("src").join("lib.rs"), "fn main(){}").unwrap();
        fs::create_dir(dir.join("docs")).unwrap();
        fs::write(dir.join("docs").join("notes.md"), "# Notes\n").unwrap();

        // Act
        let a = RepoSnapshot::from_path(&dir).unwrap();
        let b = RepoSnapshot::from_path(&dir).unwrap();

        // Assert — byte-identical JSON.
        let ja = serde_json::to_string(&a).unwrap();
        let jb = serde_json::to_string(&b).unwrap();
        assert_eq!(ja, jb, "snapshot must be deterministic across runs");

        cleanup(&dir);
    }

    #[test]
    fn snapshot_caps_markdown_headings_at_100() {
        // Arrange — 150 .md files at the root, each with `# Title N`.
        let dir = temp_dir("md-cap");
        for i in 0..150 {
            // Zero-pad so sort order is deterministic and predictable.
            fs::write(
                dir.join(format!("doc-{:03}.md", i)),
                format!("# Title {}\n", i),
            )
            .unwrap();
        }

        // Act
        let snap = RepoSnapshot::from_path(&dir).unwrap();

        // Assert — exactly 100 entries; sorted by path; the kept
        // entries are the lexically-smallest 100 (doc-000.md..doc-099.md).
        assert_eq!(snap.markdown_headings.len(), MARKDOWN_HEADING_CAP);
        let paths: Vec<&str> =
            snap.markdown_headings.iter().map(|h| h.path.as_str()).collect();
        // Sorted check.
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
        // Lexical-smallest 100 kept.
        assert_eq!(paths[0], "doc-000.md");
        assert_eq!(paths[99], "doc-099.md");

        cleanup(&dir);
    }

    #[test]
    fn snapshot_excludes_pidx_managed_files_at_top_level() {
        // Arrange — top-level architecture.md and CHANGELOG.md
        // (pidx's own outputs) plus a non-managed file.
        let dir = temp_dir("managed-files");
        fs::write(dir.join("architecture.md"), "# Architecture\n## Overview\nx\n").unwrap();
        fs::write(dir.join("CHANGELOG.md"), "# Changelog\n").unwrap();
        fs::write(dir.join("README.md"), "# Real\n").unwrap();
        // Same name nested deeper — this is user content and SHOULD
        // appear in the snapshot.
        fs::create_dir(dir.join("docs")).unwrap();
        fs::write(dir.join("docs").join("CHANGELOG.md"), "# Nested\n").unwrap();

        // Act
        let snap = RepoSnapshot::from_path(&dir).unwrap();

        // Assert — top-level managed files excluded; nested CHANGELOG
        // kept; README kept.
        let root_names: Vec<&str> =
            snap.root_files.iter().map(|f| f.path.as_str()).collect();
        assert!(!root_names.contains(&"architecture.md"),
            "top-level architecture.md must be excluded (pidx writes it)");
        assert!(!root_names.contains(&"CHANGELOG.md"),
            "top-level CHANGELOG.md must be excluded (pidx writes it)");
        assert!(root_names.contains(&"README.md"));
        let md_paths: Vec<&str> =
            snap.markdown_headings.iter().map(|h| h.path.as_str()).collect();
        assert!(md_paths.contains(&"README.md"));
        assert!(md_paths.contains(&"docs/CHANGELOG.md"),
            "nested CHANGELOG.md (user content) must be kept");
        assert!(!md_paths.contains(&"architecture.md"));
        assert!(!md_paths.contains(&"CHANGELOG.md"));

        cleanup(&dir);
    }

    #[test]
    fn snapshot_skips_hidden_files_and_dirs_at_root() {
        // Arrange — `.git/`, `.gitignore` at the root must not show up.
        let dir = temp_dir("hidden");
        fs::create_dir(dir.join(".git")).unwrap();
        fs::write(dir.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(dir.join(".gitignore"), "target/\n").unwrap();
        fs::write(dir.join("README.md"), "# X\n").unwrap();

        // Act
        let snap = RepoSnapshot::from_path(&dir).unwrap();

        // Assert
        assert!(!snap.directories.iter().any(|d| d.path == ".git"));
        assert!(!snap.root_files.iter().any(|f| f.path == ".gitignore"));
        assert!(snap.root_files.iter().any(|f| f.path == "README.md"));

        cleanup(&dir);
    }
}
