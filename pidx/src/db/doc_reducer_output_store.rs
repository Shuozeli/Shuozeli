//! Cache store for `doc_reducer_outputs`.
//!
//! Keyed on `(repo_id, kind, scope_key)` — for the changelog reducer
//! `kind == "changelog"` and `scope_key == "week-2026-W18"`.
//! `input_hash` is recomputed on every reduce attempt; on cache lookup
//! the caller compares the stored hash against the recomputed one and
//! treats a mismatch as a cache miss (the row is overwritten on the
//! next successful LLM call).

use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};

/// Row read from `doc_reducer_outputs`.
#[derive(Debug, Clone)]
pub struct CachedReducerOutput {
    pub input_hash: String,
    pub output: String,
    #[allow(dead_code)] // surfaced for future telemetry
    pub llm_provider: String,
    #[allow(dead_code)]
    pub llm_model: String,
    #[allow(dead_code)]
    pub rendered_at: i64,
}

/// Look up a cached reducer output. Returns `Ok(None)` for cache miss.
///
/// The caller is responsible for comparing `result.input_hash` against
/// a freshly recomputed hash; we do that comparison outside the store
/// so the hashing scheme stays a property of the reducer (not the DB).
pub fn get_reducer_output(
    conn: &Connection,
    repo_id: i64,
    kind: &str,
    scope_key: &str,
) -> anyhow::Result<Option<CachedReducerOutput>> {
    let row = conn
        .query_row(
            "SELECT input_hash, output, llm_provider, llm_model, rendered_at
             FROM doc_reducer_outputs
             WHERE repo_id = ?1 AND kind = ?2 AND scope_key = ?3",
            params![repo_id, kind, scope_key],
            |row| {
                Ok(CachedReducerOutput {
                    input_hash: row.get(0)?,
                    output: row.get(1)?,
                    llm_provider: row.get(2)?,
                    llm_model: row.get(3)?,
                    rendered_at: row.get(4)?,
                })
            },
        )
        .optional()
        .context("Failed to query doc_reducer_outputs")?;
    Ok(row)
}

/// Upsert a reducer output. Uses ON CONFLICT DO UPDATE so a re-reduce
/// after a prompt-version bump cleanly replaces the old row.
#[allow(clippy::too_many_arguments)]
pub fn upsert_reducer_output(
    conn: &Connection,
    repo_id: i64,
    kind: &str,
    scope_key: &str,
    input_hash: &str,
    output: &str,
    llm_provider: &str,
    llm_model: &str,
    rendered_at: i64,
) -> anyhow::Result<()> {
    conn.execute(
        r#"INSERT INTO doc_reducer_outputs
             (repo_id, kind, scope_key, input_hash, output,
              llm_provider, llm_model, rendered_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
           ON CONFLICT(repo_id, kind, scope_key) DO UPDATE SET
             input_hash   = excluded.input_hash,
             output       = excluded.output,
             llm_provider = excluded.llm_provider,
             llm_model    = excluded.llm_model,
             rendered_at  = excluded.rendered_at"#,
        params![
            repo_id,
            kind,
            scope_key,
            input_hash,
            output,
            llm_provider,
            llm_model,
            rendered_at,
        ],
    )
    .context("Failed to upsert doc_reducer_outputs row")?;
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
        conn.execute(
            "INSERT INTO repos (owner, name, open_issues) VALUES ('test', 'test-repo', 0)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn get_reducer_output_returns_none_for_cache_miss() {
        // Arrange
        let conn = open_in_memory();

        // Act
        let result = get_reducer_output(&conn, 1, "changelog", "week-2026-W18").unwrap();

        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn upsert_then_get_round_trips_all_fields() {
        // Arrange
        let conn = open_in_memory();

        // Act
        upsert_reducer_output(
            &conn,
            1,
            "changelog",
            "week-2026-W18",
            "deadbeefhash",
            "### Week of 2026-04-27\n\n#### Fixed\n- thing\n",
            "minimax",
            "MiniMax-M2",
            1_700_000_000,
        )
        .unwrap();
        let got = get_reducer_output(&conn, 1, "changelog", "week-2026-W18")
            .unwrap()
            .unwrap();

        // Assert
        assert_eq!(got.input_hash, "deadbeefhash");
        assert!(got.output.contains("### Week of 2026-04-27"));
        assert_eq!(got.llm_provider, "minimax");
        assert_eq!(got.llm_model, "MiniMax-M2");
        assert_eq!(got.rendered_at, 1_700_000_000);
    }

    #[test]
    fn upsert_replaces_existing_row_on_conflict() {
        // Arrange — seed an old row.
        let conn = open_in_memory();
        upsert_reducer_output(
            &conn, 1, "changelog", "week-2026-W18", "old_hash", "old output",
            "minimax", "MiniMax-M2", 1,
        )
        .unwrap();

        // Act — write again with same primary key, different payload.
        upsert_reducer_output(
            &conn, 1, "changelog", "week-2026-W18", "new_hash", "new output",
            "minimax", "MiniMax-M2", 2,
        )
        .unwrap();
        let got = get_reducer_output(&conn, 1, "changelog", "week-2026-W18")
            .unwrap()
            .unwrap();

        // Assert
        assert_eq!(got.input_hash, "new_hash");
        assert_eq!(got.output, "new output");
        assert_eq!(got.rendered_at, 2);
    }

    #[test]
    fn different_scopes_keep_separate_rows() {
        // Arrange
        let conn = open_in_memory();
        upsert_reducer_output(
            &conn, 1, "changelog", "week-2026-W17", "h17", "out17",
            "minimax", "MiniMax-M2", 1,
        )
        .unwrap();

        // Act
        upsert_reducer_output(
            &conn, 1, "changelog", "week-2026-W18", "h18", "out18",
            "minimax", "MiniMax-M2", 2,
        )
        .unwrap();
        let r17 = get_reducer_output(&conn, 1, "changelog", "week-2026-W17")
            .unwrap()
            .unwrap();
        let r18 = get_reducer_output(&conn, 1, "changelog", "week-2026-W18")
            .unwrap()
            .unwrap();

        // Assert
        assert_eq!(r17.output, "out17");
        assert_eq!(r18.output, "out18");
    }
}
