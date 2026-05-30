# ChronoSeal Third-Party Wrapper & Integration Guide (WRAPPER_GUIDE)

This document provides stable guidance for developers building third-party integration wrappers, clients, or SDKs around the `chronoseald` daemon.

---

## 1. Public Contract & Stability Guarantees

As a protocol-first Unix daemon, `chronoseald` guarantees stability on the public network interface.

### Guaranteed Stable
*   **Endpoints:** `POST /init` and `POST /hb`.
*   **JSON Fields:** The structure and naming of request and response keys.
*   **VM Instruction Set:** The behavior and encoding of the 10 core VM opcodes (`0x00`..=`0x09`).
*   **Signature Serialization:** Alphabetical key-sorting rules using `BTreeMap` serialization.
*   **Hash Progression:** Blake3 chain folding rules.

### Private (Unstable / Subject to Change)
*   **Database Engines & Schemas:** SQLite table structure, Valkey key formatting, and indexes.
*   **Daemon CLI Flags:** Internal metrics query formats.
*   **Memory Structures:** Thread boundaries, session caches, and synchronization locks.

---

## 2. API Versioning & Deprecation Policy

*   **Version Format:** API endpoints do not contain version prefixes (e.g., `/v1/hb`). Instead, protocol versioning is coupled to the daemon release version.
*   **Breaking Protocol Changes:** Any change to the core hash function (Blake3) or the VM instruction set will trigger a major release (e.g., `v2.0.0`).
*   **Deprecation Cycle:** Deprecated features will be supported for at least one minor release cycle, documented in `docs/PROTOCOL_STABILITY.md`.

---

## 3. Reference Implementation Steps for Wrappers

To build a client-side wrapper or application adapter for `chronoseald`:

1.  **Handshake:** Send `POST /init` with the hex-encoded Ed25519 public key. Save the returned `session_id`, `salt`, `opcodes_b64`, and `mutation_order_b64`.
2.  **VM Execution:** Run the math VM program (decoded from `opcodes_b64`) using the client wasm runtime to get the target `stack_state`.
3.  **Gene Mutation:** Decode `mutation_order_b64`, apply the mutation steps to the local gene buffer, and compute the new commitment hash.
4.  **Signing:** Build the canonical alphabetical JSON message, sign it, and send `POST /hb`.
5.  **Chain Advancement:** On success, extract `next_salt` and `next_mutation_order_b64` to prepare the next heartbeat request.
