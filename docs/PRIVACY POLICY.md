# ChronoSeal Privacy Policy

ChronoSeal is a privacy-oriented browser attestation system. It is designed to validate short-lived session continuity without creating persistent user profiles.

This document describes what ChronoSeal itself collects and stores. Applications that integrate ChronoSeal may collect additional data under their own policies.

## Data ChronoSeal Processes

ChronoSeal processes the minimum protocol data needed to validate a live browser session.

| Data | Purpose |
|---|---|
| `session_id` | Opaque session lookup key |
| public key | Verify signed heartbeats for the session |
| `salt` | Hash-chain progression |
| `initial_hash` / `prev_hash` / `last_hash` | Replay-resistant continuity |
| `timestamp` | Drift and liveness validation |
| mouse event samples | Behavioral plausibility checks |
| VM stack state | Input to hash-chain progression |
| basic fingerprint fields | Sanity validation |
| gene bytes and environment records | Mutation continuity |
| pending mutation program and step | Next heartbeat verification |
| expiration and last-seen timestamps | Session lifecycle and cleanup |

Basic fingerprint fields currently include:

- aspect ratio
- device pixel ratio
- hardware concurrency

## Data ChronoSeal Does Not Intentionally Collect

ChronoSeal does not intentionally collect or build:

- browser history
- page content history
- account identity
- email addresses
- names
- payment data
- location history
- cross-site tracking identifiers
- persistent fingerprint databases
- long-term behavioral profiles

ChronoSeal is not intended for analytics, advertising, or identity graph construction.

## Session Lifetime

Sessions are short-lived and expire according to `expiration_minutes`, which defaults to 30 minutes.

Expired sessions are removed by cleanup behavior. In-memory storage is lost when the process exits.

## Storage Modes and Persistence

| Mode | Persistence |
|---|---|
| `sqlite-in-memory` | process lifetime only |
| `sqlite-in-disk` | persisted to the configured SQLite file |
| `valkey` | persisted according to the Valkey deployment configuration |

Persistent state is operator-selected. The default backend is `sqlite-in-memory`.

## Client-Side Key Handling

The browser WASM runtime generates an Ed25519 keypair for the session.

- The public key is sent to `/init`.
- The private key is not sent to the server.
- Heartbeat payloads are signed in the browser runtime.

This is a continuity mechanism, not a long-term identity mechanism.

## Silent Rejection

ChronoSeal returns the same basic heartbeat status for accepted and rejected heartbeat requests:

```json
{
  "status": "ok"
}
```

Accepted responses additionally include next-state fields. Rejected responses omit them.

This reduces attacker feedback and avoids returning detailed failure classifications to clients.

## Logs

Operators control logging through `CHRONOSEAL_LOG`, `RUST_LOG`, and optional log-file configuration.

Production deployments should avoid debug logging because internal session identifiers or validation context may appear in logs.

## Operator Responsibilities

Operators should:

- serve traffic over HTTPS
- protect SQLite, Valkey, and log storage
- restrict access to metrics and stats endpoints
- choose persistence mode deliberately
- disclose any application-level data collection separately

## Summary

ChronoSeal validates live session continuity using short-lived cryptographic and deterministic state. It is designed to raise automation cost without becoming a persistent tracking or profiling system.
