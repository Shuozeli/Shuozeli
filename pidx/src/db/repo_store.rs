use anyhow::Context;
use rusqlite::{Connection, params};

#[derive(Debug, Clone)]
pub struct RepoRow {
    pub id: i64,
    pub owner: String,
    pub name: String,
    pub language: Option<String>,
    pub description: Option<String>,
    pub open_issues: i32,
    pub pushed_at: Option<String>,
    pub synced_at: Option<String>,
    pub category: Option<String>,
    pub last_processed_sha: Option<String>,
}

pub fn upsert_repo(
    conn: &Connection,
    owner: &str,
    name: &str,
    language: Option<&str>,
    description: Option<&str>,
    open_issues: i32,
    pushed_at: Option<&str>,
    category: Option<&str>,
) -> anyhow::Result<i64> {
    conn.execute(
        r#"INSERT INTO repos (owner, name, language, description, open_issues, pushed_at, synced_at, category)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), ?7)
           ON CONFLICT(owner, name) DO UPDATE SET
             language = excluded.language,
             description = excluded.description,
             open_issues = excluded.open_issues,
             pushed_at = excluded.pushed_at,
             synced_at = datetime('now'),
             category = excluded.category"#,
        params![owner, name, language, description, open_issues, pushed_at, category],
    )
    .context("Failed to upsert repo")?;

    let id = conn
        .query_row(
            "SELECT id FROM repos WHERE owner = ?1 AND name = ?2",
            params![owner, name],
            |row| row.get(0),
        )
        .context("Failed to get repo id")?;
    Ok(id)
}

pub fn get_all_repos(conn: &Connection) -> anyhow::Result<Vec<RepoRow>> {
    let mut stmt = conn
        .prepare("SELECT id, owner, name, language, description, open_issues, pushed_at, synced_at, category, last_processed_sha FROM repos ORDER BY name")
        .context("Failed to prepare repo query")?;

    let rows = stmt
        .query_map([], |row| {
            Ok(RepoRow {
                id: row.get(0)?,
                owner: row.get(1)?,
                name: row.get(2)?,
                language: row.get(3)?,
                description: row.get(4)?,
                open_issues: row.get(5)?,
                pushed_at: row.get(6)?,
                synced_at: row.get(7)?,
                category: row.get(8)?,
                last_processed_sha: row.get(9)?,
            })
        })
        .context("Failed to query repos")?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read repo row")?);
    }
    Ok(result)
}

/// Advance the `repos.last_processed_sha` cursor. Phase 2 calls this
/// after a successful changelog render+write so subsequent runs only
/// touch new commits. The atomicity matters: callers MUST NOT call
/// this until the file write has succeeded.
pub fn update_last_processed_sha(
    conn: &Connection,
    repo_id: i64,
    sha: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE repos SET last_processed_sha = ?1 WHERE id = ?2",
        params![sha, repo_id],
    )
    .context("Failed to update repos.last_processed_sha")?;
    Ok(())
}

pub fn get_repo_by_name(
    conn: &Connection,
    owner: &str,
    name: &str,
) -> anyhow::Result<Option<RepoRow>> {
    let mut stmt = conn
        .prepare("SELECT id, owner, name, language, description, open_issues, pushed_at, synced_at, category, last_processed_sha FROM repos WHERE owner = ?1 AND name = ?2")
        .context("Failed to prepare repo query")?;

    let mut rows = stmt
        .query_map(params![owner, name], |row| {
            Ok(RepoRow {
                id: row.get(0)?,
                owner: row.get(1)?,
                name: row.get(2)?,
                language: row.get(3)?,
                description: row.get(4)?,
                open_issues: row.get(5)?,
                pushed_at: row.get(6)?,
                synced_at: row.get(7)?,
                category: row.get(8)?,
                last_processed_sha: row.get(9)?,
            })
        })
        .context("Failed to query repo")?;

    match rows.next() {
        Some(row) => Ok(Some(row.context("Failed to read repo row")?)),
        None => Ok(None),
    }
}
