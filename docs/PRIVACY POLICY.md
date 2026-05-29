# ChronoSeal Privacy Policy

ChronoSeal is a privacy-first cryptographic attestation system. It is intentionally designed to avoid long-term profiling, tracking, and persistent identity storage.

## What ChronoSeal Collects

ChronoSeal only collects the minimum ephemeral data required to validate a live browser session:

* `session_id` — ephemeral session identifier
* `prev_hash` / `initial_hash` — cryptographic chain state
* `timestamp` — heartbeat timing information
* `entropy_data` — recent mouse event samples for behavioral plausibility
* `stack_state` — VM execution result for heartbeat uniqueness
* `fingerprint` signals — basic browser sanity values such as aspect ratio, DPR, and hardware concurrency
* `mutation_step` / `gene_commitment` — synthetic mutation parity values for protocol continuity

## What ChronoSeal Does Not Store

ChronoSeal does not store or persist:

* IP addresses as a core artifact
* browser history
* user identifiers
* personal data
* device fingerprint databases
* long-term behavioral profiles
* cross-session tracking records

If you need browser telemetry or user profiling, ChronoSeal is not the right tool.

## Session Ephemerality

By default, ChronoSeal uses `sqlite-in-memory` storage. Sessions are ephemeral and are expected to be recreated after process restarts.

Persistent state is only stored when the operator explicitly configures `sqlite-disk` or `valkey`.

## Client-Side Key Handling

The Ed25519 signing keypair is generated inside the WASM runtime and is never serialized or transmitted in full.

* Private key: stays inside WASM linear memory
* Public key: transmitted once during session initialization

This design minimizes the amount of sensitive material exposed outside the browser runtime.

## Intentional Silent Rejection

ChronoSeal intentionally returns a uniform `{"status":"ok"}` response for invalid heartbeats.

This is a privacy-preserving decision: it avoids emitting detailed rejection reasons that could be used to fingerprint or probe clients.

## Data Retention

Session state is retained only as long as it is needed for heartbeat continuity.

Expired sessions are purged automatically by cleanup tasks. Ephemeral backend modes do not write state to disk beyond the current process lifetime.

## Transparency

The source code is open and the verification model is documented. Operators can inspect exactly what ChronoSeal stores and validates.

## Summary

ChronoSeal is designed to provide anti-automation defense without becoming a tracking or surveillance platform.

It is a privacy-aware, ephemeral attestation layer with strong operational guardrails.
