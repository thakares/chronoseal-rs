use ed25519_dalek::{Signature, VerifyingKey};
use shared::protocol::HeartbeatRequest;
use std::collections::BTreeMap;

/// Serializes the heartbeat request into a canonical JSON representation for signature verification.
///
/// Uses `BTreeMap` to order top-level keys alphabetically, matching the JavaScript client's
/// sorting algorithm: `JSON.stringify(obj, Object.keys(obj).sort())`.
///
/// # Arguments
/// * `req` - The heartbeat request to serialize.
pub fn canonical_signing_message(
    req: &HeartbeatRequest,
) -> Result<String, Box<dyn std::error::Error>> {
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

/// Verifies the Ed25519 signature of a client's heartbeat request.
///
/// Decodes the signature and compares it strictly against the canonical JSON message
/// using the client's public key.
///
/// # Arguments
/// * `pub_key_bytes` - The client's public key bytes.
/// * `req` - The heartbeat request payload containing the signature.
pub fn verify_signature(
    pub_key_bytes: &[u8],
    req: &HeartbeatRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let pk = VerifyingKey::from_bytes(&pub_key_bytes.try_into().map_err(|_| "invalid pubkey")?)?;
    let sig_bytes = hex::decode(&req.signature)?;
    let sig = Signature::from_slice(&sig_bytes)?;
    let message = canonical_signing_message(req)?;

    pk.verify_strict(message.as_bytes(), &sig)?;
    Ok(())
}
