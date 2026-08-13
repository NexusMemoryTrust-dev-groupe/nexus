# Build Reproducibility

What a given commit must build with, and how to verify that a local build
matches what CI ships. This file is the single source of truth for pinned
versions; CI and local scripts must stay in sync with it.

- App version: `1.1.0` (`src-tauri/tauri.conf.json`, `package.json`)

## Toolchain

| Component  | Pinned version | Source of truth            |
| ---------- | -------------- | -------------------------- |
| Rust       | `1.95.0`       | `src-tauri/Cargo.toml`     |
| Cargo      | `1.95.0`       | ships with Rust            |
| Node.js    | `24.x`         | `.github/workflows/ci.yml` |
| npm        | `11.x`         | ships with Node 24         |

- Rust is **not** pinned via `rust-toolchain.toml`; CI installs the `stable`
  channel via `dtolnay/rust-toolchain@stable`. Verify the actual compiler
  version in the CI log of the release commit (`rustc --version`).
- Node is pinned in CI to `node-version: 24` (frontend job) and `20`
  (installer-scripts job, which only runs `node --check`, so the split is
  intentional and not a reproducibility risk).
- Local verification:

  ```powershell
  rustc --version   # expect rustc 1.95.0
  cargo --version   # expect cargo 1.95.0
  node --version    # expect v24.x
  npm --version     # expect 11.x
  ```

## Lockfiles (committed)

Both lockfiles are committed and are the only supported install paths — never
`cargo update` or a bare `npm install` in a release build.

| Lockfile                | Covers            | CI install command |
| ----------------------- | ----------------- | ------------------ |
| `src-tauri/Cargo.lock`  | Rust dependencies | `cargo build` (implicit) |
| `package-lock.json`     | Frontend deps     | `npm ci`           |

Rules:

- `npm ci` is mandatory in CI; `npm install` is only for adding a new
  dependency, and it must be followed by a commit of `package-lock.json`.
- `cargo` resolves `Cargo.lock` automatically; do not delete it.
- A diff in either lockfile without a matching `Cargo.toml` / `package.json`
  change must be reviewed for dependency drift.

## ONNX embedding model (fastembed)

The semantic search engine uses `EmbeddingModel::AllMiniLML6V2` from the
`fastembed` crate (`fastembed = "5"`, ONNX runtime). The model is **not**
vendored in the repository; it is downloaded on first use by fastembed and
cached on disk.

### Model identity

| Property       | Value                                             |
| -------------- | ------------------------------------------------- |
| Model          | `Qdrant/all-MiniLM-L6-v2-onnx` (all-MiniLM-L6-v2) |
| Cache layout   | `models--Qdrant--all-MiniLM-L6-v2-onnx/snapshots/5f1b8cd78bc4fb444dd171e59b18f3a3af89a079/` |
| Default cache  | `.fastembed_cache` next to the database directory |
| Override       | `FASTEMBED_CACHE_DIR` (used by CI)                |

### Pinned file hashes (SHA-256)

These are the exact bytes the app was built and tested against. Any CI or
release verification that ships a model must match them:

| File                    | Size (bytes) | SHA-256                                                           |
| ----------------------- | ------------ | ----------------------------------------------------------------- |
| `model.onnx`            | 90 387 630   | `bbd7b466f6d58e646fdc2bd5fd67b2f5e93c0b687011bd4548c420f7bd46f0c5` |
| `tokenizer.json`        | 711 661      | `da0e79933b9ed51798a3ae27893d3c5fa4a201126cef75586296df9b4d2c62a0` |
| `tokenizer_config.json` | 1 433        | `bd2e06a5b20fd1b13ca988bedc8763d332d242381b4fbc98f8fead4524158f79` |
| `special_tokens_map.json` | 695        | `5d5b662e421ea9fac075174bb0688ee0d9431699900b90662acd44b2a350503a` |
| `config.json`           | 650          | `1b4d8e2a3988377ed8b519a31d8d31025a25f1c5f8606998e8014111438efcd7` |

### How to verify a local cache

```powershell
$snap = "$env:LOCALAPPDATA\nexus\.fastembed_cache\models--Qdrant--all-MiniLM-L6-v2-onnx\snapshots\5f1b8cd78bc4fb444dd171e59b18f3a3af89a079"
Get-ChildItem $snap -File | ForEach-Object {
  "$($_.Name)`t$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower())"
}
```

If hashes differ, delete the snapshot directory and let fastembed re-download
it. If the model cannot be downloaded (air-gapped build), the engine degrades
to deterministic hash-based vectors — functionally correct but **not**
semantically meaningful, and performance metrics will differ. Never ship that
state as a release.

## Verifying a build matches CI

```powershell
# Backend
cargo build --release --bins -j 2          # inside src-tauri/
cargo test  --all-targets -j 2
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check

# Frontend
npm ci
npx tsc --noEmit
npm run lint
npm test -- --run
npm run build
```

All commands must exit `0` on a clean checkout of the release commit.
