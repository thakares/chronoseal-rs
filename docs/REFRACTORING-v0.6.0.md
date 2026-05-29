# ChronoSeal v0.6.0 — Synthetic Gene Mutation System

## Overview & Motivation
ChronoSeal v0.6.0 introduces a synthetic mutation chain model to strengthen attestation liveness and anti-replay guarantees while preserving privacy-first behavior. The core model combines:
- a primary byte-oriented gene buffer (`Vec<u8>`), and
- a bounded secondary environment map (`Vec<(u16 symbol, u32 quantity)>`).

Each heartbeat now carries deterministic mutation progression evidence (`mutation_step`, `gene_commitment`) that is validated server-side against the exact server-issued mutation order. This design increases attacker workload by coupling cryptographic chain continuity with stateful deterministic mutation parity.

## Architectural Goals
1. Keep runtime behavior deterministic across server and WASM execution.
2. Preserve ephemerality and low operational complexity.
3. Minimize additional latency on the heartbeat path.
4. Improve protocol resistance against replay and mutation tampering.
5. Maintain a maintainable codebase with explicit invariants and focused modules.

## Design Decisions
1. **Shared mutation engine**  
   Mutation opcode semantics live in `shared/src/vm_extensions.rs` to guarantee server/client parity from one implementation.

2. **Deterministic gene commitment**  
   A domain-separated BLAKE3 commitment (`chronoseal/gene/v1`) binds both gene bytes and sorted environment records.

3. **Bounded mutation complexity**  
   Mutation program length is capped (`MAX_MUTATION_PROGRAM_BYTES`) and environment cardinality is capped (`MAX_ENV_RECORDS`).

4. **Strict validation on ingest**  
   Environment payloads are validated for sortedness, uniqueness, non-zero quantity, and length constraints.

5. **Protocol-level mutation handshake**  
   `InitResponse` and `Heartbeat` payloads now include mutation step/order and commitment fields.

6. **DB backend control via `db_type`**  
   Server CLI/config now supports:
   - `sqlite-in-memory` (default)
   - `sqlite-in-disk` (active; uses `db_path`)
   - `valkey` (active compatibility mode; currently falls back to in-memory)

## Implementation Plan
1. Add gene model + deterministic commitment in `shared/gene.rs`.
2. Implement v0.6.0 mutation opcode set in shared VM extensions.
3. Persist mutation state per session (`gene`, `environment`, `pending_mutation`, `pending_mutation_step`).
4. Extend protocol schema for mutation fields in init/heartbeat exchange.
5. Validate mutation step + commitment parity before accepting heartbeat updates.
6. Add WASM preview/commit mutation lifecycle mirroring server behavior.
7. Add `db_type` CLI/config flow and runtime backend initialization strategy.
8. Add migration-safe schema extension (column existence checks + index creation).

## Testing Strategy (detailed section)
ChronoSeal v0.6.0 test coverage is organized across unit, integration, and randomized/fuzz-style validation.

1. **Unit, integration, and property tests**
   - Unit tests for gene invariants and encoding/decoding.
   - Unit tests for every mutation opcode with stack-effect assertions.
   - Integration tests for full session lifecycle and heartbeat acceptance/rejection paths.
   - Table-driven randomized tests and fuzz-style random bytecode tests to validate deterministic failure/success symmetry.

2. **Server-client parity testing**
   - Shared opcode engine parity tests across seeded mutation sequences.
   - Multi-step mutation chain test (`test_mutation_chain`) asserting identical server/client final state.
   - 10+ heartbeat deterministic simulation tests in session integration suite.

3. **Evasion / attack simulation testing**
   - Replay attack simulation.
   - Mutation step mismatch rejection.
   - Mutation commitment tampering rejection.
   - Malformed server mutation payload rejection.
   - Stack underflow / unknown opcode / truncated program rejection.

4. **Performance regression testing**
   - Bounded execution checks through capped program size and bounded record counts.
   - Timing smoke regression test for mutation execution loops.
   - End-to-end heartbeat test coverage to detect behavior regressions on hot paths.

## Security Analysis
1. **Replay resistance**  
   Heartbeats are now tied to both chain hash and mutation step progression.

2. **Mutation tampering resistance**  
   Server recomputes candidate gene state from authoritative pending mutation program and rejects commitment mismatch.

3. **Protocol ambiguity reduction**  
   Canonical signing payload includes mutation fields, reducing exploitable unsigned state.

4. **Input hardening**  
   Program size limits, stack underflow checks, and strict environment decoding reduce parser abuse and malformed payload amplification.

5. **Deterministic failure semantics**  
   Invalid mutation instructions fail predictably and symmetrically across server and WASM paths.

## Performance Considerations
1. Mutation instructions are lightweight and mostly O(1); only `INSERT`/`DELETE` are O(n) but bounded by max gene size.
2. Environment operations use sorted-vector binary search with tight upper bound (`MAX_ENV_RECORDS`).
3. Commitment hashing is linear in gene size and record count, both bounded.
4. Shared engine avoids duplicate logic and divergence-induced debugging overhead.

## Migration & Backward Compatibility
1. Schema migration is additive; new columns are created when missing.
2. Existing deployments without mutation fields require updated client+server pair for heartbeat compatibility.
3. `db_type` defaults to in-memory to preserve ephemeral behavior.
4. `sqlite-in-disk` is now directly usable via `db_path`.
5. `valkey` currently runs in compatibility mode (in-memory fallback) to avoid startup failure while preserving CLI contract.

## Risks & Mitigations
1. **Risk: State divergence between server and client**  
   Mitigation: shared opcode engine + deterministic seeded parity tests + multi-heartbeat integration tests.

2. **Risk: Mutation opcode abuse via malformed programs**  
   Mitigation: strict parsing, length caps, explicit underflow/unknown-opcode errors.

3. **Risk: Performance regressions**  
   Mitigation: bounded structures, smoke timing tests, and focused hot-path validation.

4. **Risk: Backend confusion during `db_type` rollout**  
   Mitigation: explicit CLI command (`chronoseal db-type`), config output visibility, and clear runtime compatibility behavior.
