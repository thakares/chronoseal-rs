# ChronoSeal Testing Strategy

ChronoSeal maintains a rigorous, security-first test suite focused on cryptographic correctness, deterministic server ↔ WASM parity, mutation engine integrity, replay resistance, tampering detection, behavioral validation, and storage reliability.

As of **v1.0.1**, the project contains **95 passing tests** across the server, WASM, and shared protocol crates.

| Crate                        | Tests  |
| ---------------------------- | ------ |
| `chronoseal-server`          | 33     |
| `chronoseal-wasm`            | 24     |
| `shared` (unit)              | 36     |
| `shared` (property-based)    | 2      |
| **Total**                    | **95** |

---

## Test Philosophy

ChronoSeal testing prioritizes:

- **Security invariants** over raw coverage metrics
- **Deterministic parity** between server and browser WASM runtimes
- **Negative-path testing** (tampering, replay, malformed input, edge cases)
- **Property-based and fuzz-style testing** for mutation logic and VM robustness
- **Performance regression detection**
- **Long-term protocol stability**

Particular emphasis is placed on ensuring that browser-side WASM execution produces identical results to server-side validation.

---

# Test Categories

## 1. Configuration & CLI

Configuration tests verify:

- Database backend selection
- TOML configuration parsing
- Command-line override behavior
- Default configuration values
- Runtime initialization logic

Supported backends include:

- `sqlite-in-memory`
- `sqlite-in-disk`
- `valkey` (redis-compatible via r2d2 connection pool)

Example tests:

```
test_apply_run_args_overrides_db_type
test_default_db_type_is_sqlite_in_memory
test_toml_parses_db_type_kebab_case
test_init_db_pool_sqlite_in_memory
test_init_db_pool_sqlite_in_disk
test_init_db_pool_valkey_compat_mode
test_valkey_store_operations
```

---

## 2. Session Lifecycle & Verification

Session tests validate:

- Session creation
- Public key validation
- Expiration handling
- Replay attack prevention
- Mutation step enforcement
- Commitment verification
- Long-running deterministic parity

Example tests:

```
test_create_session_rejects_invalid_public_key_length
test_expired_session_is_rejected
test_replay_attack_is_rejected
test_mutation_step_mismatch_is_rejected
test_mutation_commitment_tamper_is_rejected
test_session_lifecycle_and_verification
test_repeated_simulation_keeps_server_and_client_commitments_equal
test_deterministic_server_client_parity_across_many_heartbeats
```

---

## 3. Mutation Engine (Core Focus)

The Synthetic Gene Mutation Engine is one of the most security-critical components in ChronoSeal.

Testing focuses on:

- Deterministic server/client parity
- Mutation order execution
- Gene state integrity
- Preview → Commit → Discard lifecycle
- Randomized mutation programs
- Edge-case validation
- Performance regression detection

Example tests:

```
test_server_client_parity_across_random_orders
test_generate_order_is_deterministic_for_seeded_rng
test_invalid_positions_wrap_deterministically
test_mutation_chain
test_fuzz_style_random_program_bytes_do_not_diverge
test_performance_smoke_mutation_execution
```

---

## 4. Heartbeat Handler

Heartbeat validation tests verify:

- Successful state advancement
- Silent rejection behavior
- Commitment validation
- Rate limiting
- Next-state mutation generation

Example tests:

```
test_handler_success_returns_next_mutation_fields
test_handler_tampered_commitment_is_silent_failure
test_handler_rate_limit_returns_no_mutation_data
```

---

## 5. Trust & Behavioral Validation

Behavioral validation tests verify:

- Minimum mouse activity
- Minimum movement distance
- Pause detection
- Speed thresholds
- Optional activity requirements
- Fingerprint-related validation paths

Example tests:

```
test_validate_mouse_success
test_validate_mouse_insufficient_events
test_validate_mouse_insufficient_distance
test_validate_mouse_too_fast
test_validate_mouse_no_pauses
test_validate_mouse_require_activity_toggle
```

---

## 6. Storage Layer

Storage tests verify:

- SQLite in-memory operation
- SQLite disk-backed operation and pool concurrency
- Valkey compatibility mode and r2d2 pool concurrency
- Session CRUD behavior including the `opcodes` field
- Expiration cleanup
- Runtime statistics reporting

These tests ensure storage implementations remain interchangeable without affecting protocol behavior.

Example tests:

```
test_sqlite_pool_concurrency
test_valkey_pool_concurrency
test_valkey_store_operations
```

> **Note:** The concurrent write collision path in `update_session` (the `old_last_hash`
> optimistic concurrency guard) is not yet covered by an automated test. Two goroutines
> advancing the same chain simultaneously is a security-relevant race condition.
> A dedicated test is planned for v1.1.0 (see Future Improvements).

---

## 7. VM Core

The VM core is tested extensively across both WASM and shared crates.

Coverage includes:

- ADD, SUB, MUL, XOR, AND, OR, NOT, HASH, ROT, PUSH

Edge cases include:

- Stack underflow
- Truncated instructions
- Unknown opcodes
- Wrapping arithmetic
- Invalid instruction streams

Example tests:

```
test_add
test_add_wrapping
test_sub
test_sub_wrapping
test_mul
test_hash
test_underflow_binary
test_underflow_unary
test_incomplete_push
test_rejects_unknown_opcode
test_rejects_truncated_instruction
```

---

## 8. Property-Based Tests

ChronoSeal uses [`proptest`](https://github.com/proptest-rs/proptest) for property-based testing of core protocol invariants against arbitrary random input.

Tests live in `shared/tests/proptests.rs` and run as part of `cargo test --workspace`.

```
test_vm_execute_never_panics
test_gene_environment_roundtrip_never_panics
```

`test_vm_execute_never_panics` feeds arbitrary `Vec<u8>` byte sequences into the stack machine
and asserts that execution never panics and that the instruction pointer never exceeds the
program length. This guards against any future VM opcode handler introducing undefined
behaviour on malformed input.

`test_gene_environment_roundtrip_never_panics` feeds arbitrary bytes into
`gene::decode_environment` and asserts graceful failure rather than a panic, covering the full
space of malformed environment payloads a client could send.

---

# Server Test Coverage (`chronoseal-server`)

The server crate currently contains **33 tests** covering:

- Configuration
- Runtime initialization
- Session management
- Heartbeat validation
- Rate limiting
- Trust validation

The server tests focus heavily on protocol enforcement and security validation.

---

# WASM Test Coverage (`chronoseal-wasm`)

The WASM crate currently contains **24 tests** covering:

- VM execution
- Browser-side mutation lifecycle
- Gene initialization
- Mutation preview
- Mutation commit/discard behavior
- Deterministic parity with shared logic

Example tests:

```
test_preview_commitment_matches_shared_engine
test_commit_applies_preview
test_discard_preview_keeps_committed_state
test_table_driven_parity_across_many_generated_orders
```

These tests ensure browser-generated commitments remain consistent with server expectations.

---

# Shared Crate Coverage (`shared`)

The shared crate currently contains **36 unit tests** and **2 property-based tests**, representing
the core protocol implementation used by both server and browser runtimes.

Coverage includes:

### Synthetic Gene Engine

```
test_new_state_with_default_size
test_new_state_rejects_invalid_sizes
test_commitment_changes_when_gene_or_environment_changes
test_encode_decode_environment_roundtrip
test_table_driven_randomized_environment_roundtrip
```

### Mutation Engine

```
test_opcode_insert
test_opcode_delete
test_opcode_mutate_point
test_opcode_apply_mutagen
test_opcode_finalize_gene_hash
test_mutation_chain
```

### Validation & Hardening

```
test_rejects_stack_underflow
test_rejects_truncated_instruction
test_rejects_unknown_opcode
test_zero_length_gene_is_rejected
```

### Deterministic Parity

```
test_server_client_parity_across_random_orders
test_generate_order_is_deterministic_for_seeded_rng
test_invalid_positions_wrap_deterministically
```

### Fuzz & Regression Testing

```
test_fuzz_style_random_program_bytes_do_not_diverge
test_performance_smoke_mutation_execution
test_vm_instruction_budget_soft_cap
```

### Property-Based Tests (`shared/tests/proptests.rs`)

```
test_vm_execute_never_panics
test_gene_environment_roundtrip_never_panics
```

---

# Tooling Crates

## `chronoseal-replay`

A standalone replay and audit tool for offline verification of recorded ChronoSeal session
chains. It is a developer and forensic utility, not a library, and currently carries no
automated tests. Integration tests against captured session fixtures are planned.

## `fuzz/`

Contains libFuzzer targets for deeper coverage of the VM and gene codec. Run separately
via `cargo +nightly fuzz run <target>` — not part of the standard `cargo test` suite.

---

# Running the Test Suite

Run the full workspace:

```
cargo test --workspace
```

Run individual crates:

```
cargo test -p shared
cargo test -p chronoseal-wasm
cargo test -p chronoseal-server
```

Display test output:

```
cargo test -- --nocapture
```

---

# Critical Security Tests

The following tests protect ChronoSeal's core protocol guarantees and should be treated as
**release-blocking** if they fail:

```
test_mutation_commitment_tamper_is_rejected
test_replay_attack_is_rejected
test_handler_tampered_commitment_is_silent_failure
test_server_client_parity_across_random_orders
test_deterministic_server_client_parity_across_many_heartbeats
test_fuzz_style_random_program_bytes_do_not_diverge
test_vm_execute_never_panics
test_gene_environment_roundtrip_never_panics
```

These tests directly validate resistance to replay attacks, protocol divergence, mutation
tampering, commitment forgery, and VM panic on adversarial input.

---

# Contributing New Tests

When adding new functionality:

1. Prefer placing protocol logic tests in `shared/`
2. Ensure server ↔ WASM parity is validated
3. Include negative-path test cases
4. Add randomized or property-based testing where appropriate
5. Update this document when introducing major new categories

---

# Future Improvements

- **Concurrent chain write collision test** — verify that two simultaneous heartbeats for the
  same session are handled correctly by the `old_last_hash` optimistic concurrency guard in
  `update_session` (security-critical, planned for v1.1.0)
- **Browser-driven end-to-end integration tests** — full Playwright or wasm-bindgen-test
  harness exercising the complete init → heartbeat loop in a real browser environment
- **Valkey failover testing** — verify graceful degradation and reconnection under r2d2 pool
  exhaustion and server-side connection drops
- **Automated benchmark execution in CI** — enforce performance regression thresholds for
  mutation engine and hash chain operations
- **Expanded mutation-engine fuzzing** — additional libFuzzer targets for `vm_extensions`
  opcodes introduced in v0.7.0
- **CI-enforced performance regression thresholds** — gate releases on measured latency bounds

---

# Conclusion

ChronoSeal's testing strategy is centered on preserving deterministic behavior, cryptographic
correctness, and protocol integrity.

The current suite of **95 tests** provides broad coverage across:

- Session security
- Heartbeat validation
- Mutation engine correctness
- Deterministic server/WASM parity
- Trust and behavioral validation
- Storage abstraction
- Replay resistance
- Protocol hardening
- Property-based VM and gene codec robustness

Maintaining and expanding this test suite remains a core project priority as ChronoSeal evolves.

**Last Updated:** May 2026 (v1.0.1)
