# ChronoSeal WASM Build Guide

ChronoSeal uses a Rust-based WASM runtime to power browser-side attestation logic, signing, hash chaining, VM execution, and mutation commitment preview.

## Why WASM

The WASM runtime provides a deterministic, sandboxed environment for the following tasks:

* generate Ed25519 keypairs in-browser
* sign canonical heartbeat payloads
* execute randomized VM opcode programs
* compute Blake3 hash chain progression
* preview and commit synthetic gene mutations

This enables server/client parity and prevents the private key from leaving the browser runtime.

## Build Requirements

Install the Rust WASM target and `wasm-pack`:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

Verify:

```bash
wasm-pack --version
```

## Build the WASM Module

From the repository root:

```bash
wasm-pack build wasm --target web --release
rm -rf frontend/pkg
mv wasm/pkg frontend/pkg
```

`--target web` produces an ES module compatible with the existing frontend JavaScript.

`--release` enables optimizations for runtime performance and size.

## Output

After a successful build, `frontend/pkg/` contains:

* `antibot_wasm.js`
* `antibot_wasm_bg.wasm`
* `antibot_wasm_bg.js`
* `antibot_wasm.d.ts`
* `antibot_wasm_bg.d.ts`
* `package.json`

The frontend expects the WASM package under `frontend/pkg/`.

## Runtime Exports

The WASM module exports the following functions:

* `generate_keypair()` — generate a new Ed25519 keypair and return public key hex
* `get_public_key()` — return the current public key hex
* `sign_message(msg)` — sign a UTF-8 payload and return the hex signature
* `compute_next_hash(prev, ts, entropy, stack, salt)` — compute the next Blake3 chain hash
* `run_program(b64)` — execute a base64 VM program and return stack state
* `init_gene_state(gene_size)` — initialise the synthetic gene buffer
* `preview_gene_commitment(order_b64)` — preview the next gene commitment from a mutation order
* `commit_gene_preview()` — commit the previewed mutation after successful heartbeat
* `discard_gene_preview()` — discard the previewed mutation after rejection or error
* `current_gene_commitment()` — return the current committed gene commitment

## Browser Integration

The frontend imports the generated module like this:

```js
import init, {
  generate_keypair,
  sign_message,
  compute_next_hash,
  run_program,
  init_gene_state,
  preview_gene_commitment,
  commit_gene_preview,
  discard_gene_preview,
  current_gene_commitment
} from './pkg/antibot_wasm.js';
```

`await init()` must be called before invoking any other exported function.

## Deployment Note

The `.wasm` binary must be served with the correct MIME type:

```
Content-Type: application/wasm
```

The built-in Axum static file handler already sets the appropriate MIME type for `.wasm` files.

## Build Script

Use the convenience script:

```bash
bash scripts/build.sh
```

This builds the WASM package, moves it into `frontend/pkg/`, and builds the server binary.

## Recommended Development Flow

* For WASM-only changes:

```bash
wasm-pack build wasm --target web
rm -rf frontend/pkg
mv wasm/pkg frontend/pkg
```

* For server-only changes:

```bash
cargo build -p server
```

## Notes

Generated files in `wasm/pkg/` and `frontend/pkg/` are not tracked in source control.
They are build artifacts and should be regenerated as part of the release workflow.
