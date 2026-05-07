pub struct AppState {
    pub db: tokio::sync::Mutex<rusqlite::Connection>,
    pub rate_limiter: tokio::sync::Mutex<crate::ratelimit::RateLimiter>,
}

use rusqlite::params;
use shared::protocol::{HeartbeatRequest, InitResponse};
use crate::{crypto, trust, fingerprint, vm, storage};

pub fn create_session(
    conn: &rusqlite::Connection,
    pub_key_hex: &str,
) -> Result<InitResponse, Box<dyn std::error::Error>> {
    let pub_key = hex::decode(pub_key_hex)?;
    if pub_key.len() != shared::constants::SESSION_ID_LEN {
        return Err("invalid pubkey len".into());
    }
    let session_id = hex::encode(rand::random::<[u8; shared::constants::SESSION_ID_LEN]>());
    let salt = rand::random::<[u8; shared::constants::SALT_LEN]>();
    let now = storage::current_time_ms();
    let expires_at = now + (shared::constants::EXPIRATION_MINUTES as u64) * 60 * 1000;

    let initial_hash = shared::hashing::initial_hash(&session_id, &pub_key, &salt);

    conn.execute(
        "INSERT INTO sessions (session_id, public_key, salt, last_hash, created_at, last_seen, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![session_id, pub_key, salt.to_vec(), initial_hash, now, now, expires_at],
    )?;

    let opcodes = vm::generate_random_program(8..=16);
    let opcodes_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &opcodes);

    Ok(InitResponse {
        session_id,
        salt: hex::encode(salt),
        opcodes_b64,
        initial_hash: hex::encode(&initial_hash),
        expires_at,
    })
}

pub fn verify_heartbeat(
    conn: &rusqlite::Connection,
    req: &HeartbeatRequest,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut stmt = conn.prepare(
        "SELECT public_key, salt, last_hash, expires_at FROM sessions WHERE session_id = ?1",
    )?;
    let (pub_key, salt, stored_last_hash, expires_at): (Vec<u8>, Vec<u8>, Vec<u8>, u64) =
        stmt.query_row(params![req.session_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;

    let now = storage::current_time_ms();
    if now > expires_at {
        return Err("expired".into());
    }

    // 1. Verify signature
    crypto::verify_signature(&pub_key, req)?;

    // 2. Check chain continuity
    if stored_last_hash != hex::decode(&req.prev_hash)? {
        return Err("chain broken".into());
    }

    // 3. Time window
    let diff = (now as i64) - (req.timestamp as i64);
    if diff.abs() > shared::constants::MAX_TIMESTAMP_DRIFT_MS {
        return Err("timestamp drift".into());
    }

    // 4. Trusted mouse & fingerprint
    trust::validate_mouse(&req.entropy_data)?;
    fingerprint::validate(&req.fingerprint)?;

    // 5. Compute new hash
    let prev_hash_bytes = hex::decode(&req.prev_hash)?;
    let new_hash = shared::hashing::next_chain_hash(
        &prev_hash_bytes,
        req.timestamp,
        &req.entropy_data,
        &req.stack_state,
        &salt,
    );

    // 6. New salt for client
    let next_salt = rand::random::<[u8; shared::constants::SALT_LEN]>();
    let next_salt_hex = hex::encode(next_salt);

    conn.execute(
        "UPDATE sessions SET last_hash=?1, salt=?2, chain_length=chain_length+1, last_seen=?3 WHERE session_id=?4",
        params![new_hash, next_salt.to_vec(), now, req.session_id],
    )?;

    Ok(next_salt_hex)
}