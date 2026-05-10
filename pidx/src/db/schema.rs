/// Initial table layout (version 0 → 1). Uses `IF NOT EXISTS` so it is safe
/// to run on a fresh database; for an existing database that pre-dates the
/// `user_version` migration scheme we ran this batch before bumping the
/// version, so re-running is a no-op.
pub const CREATE_TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS repos (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    owner       TEXT NOT NULL,
    name        TEXT NOT NULL,
    language    TEXT,
    description TEXT,
    open_issues INTEGER NOT NULL DEFAULT 0,
    pushed_at   TEXT,
    synced_at   TEXT,
    category    TEXT,
    UNIQUE(owner, name)
);

CREATE TABLE IF NOT EXISTS commits (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id      INTEGER NOT NULL REFERENCES repos(id),
    sha          TEXT NOT NULL,
    message      TEXT NOT NULL,
    author       TEXT,
    committed_at TEXT NOT NULL,
    category     TEXT NOT NULL,
    UNIQUE(repo_id, sha)
);

CREATE TABLE IF NOT EXISTS issues (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id    INTEGER NOT NULL REFERENCES repos(id),
    number     INTEGER NOT NULL,
    title      TEXT NOT NULL,
    state      TEXT NOT NULL,
    labels     TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT,
    closed_at  TEXT,
    UNIQUE(repo_id, number)
);

CREATE TABLE IF NOT EXISTS releases (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id      INTEGER NOT NULL REFERENCES repos(id),
    tag_name     TEXT NOT NULL,
    name         TEXT,
    body         TEXT,
    published_at TEXT,
    UNIQUE(repo_id, tag_name)
);

CREATE TABLE IF NOT EXISTS llm_summaries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repos(id),
    analyzed_at     TEXT NOT NULL,
    model           TEXT,
    status_summary  TEXT,
    risks           TEXT,
    recommendations TEXT,
    raw_content     TEXT NOT NULL,
    ingested_at     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_name  TEXT NOT NULL,
    event_type TEXT NOT NULL,
    detail     TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

/// LLM doc pipeline (Phase 0): per-commit classification cache,
/// per-repo per-kind reducer output cache, and the
/// `repos.last_processed_sha` cursor used by `pidx changelog`.
pub const MIGRATION_V2_LLM_DOC_PIPELINE: &str = r#"
CREATE TABLE IF NOT EXISTS commit_classifications (
    repo_id          INTEGER NOT NULL,
    sha              TEXT    NOT NULL,
    prompt_version   INTEGER NOT NULL,
    category         TEXT    NOT NULL,
    summary          TEXT    NOT NULL,
    impact           TEXT    NOT NULL,
    llm_provider     TEXT    NOT NULL,
    llm_model        TEXT    NOT NULL,
    classified_at    INTEGER NOT NULL,
    PRIMARY KEY (repo_id, sha, prompt_version),
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS doc_reducer_outputs (
    repo_id          INTEGER NOT NULL,
    kind             TEXT    NOT NULL,
    scope_key        TEXT    NOT NULL,
    input_hash       TEXT    NOT NULL,
    output           TEXT    NOT NULL,
    llm_provider     TEXT    NOT NULL,
    llm_model        TEXT    NOT NULL,
    rendered_at      INTEGER NOT NULL,
    PRIMARY KEY (repo_id, kind, scope_key),
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);
"#;

/// `ALTER TABLE` step for v2. Kept separate because `ADD COLUMN` is not
/// idempotent in SQLite and the migration runner gates it on `user_version`.
pub const MIGRATION_V2_ADD_LAST_PROCESSED_SHA: &str =
    "ALTER TABLE repos ADD COLUMN last_processed_sha TEXT;";
