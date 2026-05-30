use crate::protocol::{EntropyData, StackState};
use blake3::Hasher;

/// Computes the initial hash for a brand-new attestation session.
///
/// The hash is constructed as:
/// `Blake3(session_id || pub_key || salt)`
///
/// # Arguments
/// * `session_id` - The unique hex-encoded identifier for the session.
/// * `pub_key` - The client's Ed25519 public key.
/// * `salt` - The initial server-issued salt.
pub fn initial_hash(session_id: &str, pub_key: &[u8], salt: &[u8]) -> Vec<u8> {
    let mut h = Hasher::new();
    h.update(session_id.as_bytes());
    h.update(pub_key);
    h.update(salt);
    h.finalize().as_bytes().to_vec()
}

/// Computes the next hash in the Blake3 attestation chain.
///
/// This mixes in the previous hash head, the client timestamp, the serialized entropy data,
/// the VM stack state, and the server-issued salt. Uses `serde_json::to_vec` to avoid
/// intermediate heap string allocations and UTF-8 verification checks.
///
/// # Arguments
/// * `prev_hash` - The previous hash-chain head.
/// * `timestamp` - The client-supplied heartbeat timestamp.
/// * `entropy` - The collected browser interaction entropy.
/// * `stack` - The final VM stack state after running the opcode program.
/// * `salt` - The server-issued salt for rotation.
pub fn next_chain_hash(
    prev_hash: &[u8],
    timestamp: u64,
    entropy: &EntropyData,
    stack: &StackState,
    salt: &[u8],
) -> Vec<u8> {
    let entropy_bytes = serde_json::to_vec(entropy).unwrap();
    let stack_bytes = serde_json::to_vec(stack).unwrap();

    let entropy_hash = blake3::hash(&entropy_bytes);
    let stack_hash = blake3::hash(&stack_bytes);

    let mut h = Hasher::new();
    h.update(salt);
    h.update(prev_hash);
    h.update(&timestamp.to_le_bytes());
    h.update(entropy_hash.as_bytes());
    h.update(stack_hash.as_bytes());
    h.finalize().as_bytes().to_vec()
}

/// Computes a 32-bit FNV-like Blake3 hash of all stack elements.
///
/// This is used by the VM `HASH` opcode to fold the current stack state into a single value.
///
/// # Arguments
/// * `stack` - The list of u32 stack elements to hash.
pub fn hash_stack(stack: &[u32]) -> u32 {
    let data: Vec<u8> = stack.iter().flat_map(|x| x.to_le_bytes()).collect();
    let hash = blake3::hash(&data);
    u32::from_le_bytes(hash.as_bytes()[..4].try_into().unwrap())
}

