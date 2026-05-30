# ChronoSeal Protocol Stability Policy (PROTOCOL_STABILITY)

This document defines the stable interfaces and boundaries of the ChronoSeal project to guide third-party integration development and future internal architectural evolutions.

---

## 1. Stable Public Contract

The public surface of ChronoSeal is frozen at version 1.0 and consists of:

1.  **Wire Protocol API:**
    *   `POST /init`: Handshake schema (parameters, response fields).
    *   `POST /hb`: Heartbeat schema (payload parameters, response fields).
2.  **State Transition Semantics:**
    *   The BLAKE3 hash chain progression rules.
    *   The virtual machine opcodes and stack execution rules.
    *   The Synthetic Gene Mutation logic and context-bound commitments.
3.  **Daemon CLI & Config Schema:**
    *   Commands (`run`, `status`, `health`, etc.).
    *   TOML configuration keys.

---

## 2. Private Internal Boundaries

All implementation details are subject to change without notice. Wrappers, clients, and applications must not depend on:

*   **Internal Rust APIs:** ChronoSeal is a Unix daemon. It does not export a public Rust library SDK. Internal Rust modules (`server::storage`, `server::session`, etc.) are private.
*   **Database Schema:** The SQLite table structure, indexes, or column names are private to the daemon.
*   **Valkey Key Structures:** The layout of session keys, sorted set indexes, and pipelines are implementation details.

---

## 3. Protocol Evolution Policy

*   **Minor Updates:** Can introduce new optional configuration fields or metrics.
*   **Major Updates:** May change the VM instruction set or hash chain primitives, requiring new WASM builds.
