# ChronoSeal WASM Build Guide

ChronoSeal uses a Rust-generated WASM package for browser-side attestation. The package is built from `wasm/` and copied into `frontend/pkg`.

## Responsibilities

The WASM runtime:

- generates a browser-local Ed25519 keypair
- signs canonical heartbeat payloads
- computes Blake3 hash-chain progression
- executes server-issued VM opcode programs
- initializes synthetic gene state
- previews gene mutation commitments
- commits or discards preview state after heartbeat response

The WASM runtime is not treated as a secure enclave. The server independently recomputes deterministic state.

## Requirements

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

Verify:

```bash
wasm-pack --version
```

## Build

From the repository root:

```bash
wasm-pack build wasm --target web --release
rm -rf frontend/pkg
mv wasm/pkg frontend/pkg
```

`--target web` emits native ES modules compatible with the static frontend.

Development build:

```bash
wasm-pack build wasm --target web
rm -rf frontend/pkg
mv wasm/pkg frontend/pkg
```

Full project build:

```bash
bash scripts/build.sh
```

## Output Files

The package name comes from the crate name `chronoseal-wasm`, so generated files use the `chronoseal_wasm` prefix.

Expected `frontend/pkg/` contents include:

- `chronoseal_wasm.js`
- `chronoseal_wasm_bg.wasm`
- `chronoseal_wasm.d.ts`
- `package.json`

Generated files in `wasm/pkg/` and `frontend/pkg/` are build artifacts and should be regenerated during release.

## Browser Import

```js
import init, {
  generate_keypair,
  get_public_key,
  sign_message,
  compute_next_hash,
  run_program,
  init_gene_state,
  preview_gene_commitment,
  commit_gene_preview,
  discard_gene_preview,
  current_gene_commitment
} from './pkg/chronoseal_wasm.js';
```

Call `await init()` before using any exported function.

## Exported Functions

| Function | Signature | Failure value |
|---|---|---|
| `generate_keypair()` | `() -> string` | `""` only on unexpected failure |
| `get_public_key()` | `() -> string` | `""` if no keypair exists |
| `sign_message(msg)` | `(string) -> string` | `""` if no keypair exists or signing fails |
| `compute_next_hash(prev, ts, entropy, stack, salt)` | `(string, u64, string, string, string) -> string` | panic/error path should be avoided by valid inputs |
| `run_program(b64)` | `(string) -> JsValue` | returns empty/default stack state on invalid execution path |
| `init_gene_state(gene_size)` | `(u32) -> bool` | `false` |
| `preview_gene_commitment(order_b64, session_id, mutation_step, rounds)` | `(string, string, u64, u8) -> string` | `""` |
| `commit_gene_preview()` | `() -> bool` | `false` |
| `discard_gene_preview()` | `() -> void` | none |
| `current_gene_commitment(session_id, mutation_step)` | `(string, u64) -> string` | `""` if no committed state exists |

`rounds = 0` in `preview_gene_commitment` selects the shared default mutation round count.

## Mutation State Lifecycle

The browser must keep two gene states:

- committed state: the last accepted state
- preview state: candidate state for the heartbeat currently being sent

Expected sequence:

1. Call `init_gene_state(gene_size)` after `/init`.
2. Call `preview_gene_commitment(order_b64, session_id, mutation_step, rounds)` before signing `/hb`.
3. Include the returned commitment and mutation step in the signed heartbeat.
4. If the response contains next-state fields, call `commit_gene_preview()`.
5. If the heartbeat is rejected or errors, call `discard_gene_preview()`.

Never commit preview state before the server accepts the heartbeat.

## Hash-Chain Ordering

After an accepted heartbeat, compute the next local hash with the salt that was active when the heartbeat was sent. Then replace the local salt with `next_salt`.

Correct order:

```js
const sentSalt = currentSalt;
currentSalt = resp.next_salt;
prevHash = compute_next_hash(prevHash, timestamp, entropyJson, stackStateJson, sentSalt);
```

This mirrors the server, which computes and stores the new hash before rotating to the next salt.

## Serving WASM

The `.wasm` file must be served with:

```text
Content-Type: application/wasm
```

ChronoSeal's built-in static file service handles this for normal deployments.

## Validation

Recommended checks after WASM changes:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
wasm-pack build wasm --target web
```

Then refresh `frontend/pkg`:

```bash
rm -rf frontend/pkg
mv wasm/pkg frontend/pkg
```
