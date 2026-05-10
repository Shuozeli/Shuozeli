//! Cache store for `commit_classifications`.
//!
//! Keyed on `(repo_id, sha, prompt_version)` — bumping the prompt
//! version makes the next run insert fresh rows that overwrite the old
//! ones via `INSERT OR REPLACE` semantics (the `ON CONFLICT(...) DO
//! UPDATE` form, which we use because SQLite's `INSERT OR REPLACE`
//! drops + re-inserts and would break ON DELETE CASCADE behavior on
//! related rows in future schema growth).

use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};

use crate::llm::{Classification, CommitCategory, CommitImpact};

/// Row read out of `commit_classifications`. `summary` / `impact` are
/// load-bearing for Phase 2 reducers; `llm_provider` / `llm_model` /
/// `classified_at` are surfaced for future telemetry. `dead_code` is
/// allowed at the struct level so clippy doesn't yell while Phase 2+
/// is still landing.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CachedClassification {
    pub category: CommitCategory,
    pub summary: String,
    pub impact: CommitImpact,
    pub llm_provider: String,
    pub llm_model: String,
    pub classified_at: i64,
}

fn category_str(c: CommitCategory) -> &'static str {
    match c {
        CommitCategory::Added => "Added",
        CommitCategory::Changed => "Changed",
        CommitCategory::Fixed => "Fixed",
        CommitCategory::Removed => "Removed",
        CommitCategory::Internal => "Internal",
    }
}

fn parse_category(s: &str) -> anyhow::Result<CommitCategory> {
    match s {
        "Added" => Ok(CommitCategory::Added),
        "Changed" => Ok(CommitCategory::Changed),
        "Fixed" => Ok(CommitCategory::Fixed),
        "Removed" => Ok(CommitCategory::Removed),
        "Internal" => Ok(CommitCategory::Internal),
        other => anyhow::bail!("unknown category in commit_classifications: {other}"),
    }
}

fn impact_str(i: CommitImpact) -> &'static str {
    match i {
        CommitImpact::Minor => "minor",
        CommitImpact::Major => "major",
        CommitImpact::Breaking => "breaking",
    }
}

fn parse_impact(s: &str) -> anyhow::Result<CommitImpact> {
    match s {
        "minor" => Ok(CommitImpact::Minor),
        "major" => Ok(CommitImpact::Major),
        "breaking" => Ok(CommitImpact::Breaking),
        other => anyhow::bail!("unknown impact in commit_classifications: {other}"),
    }
}

/// Look up a cached classification. Returns `Ok(None)` for cache miss.
pub fn get_classification(
    conn: &Connection,
    repo_id: i64,
    sha: &str,
    prompt_version: u32,
) -> anyhow::Result<Option<CachedClassification>> {
    let row = conn
        .query_row(
            "SELECT category, summary, impact, llm_provider, llm_model, classified_at
             FROM commit_classifications
             WHERE repo_id = ?1 AND sha = ?2 AND prompt_version = ?3",
            params![repo_id, sha, prompt_version],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()
        .context("Failed to query commit_classifications")?;

    match row {
        None => Ok(None),
        Some((cat, summary, impact, provider, model, ts)) => {
            Ok(Some(CachedClassification {
                category: parse_category(&cat)?,
                summary,
                impact: parse_impact(&impact)?,
                llm_provider: provider,
                llm_model: model,
                classified_at: ts,
            }))
        }
    }
}

/// Upsert a classification. Uses ON CONFLICT DO UPDATE so a re-run
/// after a prompt-version bump cleanly replaces the old row.
///
/// Eight args (one over clippy's default seven) is intentional: the
/// schema has eight columns and bundling them into a struct just to
/// shave one parameter would obscure the wire format. See
/// `commit_classifications` PRIMARY KEY in `db/schema.rs`.
#[allow(clippy::too_many_arguments)]
pub fn upsert_classification(
    conn: &Connection,
    repo_id: i64,
    sha: &str,
    prompt_version: u32,
    classification: &Classification,
    llm_provider: &str,
    llm_model: &str,
    classified_at: i64,
) -> anyhow::Result<()> {
    conn.execute(
        r#"INSERT INTO commit_classifications
             (repo_id, sha, prompt_version, category, summary, impact,
              llm_provider, llm_model, classified_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
           ON CONFLICT(repo_id, sha, prompt_version) DO UPDATE SET
             category      = excluded.category,
             summary       = excluded.summary,
             impact        = excluded.impact,
             llm_provider  = excluded.llm_provider,
             llm_model     = excluded.llm_model,
             classified_at = excluded.classified_at"#,
        params![
            repo_id,
            sha,
            prompt_version,
            category_str(classification.category),
            classification.summary,
            impact_str(classification.impact),
            llm_provider,
            llm_model,
            classified_at,
        ],
    )
    .context("Failed to upsert commit_classifications row")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema;

    fn open_in_memory() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        conn.execute_batch(schema::MIGRATION_V2_LLM_DOC_PIPELINE).unwrap();
        // Seed one repo so our FK is satisfied.
        conn.execute(
            "INSERT INTO repos (owner, name, open_issues) VALUES ('test', 'test-repo', 0)",
            [],
        )
        .unwrap();
        conn
    }

    fn sample() -> Classification {
        Classification {
            category: CommitCategory::Fixed,
            summary: "abort acquire loops on drain".into(),
            impact: CommitImpact::Minor,
        }
    }

    #[test]
    fn get_classification_returns_none_for_cache_miss() {
        // Arrange
        let conn = open_in_memory();

        // Act
        let result = get_classification(&conn, 1, "deadbeef", 1).unwrap();

        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn upsert_then_get_round_trips_all_fields() {
        // Arrange
        let conn = open_in_memory();
        let cls = sample();

        // Act
        upsert_classification(
            &conn,
            1,
            "abc123",
            1,
            &cls,
            "minimax",
            "MiniMax-M2",
            1_700_000_000,
        )
        .unwrap();
        let got = get_classification(&conn, 1, "abc123", 1).unwrap().unwrap();

        // Assert
        assert_eq!(got.category, CommitCategory::Fixed);
        assert_eq!(got.summary, "abort acquire loops on drain");
        assert_eq!(got.impact, CommitImpact::Minor);
        assert_eq!(got.llm_provider, "minimax");
        assert_eq!(got.llm_model, "MiniMax-M2");
        assert_eq!(got.classified_at, 1_700_000_000);
    }

    #[test]
    fn upsert_replaces_existing_row_on_conflict() {
        // Arrange — seed a row, then write a different one with same key.
        let conn = open_in_memory();
        upsert_classification(
            &conn,
            1,
            "abc",
            1,
            &Classification {
                category: CommitCategory::Internal,
                summary: "old".into(),
                impact: CommitImpact::Minor,
            },
            "minimax",
            "MiniMax-M2",
            1,
        )
        .unwrap();

        // Act
        upsert_classification(
            &conn,
            1,
            "abc",
            1,
            &Classification {
                category: CommitCategory::Fixed,
                summary: "new".into(),
                impact: CommitImpact::Major,
            },
            "minimax",
            "MiniMax-M2",
            2,
        )
        .unwrap();
        let got = get_classification(&conn, 1, "abc", 1).unwrap().unwrap();

        // Assert
        assert_eq!(got.summary, "new");
        assert_eq!(got.category, CommitCategory::Fixed);
        assert_eq!(got.impact, CommitImpact::Major);
        assert_eq!(got.classified_at, 2);
    }

    #[test]
    fn different_prompt_versions_keep_separate_rows() {
        // Arrange
        let conn = open_in_memory();
        upsert_classification(
            &conn,
            1,
            "abc",
            1,
            &Classification {
                category: CommitCategory::Internal,
                summary: "v1".into(),
                impact: CommitImpact::Minor,
            },
            "minimax",
            "MiniMax-M2",
            1,
        )
        .unwrap();

        // Act
        upsert_classification(
            &conn,
            1,
            "abc",
            2,
            &Classification {
                category: CommitCategory::Fixed,
                summary: "v2".into(),
                impact: CommitImpact::Minor,
            },
            "minimax",
            "MiniMax-M2",
            2,
        )
        .unwrap();
        let v1 = get_classification(&conn, 1, "abc", 1).unwrap().unwrap();
        let v2 = get_classification(&conn, 1, "abc", 2).unwrap().unwrap();

        // Assert
        assert_eq!(v1.summary, "v1");
        assert_eq!(v2.summary, "v2");
    }
}
