//! Versioned, portable project export/import (plan 9.2).
//!
//! `ProjectExport` is a self-describing JSON snapshot of everything Nexus
//! knows about a project: the decision journal, the knowledge graph, skills,
//! versioning provenance and context snapshots. The format is versioned
//! (`EXPORT_FORMAT_VERSION`) and round-trips exactly: importing an export into
//! an empty database preserves every ID and timestamp, so
//! `export(import(export(db))) == export(db)` (verified by the roundtrip test).
//!
//! This is a *portable project format* — distinct from `core/backup.rs`, which
//! copies the raw SQLite file. A backup restores bit-for-bit; an export is a
//! humans-readable artifact that can move between installations, be diffed and
//! be consumed by tooling.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::core::audit::{AuditEvent, AuditRepository};
use crate::core::context::{ContextSnapshot, ContextStore};
use crate::core::graph::entity::Entity;
use crate::core::graph::graph_store::GraphStore;
use crate::core::graph::relationship::Relationship;
use crate::core::knowledge::skills::{Skill, SkillRepository};
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::result::{AppError, Result};
use crate::core::versioning::automatic_commit::AutomaticCommit;
use crate::core::versioning::causality_record::CausalityRecord;
use crate::core::versioning::version_edge::VersionEdge;
use crate::storage::sqlite::{
    SqliteAuditRepository, SqliteContextRepository, SqliteGraphRepository, SqliteMemoryRepository,
    SqliteVersioningRepository,
};

/// Current format version. Bump whenever the JSON shape changes in a way that
/// breaks older readers; import rejects anything that does not match.
pub const EXPORT_FORMAT_VERSION: u32 = 1;

/// Header identifying the export artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub format_version: u32,
    pub app_version: String,
    /// RFC3339 UTC timestamp of when the export was produced.
    pub exported_at: String,
}

/// The complete portable snapshot of a Nexus project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectExport {
    pub manifest: ExportManifest,
    /// Decision-journal entries (append-only provenance of *why* things are
    /// the way they are).
    pub decisions: Vec<AuditEvent>,
    /// Remembered facts with their trust lifecycle state.
    pub memories: Vec<MemoryRecord>,
    /// Knowledge-graph nodes.
    pub entities: Vec<Entity>,
    /// Knowledge-graph edges.
    pub relationships: Vec<Relationship>,
    /// Runnable skills (commands agents can invoke).
    pub skills: Vec<Skill>,
    /// Versioning provenance: automatic commits (the change history).
    pub commits: Vec<AutomaticCommit>,
    /// Causality records: which version was caused by what decision.
    pub causality: Vec<CausalityRecord>,
    /// Edges of the version graph.
    pub version_edges: Vec<VersionEdge>,
    /// Persisted context packages (point-in-time captures of a working set).
    pub snapshots: Vec<ContextSnapshot>,
}

impl ProjectExport {
    /// Build the manifest for "now".
    pub fn manifest_for_now() -> ExportManifest {
        ExportManifest {
            format_version: EXPORT_FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Serialize the export to pretty JSON.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AppError::Serialization(format!("export: {e}")))
    }

    /// Parse an export from JSON, validating the format version.
    pub fn from_json(json: &str) -> Result<Self> {
        let export: ProjectExport = serde_json::from_str(json)
            .map_err(|e| AppError::Serialization(format!("import: {e}")))?;
        if export.manifest.format_version != EXPORT_FORMAT_VERSION {
            return Err(AppError::Validation(format!(
                "Unsupported export format: got {}, this build supports {}",
                export.manifest.format_version, EXPORT_FORMAT_VERSION
            )));
        }
        Ok(export)
    }

    /// Number of records in each section (for the import report / UI).
    pub fn counts(&self) -> ExportCounts {
        ExportCounts {
            decisions: self.decisions.len(),
            memories: self.memories.len(),
            entities: self.entities.len(),
            relationships: self.relationships.len(),
            skills: self.skills.len(),
            commits: self.commits.len(),
            causality: self.causality.len(),
            version_edges: self.version_edges.len(),
            snapshots: self.snapshots.len(),
        }
    }
}

/// Per-section record counts.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ExportCounts {
    pub decisions: usize,
    pub memories: usize,
    pub entities: usize,
    pub relationships: usize,
    pub skills: usize,
    pub commits: usize,
    pub causality: usize,
    pub version_edges: usize,
    pub snapshots: usize,
}

/// Result of a successful import: how many records were written per section.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub counts: ExportCounts,
}

// ── Repo plumbing ────────────────────────────────────────────────────
//
// Each repository owns its own connection to the same database file (the app
// does the same: graph, memory, snapshots, versioning all open independent
// connections). `export_project_at`/`import_project_at` take an explicit db
// path so tests can run against throwaway files without touching global state.

fn open_conn(db: &Path) -> Result<rusqlite::Connection> {
    crate::db::open_connection_at(db).map_err(AppError::Database)
}

fn open_mem(db: &Path) -> Result<SqliteMemoryRepository> {
    SqliteMemoryRepository::new(open_conn(db)?)
}

fn open_graph(db: &Path) -> Result<SqliteGraphRepository> {
    SqliteGraphRepository::new(open_conn(db)?)
}

fn open_audit(db: &Path) -> Result<SqliteAuditRepository> {
    SqliteAuditRepository::new(open_conn(db)?)
}

fn open_skills(db: &Path) -> Result<SkillRepository> {
    SkillRepository::new(open_conn(db)?)
}

fn open_versioning(db: &Path) -> Result<SqliteVersioningRepository> {
    SqliteVersioningRepository::new(open_conn(db)?)
}

fn open_context(db: &Path) -> Result<SqliteContextRepository> {
    SqliteContextRepository::new(open_conn(db)?)
}

// ── Export ───────────────────────────────────────────────────────────

/// Read every section of the project into a portable [`ProjectExport`].
pub async fn export_project_at(db: &Path) -> Result<ProjectExport> {
    let mem = open_mem(db)?;
    let graph = open_graph(db)?;
    let audit = open_audit(db)?;
    let skills = open_skills(db)?;
    let versioning = open_versioning(db)?;
    let context = open_context(db)?;

    let memory_count = mem.count().await?;
    let memories = mem.list(memory_count as u32, 0).await?;
    let mut decisions = audit.list_all_events().await?;
    decisions.sort_by_key(|a| a.created_at);
    let entities = graph.list_all_entities().await?;
    let mut relationships = graph.list_all_relationships().await?;
    relationships.sort_by_key(|a| a.created_at);
    let mut skills = skills.list()?;
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    let commits = versioning.list_all_commits()?;
    let causality = versioning.list_all_causality()?;
    let version_edges = versioning.list_all_version_edges()?;
    let snapshots = context.list_all_snapshots().await?;

    Ok(ProjectExport {
        manifest: ProjectExport::manifest_for_now(),
        decisions,
        memories,
        entities,
        relationships,
        skills,
        commits,
        causality,
        version_edges,
        snapshots,
    })
}

/// Export the live database (same as [`export_project_at`] with the app path).
pub async fn export_project() -> Result<ProjectExport> {
    export_project_at(&crate::db::db_path()).await
}

// ── Import ───────────────────────────────────────────────────────────

/// Write an export into the database at `db`, preserving every ID and
/// timestamp. Insert order respects foreign keys (entities before the
/// relationships that point at them). Import is intended for an empty/fresh
/// database — re-importing into a non-empty one fails on ID conflicts.
pub async fn import_project_at(db: &Path, export: &ProjectExport) -> Result<ImportReport> {
    if export.manifest.format_version != EXPORT_FORMAT_VERSION {
        return Err(AppError::Validation(format!(
            "Unsupported export format: got {}, this build supports {}",
            export.manifest.format_version, EXPORT_FORMAT_VERSION
        )));
    }

    let mem = open_mem(db)?;
    let graph = open_graph(db)?;
    let audit = open_audit(db)?;
    let skills = open_skills(db)?;
    let versioning = open_versioning(db)?;
    let context = open_context(db)?;

    // Memories first: audit events reference memory IDs (no FK enforced, but
    // keep the journal consistent with the facts it explains).
    for memory in &export.memories {
        mem.save(memory).await?;
    }
    // Entities before relationships: graph_relationships has an FK to
    // graph_entities (ON DELETE CASCADE), so edges cannot exist first.
    for entity in &export.entities {
        graph.add_entity(entity).await?;
    }
    for relationship in &export.relationships {
        graph.add_relationship(relationship).await?;
    }
    for event in &export.decisions {
        audit.add_event(event).await?;
    }
    for skill in &export.skills {
        skills.insert_with_id(skill)?;
    }
    // Provenance after entities/memories so version chains reference rows that
    // actually exist (no FK enforced, but keeps the data coherent).
    for commit in &export.commits {
        versioning.insert_commit(commit)?;
    }
    for record in &export.causality {
        versioning.insert_causality(record)?;
    }
    for edge in &export.version_edges {
        versioning.insert_version_edge(edge)?;
    }
    for snapshot in &export.snapshots {
        context.save_snapshot(snapshot).await?;
    }

    Ok(ImportReport {
        counts: export.counts(),
    })
}

/// Import into the live database (same as [`import_project_at`] with the app
/// path).
pub async fn import_project(export: &ProjectExport) -> Result<ImportReport> {
    import_project_at(&crate::db::db_path(), export).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::context_package::{ContextPackage, UserIntent};
    use crate::core::context::context_snapshot::ContextSnapshot;
    use crate::core::graph::entity::Entity;
    use crate::core::graph::entity_types::EntityType;
    use crate::core::graph::relationship::Relationship;
    use crate::core::graph::relationship_types::RelationshipType;
    use crate::core::memory::types::MemorySource;
    use crate::core::versioning::automatic_commit::ChangeType;
    use crate::core::versioning::version_edge::VersionEdgeType;
    use crate::storage::sqlite::schema;

    /// Create a throwaway SQLite file, migrate it and return its path —
    /// same pattern as the backup tests (no global env is touched).
    fn fresh_db(prefix: &str) -> std::path::PathBuf {
        let tmp = std::env::temp_dir().join(format!("nexus-{prefix}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let db = tmp.join("nexus.db");
        let conn = crate::db::open_connection_at(&db).expect("open test db");
        schema::apply_migrations(&conn).expect("migrate test db");
        drop(conn);
        db
    }

    /// Seed one of each section into the db at `path`.
    async fn seed(path: &std::path::Path) {
        let mem = open_mem(path).unwrap();
        let graph = open_graph(path).unwrap();
        let audit = open_audit(path).unwrap();
        let skills = open_skills(path).unwrap();
        let versioning = open_versioning(path).unwrap();
        let context = open_context(path).unwrap();

        // Memory + its decision-journal entry.
        let memory = MemoryRecord::new(
            "Export/Import decision".to_string(),
            "We chose a versioned JSON export so artifacts survive tool changes.".to_string(),
            "alice".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        mem.save(&memory).await.unwrap();
        audit
            .add_event(&AuditEvent::new(
                memory.id.clone(),
                crate::core::audit::AuditEventType::Created,
                "alice".to_string(),
                Some("decided".to_string()),
                None,
            ))
            .await
            .unwrap();

        // Graph: two entities + one relationship between them.
        let e1 = Entity::new(
            EntityType::Project,
            "nexus".to_string(),
            "the app".to_string(),
        );
        let e2 = Entity::new(
            EntityType::Technology,
            "rust".to_string(),
            "implementation".to_string(),
        );
        graph.add_entity(&e1).await.unwrap();
        graph.add_entity(&e2).await.unwrap();
        let rel =
            Relationship::new(e1.id.clone(), e2.id.clone(), RelationshipType::Uses, 0.9).unwrap();
        graph.add_relationship(&rel).await.unwrap();

        // Skill.
        skills
            .upsert("echo-skill", "Echo args", "echo", "scripts/skills/echo.js")
            .unwrap();

        // Versioning provenance: one commit, one causality record, one edge.
        let commit = AutomaticCommit {
            id: "commit-1".to_string(),
            hash: "abc123".to_string(),
            version_number: 1,
            entity_type: "MemoryRecord".to_string(),
            entity_id: memory.id.clone(),
            change_type: ChangeType::Created,
            diff: Some(r#"{"title":"Export/Import decision"}"#.to_string()),
            baseline_snapshot_id: None,
            is_baseline: true,
            created_at: chrono::Utc::now(),
            created_by: "system".to_string(),
            triggering_event_type: "EntityCreated".to_string(),
            triggering_event_id: "evt-1".to_string(),
            change_reason: Some("seeded by test".to_string()),
            linked_entity_ids: vec![],
            linked_decision_ids: vec![],
            is_indexed: false,
            is_archived: false,
            size_bytes: 64,
        };
        versioning.insert_commit(&commit).unwrap();
        versioning
            .insert_causality(&CausalityRecord::new(
                memory.id.clone(),
                "commit-1".to_string(),
                "seeded".to_string(),
                vec!["entity-1".to_string()],
            ))
            .unwrap();
        versioning
            .insert_version_edge(&VersionEdge::new(
                "commit-1".to_string(),
                "commit-2".to_string(),
                VersionEdgeType::EvolvedTo,
            ))
            .unwrap();

        // Context snapshot.
        let pkg = ContextPackage::new(UserIntent {
            query: "what is nexus?".to_string(),
            intent_type: crate::core::context::context_package::IntentType::Search,
            confidence: 0.9,
            keywords: vec!["nexus".to_string()],
            temporal: None,
        });
        let snap = ContextSnapshot::new(e1.id.clone(), pkg, Some("seed".to_string()));
        context.save_snapshot(&snap).await.unwrap();
    }

    fn section_json(export: &ProjectExport) -> serde_json::Value {
        serde_json::to_value(export).unwrap()
    }

    /// The round-trip guarantee: exporting, importing into a fresh database
    /// and exporting again must reproduce every section byte-for-byte (the
    /// manifest's `exported_at` is the only thing that legitimately differs).
    #[tokio::test]
    async fn roundtrip_preserves_all_sections() {
        let src = fresh_db("export-roundtrip-src");
        seed(&src).await;

        let export = export_project_at(&src).await.expect("export source");
        assert!(export.counts().memories == 1);
        assert!(export.counts().entities == 2);
        assert!(export.counts().relationships == 1);
        assert!(export.counts().decisions == 1);
        assert!(export.counts().skills == 1);
        assert!(export.counts().commits == 1);
        assert!(export.counts().causality == 1);
        assert!(export.counts().version_edges == 1);
        assert!(export.counts().snapshots == 1);

        // JSON round-trip: serialize → deserialize → same counts.
        let json = export.to_json().unwrap();
        let parsed = ProjectExport::from_json(&json).expect("parse export");
        assert_eq!(parsed.counts(), export.counts());

        // Import into a fresh database and export again.
        let dst = fresh_db("export-roundtrip-dst");
        let report = import_project_at(&dst, &parsed).await.expect("import");
        assert_eq!(report.counts, export.counts());

        let re_export = export_project_at(&dst).await.expect("export destination");
        let export_value = section_json(&export);
        let re_export_value = section_json(&re_export);

        let sections = [
            "decisions",
            "memories",
            "entities",
            "relationships",
            "skills",
            "commits",
            "causality",
            "version_edges",
            "snapshots",
        ];
        for section in sections {
            assert_eq!(
                export_value[section], re_export_value[section],
                "section '{section}' must round-trip exactly"
            );
        }
        assert_eq!(export_value["manifest"]["format_version"], 1);
        assert_eq!(
            export_value["manifest"]["app_version"],
            re_export_value["manifest"]["app_version"]
        );

        let _ = std::fs::remove_dir_all(src.parent().unwrap());
        let _ = std::fs::remove_dir_all(dst.parent().unwrap());
    }

    /// IDs survive the round-trip: the imported memory keeps its original ID.
    #[tokio::test]
    async fn roundtrip_preserves_ids() {
        let src = fresh_db("export-ids-src");
        seed(&src).await;
        let export = export_project_at(&src).await.expect("export");
        let original_id = export.memories[0].id.clone();

        let dst = fresh_db("export-ids-dst");
        import_project_at(&dst, &export).await.expect("import");
        let re_export = export_project_at(&dst).await.expect("re-export");

        assert_eq!(re_export.memories[0].id, original_id);
        assert_eq!(re_export.skills[0].id, export.skills[0].id);
        assert_eq!(re_export.commits[0].id, "commit-1");
        assert_eq!(re_export.snapshots[0].id, export.snapshots[0].id);
        assert_eq!(re_export.relationships[0].id, export.relationships[0].id);

        let _ = std::fs::remove_dir_all(src.parent().unwrap());
        let _ = std::fs::remove_dir_all(dst.parent().unwrap());
    }

    /// A future/foreign format version is rejected instead of silently
    /// mis-imported.
    #[tokio::test]
    async fn rejects_unknown_format_version() {
        let src = fresh_db("export-ver-src");
        seed(&src).await;
        let mut export = export_project_at(&src).await.expect("export");
        export.manifest.format_version = 999;

        let dst = fresh_db("export-ver-dst");
        let err = import_project_at(&dst, &export)
            .await
            .expect_err("must reject");
        assert!(err.to_string().contains("format"));
        let _ = std::fs::remove_dir_all(src.parent().unwrap());
        let _ = std::fs::remove_dir_all(dst.parent().unwrap());
    }
}
