use ed25519_dalek::{Signer, SigningKey};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {
    static KEYPAIR: RefCell<Option<SigningKey>> = RefCell::new(None);
}

#[wasm_bindgen]
pub fn generate_keypair() -> String {
    let mut rng = rand::thread_rng();
    let sk = SigningKey::generate(&mut rng);
    let pk = sk.verifying_key();
    let hex_pub = hex::encode(pk.as_bytes());
    KEYPAIR.with(|kp| *kp.borrow_mut() = Some(sk));
    hex_pub
}

/// Returns the hex-encoded public key, or an empty string if the keypair has not
/// been generated yet.  Callers must check for an empty return value.
#[wasm_bindgen]
pub fn get_public_key() -> String {
    KEYPAIR.with(|kp| {
        kp.borrow()
            .as_ref()
            .map(|sk| hex::encode(sk.verifying_key().as_bytes()))
            .unwrap_or_default()
    })
}

/// Signs `message_json` and returns a hex-encoded signature, or an empty string
/// if the keypair has not been initialised.  Callers must check for an empty
/// return value before sending a heartbeat.
#[wasm_bindgen]
pub fn sign_message(message_json: &str) -> String {
    KEYPAIR.with(|kp| {
        kp.borrow()
            .as_ref()
            .map(|sk| hex::encode(sk.sign(message_json.as_bytes()).to_bytes()))
            .unwrap_or_default()
    })
}

#[wasm_bindgen]
pub fn compute_next_hash(
    prev_hash_hex: &str,
    timestamp: u64,
    entropy_data_json: &str,
    stack_state_json: &str,
    salt_hex: &str,
) -> String {
    let prev = hex::decode(prev_hash_hex).unwrap_or_default();
    let salt = hex::decode(salt_hex).unwrap_or_default();
    let entropy =
        serde_json::from_str::<shared::protocol::EntropyData>(entropy_data_json).unwrap();
    let stack =
        serde_json::from_str::<shared::protocol::StackState>(stack_state_json).unwrap();
    let new = shared::hashing::next_chain_hash(&prev, timestamp, &entropy, &stack, &salt);
    hex::encode(new)
}