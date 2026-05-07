use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn init_db() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open_in_memory()?;
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

pub fn current_time_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}