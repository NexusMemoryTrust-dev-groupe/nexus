//! Plan 7.2 — release channels (Stable / Beta / Nightly) auto-update.
//!
//! The Tauri updater resolves update *endpoints* from configuration and tries
//! them in order until one returns a parseable manifest. The GitHub "latest"
//! download URL only ever points at the newest *non-prerelease* release, so
//! beta and nightly builds cannot ride the same endpoint as stable: they are
//! published as prereleases and their manifests are also mirrored onto
//! channel-pointer releases (`channel-beta`, `channel-nightly`) so a URL that
//! never changes always carries the newest build of that channel.
//!
//! Channel endpoint cascade. The updater probes endpoints *in order* and
//! commits to the first one whose manifest parses successfully — if that
//! version is not newer than the installed one, the check ends there and does
//! NOT fall through to the remaining endpoints. The cascade below therefore
//! only bridges gaps while a channel pointer release does not exist yet (404 →
//! next endpoint); keeping beta/nightly users converging to newer stable
//! releases is the CI's job (it refreshes the pointers on every release):
//!   stable  → latest.json                 (GitHub "latest release")
//!   beta    → channel-beta/beta.json  → latest.json
//!   nightly → channel-nightly/nightly.json → channel-beta/beta.json → latest.json
//!
//! The channel is chosen at runtime from a config key (`update_channel`,
//! `configuration_kv`), which is why the check runs here on the Rust side:
//! the JS `check()` API cannot override endpoints.

use std::time::Duration;

use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::UpdaterExt;
use url::Url;

/// Config key that selects the update channel (values: `stable`, `beta`, `nightly`).
pub const UPDATE_CHANNEL_KEY: &str = "update_channel";
/// Config key recording the version being installed (plan 7.1): the next
/// startup runs the post-update health check and clears it on success.
pub const UPDATE_PENDING_KEY: &str = "update_pending";
/// Config key recording the last version whose post-update health check
/// failed, with the reason (plan 7.1) — surfaces broken installs instead of
/// silently starting on them.
pub const UPDATE_FAILED_KEY: &str = "update_failed";
/// Stable channel — updates from the newest non-prerelease GitHub release.
pub const CHANNEL_STABLE: &str = "stable";
/// Beta channel — newest beta build, falling back to stable.
pub const CHANNEL_BETA: &str = "beta";
/// Nightly channel — newest nightly build, falling back to beta, then stable.
pub const CHANNEL_NIGHTLY: &str = "nightly";

/// GitHub releases base URL for the Nexus repository.
const RELEASES_BASE: &str = "https://github.com/NexusMemoryTrust-dev-groupe/nexus/releases";

/// Normalizes a stored channel value; anything unknown defaults to `stable`.
pub fn normalize_channel(raw: Option<&str>) -> &'static str {
    match raw {
        Some(CHANNEL_BETA) => CHANNEL_BETA,
        Some(CHANNEL_NIGHTLY) => CHANNEL_NIGHTLY,
        _ => CHANNEL_STABLE,
    }
}

/// Resolve the configured update channel (default: `stable`).
pub fn resolve_channel() -> String {
    let stored = crate::commands::config::get_config_sync(UPDATE_CHANNEL_KEY.to_string())
        .ok()
        .flatten();
    normalize_channel(stored.as_deref()).to_string()
}

/// Endpoints probed in order for a channel. The updater uses the first one
/// that returns a parseable manifest, so more specific channels come first
/// and fall back to "older" channels when the pointer release has not been
/// created yet (404 → next endpoint).
pub fn channel_endpoints(channel: &str) -> Vec<Url> {
    let latest = format!("{RELEASES_BASE}/latest/download/latest.json");
    let source: Vec<String> = match normalize_channel(Some(channel)) {
        CHANNEL_BETA => vec![
            format!("{RELEASES_BASE}/download/channel-beta/beta.json"),
            latest,
        ],
        CHANNEL_NIGHTLY => vec![
            format!("{RELEASES_BASE}/download/channel-nightly/nightly.json"),
            format!("{RELEASES_BASE}/download/channel-beta/beta.json"),
            latest,
        ],
        _ => vec![latest],
    };
    source
        .into_iter()
        .filter_map(|u| Url::parse(&u).ok())
        .collect()
}

/// Spawn the background auto-update check (plan 7.2).
///
/// Mirrors the old frontend behaviour (silent check ~5s after startup, no UI)
/// but runs on the Rust side so the channel can be resolved from config and
/// the matching endpoints passed to the updater.
pub fn spawn_auto_update<R: Runtime>(app: AppHandle<R>) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Plan 7.1: if the previous launch installed an update, verify the app
        // still works (DB open + schema + MCP initialize) before probing for
        // the next one. A broken install is recorded in `update_failed`
        // instead of being silently retried or starting half-usable.
        verify_post_update_health().await;

        let channel = resolve_channel();
        let endpoints = channel_endpoints(&channel);
        tracing::info!(channel, endpoints = ?endpoints.len(), "auto-update check");

        let builder = match handle.updater_builder().endpoints(endpoints) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("auto-update: invalid endpoints for channel {channel}: {e}");
                return;
            }
        };
        let updater = match builder.build() {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!("auto-update: failed to build updater: {e}");
                return;
            }
        };

        // Plan 7.1: `download_and_install` runs the installer and relaunches
        // the app, so the "download at 10/30/50/90%" interruption window ends
        // before the marker is written — the install only starts afterwards.
        // The marker survives the restart and is consumed by
        // `verify_post_update_health` on the next launch.
        match updater.check().await {
            Ok(Some(update)) => {
                tracing::info!(
                    version = %update.version,
                    channel,
                    "auto-update: downloading and installing"
                );
                if let Err(e) = mark_update_pending(&update.version) {
                    tracing::warn!("auto-update: cannot record pending marker: {e}");
                }
                if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
                    tracing::error!("auto-update: install failed: {e}");
                }
            }
            Ok(None) => tracing::debug!(channel, "auto-update: no newer version"),
            Err(e) => tracing::warn!("auto-update: check failed (offline?): {e}"),
        }
    });
}

// ── Plan 7.1 — update rollback: post-update health check ─────────────────────

/// Record the version being installed so the next launch can verify the app
/// came back healthy (plan 7.1).
pub fn mark_update_pending(version: &str) -> Result<(), String> {
    crate::commands::config::set_config_sync(UPDATE_PENDING_KEY, version)
}

/// Clear the pending marker once the post-update health check passes.
pub fn clear_update_pending() -> Result<(), String> {
    crate::commands::config::delete_config_sync(UPDATE_PENDING_KEY)
}

/// Store the version whose post-update health check failed, plus the reason.
pub fn mark_update_failed(version: &str, reason: &str) -> Result<(), String> {
    crate::commands::config::set_config_sync(UPDATE_FAILED_KEY, &format!("{version}: {reason}"))
}

/// Health probe for a live database connection: it must be at the schema
/// version this binary knows. Older DB = binary opened a database written by
/// a *newer* release (downgrade) and must not proceed silently.
fn verify_db_health(conn: &rusqlite::Connection) -> Result<(), String> {
    let actual = crate::storage::sqlite::schema::get_schema_version(conn)
        .map_err(|e| format!("schema version read failed: {e}"))?;
    let expected = crate::storage::sqlite::schema::latest_schema_version();
    if actual != expected {
        return Err(format!(
            "schema version mismatch: db={actual}, binary expects {expected}"
        ));
    }
    Ok(())
}

/// Post-update health check (plan 7.1). Runs on launch when an update is
/// pending: opens the DB, verifies the schema version, and smoke-tests the MCP
/// `initialize` handshake. Clears the pending marker on success; on failure
/// records the broken version in `update_failed` (rollback is a manual
/// reinstall of the previous installer — documented in COMPATIBILITY.md).
async fn verify_post_update_health() {
    let pending = match crate::commands::config::get_config_sync(UPDATE_PENDING_KEY.to_string()) {
        Ok(Some(v)) if !v.is_empty() => v,
        Ok(_) => return, // no pending update
        Err(e) => {
            tracing::warn!("post-update health: cannot read pending marker: {e}");
            return;
        }
    };

    // DB open + schema — the same checks `main` performs on every launch, but
    // here they are *attributed* to the update so a broken release is caught
    // once instead of silently starting on it.
    let db = match crate::db::open_connection()
        .and_then(|conn| verify_db_health(&conn).map(|()| conn))
    {
        Ok(conn) => Some(conn),
        Err(e) => {
            tracing::error!(
                version = %pending,
                error = %e,
                "post-update health check FAILED — database is not at the expected schema"
            );
            let _ = mark_update_failed(&pending, &e);
            return;
        }
    };

    // MCP initialize smoke test through the same dispatch the stdio server
    // uses. `handle_request_line` is the documented public entry point.
    let init = crate::ai::mcp_server::handle_request_line(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"nexus-post-update","version":"1.0.0"}}}"#,
    )
    .await;

    match init {
        Some(_) => {
            tracing::info!(version = %pending, "post-update health check PASSED");
            let _ = clear_update_pending();
        }
        None => {
            let reason = "MCP initialize returned no response after update";
            tracing::error!(version = %pending, "post-update health check FAILED — {reason}");
            let _ = mark_update_failed(&pending, reason);
        }
    }

    drop(db);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_known_channels() {
        assert_eq!(normalize_channel(Some(CHANNEL_STABLE)), CHANNEL_STABLE);
        assert_eq!(normalize_channel(Some(CHANNEL_BETA)), CHANNEL_BETA);
        assert_eq!(normalize_channel(Some(CHANNEL_NIGHTLY)), CHANNEL_NIGHTLY);
    }

    #[test]
    fn normalize_defaults_to_stable() {
        assert_eq!(normalize_channel(None), CHANNEL_STABLE);
        assert_eq!(normalize_channel(Some("")), CHANNEL_STABLE);
        assert_eq!(normalize_channel(Some("canary")), CHANNEL_STABLE);
    }

    #[test]
    fn stable_channel_has_single_endpoint() {
        let eps = channel_endpoints(CHANNEL_STABLE);
        assert_eq!(eps.len(), 1);
        assert_eq!(
            eps[0].as_str(),
            "https://github.com/NexusMemoryTrust-dev-groupe/nexus/releases/latest/download/latest.json"
        );
    }

    #[test]
    fn beta_channel_tries_pointer_then_stable() {
        let eps = channel_endpoints(CHANNEL_BETA);
        assert_eq!(eps.len(), 2);
        assert!(
            eps[0]
                .as_str()
                .ends_with("/releases/download/channel-beta/beta.json")
        );
        assert!(
            eps[1]
                .as_str()
                .ends_with("/releases/latest/download/latest.json")
        );
    }

    #[test]
    fn nightly_channel_cascades_nightly_beta_stable() {
        let eps = channel_endpoints(CHANNEL_NIGHTLY);
        assert_eq!(eps.len(), 3);
        assert!(
            eps[0]
                .as_str()
                .ends_with("/releases/download/channel-nightly/nightly.json")
        );
        assert!(
            eps[1]
                .as_str()
                .ends_with("/releases/download/channel-beta/beta.json")
        );
        assert!(
            eps[2]
                .as_str()
                .ends_with("/releases/latest/download/latest.json")
        );
    }

    #[test]
    fn health_accepts_db_at_latest_schema() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&conn).unwrap();
        assert!(verify_db_health(&conn).is_ok());
    }

    #[test]
    fn health_rejects_db_at_older_schema() {
        // Fresh in-memory DB: schema version 0, binary expects the latest.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let err = verify_db_health(&conn).unwrap_err();
        assert!(
            err.contains("schema version mismatch"),
            "unexpected error: {err}"
        );
        assert!(err.contains("binary expects"));
    }
}
