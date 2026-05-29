use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use valkey::Client as ValkeyClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStats {
    pub sessions: u64,
    pub expired_sessions: u64,
    pub max_chain_length: u64,
}

#[derive(Debug, Clone)]
pub enum DbPool {
    Sqlite(r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>),
    Valkey(ValkeyStore),
}

#[derive(Debug, Clone)]
pub struct ValkeyStore {
    client: Arc<Mutex<ValkeyClient>>,
    index_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub public_key: Vec<u8>,
    pub salt: Vec<u8>,
    pub last_hash: Vec<u8>,
    pub chain_length: u64,
    pub created_at: u64,
    pub last_seen: u64,
    pub expires_at: u64,
    pub gene: Vec<u8>,
    pub environment: Vec<u8>,
    pub pending_mutation: Vec<u8>,
    pub pending_mutation_step: u64,
}

impl DbPool {
    pub fn init(config: &Config) -> Result<Self, Box<dyn std::error::Error>> {
        match config.db_type {
            crate::config::DbType::SqliteInMemory => {
                let pool = init_sqlite_pool(Path::new(":memory:"))?;
                Ok(DbPool::Sqlite(pool))
            }
            crate::config::DbType::SqliteInDisk => {
                let pool = init_sqlite_pool(&config.db_path)?;
                Ok(DbPool::Sqlite(pool))
            }
            crate::config::DbType::Valkey => {
                let addr = std::env::var("CHRONOSEAL_VALKEY_ADDR").unwrap_or_else(|_| "127.0.0.1:6666".to_string());
                match ValkeyClient::connect(addr) {
                    Ok(client) => Ok(DbPool::Valkey(ValkeyStore {
                        client: Arc::new(Mutex::new(client)),
                        index_key: "sessions:ids".to_string(),
                    })),
                    Err(err) => {
                        tracing::warn!("valkey connection failed, falling back to sqlite-in-memory: {err}");
                        let pool = init_sqlite_pool(Path::new(":memory:"))?;
                        Ok(DbPool::Sqlite(pool))
                    }
                }
            }
        }
    }

    pub fn insert_session(&self, record: &SessionRecord) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            DbPool::Sqlite(pool) => {
                let conn = pool.get()?;
                let mut stmt = conn.prepare(
                    "INSERT INTO sessions (
                        session_id, public_key, salt, last_hash, chain_length,
                        created_at, last_seen, expires_at, gene, environment,
                        pending_mutation, pending_mutation_step
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                )?;
                stmt.execute(rusqlite::params![
                    record.session_id,
                    &record.public_key,
                    &record.salt,
                    &record.last_hash,
                    record.chain_length,
                    record.created_at,
                    record.last_seen,
                    record.expires_at,
                    &record.gene,
                    &record.environment,
                    &record.pending_mutation,
                    record.pending_mutation_step,
                ])?;
                Ok(())
            }
            DbPool::Valkey(store) => store.insert_session(record),
        }
    }

    pub fn load_session(&self, session_id: &str) -> Result<Option<SessionRecord>, Box<dyn std::error::Error>> {
        match self {
            DbPool::Sqlite(pool) => {
                let conn = pool.get()?;
                let mut stmt = conn.prepare(
                    "SELECT session_id, public_key, salt, last_hash, chain_length, created_at, last_seen, expires_at, gene, environment, pending_mutation, pending_mutation_step
                     FROM sessions WHERE session_id = ?1",
                )?;
                let row = stmt.query_row([session_id], |row| {
                    Ok(SessionRecord {
                        session_id: row.get(0)?,
                        public_key: row.get(1)?,
                        salt: row.get(2)?,
                        last_hash: row.get(3)?,
                        chain_length: row.get(4)?,
                        created_at: row.get(5)?,
                        last_seen: row.get(6)?,
                        expires_at: row.get(7)?,
                        gene: row.get(8)?,
                        environment: row.get(9)?,
                        pending_mutation: row.get(10)?,
                        pending_mutation_step: row.get(11)?,
                    })
                });
                match row {
                    Ok(rec) => Ok(Some(rec)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(err) => Err(Box::new(err)),
                }
            }
            DbPool::Valkey(store) => store.load_session(session_id),
        }
    }

    pub fn update_session(&self, record: &SessionRecord) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            DbPool::Sqlite(pool) => {
                let conn = pool.get()?;
                conn.execute(
                    "UPDATE sessions SET
                        public_key=?1,
                        salt=?2,
                        last_hash=?3,
                        chain_length=?4,
                        created_at=?5,
                        last_seen=?6,
                        expires_at=?7,
                        gene=?8,
                        environment=?9,
                        pending_mutation=?10,
                        pending_mutation_step=?11
                     WHERE session_id=?12",
                    rusqlite::params![
                        &record.public_key,
                        &record.salt,
                        &record.last_hash,
                        record.chain_length,
                        record.created_at,
                        record.last_seen,
                        record.expires_at,
                        &record.gene,
                        &record.environment,
                        &record.pending_mutation,
                        record.pending_mutation_step,
                        &record.session_id,
                    ],
                )?;
                Ok(())
            }
            DbPool::Valkey(store) => store.insert_session(record),
        }
    }

    pub fn delete_expired_sessions(&self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            DbPool::Sqlite(pool) => {
                let conn = pool.get()?;
                conn.execute(
                    "DELETE FROM sessions WHERE expires_at < ?1",
                    rusqlite::params![current_time_ms()],
                )?;
                Ok(())
            }
            DbPool::Valkey(store) => store.purge_expired_sessions(),
        }
    }

    pub fn stats(&self) -> Result<StoreStats, Box<dyn std::error::Error>> {
        match self {
            DbPool::Sqlite(pool) => {
                let conn = pool.get()?;
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
            DbPool::Valkey(store) => store.stats(),
        }
    }
}

#[cfg(test)]
pub fn init_pool(path: &Path) -> Result<DbPool, Box<dyn std::error::Error>> {
    let pool = init_sqlite_pool(path)?;
    Ok(DbPool::Sqlite(pool))
}

fn init_sqlite_pool(path: &Path) -> Result<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>, Box<dyn std::error::Error>> {
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

impl ValkeyStore {
    fn session_key(&self, session_id: &str) -> String {
        format!("session:{}", session_id)
    }

    fn load_session(&self, session_id: &str) -> Result<Option<SessionRecord>, Box<dyn std::error::Error>> {
        let mut client = self.client.lock().unwrap();
        if let Some(payload) = client.get(&self.session_key(session_id))? {
            let record = serde_json::from_str(&payload)?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    fn insert_session(&self, record: &SessionRecord) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = self.client.lock().unwrap();
        let value = serde_json::to_string(record)?;
        client.set(&self.session_key(&record.session_id), &value)?;
        let existing = client.get(&self.index_key)?;
        let mut ids = existing.unwrap_or_default();
        if !ids.split('\n').any(|id| id == record.session_id) {
            if !ids.is_empty() {
                ids.push('\n');
            }
            ids.push_str(&record.session_id);
            client.set(&self.index_key, &ids)?;
        }
        Ok(())
    }

    fn purge_expired_sessions(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = self.client.lock().unwrap();
        let ids = client.get(&self.index_key)?.unwrap_or_default();
        let now = current_time_ms();
        let mut remaining: Vec<String> = Vec::new();
        for id in ids.split('\n').filter(|id| !id.is_empty()) {
            if let Some(payload) = client.get(&self.session_key(id))? {
                if let Ok(record) = serde_json::from_str::<SessionRecord>(&payload) {
                    if record.expires_at > now {
                        remaining.push(id.to_string());
                    }
                }
            }
        }
        client.set(&self.index_key, &remaining.join("\n"))?;
        Ok(())
    }

    fn stats(&self) -> Result<StoreStats, Box<dyn std::error::Error>> {
        let mut client = self.client.lock().unwrap();
        let ids = client.get(&self.index_key)?.unwrap_or_default();
        let now = current_time_ms();
        let mut sessions = 0;
        let mut expired_sessions = 0;
        let mut max_chain_length = 0;
        for id in ids.split('\n').filter(|id| !id.is_empty()) {
            if let Some(payload) = client.get(&self.session_key(id))? {
                if let Ok(record) = serde_json::from_str::<SessionRecord>(&payload) {
                    sessions += 1;
                    if record.expires_at < now {
                        expired_sessions += 1;
                    }
                    max_chain_length = max_chain_length.max(record.chain_length);
                }
            }
        }
        Ok(StoreStats {
            sessions,
            expired_sessions,
            max_chain_length,
        })
    }
}

pub fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
