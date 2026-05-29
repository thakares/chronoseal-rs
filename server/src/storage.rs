use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStats {
    pub sessions: u64,
    pub expired_sessions: u64,
    pub max_chain_length: u64,
}

pub type DbPool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

pub fn init_pool(path: &Path) -> Result<DbPool, Box<dyn std::error::Error>> {
    let manager = if path == Path::new(":memory:") {
        r2d2_sqlite::SqliteConnectionManager::memory()
    } else {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        r2d2_sqlite::SqliteConnectionManager::file(path)
    };

    let pool = r2d2::Pool::new(manager)?;
    let conn = pool.get()?;
    init_schema(&conn)?;
    Ok(pool)
}

fn init_schema(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            public_key BLOB NOT NULL,
            salt BLOB NOT NULL,
            last_hash BLOB NOT NULL,
            chain_length INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            last_seen INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            gene BLOB NOT NULL DEFAULT X'',
            environment BLOB NOT NULL DEFAULT X'',
            pending_mutation BLOB NOT NULL DEFAULT X'',
            pending_mutation_step INTEGER NOT NULL DEFAULT 0
        );",
    )?;
    ensure_column(
        conn,
        "gene",
        "ALTER TABLE sessions ADD COLUMN gene BLOB NOT NULL DEFAULT X''",
    )?;
    ensure_column(
        conn,
        "environment",
        "ALTER TABLE sessions ADD COLUMN environment BLOB NOT NULL DEFAULT X''",
    )?;
    ensure_column(
        conn,
        "pending_mutation",
        "ALTER TABLE sessions ADD COLUMN pending_mutation BLOB NOT NULL DEFAULT X''",
    )?;
    ensure_column(
        conn,
        "pending_mutation_step",
        "ALTER TABLE sessions ADD COLUMN pending_mutation_step INTEGER NOT NULL DEFAULT 0",
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);",
    )?;
    Ok(())
}

fn ensure_column(
    conn: &rusqlite::Connection,
    column: &str,
    alter_sql: &str,
) -> Result<(), rusqlite::Error> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM pragma_table_info('sessions') WHERE name = ?1
        )",
        [column],
        |row| row.get(0),
    )?;
    if !exists {
        conn.execute_batch(alter_sql)?;
    }
    Ok(())
}

pub fn stats(conn: &Connection) -> Result<StoreStats, rusqlite::Error> {
    let now = current_time_ms();
    let sessions = conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
    let expired_sessions = conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE expires_at < ?1",
        [now],
        |row| row.get(0),
    )?;
    let max_chain_length = conn.query_row(
        "SELECT COALESCE(MAX(chain_length), 0) FROM sessions",
        [],
        |row| row.get(0),
    )?;
    Ok(StoreStats {
        sessions,
        expired_sessions,
        max_chain_length,
    })
}

pub fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
