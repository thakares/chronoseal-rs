# ChronoSeal Debugging & Failure Mode Guide (WHY_IT_FAILS)

This document provides a technical diagnostic reference for developers, operators, and integration security teams. It explains why a client heartbeat or session initialization fails verification, and how to debug desynchronization issues.

---

## 1. Silent Rejections vs. HTTP Failures

To deny attackers a feedback oracle, the ChronoSeal heartbeat endpoint (`POST /hb`) always returns HTTP status `200 OK` with `{"status": "ok"}` on semantic verification failures.

*   **Successful Attestation:** The JSON response contains the rotated next state information: `next_salt`, `next_mutation_step`, and `next_mutation_order_b64`.
*   **Silently Rejected Attestation:** The JSON response *omits* these three fields. The client is expected to roll back the state preview and retry.

---

## 2. Common Verification Failure Modes

### A. Clock Drift (`TimestampDrift`)
*   **Error Cause:** The client machine's local system time differs from the server's time by more than the configured `max_timestamp_drift_ms` (default 30 seconds).
*   **Diagnostic Signal:** The `/hb` response omits next state parameters.
*   **Remediation:** Synchronize both client and server clocks using NTP (Network Time Protocol). On the client, use NTP-synced system clocks or query server timestamp headers during initialization to compute a local clock offset.

### B. Replay Attempts / Out-of-Sequence (`ChainBroken`)
*   **Error Cause:** The request `prev_hash` does not match the server-stored `last_hash` for the session.
*   **Root Causes:**
    1.  The client replayed a previously captured heartbeat payload.
    2.  The client lost the network response containing the rotated next state parameters and retried with stale state.
    3.  A concurrent request succeeded first, updating the session's hash state.
*   **Remediation:** If network issues cause packet loss, the client must discard the session and initiate a new `/init` handshake. Heartbeats cannot be replayed or resumed from a historical state.

### C. Signature Failures (`Signature`)
*   **Error Cause:** The Ed25519 signature over the canonical JSON payload is invalid.
*   **Root Causes:**
    1.  The client signed a payload that differed in ordering or format from the server's canonical serialization. (Ensure key sorting matches alphabetically: `entropyData`, `fingerprint`, `geneCommitment`, `mutationStep`, `prevHash`, `sessionId`, `stackState`, `timestamp`).
    2.  Different platform engines formatted floats or large numbers differently.
    3.  The public key registered during `/init` does not match the signing key.
*   **Remediation:** Ensure both frontend and backend use strict canonical serializations (BTreeMap alphabetically sorted keys).

### D. VM Stack State Mismatch (`VmStackMismatch`)
*   **Error Cause:** The client's submitted `stack_state` (VM stack and instruction pointer `ip`) does not match the server-side re-execution of the session's random math program.
*   **Root Causes:**
    1.  An automated client bypassed the VM bytecode interpreter.
    2.  The client VM interpreter diverged mathematically (e.g. word size wrapping or logical op mismatches).
*   **Remediation:** Check the VM interpreter implementation parity between the client wasm and `shared::vm`.

### E. Mutation Commitment Mismatch (`MutationCommitmentMismatch`)
*   **Error Cause:** The client's computed `gene_commitment` does not match the server-applied gene mutation.
*   **Root Causes:**
    1.  The client used a different number of `mutation_rounds` than the server config.
    2.  The mutation order execution logic diverged.
*   **Remediation:** Verify that the client wasm correctly parsed `mutation_rounds` from `/init` and passed it to the generator.

### F. Rate Limiting (`RateLimiter`)
*   **Error Cause:** The client submitted more requests than allowed by the server's rate-limiting config (e.g., `rate_limit_count` per `rate_limit_window_secs`).
*   **Diagnostic Signal:** The server returns `200 OK` with `{"status": "ok"}` but no next state data.
*   **Remediation:** Reduce heartbeat frequency or adjust rate limit parameters in the daemon configuration.
