pub struct AppState {
    pub db_pool: crate::storage::DbPool,
    pub rate_limiter: crate::ratelimit::RateLimiter,
    pub config: std::sync::RwLock<crate::config::Config>,
    pub heartbeats_total: std::sync::atomic::AtomicU64,
    pub verification_failures_total: std::sync::atomic::AtomicU64,
    pub mutation_failures_total: std::sync::atomic::AtomicU64,
    pub replay_attempts_total: std::sync::atomic::AtomicU64,
    pub storage_latency_ns: std::sync::atomic::AtomicU64,
    pub storage_ops_count: std::sync::atomic::AtomicU64,
    pub http_latency_ns: std::sync::atomic::AtomicU64,
    pub http_ops_count: std::sync::atomic::AtomicU64,
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
    db: &storage::DbPool,
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

    let record = storage::SessionRecord {
        session_id: session_id.clone(),
        public_key: pub_key,
        salt: salt.to_vec(),
        last_hash: initial_hash.clone(),
        chain_length: 1,
        created_at: now,
        last_seen: now,
        expires_at,
        gene: gene_state.gene,
        environment: environment_blob,
        pending_mutation: initial_mutation.program,
        pending_mutation_step: initial_mutation.step,
        opcodes,
    };

    db.insert_session(&record)
        .map_err(|err| crate::errors::SessionError::Storage(err.to_string()))?;

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
        mutation_rounds: config.mutation_rounds,
    })
}

pub fn verify_heartbeat(
    db: &storage::DbPool,
    config: &crate::config::Config,
    req: &HeartbeatRequest,
) -> Result<HeartbeatVerificationResult, crate::errors::VerificationError> {
    let session = db
        .load_session(&req.session_id)
        .map_err(|e| crate::errors::VerificationError::Storage(e.to_string()))?;
    let session = session.ok_or(crate::errors::VerificationError::SessionNotFound)?;

    let now = storage::current_time_ms();
    if now > session.expires_at {
        return Err(crate::errors::VerificationError::Expired);
    }

    // 1. Verify signature
    crypto::verify_signature(&session.public_key, req)
        .map_err(|e| crate::errors::VerificationError::Signature(e.to_string()))?;

    // 2. Check chain continuity
    let prev_hash_bytes = hex::decode(&req.prev_hash)?;
    if session.last_hash != prev_hash_bytes {
        return Err(crate::errors::VerificationError::ChainBroken);
    }

    // 3. Mutation step and deterministic mutation parity
    if req.mutation_step != session.pending_mutation_step {
        return Err(crate::errors::VerificationError::MutationStepMismatch {
            expected: session.pending_mutation_step,
            got: req.mutation_step,
        });
    }

    let environment = gene::decode_environment(&session.environment)
        .map_err(|e| crate::errors::VerificationError::GeneState(e.to_string()))?;
    let server_state = GeneState {
        gene: session.gene.clone(),
        environment,
    };
    let candidate_state = vm_extensions::apply_program_clone_with_rounds(
        &server_state,
        &session.pending_mutation,
        config.mutation_rounds,
    )
    .map_err(|e| crate::errors::VerificationError::MutationProgram(e.to_string()))?;
    let expected_gene_commitment =
        gene::commitment_hex_with_context(&candidate_state, &req.session_id, req.mutation_step);
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

    // 5.5 Verify VM execution state
    let expected_stack = shared::vm::execute(&session.opcodes);
    if req.stack_state.stack != expected_stack.stack || req.stack_state.ip != expected_stack.ip {
        return Err(crate::errors::VerificationError::VmStackMismatch);
    }

    // 6. Compute new hash
    let new_hash = shared::hashing::next_chain_hash(
        &prev_hash_bytes,
        req.timestamp,
        &req.entropy_data,
        &req.stack_state,
        &session.salt,
    );

    // 7. Prepare next mutation order and salt
    let next_step = session.pending_mutation_step + 1;
    let next_mutation = vm_extensions::generate_order(next_step, candidate_state.gene.len());
    let next_mutation_b64 = vm_extensions::encode_order_b64(&next_mutation);

    let next_salt = rand::random::<[u8; shared::constants::SALT_LEN]>();
    let next_salt_hex = hex::encode(next_salt);
    let next_environment_blob = gene::encode_environment(&candidate_state.environment)
        .map_err(|e| crate::errors::VerificationError::GeneState(e.to_string()))?;

    let update_record = storage::SessionRecord {
        session_id: req.session_id.clone(),
        public_key: session.public_key,
        salt: next_salt.to_vec(),
        last_hash: new_hash.clone(),
        chain_length: session.chain_length + 1,
        created_at: session.created_at,
        last_seen: now,
        expires_at: session.expires_at,
        gene: candidate_state.gene,
        environment: next_environment_blob,
        pending_mutation: next_mutation.program,
        pending_mutation_step: next_step,
        opcodes: session.opcodes,
    };
    db.update_session(&update_record, &session.last_hash)
        .map_err(|e| {
            if e.to_string().contains("Concurrent update detected") {
                crate::errors::VerificationError::ConcurrentUpdate
            } else {
                crate::errors::VerificationError::Storage(e.to_string())
            }
        })?;

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
    use rusqlite::params;
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
        opcodes_b64: String,
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
        db: &storage::DbPool,
        config: &crate::config::Config,
    ) -> (InitResponse, SigningKey) {
        let mut rng = rand::thread_rng();
        let sk = SigningKey::generate(&mut rng);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());
        let init = create_session(db, config, &pk_hex).unwrap();
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
            opcodes_b64: init.opcodes_b64.clone(),
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

        let program_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &client.opcodes_b64,
        )
        .unwrap();
        let stack = shared::vm::execute(&program_bytes);

        let mut req = HeartbeatRequest {
            session_id: client.session_id.clone(),
            prev_hash: client.prev_hash.clone(),
            timestamp,
            entropy_data: entropy.clone(),
            stack_state: stack.clone(),
            fingerprint: test_fingerprint(),
            mutation_step: client.pending_mutation_step,
            gene_commitment: gene::commitment_hex_with_context(
                &candidate_state,
                &client.session_id,
                client.pending_mutation_step,
            ),
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

    fn load_server_gene_state(db: &storage::DbPool, session_id: &str) -> GeneState {
        let session = db.load_session(session_id).unwrap().unwrap();
        GeneState {
            gene: session.gene,
            environment: gene::decode_environment(&session.environment).unwrap(),
        }
    }

    fn run_successful_heartbeat(
        db: &storage::DbPool,
        config: &crate::config::Config,
        client: &mut SimulatedClient,
    ) -> HeartbeatRequest {
        let timestamp = storage::current_time_ms();
        let (req, candidate_state, entropy, stack) = build_request(client, timestamp);
        let result = verify_heartbeat(db, config, &req).unwrap();
        apply_successful_response(client, &req, candidate_state, &entropy, &stack, &result);
        req
    }

    #[test]
    fn test_session_lifecycle_and_verification() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let config = test_config();

        let (init, signing_key) = create_test_session(&pool, &config);
        assert_eq!(init.gene_size, config.gene_size as u32);
        assert!(!init.mutation_order_b64.is_empty());
        assert_eq!(init.mutation_step, 1);

        let mut client = client_from_init(&init, signing_key);
        for _ in 0..5 {
            run_successful_heartbeat(&pool, &config, &mut client);
        }

        let stats = pool.stats().unwrap();
        assert_eq!(stats.sessions, 1);
        assert_eq!(stats.max_chain_length, 6);
    }

    #[test]
    fn test_deterministic_server_client_parity_across_many_heartbeats() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&pool, &config);
        let mut client = client_from_init(&init, signing_key);

        for _ in 0..12 {
            run_successful_heartbeat(&pool, &config, &mut client);
            let server_state = load_server_gene_state(&pool, &client.session_id);
            assert_eq!(server_state, client.committed_gene_state);
        }
    }

    #[test]
    fn test_replay_attack_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&pool, &config);
        let mut client = client_from_init(&init, signing_key);

        let timestamp = storage::current_time_ms();
        let (req, candidate_state, entropy, stack) = build_request(&client, timestamp);
        let result = verify_heartbeat(&pool, &config, &req).unwrap();
        apply_successful_response(
            &mut client,
            &req,
            candidate_state,
            &entropy,
            &stack,
            &result,
        );

        let replay = verify_heartbeat(&pool, &config, &req);
        assert!(matches!(
            replay.unwrap_err(),
            crate::errors::VerificationError::ChainBroken
        ));
    }

    #[test]
    fn test_mutation_step_mismatch_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&pool, &config);
        let client = client_from_init(&init, signing_key);

        let timestamp = storage::current_time_ms();
        let (mut req, _, _, _) = build_request(&client, timestamp);
        req.mutation_step += 1;
        sign_request(&client.signing_key, &mut req);

        let err = verify_heartbeat(&pool, &config, &req).unwrap_err();
        assert!(matches!(
            err,
            crate::errors::VerificationError::MutationStepMismatch { .. }
        ));
    }

    #[test]
    fn test_mutation_commitment_tamper_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&pool, &config);
        let client = client_from_init(&init, signing_key);

        let timestamp = storage::current_time_ms();
        let (mut req, _, _, _) = build_request(&client, timestamp);
        req.gene_commitment = "00".repeat(32);
        sign_request(&client.signing_key, &mut req);

        let err = verify_heartbeat(&pool, &config, &req).unwrap_err();
        assert!(matches!(
            err,
            crate::errors::VerificationError::MutationCommitmentMismatch
        ));
    }

    #[test]
    fn test_malformed_server_mutation_program_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = match &pool {
            storage::DbPool::Sqlite(pool) => pool.get().unwrap(),
            _ => panic!("expected sqlite pool for test"),
        };
        let config = test_config();
        let (init, signing_key) = create_test_session(&pool, &config);
        let client = client_from_init(&init, signing_key);

        conn.execute(
            "UPDATE sessions SET pending_mutation=?1 WHERE session_id=?2",
            params![vec![0xFFu8], client.session_id.clone()],
        )
        .unwrap();

        let updated: Vec<u8> = conn
            .query_row(
                "SELECT pending_mutation FROM sessions WHERE session_id=?1",
                params![client.session_id.clone()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(updated, vec![0xFFu8]);

        let timestamp = storage::current_time_ms();
        let (req, _, _, _) = build_request(&client, timestamp);
        let err = verify_heartbeat(&pool, &config, &req).unwrap_err();
        assert!(matches!(
            err,
            crate::errors::VerificationError::MutationProgram(_)
        ));
    }

    #[test]
    fn test_expired_session_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let conn = match &pool {
            storage::DbPool::Sqlite(pool) => pool.get().unwrap(),
            _ => panic!("expected sqlite pool for test"),
        };
        let config = test_config();
        let (init, signing_key) = create_test_session(&pool, &config);
        let client = client_from_init(&init, signing_key);

        conn.execute(
            "UPDATE sessions SET expires_at=?1 WHERE session_id=?2",
            params![0u64, client.session_id.clone()],
        )
        .unwrap();

        let timestamp = storage::current_time_ms();
        let (req, _, _, _) = build_request(&client, timestamp);
        let err = verify_heartbeat(&pool, &config, &req).unwrap_err();
        assert!(matches!(err, crate::errors::VerificationError::Expired));
    }

    #[test]
    fn test_create_session_rejects_invalid_public_key_length() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let config = test_config();
        let err = create_session(&pool, &config, "00ff").unwrap_err();
        assert!(matches!(
            err,
            crate::errors::SessionError::InvalidPublicKeyLength
        ));
    }

    #[test]
    fn test_stale_mutation_step_after_success_is_rejected() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let config = test_config();
        let (init, signing_key) = create_test_session(&pool, &config);
        let mut client = client_from_init(&init, signing_key);

        run_successful_heartbeat(&pool, &config, &mut client);

        let timestamp = storage::current_time_ms();
        let (mut req, _, _, _) = build_request(&client, timestamp);
        req.mutation_step -= 1;
        sign_request(&client.signing_key, &mut req);

        let err = verify_heartbeat(&pool, &config, &req).unwrap_err();
        assert!(matches!(
            err,
            crate::errors::VerificationError::MutationStepMismatch { .. }
        ));
    }

    #[test]
    fn test_repeated_simulation_keeps_server_and_client_commitments_equal() {
        let pool = storage::init_pool(Path::new(":memory:")).unwrap();
        let mut config = test_config();
        config.gene_size = 128;
        let (init, signing_key) = create_test_session(&pool, &config);
        let mut client = client_from_init(&init, signing_key);

        for _ in 0..10 {
            run_successful_heartbeat(&pool, &config, &mut client);
            let server_state = load_server_gene_state(&pool, &client.session_id);
            assert_eq!(
                gene::commitment(&server_state),
                gene::commitment(&client.committed_gene_state)
            );
        }
    }
}
