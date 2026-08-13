use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::backup::Progress;
use rusqlite::{Connection, MAIN_DB};
use serde::{Deserialize, Serialize};

use crate::core::result::{AppError, Result};
use crate::infra::{new_request_id, run_operation};
use crate::storage::sqlite::schema;

/// Magic header of a `.nexusbackup` file: `NEXUSBK1`.
const BACKUP_MAGIC: &[u8; 8] = b"NEXUSBK1";
/// Format version of the backup container (bump on breaking format change).
const BACKUP_FORMAT_VERSION: u32 = 1;

/// File extension used for backups.
pub const BACKUP_EXTENSION: &str = "nexusbackup";

/// Serialized metadata recorded inside the backup container.
#[derive(Serialize, Deserialize)]
struct BackupManifest {
    format_version: u32,
    created_at: String,
    schema_version: i32,
    nexus_version: String,
    memory_count: u64,
    payload_len: u64,
    payload_sha256: String,
}

/// Backup metadata surfaced to the UI / CLI.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub file_name: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub schema_version: i32,
    pub memory_count: u64,
    pub sha256: String,
    pub verified: bool,
}

/// Report of a completed restore.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub restored_from: String,
    pub restored_at: String,
    pub schema_version: i32,
    pub memory_count: u64,
    pub pre_restore_backup: String,
}

/// Small hex encoder (avoids pulling a dependency just for formatting).
fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn sha256(data: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, data);
    hex_encode(digest.as_ref())
}

/// Default directory for backups: `<db dir>/backups`.
pub fn default_backup_dir() -> PathBuf {
    default_backup_dir_at(&crate::db::db_path())
}

/// Same as [`default_backup_dir`] but relative to an explicit database path.
pub fn default_backup_dir_at(db_path: &Path) -> PathBuf {
    let db_dir = db_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    db_dir.join("backups")
}

/// Count memory records on a live connection (used for manifest metadata).
fn memory_count(conn: &Connection) -> Result<u64> {
    conn.query_row("SELECT COUNT(*) FROM memory_records", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|n| n as u64)
    .map_err(|e| AppError::Database(format!("failed to count memories: {e}")))
}

/// Create a full backup of the live database into `dest_dir`.
///
/// The backup is a single self-contained `.nexusbackup` file:
///
/// ```text
/// [8 bytes  magic "NEXUSBK1"]
/// [4 bytes  u32 LE manifest length]
/// [N bytes  JSON manifest]
/// [M bytes  raw SQLite snapshot payload]
/// ```
///
/// The snapshot is produced with the SQLite Online Backup API, so it is
/// consistent even while the app keeps writing (WAL mode) — no file-level
/// copy race, no `VACUUM` exclusive lock.
pub fn create_backup(dest_dir: &Path) -> Result<BackupInfo> {
    create_backup_at(&crate::db::db_path(), dest_dir)
}

/// Same as [`create_backup`] but against an explicit database path — used by
/// tests for full isolation (no global env mutation, no cross-test races).
pub fn create_backup_at(db_path: &Path, dest_dir: &Path) -> Result<BackupInfo> {
    let request_id = new_request_id();
    run_operation("backup_create", "backup", &request_id, || {
        create_backup_inner(db_path, dest_dir)
    })
}

fn create_backup_inner(db_path: &Path, dest_dir: &Path) -> Result<BackupInfo> {
    fs::create_dir_all(dest_dir).map_err(|e| {
        AppError::backup_failure(format!(
            "cannot create backup dir {}: {e}",
            dest_dir.display()
        ))
    })?;

    let live = crate::db::open_connection_at(db_path)
        .map_err(|e| AppError::backup_failure(format!("cannot open live db: {e}")))?;
    let schema_version = schema::get_schema_version(&live)
        .map_err(|e| AppError::backup_failure(format!("cannot read schema version: {e}")))?;
    let count = memory_count(&live)?;

    // 1. Snapshot the live DB into a temporary SQLite file via the backup API.
    let temp_dir = std::env::temp_dir();
    let snapshot_path = temp_dir.join(format!("nexus-snapshot-{}.db", uuid::Uuid::new_v4()));
    live.backup(MAIN_DB, &snapshot_path, Some(progress_noop))
        .map_err(|e| AppError::backup_failure(format!("online backup failed: {e}")))?;
    // Make sure everything is on disk before we read the bytes.
    {
        let snapshot = Connection::open(&snapshot_path)
            .map_err(|e| AppError::backup_failure(format!("cannot open snapshot file: {e}")))?;
        snapshot
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| AppError::backup_failure(format!("checkpoint failed: {e}")))?;
    }

    // 2. Read snapshot bytes, compute digest, build the container.
    let payload = fs::read(&snapshot_path).map_err(|e| {
        AppError::backup_failure(format!(
            "cannot read snapshot {}: {e}",
            snapshot_path.display()
        ))
    })?;
    let payload_sha256 = sha256(&payload);
    let created_at = Utc::now().to_rfc3339();

    let manifest = BackupManifest {
        format_version: BACKUP_FORMAT_VERSION,
        created_at: created_at.clone(),
        schema_version,
        nexus_version: env!("CARGO_PKG_VERSION").to_string(),
        memory_count: count,
        payload_len: payload.len() as u64,
        payload_sha256: payload_sha256.clone(),
    };
    let manifest_json = serde_json::to_vec(&manifest)
        .map_err(|e| AppError::backup_failure(format!("cannot serialize manifest: {e}")))?;

    let file_name = format!(
        "nexus-backup-{}-{}.{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        &uuid::Uuid::new_v4().simple().to_string()[..8],
        BACKUP_EXTENSION
    );
    let dest_path = dest_dir.join(&file_name);
    let mut out = Vec::with_capacity(12 + manifest_json.len() + payload.len());
    out.extend_from_slice(BACKUP_MAGIC);
    out.extend_from_slice(&(manifest_json.len() as u32).to_le_bytes());
    out.extend_from_slice(&manifest_json);
    out.extend_from_slice(&payload);
    fs::write(&dest_path, &out).map_err(|e| {
        AppError::backup_failure(format!("cannot write backup {}: {e}", dest_path.display()))
    })?;

    // 3. Cleanup the temp snapshot file.
    let _ = fs::remove_file(&snapshot_path);

    // 4. Record the backup in history (append-only journal).
    record_history(
        db_path,
        &dest_path,
        &created_at,
        schema_version,
        &payload_sha256,
        "active",
        None,
    )?;

    let info = BackupInfo {
        path: dest_path.display().to_string(),
        file_name,
        created_at,
        size_bytes: out.len() as u64,
        schema_version,
        memory_count: count,
        sha256: payload_sha256,
        verified: true,
    };
    Ok(info)
}

/// No-op progress callback (required by the backup API; we run it synchronously
/// so there is nothing to report into).
fn progress_noop(_p: Progress) {}

/// Read and validate the manifest + digest of a `.nexusbackup` file without
/// extracting it. Returns the manifest, and the byte-range of the payload.
fn read_manifest(path: &Path) -> Result<(BackupManifest, u64)> {
    let bytes = fs::read(path).map_err(|e| {
        AppError::backup_failure(format!("cannot read backup {}: {e}", path.display()))
    })?;
    if bytes.len() < 12 || &bytes[..8] != BACKUP_MAGIC {
        return Err(AppError::backup_failure(format!(
            "{} is not a valid .nexusbackup file (bad magic)",
            path.display()
        )));
    }
    let manifest_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if bytes.len() < 12 + manifest_len {
        return Err(AppError::backup_failure(format!(
            "{} is truncated: manifest exceeds file size",
            path.display()
        )));
    }
    let manifest: BackupManifest =
        serde_json::from_slice(&bytes[12..12 + manifest_len]).map_err(|e| {
            AppError::backup_failure(format!("corrupt manifest in {}: {e}", path.display()))
        })?;
    if manifest.format_version != BACKUP_FORMAT_VERSION {
        return Err(AppError::backup_failure(format!(
            "{} uses backup format v{}, this build supports v{}",
            path.display(),
            manifest.format_version,
            BACKUP_FORMAT_VERSION
        )));
    }
    let payload_start = 12 + manifest_len;
    if bytes.len() as u64 != (payload_start as u64) + manifest.payload_len {
        return Err(AppError::backup_failure(format!(
            "{} has inconsistent payload length (expected {}, found {})",
            path.display(),
            manifest.payload_len,
            bytes.len() as u64 - payload_start as u64
        )));
    }
    let digest = sha256(&bytes[payload_start..]);
    if digest != manifest.payload_sha256 {
        return Err(AppError::backup_failure(format!(
            "{} failed checksum verification (payload corrupted or tampered)",
            path.display()
        )));
    }
    Ok((manifest, payload_start as u64))
}

/// Verify a `.nexusbackup` file: header, checksum, and (optionally, when the
/// snapshot opens) SQLite integrity. The integrity check is best-effort — if
/// the payload cannot be opened, verification fails.
pub fn verify_backup(path: &Path) -> Result<BackupInfo> {
    let (manifest, _) = read_manifest(path)?;
    let size_bytes = fs::metadata(path)
        .map_err(|e| AppError::backup_failure(format!("cannot stat {}: {e}", path.display())))?
        .len();

    // Extract the payload to a temp file and run `PRAGMA integrity_check`.
    let payload = extract_payload(path)?;
    let temp = std::env::temp_dir().join(format!("nexus-verify-{}.db", uuid::Uuid::new_v4()));
    fs::write(&temp, &payload)
        .map_err(|e| AppError::backup_failure(format!("cannot stage payload for verify: {e}")))?;
    let conn = Connection::open(&temp)
        .map_err(|e| AppError::backup_failure(format!("cannot open snapshot: {e}")))?;
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| AppError::backup_failure(format!("integrity check failed: {e}")))?;
    drop(conn);
    let _ = fs::remove_file(&temp);
    if integrity != "ok" {
        return Err(AppError::backup_failure(format!(
            "{} failed SQLite integrity check: {integrity}",
            path.display()
        )));
    }

    Ok(BackupInfo {
        path: path.display().to_string(),
        file_name: path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        created_at: manifest.created_at,
        size_bytes,
        schema_version: manifest.schema_version,
        memory_count: manifest.memory_count,
        sha256: manifest.payload_sha256,
        verified: true,
    })
}

/// Extract the raw SQLite snapshot payload from a `.nexusbackup` file.
fn extract_payload(path: &Path) -> Result<Vec<u8>> {
    let bytes = fs::read(path).map_err(|e| {
        AppError::backup_failure(format!("cannot read backup {}: {e}", path.display()))
    })?;
    let (_, payload_start) = read_manifest(path)?;
    Ok(bytes[payload_start as usize..].to_vec())
}

/// List all `.nexusbackup` files in a directory, newest first.
pub fn list_backups(dir: &Path) -> Result<Vec<BackupInfo>> {
    let mut infos = Vec::new();
    if !dir.exists() {
        return Ok(infos);
    }
    let entries = fs::read_dir(dir)
        .map_err(|e| AppError::backup_failure(format!("cannot read {}: {e}", dir.display())))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == BACKUP_EXTENSION)
            && let Ok(info) = verify_backup(&path)
        {
            infos.push(info);
        }
    }
    infos.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(infos)
}

/// Delete a backup file and mark it as deleted in history.
pub fn delete_backup(path: &Path) -> Result<()> {
    delete_backup_at(&crate::db::db_path(), path)
}

/// Same as [`delete_backup`] but against an explicit database path.
pub fn delete_backup_at(db_path: &Path, path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(AppError::backup_failure(format!(
            "backup {} does not exist",
            path.display()
        )));
    }
    fs::remove_file(path)
        .map_err(|e| AppError::backup_failure(format!("cannot delete {}: {e}", path.display())))?;
    mark_history(db_path, path, "deleted")?;
    Ok(())
}

/// Restore a `.nexusbackup` into the live database.
///
/// Strategy — no file swapping under open connections:
/// 1. Full verification (checksum + SQLite integrity) of the backup.
/// 2. Schema compatibility check: refuse a backup whose schema is newer than
///    the current build; warn-and-continue (after a safety backup) when older.
/// 3. Safety backup of the current state (so a restore is always reversible).
/// 4. Apply the snapshot into the live DB via the Online Backup API — atomic
///    at the SQLite level, safe with WAL, no window of a half-written DB.
pub fn restore_backup(path: &Path) -> Result<RestoreReport> {
    restore_backup_at(&crate::db::db_path(), path)
}

/// Same as [`restore_backup`] but against an explicit database path.
pub fn restore_backup_at(db_path: &Path, path: &Path) -> Result<RestoreReport> {
    let request_id = new_request_id();
    run_operation("backup_restore", "backup", &request_id, || {
        restore_backup_inner(db_path, path)
    })
}

fn restore_backup_inner(db_path: &Path, path: &Path) -> Result<RestoreReport> {
    // Full verification (checksum + integrity) — fail fast before touching
    // anything; the returned info is intentionally discarded.
    verify_backup(path)?;
    let (manifest, _) = read_manifest(path)?;

    let live = crate::db::open_connection_at(db_path)
        .map_err(|e| AppError::backup_failure(format!("cannot open live db: {e}")))?;
    let current_version = schema::get_schema_version(&live)
        .map_err(|e| AppError::backup_failure(format!("cannot read current schema: {e}")))?;

    // Refuse restoring a NEWER schema into an older app build — tables the app
    // does not know about would be silently orphaned.
    if manifest.schema_version > current_version {
        return Err(AppError::backup_failure(format!(
            "backup schema v{} is newer than current schema v{} — upgrade the app before restoring",
            manifest.schema_version, current_version
        )));
    }

    // Safety net: snapshot current state before touching anything.
    let safety_dir = default_backup_dir_at(db_path);
    let safety = create_backup_at(db_path, &safety_dir)?;

    // Extract payload and stage it as a file the restore API can read.
    let payload = extract_payload(path)?;
    let temp = std::env::temp_dir().join(format!("nexus-restore-{}.db", uuid::Uuid::new_v4()));
    fs::write(&temp, &payload)
        .map_err(|e| AppError::backup_failure(format!("cannot stage restore payload: {e}")))?;

    // Apply the snapshot INTO the live DB. `Connection::restore` uses the
    // SQLite Online Backup API: it copies the source file into the live
    // connection's main database in one atomic operation, safe under WAL.
    let mut live = live;
    live.restore(MAIN_DB, &temp, Some(progress_noop))
        .map_err(|e| AppError::backup_failure(format!("restore apply failed: {e}")))?;
    let _ = fs::remove_file(&temp);

    // Re-run migrations so the restored DB catches up to the app's schema if
    // the backup was older (idempotent; a no-op when versions match).
    let _ = schema::apply_migrations(&live);

    let restored_at = Utc::now().to_rfc3339();
    let _ = mark_history(db_path, path, "restored");

    Ok(RestoreReport {
        restored_from: path.display().to_string(),
        restored_at,
        schema_version: manifest.schema_version,
        memory_count: manifest.memory_count,
        pre_restore_backup: safety.path,
    })
}

// ---------------------------------------------------------------------------
// backup_history journal
// ---------------------------------------------------------------------------

fn record_history(
    db_path: &Path,
    path: &Path,
    created_at: &str,
    schema_version: i32,
    sha: &str,
    status: &str,
    restored_at: Option<&str>,
) -> Result<()> {
    let conn = crate::db::open_connection_at(db_path)
        .map_err(|e| AppError::backup_failure(format!("cannot open db for history: {e}")))?;
    let size = fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
    conn.execute(
        "INSERT INTO backup_history (id, path, created_at, schema_version, size_bytes, sha256, status, restored_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            path.display().to_string(),
            created_at,
            schema_version,
            size,
            sha,
            status,
            restored_at,
        ],
    )
    .map_err(|e| AppError::backup_failure(format!("cannot record backup history: {e}")))?;
    Ok(())
}

fn mark_history(db_path: &Path, path: &Path, status: &str) -> Result<()> {
    let conn = crate::db::open_connection_at(db_path)
        .map_err(|e| AppError::backup_failure(format!("cannot open db for history: {e}")))?;
    conn.execute(
        "UPDATE backup_history SET status = ?1 WHERE path = ?2",
        rusqlite::params![status, path.display().to_string()],
    )
    .map_err(|e| AppError::backup_failure(format!("cannot update backup history: {e}")))?;
    Ok(())
}

/// List the backup journal (all backups ever created, regardless of file
/// existence). Used by the UI to render history.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub path: String,
    pub created_at: String,
    pub schema_version: i32,
    pub size_bytes: i64,
    pub sha256: String,
    pub status: String,
    pub restored_at: Option<String>,
}

pub fn list_history() -> Result<Vec<HistoryEntry>> {
    list_history_at(&crate::db::db_path())
}

/// Same as [`list_history`] but against an explicit database path (testable
/// in isolation without touching the global data-dir env).
pub fn list_history_at(db_path: &Path) -> Result<Vec<HistoryEntry>> {
    let conn = crate::db::open_connection_at(db_path)
        .map_err(|e| AppError::backup_failure(format!("cannot open db for history: {e}")))?;
    let mut stmt = conn
        .prepare(
            "SELECT path, created_at, schema_version, size_bytes, sha256, status, restored_at
             FROM backup_history ORDER BY created_at DESC",
        )
        .map_err(|e| AppError::backup_failure(format!("cannot query backup history: {e}")))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(HistoryEntry {
                path: r.get(0)?,
                created_at: r.get(1)?,
                schema_version: r.get(2)?,
                size_bytes: r.get(3)?,
                sha256: r.get(4)?,
                status: r.get(5)?,
                restored_at: r.get(6)?,
            })
        })
        .map_err(|e| AppError::backup_failure(format!("cannot iterate backup history: {e}")))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| {
            AppError::backup_failure(format!("cannot read backup history row: {e}"))
        })?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a throwaway SQLite file, migrate it and return its path. All
    /// backup helpers take an explicit db path — no global env is touched, so
    /// tests never race with each other (previously this module mutated
    /// LOCALAPPDATA/HOME, which broke parallel test runs).
    fn fresh_db(prefix: &str) -> std::path::PathBuf {
        let tmp = std::env::temp_dir().join(format!("nexus-{prefix}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let db = tmp.join("nexus.db");
        let conn = crate::db::open_connection_at(&db).expect("open test db");
        schema::apply_migrations(&conn).expect("migrate test db");
        drop(conn);
        db
    }

    /// Cheap structural test: create → verify → list → delete round-trip on a
    /// throwaway file database.
    #[test]
    fn backup_roundtrip_lifecycle() {
        let db = fresh_db("backup");
        let conn = crate::db::open_connection_at(&db).expect("open test db");
        // Insert one memory so the manifest count is non-zero.
        conn.execute(
            "INSERT INTO memory_records (id, title, summary, content, created_at, updated_at, author, source)
             VALUES ('m1', 'hello', '', 'hello world', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'test', 'test')",
            [],
        )
        .expect("insert memory");
        drop(conn);

        let dir = db.parent().unwrap().join("backups");
        let info = create_backup_at(&db, &dir).expect("create backup");
        assert!(info.verified);
        assert_eq!(info.memory_count, 1);
        assert!(Path::new(&info.path).exists());

        let verified = verify_backup(Path::new(&info.path)).expect("verify backup");
        assert!(verified.verified);

        let listed = list_backups(&dir).expect("list backups");
        assert_eq!(listed.len(), 1);

        let history = list_history_at(&db).expect("history");
        assert!(!history.is_empty());

        delete_backup_at(&db, Path::new(&info.path)).expect("delete backup");
        assert!(!Path::new(&info.path).exists());

        let _ = fs::remove_dir_all(db.parent().unwrap());
    }

    #[test]
    fn verify_rejects_corrupted_file() {
        let tmp = std::env::temp_dir().join(format!("nexus-test-corrupt-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("bad.nexusbackup");
        fs::write(&path, b"NEXUSBK1\x00\x00\x00\x00junk").unwrap();
        let err = verify_backup(&path).expect_err("should reject garbage");
        assert!(err.to_string().contains("not a valid") || err.to_string().contains("manifest"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        // Build a valid backup then flip a byte in the payload region.
        let db = fresh_db("tamper");
        let dir = db.parent().unwrap().join("backups");
        let info = create_backup_at(&db, &dir).expect("create backup");
        let mut bytes = fs::read(&info.path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        fs::write(&info.path, &bytes).unwrap();

        let err = verify_backup(Path::new(&info.path)).expect_err("must fail checksum");
        assert!(err.to_string().contains("checksum"));

        let _ = fs::remove_dir_all(db.parent().unwrap());
    }

    #[test]
    fn restore_roundtrip_restores_content() {
        let db = fresh_db("restore");
        let conn = crate::db::open_connection_at(&db).expect("open test db");
        conn.execute(
            "INSERT INTO memory_records (id, title, summary, content, created_at, updated_at, author, source)
             VALUES ('keep', 'keep me', '', 'original content', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'test', 'test')",
            [],
        )
        .expect("insert memory");
        drop(conn);

        let dir = db.parent().unwrap().join("backups");
        let info = create_backup_at(&db, &dir).expect("create backup");

        // Mutate the DB so the restore has something to revert.
        let conn = crate::db::open_connection_at(&db).expect("open test db");
        conn.execute("DELETE FROM memory_records WHERE id = 'keep'", [])
            .expect("delete memory");
        drop(conn);

        let report = restore_backup_at(&db, Path::new(&info.path)).expect("restore backup");
        assert_eq!(report.memory_count, 1);
        assert!(report.pre_restore_backup.ends_with(".nexusbackup"));

        // Content is back after the restore.
        let conn = crate::db::open_connection_at(&db).expect("open test db");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_records", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1);
        let title: String = conn
            .query_row(
                "SELECT title FROM memory_records WHERE id = 'keep'",
                [],
                |r| r.get(0),
            )
            .expect("title");
        assert_eq!(title, "keep me");
        drop(conn);

        let _ = fs::remove_dir_all(db.parent().unwrap());
    }

    #[test]
    fn restore_refuses_newer_schema() {
        let db = fresh_db("newer-schema");
        let dir = db.parent().unwrap().join("backups");
        let info = create_backup_at(&db, &dir).expect("create backup");

        // Rewrite the manifest with a schema version far in the future: the
        // payload region stays intact, but the manifest schema check must trip.
        let bytes = fs::read(&info.path).unwrap();
        let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&bytes[12..12 + len]).expect("manifest json");
        manifest["schema_version"] = serde_json::json!(9999);
        let new_manifest = serde_json::to_vec(&manifest).unwrap();
        let mut out = Vec::with_capacity(12 + new_manifest.len() + bytes.len() - 12 - len);
        out.extend_from_slice(&bytes[..8]);
        out.extend_from_slice(&(new_manifest.len() as u32).to_le_bytes());
        out.extend_from_slice(&new_manifest);
        out.extend_from_slice(&bytes[12 + len..]);
        fs::write(&info.path, &out).unwrap();

        let err = restore_backup_at(&db, Path::new(&info.path)).expect_err("must refuse");
        assert!(err.to_string().contains("newer than current schema"));

        let _ = fs::remove_dir_all(db.parent().unwrap());
    }

    // ── Coverage: global wrappers and error branches ─────────────────────────

    /// Rebuild a `.nexusbackup` container from its parts with a tweaked
    /// manifest and an optional replacement payload.
    fn rebuild_container(
        bytes: &[u8],
        new_payload: &[u8],
        tweak: impl FnOnce(&mut serde_json::Value),
    ) -> Vec<u8> {
        let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&bytes[12..12 + len]).expect("manifest json");
        tweak(&mut manifest);
        let new_manifest = serde_json::to_vec(&manifest).unwrap();
        let mut out = Vec::with_capacity(12 + new_manifest.len() + new_payload.len());
        out.extend_from_slice(&bytes[..8]);
        out.extend_from_slice(&(new_manifest.len() as u32).to_le_bytes());
        out.extend_from_slice(&new_manifest);
        out.extend_from_slice(new_payload);
        out
    }

    #[test]
    fn create_backup_rejects_uncreatable_dest_dir() {
        let db = fresh_db("nodest");
        // A regular *file* where the backups directory should be created.
        let blocker = db.parent().unwrap().join("blocker.file");
        fs::write(&blocker, b"occupied").unwrap();
        let err = create_backup_at(&db, &blocker).expect_err("file as dest dir must fail");
        assert!(
            err.to_string().contains("cannot create backup dir"),
            "got: {err}"
        );
        let _ = fs::remove_dir_all(db.parent().unwrap());
    }

    #[test]
    fn read_manifest_rejects_bad_magic_and_truncation() {
        let tmp = std::env::temp_dir().join(format!("nexus-magic-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let p = tmp.join("bad.nexusbackup");

        // Too short to even carry a manifest length.
        fs::write(&p, b"XXXX").unwrap();
        let err = verify_backup(&p).expect_err("short file must be rejected");
        assert!(err.to_string().contains("not a valid"), "got: {err}");

        // Right size, wrong magic.
        fs::write(&p, b"NOTNEXUS!\x00\x00\x00\x00").unwrap();
        let err = verify_backup(&p).expect_err("bad magic must be rejected");
        assert!(err.to_string().contains("not a valid"), "got: {err}");

        // Manifest length beyond the end of the file: just a 12-byte header
        // claiming a far larger manifest than the file actually carries.
        let mut header = Vec::new();
        header.extend_from_slice(BACKUP_MAGIC);
        header.extend_from_slice(&1000u32.to_le_bytes());
        fs::write(&p, &header).unwrap();
        let err = verify_backup(&p).expect_err("truncated manifest must be rejected");
        assert!(err.to_string().contains("truncated"), "got: {err}");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn read_manifest_rejects_future_format_and_bad_payload_length() {
        let db = fresh_db("versions");
        let dir = db.parent().unwrap().join("backups");
        let info = create_backup_at(&db, &dir).expect("create backup");
        let bytes = fs::read(&info.path).unwrap();
        let payload_start = 12 + u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let payload = &bytes[payload_start..];

        // Unsupported container format version.
        let future = rebuild_container(&bytes, payload, |m| {
            m["format_version"] = serde_json::json!(2);
        });
        fs::write(&info.path, &future).unwrap();
        let err = verify_backup(Path::new(&info.path)).expect_err("future format must be rejected");
        assert!(err.to_string().contains("format v"), "got: {err}");

        // Payload length that disagrees with the actual file size.
        let inconsistent = rebuild_container(&bytes, payload, |m| {
            if let Some(n) = m["payload_len"].as_u64() {
                m["payload_len"] = serde_json::json!(n + 100);
            }
        });
        fs::write(&info.path, &inconsistent).unwrap();
        let err =
            verify_backup(Path::new(&info.path)).expect_err("length mismatch must be rejected");
        assert!(err.to_string().contains("inconsistent"), "got: {err}");

        let _ = fs::remove_dir_all(db.parent().unwrap());
    }

    #[test]
    fn verify_rejects_payload_that_fails_sqlite_integrity() {
        // A payload that passes the checksum but is a malformed SQLite image.
        // Truncating the file makes SQLite *error* on the query; corrupting a
        // chunk in the middle (a b-tree page) makes `PRAGMA integrity_check`
        // *report* a non-"ok" row — that is the branch we exercise.
        let db = fresh_db("integrity");
        let dir = db.parent().unwrap().join("backups");
        let info = create_backup_at(&db, &dir).expect("create backup");
        let bytes = fs::read(&info.path).unwrap();
        let payload_start = 12 + u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;

        let mut broken = bytes[payload_start..].to_vec();
        let mid = broken.len() / 2;
        for b in broken.iter_mut().skip(mid).take(256) {
            *b = 0xFF;
        }
        let rebuilt = rebuild_container(&bytes, &broken, |m| {
            m["payload_len"] = serde_json::json!(broken.len() as u64);
            m["payload_sha256"] = serde_json::json!(sha256(&broken));
        });
        fs::write(&info.path, &rebuilt).unwrap();

        let err = verify_backup(Path::new(&info.path)).expect_err("broken sqlite must fail");
        assert!(err.to_string().contains("integrity check"), "got: {err}");

        let _ = fs::remove_dir_all(db.parent().unwrap());
    }

    #[test]
    fn verify_backup_reports_unreadable_path() {
        // Covers the fs::read error branch inside read_manifest.
        let err = verify_backup(Path::new(r"Z:\definitely\missing.nexusbackup"))
            .expect_err("unreadable path must error");
        assert!(err.to_string().contains("cannot read backup"), "got: {err}");
    }

    #[test]
    fn create_backup_reports_history_write_failure() {
        // Drops backup_history so the post-backup journal insert fails — the
        // `?` error path of record_history must propagate.
        let db = fresh_db("histfail");
        {
            let conn = crate::db::open_connection_at(&db).expect("open test db");
            conn.execute_batch("DROP TABLE backup_history")
                .expect("drop history");
        }
        let dir = db.parent().unwrap().join("backups");
        let err = create_backup_at(&db, &dir).expect_err("history failure must fail backup");
        assert!(
            err.to_string().contains("cannot record backup history"),
            "got: {err}"
        );
        let _ = fs::remove_dir_all(db.parent().unwrap());
    }

    #[test]
    fn list_backups_missing_dir_is_empty_and_extract_payload_errors() {
        let none = std::env::temp_dir().join(format!("nexus-nodir-{}", uuid::Uuid::new_v4()));
        assert!(
            list_backups(&none)
                .expect("missing dir lists empty")
                .is_empty()
        );

        let err = extract_payload(Path::new(r"Z:\definitely\missing.nexusbackup"))
            .expect_err("unreadable path must error");
        assert!(err.to_string().contains("cannot read backup"), "got: {err}");
    }
}
