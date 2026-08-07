//! Code graph — structured layer over source files.
//!
//! The interpreter parsers ([`crate::core::interpreter::code_parser`]) already
//! extract classes, functions, structs, traits and interfaces from 7+
//! languages. What they never produced is the *structure*: which file defines
//! what, and — most importantly — the dependency edges between files
//! (`import` / `require` / `use` / `#include` / `mod`).
//!
//! This module persists that structure so an agent can answer questions like
//! "what depends on the tokenizer?", "which module imports `context_builder`?",
//! "is this symbol external or ours?" — without dumping source files into
//! semantic memory.
//!
//! It is deliberately separate from the memory graph and from
//! `project_documents`: code structure is a *map*, not a memory, and mixing
//! the two degrades both.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::core::entity_id::EntityId;
use crate::core::knowledge::content_checksum;
use crate::core::result::{AppError, Result};

/// A source file indexed into the code graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFile {
    pub id: EntityId,
    pub path: String,
    pub title: String,
    pub language: String,
    pub checksum: String,
    pub line_count: u32,
    pub symbol_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

/// A symbol (class / function / struct / trait / interface / method) extracted
/// by the language parser and tied to its file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSymbol {
    pub id: EntityId,
    pub file_id: String,
    pub name: String,
    pub kind: String,
    pub language: String,
    pub signature: String,
    pub line: u32,
    pub created_at: String,
}

/// A dependency edge: this file imports/uses `target`.
///
/// `is_external` is true when the target is a third-party package / system
/// header rather than a file inside the indexed tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeDependency {
    pub id: EntityId,
    pub file_id: String,
    pub target: String,
    pub kind: String,
    pub is_external: bool,
    pub created_at: String,
}

/// A symbol hit from [`CodeGraphRepository::search_symbols`] — the symbol
/// together with the file that defines it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolHit {
    pub symbol: CodeSymbol,
    pub file_path: String,
    pub file_language: String,
}

/// A reverse edge from [`CodeGraphRepository::dependents_of`] — a file that
/// depends on the queried target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReverseHit {
    pub file_path: String,
    pub kind: String,
}

/// Outcome of importing a directory of source files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeImportReport {
    pub scanned: u64,
    pub indexed: u64,
    pub unchanged: u64,
    pub symbols: u64,
    pub dependencies: u64,
    pub pruned: u64,
    pub failed: u64,
    pub errors: Vec<String>,
}

/// SQLite-backed repository for the code graph.
pub struct CodeGraphRepository {
    conn: Connection,
}

impl CodeGraphRepository {
    pub fn new(conn: Connection) -> Result<Self> {
        crate::storage::sqlite::schema::apply_migrations(&conn)?;
        Ok(Self { conn })
    }

    pub fn open() -> Result<Self> {
        let conn = crate::db::open_connection().map_err(AppError::Database)?;
        Self::new(conn)
    }

    /// Upsert a code file (by path). Returns `true` when the file changed and
    /// its symbols/dependencies must be refreshed.
    pub fn upsert_file(
        &self,
        path: &str,
        title: &str,
        language: &str,
        checksum: &str,
        line_count: u32,
    ) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT checksum FROM code_files WHERE path = ?1",
                params![path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?;

        let id = match existing {
            Some(old) if old == checksum => return Ok(false),
            Some(_) => {
                let id: String = self
                    .conn
                    .query_row(
                        "SELECT id FROM code_files WHERE path = ?1",
                        params![path],
                        |row| row.get(0),
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                self.conn
                    .execute(
                        "UPDATE code_files
                         SET title = ?2, language = ?3, checksum = ?4, line_count = ?5,
                             updated_at = ?6
                         WHERE id = ?1",
                        params![id, title, language, checksum, line_count, now],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                id
            }
            None => {
                let id = EntityId::new();
                self.conn
                    .execute(
                        "INSERT INTO code_files
                         (id, path, title, language, checksum, line_count, symbol_count, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?7)",
                        params![
                            id.as_str(),
                            path,
                            title,
                            language,
                            checksum,
                            line_count,
                            now
                        ],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                id.as_str().to_string()
            }
        };

        // Refresh symbols and dependencies for this file.
        let id_owned = id.clone();
        let conn = &self.conn;
        conn.execute(
            "DELETE FROM code_symbols WHERE file_id = ?1",
            params![id_owned],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        conn.execute(
            "DELETE FROM code_dependencies WHERE file_id = ?1",
            params![id_owned],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(true)
    }

    pub fn add_symbol(
        &self,
        file_id: &str,
        name: &str,
        kind: &str,
        language: &str,
        signature: &str,
        line: u32,
    ) -> Result<()> {
        let id = EntityId::new();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO code_symbols
                 (id, file_id, name, kind, language, signature, line, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id.as_str(),
                    file_id,
                    name,
                    kind,
                    language,
                    signature,
                    line,
                    now
                ],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn add_dependency(
        &self,
        file_id: &str,
        target: &str,
        kind: &str,
        is_external: bool,
    ) -> Result<()> {
        let id = EntityId::new();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO code_dependencies
                 (id, file_id, target, kind, is_external, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id.as_str(), file_id, target, kind, is_external, now],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    /// Mark a file's symbol count after indexing (used for display only).
    pub fn set_symbol_count(&self, file_id: &str, count: u32) -> Result<()> {
        self.conn
            .execute(
                "UPDATE code_files SET symbol_count = ?2 WHERE id = ?1",
                params![file_id, count],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn list(&self, limit: u32) -> Result<Vec<CodeFile>> {
        let limit = limit.min(2000);
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, path, title, language, checksum, line_count, symbol_count, created_at, updated_at
                 FROM code_files ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([limit], Self::row_to_file)
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn get(&self, id: &EntityId) -> Result<Option<CodeFile>> {
        self.conn
            .query_row(
                "SELECT id, path, title, language, checksum, line_count, symbol_count, created_at, updated_at
                 FROM code_files WHERE id = ?1",
                params![id.as_str()],
                Self::row_to_file,
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn get_by_path(&self, path: &str) -> Result<Option<CodeFile>> {
        self.conn
            .query_row(
                "SELECT id, path, title, language, checksum, line_count, symbol_count, created_at, updated_at
                 FROM code_files WHERE path = ?1",
                params![path],
                Self::row_to_file,
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn count(&self) -> Result<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM code_files", [], |row| row.get(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(n.max(0) as u64)
    }

    pub fn symbol_count(&self) -> Result<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM code_symbols", [], |row| row.get(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(n.max(0) as u64)
    }

    pub fn dependency_count(&self) -> Result<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM code_dependencies", [], |row| {
                row.get(0)
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(n.max(0) as u64)
    }

    /// All indexed paths — used to prune files removed from disk.
    pub fn all_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM code_files")
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

    /// Remove a file and everything that points at it.
    pub fn delete_by_path(&self, path: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM code_files WHERE path = ?1", params![path])
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    /// Symbols of a single file.
    pub fn symbols_of(&self, file_id: &str) -> Result<Vec<CodeSymbol>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_id, name, kind, language, signature, line, created_at
                 FROM code_symbols WHERE file_id = ?1 ORDER BY line",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([file_id], Self::row_to_symbol)
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(out)
    }

    /// Dependencies of a single file (by path).
    pub fn dependencies_of(&self, path: &str) -> Result<Vec<CodeDependency>> {
        let Some(file) = self.get_by_path(path)? else {
            return Ok(Vec::new());
        };
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_id, target, kind, is_external, created_at
                 FROM code_dependencies WHERE file_id = ?1 ORDER BY target",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([file.id.as_str()], Self::row_to_dependency)
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(out)
    }

    /// Search symbols by (substring) name. Returns symbols with their file.
    pub fn search_symbols(&self, query: &str, limit: u32) -> Result<Vec<SymbolHit>> {
        let limit = limit.min(100) as i64;
        let pattern = format!("%{}%", query.to_lowercase());
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.id, s.file_id, s.name, s.kind, s.language, s.signature, s.line, s.created_at,
                        f.path, f.language
                 FROM code_symbols s
                 JOIN code_files f ON f.id = s.file_id
                 WHERE LOWER(s.name) LIKE ?1
                 ORDER BY s.name LIMIT ?2",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(params![pattern, limit], |row| {
                let symbol = Self::row_to_symbol(row)?;
                Ok((symbol, row.get::<_, String>(8)?, row.get::<_, String>(9)?))
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            let (symbol, file_path, file_language) =
                row.map_err(|e| AppError::Database(e.to_string()))?;
            out.push(SymbolHit {
                symbol,
                file_path,
                file_language,
            });
        }
        Ok(out)
    }

    /// Files that depend on `target` (reverse edges). Returns (file, kind).
    pub fn dependents_of(&self, target: &str) -> Result<Vec<ReverseHit>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT f.path, d.kind
                 FROM code_dependencies d
                 JOIN code_files f ON f.id = d.file_id
                 WHERE d.target = ?1 AND d.is_external = 0
                 ORDER BY f.path",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([target], |row| {
                Ok(ReverseHit {
                    file_path: row.get(0)?,
                    kind: row.get(1)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(out)
    }

    fn row_to_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<CodeFile> {
        let id_str: String = row.get(0)?;
        Ok(CodeFile {
            id: EntityId::parse(&id_str).unwrap_or_else(|_| EntityId::new()),
            path: row.get(1)?,
            title: row.get(2)?,
            language: row.get(3)?,
            checksum: row.get(4)?,
            line_count: row.get(5)?,
            symbol_count: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    }

    fn row_to_symbol(row: &rusqlite::Row<'_>) -> rusqlite::Result<CodeSymbol> {
        let id_str: String = row.get(0)?;
        Ok(CodeSymbol {
            id: EntityId::parse(&id_str).unwrap_or_else(|_| EntityId::new()),
            file_id: row.get(1)?,
            name: row.get(2)?,
            kind: row.get(3)?,
            language: row.get(4)?,
            signature: row.get(5)?,
            line: row.get(6)?,
            created_at: row.get(7)?,
        })
    }

    fn row_to_dependency(row: &rusqlite::Row<'_>) -> rusqlite::Result<CodeDependency> {
        let id_str: String = row.get(0)?;
        Ok(CodeDependency {
            id: EntityId::parse(&id_str).unwrap_or_else(|_| EntityId::new()),
            file_id: row.get(1)?,
            target: row.get(2)?,
            kind: row.get(3)?,
            is_external: row.get::<_, i64>(4)? != 0,
            created_at: row.get(5)?,
        })
    }
}

/// Directories never worth indexing (same list as documents.rs).
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

/// Language label for a file extension (mirrors file_interpreter.rs).
fn language_for(ext: &str) -> Option<&'static str> {
    match ext {
        "py" => Some("Python"),
        "js" | "jsx" | "mjs" => Some("JavaScript"),
        "ts" | "tsx" => Some("TypeScript"),
        "rs" => Some("Rust"),
        "go" => Some("Go"),
        "java" => Some("Java"),
        "c" | "h" => Some("C"),
        "cpp" | "hpp" | "cc" | "cxx" => Some("C++"),
        _ => None,
    }
}

/// True when the file is a code file we can parse dependencies for.
fn is_code_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| language_for(&e.to_lowercase()).is_some())
        .unwrap_or(false)
}

/// Recursively walk `dir`, index every supported source file, extract symbols
/// (via the existing interpreter parsers) and dependency edges (via a
/// language-aware line scan). Prunes entries that disappeared from disk.
pub fn import_code_directory(repo: &CodeGraphRepository, dir: &Path) -> Result<CodeImportReport> {
    if !dir.is_dir() {
        return Err(AppError::Validation(format!(
            "'{}' is not a directory",
            dir.display()
        )));
    }

    let mut report = CodeImportReport::default();
    let mut keep: Vec<String> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];

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
            } else if is_code_file(&path) {
                report.scanned += 1;
                let path_str = path.to_string_lossy().to_string();
                keep.push(path_str.clone());
                match index_one_file(repo, &path, &path_str) {
                    Ok(changed) => {
                        if changed {
                            report.indexed += 1;
                        } else {
                            report.unchanged += 1;
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

    // Prune files that no longer exist on disk.
    let existing = repo.all_paths()?;
    for path in existing {
        if !keep.contains(&path) {
            repo.delete_by_path(&path)?;
            report.pruned += 1;
        }
    }

    Ok(report)
}

/// Index one file: parse with the interpreter, store symbols + dependencies.
fn index_one_file(repo: &CodeGraphRepository, path: &Path, path_str: &str) -> Result<bool> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let Some(language) = language_for(&ext) else {
        return Ok(false);
    };

    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {}", path.display(), e)))?;
    let checksum = content_checksum(&content);
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path_str.to_string());
    let line_count = content.lines().count() as u32;

    let changed = repo.upsert_file(path_str, &title, language, &checksum, line_count)?;
    if !changed {
        return Ok(false);
    }

    let Some(file) = repo.get_by_path(path_str)? else {
        return Ok(true);
    };
    let file_id = file.id.as_str().to_string();

    // 1. Symbols from the existing interpreter parser.
    let parsed = crate::core::interpreter::file_interpreter::interpret_file(path, &content);
    let mut symbol_count: u32 = 0;
    for entity in &parsed.sub_entities {
        let kind = entity
            .metadata
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("symbol")
            .to_string();
        let signature = entity.description.clone();
        repo.add_symbol(&file_id, &entity.title, &kind, language, &signature, 0)?;
        symbol_count += 1;
    }
    repo.set_symbol_count(&file_id, symbol_count)?;

    // 2. Dependency edges from a language-aware scan.
    let deps = detect_dependencies(&ext, &content);
    for dep in deps {
        // Resolve relative targets to a canonical relative path when possible.
        let (target, is_external) = resolve_target(&dep.target, path);
        repo.add_dependency(&file_id, &target, &dep.kind, is_external)?;
    }

    Ok(true)
}

/// A raw dependency as written in source (before path resolution).
struct RawDep {
    kind: String,
    target: String,
}

/// Detect `import`/`require`/`use`/`#include`/`mod` edges per language.
/// Line-based by design: it needs to be fast and good enough for a map, not a
/// compiler.
fn detect_dependencies(ext: &str, content: &str) -> Vec<RawDep> {
    let mut deps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        match ext {
            "py" => {
                if trimmed.starts_with('#') {
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("import ") {
                    if let Some(name) = rest.split_whitespace().next() {
                        deps.push(RawDep {
                            kind: "import".into(),
                            target: name.trim_end_matches(',').to_string(),
                        });
                    }
                } else if let Some(rest) = trimmed.strip_prefix("from ")
                    && let Some(name) = rest.split_whitespace().next()
                {
                    deps.push(RawDep {
                        kind: "import".into(),
                        target: name.to_string(),
                    });
                }
            }
            "js" | "jsx" | "mjs" | "ts" | "tsx" => {
                if trimmed.starts_with("import ") || trimmed.starts_with("export ") {
                    // import ... from 'x' / import 'x'
                    if let Some(q) = trimmed.find('"').or_else(|| trimmed.find('\'')) {
                        let quote = trimmed.as_bytes()[q] as char;
                        let rest = &trimmed[q + 1..];
                        if let Some(end) = rest.find(quote) {
                            let target = &rest[..end];
                            if !target.is_empty() {
                                deps.push(RawDep {
                                    kind: "import".into(),
                                    target: target.to_string(),
                                });
                            }
                        }
                    }
                } else if let Some(rq) = trimmed.find("require(") {
                    let rest = &trimmed[rq + 8..];
                    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'');
                    if let Some(quote) = quote
                        && let Some(end) = rest[1..].find(quote)
                    {
                        let target = &rest[1..1 + end];
                        deps.push(RawDep {
                            kind: "require".into(),
                            target: target.to_string(),
                        });
                    }
                }
            }
            "rs" => {
                if let Some(rest) = trimmed.strip_prefix("use ") {
                    if let Some(name) = rest.split_whitespace().next() {
                        // Strip local prefixes (crate:: / self:: / super::) so
                        // `use crate::main` resolves to the sibling `main.rs`.
                        let stripped = name
                            .trim_start_matches("crate::")
                            .trim_start_matches("self::")
                            .trim_start_matches("super::")
                            .trim_end_matches(';');
                        let target = stripped.split("::").next().unwrap_or(stripped).to_string();
                        deps.push(RawDep {
                            kind: "use".into(),
                            target,
                        });
                    }
                } else if trimmed.starts_with("mod ") {
                    // `mod foo;` — a sibling module file.
                    if let Some(name) = trimmed
                        .strip_prefix("mod ")
                        .and_then(|r| r.split_whitespace().next())
                    {
                        deps.push(RawDep {
                            kind: "mod".into(),
                            target: name.trim_end_matches(';').to_string(),
                        });
                    }
                }
            }
            "go" => {
                if let Some(rest) = trimmed.strip_prefix("import ") {
                    let name = rest.trim().trim_matches('"');
                    if !name.is_empty() && !name.starts_with('(') {
                        deps.push(RawDep {
                            kind: "import".into(),
                            target: name.to_string(),
                        });
                    }
                } else if let Some(rest) = trimmed.strip_prefix("import (") {
                    // multi-line block — handle each quoted path in a lazy manner:
                    // the lines inside will be `"pkg"`, caught by the generic branch below.
                    let _ = rest;
                }
            }
            "java" => {
                if let Some(rest) = trimmed.strip_prefix("import ") {
                    let name = rest.trim_end_matches(';').trim();
                    if !name.is_empty() {
                        deps.push(RawDep {
                            kind: "import".into(),
                            target: name.to_string(),
                        });
                    }
                }
            }
            "c" | "cpp" | "h" | "hpp" | "cc" | "cxx" => {
                if let Some(rest) = trimmed.strip_prefix("#include") {
                    let name = rest
                        .trim()
                        .trim_matches('<')
                        .trim_matches('>')
                        .trim_matches('"');
                    if !name.is_empty() {
                        deps.push(RawDep {
                            kind: "include".into(),
                            target: name.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    deps
}

/// Resolve a raw dependency target to a canonical form.
///
/// Relative targets (`./x`, `../y`) and Rust locals (`crate`, `self`,
/// `super`) are internal. A Rust `mod foo;` that resolves to a sibling
/// `foo.rs` on disk is canonicalized to that file's path (internal).
/// Everything else (packages, std headers, bare crates) is external.
fn resolve_target(target: &str, from_file: &Path) -> (String, bool) {
    let t = target.trim();
    if t.is_empty() {
        return (t.to_string(), true);
    }

    let is_internal = t.starts_with('.')
        || t.starts_with("crate")
        || t.starts_with("self")
        || t.starts_with("super");

    // Rust `mod foo;` → sibling file `foo.rs` when it exists. Keep the bare
    // name as the target so reverse queries (`dependents_of("foo")`) work;
    // the edge is simply marked internal.
    if from_file.extension().map(|e| e.to_str().unwrap_or("")) == Some("rs")
        && !t.contains('.')
        && !t.contains('/')
        && !t.contains(':')
        && let Some(parent) = from_file.parent()
    {
        let sibling = parent.join(format!("{}.rs", t));
        if sibling.exists() {
            return (t.to_string(), false); // internal: resolves to a real file
        }
    }

    (t.to_string(), !is_internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_repo() -> CodeGraphRepository {
        let conn = Connection::open_in_memory().unwrap();
        CodeGraphRepository::new(conn).unwrap()
    }

    #[test]
    fn upsert_file_creates_then_noops() {
        let repo = test_repo();
        let changed = repo
            .upsert_file("/proj/a.rs", "a.rs", "Rust", "abc", 10)
            .unwrap();
        assert!(changed, "first insert must report change");
        let unchanged = repo
            .upsert_file("/proj/a.rs", "a.rs", "Rust", "abc", 10)
            .unwrap();
        assert!(!unchanged, "same checksum must be a no-op");
        assert_eq!(repo.count().unwrap(), 1);
        let file = repo.get_by_path("/proj/a.rs").unwrap().unwrap();
        assert_eq!(file.language, "Rust");
        assert_eq!(file.line_count, 10);
    }

    #[test]
    fn changed_checksum_refreshes() {
        let repo = test_repo();
        repo.upsert_file("/proj/a.rs", "a.rs", "Rust", "abc", 10)
            .unwrap();
        let changed = repo
            .upsert_file("/proj/a.rs", "a.rs", "Rust", "def", 12)
            .unwrap();
        assert!(changed, "changed checksum must report change");
        assert_eq!(
            repo.get_by_path("/proj/a.rs").unwrap().unwrap().line_count,
            12
        );
    }

    #[test]
    fn symbols_and_dependencies_roundtrip() {
        let repo = test_repo();
        repo.upsert_file("/proj/a.rs", "a.rs", "Rust", "abc", 10)
            .unwrap();
        let file = repo.get_by_path("/proj/a.rs").unwrap().unwrap();
        repo.add_symbol(file.id.as_str(), "main", "function", "Rust", "fn main()", 3)
            .unwrap();
        repo.add_dependency(file.id.as_str(), "std", "use", true)
            .unwrap();
        assert_eq!(repo.symbol_count().unwrap(), 1);
        assert_eq!(repo.dependency_count().unwrap(), 1);
        let symbols = repo.symbols_of(file.id.as_str()).unwrap();
        assert_eq!(symbols[0].name, "main");
        assert_eq!(symbols[0].kind, "function");
        let deps = repo.dependencies_of("/proj/a.rs").unwrap();
        assert_eq!(deps[0].target, "std");
        assert!(deps[0].is_external);
    }

    #[test]
    fn search_symbols_finds_substring() {
        let repo = test_repo();
        repo.upsert_file("/proj/lib.rs", "lib.rs", "Rust", "abc", 10)
            .unwrap();
        let file = repo.get_by_path("/proj/lib.rs").unwrap().unwrap();
        repo.add_symbol(
            file.id.as_str(),
            "Tokenizer",
            "struct",
            "Rust",
            "struct Tokenizer",
            1,
        )
        .unwrap();
        let hits = repo.search_symbols("token", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].symbol.name, "Tokenizer");
        assert_eq!(hits[0].file_path, "/proj/lib.rs");
    }

    #[test]
    fn detect_python_imports() {
        let deps = detect_dependencies(
            "py",
            "import os\nfrom typing import List\nimport numpy as np\n",
        );
        let targets: Vec<&str> = deps.iter().map(|d| d.target.as_str()).collect();
        assert_eq!(targets, vec!["os", "typing", "numpy"]);
    }

    #[test]
    fn detect_rust_use_and_mod() {
        let deps = detect_dependencies(
            "rs",
            "use std::collections::HashMap;\nuse crate::db;\nmod tokenizer;\n",
        );
        let targets: Vec<&str> = deps.iter().map(|d| d.target.as_str()).collect();
        assert_eq!(targets, vec!["std", "db", "tokenizer"]);
        let kinds: Vec<&str> = deps.iter().map(|d| d.kind.as_str()).collect();
        assert_eq!(kinds, vec!["use", "use", "mod"]);
    }

    #[test]
    fn detect_js_imports_and_requires() {
        let deps = detect_dependencies(
            "ts",
            "import fs from 'fs';\nimport { a } from './local';\nconst x = require('lodash');\n",
        );
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].target, "fs");
        assert_eq!(deps[1].target, "./local");
        assert_eq!(deps[2].target, "lodash");
    }

    #[test]
    fn detect_include_and_go() {
        let deps = detect_dependencies("c", "#include <stdio.h>\n#include \"mylib.h\"\n");
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].target, "stdio.h");
        assert_eq!(deps[1].target, "mylib.h");

        let go_deps = detect_dependencies("go", "import \"fmt\"\nimport (\n\t\"strings\"\n)\n");
        assert_eq!(go_deps.len(), 1);
        assert_eq!(go_deps[0].target, "fmt");
    }

    #[test]
    fn resolve_target_marks_external_vs_internal() {
        let from = Path::new("/proj/src/main.rs");
        let (t1, e1) = resolve_target("./local", from);
        assert_eq!(t1, "./local");
        assert!(!e1, "relative import is internal");
        let (_, e2) = resolve_target("serde", from);
        assert!(e2, "bare crate name without matching file is external");
    }

    #[test]
    fn import_directory_walks_and_parses() {
        let tmp = std::env::temp_dir().join(format!("nexus-cg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("src")).unwrap();
        std::fs::write(
            tmp.join("src").join("main.rs"),
            "use std::fs;\nfn main() {}\nstruct App {}\n",
        )
        .unwrap();
        std::fs::write(
            tmp.join("src").join("lib.rs"),
            "use crate::main;\npub fn helper() {}\n",
        )
        .unwrap();
        std::fs::write(tmp.join("readme.md"), "# readme\n").unwrap();
        std::fs::write(tmp.join("src").join("data.json"), "{}").unwrap();

        let repo = test_repo();
        let report = import_code_directory(&repo, &tmp).unwrap();
        assert_eq!(report.scanned, 2, "only code files scanned");
        assert_eq!(report.indexed, 2);
        assert_eq!(repo.count().unwrap(), 2);
        assert_eq!(
            repo.symbol_count().unwrap(),
            3,
            "main.rs: main + App; lib.rs: helper"
        );

        // main.rs depends on std; lib.rs depends on crate (internal -> main).
        let main_deps = repo
            .dependencies_of(&tmp.join("src").join("main.rs").to_string_lossy())
            .unwrap();
        assert!(!main_deps.is_empty(), "main.rs must have deps");

        // Re-import is idempotent.
        let report2 = import_code_directory(&repo, &tmp).unwrap();
        assert_eq!(report2.indexed, 0);
        assert_eq!(report2.unchanged, 2);

        // Removing a file prunes it.
        std::fs::remove_file(tmp.join("src").join("lib.rs")).unwrap();
        let report3 = import_code_directory(&repo, &tmp).unwrap();
        assert_eq!(report3.pruned, 1);
        assert_eq!(repo.count().unwrap(), 1);
    }
}
