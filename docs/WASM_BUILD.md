# ChronoSeal — WASM Build Guide

## Overview

The client-side cryptographic core of ChronoSeal is written in Rust and
compiled to WebAssembly (WASM). The JavaScript frontend (`heartbeat.js`)
imports functions from this WASM module to generate keypairs, sign heartbeat
payloads, compute hash chain links, and execute the stack machine program.

The import line in `heartbeat.js`:

```js
import init, { generate_keypair, sign_message, compute_next_hash, run_program }
  from './pkg/antibot_wasm.js';
```

`./pkg/antibot_wasm.js` is a **generated file**. It does not exist in the
repository and must be produced by building the `wasm/` crate before running
the server.

---

## How the WASM Module is Built

The tool that compiles Rust to WASM and generates the JavaScript glue is
[`wasm-pack`](https://rustwasm.github.io/wasm-pack/).

When you run:

```bash
wasm-pack build wasm --target web --release
```

wasm-pack does the following in sequence:

1. Compiles `wasm/src/lib.rs` (and its submodules) to a `.wasm` binary using
   the `wasm32-unknown-unknown` target.
2. Runs `wasm-bindgen` to inspect every `#[wasm_bindgen]`-annotated function
   and struct and generate a JavaScript wrapper for each one.
3. Optionally runs `wasm-opt` (from Binaryen) to size-optimise the binary.
4. Writes all output to `wasm/pkg/`.

---

## Output: `wasm/pkg/`

After a successful build, `wasm/pkg/` contains:

```
wasm/pkg/
├── antibot_wasm.js          ← ES module; the file heartbeat.js imports
├── antibot_wasm_bg.wasm     ← compiled WASM binary (~300–800 KB release)
├── antibot_wasm_bg.js       ← internal memory bridge (do not import directly)
├── antibot_wasm.d.ts        ← TypeScript type declarations
├── antibot_wasm_bg.d.ts     ← TypeScript declarations for the bg module
└── package.json
```

### `antibot_wasm.js`

This is the public entry point. It contains:

- An `init()` function that fetches and instantiates the `.wasm` binary.
- One JavaScript wrapper function for each `#[wasm_bindgen]` export in
  `wasm/src/`:

| Rust export | JS wrapper | Description |
|---|---|---|
| `generate_keypair()` | `generate_keypair()` | Generate Ed25519 keypair; return hex public key |
| `get_public_key()` | `get_public_key()` | Return hex public key, or `""` if not initialised |
| `sign_message(msg)` | `sign_message(msg)` | Sign string; return hex signature, or `""` if not initialised |
| `compute_next_hash(prev, ts, entropy, stack, salt)` | `compute_next_hash(...)` | Compute next Blake3 chain hash |
| `run_program(b64)` | `run_program(b64)` | Execute base64 VM program; return `{ stack, ip }` |

### `antibot_wasm_bg.wasm`

The compiled binary. The `.bg` suffix means "background" — this is the raw
WASM that `antibot_wasm.js` loads internally. You should not reference this
file directly in your HTML.

---

## Step-by-Step Build

### 1. Install the Rust WASM target

```bash
rustup target add wasm32-unknown-unknown
```

This is a one-time step. Without it, the Rust compiler cannot produce WASM
output.

### 2. Install wasm-pack

```bash
cargo install wasm-pack
```

Or via the installer script:

```bash
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

Verify:

```bash
wasm-pack --version
# wasm-pack 0.13.x
```

### 3. Build the WASM module

From the project root:

```bash
wasm-pack build wasm --target web --release
```

`--target web` produces an ES module (`import`/`export` syntax) suitable for
use directly in a browser without a bundler. Other targets (`bundler`,
`nodejs`, `no-modules`) produce different output formats and are not
compatible with the ChronoSeal frontend as written.

`--release` enables Rust's release optimisations (inlining, dead code
elimination, size reduction). Omit it during development for faster builds
and better panic messages.

### 4. Move the output to the frontend

```bash
rm -rf frontend/pkg
mv wasm/pkg frontend/pkg
```

The frontend expects the WASM module at `frontend/pkg/antibot_wasm.js`
because `heartbeat.js` imports from `./pkg/antibot_wasm.js` relative to
the `frontend/` directory, which is where the server's static file handler
is rooted.

---

## Using the Build Script

The convenience script at `scripts/build.sh` performs all steps in order:

```bash
bash scripts/build.sh
```

This builds the WASM module, moves it to `frontend/pkg/`, and then builds
the server binary. Run this for a clean full build before deployment.

For development iteration where you are only changing Rust WASM code:

```bash
wasm-pack build wasm --target web   # (omit --release for speed)
rm -rf frontend/pkg && mv wasm/pkg frontend/pkg
```

For development where you are only changing server code:

```bash
cargo build -p server
```

---

## How `heartbeat.js` Loads the Module

`heartbeat.js` uses a standard ES module dynamic import pattern:

```js
import init, { generate_keypair, sign_message, compute_next_hash, run_program }
  from './pkg/antibot_wasm.js';

export async function initHeartbeat() {
    // 1. Fetch and instantiate the .wasm binary
    await init();

    // 2. Generate keypair — private key stored in WASM memory only
    const pubKeyHex = generate_keypair();

    // 3. Send public key to server, receive session_id and chain seed
    // ...
}
```

`init()` is the default export from `antibot_wasm.js`. It fetches
`antibot_wasm_bg.wasm` (from the same `pkg/` directory) via `fetch()`,
compiles it in the browser's WASM engine, and links it to the JS glue
layer. After `await init()` returns, all the named exports
(`generate_keypair`, `sign_message`, etc.) are ready to call.

The `init()` call must complete before any other WASM function is called.
Calling `sign_message()` or `compute_next_hash()` before `await init()`
returns will produce an empty string (the module is not yet instantiated).

---

## Serving the WASM Binary

Browsers require WASM files to be served with the correct MIME type:

```
Content-Type: application/wasm
```

Most web servers set this automatically for `.wasm` files. If you see the
error:

```
WebAssembly.instantiate(): Response has unsupported MIME type
```

Add the MIME type to your server configuration:

**nginx:**
```nginx
types {
    application/wasm  wasm;
}
```

**Apache `.htaccess`:**
```apache
AddType application/wasm .wasm
```

The Axum `ServeDir` handler used by ChronoSeal's built-in static server
sets the correct MIME type automatically via `tower-http`.

---

## What Is Not in the Repository

| Path | Why excluded |
|---|---|
| `wasm/pkg/` | Generated build output — changes on every build |
| `frontend/pkg/` | Same generated output, moved to serve location |
| `target/` | Standard Rust build artefacts |

Both `wasm/pkg/` and `frontend/pkg/` are listed in `.gitignore`. Committing
them would bloat the repository (the `.wasm` binary alone is 300–800 KB),
create noisy diffs on every rebuild, and give a false impression that the
WASM module is pre-built and ready to use without a build step.

---

## Troubleshooting

### `wasm32-unknown-unknown` target not found

```
error[E0463]: can't find crate for `std`
```

Fix:

```bash
rustup target add wasm32-unknown-unknown
```

### `wasm-pack` not found

```bash
cargo install wasm-pack
```

### `wasm-opt` not found (warning, not an error)

wasm-pack prints a warning if `wasm-opt` is not installed. The build still
succeeds; the binary is just not size-optimised.

```bash
# On Debian/Ubuntu/Arch
sudo apt install binaryen      # Debian/Ubuntu
sudo pacman -S binaryen        # Arch
```

### `antibot_wasm_bg.wasm` fetch fails (404)

The `.wasm` file is not being served from `frontend/pkg/`. Verify:

```bash
ls /mnt/Programs/ChronoSeal/frontend/pkg/
# Should list: antibot_wasm.js  antibot_wasm_bg.wasm  ...
```

If the directory is empty or missing, re-run the build steps above.

### MIME type error in browser

See the "Serving the WASM Binary" section above.

### `sign_message` or `generate_keypair` returns empty string

The WASM keypair has not been initialised. Ensure `await init()` and
`generate_keypair()` are called (and awaited) before any other WASM
function. Check the browser console for any errors during `init()`.
