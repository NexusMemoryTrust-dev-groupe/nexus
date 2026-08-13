# Installer Matrix & e2e Checklist

Plan 7.9 — every supported Windows install path, who enforces it, and how to
verify a release before publishing. Coverage is a release gate: a release that
fails any cell marked **✅ enforced** must not publish.

App version: `1.1.0`. Bundler output: NSIS (`*_x64-setup.exe` /
`*_arm64-setup.exe`) + MSI (`*.msi`) per architecture. Signature: every NSIS
installer ships next to a `.sig` (updater) and every download verifies
`SHA256SUMS.txt` (checksum).

## Supported matrix

| OS | Arch | Installer | Install mode | WebView2 | Upgrade | Uninstall | Non-admin |
|---|---|---|---|---|---|---|---|
| Windows 10 (1809+) | x64 | NSIS ✅ / MSI ✅ | per-user + per-machine (`installMode: both`) | auto (silent bootstrapper) | ✅ in place, data kept | ✅ NSIS + Add/Remove | ✅ per-user scope |
| Windows 10 (1809+) | ARM64 | NSIS ✅ / MSI ✅ | per-user + per-machine | auto (ARM64 WebView2) | ✅ | ✅ | ✅ |
| Windows 11 | x64 | NSIS ✅ / MSI ✅ | per-user + per-machine | auto | ✅ | ✅ | ✅ |
| Windows 11 | ARM64 | NSIS ✅ / MSI ✅ | per-user + per-machine | auto (Surface/Snapdragon) | ✅ | ✅ | ✅ |
| Windows 10 < 1809 | x64 | — ❌ | — | WebView2 requires 1809+ | — | — | — |
| 32-bit (x86) | i686 | — ❌ | — | ONNX Runtime ships no i686 | — | — | — |

Enforced by: `.github/workflows/release.yml` matrix (`x64` +
`aarch64-pc-windows-msvc`), `--bundles nsis,msi`, and the `collect` step that
fails when an installer or `.sig` is missing for an architecture.

## Scenario checklist

Each scenario is a manual/scripted verification on a real machine for the
OS/arch combos above. The columns that CI can already prove are marked
**CI-gated**; the rest are release-day manual checks with a documented script.

### S1 — Clean install

- [ ] NSIS: run `*_setup.exe` → wizard shows language selector (Russian/English)
- [ ] Install to a custom location, including another drive (e.g. `D:\Nexus`)
- [ ] Per-user scope: no UAC prompt
- [ ] App launches, DB created at `%LOCALAPPDATA%\Nexus\nexus.db`
- [ ] Setup wizard runs (Node.js/OpenCode check + API key + register)
- [ ] MSI: `msiexec /i Nexus_*_x64_en-US.msi` installs and launches
- [ ] **CI-gated**: `npx tauri build` produced both `setup.exe` + `.sig` + `.msi`;
  asset name contains the architecture (else the download scripts 404)

### S2 — Upgrade (out-of-place)

- [ ] Install `vA` → run `vB` installer → wizard offers update path
- [ ] Version in About/`get_db_stats` becomes `vB`
- [ ] Memory records, graph, config (`configuration_kv`, incl. `update_channel`)
  survive (same data dir)
- [ ] Updater path: `spawn_auto_update` writes `update_pending`, installs `vB`,
  next launch post-update health check passes and clears the marker
- [ ] **CI-gated**: DB migrations are additive + idempotent; `Version
  consistency` job ensures tag == manifest version; schema tests in
  `schema.rs` (`apply_migrations_is_idempotent`)

### S3 — Auto-update (channel)

- [ ] `update_channel=stable` → `…/releases/latest/download/latest.json` resolves
- [ ] `update_channel=beta` → `…/releases/download/channel-beta/beta.json` (then fallback)
- [ ] `update_channel=nightly` → nightly pointer → beta → stable cascade
- [ ] Interrupted download (kill app at ~10/30/50/90%) — old version keeps
  running; next launch still works
- [ ] **CI-gated**: updater endpoint tests (`infra/updater.rs`,
  `channel_endpoints`), `release.yml` channel resolution + pointer refresh

### S4 — Checksum / signature trust

- [ ] `SHA256SUMS.txt` matches every downloaded artifact
- [ ] Update signature verifies against the pinned `pubkey` in `tauri.conf.json`
  (an unsigned update is refused at install)
- [ ] **CI-gated**: release `collect` fails without a `.sig`;
  `nexus-install-lib.test.js` pins the asset matcher + checksum logic
  (it previously broke silently — this test exists so it cannot regress)

### S5 — Uninstall

- [ ] Add/Remove Programs shows Nexus (both scopes where installed)
- [ ] Uninstall removes the app binaries without admin prompt (per-user)
- [ ] Per-machine install: uninstall via per-machine registry key also works
- [ ] Data dir `%LOCALAPPDATA%\Nexus` is **kept** (uninstaller must not delete
  user data) — define: consent dialog before any data removal
- [ ] **CI-gated**: `findInstalledBinary()` in `nexus-install-lib.js` locates
  installs via HKCU/HKLM `Uninstall` keys (`com.nexus.memorytrust`, `Nexus`)

### S6 — Non-admin

- [ ] Fresh install into per-user scope without elevation
- [ ] App launches and auto-updates work without elevation (data + installer
  cache under `%LOCALAPPDATA%`)
- [ ] **CI-gated**: `cacheDir()` and `db_path()` resolve under `LOCALAPPDATA`;
  `detectArch()` handles ARM64 + WOW64 correctly (unit-tested)

## Data-preservation contract

The NSIS uninstaller is configured to remove installed binaries but not the
user data directory. This is deliberate: Nexus data (DB, snapshots, savings)
lives in `%LOCALAPPDATA%\Nexus` and must survive uninstall for a same-day
reinstall. Any change to this contract must be reviewed against
`docs/COMPATIBILITY.md` (DB schema compatibility) before release.

## How to run the matrix

1. Build locally: `npx tauri build --bundles nsis,msi` (or use CI artifacts).
2. For each OS/arch combo check S1 → S6 above on a **real machine or VM**
   (CI only guarantees builds; upgrade/interruption need a live run).
3. Record results in the release notes (see `README.md` §Журнал изменений).

## Current coverage

| Scenario | Code/CI gate | Manual verification needed |
|---|---|---|
| S1 Clean install | builds + asset-name check | wizard, custom dir, launch |
| S2 Upgrade | additive migrations, version consistency | real upgrade run |
| S3 Auto-update | endpoint tests + channel CI | interrupted download |
| S4 Checksum | `.sig` gate + install-lib tests | full sha256 spot check |
| S5 Uninstall | registry-location tests | uninstall + data retention |
| S6 Non-admin | LOCALAPPDATA resolution tests | no-UAC install on VM |