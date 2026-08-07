//! Project documents — the RAG corpus.
//!
//! A document is a `.md`/`.txt` file from the user's project (or anywhere on
//! disk) imported into the database with its content and a checksum. The
//! checksum makes re-imports idempotent: unchanged files are skipped, changed
//! files are re-stored, and the semantic fingerprint is refreshed.
//!
//! Documents live in their own table (`project_documents`) and their
//! embeddings in `document_fingerprints` — separate from memory fingerprints
//! so vector search can query docs and memories independently.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::core::entity_id::EntityId;
use crate::core::knowledge::content_checksum;
use crate::core::result::{AppError, Result};

/// A project document imported into the knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub id: EntityId,
    pub path: String,
    pub title: String,
    pub content: String,
    pub doc_type: String,
    pub source: String,
    pub checksum: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Outcome of importing a directory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub scanned: u64,
    pub imported: u64,
    pub updated: u64,
    pub unchanged: u64,
    pub failed: u64,
    pub errors: Vec<String>,
}

/// SQLite-backed repository for project documents.
pub struct ProjectDocumentRepository {
    conn: Connection,
}

impl ProjectDocumentRepository {
    pub fn new(conn: Connection) -> Result<Self> {
        crate::storage::sqlite::schema::apply_migrations(&conn)?;
        Ok(Self { conn })
    }

    pub fn open() -> Result<Self> {
        let conn = crate::db::open_connection().map_err(AppError::Database)?;
        Self::new(conn)
    }

    /// Upsert a document by path. Returns `true` when the content changed
    /// (new or updated), `false` when it was already current (no-op).
    pub fn upsert(
        &self,
        path: &str,
        title: &str,
        content: &str,
        doc_type: &str,
        source: &str,
    ) -> Result<bool> {
        let checksum = content_checksum(content);
        let now = chrono::Utc::now().to_rfc3339();

        let existing: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT id, checksum FROM project_documents WHERE path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?;

        match existing {
            Some((_id, old_checksum)) if old_checksum == checksum => Ok(false),
            Some((id, _)) => {
                self.conn
                    .execute(
                        "UPDATE project_documents
                         SET title = ?2, content = ?3, doc_type = ?4, source = ?5,
                             checksum = ?6, updated_at = ?7
                         WHERE id = ?1",
                        params![id, title, content, doc_type, source, checksum, now],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                Ok(true)
            }
            None => {
                let id = EntityId::new();
                self.conn
                    .execute(
                        "INSERT INTO project_documents
                         (id, path, title, content, doc_type, source, checksum, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                        params![
                            id.as_str(),
                            path,
                            title,
                            content,
                            doc_type,
                            source,
                            checksum,
                            now,
                        ],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                Ok(true)
            }
        }
    }

    /// List documents, newest first.
    pub fn list(&self, limit: u32) -> Result<Vec<ProjectDocument>> {
        let limit = limit.min(1000);
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, path, title, content, doc_type, source, checksum, created_at, updated_at
                 FROM project_documents ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([limit], Self::row_to_doc)
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn get(&self, id: &EntityId) -> Result<Option<ProjectDocument>> {
        self.conn
            .query_row(
                "SELECT id, path, title, content, doc_type, source, checksum, created_at, updated_at
                 FROM project_documents WHERE id = ?1",
                params![id.as_str()],
                Self::row_to_doc,
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn count(&self) -> Result<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM project_documents", [], |row| {
                row.get(0)
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(n.max(0) as u64)
    }

    pub fn delete(&self, id: &EntityId) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM project_documents WHERE id = ?1",
                params![id.as_str()],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        // Drop its fingerprint too, so deleted docs stop surfacing in search.
        crate::core::context::indexer::spawn_forget_document(id);
        Ok(())
    }

    /// Paths of all documents — used to detect files removed from disk.
    pub fn all_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM project_documents")
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(out)
    }

    /// Remove every document whose path is no longer in `keep` (used after a
    /// directory re-import to drop stale entries). Returns removed count.
    pub fn prune_not_in(&self, keep: &[String]) -> Result<u64> {
        let keep_set: std::collections::HashSet<String> = keep.iter().cloned().collect();
        let existing = self.all_paths()?;
        let mut removed: u64 = 0;
        for path in existing {
            if !keep_set.contains(&path) {
                let id: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT id FROM project_documents WHERE path = ?1",
                        params![path],
                        |row| row.get(0),
                    )
                    .ok();
                if let Some(id_str) = id
                    && let Ok(id) = EntityId::parse(&id_str)
                {
                    self.delete(&id)?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    fn row_to_doc(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectDocument> {
        let id_str: String = row.get(0)?;
        Ok(ProjectDocument {
            id: EntityId::parse(&id_str).unwrap_or_else(|_| EntityId::new()),
            path: row.get(1)?,
            title: row.get(2)?,
            content: row.get(3)?,
            doc_type: row.get(4)?,
            source: row.get(5)?,
            checksum: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    }
}

/// Directories never worth indexing.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    ".svn",
    ".hg",
    "__pycache__",
    ".venv",
    "venv",
    ".next",
    ".cache",
];

/// File extensions treated as importable text documents.
fn is_doc_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "md" | "markdown" | "txt"
    )
}

/// Recursively walk `dir` and upsert every `.md`/`.markdown`/`.txt` file.
///
/// Stale entries (files that were indexed before but are gone from disk) are
/// pruned at the end, so the corpus mirrors the folder instead of growing
/// zombie rows.
pub fn import_directory(repo: &ProjectDocumentRepository, dir: &Path) -> Result<ImportReport> {
    if !dir.is_dir() {
        return Err(AppError::Validation(format!(
            "'{}' is not a directory",
            dir.display()
        )));
    }

    let mut report = ImportReport::default();
    let mut keep: Vec<String> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            report.failed += 1;
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if SKIP_DIRS.contains(&name.as_str()) || name.starts_with('.') {
                    continue;
                }
                stack.push(path);
            } else if is_doc_file(&path) {
                report.scanned += 1;
                let path_str = path.to_string_lossy().to_string();
                keep.push(path_str.clone());
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let title = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| path_str.clone());
                        let doc_type = match path.extension().and_then(|e| e.to_str()) {
                            Some("txt") => "plaintext".to_string(),
                            _ => "markdown".to_string(),
                        };
                        match repo.upsert(&path_str, &title, &content, &doc_type, "import") {
                            Ok(true) => {
                                report.imported += 1;
                                // Refresh the semantic fingerprint so the new
                                // content is immediately searchable.
                                crate::core::context::indexer::spawn_index_document_by_path(
                                    &path_str, &content,
                                );
                            }
                            Ok(false) => report.unchanged += 1,
                            Err(e) => {
                                report.failed += 1;
                                report.errors.push(format!("{}: {}", path_str, e));
                            }
                        }
                    }
                    Err(e) => {
                        report.failed += 1;
                        report.errors.push(format!("{}: {}", path_str, e));
                    }
                }
            }
        }
    }

    match repo.prune_not_in(&keep) {
        Ok(removed) => {
            report.updated = removed;
            // Pruned docs also dropped their fingerprints via `delete`.
            tracing::info!("Docs import: pruned {} stale entries", removed);
        }
        Err(e) => report.errors.push(format!("prune: {}", e)),
    }

    Ok(report)
}

/// A search hit: document plus its relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentHit {
    pub document: ProjectDocument,
    pub score: f64,
}

/// Search the imported document corpus.
///
/// Combines two signals so the search works with or without a loaded ONNX
/// model:
///
/// * text overlap — fraction of query words found in title+content (always
///   available, cheap, deterministic);
/// * semantic similarity — cosine distance against `document_fingerprints`
///   when the embeddings exist.
///
/// The final score is the *maximum* of the two, mirroring how the memory
/// conflict detector combines signals: a strong keyword match must not be
/// buried by an unrelated vector.
pub fn search_docs(query: &str, limit: u32) -> Result<Vec<DocumentHit>> {
    let repo = ProjectDocumentRepository::open()?;
    let limit = limit.min(100) as usize;
    let docs = repo.list(1000)?;
    if docs.is_empty() {
        return Ok(Vec::new());
    }

    let query_lower = query.to_lowercase();
    let query_words: Vec<String> = query_lower
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_'))
        .filter(|w| !w.is_empty() && w.len() > 1)
        .map(|w| w.to_string())
        .collect();

    // Semantic scores keyed by document id, when fingerprints exist.
    let mut semantic: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    if let Ok(conn) = crate::db::open_connection()
        && let Ok(search) = crate::core::context::semantic_search::SemanticSearch::new(conn)
        && let Ok(hits) = search.search_documents(query, limit as u32 * 3)
    {
        for (id, score) in hits {
            semantic.insert(id.as_str().to_string(), score);
        }
    }

    let mut results: Vec<DocumentHit> = Vec::new();
    for doc in &docs {
        if query_words.is_empty() {
            results.push(DocumentHit {
                document: doc.clone(),
                score: 1.0,
            });
            continue;
        }

        let haystack = format!("{} {}\n{}", doc.title, doc.path, doc.content).to_lowercase();
        let matched = query_words
            .iter()
            .filter(|w| haystack.contains(w.as_str()))
            .count();
        let text_score = if query_words.is_empty() {
            0.0
        } else {
            matched as f64 / query_words.len() as f64
        };

        let sem_score = semantic.get(doc.id.as_str()).copied().unwrap_or(0.0);
        let score = text_score.max(sem_score);
        if score > 0.0 {
            results.push(DocumentHit {
                document: doc.clone(),
                score,
            });
        }
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_repo() -> ProjectDocumentRepository {
        let conn = Connection::open_in_memory().unwrap();
        ProjectDocumentRepository::new(conn).unwrap()
    }

    #[test]
    fn upsert_creates_then_noops() {
        let repo = test_repo();
        let created = repo
            .upsert("/p/a.md", "A", "# A\nbody", "markdown", "import")
            .unwrap();
        assert!(created);
        let noop = repo
            .upsert("/p/a.md", "A", "# A\nbody", "markdown", "import")
            .unwrap();
        assert!(!noop);
        assert_eq!(repo.count().unwrap(), 1);
    }

    #[test]
    fn upsert_updates_on_change() {
        let repo = test_repo();
        repo.upsert("/p/a.md", "A", "v1", "markdown", "import")
            .unwrap();
        let changed = repo
            .upsert("/p/a.md", "A", "v2 content", "markdown", "import")
            .unwrap();
        assert!(changed);
        let docs = repo.list(10).unwrap();
        assert_eq!(docs[0].content, "v2 content");
        assert_eq!(repo.count().unwrap(), 1);
    }

    #[test]
    fn import_directory_walks_recursively() {
        let tmp = std::env::temp_dir().join(format!("nexus-docs-test-{}", EntityId::new()));
        std::fs::create_dir_all(tmp.join("sub")).unwrap();
        std::fs::write(tmp.join("readme.md"), "# Readme\nProject docs").unwrap();
        std::fs::write(tmp.join("sub").join("api.txt"), "API reference").unwrap();
        std::fs::write(tmp.join("main.rs"), "fn main() {}").unwrap(); // not a doc
        std::fs::write(tmp.join("notes.md"), "ignored").unwrap();
        std::fs::create_dir_all(tmp.join("node_modules")).unwrap();
        std::fs::write(tmp.join("node_modules").join("x.md"), "skip me").unwrap();

        let repo = test_repo();
        let report = import_directory(&repo, &tmp).unwrap();

        assert_eq!(report.scanned, 3);
        assert_eq!(report.imported, 3);
        assert_eq!(repo.count().unwrap(), 3);

        // Re-import is idempotent
        let report2 = import_directory(&repo, &tmp).unwrap();
        assert_eq!(report2.imported, 0);
        assert_eq!(report2.unchanged, 3);
        assert_eq!(repo.count().unwrap(), 3);

        // Remove one file; re-import prunes it
        std::fs::remove_file(tmp.join("sub").join("api.txt")).unwrap();
        let report3 = import_directory(&repo, &tmp).unwrap();
        assert_eq!(report3.updated, 1);
        assert_eq!(repo.count().unwrap(), 2);

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn delete_removes_document() {
        let repo = test_repo();
        repo.upsert("/p/a.md", "A", "body", "markdown", "import")
            .unwrap();
        let doc = repo.list(10).unwrap().remove(0);
        repo.delete(&doc.id).unwrap();
        assert_eq!(repo.count().unwrap(), 0);
    }
}
