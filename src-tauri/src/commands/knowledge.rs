//! Tauri commands for the project knowledge base: RAG documents, AGENTS.md
//! instructions, and skills.

use serde::{Deserialize, Serialize};

use crate::core::knowledge::agents::{self, AgentsFile, AgentsRepository};
use crate::core::knowledge::code_graph::{
    CodeFile, CodeGraphRepository, CodeImportReport, ReverseHit, SymbolHit,
};
use crate::core::knowledge::documents::{
    DocumentHit, ImportReport, ProjectDocument, ProjectDocumentRepository,
};
use crate::core::knowledge::skills::{Skill, SkillOutput, SkillRepository, SkillRunner};

// ═══════════════════════════════════════════════════════════════
//  Project documents (RAG corpus)
// ═══════════════════════════════════════════════════════════════

/// Import all `.md`/`.markdown`/`.txt` files from a folder into the knowledge
/// base. Idempotent: unchanged files are skipped, changed files re-indexed,
/// removed files pruned.
#[tauri::command]
pub async fn import_docs(folder_path: String) -> Result<ImportReport, String> {
    let repo = ProjectDocumentRepository::open().map_err(|e| e.to_string())?;
    let dir = std::path::PathBuf::from(&folder_path);
    crate::core::knowledge::documents::import_directory(&repo, &dir).map_err(|e| e.to_string())
}

/// List imported project documents, newest first.
#[tauri::command]
pub async fn list_docs(limit: Option<u32>) -> Result<Vec<ProjectDocument>, String> {
    let repo = ProjectDocumentRepository::open().map_err(|e| e.to_string())?;
    repo.list(limit.unwrap_or(100)).map_err(|e| e.to_string())
}

/// Search project documents by text overlap (always available) and semantic
/// similarity (when fingerprints exist). Returns documents with scores.
#[tauri::command]
pub async fn search_docs(query: String, limit: Option<u32>) -> Result<Vec<DocumentHit>, String> {
    crate::core::knowledge::documents::search_docs(&query, limit.unwrap_or(10))
        .map_err(|e| e.to_string())
}

/// Get knowledge base statistics for the UI.
#[tauri::command]
pub async fn knowledge_stats() -> Result<KnowledgeStats, String> {
    let docs = ProjectDocumentRepository::open().map_err(|e| e.to_string())?;
    let agents = AgentsRepository::open().map_err(|e| e.to_string())?;
    let skills = SkillRepository::open().map_err(|e| e.to_string())?;

    Ok(KnowledgeStats {
        document_count: docs.count().map_err(|e| e.to_string())?,
        agents_count: agents.list().map_err(|e| e.to_string())?.len() as u64,
        skill_count: skills.list().map_err(|e| e.to_string())?.len() as u64,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeStats {
    pub document_count: u64,
    pub agents_count: u64,
    pub skill_count: u64,
}

// ═══════════════════════════════════════════════════════════════
//  AGENTS.md instructions
// ═══════════════════════════════════════════════════════════════

/// Read an agents instruction file by name (default `AGENTS.md`).
#[tauri::command]
pub async fn agents_read(name: Option<String>) -> Result<Option<AgentsFile>, String> {
    let repo = AgentsRepository::open().map_err(|e| e.to_string())?;
    let name = name.unwrap_or_else(|| agents::DEFAULT_AGENTS_NAME.to_string());
    repo.get(&name).map_err(|e| e.to_string())
}

/// List all stored agents instruction files.
#[tauri::command]
pub async fn agents_list() -> Result<Vec<AgentsFile>, String> {
    let repo = AgentsRepository::open().map_err(|e| e.to_string())?;
    repo.list().map_err(|e| e.to_string())
}

/// Save (or update) an agents instruction file.
#[tauri::command]
pub async fn agents_save(
    name: String,
    content: String,
    path: Option<String>,
) -> Result<AgentsFile, String> {
    let repo = AgentsRepository::open().map_err(|e| e.to_string())?;
    repo.upsert(&name, &content, path.as_deref().unwrap_or(""))
        .map_err(|e| e.to_string())?;
    repo.get(&name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Saved file not found".to_string())
}

/// Delete an agents instruction file.
#[tauri::command]
pub async fn agents_delete(name: String) -> Result<(), String> {
    let repo = AgentsRepository::open().map_err(|e| e.to_string())?;
    repo.delete(&name).map_err(|e| e.to_string())
}

/// Generate an AGENTS.md from live system data and store it as the active
/// instruction file — the "documentation skill".
#[tauri::command]
pub async fn agents_generate() -> Result<AgentsFile, String> {
    let content = agents::generate_agents_file();
    let repo = AgentsRepository::open().map_err(|e| e.to_string())?;
    repo.upsert(agents::DEFAULT_AGENTS_NAME, &content, "generated:agents.md")
        .map_err(|e| e.to_string())?;
    repo.get(agents::DEFAULT_AGENTS_NAME)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Generated file not found".to_string())
}

// ═══════════════════════════════════════════════════════════════
//  Skills
// ═══════════════════════════════════════════════════════════════

/// List all registered skills.
#[tauri::command]
pub async fn skills_list() -> Result<Vec<Skill>, String> {
    let repo = SkillRepository::open().map_err(|e| e.to_string())?;
    repo.list().map_err(|e| e.to_string())
}

/// Register (or update) a skill.
#[tauri::command]
pub async fn skills_register(
    name: String,
    description: String,
    command: String,
    script_path: Option<String>,
) -> Result<Skill, String> {
    let repo = SkillRepository::open().map_err(|e| e.to_string())?;
    repo.upsert(
        &name,
        &description,
        &command,
        script_path.as_deref().unwrap_or(""),
    )
    .map_err(|e| e.to_string())
}

/// Delete a skill by id.
#[tauri::command]
pub async fn skills_delete(id: String) -> Result<(), String> {
    let repo = SkillRepository::open().map_err(|e| e.to_string())?;
    let entity_id = crate::core::entity_id::EntityId::parse(&id).map_err(|e| e.to_string())?;
    repo.delete(&entity_id).map_err(|e| e.to_string())
}

/// Run a skill by name with the given arguments.
#[tauri::command]
pub async fn skills_run(name: String, args: Option<Vec<String>>) -> Result<SkillOutput, String> {
    let args = args.unwrap_or_default();
    SkillRunner::run(&name, &args).map_err(|e| e.to_string())
}

// ═══════════════════════════════════════════════════════════════
//  Code graph — structured map of source files
// ═══════════════════════════════════════════════════════════════

/// Import a folder of source files into the code graph. Symbols are extracted
/// with the existing language parsers; dependency edges (`import`/`use`/
/// `#include`/`mod`) are recorded so agents can answer "what depends on X?".
#[tauri::command]
pub async fn code_import(folder_path: String) -> Result<CodeImportReport, String> {
    let repo = CodeGraphRepository::open().map_err(|e| e.to_string())?;
    let dir = std::path::PathBuf::from(&folder_path);
    crate::core::knowledge::code_graph::import_code_directory(&repo, &dir)
        .map_err(|e| e.to_string())
}

/// List indexed source files, newest first.
#[tauri::command]
pub async fn code_list(limit: Option<u32>) -> Result<Vec<CodeFile>, String> {
    let repo = CodeGraphRepository::open().map_err(|e| e.to_string())?;
    repo.list(limit.unwrap_or(100)).map_err(|e| e.to_string())
}

/// Search symbols by name across all indexed files.
#[tauri::command]
pub async fn code_search(query: String, limit: Option<u32>) -> Result<Vec<SymbolHit>, String> {
    let repo = CodeGraphRepository::open().map_err(|e| e.to_string())?;
    repo.search_symbols(&query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

/// Dependencies of one indexed file (by path).
#[tauri::command]
pub async fn code_deps(
    path: String,
) -> Result<Vec<crate::core::knowledge::code_graph::CodeDependency>, String> {
    let repo = CodeGraphRepository::open().map_err(|e| e.to_string())?;
    repo.dependencies_of(&path).map_err(|e| e.to_string())
}

/// Files that depend on the given target (reverse edges, internal only).
#[tauri::command]
pub async fn code_dependents(target: String) -> Result<Vec<ReverseHit>, String> {
    let repo = CodeGraphRepository::open().map_err(|e| e.to_string())?;
    repo.dependents_of(&target).map_err(|e| e.to_string())
}

/// Code graph statistics for the UI / AGENTS.md generator.
#[tauri::command]
pub async fn code_stats() -> Result<CodeGraphStats, String> {
    let repo = CodeGraphRepository::open().map_err(|e| e.to_string())?;
    Ok(CodeGraphStats {
        file_count: repo.count().map_err(|e| e.to_string())?,
        symbol_count: repo.symbol_count().map_err(|e| e.to_string())?,
        dependency_count: repo.dependency_count().map_err(|e| e.to_string())?,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraphStats {
    pub file_count: u64,
    pub symbol_count: u64,
    pub dependency_count: u64,
}

// ═══════════════════════════════════════════════════════════════
//  Shared help (used by the AGENTS.md generator and copilot)
// ═══════════════════════════════════════════════════════════════

/// Command → description pairs shown in the generated AGENTS.md.
pub fn copilot_command_help() -> Vec<(&'static str, &'static str)> {
    vec![
        ("/memories", "list all memories"),
        ("/memory <id>", "get a memory"),
        ("/create-memory <title> <content>", "create a memory"),
        ("/search <query>", "search memories"),
        ("/graph", "graph statistics"),
        ("/entity <id>", "get an entity"),
        ("/create-entity <type> <title>", "create an entity"),
        ("/context <query>", "build a context package"),
        ("/stats", "database statistics"),
        ("/health", "system health"),
        ("/savings", "token savings"),
        ("/docs-import <folder>", "import project docs (RAG)"),
        ("/docs-search <query>", "search project docs"),
        ("/agents", "read AGENTS.md instructions"),
        ("/agents-generate", "generate AGENTS.md from live data"),
        ("/skills", "list skills"),
        ("/skill-run <name> [args...]", "run a skill"),
        (
            "/code-import <folder>",
            "index source files into the code graph",
        ),
        ("/code-search <symbol>", "search code symbols by name"),
        ("/code-deps <path>", "dependencies of a source file"),
        ("/code-dependents <target>", "files that depend on a target"),
    ]
}
