pub mod commit_store;
pub mod issue_store;
pub mod llm_summary_store;
pub mod release_store;
pub mod repo_store;
pub mod schema;
pub mod sync_log_store;

use std::path::Path;

use anyhow::Context;
use rusqlite::Connection;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create database directory")?;
        }
        let conn = Connection::open(path).context("Failed to open database")?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set pragmas")?;
        let db = Database { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        // Versioned migrations gated on PRAGMA user_version. Each step is
        // idempotent against a clean DB (CREATE IF NOT EXISTS) but the
        // version gate is what makes ALTER-style steps safe to re-run.
        let current: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .context("Failed to read user_version")?;

        // v0 → v1: initial schema. We always run this batch (it's
        // IF NOT EXISTS) so legacy databases that pre-date the migration
        // scheme get caught up before further steps run.
        self.conn
            .execute_batch(schema::CREATE_TABLES)
            .context("Failed to apply v1 (initial schema)")?;
        if current < 1 {
            self.conn
                .execute_batch("PRAGMA user_version = 1;")
                .context("Failed to bump user_version to 1")?;
        }

        // v1 → v2: LLM doc pipeline tables + repos.last_processed_sha.
        if current < 2 {
            self.conn
                .execute_batch(schema::MIGRATION_V2_LLM_DOC_PIPELINE)
                .context("Failed to apply v2 (LLM doc pipeline tables)")?;
            // ADD COLUMN is not IF NOT EXISTS — the version gate above
            // guarantees we only attempt it once per database.
            self.conn
                .execute_batch(schema::MIGRATION_V2_ADD_LAST_PROCESSED_SHA)
                .context("Failed to apply v2 (repos.last_processed_sha)")?;
            self.conn
                .execute_batch("PRAGMA user_version = 2;")
                .context("Failed to bump user_version to 2")?;
        }

        Ok(())
    }

    /// Execute a closure within a transaction. All DB access goes through here.
    pub fn tx<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&Connection) -> anyhow::Result<T>,
    {
        let tx = self
            .conn
            .unchecked_transaction()
            .context("Failed to begin transaction")?;
        let result = f(&tx)?;
        tx.commit().context("Failed to commit transaction")?;
        Ok(result)
    }
}
