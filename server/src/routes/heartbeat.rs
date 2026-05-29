use crate::session::AppState;
use axum::{extract::State, http::StatusCode, Json};
use shared::protocol::{HeartbeatRequest, HeartbeatResponse};
use std::sync::Arc;

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HeartbeatRequest>,
) -> (StatusCode, Json<HeartbeatResponse>) {
    // Rate limiting
    {
        let (limit, window_secs) = {
            let cfg = state.get_config();
            (cfg.rate_limit_count, cfg.rate_limit_window_secs)
        };
        let mut rl = state.rate_limiter.lock().await;
        if !rl.check(&payload.session_id, limit, window_secs) {
            tracing::debug!("Rate limit hit: {}", payload.session_id);
            return (
                StatusCode::OK,
                Json(HeartbeatResponse {
                    status: "ok".into(),
                    next_salt: None,
                    next_mutation_step: None,
                    next_mutation_order_b64: None,
                }),
            );
        }
    }

    let config = state.get_config();
    match crate::session::verify_heartbeat(&state.db_pool, &config, &payload) {
        Ok(result) => (
            StatusCode::OK,
            Json(HeartbeatResponse {
                status: "ok".into(),
                next_salt: Some(result.next_salt_hex),
                next_mutation_step: Some(result.next_mutation_step),
                next_mutation_order_b64: Some(result.next_mutation_order_b64),
            }),
        ),
        Err(e) => {
            tracing::warn!("Heartbeat failed for {}: {}", payload.session_id, e);
            (
                StatusCode::OK,
                Json(HeartbeatResponse {
                    status: "ok".into(),
                    next_salt: None,
                    next_mutation_step: None,
                    next_mutation_order_b64: None,
                }),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::State, Json};
    use ed25519_dalek::{Signer, SigningKey};
    use shared::protocol::{EntropyData, Fingerprint, InitResponse, MouseEvent, StackState};
    use std::path::Path;

    fn test_config() -> crate::config::Config {
        crate::config::Config {
            expiration_minutes: 30,
            max_timestamp_drift_ms: 30_000,
            min_mouse_total_dist: 1.0,
            max_mouse_avg_speed: 4.0,
            min_pause_count: 0,
            require_mouse_activity: false,
            gene_size: 64,
            rate_limit_count: 20,
            rate_limit_window_secs: 10,
            ..crate::config::Config::default()
        }
    }

    fn sign_request(sk: &SigningKey, req: &mut HeartbeatRequest) {
        let msg = crate::crypto::canonical_signing_message(req).unwrap();
        req.signature = hex::encode(sk.sign(msg.as_bytes()).to_bytes());
    }

    fn build_request(
        init: &InitResponse,
        sk: &SigningKey,
        mutation_step: u64,
        mutation_order_b64: &str,
    ) -> HeartbeatRequest {
        let entropy_data = EntropyData {
            events: vec![
                MouseEvent {
                    x: 1.0,
                    y: 1.0,
                    timestamp_ms: 1.0,
                },
                MouseEvent {
                    x: 3.0,
                    y: 1.0,
                    timestamp_ms: 2.0,
                },
                MouseEvent {
                    x: 3.0,
                    y: 1.0,
                    timestamp_ms: 120.0,
                },
            ],
        };
        let stack_state = StackState {
            stack: vec![9, 10, 11],
            ip: 2,
        };

        let order =
            shared::vm_extensions::decode_order_b64(mutation_step, mutation_order_b64).unwrap();
        let committed = shared::gene::new_state(init.gene_size as usize).unwrap();
        let candidate =
            shared::vm_extensions::apply_program_clone(&committed, &order.program).unwrap();

        let mut req = HeartbeatRequest {
            session_id: init.session_id.clone(),
            prev_hash: init.initial_hash.clone(),
            timestamp: crate::storage::current_time_ms(),
            entropy_data,
            stack_state,
            fingerprint: Fingerprint {
                aspect_ratio: "1.77".to_string(),
                device_pixel_ratio: "2.0".to_string(),
                hardware_concurrency: 8,
            },
            mutation_step,
            gene_commitment: shared::gene::commitment_hex_with_context(
                &candidate,
                &init.session_id,
                mutation_step,
            ),
            signature: String::new(),
        };
        sign_request(sk, &mut req);
        req
    }

    async fn setup_state_and_session(
        config: crate::config::Config,
    ) -> (Arc<AppState>, InitResponse, SigningKey) {
        let pool = crate::storage::init_pool(Path::new(":memory:")).unwrap();
        let state = Arc::new(AppState {
            db_pool: pool.clone(),
            rate_limiter: tokio::sync::Mutex::new(crate::ratelimit::RateLimiter::new()),
            config: std::sync::RwLock::new(config.clone()),
        });

        let mut rng = rand::thread_rng();
        let sk = SigningKey::generate(&mut rng);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());
        let init = crate::session::create_session(&pool, &config, &pk_hex).unwrap();
        (state, init, sk)
    }

    #[tokio::test]
    async fn test_handler_success_returns_next_mutation_fields() {
        let config = test_config();
        let (state, init, sk) = setup_state_and_session(config).await;
        let req = build_request(&init, &sk, init.mutation_step, &init.mutation_order_b64);

        let (status, Json(body)) = handler(State(state), Json(req)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.status, "ok");
        assert!(body.next_salt.is_some());
        assert!(body.next_mutation_step.is_some());
        assert!(body.next_mutation_order_b64.is_some());
    }

    #[tokio::test]
    async fn test_handler_tampered_commitment_is_silent_failure() {
        let config = test_config();
        let (state, init, sk) = setup_state_and_session(config).await;
        let mut req = build_request(&init, &sk, init.mutation_step, &init.mutation_order_b64);
        req.gene_commitment = "00".repeat(32);
        sign_request(&sk, &mut req);

        let (status, Json(body)) = handler(State(state), Json(req)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.status, "ok");
        assert!(body.next_salt.is_none());
        assert!(body.next_mutation_step.is_none());
        assert!(body.next_mutation_order_b64.is_none());
    }

    #[tokio::test]
    async fn test_handler_rate_limit_returns_no_mutation_data() {
        let mut config = test_config();
        config.rate_limit_count = 0;
        let (state, init, sk) = setup_state_and_session(config).await;
        let req = build_request(&init, &sk, init.mutation_step, &init.mutation_order_b64);

        let (status, Json(body)) = handler(State(state), Json(req)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.status, "ok");
        assert!(body.next_salt.is_none());
        assert!(body.next_mutation_step.is_none());
        assert!(body.next_mutation_order_b64.is_none());
    }
}
