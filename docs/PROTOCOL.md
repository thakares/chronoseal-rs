# ChronoSeal Protocol Specification (PROTOCOL)

This document defines the formal wire protocol, state transitions, cryptographic primitives, and execution invariants of the ChronoSeal browser attestation system.

---

## 1. Sequence Flow & Handshake

ChronoSeal operates as a stateful, sequential challenge-response chain over HTTP/REST.

```
Client (JS/WASM)                                         Server (chronoseald)
      |                                                        |
      | 1. POST /init { public_key: String } -----------------> |
      |                                                        | (Generates VM Opcodes)
      |                                                        | (Computes initial hash chain head H_0)
      |                                                        | (Saves initial session record)
      | <--- 200 OK { InitResponse } --------------------------|
      |                                                        |
      | [Client executes VM program & prepares gene preview]   |
      |                                                        |
      | 2. POST /hb { HeartbeatRequest } ---------------------> |
      |                                                        | (Loads session & executes CAS check)
      |                                                        | (Verifies Ed25519 signature)
      |                                                        | (Validates VM stack-state parity)
      |                                                        | (Computes expected gene mutation)
      |                                                        | (Validates hash chain continuity H_N == expected)
      |                                                        | (Rotates salt & issues next mutation order)
      | <--- 200 OK { HeartbeatResponse } ---------------------| (Saves updated session record)
      |                                                        |
```

---

## 2. Cryptographic Transition Mechanics

### A. Handshake Phase (`/init`)
The client registers a 32-byte Ed25519 verifying key represented as a hex string. 
The server:
1.  Generates a 32-byte session ID ($ID$) and a 16-byte initial salt ($S_0$).
2.  Computes the initial hash chain head:
    $$H_0 = \text{Blake3}(ID \parallel PK_{\text{client}} \parallel S_0)$$
3.  Generates a random VM program of size $8..=16$ bytes.
4.  Creates the initial mutation order program $M_1$.
5.  Persists the session record in the database.

---

### B. Heartbeat progression (`/hb`)
For each heartbeat step $n \ge 1$:
The client submits:
*   `prev_hash`: $H_{n-1}$ (hex encoded).
*   `timestamp`: $T_n$ (milliseconds).
*   `entropy_data`: Mouse movement arrays.
*   `stack_state`: The VM final stack and instruction pointer `ip` after execution.
*   `gene_commitment`: Hex-encoded commitment of the mutated gene state.
*   `signature`: Ed25519 signature of the canonical alphabetical JSON payload.

The server:
1.  Loads the session record from storage, enforcing optimistic locking (CAS check) to confirm the database `last_hash` matches $H_{n-1}$.
2.  Validates the Ed25519 signature against the canonical alphabetical serialization.
3.  Re-executes the session's VM opcodes and asserts the client's `stack_state` matches the output.
4.  Applies the mutation order $M_n$ to the stored gene state and calculates the expected commitment:
    $$C_n = \text{Blake3}(\text{CandidateGene} \parallel ID \parallel n)$$
    Asserts the client's `gene_commitment` matches.
5.  Validates that $|T_{\text{server}} - T_n| \le \text{max\_drift}$.
6.  Advances the hash chain:
    $$H_n = \text{Blake3}(H_{n-1} \parallel T_n \parallel \text{Blake3}(E_n) \parallel \text{Blake3}(S_n) \parallel S_{n-1})$$
7.  Rotates the salt to $S_n$ and issues the next mutation order $M_{n+1}$.

---

## 3. VM Instruction Specification

The client VM executes instructions sequentially. The instruction set consists of:

*   `0x00`: Pushes the next 4 bytes in the instruction stream onto the stack as a `u32` value (little-endian).
*   `0x01`..=`0x07`: Binary operators. Requires at least 2 elements on the stack:
    *   `0x01`: Wrapping Add (`a.wrapping_add(b)`)
    *   `0x02`: Wrapping Sub (`a.wrapping_sub(b)`)
    *   `0x03`: Wrapping Mul (`a.wrapping_mul(b)`)
    *   `0x04`: XOR (`a ^ b`)
    *   `0x05`: AND (`a & b`)
    *   `0x06`: OR (`a | b`)
    *   `0x07`: Rotate Left (`a.rotate_left(b % 32)`)
*   `0x08`: Unary Bitwise Not (`!a`). Requires at least 1 element on the stack.
*   `0x09`: Hash Stack. Hashes all stack elements using BLAKE3 and reduces it to a single `u32` value, clearing the stack and pushing the hash.
*   *Any other opcode:* Terminates VM execution immediately.
