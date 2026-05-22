use shared::protocol::StackState;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run_program(program_b64: &str) -> JsValue {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(program_b64)
        .unwrap();
    let state = execute(&bytes);
    serde_wasm_bindgen::to_value(&state).unwrap()
}

fn execute(program: &[u8]) -> StackState {
    let mut stack: Vec<u32> = Vec::new();
    let mut ip: usize = 0;
    while ip < program.len() {
        let op = program[ip];
        ip += 1;
        match op {
            0x00 => {
                if ip + 4 > program.len() {
                    break;
                }
                let val = u32::from_le_bytes([
                    program[ip],
                    program[ip + 1],
                    program[ip + 2],
                    program[ip + 3],
                ]);
                ip += 4;
                stack.push(val);
            }
            0x01..=0x07 => {
                if stack.len() < 2 {
                    break;
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let r = match op {
                    0x01 => a.wrapping_add(b),
                    0x02 => a.wrapping_sub(b),
                    0x03 => a.wrapping_mul(b),
                    0x04 => a ^ b,
                    0x05 => a & b,
                    0x06 => a | b,
                    0x07 => a.rotate_left(b % 32),
                    _ => unreachable!(),
                };
                stack.push(r);
            }
            0x08 => {
                if stack.is_empty() {
                    break;
                }
                let a = stack.pop().unwrap();
                stack.push(!a);
            }
            0x09 => {
                let r = shared::hashing::hash_stack(&stack);
                stack.clear();
                stack.push(r);
            }
            _ => break,
        }
    }
    StackState {
        stack,
        ip: ip as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push() {
        // PUSH 42, PUSH 100
        let program = vec![0x00, 42, 0, 0, 0, 0x00, 100, 0, 0, 0];
        let state = execute(&program);
        assert_eq!(state.stack, vec![42, 100]);
        assert_eq!(state.ip, 10);
    }

    #[test]
    fn test_add() {
        // PUSH 5, PUSH 10, ADD
        let program = vec![0x00, 5, 0, 0, 0, 0x00, 10, 0, 0, 0, 0x01];
        let state = execute(&program);
        assert_eq!(state.stack, vec![15]);
    }

    #[test]
    fn test_add_wrapping() {
        // PUSH u32::MAX, PUSH 1, ADD
        let program = vec![0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 1, 0, 0, 0, 0x01];
        let state = execute(&program);
        assert_eq!(state.stack, vec![0]);
    }

    #[test]
    fn test_sub() {
        // PUSH 20, PUSH 7, SUB
        let program = vec![0x00, 20, 0, 0, 0, 0x00, 7, 0, 0, 0, 0x02];
        let state = execute(&program);
        assert_eq!(state.stack, vec![13]);
    }

    #[test]
    fn test_sub_wrapping() {
        // PUSH 0, PUSH 1, SUB
        let program = vec![0x00, 0, 0, 0, 0, 0x00, 1, 0, 0, 0, 0x02];
        let state = execute(&program);
        assert_eq!(state.stack, vec![u32::MAX]);
    }

    #[test]
    fn test_mul() {
        // PUSH 6, PUSH 7, MUL
        let program = vec![0x00, 6, 0, 0, 0, 0x00, 7, 0, 0, 0, 0x03];
        let state = execute(&program);
        assert_eq!(state.stack, vec![42]);
    }

    #[test]
    fn test_xor() {
        // PUSH 0b1010, PUSH 0b1100, XOR
        let program = vec![0x00, 0b1010, 0, 0, 0, 0x00, 0b1100, 0, 0, 0, 0x04];
        let state = execute(&program);
        assert_eq!(state.stack, vec![0b0110]);
    }

    #[test]
    fn test_and() {
        // PUSH 0b1010, PUSH 0b1100, AND
        let program = vec![0x00, 0b1010, 0, 0, 0, 0x00, 0b1100, 0, 0, 0, 0x05];
        let state = execute(&program);
        assert_eq!(state.stack, vec![0b1000]);
    }

    #[test]
    fn test_or() {
        // PUSH 0b1010, PUSH 0b1100, OR
        let program = vec![0x00, 0b1010, 0, 0, 0, 0x00, 0b1100, 0, 0, 0, 0x06];
        let state = execute(&program);
        assert_eq!(state.stack, vec![0b1110]);
    }

    #[test]
    fn test_rot() {
        // PUSH 1, PUSH 4, ROT
        let program = vec![0x00, 1, 0, 0, 0, 0x00, 4, 0, 0, 0, 0x07];
        let state = execute(&program);
        assert_eq!(state.stack, vec![16]);
    }

    #[test]
    fn test_not() {
        // PUSH 0, NOT
        let program = vec![0x00, 0, 0, 0, 0, 0x08];
        let state = execute(&program);
        assert_eq!(state.stack, vec![u32::MAX]);
    }

    #[test]
    fn test_hash() {
        // PUSH 10, PUSH 20, HASH
        let program = vec![0x00, 10, 0, 0, 0, 0x00, 20, 0, 0, 0, 0x09];
        let state = execute(&program);
        assert_eq!(state.stack.len(), 1);
        let expected_hash = shared::hashing::hash_stack(&[10, 20]);
        assert_eq!(state.stack[0], expected_hash);
    }

    #[test]
    fn test_underflow_binary() {
        // PUSH 42, ADD (needs 2 values, only 1 on stack)
        let program = vec![0x00, 42, 0, 0, 0, 0x01];
        let state = execute(&program);
        // ADD breaks when stack.len() < 2, stack has 42 left, ip is at the opcode ADD (6)
        assert_eq!(state.stack, vec![42]);
        assert_eq!(state.ip, 6);
    }

    #[test]
    fn test_underflow_unary() {
        // NOT (needs 1 value, empty stack)
        let program = vec![0x08];
        let state = execute(&program);
        assert_eq!(state.stack, Vec::<u32>::new());
        assert_eq!(state.ip, 1);
    }

    #[test]
    fn test_incomplete_push() {
        // PUSH opcode, but only 2 bytes instead of 4
        let program = vec![0x00, 42, 0];
        let state = execute(&program);
        assert_eq!(state.stack, Vec::<u32>::new());
        assert_eq!(state.ip, 1); // execution stopped at op 0x00 because ip + 4 > program.len()
    }
}
