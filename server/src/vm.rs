use rand::Rng;
use shared::{
    gene::GeneState,
    vm_extensions::{self, ExecutionTrace, MutationError, MutationOrder},
};

/// Generates a randomized VM opcode instruction program within a length range.
///
/// Builds a program of mathematical and stack ops (e.g. literals, ADD, SUB, XOR, HASH)
/// with dynamic depth checking to ensure valid stacks and prevent out of bounds execution.
///
/// # Arguments
/// * `len_range` - The inclusive range of instruction counts to generate.
pub fn generate_random_program(len_range: std::ops::RangeInclusive<usize>) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let count = rng.gen_range(len_range);
    let mut ops = Vec::new();
    let mut depth: i32 = 0;
    for _ in 0..count {
        if depth < 2 {
            // Not enough operands for any binary op — push a literal.
            ops.push(0x00);
            let val = rng.r#gen::<u32>();
            ops.extend_from_slice(&val.to_le_bytes());
            depth += 1;
        } else {
            let op = rng.gen_range(0u8..10);
            match op {
                0x00 => {
                    // PUSH literal
                    ops.push(0x00);
                    let val = rng.r#gen::<u32>();
                    ops.extend_from_slice(&val.to_le_bytes());
                    depth += 1;
                }
                0x01..=0x07 => {
                    // Binary ops (ADD, SUB, MUL, XOR, AND, OR, ROT): pops 2, pushes 1 → net −1
                    ops.push(op);
                    depth -= 1;
                }
                0x08 => {
                    // Unary NOT: pops 1, pushes 1 → net 0; depth unchanged
                    ops.push(0x08);
                }
                0x09 => {
                    // HASH: collapses entire stack to one u32 → depth becomes 1
                    ops.push(0x09);
                    depth = 1;
                }
                _ => unreachable!(),
            }
        }
    }
    ops
}

/// Executes a raw VM mutation program bytecode slice against a `GeneState`.
///
/// # Arguments
/// * `state` - The mutable gene state to mutate.
/// * `program` - The raw VM instruction program.
#[allow(dead_code)]
pub fn execute_mutation_program(
    state: &mut GeneState,
    program: &[u8],
) -> Result<ExecutionTrace, MutationError> {
    vm_extensions::execute_program(state, program)
}

/// Executes a `MutationOrder` program against a `GeneState`.
///
/// # Arguments
/// * `state` - The mutable gene state.
/// * `order` - The mutation order.
#[allow(dead_code)]
pub fn execute_mutation_order(
    state: &mut GeneState,
    order: &MutationOrder,
) -> Result<ExecutionTrace, MutationError> {
    vm_extensions::execute_program(state, &order.program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use shared::gene::{commitment, new_state};

    #[test]
    fn test_execute_mutation_program_wraps_shared_engine() {
        let mut state = new_state(8).unwrap();
        let program = vec![vm_extensions::OP_MUTATE_POINT, 0, 0, 1];
        let trace = execute_mutation_program(&mut state, &program).unwrap();
        assert_eq!(state.gene[0], 1);
        assert_eq!(trace.final_ip, program.len());
    }

    #[test]
    fn test_execute_mutation_order_determinism() {
        let mut rng_a = rand::rngs::StdRng::seed_from_u64(101);
        let mut rng_b = rand::rngs::StdRng::seed_from_u64(101);
        let order_a = vm_extensions::generate_order_with_rng(&mut rng_a, 9, 64);
        let order_b = vm_extensions::generate_order_with_rng(&mut rng_b, 9, 64);
        assert_eq!(order_a, order_b);

        let mut state_a = new_state(64).unwrap();
        let mut state_b = new_state(64).unwrap();
        let trace_a = execute_mutation_order(&mut state_a, &order_a).unwrap();
        let trace_b = execute_mutation_order(&mut state_b, &order_b).unwrap();

        assert_eq!(state_a, state_b);
        assert_eq!(trace_a.final_stack, trace_b.final_stack);
        assert_eq!(commitment(&state_a), commitment(&state_b));
    }
}
