# ChronoSeal Security Assumptions & Guarantees

This document details the trust boundary models, security assumptions, and non-goals of the ChronoSeal system.

---

## 1. Core Threat Philosophy

ChronoSeal is a **cost-raising security layer**. It is designed to force automated scraping, botting, and replay tools to execute a fully compliant JavaScript/WASM execution runtime. It does not provide absolute hardware attestation or proof of human presence.

---

## 2. Non-Goals (What ChronoSeal is NOT)

1.  **Proof of Humanity:** ChronoSeal does not check if the user is a human. A headless browser running with standard input event automation will pass verification if it runs the WASM runtime correctly.
2.  **Anti-Debugging/Enclave Security:** ChronoSeal does not run inside a secure hardware enclave on the client. An attacker has complete control of the client wasm environment, memory, and key storage.
3.  **Perfect Browser Verification:** ChronoSeal cannot guarantee the client is a real Chrome/Firefox browser. It guarantees that the client maintains the state chain and executes the math VM program.

---

## 3. Threat Matrix & Attacker Cost Model

*   **Commodity HTTP Clients (Python `requests`, `curl`):** *Blocked.* Attackers cannot sign payloads, run the mathematical VM, or maintain the stateful BLAKE3 hash chain.
*   **Headless Automation (Puppeteer, Playwright):** *Partially Contained.* The automation script must execute the full browser environment, load the WASM module, feed valid parameters, and generate realistic mouse movement coordinates. This imposes significantly higher CPU and resource overhead on the attacker.
*   **Custom WASM Emulators:** *Raised Cost.* A determined reverse engineer can extract the WASM module and build a custom state runner in Node.js or Go. ChronoSeal counters this by using a stateful **Synthetic Gene Mutation Engine**, where the state vector mutations are governed dynamically by the server, requiring the emulator to replicate the entire mutation spec.

---

## 4. Key Invariants

1.  **Chain Continuity:** A session state cannot bifurcate. Every heartbeat must advance the state head using the latest salt.
2.  **VM Parity:** Stack state must exactly match the execution output of the server's issued opcode sequence.
3.  **Dynamic Challenges:** Client gene updates must match the server-issued mutation program.
