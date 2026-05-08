use rand::Rng;

pub fn generate_random_program(len_range: std::ops::RangeInclusive<usize>) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let count = rng.gen_range(len_range);
    let mut ops = Vec::new();
    let mut depth: i32 = 0;
    for _ in 0..count {
        if depth < 2 {
            // Not enough operands for any binary op — push a literal.
            ops.push(0x00);
            let val = rng.gen::<u32>();
            ops.extend_from_slice(&val.to_le_bytes());
            depth += 1;
        } else {
            let op = rng.gen_range(0u8..10);
            match op {
                0x00 => {
                    // PUSH literal
                    ops.push(0x00);
                    let val = rng.gen::<u32>();
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

// Server does not need to execute the program; client does.