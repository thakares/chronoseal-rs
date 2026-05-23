pub struct AppState {
    pub db_pool: crate::storage::DbPool,
    pub rate_limiter: tokio::sync::Mutex<crate::ratelimit::RateLimiter>,
    pub config: std::sync::RwLock<crate::config::Config>,
}

impl AppState {
    pub fn get_config(&self) -> crate::config::Config {
        if let Ok(cfg) = self.config.read() {
            cfg.clone()
        } else {
            crate::config::Config::default()
        }
    }
}

use crate::{crypto, fingerprint, storage, trust, vm};
use rusqlite::params;
use shared::protocol::{HeartbeatRequest, InitResponse};

pub fn create_session(
    conn: &rusqlite::Connection,
    config: &crate::config::Config,
    pub_key_hex: &str,
) -> Result<InitResponse, crate::errors::SessionError> {
    let pub_key = hex::decode(pub_key_hex)?;
    if pub_key.len() != shared::constants::SESSION_ID_LEN {
        return Err(crate::errors::SessionError::InvalidPublicKeyLength);
    }
    let session_id = hex::encode(rand::random::<[u8; shared::constants::SESSION_ID_LEN]>());
    let salt = rand::random::<[u8; shared::constants::SALT_LEN]>();
    let now = storage::current_time_ms();
    let expires_at = now + (config.expiration_minutes as u64) * 60 * 1000;

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
        heartbeat_min_interval_ms: config.heartbeat_min_interval_ms,
        heartbeat_max_interval_ms: config.heartbeat_max_interval_ms,
    })
}

pub fn verify_heartbeat(
    conn: &rusqlite::Connection,
    config: &crate::config::Config,
    req: &HeartbeatRequest,
) -> Result<String, crate::errors::VerificationError> {
    let mut stmt = conn.prepare(
        "SELECT public_key, salt, last_hash, expires_at FROM sessions WHERE session_id = ?1",
    )?;
    let (pub_key, salt, stored_last_hash, expires_at): (Vec<u8>, Vec<u8>, Vec<u8>, u64) = stmt
        .query_row(params![req.session_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|e| {
            if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                crate::errors::VerificationError::SessionNotFound
            } else {
                crate::errors::VerificationError::Database(e)
            }
        })?;

    let now = storage::current_time_ms();
    if now > expires_at {
        return Err(crate::errors::VerificationError::Expired);
    }

    // 1. Verify signature
    crypto::verify_signature(&pub_key, req)
        .map_err(|e| crate::errors::VerificationError::Signature(e.to_string()))?;

    // 2. Check chain continuity
    if stored_last_hash != hex::decode(&req.prev_hash)? {
        return Err(crate::errors::VerificationError::ChainBroken);
    }

    // 3. Time window
    let diff = (now as i64) - (req.timestamp as i64);
    if diff.abs() > config.max_timestamp_drift_ms {
        return Err(crate::errors::VerificationError::TimestampDrift);
    }

    // 4. Trusted mouse & fingerprint
    trust::validate_mouse(&req.entropy_data, config)
        .map_err(|e| crate::errors::VerificationError::TrustFailed(e.to_string()))?;
    fingerprint::validate(&req.fingerprint)
        .map_err(|e| crate::errors::VerificationError::FingerprintFailed(e.to_string()))?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use shared::protocol::{EntropyData, Fingerprint, HeartbeatRequest, StackState};
    use std::path::Path;

    fn sign_request(sk: &SigningKey, req: &mut HeartbeatRequest) {
        let mut payload: std::collections::BTreeMap<&str, serde_json::Value> =
            std::collections::BTreeMap::new();
        payload.insert(
            "entropyData",
            serde_json::to_value(&req.entropy_data).unwrap(),
        );
        payload.insert(
            "fingerprint",
            serde_json::to_value(&req.fingerprint).unwrap(),
        );
        payload.insert("prevHash", serde_json::json!(req.prev_hash));
        payload.insert("sessionId", serde_json::json!(req.session_id));
        payload.insert(
            "stackState",
            serde_json::to_value(&req.stack_state).unwrap(),
        );
        payload.insert("timestamp", serde_json::json!(req.timestamp));
        let message = serde_json::to_string(&payload).unwrap();
        let sig = sk.sign(message.as_bytes());
        req.signature = hex::encode(sig.to_bytes());
    }

    #[test]
    fn test_session_lifecycle_and_verification() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();

        let config = crate::config::Config {
            expiration_minutes: 30,
            max_timestamp_drift_ms: 30000,
            min_mouse_total_dist: 10.0,
            max_mouse_avg_speed: 2.0,
            min_pause_count: 1,
            require_mouse_activity: false, // simpler for tests
            ..crate::config::Config::default()
        };

        // Generate Ed25519 keypair
        let mut rng = rand::thread_rng();
        let sk = SigningKey::generate(&mut rng);
        let pk = sk.verifying_key();
        let pub_key_hex = hex::encode(pk.to_bytes());

        // 1. Create Session
        let start_time = storage::current_time_ms();
        let init_resp = create_session(&conn, &config, &pub_key_hex).unwrap();
        assert!(init_resp.expires_at >= start_time + 30 * 60 * 1000);
        assert!(init_resp.expires_at <= storage::current_time_ms() + 30 * 60 * 1000);

        // Verify stats
        let stats = storage::stats(&conn).unwrap();
        assert_eq!(stats.sessions, 1);
        assert_eq!(stats.expired_sessions, 0);

        // 2. Heartbeat Verification
        let now = storage::current_time_ms();
        let entropy_data = EntropyData { events: vec![] };
        let stack_state = StackState {
            stack: vec![42],
            ip: 5,
        };
        let fingerprint = Fingerprint {
            aspect_ratio: "1.77".to_string(),
            device_pixel_ratio: "2.0".to_string(),
            hardware_concurrency: 8,
        };

        let mut req = HeartbeatRequest {
            session_id: init_resp.session_id.clone(),
            prev_hash: init_resp.initial_hash.clone(),
            timestamp: now,
            entropy_data,
            stack_state,
            fingerprint,
            signature: "".to_string(),
        };

        sign_request(&sk, &mut req);

        // Verify successful heartbeat
        let next_salt = verify_heartbeat(&conn, &config, &req).unwrap();
        assert!(!next_salt.is_empty());

        // Try duplicate/broken hash chain (prev_hash unchanged but expected next hash in DB)
        let res = verify_heartbeat(&conn, &config, &req);
        assert!(res.is_err());
        assert!(matches!(
            res.unwrap_err(),
            crate::errors::VerificationError::ChainBroken
        ));
    }
}
