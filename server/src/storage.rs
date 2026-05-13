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

pub fn init_db(path: &Path) -> Result<Connection, rusqlite::Error> {
    if path == Path::new(":memory:") {
        return init_schema(Connection::open_in_memory()?);
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    init_schema(Connection::open(path)?)
}

fn init_schema(conn: Connection) -> Result<Connection, rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            public_key BLOB NOT NULL,
            salt BLOB NOT NULL,
            last_hash BLOB NOT NULL,
            chain_length INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            last_seen INTEGER NOT NULL,
            expires_at INTEGER NOT NULL
        );",
    )?;
    Ok(conn)
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
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}
