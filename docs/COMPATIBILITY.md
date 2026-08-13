# Compatibility Policy & Matrix

What must keep working when Nexus is upgraded, downgraded, or moved between
machines. Every guarantee in this document is backed by a test or a CI gate —
see the "Enforced by" column.

App version: `1.1.0`. Schema version: `32`.

## Versioning model

Nexus follows **semantic versioning** (`MAJOR.MINOR.PATCH`, see
`src-tauri/tauri.conf.json`):

- **MAJOR** — breaking changes: incompatible DB schema, removed MCP tools,
  dropped platform support, changed data format.
- **MINOR** — additive: new MCP tools, new config keys, new features. Old data
  and old clients keep working; new schema is added by additive migrations.
- **PATCH** — fixes that change no contracts.

The updater (`tauri-plugin-updater`) installs *newer* versions automatically.
Downgrades are manual (reinstall an older installer) and are only supported
within the same MAJOR version.

## Platform matrix

| OS | Arch | Installer | Support | Notes |
|---|---|---|---|---|
| Windows 10 (1809+) | x64 | NSIS + MSI | ✅ full | WebView2 auto-installed |
| Windows 10 (1809+) | ARM64 | NSIS + MSI | ✅ full | x64 emulation not used; native ARM64 build |
| Windows 11 | x64 | NSIS + MSI | ✅ full | |
| Windows 11 | ARM64 | NSIS + MSI | ✅ full | Surface, Snapdragon |
| Windows 10 < 1809 | x64 | — | ❌ unsupported | WebView2 requirement |
| 32-bit (x86) | i686 | — | ❌ unsupported | ONNX Runtime ships no i686 binaries |
| macOS / Linux | — | — | ❌ not shipped | codebase is portable; no releases |

Enforced by: `.github/workflows/release.yml` builds x64 + ARM64 only; the
collect step fails if a `setup` asset lacks the architecture in its name.

## Database schema compatibility

Data lives in `%LOCALAPPDATA%\Nexus\nexus.db` (SQLite, WAL mode).

| Scenario | Supported | How | Enforced by |
|---|---|---|---|
| Upgrade to a newer MINOR/PATCH | ✅ | Additive migrations V1→V32, applied on open; idempotent (`ADD COLUMN` guarded by `column_exists`) | `schema` integration tests |
| Upgrade to a newer MAJOR | ✅ (one-way) | Migrations run before the app serves requests; old DB is migrated in place | `schema_version_increases` test |
| Downgrade to an older PATCH within same MAJOR | ✅ | No schema change between patches → same schema version | n/a |
| Downgrade to an older MINOR | ✅ only if schema version ≤ installed | `latest_schema_version()` is the max the binary knows; an older binary refuses a newer DB (`rollback_last_migration` exists for ops but is not a user path) | `rollback_last_migration` test |
| Moving the data folder to another machine | ✅ | DB is self-contained; paths are resolved at runtime | `db_path` unit tests |

Forward guarantee: a database opened by version N can be opened by any version
≥ N. Backward guarantee: version N cannot open a database created by version
> N (it stops with a clear schema-version error instead of corrupting data).

Enforced by: `src-tauri/src/storage/sqlite/schema.rs` — migrations are
compiled into the binary (`include_str!`), never read from disk, so a binary
can only ever see the migrations it was built with.

## Configuration compatibility

- Configuration lives in the `configuration_kv` SQLite table (and
  `configuration_provider.rs` for typed access). Unknown keys are ignored on
  read; unknown **feature flags** are rejected on write
  (`AppError::Configuration`).
- Feature flags default to ON = current production behavior, so a new install
  behaves identically to a downgraded one. Turning a flag OFF changes behavior
  without a schema change (no data migration — see
  `docs/BUILD_REPRODUCIBILITY.md` §feature flags and
  `src-tauri/src/core/config/feature_flags.rs`).
- Missing config keys fall back to built-in defaults; `config_set` writes are
  additive and never delete other keys.

Enforced by: `feature_flags` unit tests (7) + `configuration_kv` tests.

## MCP API compatibility

| Component | Value | Policy |
|---|---|---|
| MCP protocol version | `2024-11-05` | fixed; the server rejects unknown protocol versions |
| Nexus MCP API version | `1.0.0` (`MCP_API_VERSION`) | reported by `initialize`; clients should pin against it |
| Tools | 143 | tools are **additive only** within a MAJOR; deprecation is flagged (`deprecated: true`) for one release before removal |
| Tool schemas | JSON Schema | parameters may be added; existing parameters keep types; new required parameters only in a MAJOR |

`docs/mcp/` (README + reference + tools.json) is a generated snapshot of
`tool_definitions()`; regenerate with `scripts/generate-mcp-docs.mjs` when the
Rust schemas change.

Enforced by: `tool_definitions_have_required_fields`, `tools/list` smoke tests,
and the `initialize_reports_api_version` test in `mcp_server.rs`.

## Auto-update compatibility

- Updates are checked on the Rust side (`spawn_auto_update`, ~5 s after start,
  silent) and install **only newer** versions; the manifest version must match
  the tag it is attached to (checked by the `Version consistency` job).
- Release channel is selected by the `update_channel` config key
  (`configuration_kv`): `stable` (default), `beta`, `nightly`. Unknown values
  normalize to `stable`. This is a *channel* key — it does not change the MCP
  API or the DB schema, only which release feed the updater polls.
- Endpoint cascade per channel (the updater commits to the first manifest that
  parses; a 404 falls through to the next endpoint):

  | Channel | Endpoints (in order) |
  |---|---|
  | stable | `…/releases/latest/download/latest.json` |
  | beta | `…/releases/download/channel-beta/beta.json` → stable |
  | nightly | `…/releases/download/channel-nightly/nightly.json` → beta → stable |

- Beta/nightly releases are GitHub **prereleases**, so
  `…/releases/latest/download/latest.json` always resolves to the newest
  *stable* build and stable-channel clients never download a pre-release.
- The `channel-beta` / `channel-nightly` releases are floating pointers that
  CI refreshes (`--clobber`) on **every** release — including stable ones — so
  beta/nightly users converge to the newest stable build once it ships; a
  pointer is never stepped backwards (semver guard in `release.yml`).
- New installs and untouched configs stay on `stable`, preserving the
  pre-7.2 endpoint exactly.
- Installers are verified against `SHA256SUMS.txt` before/during install.
- Plan 7.1 (rollback health check): before installing, the updater records the
  target version in the `update_pending` config key; on the next launch a
  post-update probe verifies DB open + schema version + MCP `initialize`, then
  clears the marker on success or records `update_failed` (version + reason) on
  failure so a broken install is surfaced instead of silently starting. See
  `docs/PRODUCTION_READINESS_PLAN.md` item 7.1.

## Workspace / file format compatibility

- Exported context (Markdown / JSON / plain text) is a stable contract; fields
  may be added, never renamed or removed within a MAJOR.
- Workspace sandbox rules (`sandbox.extra_roots`, whitelist) are stored in
  config; unknown rules degrade to the default (whitelist only) rather than
  failing.

## Changelog contract

Every release notes which contract changed:

- **DB**: schema version bump + migration list
- **MCP**: new tools / deprecated tools / schema changes
- **Config**: new keys, changed defaults
- **Platform**: added/dropped OS or arch support

See `README.md` §Журнал изменений for the running log.

---

*Compatibility is a release gate: any PR that changes a contract listed above
must update this document and the corresponding test in the same commit.*
