use blake3::Hasher;
use crate::protocol::{EntropyData, StackState};

/// Initial hash for a brand-new session: Blake3(session_id || pub_key || salt)
pub fn initial_hash(session_id: &str, pub_key: &[u8], salt: &[u8]) -> Vec<u8> {
    let mut h = Hasher::new();
    h.update(session_id.as_bytes());
    h.update(pub_key);
    h.update(salt);
    h.finalize().as_bytes().to_vec()
}

/// Next hash in the chain: Blake3 with the salt mixed in (no keyed mode needed)
pub fn next_chain_hash(
    prev_hash: &[u8],
    timestamp: u64,
    entropy: &EntropyData,
    stack: &StackState,
    salt: &[u8],
) -> Vec<u8> {
    let entropy_json = serde_json::to_string(entropy).unwrap();
    let stack_json = serde_json::to_string(stack).unwrap();

    let entropy_hash = blake3::hash(entropy_json.as_bytes());
    let stack_hash = blake3::hash(stack_json.as_bytes());

    let mut h = Hasher::new();
    // Mix the salt into the hash state
    h.update(salt);
    h.update(prev_hash);
    h.update(&timestamp.to_le_bytes());
    h.update(entropy_hash.as_bytes());
    h.update(stack_hash.as_bytes());
    h.finalize().as_bytes().to_vec()
}

/// Hash of all stack items for VM HASH opcode
pub fn hash_stack(stack: &[u32]) -> u32 {
    let data: Vec<u8> = stack.iter().flat_map(|x| x.to_le_bytes()).collect();
    let hash = blake3::hash(&data);
    u32::from_le_bytes(hash.as_bytes()[..4].try_into().unwrap())
}