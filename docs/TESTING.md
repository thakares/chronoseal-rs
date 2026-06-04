# ChronoSeal Testing Strategy

ChronoSeal maintains a security-focused test suite designed to validate cryptographic correctness, deterministic server ↔ WASM parity, replay resistance, mutation engine integrity, browser fingerprint validation, behavioral trust checks, storage reliability, and protocol hardening.

As of **v1.0.2**, the project contains **100 passing tests** across the server, WASM, shared protocol, and property-testing suites.

| Crate                           |   Tests |
| ------------------------------- | ------: |
| `chronoseal-server`             |      38 |
| `chronoseal-wasm`               |      24 |
| `shared` (unit tests)           |      36 |
| `shared` (property-based tests) |       2 |
| `chronoseal-replay`             |       0 |
| **Total**                       | **100** |

---

# Test Philosophy

ChronoSeal prioritizes testing of security invariants rather than raw coverage percentages.

Primary goals:

* Verify deterministic server ↔ WASM behavior
* Detect protocol divergence early
* Prevent replay attacks
* Validate mutation engine correctness
* Detect malformed and adversarial input handling
* Prevent VM and protocol panics
* Maintain storage backend compatibility
* Protect browser attestation continuity guarantees

The project emphasizes negative-path testing and adversarial validation rather than only testing successful execution paths.

---

# Test Categories

## 1. Configuration & Runtime

Configuration tests verify:

* Default configuration values
* TOML parsing
* Runtime initialization
* Database backend selection
* CLI override behavior

Covered functionality:

* SQLite in-memory backend
* SQLite disk backend
* Valkey compatibility mode
* Runtime configuration validation

Example tests:

```text
test_default_db_type_is_sqlite_in_memory
test_apply_run_args_overrides_db_type
test_toml_parses_db_type_kebab_case
test_db_type_report_lists_backends
test_init_db_pool_sqlite_in_memory
test_init_db_pool_sqlite_in_disk
test_init_db_pool_valkey_compat_mode
```

---

## 2. Browser Fingerprint Validation

Introduced and expanded in v1.0.2.

Fingerprint validation protects the attestation pipeline from malformed or unrealistic browser metadata.

Validation coverage includes:

* Aspect ratio validation
* Device pixel ratio validation
* Hardware concurrency validation
* Boundary value acceptance
* NaN rejection
* Infinity rejection
* Malformed numeric value rejection

Example tests:

```text
accepts_valid_fingerprint
accepts_boundary_values
rejects_invalid_aspect_ratios
rejects_invalid_device_pixel_ratios
rejects_invalid_hardware_concurrency
```

Validation constraints currently include:

| Field               | Allowed Range         |
| ------------------- | --------------------- |
| aspectRatio         | finite positive value |
| devicePixelRatio    | greater than zero     |
| hardwareConcurrency | 1..=256               |

---

## 3. Session Lifecycle & Protocol Verification

Session tests verify:

* Session creation
* Session expiration
* Public key validation
* Replay attack resistance
* Mutation commitment verification
* Mutation step enforcement
* Deterministic long-running parity

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

## 4. Heartbeat Validation

Heartbeat tests validate:

* Successful state advancement
* Silent rejection semantics
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

## 5. Behavioral Trust Validation

Trust validation focuses on lightweight behavioral signals.

Coverage includes:

* Minimum event count
* Minimum movement distance
* Pause detection
* Maximum speed thresholds
* Activity requirement toggles

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

Storage tests verify backend correctness and concurrency behavior.

Covered backends:

* SQLite in-memory
* SQLite disk
* Valkey compatibility mode

Example tests:

```text
test_sqlite_pool_concurrency
test_valkey_pool_concurrency
test_valkey_store_operations
```

Validation includes:

* Session persistence
* Session updates
* Concurrent access
* Statistics collection
* Backend compatibility

---

## 7. Rate Limiting

Rate limiter tests verify:

* Request counting
* Window expiration
* Stale entry eviction

Example tests:

```text
test_rate_limiter
test_rate_limiter_eviction
```

---

## 8. VM Core

The VM implementation is tested across both WASM and shared crates.

Covered operations:

```text
ADD
SUB
MUL
XOR
AND
OR
NOT
HASH
ROT
PUSH
```

Validation includes:

* Wrapping arithmetic
* Stack underflow detection
* Invalid opcode rejection
* Truncated instruction rejection
* Instruction safety

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

## 9. Synthetic Gene Mutation Engine

The mutation engine is a critical security component.

Coverage includes:

* Mutation order execution
* Deterministic parity
* Randomized mutation programs
* Preview lifecycle
* Commit lifecycle
* Discard lifecycle
* Environment validation
* Gene integrity

Example tests:

```text
test_mutation_chain
test_generate_order_is_deterministic_for_seeded_rng
test_server_client_parity_across_random_orders
test_preview_commitment_matches_shared_engine
test_commit_applies_preview
test_discard_preview_keeps_committed_state
test_table_driven_parity_across_many_generated_orders
test_fuzz_style_random_program_bytes_do_not_diverge
```

---

## 10. Property-Based Testing

ChronoSeal uses `proptest` to validate protocol invariants under arbitrary input.

Property tests:

```text
test_vm_execute_never_panics
test_gene_environment_roundtrip_never_panics
```

These tests continuously exercise malformed and randomized inputs to ensure graceful handling and panic resistance.

---

# Running the Test Suite

Run all tests:

```bash
cargo test --workspace
```

Run server tests:

```bash
cargo test -p chronoseal-server
```

Run fingerprint tests only:

```bash
cargo test -p chronoseal-server fingerprint
```

Run WASM tests:

```bash
cargo test -p chronoseal-wasm
```

Run shared tests:

```bash
cargo test -p shared
```

Show output:

```bash
cargo test -- --nocapture
```

---

# Critical Security Tests

The following tests are considered release-blocking:

```text
test_replay_attack_is_rejected
test_mutation_commitment_tamper_is_rejected
test_handler_tampered_commitment_is_silent_failure
test_deterministic_server_client_parity_across_many_heartbeats
test_server_client_parity_across_random_orders
test_vm_execute_never_panics
test_gene_environment_roundtrip_never_panics
rejects_invalid_aspect_ratios
rejects_invalid_device_pixel_ratios
rejects_invalid_hardware_concurrency
```

These tests directly protect protocol integrity, replay resistance, mutation validation, deterministic execution, and fingerprint hardening.

---

# Conclusion

ChronoSeal's testing strategy focuses on preserving deterministic behavior, protocol integrity, cryptographic correctness, browser ↔ server parity, and resistance to malformed or adversarial input.

The current suite of **100 passing tests** provides comprehensive coverage across:

* Configuration
* Runtime initialization
* Browser fingerprint validation
* Session lifecycle management
* Heartbeat verification
* Mutation engine execution
* VM safety
* Behavioral validation
* Storage backends
* Replay resistance
* Property-based protocol hardening

Maintaining and expanding this test suite remains a core project priority.

**Last Updated:** June 2026 (v1.0.2)

