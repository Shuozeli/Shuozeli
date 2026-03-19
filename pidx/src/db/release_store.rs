use anyhow::Context;
use rusqlite::{Connection, params};

#[derive(Debug, Clone)]
pub struct ReleaseRow {
    pub id: i64,
    pub repo_id: i64,
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub published_at: Option<String>,
}

pub fn upsert_release(
    conn: &Connection,
    repo_id: i64,
    tag_name: &str,
    name: Option<&str>,
    body: Option<&str>,
    published_at: Option<&str>,
) -> anyhow::Result<()> {
    conn.execute(
        r#"INSERT INTO releases (repo_id, tag_name, name, body, published_at)
           VALUES (?1, ?2, ?3, ?4, ?5)
           ON CONFLICT(repo_id, tag_name) DO UPDATE SET
             name = excluded.name,
             body = excluded.body,
             published_at = excluded.published_at"#,
        params![repo_id, tag_name, name, body, published_at],
    )
    .context("Failed to upsert release")?;
    Ok(())
}

pub fn get_releases_between(
    conn: &Connection,
    repo_id: i64,
    since: &str,
    until: &str,
) -> anyhow::Result<Vec<ReleaseRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, tag_name, name, body, published_at
             FROM releases
             WHERE repo_id = ?1 AND published_at >= ?2 AND published_at < ?3
             ORDER BY published_at DESC",
        )
        .context("Failed to prepare release query")?;

    let rows = stmt
        .query_map(params![repo_id, since, until], |row| {
            Ok(ReleaseRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                tag_name: row.get(2)?,
                name: row.get(3)?,
                body: row.get(4)?,
                published_at: row.get(5)?,
            })
        })
        .context("Failed to query releases")?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read release row")?);
    }
    Ok(result)
}

pub fn get_releases_for_repo(
    conn: &Connection,
    repo_id: i64,
) -> anyhow::Result<Vec<ReleaseRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, tag_name, name, body, published_at
             FROM releases
             WHERE repo_id = ?1
             ORDER BY published_at DESC",
        )
        .context("Failed to prepare release query")?;

    let rows = stmt
        .query_map(params![repo_id], |row| {
            Ok(ReleaseRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                tag_name: row.get(2)?,
                name: row.get(3)?,
                body: row.get(4)?,
                published_at: row.get(5)?,
            })
        })
        .context("Failed to query releases")?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read release row")?);
    }
    Ok(result)
}
