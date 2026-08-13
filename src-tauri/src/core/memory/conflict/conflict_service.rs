//! Conflict service — business logic of the Memory Conflict Engine.
//!
//! Orchestrates the three repositories (memory, conflict groups, audit) around
//! the pure Current Truth Engine (`truth` + `engine` modules):
//!
//! - `create_conflict_group` — open a group around contradicting memories.
//! - `get_conflicts` / `get_conflict` — query open/resolved groups.
//! - `get_conflict_truth` — run the engine over a group's members without
//!   persisting anything: winner + confidence + human-readable reasons.
//! - `resolve_conflict` — settle a group: the winner becomes `Current`
//!   (engine) or `UserConfirmed` (human), every loser becomes `Superseded`
//!   with `superseded_by_id` pointing at the winner, the group is marked
//!   `Resolved` with the full resolution, and each loser gets a `Superseded`
//!   audit event (`detail="conflict resolved"`).
//! - `sync_conflict_groups` — reconcile: group all `Conflicted` records into
//!   open groups (reusing existing open groups, never duplicating them). This
//!   is the integration point with `detect_and_mark_conflicts`.

use std::sync::Arc;

use chrono::Utc;

use crate::core::audit::{AuditEvent, AuditEventType, AuditRepository};
use crate::core::entity_id::EntityId;
use crate::core::memory::conflict::engine::determine_truth;
use crate::core::memory::conflict::truth::TruthContext;
use crate::core::memory::conflict::{
    ConflictGroup, ConflictRepository, ConflictResolution, ConflictStatus, TruthVerdict,
};
use crate::core::memory::memory_lifecycle::{CONFLICT_SIMILARITY, text_overlap};
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::MemoryState;
use crate::core::result::{AppError, Result};

/// Business-logic service for conflict groups and their resolution.
pub struct ConflictService {
    conflict_repo: Arc<dyn ConflictRepository>,
    memory_repo: Arc<dyn MemoryRepository>,
    audit_repo: Arc<dyn AuditRepository>,
}

impl ConflictService {
    pub fn new(
        conflict_repo: Arc<dyn ConflictRepository>,
        memory_repo: Arc<dyn MemoryRepository>,
        audit_repo: Arc<dyn AuditRepository>,
    ) -> Self {
        Self {
            conflict_repo,
            memory_repo,
            audit_repo,
        }
    }

    /// Open a new conflict group around the given members. The members are
    /// expected to be already marked `Conflicted` by the detector.
    pub async fn create_conflict_group(
        &self,
        topic: &str,
        member_ids: Vec<EntityId>,
    ) -> Result<EntityId> {
        if member_ids.len() < 2 {
            return Err(AppError::Validation(
                "A conflict group needs at least two members".to_string(),
            ));
        }
        let group = ConflictGroup::new(topic.to_string(), member_ids);
        self.conflict_repo.save_group(&group).await
    }

    /// All conflict groups, newest first, optionally filtered by status.
    pub async fn get_conflicts(
        &self,
        status: Option<ConflictStatus>,
    ) -> Result<Vec<ConflictGroup>> {
        self.conflict_repo.list_groups(status).await
    }

    /// One conflict group by id.
    pub async fn get_conflict(&self, id: &EntityId) -> Result<ConflictGroup> {
        self.conflict_repo
            .get_group(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Conflict group {} not found", id)))
    }

    /// Open (unresolved) groups that contain the given memory — used by the
    /// context builder to exclude unresolved contradictions from context.
    pub async fn open_groups_containing(&self, memory_id: &EntityId) -> Result<Vec<ConflictGroup>> {
        self.conflict_repo.open_groups_containing(memory_id).await
    }

    /// Run the Current Truth Engine over the group's members. Read-only: the
    /// verdict is computed from the current records but nothing is persisted.
    pub async fn get_conflict_truth(&self, id: &EntityId) -> Result<TruthVerdict> {
        let group = self.get_conflict(id).await?;
        let members = self.load_members(&group).await?;
        determine_truth(&members, &TruthContext::now()).ok_or_else(|| {
            AppError::NotFound(format!("Conflict group {} has no readable members", id))
        })
    }

    /// Settle a conflict.
    ///
    /// * `by` — `"user"` or `"engine"`. A human choice promotes the winner to
    ///   `UserConfirmed` (strongest state); an engine choice keeps it
    ///   `Current`.
    /// * Every loser becomes `Superseded` with `superseded_by_id` = winner.
    /// * The group becomes `Resolved` with the full resolution (winner,
    ///   confidence, reasons, who decided and when).
    /// * Each loser gets a `Superseded` audit event
    ///   (`detail="conflict resolved"`, `related_memory_id` = winner).
    ///
    /// Returns the stored resolution.
    pub async fn resolve_conflict(
        &self,
        id: &EntityId,
        winner_id: &EntityId,
        by: &str,
        reason: Option<&str>,
    ) -> Result<ConflictResolution> {
        if by != "user" && by != "engine" {
            return Err(AppError::Validation(format!(
                "resolve_conflict by must be 'user' or 'engine', got '{}'",
                by
            )));
        }

        let mut group = self.get_conflict(id).await?;
        if group.status == ConflictStatus::Resolved {
            return Err(AppError::Validation(format!(
                "Conflict group {} is already resolved",
                id
            )));
        }
        if !group.contains(winner_id) {
            return Err(AppError::Validation(format!(
                "Winner {} is not a member of conflict group {}",
                winner_id, id
            )));
        }

        let members = self.load_members(&group).await?;
        let now = Utc::now();

        // Re-run the engine to record how plausible the winner was at the
        // moment of resolution. If the human overrode the engine, confidence
        // reflects the override honestly (0.5 = no engine support).
        let engine_verdict = determine_truth(&members, &TruthContext { now });
        let (confidence, mut reasons) = match &engine_verdict {
            Some(v) if v.winner_id == *winner_id => (v.confidence, v.reasons.clone()),
            _ => (
                0.5,
                vec![
                    reason
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "user override — no engine support".to_string()),
                ],
            ),
        };
        if let Some(r) = reason {
            reasons.push(r.to_string());
        }

        let mut winner_record: Option<MemoryRecord> = None;
        for member in &members {
            if member.id == *winner_id {
                let mut w = member.clone();
                if by == "user" {
                    w.memory_state = MemoryState::UserConfirmed;
                    w.confirmed_by = Some(by.to_string());
                    w.confirmed_at = Some(now);
                } else {
                    w.memory_state = MemoryState::Current;
                }
                w.touch();
                self.memory_repo.update(&w).await?;
                winner_record = Some(w);
                continue;
            }
            // Loser: superseded, pointing at the winner.
            let mut l = member.clone();
            l.memory_state = MemoryState::Superseded;
            l.superseded_by_id = Some(winner_id.as_str().to_string());
            l.touch();
            self.memory_repo.update(&l).await?;
        }
        if winner_record.is_none() {
            return Err(AppError::NotFound(format!(
                "Winner memory {} not found",
                winner_id
            )));
        }

        let resolution = ConflictResolution {
            winner_id: winner_id.clone(),
            confidence,
            reasons,
            by: by.to_string(),
            at: now,
        };
        group.status = ConflictStatus::Resolved;
        group.resolved_at = Some(now);
        group.resolution = Some(resolution.clone());
        self.conflict_repo.update_group(&group).await?;

        // Audit: each loser records the supersession caused by the resolution.
        for member in &members {
            if member.id == *winner_id {
                continue;
            }
            self.audit_repo
                .add_event(&AuditEvent::new(
                    member.id.clone(),
                    AuditEventType::Superseded,
                    by.to_string(),
                    Some("conflict resolved".to_string()),
                    Some(winner_id.as_str().to_string()),
                ))
                .await?;
        }

        Ok(resolution)
    }

    /// Reconcile the group table with reality: every record currently marked
    /// `Conflicted` must belong to exactly one open group.
    ///
    /// Records are clustered by pairwise text overlap (`CONFLICT_SIMILARITY`).
    /// Existing open groups are reused (members merged), never duplicated.
    /// Returns how many groups were created or updated.
    ///
    /// This is the integration point for `detect_and_mark_conflicts`: after
    /// the detector flags both sides of a contradiction, calling this turns the
    /// flags into a resolvable group.
    pub async fn sync_conflict_groups(&self) -> Result<usize> {
        let records = self.memory_repo.list(100_000, 0).await?;
        let conflicted: Vec<MemoryRecord> = records
            .into_iter()
            .filter(|r| r.memory_state == MemoryState::Conflicted)
            .collect();
        if conflicted.is_empty() {
            return Ok(0);
        }

        // Union-find: two conflicted records belong together when they share
        // the same normalized topic (title) OR overlap on the same statement
        // (the threshold the detector used to flag them). Title equality keeps
        // "Use SQL Server" vs "Use PostgreSQL" in one group even when the word
        // overlap dips below the flagging threshold on longer sentences.
        let n = conflicted.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut [usize], mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
                x = parent[x];
            }
            x
        }
        let normalize_title = |t: &str| -> String { t.trim().to_lowercase() };
        for i in 0..n {
            let ai = format!("{} {}", conflicted[i].title, conflicted[i].content);
            for j in (i + 1)..n {
                let same_topic =
                    normalize_title(&conflicted[i].title) == normalize_title(&conflicted[j].title);
                if same_topic {
                    let ri = find(&mut parent, i);
                    let rj = find(&mut parent, j);
                    if ri != rj {
                        parent[ri] = rj;
                    }
                    continue;
                }
                let aj = format!("{} {}", conflicted[j].title, conflicted[j].content);
                if text_overlap(&ai, &aj) >= CONFLICT_SIMILARITY {
                    let ri = find(&mut parent, i);
                    let rj = find(&mut parent, j);
                    if ri != rj {
                        parent[ri] = rj;
                    }
                }
            }
        }

        // Existing open groups: reuse by membership.
        let open = self
            .conflict_repo
            .list_groups(Some(ConflictStatus::Open))
            .await?;

        // Cluster conflicted records by their union-find root.
        let mut clusters: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            clusters.entry(root).or_default().push(i);
        }

        let mut touched = 0usize;
        for indices in clusters.values() {
            let mut members: Vec<MemoryRecord> =
                indices.iter().map(|&i| conflicted[i].clone()).collect();
            members.sort_by_key(|a| a.created_at);

            // Reuse an existing open group that already contains any member.
            let reusable = open.iter().find(|g| {
                members
                    .iter()
                    .any(|m| g.member_ids.iter().any(|id| id == &m.id))
            });

            match reusable {
                Some(existing) => {
                    let mut updated = existing.clone();
                    let before = updated.member_ids.len();
                    for m in &members {
                        if !updated.contains(&m.id) {
                            updated.member_ids.push(m.id.clone());
                        }
                    }
                    if updated.member_ids.len() != before {
                        self.conflict_repo.update_group(&updated).await?;
                        touched += 1;
                    }
                }
                None => {
                    let group = ConflictGroup::new(
                        members[0].title.clone(),
                        members.iter().map(|m| m.id.clone()).collect(),
                    );
                    self.conflict_repo.save_group(&group).await?;
                    touched += 1;
                }
            }
        }
        Ok(touched)
    }

    /// Load the member records of a group, skipping any that were deleted.
    async fn load_members(&self, group: &ConflictGroup) -> Result<Vec<MemoryRecord>> {
        let mut members = Vec::new();
        for id in &group.member_ids {
            if let Some(record) = self.memory_repo.get_by_id(id).await? {
                members.push(record);
            }
        }
        Ok(members)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::types::MemorySource;
    use rusqlite::Connection;
    use std::sync::Arc;

    /// Shared-file test harness: three repositories over ONE temp database, so
    /// the service integration (memory + conflict + audit) works end-to-end.
    struct TestEnv {
        service: ConflictService,
        memory: Arc<crate::storage::sqlite::SqliteMemoryRepository>,
        db_path: std::path::PathBuf,
    }

    impl TestEnv {
        fn new() -> Self {
            // UUID v4, not pid+nanos: parallel tests on Windows can hit the
            // same clock tick (~100ns resolution) and then share one temp
            // file, racing each other's migrations ("duplicate column name").
            let db_path = std::env::temp_dir().join(format!(
                "nexus_conflict_test_{}.db",
                crate::core::entity_id::EntityId::new()
            ));

            // Each repository owns its own connection to the same file. Without
            // a busy timeout a concurrent writer fails fast with SQLITE_BUSY —
            // mirror the production `db::configure` behavior.
            let open_with_timeout = |path: &std::path::Path| -> Connection {
                let conn = Connection::open(path).unwrap();
                conn.busy_timeout(std::time::Duration::from_millis(5_000))
                    .unwrap();
                conn
            };

            let memory_conn = open_with_timeout(&db_path);
            let memory =
                Arc::new(crate::storage::sqlite::SqliteMemoryRepository::new(memory_conn).unwrap());

            let conflict_conn = open_with_timeout(&db_path);
            let conflict: Arc<dyn ConflictRepository> = Arc::new(
                crate::storage::sqlite::SqliteConflictRepository::new(conflict_conn).unwrap(),
            );

            let audit_conn = open_with_timeout(&db_path);
            let audit: Arc<dyn AuditRepository> =
                Arc::new(crate::storage::sqlite::SqliteAuditRepository::new(audit_conn).unwrap());

            let service = ConflictService::new(conflict, memory.clone(), audit);
            TestEnv {
                service,
                memory,
                db_path,
            }
        }

        fn save(&self, title: &str, content: &str) -> MemoryRecord {
            let rec = MemoryRecord::new(
                title.to_string(),
                content.to_string(),
                "test".to_string(),
                MemorySource::Manual,
            )
            .unwrap();
            let id = futures::executor::block_on(self.memory.save(&rec)).unwrap();
            let mut saved = rec;
            saved.id = id;
            saved
        }

        fn mark_conflicted(&self, record: &MemoryRecord) {
            let mut r = record.clone();
            r.memory_state = MemoryState::Conflicted;
            futures::executor::block_on(self.memory.update(&r)).unwrap();
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.db_path);
            let wal = format!("{}-wal", self.db_path.display());
            let shm = format!("{}-shm", self.db_path.display());
            let _ = std::fs::remove_file(wal);
            let _ = std::fs::remove_file(shm);
        }
    }

    #[tokio::test]
    async fn create_group_requires_two_members() {
        let env = TestEnv::new();
        let one = vec![EntityId::new()];
        assert!(env.service.create_conflict_group("db", one).await.is_err());
        let two = vec![EntityId::new(), EntityId::new()];
        assert!(env.service.create_conflict_group("db", two).await.is_ok());
    }

    #[tokio::test]
    async fn get_conflicts_lists_and_filters() {
        let env = TestEnv::new();
        let m1 = env.save("Database", "Use PostgreSQL");
        let m2 = env.save("Database", "Use MySQL");
        let id = env
            .service
            .create_conflict_group("Database", vec![m1.id.clone(), m2.id.clone()])
            .await
            .unwrap();

        let all = env.service.get_conflicts(None).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);

        let open = env
            .service
            .get_conflicts(Some(ConflictStatus::Open))
            .await
            .unwrap();
        assert_eq!(open.len(), 1);
        let resolved = env
            .service
            .get_conflicts(Some(ConflictStatus::Resolved))
            .await
            .unwrap();
        assert!(resolved.is_empty());
    }

    #[tokio::test]
    async fn get_conflict_missing_is_not_found() {
        let env = TestEnv::new();
        let result = env.service.get_conflict(&EntityId::new()).await;
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn truth_verdict_picks_recent_winner() {
        let env = TestEnv::new();
        let mut old = env.save("Database", "Use PostgreSQL");
        // Age the first record: freshness is measured in seconds, so two
        // records created back-to-back would otherwise tie and fall to the id
        // tie-breaker instead of the freshness signal.
        old.updated_at = chrono::Utc::now() - chrono::Duration::days(30);
        env.memory.update(&old).await.unwrap();
        env.mark_conflicted(&old);
        let new = env.save("Database", "Use MySQL");
        env.mark_conflicted(&new);
        let id = env
            .service
            .create_conflict_group("Database", vec![old.id.clone(), new.id.clone()])
            .await
            .unwrap();

        let verdict = env.service.get_conflict_truth(&id).await.unwrap();
        assert_eq!(verdict.winner_id, new.id);
        assert!(!verdict.reasons.is_empty());
    }

    #[tokio::test]
    async fn resolve_user_choice_confirms_winner_and_supersedes_loser() {
        let env = TestEnv::new();
        let old = env.save("Database", "Use PostgreSQL");
        env.mark_conflicted(&old);
        let new = env.save("Database", "Use MySQL");
        env.mark_conflicted(&new);
        let id = env
            .service
            .create_conflict_group("Database", vec![old.id.clone(), new.id.clone()])
            .await
            .unwrap();

        let resolution = env
            .service
            .resolve_conflict(&id, &old.id, "user", Some("chose PG"))
            .await
            .unwrap();
        assert_eq!(resolution.winner_id, old.id);
        assert_eq!(resolution.by, "user");

        let winner = env.memory.get_by_id(&old.id).await.unwrap().unwrap();
        assert_eq!(winner.memory_state, MemoryState::UserConfirmed);
        assert_eq!(winner.confirmed_by.as_deref(), Some("user"));
        assert!(winner.confirmed_at.is_some());

        let loser = env.memory.get_by_id(&new.id).await.unwrap().unwrap();
        assert_eq!(loser.memory_state, MemoryState::Superseded);
        assert_eq!(loser.superseded_by_id.as_deref(), Some(old.id.as_str()));

        let group = env.service.get_conflict(&id).await.unwrap();
        assert_eq!(group.status, ConflictStatus::Resolved);
        assert!(group.resolved_at.is_some());
        assert_eq!(group.resolution.as_ref().unwrap().winner_id, old.id);
    }

    #[tokio::test]
    async fn resolve_engine_choice_keeps_winner_current() {
        let env = TestEnv::new();
        let old = env.save("Database", "Use PostgreSQL");
        env.mark_conflicted(&old);
        let new = env.save("Database", "Use MySQL");
        env.mark_conflicted(&new);
        let id = env
            .service
            .create_conflict_group("Database", vec![old.id.clone(), new.id.clone()])
            .await
            .unwrap();

        let resolution = env
            .service
            .resolve_conflict(&id, &new.id, "engine", None)
            .await
            .unwrap();
        assert_eq!(resolution.by, "engine");

        let winner = env.memory.get_by_id(&new.id).await.unwrap().unwrap();
        assert_eq!(winner.memory_state, MemoryState::Current);
        assert!(winner.confirmed_by.is_none());

        let loser = env.memory.get_by_id(&old.id).await.unwrap().unwrap();
        assert_eq!(loser.memory_state, MemoryState::Superseded);
        assert_eq!(loser.superseded_by_id.as_deref(), Some(new.id.as_str()));
    }

    #[tokio::test]
    async fn resolve_writes_audit_superseded_events() {
        let env = TestEnv::new();
        let old = env.save("Database", "Use PostgreSQL");
        env.mark_conflicted(&old);
        let new = env.save("Database", "Use MySQL");
        env.mark_conflicted(&new);
        let id = env
            .service
            .create_conflict_group("Database", vec![old.id.clone(), new.id.clone()])
            .await
            .unwrap();

        env.service
            .resolve_conflict(&id, &new.id, "user", None)
            .await
            .unwrap();

        let audit_conn = Connection::open(&env.db_path).unwrap();
        let audit = crate::storage::sqlite::SqliteAuditRepository::new(audit_conn).unwrap();
        let events = audit.list_events(&old.id).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AuditEventType::Superseded);
        assert_eq!(events[0].detail.as_deref(), Some("conflict resolved"));
        assert_eq!(
            events[0].related_memory_id.as_deref(),
            Some(new.id.as_str())
        );
    }

    #[tokio::test]
    async fn resolve_twice_fails() {
        let env = TestEnv::new();
        let m1 = env.save("Database", "Use PostgreSQL");
        env.mark_conflicted(&m1);
        let m2 = env.save("Database", "Use MySQL");
        env.mark_conflicted(&m2);
        let id = env
            .service
            .create_conflict_group("Database", vec![m1.id.clone(), m2.id.clone()])
            .await
            .unwrap();

        env.service
            .resolve_conflict(&id, &m1.id, "user", None)
            .await
            .unwrap();
        let second = env
            .service
            .resolve_conflict(&id, &m1.id, "user", None)
            .await;
        assert!(second.is_err());
    }

    #[tokio::test]
    async fn resolve_winner_must_be_member() {
        let env = TestEnv::new();
        let m1 = env.save("Database", "Use PostgreSQL");
        env.mark_conflicted(&m1);
        let m2 = env.save("Database", "Use MySQL");
        env.mark_conflicted(&m2);
        let id = env
            .service
            .create_conflict_group("Database", vec![m1.id.clone(), m2.id.clone()])
            .await
            .unwrap();

        let stranger = EntityId::new();
        let result = env
            .service
            .resolve_conflict(&id, &stranger, "user", None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn sync_creates_group_for_conflicted_records() {
        let env = TestEnv::new();
        let m1 = env.save("Database", "Use PostgreSQL as the primary database");
        env.mark_conflicted(&m1);
        let m2 = env.save("Database", "Use MySQL as the primary database");
        env.mark_conflicted(&m2);

        let touched = env.service.sync_conflict_groups().await.unwrap();
        assert_eq!(touched, 1);

        let groups = env.service.get_conflicts(None).await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].member_ids.len(), 2);
        assert_eq!(groups[0].status, ConflictStatus::Open);
    }

    #[tokio::test]
    async fn sync_reuses_open_group_instead_of_duplicating() {
        let env = TestEnv::new();
        let m1 = env.save("Database", "Use PostgreSQL as the primary database");
        env.mark_conflicted(&m1);
        let m2 = env.save("Database", "Use MySQL as the primary database");
        env.mark_conflicted(&m2);

        env.service.sync_conflict_groups().await.unwrap();
        // A second pass must NOT create another group for the same records.
        let touched = env.service.sync_conflict_groups().await.unwrap();
        assert_eq!(touched, 0);

        let groups = env.service.get_conflicts(None).await.unwrap();
        assert_eq!(groups.len(), 1);
    }

    #[tokio::test]
    async fn sync_merges_new_member_into_existing_group() {
        let env = TestEnv::new();
        let m1 = env.save("Database", "Use PostgreSQL as the primary database");
        env.mark_conflicted(&m1);
        let m2 = env.save("Database", "Use MySQL as the primary database");
        env.mark_conflicted(&m2);
        env.service.sync_conflict_groups().await.unwrap();

        // A third contradicting record joins the same topic later.
        let m3 = env.save("Database", "Use SQL Server as the primary database");
        env.mark_conflicted(&m3);
        let touched = env.service.sync_conflict_groups().await.unwrap();
        assert_eq!(touched, 1);

        let groups = env.service.get_conflicts(None).await.unwrap();
        assert_eq!(groups.len(), 1, "must stay a single group");
        assert_eq!(groups[0].member_ids.len(), 3);
    }

    #[tokio::test]
    async fn sync_separates_unrelated_conflicts() {
        let env = TestEnv::new();
        let db1 = env.save("Database", "Use PostgreSQL as the primary database");
        env.mark_conflicted(&db1);
        let db2 = env.save("Database", "Use MySQL as the primary database");
        env.mark_conflicted(&db2);
        let port1 = env.save("Port", "Run the API on port 8080");
        env.mark_conflicted(&port1);
        let port2 = env.save("Port", "Run the API on port 9090");
        env.mark_conflicted(&port2);

        env.service.sync_conflict_groups().await.unwrap();

        let groups = env.service.get_conflicts(None).await.unwrap();
        assert_eq!(groups.len(), 2, "two unrelated topics -> two groups");
    }

    #[tokio::test]
    async fn sync_ignores_non_conflicted() {
        let env = TestEnv::new();
        let m1 = env.save("Database", "Use PostgreSQL as the primary database");
        // Not marked Conflicted -> must not create a group.
        env.save("Port", "Run the API on port 8080");

        let touched = env.service.sync_conflict_groups().await.unwrap();
        assert_eq!(touched, 0);
        assert_eq!(m1.title, "Database");
    }
}
