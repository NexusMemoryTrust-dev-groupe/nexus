use serde::Serialize;

use crate::core::knowledge::agent_passport::{
    AgentPassport, AgentRole, MemoryScope, PassportRepository, default_primary_passport,
    render_passport,
};
use crate::storage::sqlite::SqlitePassportRepository;

/// Serializable passport for Tauri IPC (same shape as the core struct).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPassportDto {
    pub name: String,
    pub display_name: String,
    pub role: String,
    pub description: String,
    pub skills: Vec<String>,
    pub tools: Vec<String>,
    pub constraints: Vec<String>,
    pub trust_level: u8,
    pub memory_scope: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&AgentPassport> for AgentPassportDto {
    fn from(p: &AgentPassport) -> Self {
        Self {
            name: p.name.clone(),
            display_name: p.display_name.clone(),
            role: p.role.as_str().to_string(),
            description: p.description.clone(),
            skills: p.skills.clone(),
            tools: p.tools.clone(),
            constraints: p.constraints.clone(),
            trust_level: p.trust_level,
            memory_scope: p.memory_scope.as_str().to_string(),
            is_active: p.is_active,
            created_at: p.created_at.clone(),
            updated_at: p.updated_at.clone(),
        }
    }
}

fn open_passport_repo() -> Result<SqlitePassportRepository, String> {
    let conn = crate::db::open_connection()?;
    SqlitePassportRepository::new(conn).map_err(|e| e.to_string())
}

/// Получить паспорт агента по имени (или паспорт по умолчанию, если нет).
#[tauri::command]
pub async fn passport_get(name: String) -> Result<AgentPassportDto, String> {
    let repo = open_passport_repo()?;
    let passport = repo
        .get_by_name(&name)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(default_primary_passport);
    Ok(AgentPassportDto::from(&passport))
}

/// Список всех паспортов агентов.
#[tauri::command]
pub async fn passport_list() -> Result<Vec<AgentPassportDto>, String> {
    let repo = open_passport_repo()?;
    let passports = repo.list(false).await.map_err(|e| e.to_string())?;
    Ok(passports.iter().map(AgentPassportDto::from).collect())
}

/// Создать или обновить паспорт агента.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn passport_upsert(
    name: String,
    display_name: Option<String>,
    role: Option<String>,
    description: Option<String>,
    skills: Option<Vec<String>>,
    tools: Option<Vec<String>>,
    constraints: Option<Vec<String>>,
    trust_level: Option<u8>,
    memory_scope: Option<String>,
) -> Result<AgentPassportDto, String> {
    if name.trim().is_empty() {
        return Err("Passport name must not be empty".to_string());
    }
    let repo = open_passport_repo()?;
    let existing = repo.get_by_name(&name).await.map_err(|e| e.to_string())?;
    let passport = match existing {
        Some(mut p) => {
            if let Some(d) = display_name {
                p.display_name = d;
            }
            if let Some(r) = role {
                p.role = AgentRole::parse(&r);
            }
            if let Some(d) = description {
                p.description = d;
            }
            if let Some(s) = skills {
                p.skills = s;
            }
            if let Some(t) = tools {
                p.tools = t;
            }
            if let Some(c) = constraints {
                p.constraints = c;
            }
            if let Some(t) = trust_level {
                p.trust_level = t.clamp(1, 10);
            }
            if let Some(m) = memory_scope {
                p.memory_scope = MemoryScope::parse(&m);
            }
            p
        }
        None => AgentPassport::new(
            &name,
            display_name.as_deref().unwrap_or(""),
            AgentRole::parse(role.as_deref().unwrap_or("generalist")),
            description.as_deref().unwrap_or(""),
            skills.unwrap_or_default(),
            tools.unwrap_or_default(),
            constraints.unwrap_or_default(),
            trust_level.unwrap_or(5).clamp(1, 10),
            MemoryScope::parse(memory_scope.as_deref().unwrap_or("project")),
        ),
    };
    repo.upsert(&passport).await.map_err(|e| e.to_string())?;
    Ok(AgentPassportDto::from(&passport))
}

/// Активировать/деактивировать паспорт.
#[tauri::command]
pub async fn passport_set_active(name: String, active: bool) -> Result<(), String> {
    let repo = open_passport_repo()?;
    repo.set_active(&name, active)
        .await
        .map_err(|e| e.to_string())
}

/// Удалить паспорт.
#[tauri::command]
pub async fn passport_delete(name: String) -> Result<(), String> {
    let repo = open_passport_repo()?;
    repo.delete(&name).await.map_err(|e| e.to_string())
}

/// Рендерит паспорт в markdown-блок для вставки в AGENTS.md / контекстный пакет.
#[tauri::command]
pub async fn passport_render(name: String) -> Result<String, String> {
    let repo = open_passport_repo()?;
    let passport = repo
        .get_by_name(&name)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(default_primary_passport);
    Ok(render_passport(&passport))
}
