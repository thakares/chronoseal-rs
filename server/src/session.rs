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
use shared::{
    gene::{self, GeneState},
    protocol::{HeartbeatRequest, InitResponse},
    vm_extensions,
};

#[derive(Debug, Clone)]
pub struct HeartbeatVerificationResult {
    pub next_salt_hex: String,
    pub next_mutation_step: u64,
    pub next_mutation_order_b64: String,
}

pub fn create_session(
    conn: &rusqlite::Connection,
    config: &crate::config::Config,
    pub_key_hex: &str,
) -> Result<InitResponse, crate::errors::SessionError> {
    let pub_key = hex::decode(pub_key_hex)?;
    if pub_key.len() != shared::constants::SESSION_ID_LEN {
        return Err(crate::errors::SessionError::InvalidPublicKeyLength);
    }

    let gene_state = gene::new_state(config.gene_size)
        .map_err(|err| crate::errors::SessionError::InvalidGeneConfiguration(err.to_string()))?;
    let environment_blob = gene::encode_environment(&gene_state.environment)
        .map_err(|err| crate::errors::SessionError::InvalidGeneConfiguration(err.to_string()))?;

    let session_id = hex::encode(rand::random::<[u8; shared::constants::SESSION_ID_LEN]>());
    let salt = rand::random::<[u8; shared::constants::SALT_LEN]>();
    let now = storage::current_time_ms();
    let expires_at = now + (config.expiration_minutes as u64) * 60 * 1000;
    let initial_hash = shared::hashing::initial_hash(&session_id, &pub_key, &salt);

    let opcodes = vm::generate_random_program(8..=16);
    let opcodes_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &opcodes);

    let initial_mutation = vm_extensions::generate_order(1, config.gene_size);
    let initial_mutation_b64 = vm_extensions::encode_order_b64(&initial_mutation);

    conn.execute(
        "INSERT INTO sessions (
            session_id, public_key, salt, last_hash, chain_length, created_at, last_seen, expires_at,
            gene, environment, pending_mutation, pending_mutation_step
        ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            session_id,
            pub_key,
            salt.to_vec(),
            initial_hash,
            now,
            now,
            expires_at,
            gene_state.gene,
            environment_blob,
            initial_mutation.program,
            initial_mutation.step,
        ],
    )?;

    Ok(InitResponse {
        session_id,
        salt: hex::encode(salt),
        opcodes_b64,
        initial_hash: hex::encode(&initial_hash),
        expires_at,
        heartbeat_min_interval_ms: config.heartbeat_min_interval_ms,
        heartbeat_max_interval_ms: config.heartbeat_max_interval_ms,
        gene_size: config.gene_size as u32,
        mutation_step: initial_mutation.step,
        mutation_order_b64: initial_mutation_b64,
    })
}

pub fn verify_heartbeat(
    conn: &rusqlite::Connection,
    config: &crate::config::Config,
    req: &HeartbeatRequest,
) -> Result<HeartbeatVerificationResult, crate::errors::VerificationError> {
    let mut stmt = conn.prepare(
        "SELECT public_key, salt, last_hash, expires_at, gene, environment, pending_mutation, pending_mutation_step
         FROM sessions WHERE session_id = ?1",
    )?;
    let (
        pub_key,
        salt,
        stored_last_hash,
        expires_at,
        gene_blob,
        environment_blob,
        pending_mutation,
        pending_step,
    ): (
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        u64,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        u64,
    ) = stmt
        .query_row(params![req.session_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
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
    let prev_hash_bytes = hex::decode(&req.prev_hash)?;
    if stored_last_hash != prev_hash_bytes {
        return Err(crate::errors::VerificationError::ChainBroken);
    }

    // 3. Mutation step and deterministic mutation parity
    if req.mutation_step != pending_step {
        return Err(crate::errors::VerificationError::MutationStepMismatch {
            expected: pending_step,
            got: req.mutation_step,
        });
    }

    let environment = gene::decode_environment(&environment_blob)
        .map_err(|e| crate::errors::VerificationError::GeneState(e.to_string()))?;
    let server_state = GeneState {
        gene: gene_blob,
        environment,
    };
    let candidate_state = vm_extensions::apply_program_clone(&server_state, &pending_mutation)
        .map_err(|e| crate::errors::VerificationError::MutationProgram(e.to_string()))?;
    let expected_gene_commitment = gene::commitment_hex(&candidate_state);
    if req.gene_commitment != expected_gene_commitment {
        return Err(crate::errors::VerificationError::MutationCommitmentMismatch);
    }

    // 4. Time window
    let diff = (now as i64) - (req.timestamp as i64);
    if diff.abs() > config.max_timestamp_drift_ms {
        return Err(crate::errors::VerificationError::TimestampDrift);
    }

    // 5. Trusted mouse & fingerprint
    trust::validate_mouse(&req.entropy_data, config)
        .map_err(|e| crate::errors::VerificationError::TrustFailed(e.to_string()))?;
    fingerprint::validate(&req.fingerprint)
        .map_err(|e| crate::errors::VerificationError::FingerprintFailed(e.to_string()))?;

    // 6. Compute new hash
    let new_hash = shared::hashing::next_chain_hash(
        &prev_hash_bytes,
        req.timestamp,
        &req.entropy_data,
        &req.stack_state,
        &salt,
    );

    // 7. Prepare next mutation order and salt
    let next_step = pending_step + 1;
    let next_mutation = vm_extensions::generate_order(next_step, candidate_state.gene.len());
    let next_mutation_b64 = vm_extensions::encode_order_b64(&next_mutation);

    let next_salt = rand::random::<[u8; shared::constants::SALT_LEN]>();
    let next_salt_hex = hex::encode(next_salt);
    let next_environment_blob = gene::encode_environment(&candidate_state.environment)
        .map_err(|e| crate::errors::VerificationError::GeneState(e.to_string()))?;

    conn.execute(
        "UPDATE sessions SET
            last_hash=?1,
            salt=?2,
            chain_length=chain_length+1,
            last_seen=?3,
            gene=?4,
            environment=?5,
            pending_mutation=?6,
            pending_mutation_step=?7
         WHERE session_id=?8",
        params![
            new_hash,
            next_salt.to_vec(),
            now,
            candidate_state.gene,
            next_environment_blob,
            next_mutation.program,
            next_step,
            req.session_id
        ],
    )?;

    Ok(HeartbeatVerificationResult {
        next_salt_hex,
        next_mutation_step: next_step,
        next_mutation_order_b64: next_mutation_b64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use shared::protocol::{EntropyData, Fingerprint, HeartbeatRequest, MouseEvent, StackState};
    use std::path::Path;

    #[derive(Clone)]
    struct SimulatedClient {
        signing_key: SigningKey,
        session_id: String,
        prev_hash: String,
        current_salt: String,
        pending_mutation_step: u64,
        pending_mutation_order_b64: String,
        committed_gene_state: GeneState,
    }

    fn test_config() -> crate::config::Config {
        crate::config::Config {
            expiration_minutes: 30,
            max_timestamp_drift_ms: 30_000,
            min_mouse_total_dist: 1.0,
            max_mouse_avg_speed: 4.0,
            min_pause_count: 0,
            require_mouse_activity: false,
            gene_size: 64,
            ..crate::config::Config::default()
        }
    }

    fn test_entropy() -> EntropyData {
        EntropyData {
            events: vec![
                MouseEvent {
                    x: 1.0,
                    y: 1.0,
                    timestamp_ms: 1.0,
                },
                MouseEvent {
                    x: 2.0,
                    y: 1.0,
                    timestamp_ms: 2.0,
                },
                MouseEvent {
                    x: 2.0,
                    y: 1.0,
                    timestamp_ms: 120.0,
                },
            ],
        }
    }

    fn test_stack() -> StackState {
        StackState {
            stack: vec![42, 7, 99],
            ip: 3,
        }
    }

    fn test_fingerprint() -> Fingerprint {
        Fingerprint {
            aspect_ratio: "1.77".to_string(),
            device_pixel_ratio: "2.0".to_string(),
            hardware_concurrency: 8,
        }
    }

    fn sign_request(sk: &SigningKey, req: &mut HeartbeatRequest) {
        let message = crate::crypto::canonical_signing_message(req).unwrap();
        let sig = sk.sign(message.as_bytes());
        req.signature = hex::encode(sig.to_bytes());
    }

    fn create_test_session(
        conn: &rusqlite::Connection,
        config: &crate::config::Config,
    ) -> (InitResponse, SigningKey) {
        let mut rng = rand::thread_rng();
        let sk = SigningKey::generate(&mut rng);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());
        let init = create_session(conn, config, &pk_hex).unwrap();
        (init, sk)
    }

    fn client_from_init(init: &InitResponse, signing_key: SigningKey) -> SimulatedClient {
        SimulatedClient {
            signing_key,
            session_id: init.session_id.clone(),
            prev_hash: init.initial_hash.clone(),
            current_salt: init.salt.clone(),
            pending_mutation_step: init.mutation_step,
            pending_mutation_order_b64: init.mutation_order_b64.clone(),
            committed_gene_state: gene::new_state(init.gene_size as usize).unwrap(),
        }
    }

    fn build_request(
        client: &SimulatedClient,
        timestamp: u64,
    ) -> (HeartbeatRequest, GeneState, EntropyData, StackState) {
        let order = vm_extensions::decode_order_b64(
            client.pending_mutation_step,
            &client.pending_mutation_order_b64,
        )
        .unwrap();
        let candidate_state =
            vm_extensions::apply_program_clone(&client.committed_gene_state, &order.program)
                .unwrap();
        let entropy = test_entropy();
        let stack = test_stack();

        let mut req = HeartbeatRequest {
            session_id: client.session_id.clone(),
            prev_hash: client.prev_hash.clone(),
            timestamp,
            entropy_data: entropy.clone(),
            stack_state: stack.clone(),
            fingerprint: test_fingerprint(),
            mutation_step: client.pending_mutation_step,
            gene_commitment: gene::commitment_hex(&candidate_state),
            signature: String::new(),
        };
        sign_request(&client.signing_key, &mut req);
        (req, candidate_state, entropy, stack)
    }

    fn apply_successful_response(
        client: &mut SimulatedClient,
        req: &HeartbeatRequest,
        candidate_state: GeneState,
        entropy: &EntropyData,
        stack: &StackState,
        resp: &HeartbeatVerificationResult,
    ) {
        let salt = hex::decode(&client.current_salt).unwrap();
        let prev_hash = hex::decode(&req.prev_hash).unwrap();
        let next_hash =
            shared::hashing::next_chain_hash(&prev_hash, req.timestamp, entropy, stack, &salt);

        client.prev_hash = hex::encode(next_hash);
        client.current_salt = resp.next_salt_hex.clone();
        client.pending_mutation_step = resp.next_mutation_step;
        client.pending_mutation_order_b64 = resp.next_mutation_order_b64.clone();
        client.committed_gene_state = candidate_state;
    }

    fn load_server_gene_state(conn: &rusqlite::Connection, session_id: &str) -> GeneState {
        let (gene_blob, env_blob): (Vec<u8>, Vec<u8>) = conn
            .query_row(
                "SELECT gene, environment FROM sessions WHERE session_id=?1",
                [session_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        GeneState {
            gene: gene_blob,
            environment: gene::decode_environment(&env_blob).unwrap(),
        }
    }

    fn run_successful_heartbeat(
        conn: &rusqlite::Connection,
        config: &crate::config::Config,
        client: &mut SimulatedClient,
    ) -> HeartbeatRequest {
        let timestamp = storage::current_time_ms();
        let (req, candidate_state, entropy, stack) = build_request(client, timestamp);
        let result = verify_heartbeat(conn, config, &req).unwrap();
        apply_successful_response(client, &req, candidate_state, &entropy, &stack, &result);
        req
    }

    #[test]
    fn test_session_lifecycle_and_verification() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();
        let config = test_config();

        let (init, signing_key) = create_test_session(&conn, &config);
        assert_eq!(init.gene_size, config.gene_size as u32);
        assert!(!init.mutation_order_b64.is_empty());
        assert_eq!(init.mutation_step, 1);

        let mut client = client_from_init(&init, signing_key);
        for _ in 0..5 {
            run_successful_heartbeat(&conn, &config, &mut client);
        }

        let stats = storage::stats(&conn).unwrap();
        assert_eq!(stats.sessions, 1);
        assert_eq!(stats.max_chain_length, 6);
    }

    #[test]
    fn test_deterministic_server_client_parity_across_many_heartbeats() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&conn, &config);
        let mut client = client_from_init(&init, signing_key);

        for _ in 0..12 {
            run_successful_heartbeat(&conn, &config, &mut client);
            let server_state = load_server_gene_state(&conn, &client.session_id);
            assert_eq!(server_state, client.committed_gene_state);
        }
    }

    #[test]
    fn test_replay_attack_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&conn, &config);
        let mut client = client_from_init(&init, signing_key);

        let timestamp = storage::current_time_ms();
        let (req, candidate_state, entropy, stack) = build_request(&client, timestamp);
        let result = verify_heartbeat(&conn, &config, &req).unwrap();
        apply_successful_response(
            &mut client,
            &req,
            candidate_state,
            &entropy,
            &stack,
            &result,
        );

        let replay = verify_heartbeat(&conn, &config, &req);
        assert!(matches!(
            replay.unwrap_err(),
            crate::errors::VerificationError::ChainBroken
        ));
    }

    #[test]
    fn test_mutation_step_mismatch_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&conn, &config);
        let client = client_from_init(&init, signing_key);

        let timestamp = storage::current_time_ms();
        let (mut req, _, _, _) = build_request(&client, timestamp);
        req.mutation_step += 1;
        sign_request(&client.signing_key, &mut req);

        let err = verify_heartbeat(&conn, &config, &req).unwrap_err();
        assert!(matches!(
            err,
            crate::errors::VerificationError::MutationStepMismatch { .. }
        ));
    }

    #[test]
    fn test_mutation_commitment_tamper_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&conn, &config);
        let client = client_from_init(&init, signing_key);

        let timestamp = storage::current_time_ms();
        let (mut req, _, _, _) = build_request(&client, timestamp);
        req.gene_commitment = "00".repeat(32);
        sign_request(&client.signing_key, &mut req);

        let err = verify_heartbeat(&conn, &config, &req).unwrap_err();
        assert!(matches!(
            err,
            crate::errors::VerificationError::MutationCommitmentMismatch
        ));
    }

    #[test]
    fn test_malformed_server_mutation_program_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&conn, &config);
        let client = client_from_init(&init, signing_key);

        conn.execute(
            "UPDATE sessions SET pending_mutation=?1 WHERE session_id=?2",
            params![vec![0xFFu8], client.session_id.clone()],
        )
        .unwrap();

        let timestamp = storage::current_time_ms();
        let (req, _, _, _) = build_request(&client, timestamp);
        let err = verify_heartbeat(&conn, &config, &req).unwrap_err();
        assert!(matches!(
            err,
            crate::errors::VerificationError::MutationProgram(_)
        ));
    }

    #[test]
    fn test_expired_session_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&conn, &config);
        let client = client_from_init(&init, signing_key);

        conn.execute(
            "UPDATE sessions SET expires_at=?1 WHERE session_id=?2",
            params![0u64, client.session_id.clone()],
        )
        .unwrap();

        let timestamp = storage::current_time_ms();
        let (req, _, _, _) = build_request(&client, timestamp);
        let err = verify_heartbeat(&conn, &config, &req).unwrap_err();
        assert!(matches!(err, crate::errors::VerificationError::Expired));
    }

    #[test]
    fn test_create_session_rejects_invalid_public_key_length() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();
        let config = test_config();
        let err = create_session(&conn, &config, "00ff").unwrap_err();
        assert!(matches!(
            err,
            crate::errors::SessionError::InvalidPublicKeyLength
        ));
    }

    #[test]
    fn test_stale_mutation_step_after_success_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&conn, &config);
        let mut client = client_from_init(&init, signing_key);

        run_successful_heartbeat(&conn, &config, &mut client);

        let timestamp = storage::current_time_ms();
        let (mut req, _, _, _) = build_request(&client, timestamp);
        req.mutation_step -= 1;
        sign_request(&client.signing_key, &mut req);

        let err = verify_heartbeat(&conn, &config, &req).unwrap_err();
        assert!(matches!(
            err,
            crate::errors::VerificationError::MutationStepMismatch { .. }
        ));
    }

    #[test]
    fn test_repeated_simulation_keeps_server_and_client_commitments_equal() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = pool.get().unwrap();
        let mut config = test_config();
        config.gene_size = 128;
        let (init, signing_key) = create_test_session(&conn, &config);
        let mut client = client_from_init(&init, signing_key);

        for _ in 0..10 {
            run_successful_heartbeat(&conn, &config, &mut client);
            let server_state = load_server_gene_state(&conn, &client.session_id);
            assert_eq!(
                gene::commitment(&server_state),
                gene::commitment(&client.committed_gene_state)
            );
        }
    }
}
