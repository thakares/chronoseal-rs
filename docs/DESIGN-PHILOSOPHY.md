# ChronoSeal Design Philosophy

ChronoSeal is built for operators who value clarity, stability, and Unix-native infrastructure.

## Core Philosophy

ChronoSeal is a Unix-first, CLI-first cryptographic attestation daemon. It is intentionally designed to feel like infrastructure software such as `nginx`, `redis-server`, or `systemd` itself.

### Design priorities

* **Unix-native operation** — systemd integration, PID files, structured logs, and predictable lifecycle semantics.
* **CLI as source of truth** — all runtime operations available through the command line.
* **Minimal opacity** — no hidden telemetry, no opaque fingerprinting database.
* **Privacy-first** — ephemeral session state and no persistent user profiling.
* **Deterministic runtime behavior** — shared Rust/WASM implementation for the mutation engine and heartbeat protocol.
* **Incremental cost escalation** — make automation painful to scale without claiming impossible security.
* **Operational transparency** — expose health, metrics, status, and config as first-class artifacts.

## Execution Model

ChronoSeal emphasizes deterministic, stateless request validation with a lightweight server-side session store.

* The server persists only the small session state required for continuity.
* The client executes a deterministic WASM runtime for every heartbeat.
* The protocol is intentionally ambiguous on rejection to avoid leaking validation rules.

## Non-Goals

ChronoSeal does not aim to be:

* a tracking platform
* a browser fingerprinting database
* a long-term behavioral analytics engine
* a platform for user profiling
* a SaaS or cloud-first service

Instead, ChronoSeal aims to be an infrastructure layer that raises attacker cost while leaving legitimate users unobstructed.

## Operational Assumptions

ChronoSeal assumes:

* the host environment is Linux
* systemd is available for service management
* TLS is used in production
* browser clients can execute WASM
* operators can manage native binaries and configuration files

## Privacy and Trust

The project is designed so that the verification mechanism is:

* ephemeral
* difficult to reverse-engineer at scale
* not based on personal identifiers
* not dependent on long-term user history

These choices reflect the belief that the best anti-automation system is one that can be operated without becoming a surveillance platform.
