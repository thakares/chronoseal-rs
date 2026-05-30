use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use redis::Commands;

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
    pool: r2d2::Pool<redis::Client>,
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
    pub opcodes: Vec<u8>,
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
                let addr = std::env::var("CHRONOSEAL_VALKEY_ADDR")
                    .unwrap_or_else(|_| "127.0.0.1:6666".to_string());
                let connection_string = if addr.starts_with("redis://") || addr.starts_with("rediss://") {
                    addr.clone()
                } else {
                    format!("redis://{}", addr)
                };
                let client = redis::Client::open(connection_string)?;
                let pool = r2d2::Pool::builder().build(client)?;
                Ok(DbPool::Valkey(ValkeyStore {
                    pool,
                    index_key: "sessions:ids".to_string(),
                }))
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
                        pending_mutation, pending_mutation_step, opcodes
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                    &record.opcodes,
                ])?;
                Ok(())
            }
            DbPool::Valkey(store) => store.insert_session(record),
        }
    }

    pub fn load_session(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionRecord>, Box<dyn std::error::Error>> {
        match self {
            DbPool::Sqlite(pool) => {
                let conn = pool.get()?;
                let mut stmt = conn.prepare(
                    "SELECT session_id, public_key, salt, last_hash, chain_length, created_at, last_seen, expires_at, gene, environment, pending_mutation, pending_mutation_step, opcodes
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
                        opcodes: row.get(12)?,
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

    pub fn update_session(
        &self,
        record: &SessionRecord,
        old_last_hash: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            DbPool::Sqlite(pool) => {
                let conn = pool.get()?;
                let rows = conn.execute(
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
                        pending_mutation_step=?11,
                        opcodes=?12
                     WHERE session_id=?13 AND last_hash=?14",
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
                        &record.opcodes,
                        &record.session_id,
                        old_last_hash,
                    ],
                )?;
                if rows == 0 {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "Concurrent update detected (CAS failed)",
                    )));
                }
                Ok(())
            }
            DbPool::Valkey(store) => store.update_session_cas(record, old_last_hash),
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
                let sessions =
                    conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
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

fn init_sqlite_pool(
    path: &Path,
) -> Result<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>, Box<dyn std::error::Error>> {
    let manager = if path == Path::new(":memory:") {
        r2d2_sqlite::SqliteConnectionManager::memory()
    } else {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        r2d2_sqlite::SqliteConnectionManager::file(path)
    };
    let manager = manager.with_init(|conn| {
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        Ok(())
    });
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
            pending_mutation_step INTEGER NOT NULL DEFAULT 0,
            opcodes BLOB NOT NULL DEFAULT X''
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
    ensure_column(
        conn,
        "opcodes",
        "ALTER TABLE sessions ADD COLUMN opcodes BLOB NOT NULL DEFAULT X''",
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

    fn load_session(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionRecord>, Box<dyn std::error::Error>> {
        let mut conn = self.pool.get()?;
        let key = self.session_key(session_id);
        let payload: Option<String> = conn.get(&key)?;
        match payload {
            Some(p) => Ok(serde_json::from_str(&p)?),
            None => Ok(None),
        }
    }

    fn insert_session(&self, record: &SessionRecord) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = self.pool.get()?;
        let key = self.session_key(&record.session_id);
        let value = serde_json::to_string(record)?;
        let now = current_time_ms();
        let ttl_seconds = (record.expires_at.saturating_sub(now) / 1000).max(1);

        redis::pipe()
            .atomic()
            .cmd("SET").arg(&key).arg(&value).arg("EX").arg(ttl_seconds)
            .cmd("ZADD").arg(&self.index_key).arg(record.expires_at).arg(&record.session_id)
            .cmd("ZADD").arg("sessions:chain_lengths").arg(record.chain_length).arg(&record.session_id)
            .query::<()>(&mut *conn)?;
        Ok(())
    }

    fn update_session_cas(
        &self,
        record: &SessionRecord,
        old_last_hash: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = self.pool.get()?;
        let key = self.session_key(&record.session_id);

        // Watch key for concurrent modification
        redis::cmd("WATCH").arg(&key).query::<()>(&mut *conn)?;

        // Fetch current and verify last_hash matches
        let payload: Option<String> = conn.get(&key)?;
        match payload {
            Some(p) => {
                let current_record: SessionRecord = serde_json::from_str(&p)?;
                if current_record.last_hash != old_last_hash {
                    redis::cmd("UNWATCH").query::<()>(&mut *conn)?;
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "Concurrent update detected (CAS failed in Valkey)",
                    )));
                }
            }
            None => {
                redis::cmd("UNWATCH").query::<()>(&mut *conn)?;
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Session not found for update in Valkey",
                )));
            }
        }

        let value = serde_json::to_string(record)?;
        let now = current_time_ms();
        let ttl_seconds = (record.expires_at.saturating_sub(now) / 1000).max(1);

        let response: Option<()> = redis::pipe()
            .atomic()
            .cmd("SET").arg(&key).arg(&value).arg("EX").arg(ttl_seconds)
            .cmd("ZADD").arg(&self.index_key).arg(record.expires_at).arg(&record.session_id)
            .cmd("ZADD").arg("sessions:chain_lengths").arg(record.chain_length).arg(&record.session_id)
            .query(&mut *conn)?;

        match response {
            Some(_) => Ok(()),
            None => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Transaction aborted due to concurrent modification",
            ))),
        }
    }

    fn purge_expired_sessions(&self) -> Result<(), Box<dyn std::error::Error>> {
        let now = current_time_ms();
        let mut conn = self.pool.get()?;
        // Fetch expired session IDs
        let expired_ids: Vec<String> = conn.zrangebyscore(&self.index_key, 0, now)?;
        if !expired_ids.is_empty() {
            redis::pipe()
                .atomic()
                .cmd("ZREM").arg(&self.index_key).arg(&expired_ids)
                .cmd("ZREM").arg("sessions:chain_lengths").arg(&expired_ids)
                .query::<()>(&mut *conn)?;
        }
        Ok(())
    }

    fn stats(&self) -> Result<StoreStats, Box<dyn std::error::Error>> {
        let now = current_time_ms();
        let mut conn = self.pool.get()?;
        let sessions: u64 = conn.zcard(&self.index_key)?;
        let expired_sessions: u64 = conn.zcount(&self.index_key, 0, now)?;

        let max_chain_length_res: Vec<(String, u64)> = conn.zrevrange_withscores("sessions:chain_lengths", 0, 0)?;
        let max_chain_length = max_chain_length_res.first().map(|(_, score)| *score).unwrap_or(0);

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

#[cfg(test)]
mod valkey_tests {
    use super::*;

    #[test]
    fn test_valkey_store_operations() {
        let addr = std::env::var("CHRONOSEAL_VALKEY_ADDR").unwrap_or_else(|_| "127.0.0.1:6379".to_string());
        let connection_string = format!("redis://{}", addr);
        let client = match redis::Client::open(connection_string) {
            Ok(c) => c,
            Err(_) => return,
        };
        let pool = match r2d2::Pool::builder().build(client) {
            Ok(p) => p,
            Err(_) => return,
        };
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        let _: () = match redis::cmd("PING").query(&mut *conn) {
            Ok(res) => res,
            Err(_) => return,
        };

        let store = ValkeyStore {
            pool,
            index_key: "test:sessions:ids".to_string(),
        };

        let _: Result<(), _> = conn.del("test:sessions:ids");

        let session_id = "test_session_123".to_string();
        let record = SessionRecord {
            session_id: session_id.clone(),
            public_key: vec![1, 2, 3],
            salt: vec![4, 5, 6],
            last_hash: vec![7, 8, 9],
            chain_length: 10,
            created_at: 1000,
            last_seen: 2000,
            expires_at: current_time_ms() + 10000,
            gene: vec![11],
            environment: vec![12],
            pending_mutation: vec![13],
            pending_mutation_step: 14,
            opcodes: vec![],
        };

        store.insert_session(&record).unwrap();

        let loaded = store.load_session(&session_id).unwrap().unwrap();
        assert_eq!(loaded.session_id, session_id);
        assert_eq!(loaded.chain_length, 10);

        let stats = store.stats().unwrap();
        assert_eq!(stats.sessions, 1);
        assert_eq!(stats.max_chain_length, 10);

        store.purge_expired_sessions().unwrap();
        let stats = store.stats().unwrap();
        assert_eq!(stats.sessions, 1);

        let _: Result<(), _> = conn.del(store.session_key(&session_id));
        let _: Result<(), _> = conn.del(&store.index_key);
    }

    #[test]
    fn test_valkey_pool_concurrency() {
        use std::thread;
        use std::sync::Arc;

        let addr = std::env::var("CHRONOSEAL_VALKEY_ADDR").unwrap_or_else(|_| "127.0.0.1:6379".to_string());
        let connection_string = format!("redis://{}", addr);
        let client = match redis::Client::open(connection_string) {
            Ok(c) => c,
            Err(_) => return,
        };
        let pool = match r2d2::Pool::builder().build(client) {
            Ok(p) => p,
            Err(_) => return,
        };
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        let _: () = match redis::cmd("PING").query(&mut *conn) {
            Ok(res) => res,
            Err(_) => return,
        };

        let store = ValkeyStore {
            pool,
            index_key: "test:concurrent:sessions:ids".to_string(),
        };
        let _: Result<(), _> = conn.del("test:concurrent:sessions:ids");

        let store_arc = Arc::new(store);
        let mut handles = Vec::new();

        for t in 0..10 {
            let store_clone = store_arc.clone();
            let session_id = format!("valkey_concurrent_{}", t);
            let handle = thread::spawn(move || {
                let record = SessionRecord {
                    session_id: session_id.clone(),
                    public_key: vec![1, 2, 3],
                    salt: vec![4, 5, 6],
                    last_hash: vec![7, 8, 9],
                    chain_length: 1,
                    created_at: 1000,
                    last_seen: 2000,
                    expires_at: current_time_ms() + 10000,
                    gene: vec![11],
                    environment: vec![12],
                    pending_mutation: vec![13],
                    pending_mutation_step: 14,
                    opcodes: vec![],
                };
                store_clone.insert_session(&record).unwrap();
                let loaded = store_clone.load_session(&session_id).unwrap().unwrap();
                assert_eq!(loaded.session_id, session_id);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = store_arc.stats().unwrap();
        assert_eq!(stats.sessions, 10);

        // Cleanup
        let mut conn = store_arc.pool.get().unwrap();
        for t in 0..10 {
            let _: Result<(), _> = conn.del(store_arc.session_key(&format!("valkey_concurrent_{}", t)));
        }
        let _: Result<(), _> = conn.del(&store_arc.index_key);
    }
}

#[cfg(test)]
mod sqlite_tests {
    use super::*;
    use std::thread;
    use std::sync::Arc;

    #[test]
    fn test_sqlite_pool_concurrency() {
        let db_path = Path::new("target/test_sqlite_concurrency.db");
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(db_path);

        let pool = init_pool(db_path).unwrap();
        let pool_arc = Arc::new(pool);
        let mut handles = Vec::new();

        for t in 0..10 {
            let pool_clone = pool_arc.clone();
            let handle = thread::spawn(move || {
                let session_id = format!("concurrent_session_{}", t);
                let record = SessionRecord {
                    session_id: session_id.clone(),
                    public_key: vec![1, 2, 3],
                    salt: vec![4, 5, 6],
                    last_hash: vec![7, 8, 9],
                    chain_length: 1,
                    created_at: 1000,
                    last_seen: 2000,
                    expires_at: current_time_ms() + 10000,
                    gene: vec![11],
                    environment: vec![12],
                    pending_mutation: vec![13],
                    pending_mutation_step: 14,
                    opcodes: vec![],
                };
                pool_clone.insert_session(&record).unwrap();
                let loaded = pool_clone.load_session(&session_id).unwrap().unwrap();
                assert_eq!(loaded.session_id, session_id);

                let mut updated = loaded;
                updated.chain_length = 2;
                let old_hash = updated.last_hash.clone();
                pool_clone.update_session(&updated, &old_hash).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = pool_arc.stats().unwrap();
        assert_eq!(stats.sessions, 10);

        std::mem::drop(pool_arc);
        let _ = std::fs::remove_file(db_path);
    }
}


