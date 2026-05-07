# ChronoSeal Architecture

## Core Principles

- Continuous browser attestation
- Cryptographic heartbeat chains
- WASM-isolated secrets
- Behavioral entropy verification
- Silent mitigation

## Components

### WASM Runtime

Responsible for:
- heartbeat generation
- signature generation
- entropy collection
- VM execution

### Server

Responsible for:
- session verification
- trust scoring
- chain validation
- mitigation

## Threat Model

Designed to increase:
- scraping cost
- operational complexity
- synchronization burden

ChronoSeal does not attempt impossible perfect prevention.
