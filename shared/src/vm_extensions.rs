use crate::{
    constants::{
        DEFAULT_MUTATION_ROUNDS, HASH_OPCODE_INSTRUCTION_COST, MAX_GENE_SIZE,
        MAX_MUTATION_INSTRUCTION_BUDGET, MAX_MUTATION_PROGRAM_BYTES, SOFT_CAP_DURATION_MS,
    },
    gene::{
        add_env_quantity, get_env_quantity, sub_env_quantity, validate_state, GeneError, GeneState,
    },
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Instant;

// Stack-machine mutation opcodes (v0.6.0).
//
// NOTE: stack effect notation:
//   +1  => pushes one u32
//   -1  => pops one u32
//    0  => net-zero (or no stack interaction)
//
// Security/performance notes:
// - All index operands are normalized with modulo to avoid panics.
// - Program size is bounded by MAX_MUTATION_PROGRAM_BYTES.
// - Environment arithmetic is saturating and deterministic.
// - Hashing uses fixed BLAKE3 commitment and fixed transcription algorithm.
pub const OP_GENE_LOAD: u8 = 0x23; // +1
pub const OP_GENE_STORE: u8 = 0x24; // -1
pub const OP_MUTATE_POINT: u8 = 0x25; // 0
pub const OP_INSERT: u8 = 0x26; // -1
pub const OP_DELETE: u8 = 0x27; // +1
pub const OP_TRANSCRIBE: u8 = 0x28; // +1
pub const OP_APPLY_MUTAGEN: u8 = 0x29; // -1
pub const OP_FINALIZE_GENE_HASH: u8 = 0x2A; // +1
pub const OP_CONSUME: u8 = 0x2B; // 0 (pop amount, push remaining)
pub const OP_PRODUCE: u8 = 0x2C; // 0 (pop amount, push resulting quantity)

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationOrder {
    pub step: u64,
    pub program: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionTrace {
    pub final_ip: usize,
    pub final_stack: Vec<u32>,
    pub final_gene_commitment_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationError {
    ProgramTooLong { len: usize },
    TruncatedInstruction { opcode: u8, ip: usize },
    UnknownOpcode(u8),
    EmptyGene,
    StackUnderflow { opcode: u8, ip: usize },
    GeneFull { current_len: usize },
    Base64(base64::DecodeError),
    Gene(GeneError),
}

impl std::fmt::Display for MutationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProgramTooLong { len } => write!(f, "mutation program too long: {len} bytes"),
            Self::TruncatedInstruction { opcode, ip } => {
                write!(f, "truncated instruction {opcode:#04x} at ip={ip}")
            }
            Self::UnknownOpcode(opcode) => write!(f, "unknown mutation opcode: {opcode:#04x}"),
            Self::EmptyGene => write!(f, "cannot mutate an empty gene"),
            Self::StackUnderflow { opcode, ip } => {
                write!(f, "stack underflow in opcode {opcode:#04x} at ip={ip}")
            }
            Self::GeneFull { current_len } => {
                write!(f, "cannot insert; gene already at max size ({current_len})")
            }
            Self::Base64(err) => write!(f, "invalid base64 mutation order: {err}"),
            Self::Gene(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for MutationError {}

impl From<GeneError> for MutationError {
    fn from(value: GeneError) -> Self {
        Self::Gene(value)
    }
}

pub fn encode_order_b64(order: &MutationOrder) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &order.program)
}

pub fn decode_order_b64(step: u64, b64: &str) -> Result<MutationOrder, MutationError> {
    let program = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
        .map_err(MutationError::Base64)?;
    if program.len() > MAX_MUTATION_PROGRAM_BYTES {
        return Err(MutationError::ProgramTooLong { len: program.len() });
    }
    Ok(MutationOrder { step, program })
}

pub fn generate_order(step: u64, gene_size: usize) -> MutationOrder {
    let mut rng = rand::thread_rng();
    generate_order_with_rng(&mut rng, step, gene_size)
}

pub fn generate_order_with_rng<R: Rng + ?Sized>(
    rng: &mut R,
    step: u64,
    gene_size: usize,
) -> MutationOrder {
    let mut program = Vec::with_capacity(128);
    let mut stack_depth: i32 = 0;
    let mut estimated_gene_len = gene_size.clamp(1, MAX_GENE_SIZE);
    let ops = rng.gen_range(20usize..=36usize);
    let mut hash_ops_needed = rng.gen_range(2..=3);

    for idx in 0..ops {
        let remaining = ops - idx;
        let op = if hash_ops_needed > 0 && remaining <= hash_ops_needed {
            OP_FINALIZE_GENE_HASH
        } else if stack_depth <= 0 {
            rng.gen_range(0u8..3u8)
        } else {
            match rng.gen_range(0u8..12u8) {
                0..=1 => OP_GENE_LOAD,
                2..=3 => OP_TRANSCRIBE,
                4 => OP_FINALIZE_GENE_HASH,
                5 => OP_GENE_STORE,
                6 => OP_MUTATE_POINT,
                7 => OP_INSERT,
                8 => OP_DELETE,
                9 => OP_APPLY_MUTAGEN,
                10 => OP_CONSUME,
                _ => OP_PRODUCE,
            }
        };

        match op {
            OP_GENE_LOAD => {
                program.push(OP_GENE_LOAD);
                push_u16(&mut program, rng.r#gen::<u16>());
                stack_depth += 1;
            }
            OP_TRANSCRIBE => {
                program.push(OP_TRANSCRIBE);
                push_u16(&mut program, rng.r#gen::<u16>());
                program.push(rng.gen_range(1u8..=16u8));
                stack_depth += 1;
            }
            OP_FINALIZE_GENE_HASH => {
                program.push(OP_FINALIZE_GENE_HASH);
                stack_depth += 1;
                hash_ops_needed = hash_ops_needed.saturating_sub(1);
            }
            OP_GENE_STORE => {
                if stack_depth > 0 {
                    program.push(OP_GENE_STORE);
                    push_u16(&mut program, rng.r#gen::<u16>());
                    stack_depth -= 1;
                }
            }
            OP_MUTATE_POINT => {
                program.push(OP_MUTATE_POINT);
                push_u16(&mut program, rng.r#gen::<u16>());
                program.push(rng.r#gen::<u8>());
            }
            OP_INSERT => {
                if stack_depth > 0 && estimated_gene_len < MAX_GENE_SIZE {
                    program.push(OP_INSERT);
                    push_u16(&mut program, rng.r#gen::<u16>());
                    stack_depth -= 1;
                    estimated_gene_len += 1;
                }
            }
            OP_DELETE => {
                program.push(OP_DELETE);
                push_u16(&mut program, rng.r#gen::<u16>());
                stack_depth += 1;
                if estimated_gene_len > 1 {
                    estimated_gene_len -= 1;
                }
            }
            OP_APPLY_MUTAGEN => {
                if stack_depth > 0 {
                    program.push(OP_APPLY_MUTAGEN);
                    push_u16(&mut program, rng.r#gen::<u16>());
                    push_u16(&mut program, rng.r#gen::<u16>());
                    stack_depth -= 1;
                }
            }
            OP_CONSUME => {
                if stack_depth > 0 {
                    program.push(OP_CONSUME);
                    push_u16(&mut program, rng.r#gen::<u16>());
                }
            }
            OP_PRODUCE if stack_depth > 0 => {
                program.push(OP_PRODUCE);
                push_u16(&mut program, rng.r#gen::<u16>());
            }
            _ => {}
        }
    }

    while hash_ops_needed > 0 && program.len() < MAX_MUTATION_PROGRAM_BYTES {
        program.push(OP_FINALIZE_GENE_HASH);
        hash_ops_needed -= 1;
    }

    MutationOrder { step, program }
}

pub fn apply_program_clone(state: &GeneState, program: &[u8]) -> Result<GeneState, MutationError> {
    apply_program_clone_with_rounds(state, program, DEFAULT_MUTATION_ROUNDS)
}

pub fn apply_program_clone_with_rounds(
    state: &GeneState,
    program: &[u8],
    rounds: u8,
) -> Result<GeneState, MutationError> {
    let mut next = state.clone();
    execute_program_with_rounds(&mut next, program, rounds)?;
    Ok(next)
}

pub fn apply_program_with_rounds(
    state: &mut GeneState,
    program: &[u8],
    rounds: u8,
) -> Result<(), MutationError> {
    let _ = execute_program_with_rounds(state, program, rounds)?;
    Ok(())
}

pub fn apply_program(state: &mut GeneState, program: &[u8]) -> Result<(), MutationError> {
    let _ = execute_program_with_rounds(state, program, DEFAULT_MUTATION_ROUNDS)?;
    Ok(())
}

pub fn execute_program_with_rounds(
    state: &mut GeneState,
    program: &[u8],
    rounds: u8,
) -> Result<ExecutionTrace, MutationError> {
    if state.gene.is_empty() {
        return Err(MutationError::EmptyGene);
    }
    validate_state(state)?;
    if program.len() > MAX_MUTATION_PROGRAM_BYTES {
        return Err(MutationError::ProgramTooLong { len: program.len() });
    }

    let program_cost = estimate_program_cost(program);
    let max_rounds = std::cmp::max(1, MAX_MUTATION_INSTRUCTION_BUDGET / program_cost);
    let actual_rounds = std::cmp::min(rounds as usize, max_rounds) as u8;
    let start = Instant::now();
    let mut trace = None;

    for _round in 0..actual_rounds {
        trace = Some(execute_program(state, program)?);
    }

    let elapsed = start.elapsed();
    tracing::debug!(
        rounds = actual_rounds,
        requested_rounds = rounds,
        elapsed_ms = elapsed.as_millis(),
        program_len = program.len(),
        "mutation execution"
    );
    if actual_rounds < rounds {
        tracing::debug!(
            requested_rounds = rounds,
            executed_rounds = actual_rounds,
            "mutation soft cap reduced mutation rounds to preserve host responsiveness"
        );
    }
    if elapsed.as_millis() > SOFT_CAP_DURATION_MS {
        tracing::debug!(
            elapsed_ms = elapsed.as_millis(),
            "mutation execution exceeded soft cap duration"
        );
    }

    Ok(trace.unwrap_or_else(|| ExecutionTrace {
        final_ip: 0,
        final_stack: Vec::new(),
        final_gene_commitment_hex: crate::gene::commitment_hex(state),
    }))
}

fn estimate_program_cost(program: &[u8]) -> usize {
    let mut ip = 0;
    let mut cost = 0;

    while ip < program.len() {
        let opcode = program[ip];
        ip += 1;
        cost += if opcode == OP_FINALIZE_GENE_HASH {
            HASH_OPCODE_INSTRUCTION_COST
        } else {
            1
        };
        ip += match opcode {
            OP_GENE_LOAD | OP_GENE_STORE | OP_INSERT | OP_DELETE | OP_CONSUME | OP_PRODUCE => 2,
            OP_MUTATE_POINT | OP_TRANSCRIBE => 3,
            OP_APPLY_MUTAGEN => 4,
            OP_FINALIZE_GENE_HASH => 0,
            _ => 0,
        }
    }

    cost.max(1)
}

pub fn execute_program(
    state: &mut GeneState,
    program: &[u8],
) -> Result<ExecutionTrace, MutationError> {
    if state.gene.is_empty() {
        return Err(MutationError::EmptyGene);
    }
    validate_state(state)?;
    if program.len() > MAX_MUTATION_PROGRAM_BYTES {
        return Err(MutationError::ProgramTooLong { len: program.len() });
    }

    let mut ip = 0usize;
    let mut stack: Vec<u32> = Vec::with_capacity(16);
    while ip < program.len() {
        let opcode_ip = ip;
        let opcode = take_u8(program, &mut ip, 0x00)?;
        match opcode {
            OP_GENE_LOAD => {
                let idx = take_u16(program, &mut ip, opcode)?;
                let normalized = normalize_index(idx as usize, state.gene.len());
                stack.push(state.gene[normalized] as u32);
            }
            OP_GENE_STORE => {
                let idx = take_u16(program, &mut ip, opcode)?;
                let value = pop_stack(&mut stack, opcode, opcode_ip)? as u8;
                let normalized = normalize_index(idx as usize, state.gene.len());
                state.gene[normalized] = value;
            }
            OP_MUTATE_POINT => {
                let idx = take_u16(program, &mut ip, opcode)?;
                let delta = take_u8(program, &mut ip, opcode)? as i8;
                let normalized = normalize_index(idx as usize, state.gene.len());
                state.gene[normalized] = state.gene[normalized].wrapping_add(delta as u8);
            }
            OP_INSERT => {
                let idx = take_u16(program, &mut ip, opcode)?;
                let value = pop_stack(&mut stack, opcode, opcode_ip)? as u8;
                if state.gene.len() >= MAX_GENE_SIZE {
                    return Err(MutationError::GeneFull {
                        current_len: state.gene.len(),
                    });
                }
                let insert_at = (idx as usize).min(state.gene.len());
                state.gene.insert(insert_at, value);
            }
            OP_DELETE => {
                let idx = take_u16(program, &mut ip, opcode)?;
                let normalized = normalize_index(idx as usize, state.gene.len());
                let removed = if state.gene.len() > 1 {
                    state.gene.remove(normalized)
                } else {
                    let prev = state.gene[0];
                    state.gene[0] = 0;
                    prev
                };
                stack.push(removed as u32);
            }
            OP_TRANSCRIBE => {
                let start = take_u16(program, &mut ip, opcode)?;
                let span = take_u8(program, &mut ip, opcode)?;
                let transcription = transcribe_window(&state.gene, start as usize, span);
                stack.push(transcription);
            }
            OP_APPLY_MUTAGEN => {
                let symbol = take_u16(program, &mut ip, opcode)?;
                let idx = take_u16(program, &mut ip, opcode)?;
                let stack_mask = pop_stack(&mut stack, opcode, opcode_ip)? as u8;
                let quantity = get_env_quantity(state, symbol);
                let mix = ((quantity as u8)
                    ^ ((quantity >> 8) as u8)
                    ^ ((quantity >> 16) as u8)
                    ^ ((quantity >> 24) as u8))
                    ^ ((symbol & 0x00ff) as u8)
                    ^ ((symbol >> 8) as u8)
                    ^ stack_mask;
                let normalized = normalize_index(idx as usize, state.gene.len());
                state.gene[normalized] ^= mix;
            }
            OP_FINALIZE_GENE_HASH => {
                let commit = crate::gene::commitment(state);
                let hash32 = u32::from_le_bytes([commit[0], commit[1], commit[2], commit[3]]);
                stack.push(hash32);
            }
            OP_CONSUME => {
                let symbol = take_u16(program, &mut ip, opcode)?;
                let amount = pop_stack(&mut stack, opcode, opcode_ip)?;
                let left = sub_env_quantity(state, symbol, amount)?;
                stack.push(left);
            }
            OP_PRODUCE => {
                let symbol = take_u16(program, &mut ip, opcode)?;
                let amount = pop_stack(&mut stack, opcode, opcode_ip)?;
                let next = add_env_quantity(state, symbol, amount)?;
                stack.push(next);
            }
            _ => return Err(MutationError::UnknownOpcode(opcode)),
        }
    }

    Ok(ExecutionTrace {
        final_ip: ip,
        final_stack: stack,
        final_gene_commitment_hex: crate::gene::commitment_hex(state),
    })
}

fn transcribe_window(gene: &[u8], start: usize, span: u8) -> u32 {
    let count = usize::from(span.max(1));
    let mut acc = 2_166_136_261u32; // FNV offset basis
    for i in 0..count {
        let idx = (start + i) % gene.len();
        acc ^= gene[idx] as u32;
        acc = acc.wrapping_mul(16_777_619); // FNV prime
    }
    acc
}

fn push_u16(buf: &mut Vec<u8>, value: u16) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn take_u8(bytes: &[u8], ip: &mut usize, opcode: u8) -> Result<u8, MutationError> {
    if *ip >= bytes.len() {
        return Err(MutationError::TruncatedInstruction { opcode, ip: *ip });
    }
    let value = bytes[*ip];
    *ip += 1;
    Ok(value)
}

fn take_u16(bytes: &[u8], ip: &mut usize, opcode: u8) -> Result<u16, MutationError> {
    if *ip + 2 > bytes.len() {
        return Err(MutationError::TruncatedInstruction { opcode, ip: *ip });
    }
    let value = u16::from_le_bytes([bytes[*ip], bytes[*ip + 1]]);
    *ip += 2;
    Ok(value)
}

fn pop_stack(stack: &mut Vec<u32>, opcode: u8, ip: usize) -> Result<u32, MutationError> {
    stack
        .pop()
        .ok_or(MutationError::StackUnderflow { opcode, ip })
}

fn normalize_index(idx: usize, len: usize) -> usize {
    idx % len
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gene::{commitment, new_state, set_env_quantity};
    use rand::{Rng, SeedableRng};
    use std::time::Instant;

    fn u16_bytes(v: u16) -> [u8; 2] {
        v.to_le_bytes()
    }

    #[test]
    fn test_opcode_gene_load() {
        let mut state = new_state(4).unwrap();
        state.gene = vec![10, 20, 30, 40];
        let trace = execute_program(&mut state, &[OP_GENE_LOAD, 1, 0]).unwrap();
        assert_eq!(trace.final_stack, vec![20]);
    }

    #[test]
    fn test_opcode_gene_store() {
        let mut state = new_state(4).unwrap();
        state.gene = vec![1, 2, 3, 4];
        let program = vec![
            OP_GENE_LOAD,
            0,
            0, // stack: [1]
            OP_GENE_STORE,
            2,
            0, // gene[2] <- 1
        ];
        execute_program(&mut state, &program).unwrap();
        assert_eq!(state.gene, vec![1, 2, 1, 4]);
    }

    #[test]
    fn test_opcode_mutate_point() {
        let mut state = new_state(4).unwrap();
        state.gene[0] = 200;
        let program = vec![OP_MUTATE_POINT, 0, 0, 100u8];
        execute_program(&mut state, &program).unwrap();
        assert_eq!(state.gene[0], 44);
    }

    #[test]
    fn test_opcode_insert() {
        let mut state = new_state(3).unwrap();
        state.gene = vec![10, 20, 30];
        let program = vec![
            OP_GENE_LOAD,
            1,
            0, // stack: [20]
            OP_INSERT,
            0,
            0, // insert 20 at position 0
        ];
        execute_program(&mut state, &program).unwrap();
        assert_eq!(state.gene, vec![20, 10, 20, 30]);
    }

    #[test]
    fn test_opcode_delete() {
        let mut state = new_state(4).unwrap();
        state.gene = vec![9, 8, 7, 6];
        let trace = execute_program(&mut state, &[OP_DELETE, 2, 0]).unwrap();
        assert_eq!(state.gene, vec![9, 8, 6]);
        assert_eq!(trace.final_stack, vec![7]);
    }

    #[test]
    fn test_opcode_transcribe() {
        let mut state = new_state(5).unwrap();
        state.gene = vec![1, 2, 3, 4, 5];
        let trace = execute_program(&mut state, &[OP_TRANSCRIBE, 1, 0, 3]).unwrap();
        assert_eq!(trace.final_stack.len(), 1);
        assert_ne!(trace.final_stack[0], 0);
    }

    #[test]
    fn test_opcode_apply_mutagen() {
        let mut state = new_state(4).unwrap();
        set_env_quantity(&mut state, 7, 0x1234_5678).unwrap();
        state.gene[1] = 0xAA;
        let program = vec![
            OP_GENE_LOAD,
            0,
            0, // stack mask source
            OP_APPLY_MUTAGEN,
            7,
            0,
            1,
            0,
        ];
        execute_program(&mut state, &program).unwrap();
        assert_ne!(state.gene[1], 0xAA);
    }

    #[test]
    fn test_opcode_finalize_gene_hash() {
        let mut state = new_state(4).unwrap();
        let trace = execute_program(&mut state, &[OP_FINALIZE_GENE_HASH]).unwrap();
        assert_eq!(trace.final_stack.len(), 1);
    }

    #[test]
    fn test_opcode_consume() {
        let mut state = new_state(4).unwrap();
        set_env_quantity(&mut state, 3, 100).unwrap();
        let program = vec![
            OP_GENE_LOAD,
            0,
            0, // stack = [0]
            OP_MUTATE_POINT,
            0,
            0,
            15, // gene[0]=15
            OP_GENE_LOAD,
            0,
            0, // stack=[0,15]
            OP_CONSUME,
            3,
            0, // consume 15
        ];
        let trace = execute_program(&mut state, &program).unwrap();
        assert_eq!(get_env_quantity(&state, 3), 85);
        assert_eq!(trace.final_stack.last().copied().unwrap(), 85);
    }

    #[test]
    fn test_opcode_produce() {
        let mut state = new_state(4).unwrap();
        set_env_quantity(&mut state, 9, 5).unwrap();
        let program = vec![
            OP_GENE_LOAD,
            0,
            0, // stack [0]
            OP_MUTATE_POINT,
            0,
            0,
            10, // gene[0]=10
            OP_GENE_LOAD,
            0,
            0, // stack [0,10]
            OP_PRODUCE,
            9,
            0, // +10
        ];
        let trace = execute_program(&mut state, &program).unwrap();
        assert_eq!(get_env_quantity(&state, 9), 15);
        assert_eq!(trace.final_stack.last().copied().unwrap(), 15);
    }

    #[test]
    fn test_zero_length_gene_is_rejected() {
        let mut state = GeneState {
            gene: vec![],
            environment: vec![],
        };
        let err = execute_program(&mut state, &[OP_FINALIZE_GENE_HASH]).unwrap_err();
        assert_eq!(err, MutationError::EmptyGene);
    }

    #[test]
    fn test_insert_rejects_max_size_gene() {
        let mut state = new_state(MAX_GENE_SIZE).unwrap();
        let program = vec![
            OP_GENE_LOAD,
            0,
            0, // push value
            OP_INSERT,
            0,
            0,
        ];
        let err = execute_program(&mut state, &program).unwrap_err();
        assert!(matches!(err, MutationError::GeneFull { .. }));
    }

    #[test]
    fn test_invalid_positions_wrap_deterministically() {
        let mut state_a = new_state(5).unwrap();
        let mut state_b = new_state(5).unwrap();
        let max_u16 = u16::MAX;
        let [a0, a1] = u16_bytes(max_u16);
        let program = vec![OP_MUTATE_POINT, a0, a1, 1];
        execute_program(&mut state_a, &program).unwrap();

        let wrapped = (max_u16 as usize % 5) as u16;
        let [w0, w1] = u16_bytes(wrapped);
        let wrapped_program = vec![OP_MUTATE_POINT, w0, w1, 1];
        execute_program(&mut state_b, &wrapped_program).unwrap();
        assert_eq!(state_a, state_b);
    }

    #[test]
    fn test_quantity_underflow_is_saturating() {
        let mut state = new_state(4).unwrap();
        set_env_quantity(&mut state, 1, 3).unwrap();
        state.gene[0] = 8;
        let program = vec![
            OP_GENE_LOAD,
            0,
            0, // 8
            OP_CONSUME,
            1,
            0, // consume 8 from qty 3 => 0
        ];
        let trace = execute_program(&mut state, &program).unwrap();
        assert_eq!(get_env_quantity(&state, 1), 0);
        assert_eq!(trace.final_stack.last().copied().unwrap(), 0);
    }

    #[test]
    fn test_rejects_unknown_opcode() {
        let mut state = new_state(8).unwrap();
        let err = execute_program(&mut state, &[0xFF]).unwrap_err();
        assert_eq!(err, MutationError::UnknownOpcode(0xFF));
    }

    #[test]
    fn test_rejects_truncated_instruction() {
        let mut state = new_state(8).unwrap();
        let err = execute_program(&mut state, &[OP_GENE_LOAD, 1]).unwrap_err();
        assert!(matches!(err, MutationError::TruncatedInstruction { .. }));
    }

    #[test]
    fn test_rejects_stack_underflow() {
        let mut state = new_state(8).unwrap();
        let err = execute_program(&mut state, &[OP_GENE_STORE, 0, 0]).unwrap_err();
        assert!(matches!(err, MutationError::StackUnderflow { .. }));
    }

    #[test]
    fn test_base64_order_roundtrip() {
        let order = MutationOrder {
            step: 17,
            program: vec![OP_GENE_LOAD, 1, 0, OP_GENE_STORE, 2, 0],
        };
        let b64 = encode_order_b64(&order);
        let decoded = decode_order_b64(order.step, &b64).unwrap();
        assert_eq!(decoded, order);
    }

    #[test]
    fn test_generate_order_is_deterministic_for_seeded_rng() {
        let mut rng_a = rand::rngs::StdRng::seed_from_u64(99);
        let mut rng_b = rand::rngs::StdRng::seed_from_u64(99);
        let order_a = generate_order_with_rng(&mut rng_a, 5, 64);
        let order_b = generate_order_with_rng(&mut rng_b, 5, 64);
        assert_eq!(order_a, order_b);
    }

    #[test]
    fn test_mutation_chain() {
        let mut server_state = new_state(32).unwrap();
        let mut client_state = new_state(32).unwrap();

        let program = vec![
            OP_GENE_LOAD,
            0,
            0,
            OP_PRODUCE,
            2,
            0, // env[2]+=gene[0]
            OP_GENE_LOAD,
            1,
            0,
            OP_APPLY_MUTAGEN,
            2,
            0,
            1,
            0, // mutagen at idx1
            OP_TRANSCRIBE,
            0,
            0,
            8, // hash window
            OP_GENE_STORE,
            2,
            0, // gene[2]=transcription_low_byte
            OP_DELETE,
            0,
            0, // stack pushes removed
            OP_INSERT,
            3,
            0, // insert removed at position 3
            OP_FINALIZE_GENE_HASH,
        ];

        let server_trace = execute_program(&mut server_state, &program).unwrap();
        let client_trace = execute_program(&mut client_state, &program).unwrap();

        assert_eq!(server_state, client_state);
        assert_eq!(server_trace.final_stack, client_trace.final_stack);
        assert_eq!(
            server_trace.final_gene_commitment_hex,
            client_trace.final_gene_commitment_hex
        );
    }

    #[test]
    fn test_server_client_parity_across_random_orders() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        for step in 0..128u64 {
            let order = generate_order_with_rng(&mut rng, step, 128);
            let mut server_state = new_state(128).unwrap();
            let mut client_state = new_state(128).unwrap();

            let server_result = execute_program(&mut server_state, &order.program);
            let client_result = execute_program(&mut client_state, &order.program);
            assert_eq!(server_result.is_ok(), client_result.is_ok());

            match (server_result, client_result) {
                (Ok(server_trace), Ok(client_trace)) => {
                    assert_eq!(server_state, client_state);
                    assert_eq!(server_trace.final_stack, client_trace.final_stack);
                    assert_eq!(
                        commitment(&server_state),
                        commitment(&client_state),
                        "step {step}"
                    );
                }
                (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn test_fuzz_style_random_program_bytes_do_not_diverge() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(2026);
        for _ in 0..256 {
            let len = rng.gen_range(1usize..=MAX_MUTATION_PROGRAM_BYTES);
            let mut program = vec![0u8; len];
            for b in &mut program {
                *b = rng.r#gen::<u8>();
            }

            let mut a = new_state(64).unwrap();
            let mut b = new_state(64).unwrap();
            let ra = execute_program(&mut a, &program);
            let rb = execute_program(&mut b, &program);
            assert_eq!(ra.is_ok(), rb.is_ok());
            if ra.is_ok() {
                assert_eq!(a, b);
            }
        }
    }

    #[test]
    fn test_performance_smoke_mutation_execution() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(11);
        let mut programs = Vec::new();
        for step in 0..200u64 {
            programs.push(generate_order_with_rng(&mut rng, step + 1, 512).program);
        }

        let start = Instant::now();
        let mut state = new_state(512).unwrap();
        for program in &programs {
            let _ = execute_program(&mut state, program);
        }
        let elapsed = start.elapsed();
        // Wide bound for CI variability; this is a regression guard, not a strict benchmark.
        assert!(
            elapsed.as_secs_f64() < 2.0,
            "mutation execution too slow: {elapsed:?}"
        );
    }
}
