use wasm_bindgen::prelude::*;
use shared::protocol::StackState;

#[wasm_bindgen]
pub fn run_program(program_b64: &str) -> JsValue {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(program_b64).unwrap();
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
                if ip + 4 > program.len() { break; }
                let val = u32::from_le_bytes([program[ip], program[ip+1], program[ip+2], program[ip+3]]);
                ip += 4;
                stack.push(val);
            }
            0x01..=0x07 => {
                if stack.len() < 2 { break; }
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
                if stack.is_empty() { break; }
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
    StackState { stack, ip: ip as u16 }
}