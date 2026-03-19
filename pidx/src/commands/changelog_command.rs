use std::fs;

use anyhow::{Context, bail};
use chrono::{Datelike, NaiveDate, Weekday};

use crate::config::Config;
use crate::db::Database;
use crate::db::commit_store;
use crate::db::issue_store;
use crate::db::release_store;
use crate::db::repo_store;

/// Parse an ISO week string like "2026-W12" into (monday_date, sunday_date).
fn parse_iso_week(week_str: &str) -> anyhow::Result<(NaiveDate, NaiveDate)> {
    let parts: Vec<&str> = week_str.split("-W").collect();
    if parts.len() != 2 {
        bail!("Invalid week format: {week_str}. Expected YYYY-WNN (e.g. 2026-W12)");
    }
    let year: i32 = parts[0].parse().context("Invalid year in week string")?;
    let week: u32 = parts[1].parse().context("Invalid week number in week string")?;
    if week == 0 || week > 53 {
        bail!("Week number must be 1-53, got {week}");
    }

    let monday = NaiveDate::from_isoywd_opt(year, week, Weekday::Mon)
        .with_context(|| format!("Invalid ISO week: {week_str}"))?;
    let sunday = NaiveDate::from_isoywd_opt(year, week, Weekday::Sun)
        .with_context(|| format!("Invalid ISO week: {week_str}"))?;

    Ok((monday, sunday))
}

/// Get the current ISO week as "YYYY-WNN".
fn current_iso_week() -> String {
    let today = chrono::Utc::now().date_naive();
    format!("{}-W{:02}", today.iso_week().year(), today.iso_week().week())
}

pub fn export(config: &Config, week: Option<&str>, repo_filter: Option<&str>) -> anyhow::Result<()> {
    let week_str = match week {
        Some(w) => w.to_string(),
        None => current_iso_week(),
    };

    let (monday, sunday) = parse_iso_week(&week_str)?;
    let since = format!("{monday}T00:00:00Z");
    let until_date = sunday.succ_opt().context("Date overflow")?;
    let until = format!("{until_date}T00:00:00Z");

    let db = Database::open(&config.db_path())?;
    let repos = db.tx(|conn| repo_store::get_all_repos(conn))?;

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut exported = 0;

    for repo in &repos {
        if let Some(filter) = repo_filter {
            if repo.name != filter {
                continue;
            }
        }

        let commits = db.tx(|conn| commit_store::get_commits_between(conn, repo.id, &since, &until))?;
        let issues_opened = db.tx(|conn| issue_store::get_issues_opened_between(conn, repo.id, &since, &until))?;
        let issues_closed = db.tx(|conn| issue_store::get_issues_closed_between(conn, repo.id, &since, &until))?;
        let releases = db.tx(|conn| release_store::get_releases_between(conn, repo.id, &since, &until))?;

        let docs_dir = Config::repo_docs_dir(&repo.name);
        fs::create_dir_all(&docs_dir)?;

        let mut content = format!(
            "---\nrepo: {}\nweek: {}\nperiod: {} to {}\ngenerated_at: {}\n---\n\n# {} -- Week {}\n\n",
            repo.name, week_str, monday, sunday, now, repo.name, week_str,
        );

        let has_activity = !commits.is_empty() || !issues_opened.is_empty() || !issues_closed.is_empty() || !releases.is_empty();

        if !has_activity {
            content.push_str("No activity this week.\n");
        } else {
            // Commits
            content.push_str(&format!("## Commits ({})\n\n", commits.len()));
            if commits.is_empty() {
                content.push_str("None\n\n");
            } else {
                for c in &commits {
                    let short_sha = &c.sha[..7.min(c.sha.len())];
                    let first_line = c.message.lines().next().unwrap_or(&c.message);
                    content.push_str(&format!("- **[{}]** `{}` {}\n", c.category, short_sha, first_line));
                }
                content.push('\n');
            }

            // Issues opened
            content.push_str(&format!("## Issues Opened ({})\n\n", issues_opened.len()));
            if issues_opened.is_empty() {
                content.push_str("None\n\n");
            } else {
                for i in &issues_opened {
                    content.push_str(&format!("- #{} {}\n", i.number, i.title));
                }
                content.push('\n');
            }

            // Issues closed
            content.push_str(&format!("## Issues Closed ({})\n\n", issues_closed.len()));
            if issues_closed.is_empty() {
                content.push_str("None\n\n");
            } else {
                for i in &issues_closed {
                    content.push_str(&format!("- #{} {}\n", i.number, i.title));
                }
                content.push('\n');
            }

            // Releases
            content.push_str(&format!("## Releases ({})\n\n", releases.len()));
            if releases.is_empty() {
                content.push_str("None\n\n");
            } else {
                for r in &releases {
                    let name = r.name.as_deref().unwrap_or(&r.tag_name);
                    content.push_str(&format!("- {} ({})\n", name, r.tag_name));
                }
                content.push('\n');
            }
        }

        let output_path = docs_dir.join(format!("weekly-{}.md", week_str));
        fs::write(&output_path, &content).context("Failed to write weekly export")?;
        exported += 1;

        let status = if has_activity { "active" } else { "quiet" };
        tracing::info!("{}: {} ({} commits, {} issues opened, {} closed, {} releases)",
            repo.name, status, commits.len(), issues_opened.len(), issues_closed.len(), releases.len());
    }

    println!("Exported weekly data for {} repos to {}", exported, Config::docs_dir().display());
    Ok(())
}
