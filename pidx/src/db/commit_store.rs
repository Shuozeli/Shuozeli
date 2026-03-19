use anyhow::Context;
use rusqlite::{Connection, params};

#[derive(Debug, Clone)]
pub struct CommitRow {
    pub id: i64,
    pub repo_id: i64,
    pub sha: String,
    pub message: String,
    pub author: Option<String>,
    pub committed_at: String,
    pub category: String,
}

pub fn upsert_commit(
    conn: &Connection,
    repo_id: i64,
    sha: &str,
    message: &str,
    author: Option<&str>,
    committed_at: &str,
    category: &str,
) -> anyhow::Result<()> {
    conn.execute(
        r#"INSERT INTO commits (repo_id, sha, message, author, committed_at, category)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)
           ON CONFLICT(repo_id, sha) DO UPDATE SET
             message = excluded.message,
             author = excluded.author,
             committed_at = excluded.committed_at,
             category = excluded.category"#,
        params![repo_id, sha, message, author, committed_at, category],
    )
    .context("Failed to upsert commit")?;
    Ok(())
}

pub fn get_commits_since(
    conn: &Connection,
    repo_id: i64,
    since: &str,
) -> anyhow::Result<Vec<CommitRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, sha, message, author, committed_at, category
             FROM commits
             WHERE repo_id = ?1 AND committed_at >= ?2
             ORDER BY committed_at DESC",
        )
        .context("Failed to prepare commit query")?;

    let rows = stmt
        .query_map(params![repo_id, since], |row| {
            Ok(CommitRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                sha: row.get(2)?,
                message: row.get(3)?,
                author: row.get(4)?,
                committed_at: row.get(5)?,
                category: row.get(6)?,
            })
        })
        .context("Failed to query commits")?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read commit row")?);
    }
    Ok(result)
}

pub fn count_commits_since(
    conn: &Connection,
    repo_id: i64,
    since: &str,
) -> anyhow::Result<u32> {
    let count: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM commits WHERE repo_id = ?1 AND committed_at >= ?2",
            params![repo_id, since],
            |row| row.get(0),
        )
        .context("Failed to count commits")?;
    Ok(count)
}

pub fn get_commits_between(
    conn: &Connection,
    repo_id: i64,
    since: &str,
    until: &str,
) -> anyhow::Result<Vec<CommitRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, sha, message, author, committed_at, category
             FROM commits
             WHERE repo_id = ?1 AND committed_at >= ?2 AND committed_at < ?3
             ORDER BY committed_at DESC",
        )
        .context("Failed to prepare commit query")?;

    let rows = stmt
        .query_map(params![repo_id, since, until], |row| {
            Ok(CommitRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                sha: row.get(2)?,
                message: row.get(3)?,
                author: row.get(4)?,
                committed_at: row.get(5)?,
                category: row.get(6)?,
            })
        })
        .context("Failed to query commits")?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read commit row")?);
    }
    Ok(result)
}

pub fn get_all_commits_for_repo(
    conn: &Connection,
    repo_id: i64,
) -> anyhow::Result<Vec<CommitRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, sha, message, author, committed_at, category
             FROM commits
             WHERE repo_id = ?1
             ORDER BY committed_at DESC",
        )
        .context("Failed to prepare commit query")?;

    let rows = stmt
        .query_map(params![repo_id], |row| {
            Ok(CommitRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                sha: row.get(2)?,
                message: row.get(3)?,
                author: row.get(4)?,
                committed_at: row.get(5)?,
                category: row.get(6)?,
            })
        })
        .context("Failed to query commits")?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read commit row")?);
    }
    Ok(result)
}
