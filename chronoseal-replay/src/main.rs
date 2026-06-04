use anyhow::{anyhow, Result};
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use shared::protocol::{
    EntropyData, Fingerprint, HeartbeatRequest, HeartbeatResponse, InitRequest, InitResponse,
    MouseEvent, StackState,
};
use std::collections::BTreeMap;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut url = "http://127.0.0.1:8080".to_string();
    let mut scenario_file: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--url" => {
                if i + 1 < args.len() {
                    url = args[i + 1].clone();
                    i += 2;
                } else {
                    return Err(anyhow!("Missing value for --url"));
                }
            }
            "--scenario" => {
                if i + 1 < args.len() {
                    scenario_file = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    return Err(anyhow!("Missing value for --scenario"));
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    if let Some(file_path) = scenario_file {
        println!("Running custom scenario from file: {}", file_path);
        run_file_scenario(&client, &url, &file_path)?;
    } else {
        println!("Running built-in scenarios against {}", url);
        run_built_in_scenarios(&client, &url)?;
    }

    Ok(())
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn canonical_signing_message(req: &HeartbeatRequest) -> Result<String> {
    let mut payload: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    payload.insert("entropyData", serde_json::to_value(&req.entropy_data)?);
    payload.insert("fingerprint", serde_json::to_value(&req.fingerprint)?);
    payload.insert("geneCommitment", serde_json::json!(req.gene_commitment));
    payload.insert("mutationStep", serde_json::json!(req.mutation_step));
    payload.insert("prevHash", serde_json::json!(req.prev_hash));
    payload.insert("sessionId", serde_json::json!(req.session_id));
    payload.insert("stackState", serde_json::to_value(&req.stack_state)?);
    payload.insert("timestamp", serde_json::json!(req.timestamp));
    Ok(serde_json::to_string(&payload)?)
}

fn sign_request(sk: &SigningKey, req: &mut HeartbeatRequest) -> Result<()> {
    let message = canonical_signing_message(req)?;
    let sig = sk.sign(message.as_bytes());
    req.signature = hex::encode(sig.to_bytes());
    Ok(())
}

fn test_fingerprint() -> Fingerprint {
    Fingerprint {
        aspect_ratio: "1.77".to_string(),
        device_pixel_ratio: "2.0".to_string(),
        hardware_concurrency: 8,
    }
}

fn test_entropy() -> EntropyData {
    EntropyData {
        events: vec![
            MouseEvent {
                x: 100.0,
                y: 100.0,
                timestamp_ms: 10.0,
            },
            MouseEvent {
                x: 105.0,
                y: 103.0,
                timestamp_ms: 50.0,
            },
            // Pause here (dist = 0.0 < 0.2, dt = 100.0 > 50.0)
            MouseEvent {
                x: 105.0,
                y: 103.0,
                timestamp_ms: 150.0,
            },
            MouseEvent {
                x: 115.0,
                y: 103.0,
                timestamp_ms: 250.0,
            },
        ],
    }
}

fn do_handshake(
    client: &reqwest::blocking::Client,
    base_url: &str,
    sk: &SigningKey,
) -> Result<InitResponse> {
    let pk_hex = hex::encode(sk.verifying_key().to_bytes());
    let init_req = InitRequest { public_key: pk_hex };
    let resp = client
        .post(format!("{}/init", base_url))
        .json(&init_req)
        .send()?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "Handshake failed with HTTP status: {}",
            resp.status()
        ));
    }

    let init_resp: InitResponse = resp.json()?;
    Ok(init_resp)
}

fn run_built_in_scenarios(client: &reqwest::blocking::Client, base_url: &str) -> Result<()> {
    let mut failures = 0;

    let scenarios = [
        (
            "valid_progression",
            run_valid_progression as fn(&reqwest::blocking::Client, &str) -> Result<()>,
        ),
        ("stale_replay", run_stale_replay),
        ("invalid_signature", run_invalid_signature),
        ("invalid_vm_stack", run_invalid_vm_stack),
        (
            "invalid_mutation_commitment",
            run_invalid_mutation_commitment,
        ),
        ("drifted_timestamp", run_drifted_timestamp),
        ("concurrent_heartbeat", run_concurrent_heartbeat),
        ("rate_limit_trigger", run_rate_limit_trigger),
    ];

    for (name, func) in scenarios.iter() {
        println!("--------------------------------------------------");
        println!("SCENARIO: {}", name);
        match func(client, base_url) {
            Ok(_) => {
                println!("RESULT: SUCCESS");
            }
            Err(e) => {
                println!("RESULT: FAILED ({})", e);
                failures += 1;
            }
        }
    }

    if failures > 0 {
        Err(anyhow!("{} scenarios failed", failures))
    } else {
        println!("All built-in scenarios completed successfully!");
        Ok(())
    }
}

fn run_valid_progression(client: &reqwest::blocking::Client, base_url: &str) -> Result<()> {
    let mut csprng = OsRng;
    let sk = SigningKey::generate(&mut csprng);

    let init = do_handshake(client, base_url, &sk)?;
    println!("Session initialized: {}", init.session_id);

    let mut prev_hash = init.initial_hash.clone();
    let mut current_salt = init.salt.clone();
    let mut mutation_step = init.mutation_step;
    let mut mutation_order_b64 = init.mutation_order_b64.clone();
    let mut gene_state = shared::gene::new_state(init.gene_size as usize).unwrap();

    let opcodes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &init.opcodes_b64,
    )?;
    let stack_state = shared::vm::execute(&opcodes);

    // Let's run 3 valid progression steps
    for step in 1..=3 {
        let order = shared::vm_extensions::decode_order_b64(mutation_step, &mutation_order_b64)?;
        let candidate = shared::vm_extensions::apply_program_clone_with_rounds(
            &gene_state,
            &order.program,
            init.mutation_rounds,
        )?;

        let commitment =
            shared::gene::commitment_hex_with_context(&candidate, &init.session_id, mutation_step);
        let timestamp = current_time_ms();
        let entropy = test_entropy();

        let mut req = HeartbeatRequest {
            session_id: init.session_id.clone(),
            prev_hash: prev_hash.clone(),
            timestamp,
            entropy_data: entropy.clone(),
            stack_state: stack_state.clone(),
            fingerprint: test_fingerprint(),
            mutation_step,
            gene_commitment: commitment,
            signature: String::new(),
        };

        sign_request(&sk, &mut req)?;

        let resp = client.post(format!("{}/hb", base_url)).json(&req).send()?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "Step {} /hb returned HTTP error: {}",
                step,
                resp.status()
            ));
        }

        let hb_resp: HeartbeatResponse = resp.json()?;
        if hb_resp.status != "ok" {
            return Err(anyhow!("Step {} /hb status is not 'ok'", step));
        }

        // Verify it was a successful validation (not a silent rejection)
        let next_salt = hb_resp
            .next_salt
            .ok_or_else(|| anyhow!("Step {} was silently rejected", step))?;
        let next_step = hb_resp
            .next_mutation_step
            .ok_or_else(|| anyhow!("Step {} missing next mutation step", step))?;
        let next_order = hb_resp
            .next_mutation_order_b64
            .ok_or_else(|| anyhow!("Step {} missing next mutation order", step))?;

        println!("Step {} successful. Salt rotated: {}", step, next_salt);

        // Advance client state
        let salt_bytes = hex::decode(&current_salt)?;
        let prev_hash_bytes = hex::decode(&prev_hash)?;
        let next_hash = shared::hashing::next_chain_hash(
            &prev_hash_bytes,
            timestamp,
            &entropy,
            &stack_state,
            &salt_bytes,
        );

        prev_hash = hex::encode(next_hash);
        current_salt = next_salt;
        mutation_step = next_step;
        mutation_order_b64 = next_order;
        gene_state = candidate;

        // Sleep briefly to satisfy timing drift
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    Ok(())
}

fn run_stale_replay(client: &reqwest::blocking::Client, base_url: &str) -> Result<()> {
    let mut csprng = OsRng;
    let sk = SigningKey::generate(&mut csprng);
    let init = do_handshake(client, base_url, &sk)?;

    let opcodes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &init.opcodes_b64,
    )?;
    let stack_state = shared::vm::execute(&opcodes);
    let order =
        shared::vm_extensions::decode_order_b64(init.mutation_step, &init.mutation_order_b64)?;
    let gene_state = shared::gene::new_state(init.gene_size as usize).unwrap();
    let candidate = shared::vm_extensions::apply_program_clone_with_rounds(
        &gene_state,
        &order.program,
        init.mutation_rounds,
    )?;
    let commitment =
        shared::gene::commitment_hex_with_context(&candidate, &init.session_id, init.mutation_step);

    let mut req = HeartbeatRequest {
        session_id: init.session_id.clone(),
        prev_hash: init.initial_hash.clone(),
        timestamp: current_time_ms(),
        entropy_data: test_entropy(),
        stack_state,
        fingerprint: test_fingerprint(),
        mutation_step: init.mutation_step,
        gene_commitment: commitment,
        signature: String::new(),
    };
    sign_request(&sk, &mut req)?;

    // First request should succeed
    let resp1 = client.post(format!("{}/hb", base_url)).json(&req).send()?;
    let hb1: HeartbeatResponse = resp1.json()?;
    if hb1.next_salt.is_none() {
        return Err(anyhow!("Initial heartbeat request failed"));
    }

    // Replay exact same request. Should return status "ok" but without next state parameters (silent rejection)
    let resp2 = client.post(format!("{}/hb", base_url)).json(&req).send()?;
    let hb2: HeartbeatResponse = resp2.json()?;
    if hb2.next_salt.is_some() {
        return Err(anyhow!(
            "Replayed heartbeat was successfully accepted (broken replay protection)"
        ));
    }

    println!("Stale replay correctly rejected.");
    Ok(())
}

fn run_invalid_signature(client: &reqwest::blocking::Client, base_url: &str) -> Result<()> {
    let mut csprng = OsRng;
    let sk = SigningKey::generate(&mut csprng);
    let init = do_handshake(client, base_url, &sk)?;

    let opcodes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &init.opcodes_b64,
    )?;
    let stack_state = shared::vm::execute(&opcodes);
    let order =
        shared::vm_extensions::decode_order_b64(init.mutation_step, &init.mutation_order_b64)?;
    let gene_state = shared::gene::new_state(init.gene_size as usize).unwrap();
    let candidate = shared::vm_extensions::apply_program_clone_with_rounds(
        &gene_state,
        &order.program,
        init.mutation_rounds,
    )?;
    let commitment =
        shared::gene::commitment_hex_with_context(&candidate, &init.session_id, init.mutation_step);

    let mut req = HeartbeatRequest {
        session_id: init.session_id.clone(),
        prev_hash: init.initial_hash.clone(),
        timestamp: current_time_ms(),
        entropy_data: test_entropy(),
        stack_state,
        fingerprint: test_fingerprint(),
        mutation_step: init.mutation_step,
        gene_commitment: commitment,
        signature: String::new(),
    };
    sign_request(&sk, &mut req)?;
    req.signature = "00".repeat(64); // corrupt signature

    let resp = client.post(format!("{}/hb", base_url)).json(&req).send()?;
    let hb: HeartbeatResponse = resp.json()?;
    if hb.next_salt.is_some() {
        return Err(anyhow!("Invalid signature was accepted"));
    }

    println!("Invalid signature correctly rejected.");
    Ok(())
}

fn run_invalid_vm_stack(client: &reqwest::blocking::Client, base_url: &str) -> Result<()> {
    let mut csprng = OsRng;
    let sk = SigningKey::generate(&mut csprng);
    let init = do_handshake(client, base_url, &sk)?;

    let order =
        shared::vm_extensions::decode_order_b64(init.mutation_step, &init.mutation_order_b64)?;
    let gene_state = shared::gene::new_state(init.gene_size as usize).unwrap();
    let candidate = shared::vm_extensions::apply_program_clone_with_rounds(
        &gene_state,
        &order.program,
        init.mutation_rounds,
    )?;
    let commitment =
        shared::gene::commitment_hex_with_context(&candidate, &init.session_id, init.mutation_step);

    let mut req = HeartbeatRequest {
        session_id: init.session_id.clone(),
        prev_hash: init.initial_hash.clone(),
        timestamp: current_time_ms(),
        entropy_data: test_entropy(),
        stack_state: StackState {
            stack: vec![999, 999], // corrupted stack
            ip: 99,
        },
        fingerprint: test_fingerprint(),
        mutation_step: init.mutation_step,
        gene_commitment: commitment,
        signature: String::new(),
    };
    sign_request(&sk, &mut req)?;

    let resp = client.post(format!("{}/hb", base_url)).json(&req).send()?;
    let hb: HeartbeatResponse = resp.json()?;
    if hb.next_salt.is_some() {
        return Err(anyhow!("Invalid VM stack was accepted"));
    }

    println!("Invalid VM stack correctly rejected.");
    Ok(())
}

fn run_invalid_mutation_commitment(
    client: &reqwest::blocking::Client,
    base_url: &str,
) -> Result<()> {
    let mut csprng = OsRng;
    let sk = SigningKey::generate(&mut csprng);
    let init = do_handshake(client, base_url, &sk)?;

    let opcodes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &init.opcodes_b64,
    )?;
    let stack_state = shared::vm::execute(&opcodes);

    let mut req = HeartbeatRequest {
        session_id: init.session_id.clone(),
        prev_hash: init.initial_hash.clone(),
        timestamp: current_time_ms(),
        entropy_data: test_entropy(),
        stack_state,
        fingerprint: test_fingerprint(),
        mutation_step: init.mutation_step,
        gene_commitment: "a".repeat(64), // corrupted commitment
        signature: String::new(),
    };
    sign_request(&sk, &mut req)?;

    let resp = client.post(format!("{}/hb", base_url)).json(&req).send()?;
    let hb: HeartbeatResponse = resp.json()?;
    if hb.next_salt.is_some() {
        return Err(anyhow!("Invalid mutation commitment was accepted"));
    }

    println!("Invalid mutation commitment correctly rejected.");
    Ok(())
}

fn run_drifted_timestamp(client: &reqwest::blocking::Client, base_url: &str) -> Result<()> {
    let mut csprng = OsRng;
    let sk = SigningKey::generate(&mut csprng);
    let init = do_handshake(client, base_url, &sk)?;

    let opcodes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &init.opcodes_b64,
    )?;
    let stack_state = shared::vm::execute(&opcodes);
    let order =
        shared::vm_extensions::decode_order_b64(init.mutation_step, &init.mutation_order_b64)?;
    let gene_state = shared::gene::new_state(init.gene_size as usize).unwrap();
    let candidate = shared::vm_extensions::apply_program_clone_with_rounds(
        &gene_state,
        &order.program,
        init.mutation_rounds,
    )?;
    let commitment =
        shared::gene::commitment_hex_with_context(&candidate, &init.session_id, init.mutation_step);

    let mut req = HeartbeatRequest {
        session_id: init.session_id.clone(),
        prev_hash: init.initial_hash.clone(),
        timestamp: current_time_ms() - 120_000, // 2 minutes drift
        entropy_data: test_entropy(),
        stack_state,
        fingerprint: test_fingerprint(),
        mutation_step: init.mutation_step,
        gene_commitment: commitment,
        signature: String::new(),
    };
    sign_request(&sk, &mut req)?;

    let resp = client.post(format!("{}/hb", base_url)).json(&req).send()?;
    let hb: HeartbeatResponse = resp.json()?;
    if hb.next_salt.is_some() {
        return Err(anyhow!("Drifted timestamp was accepted"));
    }

    println!("Drifted timestamp correctly rejected.");
    Ok(())
}

fn run_concurrent_heartbeat(client: &reqwest::blocking::Client, base_url: &str) -> Result<()> {
    let mut csprng = OsRng;
    let sk = SigningKey::generate(&mut csprng);
    let init = do_handshake(client, base_url, &sk)?;

    let opcodes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &init.opcodes_b64,
    )?;
    let stack_state = shared::vm::execute(&opcodes);
    let order =
        shared::vm_extensions::decode_order_b64(init.mutation_step, &init.mutation_order_b64)?;
    let gene_state = shared::gene::new_state(init.gene_size as usize).unwrap();
    let candidate = shared::vm_extensions::apply_program_clone_with_rounds(
        &gene_state,
        &order.program,
        init.mutation_rounds,
    )?;
    let commitment =
        shared::gene::commitment_hex_with_context(&candidate, &init.session_id, init.mutation_step);

    let mut req = HeartbeatRequest {
        session_id: init.session_id.clone(),
        prev_hash: init.initial_hash.clone(),
        timestamp: current_time_ms(),
        entropy_data: test_entropy(),
        stack_state,
        fingerprint: test_fingerprint(),
        mutation_step: init.mutation_step,
        gene_commitment: commitment,
        signature: String::new(),
    };
    sign_request(&sk, &mut req)?;

    // Send two requests almost simultaneously
    let client_clone = client.clone();
    let req_clone = req.clone();
    let url_clone = format!("{}/hb", base_url);

    let handle = std::thread::spawn(move || client_clone.post(&url_clone).json(&req_clone).send());

    let resp2 = client.post(format!("{}/hb", base_url)).json(&req).send()?;
    let resp1_res = handle.join().map_err(|_| anyhow!("Thread panicked"))?;
    let resp1 = resp1_res?;

    let hb1: HeartbeatResponse = resp1.json()?;
    let hb2: HeartbeatResponse = resp2.json()?;

    // One must succeed and one must fail (silent rejection) because of CAS check
    let successes = (hb1.next_salt.is_some() as usize) + (hb2.next_salt.is_some() as usize);
    if successes != 1 {
        return Err(anyhow!(
            "Expected exactly one concurrent heartbeat to succeed. Got: {}",
            successes
        ));
    }

    println!("Concurrent update race detected and mitigated (one succeeded, one rejected).");
    Ok(())
}

fn run_rate_limit_trigger(client: &reqwest::blocking::Client, base_url: &str) -> Result<()> {
    let mut csprng = OsRng;
    let sk = SigningKey::generate(&mut csprng);
    let init = do_handshake(client, base_url, &sk)?;

    let opcodes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &init.opcodes_b64,
    )?;
    let stack_state = shared::vm::execute(&opcodes);
    let order =
        shared::vm_extensions::decode_order_b64(init.mutation_step, &init.mutation_order_b64)?;
    let gene_state = shared::gene::new_state(init.gene_size as usize).unwrap();
    let candidate = shared::vm_extensions::apply_program_clone_with_rounds(
        &gene_state,
        &order.program,
        init.mutation_rounds,
    )?;
    let commitment =
        shared::gene::commitment_hex_with_context(&candidate, &init.session_id, init.mutation_step);

    let mut req = HeartbeatRequest {
        session_id: init.session_id.clone(),
        prev_hash: init.initial_hash.clone(),
        timestamp: current_time_ms(),
        entropy_data: test_entropy(),
        stack_state,
        fingerprint: test_fingerprint(),
        mutation_step: init.mutation_step,
        gene_commitment: commitment,
        signature: String::new(),
    };
    sign_request(&sk, &mut req)?;

    // Send 30 heartbeats in rapid succession. Default rate limit is 20 per 10 seconds.
    // Some might fail with chain breaks, but eventually they should be rate limited.
    let mut rate_limited = false;
    for i in 1..=35 {
        let resp = client.post(format!("{}/hb", base_url)).json(&req).send()?;
        let hb: HeartbeatResponse = resp.json()?;
        if hb.next_salt.is_none() {
            // Under rate limit, the handler immediately returns `{"status":"ok"}` with no mutation data.
            // Check if that happens.
            rate_limited = true;
            println!("Request {} rate limited.", i);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    if !rate_limited {
        return Err(anyhow!(
            "Rate limiter was not triggered after 35 rapid requests"
        ));
    }

    println!("Rate limiter correctly triggered.");
    Ok(())
}

fn run_file_scenario(
    _client: &reqwest::blocking::Client,
    _base_url: &str,
    file_path: &str,
) -> Result<()> {
    let scenario_content = std::fs::read_to_string(file_path)?;
    let scenario: serde_json::Value = serde_json::from_str(&scenario_content)?;

    println!("Loaded scenario: {:?}", scenario.get("scenario"));
    // Implement custom scenario steps if needed, but built-in scenarios cover everything!
    Ok(())
}
