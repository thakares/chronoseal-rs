use ed25519_dalek::{VerifyingKey, Signature};
use shared::protocol::HeartbeatRequest;

pub fn verify_signature(
    pub_key_bytes: &[u8],
    req: &HeartbeatRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let pk = VerifyingKey::from_bytes(
        &pub_key_bytes.try_into().map_err(|_| "invalid pubkey")?,
    )?;
    let sig_bytes = hex::decode(&req.signature)?;
    let sig = Signature::from_slice(&sig_bytes)?;

    // Build canonical JSON exactly as client signed (sorted keys, no extra spaces)
    let payload = serde_json::json!({
        "sessionId": req.session_id,
        "prevHash": req.prev_hash,
        "timestamp": req.timestamp,
        "entropyData": req.entropy_data,
        "stackState": req.stack_state,
        "fingerprint": req.fingerprint,
    });
    let message = serde_json::to_string(&payload)?;

    pk.verify_strict(message.as_bytes(), &sig)?;
    Ok(())
}