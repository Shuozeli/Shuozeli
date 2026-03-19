use anyhow::Context;
use rusqlite::{Connection, params};

#[derive(Debug, Clone)]
pub struct IssueRow {
    pub id: i64,
    pub repo_id: i64,
    pub number: i32,
    pub title: String,
    pub state: String,
    pub labels: String,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub closed_at: Option<String>,
}

pub fn upsert_issue(
    conn: &Connection,
    repo_id: i64,
    number: i32,
    title: &str,
    state: &str,
    labels: &str,
    created_at: &str,
    updated_at: Option<&str>,
    closed_at: Option<&str>,
) -> anyhow::Result<()> {
    conn.execute(
        r#"INSERT INTO issues (repo_id, number, title, state, labels, created_at, updated_at, closed_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
           ON CONFLICT(repo_id, number) DO UPDATE SET
             title = excluded.title,
             state = excluded.state,
             labels = excluded.labels,
             updated_at = excluded.updated_at,
             closed_at = excluded.closed_at"#,
        params![repo_id, number, title, state, labels, created_at, updated_at, closed_at],
    )
    .context("Failed to upsert issue")?;
    Ok(())
}

pub fn get_open_issues(conn: &Connection, repo_id: i64) -> anyhow::Result<Vec<IssueRow>> {
    get_issues_by_state(conn, repo_id, "open")
}

pub fn get_issues_by_state(
    conn: &Connection,
    repo_id: i64,
    state: &str,
) -> anyhow::Result<Vec<IssueRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, number, title, state, labels, created_at, updated_at, closed_at
             FROM issues
             WHERE repo_id = ?1 AND state = ?2
             ORDER BY number DESC",
        )
        .context("Failed to prepare issue query")?;

    let rows = stmt
        .query_map(params![repo_id, state], |row| {
            Ok(IssueRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                number: row.get(2)?,
                title: row.get(3)?,
                state: row.get(4)?,
                labels: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                closed_at: row.get(8)?,
            })
        })
        .context("Failed to query issues")?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read issue row")?);
    }
    Ok(result)
}

pub fn get_issues_opened_between(
    conn: &Connection,
    repo_id: i64,
    since: &str,
    until: &str,
) -> anyhow::Result<Vec<IssueRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, number, title, state, labels, created_at, updated_at, closed_at
             FROM issues
             WHERE repo_id = ?1 AND created_at >= ?2 AND created_at < ?3
             ORDER BY number DESC",
        )
        .context("Failed to prepare issue query")?;

    let rows = stmt
        .query_map(params![repo_id, since, until], |row| {
            Ok(IssueRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                number: row.get(2)?,
                title: row.get(3)?,
                state: row.get(4)?,
                labels: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                closed_at: row.get(8)?,
            })
        })
        .context("Failed to query issues")?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read issue row")?);
    }
    Ok(result)
}

pub fn get_issues_closed_between(
    conn: &Connection,
    repo_id: i64,
    since: &str,
    until: &str,
) -> anyhow::Result<Vec<IssueRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, number, title, state, labels, created_at, updated_at, closed_at
             FROM issues
             WHERE repo_id = ?1 AND closed_at >= ?2 AND closed_at < ?3
             ORDER BY number DESC",
        )
        .context("Failed to prepare issue query")?;

    let rows = stmt
        .query_map(params![repo_id, since, until], |row| {
            Ok(IssueRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                number: row.get(2)?,
                title: row.get(3)?,
                state: row.get(4)?,
                labels: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                closed_at: row.get(8)?,
            })
        })
        .context("Failed to query issues")?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read issue row")?);
    }
    Ok(result)
}

pub fn get_all_issues_for_repo(
    conn: &Connection,
    repo_id: i64,
) -> anyhow::Result<Vec<IssueRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, number, title, state, labels, created_at, updated_at, closed_at
             FROM issues
             WHERE repo_id = ?1
             ORDER BY number DESC",
        )
        .context("Failed to prepare issue query")?;

    let rows = stmt
        .query_map(params![repo_id], |row| {
            Ok(IssueRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                number: row.get(2)?,
                title: row.get(3)?,
                state: row.get(4)?,
                labels: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                closed_at: row.get(8)?,
            })
        })
        .context("Failed to query issues")?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read issue row")?);
    }
    Ok(result)
}
