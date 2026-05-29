# ChronoSeal Testing Strategy and Suite

This document describes the testing strategy for ChronoSeal and summarizes the test coverage included in v0.6.1.

ChronoSeal is a security-focused system. Testing therefore prioritizes cryptographic correctness, deterministic execution, protocol integrity, and resistance to replay or tampering rather than simple line coverage.

## Overview

As of v0.6.1, the ChronoSeal workspace contains:

| Crate               |  Tests |
| ------------------- | -----: |
| `chronoseal-server` |     30 |
| `chronoseal-wasm`   |     24 |
| `shared`            |     35 |
| **Total**           | **89** |

All tests pass successfully on the reference development environment.

## Testing Philosophy

ChronoSeal testing focuses on:

1. Cryptographic correctness
2. Deterministic server ↔ WASM parity
3. Replay and tampering resistance
4. Mutation engine integrity
5. Negative-path validation
6. Storage reliability
7. Performance regression detection

Particular emphasis is placed on ensuring that browser-side WASM execution produces identical results to server-side validation.

---

# Server Test Coverage (`chronoseal-server`)

The server crate contains 30 tests covering configuration, runtime initialization, session lifecycle management, heartbeat validation, rate limiting, and behavioral trust checks.

## Configuration

Configuration tests verify:

* database type parsing
* TOML configuration loading
* default value handling
* command-line override behavior

Examples:

```text
test_apply_run_args_overrides_db_type
test_default_db_type_is_sqlite_in_memory
test_toml_parses_db_type_kebab_case
```

## Runtime Initialization

Backend initialization tests verify:

* SQLite in-memory mode
* SQLite disk-backed mode
* Valkey compatibility mode

Examples:

```text
test_init_db_pool_sqlite_in_memory
test_init_db_pool_sqlite_in_disk
test_init_db_pool_valkey_compat_mode
```

## Session Lifecycle and Security

Session tests validate:

* public key validation
* session expiration
* replay attack prevention
* mutation step enforcement
* commitment verification
* long-running deterministic parity

Examples:

```text
test_create_session_rejects_invalid_public_key_length
test_expired_session_is_rejected
test_replay_attack_is_rejected
test_mutation_step_mismatch_is_rejected
test_mutation_commitment_tamper_is_rejected
test_session_lifecycle_and_verification
test_deterministic_server_client_parity_across_many_heartbeats
```

## Heartbeat Validation

Heartbeat tests verify:

* successful state advancement
* silent rejection behavior
* rate limiting

Examples:

```text
test_handler_success_returns_next_mutation_fields
test_handler_tampered_commitment_is_silent_failure
test_handler_rate_limit_returns_no_mutation_data
```

## Behavioral Trust Validation

Trust checks verify:

* minimum mouse activity
* minimum distance traveled
* pause detection
* speed thresholds
* optional activity requirements

Examples:

```text
test_validate_mouse_success
test_validate_mouse_insufficient_events
test_validate_mouse_insufficient_distance
test_validate_mouse_too_fast
test_validate_mouse_no_pauses
```

## Rate Limiting

Examples:

```text
test_rate_limiter
test_rate_limiter_eviction
```

---

# WASM Test Coverage (`chronoseal-wasm`)

The WASM crate contains 24 tests covering both the virtual machine and browser-side mutation lifecycle.

## Virtual Machine

Opcode correctness is validated for:

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

* stack underflow
* truncated instructions
* wrapping arithmetic

Examples:

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
```

## Browser Mutation Lifecycle

The browser runtime tests:

* gene initialization
* mutation preview
* mutation commit
* mutation discard
* commitment parity

Examples:

```text
test_init_gene_state_success
test_init_gene_state_rejects_zero
test_preview_commitment_matches_shared_engine
test_commit_applies_preview
test_discard_preview_keeps_committed_state
```

## Deterministic Parity

Examples:

```text
test_table_driven_parity_across_many_generated_orders
```

These tests ensure browser-generated commitments remain consistent with server expectations.

---

# Shared Crate Coverage (`shared`)

The shared crate contains 35 tests and represents the core security-critical logic of ChronoSeal.

This crate receives the heaviest protocol-focused testing because it is shared by both server and WASM runtimes.

## Synthetic Gene Engine

Gene state tests validate:

* initialization rules
* environment encoding
* environment decoding
* commitment generation
* quantity management

Examples:

```text
test_new_state_with_default_size
test_new_state_rejects_invalid_sizes
test_commitment_changes_when_gene_or_environment_changes
test_encode_decode_environment_roundtrip
test_table_driven_randomized_environment_roundtrip
```

## Mutation Engine

Mutation engine tests verify:

* deterministic execution
* mutation chains
* opcode correctness
* stack handling
* instruction validation

Examples:

```text
test_mutation_chain
test_opcode_insert
test_opcode_delete
test_opcode_mutate_point
test_opcode_apply_mutagen
test_opcode_finalize_gene_hash
```

## Validation and Hardening

Defensive validation tests include:

```text
test_rejects_stack_underflow
test_rejects_truncated_instruction
test_rejects_unknown_opcode
test_zero_length_gene_is_rejected
```

## Deterministic Server ↔ WASM Parity

These are among the most important tests in the project:

```text
test_server_client_parity_across_random_orders
test_generate_order_is_deterministic_for_seeded_rng
test_invalid_positions_wrap_deterministically
```

## Fuzz and Regression Testing

Examples:

```text
test_fuzz_style_random_program_bytes_do_not_diverge
test_performance_smoke_mutation_execution
```

These tests help detect behavioral divergence and unintended performance regressions.

---

# Running the Test Suite

Run the full workspace:

```bash
cargo test --workspace
```

Run individual crates:

```bash
cargo test -p chronoseal-server
cargo test -p chronoseal-wasm
cargo test -p shared
```

Show test output:

```bash
cargo test -- --nocapture
```

---

# Critical Security Tests

The following tests protect core ChronoSeal security guarantees:

```text
test_mutation_commitment_tamper_is_rejected
test_replay_attack_is_rejected
test_handler_tampered_commitment_is_silent_failure
test_server_client_parity_across_random_orders
test_deterministic_server_client_parity_across_many_heartbeats
test_fuzz_style_random_program_bytes_do_not_diverge
```

Any failure in these areas should be treated as a release-blocking issue.

---

# Future Improvements

Planned enhancements include:

* property-based testing using `proptest`
* browser-driven end-to-end integration tests
* Valkey concurrency testing
* automated benchmark execution
* expanded mutation-engine fuzzing
* CI-enforced performance regression thresholds

---

# Conclusion

ChronoSeal's testing strategy is centered on preserving deterministic behavior, cryptographic correctness, and protocol integrity.

The current suite of 89 tests provides broad coverage across:

* session security
* heartbeat validation
* mutation engine correctness
* deterministic server/WASM parity
* trust validation
* storage abstraction
* replay resistance

As ChronoSeal evolves, expanding and strengthening this test suite remains a core project priority.

**Last Updated:** May 2026 (v0.6.1)
