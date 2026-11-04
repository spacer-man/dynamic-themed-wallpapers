//! SQLite connection, schema and migrations.

use std::path::Path;

use rusqlite::Connection;

use crate::error::{Error, Result};

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Store(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Store(format!("json: {e}"))
    }
}

/// Ordered list of migrations. Index + 1 is the schema version. **Append only** — never edit
/// a migration that has shipped.
const MIGRATIONS: &[&str] = &[
    // v1
    "
    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    ) WITHOUT ROWID;

    -- Every photo that became the wallpaper.
    CREATE TABLE history (
        id           INTEGER PRIMARY KEY,
        candidate_id TEXT NOT NULL,
        shown_at     INTEGER NOT NULL,
        dhash        INTEGER NOT NULL,
        place_label  TEXT NOT NULL,
        candidate    TEXT NOT NULL,  -- Candidate as JSON (title, author, license, urls…)
        features     TEXT NOT NULL,  -- Features as JSON, for learning from feedback later
        palette      TEXT NOT NULL   -- Swatches as JSON, for the UI theme
    );
    CREATE INDEX history_shown_at ON history(shown_at DESC);
    CREATE INDEX history_candidate ON history(candidate_id);

    -- Likes and skips. One row per photo: the latest verdict wins.
    CREATE TABLE feedback (
        candidate_id TEXT PRIMARY KEY,
        liked        INTEGER NOT NULL,
        at           INTEGER NOT NULL
    ) WITHOUT ROWID;

    -- Single-row table: the taste model.
    CREATE TABLE model (
        id   INTEGER PRIMARY KEY CHECK (id = 1),
        json TEXT NOT NULL
    );

    -- Rendered wallpapers on disk. status: 'ready' (prefetched, not shown yet) or 'used'.
    CREATE TABLE cache (
        candidate_id TEXT PRIMARY KEY,
        file         TEXT NOT NULL UNIQUE,
        bytes        INTEGER NOT NULL,
        created_at   INTEGER NOT NULL,
        last_used_at INTEGER NOT NULL,
        status       TEXT NOT NULL CHECK (status IN ('ready', 'used')),
        score        REAL NOT NULL,
        dhash        INTEGER NOT NULL,
        candidate    TEXT NOT NULL,
        features     TEXT NOT NULL,
        palette      TEXT NOT NULL
    ) WITHOUT ROWID;
    CREATE INDEX cache_lru ON cache(last_used_at);
    CREATE INDEX cache_ready ON cache(status, score DESC);
    ",
];

pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    // WAL: the app and the background worker may touch the DB at the same time.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    init(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    init(Connection::open_in_memory()?)
}

fn init(mut conn: Connection) -> Result<Connection> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(Error::Store(format!(
            "database schema v{current} is newer than this app understands (v{SCHEMA_VERSION}); \
             update the app"
        )));
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as u32 + 1)?;
        tx.commit()?;
    }
    Ok(())
}
