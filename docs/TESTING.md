# ChronoSeal Testing Strategy

ChronoSeal maintains a rigorous, security-first test suite focused on cryptographic correctness, deterministic server ↔ WASM parity, mutation engine integrity, replay resistance, tampering detection, behavioral validation, and storage reliability.

As of **v0.6.1**, the project contains **89 passing tests** across the server, WASM, and shared protocol crates.

| Crate               |  Tests |
| ------------------- | -----: |
| `chronoseal-server` |     30 |
| `chronoseal-wasm`   |     24 |
| `shared`            |     35 |
| **Total**           | **89** |

---

## Test Philosophy

ChronoSeal testing prioritizes:

* **Security invariants** over raw coverage metrics
* **Deterministic parity** between server and browser WASM runtimes
* **Negative-path testing** (tampering, replay, malformed input, edge cases)
* **Fuzz-style and randomized testing** for mutation logic
* **Performance regression detection**
* **Long-term protocol stability**

Particular emphasis is placed on ensuring that browser-side WASM execution produces identical results to server-side validation.

---

# Test Categories

## 1. Configuration & CLI

Configuration tests verify:

* Database backend selection
* TOML configuration parsing
* Command-line override behavior
* Default configuration values
* Runtime initialization logic

Supported backends include:

* `sqlite-in-memory`
* `sqlite-in-disk`
* `valkey`

Example tests:

```text
test_apply_run_args_overrides_db_type
test_default_db_type_is_sqlite_in_memory
test_toml_parses_db_type_kebab_case
test_init_db_pool_sqlite_in_memory
test_init_db_pool_sqlite_in_disk
test_init_db_pool_valkey_compat_mode
```

---

## 2. Session Lifecycle & Verification

Session tests validate:

* Session creation
* Public key validation
* Expiration handling
* Replay attack prevention
* Mutation step enforcement
* Commitment verification
* Long-running deterministic parity

Example tests:

```text
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

* Deterministic server/client parity
* Mutation order execution
* Gene state integrity
* Preview → Commit → Discard lifecycle
* Randomized mutation programs
* Edge-case validation
* Performance regression detection

Example tests:

```text
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

* Successful state advancement
* Silent rejection behavior
* Commitment validation
* Rate limiting
* Next-state mutation generation

Example tests:

```text
test_handler_success_returns_next_mutation_fields
test_handler_tampered_commitment_is_silent_failure
test_handler_rate_limit_returns_no_mutation_data
```

---

## 5. Trust & Behavioral Validation

Behavioral validation tests verify:

* Minimum mouse activity
* Minimum movement distance
* Pause detection
* Speed thresholds
* Optional activity requirements
* Fingerprint-related validation paths

Example tests:

```text
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

* SQLite in-memory operation
* SQLite disk-backed operation
* Valkey compatibility mode
* Session CRUD behavior
* Expiration cleanup
* Runtime statistics reporting

These tests ensure storage implementations remain interchangeable without affecting protocol behavior.

---

## 7. VM Core

The VM core is tested extensively across both WASM and shared crates.

Coverage includes:

* ADD
* SUB
* MUL
* XOR
* AND
* OR
* NOT
* HASH
* ROT
* PUSH

Edge cases include:

* Stack underflow
* Truncated instructions
* Unknown opcodes
* Wrapping arithmetic
* Invalid instruction streams

Example tests:

```text
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

# Server Test Coverage (`chronoseal-server`)

The server crate currently contains **30 tests** covering:

* Configuration
* Runtime initialization
* Session management
* Heartbeat validation
* Rate limiting
* Trust validation

The server tests focus heavily on protocol enforcement and security validation.

---

# WASM Test Coverage (`chronoseal-wasm`)

The WASM crate currently contains **24 tests** covering:

* VM execution
* Browser-side mutation lifecycle
* Gene initialization
* Mutation preview
* Mutation commit/discard behavior
* Deterministic parity with shared logic

Example tests:

```text
test_preview_commitment_matches_shared_engine
test_commit_applies_preview
test_discard_preview_keeps_committed_state
test_table_driven_parity_across_many_generated_orders
```

These tests ensure browser-generated commitments remain consistent with server expectations.

---

# Shared Crate Coverage (`shared`)

The shared crate currently contains **35 tests** and represents the core protocol implementation used by both server and browser runtimes.

Coverage includes:

### Synthetic Gene Engine

```text
test_new_state_with_default_size
test_new_state_rejects_invalid_sizes
test_commitment_changes_when_gene_or_environment_changes
test_encode_decode_environment_roundtrip
test_table_driven_randomized_environment_roundtrip
```

### Mutation Engine

```text
test_opcode_insert
test_opcode_delete
test_opcode_mutate_point
test_opcode_apply_mutagen
test_opcode_finalize_gene_hash
test_mutation_chain
```

### Validation & Hardening

```text
test_rejects_stack_underflow
test_rejects_truncated_instruction
test_rejects_unknown_opcode
test_zero_length_gene_is_rejected
```

### Deterministic Parity

```text
test_server_client_parity_across_random_orders
test_generate_order_is_deterministic_for_seeded_rng
test_invalid_positions_wrap_deterministically
```

### Fuzz & Regression Testing

```text
test_fuzz_style_random_program_bytes_do_not_diverge
test_performance_smoke_mutation_execution
```

---

# Running the Test Suite

Run the full workspace:

```bash
cargo test --workspace
```

Run individual crates:

```bash
cargo test -p shared
cargo test -p chronoseal-wasm
cargo test -p chronoseal-server
```

Display test output:

```bash
cargo test -- --nocapture
```

---

# Critical Security Tests

The following tests protect ChronoSeal's core protocol guarantees and should be treated as **release-blocking** if they fail:

```text
test_mutation_commitment_tamper_is_rejected
test_replay_attack_is_rejected
test_handler_tampered_commitment_is_silent_failure
test_server_client_parity_across_random_orders
test_deterministic_server_client_parity_across_many_heartbeats
test_fuzz_style_random_program_bytes_do_not_diverge
```

These tests directly validate resistance to replay attacks, protocol divergence, mutation tampering, and commitment forgery.

---

# Contributing New Tests

When adding new functionality:

1. Prefer placing protocol logic tests in `shared/`
2. Ensure server ↔ WASM parity is validated
3. Include negative-path test cases
4. Add randomized testing where appropriate
5. Update this document when introducing major new categories

---

# Future Improvements

Planned enhancements include:

* Property-based testing using `proptest`
* Browser-driven end-to-end integration tests
* Valkey concurrency and failover testing
* Automated benchmark execution in CI
* Expanded mutation-engine fuzzing
* CI-enforced performance regression thresholds

---

# Conclusion

ChronoSeal's testing strategy is centered on preserving deterministic behavior, cryptographic correctness, and protocol integrity.

The current suite of **89 tests** provides broad coverage across:

* Session security
* Heartbeat validation
* Mutation engine correctness
* Deterministic server/WASM parity
* Trust validation
* Storage abstraction
* Replay resistance
* Protocol hardening

Maintaining and expanding this test suite remains a core project priority as ChronoSeal evolves.

**Last Updated:** May 2026 (v0.6.1)

