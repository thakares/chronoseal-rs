use std::cell::RefCell;
use std::time::Instant;
use wasm_bindgen::prelude::*;

thread_local! {
    static GENE_STATE: RefCell<Option<shared::gene::GeneState>> = const { RefCell::new(None) };
    static PREVIEW_STATE: RefCell<Option<shared::gene::GeneState>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub fn init_gene_state(gene_size: u32) -> bool {
    let Ok(state) = shared::gene::new_state(gene_size as usize) else {
        return false;
    };
    GENE_STATE.with(|slot| *slot.borrow_mut() = Some(state));
    PREVIEW_STATE.with(|slot| *slot.borrow_mut() = None);
    true
}

#[wasm_bindgen]
pub fn preview_gene_commitment(order_b64: &str, session_id: &str, mutation_step: u64, rounds: u8) -> String {
    let order = match shared::vm_extensions::decode_order_b64(mutation_step, order_b64) {
        Ok(order) => order,
        Err(_) => return String::new(),
    };

    let start = Instant::now();
    let candidate = GENE_STATE.with(|slot| {
        let state = slot.borrow();
        let Some(current) = state.as_ref() else {
            return None;
        };
        shared::vm_extensions::apply_program_clone_with_rounds(current, &order.program, if rounds == 0 { shared::constants::DEFAULT_MUTATION_ROUNDS } else { rounds }).ok()
    });
    let elapsed = start.elapsed();
    tracing::debug!(session_id = %session_id, mutation_step = mutation_step, elapsed_ms = elapsed.as_millis(), "wasm mutation preview execution");

    let Some(candidate) = candidate else {
        return String::new();
    };
    let commitment = shared::gene::commitment_hex_with_context(&candidate, session_id, mutation_step);
    PREVIEW_STATE.with(|slot| *slot.borrow_mut() = Some(candidate));
    commitment
}

#[wasm_bindgen]
pub fn commit_gene_preview() -> bool {
    let next = PREVIEW_STATE.with(|slot| slot.borrow_mut().take());
    let Some(next) = next else {
        return false;
    };
    GENE_STATE.with(|slot| *slot.borrow_mut() = Some(next));
    true
}

#[wasm_bindgen]
pub fn discard_gene_preview() {
    PREVIEW_STATE.with(|slot| *slot.borrow_mut() = None);
}

#[wasm_bindgen]
pub fn current_gene_commitment(session_id: &str, mutation_step: u64) -> String {
    GENE_STATE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|state| shared::gene::commitment_hex_with_context(state, session_id, mutation_step))
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn order_b64(program: Vec<u8>) -> String {
        let order = shared::vm_extensions::MutationOrder { step: 1, program };
        shared::vm_extensions::encode_order_b64(&order)
    }

    #[test]
    fn test_init_gene_state_success() {
        assert!(init_gene_state(64));
        let commitment = current_gene_commitment("deadbeef", 1);
        assert_eq!(commitment.len(), 64);
    }

    #[test]
    fn test_init_gene_state_rejects_zero() {
        assert!(!init_gene_state(0));
    }

    #[test]
    fn test_preview_requires_initialized_state() {
        discard_gene_preview();
        GENE_STATE.with(|slot| *slot.borrow_mut() = None);
        let c = preview_gene_commitment(
            &order_b64(vec![
                shared::vm_extensions::OP_MUTATE_POINT,
                0,
                0,
                1,
            ]),
            "deadbeef",
            1,
            0,
        );
        assert!(c.is_empty());
    }

    #[test]
    fn test_preview_rejects_invalid_order() {
        init_gene_state(16);
        let c = preview_gene_commitment("***bad-base64***", "deadbeef", 1, 0);
        assert!(c.is_empty());
    }

    #[test]
    fn test_commit_applies_preview() {
        init_gene_state(16);
        let before = current_gene_commitment("deadbeef", 1);
        let order = order_b64(vec![shared::vm_extensions::OP_MUTATE_POINT, 0, 0, 1]);
        let preview = preview_gene_commitment(&order, "deadbeef", 1, 0);
        assert_ne!(preview, before);
        assert!(commit_gene_preview());
        let after = current_gene_commitment("deadbeef", 1);
        assert_eq!(preview, after);
    }

    #[test]
    fn test_discard_preview_keeps_committed_state() {
        init_gene_state(16);
        let before = current_gene_commitment("deadbeef", 1);
        let order = order_b64(vec![shared::vm_extensions::OP_MUTATE_POINT, 0, 0, 0xFF]);
        let preview = preview_gene_commitment(&order, "deadbeef", 1, 0);
        assert_ne!(preview, before);
        discard_gene_preview();
        let after = current_gene_commitment("deadbeef", 1);
        assert_eq!(before, after);
    }

    #[test]
    fn test_commit_without_preview_returns_false() {
        init_gene_state(16);
        discard_gene_preview();
        assert!(!commit_gene_preview());
    }

    #[test]
    fn test_preview_commitment_matches_shared_engine() {
        init_gene_state(16);
        let order = shared::vm_extensions::MutationOrder {
            step: 3,
            program: vec![
                shared::vm_extensions::OP_GENE_LOAD,
                0,
                0,
                shared::vm_extensions::OP_PRODUCE,
                1,
                0,
                shared::vm_extensions::OP_GENE_LOAD,
                2,
                0,
                shared::vm_extensions::OP_APPLY_MUTAGEN,
                1,
                0,
                2,
                0,
            ],
        };
        let b64 = shared::vm_extensions::encode_order_b64(&order);

        let preview = preview_gene_commitment(&b64, "deadbeef", 3, 0);

        let mut expected = shared::gene::new_state(16).unwrap();
        shared::vm_extensions::apply_program_with_rounds(&mut expected, &order.program, shared::constants::DEFAULT_MUTATION_ROUNDS).unwrap();
        assert_eq!(preview, shared::gene::commitment_hex_with_context(&expected, "deadbeef", 3));
    }

    #[test]
    fn test_table_driven_parity_across_many_generated_orders() {
        init_gene_state(64);
        let mut rng = rand::rngs::StdRng::seed_from_u64(123);
        let mut expected = shared::gene::new_state(64).unwrap();

        for step in 0..24u64 {
            let order = shared::vm_extensions::generate_order_with_rng(&mut rng, step + 1, 64);
            let b64 = shared::vm_extensions::encode_order_b64(&order);

            let preview = preview_gene_commitment(&b64, "deadbeef", step + 1, 0);
            shared::vm_extensions::apply_program_with_rounds(
                &mut expected,
                &order.program,
                shared::constants::DEFAULT_MUTATION_ROUNDS,
            )
            .unwrap();
            let expected_commitment = shared::gene::commitment_hex_with_context(&expected, "deadbeef", step + 1);

            assert_eq!(preview, expected_commitment);
            assert!(commit_gene_preview());
            assert_eq!(current_gene_commitment("deadbeef", step + 1), expected_commitment);
        }
    }
}
