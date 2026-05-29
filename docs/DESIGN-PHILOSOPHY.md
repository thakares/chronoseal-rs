# ChronoSeal Design Philosophy

ChronoSeal is designed for operators who want a local, inspectable, Unix-native browser attestation layer rather than a hosted anti-bot black box.

## Core Position

ChronoSeal is infrastructure software. It should feel closer to `nginx`, `redis-server`, or a small system daemon than to a third-party analytics platform.

Design priorities:

- CLI-first operation
- explicit configuration
- deterministic protocol behavior
- small runtime surface
- privacy-preserving state
- observable health and metrics
- no hidden telemetry
- no persistent user profiling

## What ChronoSeal Optimizes For

### Operator Control

Operators should be able to build, run, inspect, configure, monitor, and stop the service with ordinary Unix tools.

This is why ChronoSeal provides:

- `chronoseal run`
- `chronoseal status`
- `chronoseal health`
- `chronoseal config check`
- `chronoseal metrics`
- `chronoseal stats`
- shell completions
- systemd integration

### Determinism

The protocol depends on deterministic agreement between server Rust and browser WASM.

Shared logic belongs in `shared/` when divergence would create security or correctness risk. This includes:

- protocol structs
- hash-chain semantics
- synthetic gene model
- mutation opcode behavior
- mutation order encoding

### Cost Escalation

ChronoSeal does not claim impossible security. It raises the cost of automation by making clients maintain:

- a browser-local signing key
- a signed canonical heartbeat payload
- a Blake3 hash chain
- VM execution output
- server-issued mutation progression
- plausible timing and interaction signals

The objective is to make cheap automation brittle and expensive automation more complex.

### Silent Rejection

Heartbeat rejection is intentionally ambiguous. Invalid heartbeats receive the same `status` value as accepted heartbeats, but accepted responses include next-state fields.

This avoids turning the API into a validation oracle. Integrators must check for `next_salt`, `next_mutation_step`, and `next_mutation_order_b64`.

### Privacy

ChronoSeal should not become a surveillance system.

It avoids:

- long-term user identifiers
- browser history
- cross-site identity graphs
- fingerprint databases
- behavioral profiling as a product feature

It stores only the session state required for continuity.

## Non-Goals

ChronoSeal is not:

- a CAPTCHA
- a fraud scoring engine
- an authentication provider
- a hosted SaaS product
- a persistent fingerprinting system
- a replacement for authorization checks
- a complete defense against real browser farms

## Operational Assumptions

ChronoSeal assumes:

- Linux or a Unix-like host
- systemd for production service management
- TLS in production
- browser clients can execute WASM
- operators can manage config files and service users
- application owners decide how attestation status gates protected resources

## Engineering Biases

When the project faces tradeoffs, prefer:

- explicit configuration over implicit magic
- server-side recomputation over browser trust
- bounded deterministic execution over unbounded heuristics
- clear CLI output over hidden dashboards
- local deployment over mandatory cloud dependencies
- privacy by data minimization over privacy by policy alone

## Success Criteria

ChronoSeal is succeeding when:

- legitimate browser sessions advance without user friction
- simple scrapers cannot pass the protocol
- automation requires a full stateful implementation
- operators can debug deployments with normal Unix tools
- stored data remains minimal and short-lived
- documentation reflects the implementation precisely
