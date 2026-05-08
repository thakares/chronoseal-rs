use ed25519_dalek::{Signature, VerifyingKey};
use shared::protocol::HeartbeatRequest;
use std::collections::BTreeMap;

pub fn verify_signature(
    pub_key_bytes: &[u8],
    req: &HeartbeatRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let pk = VerifyingKey::from_bytes(
        &pub_key_bytes.try_into().map_err(|_| "invalid pubkey")?,
    )?;
    let sig_bytes = hex::decode(&req.signature)?;
    let sig = Signature::from_slice(&sig_bytes)?;

    // Build canonical JSON with BTreeMap so keys are sorted alphabetically,
    // matching the JS client's JSON.stringify(obj, Object.keys(obj).sort()).
    // Sorted order: entropyData, fingerprint, prevHash, sessionId, stackState, timestamp
    let mut payload: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    payload.insert("entropyData", serde_json::to_value(&req.entropy_data)?);
    payload.insert("fingerprint", serde_json::to_value(&req.fingerprint)?);
    payload.insert("prevHash", serde_json::json!(req.prev_hash));
    payload.insert("sessionId", serde_json::json!(req.session_id));
    payload.insert("stackState", serde_json::to_value(&req.stack_state)?);
    payload.insert("timestamp", serde_json::json!(req.timestamp));
    let message = serde_json::to_string(&payload)?;

    pk.verify_strict(message.as_bytes(), &sig)?;
    Ok(())
}