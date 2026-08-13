//! Agent Passport — компактная идентификационная карточка агента (Система 6).
//!
//! Расширяет AGENTS.md (длинные инструкции) и скиллы: каждый агент получает
//! машиночитаемый паспорт — кто он, какую роль играет, какие скиллы и
//! инструменты ему доступны, чего он не должен делать и насколько его памяти
//! можно доверять. Паспорт прикрепляется к контекстному пакету и к
//! сгенерированному AGENTS.md, чтобы ИИ знал свои границы.
//!
//! Типы и чистые функции (рендер паспорта, скоринг доверия) живут здесь и
//! тестируются юнит-тестами; хранилище — `storage/sqlite/passport_repository_sqlite.rs`.

use serde::{Deserialize, Serialize};

use crate::core::result::Result;

/// Роль агента — определяет, какие паттерны поведения ожидаются.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    Generalist,
    Coder,
    Researcher,
    Reviewer,
    Orchestrator,
    MemoryKeeper,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Generalist => "generalist",
            Self::Coder => "coder",
            Self::Researcher => "researcher",
            Self::Reviewer => "reviewer",
            Self::Orchestrator => "orchestrator",
            Self::MemoryKeeper => "memory-keeper",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "coder" => Self::Coder,
            "researcher" => Self::Researcher,
            "reviewer" => Self::Reviewer,
            "orchestrator" => Self::Orchestrator,
            "memory-keeper" => Self::MemoryKeeper,
            _ => Self::Generalist,
        }
    }
}

/// Область памяти, к которой агент имеет доступ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryScope {
    Personal,
    Project,
    Team,
    Global,
}

impl MemoryScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Project => "project",
            Self::Team => "team",
            Self::Global => "global",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "personal" => Self::Personal,
            "team" => Self::Team,
            "global" => Self::Global,
            _ => Self::Project,
        }
    }
}

/// Машиночитаемый паспорт агента.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPassport {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub role: AgentRole,
    pub description: String,
    pub skills: Vec<String>,
    pub tools: Vec<String>,
    pub constraints: Vec<String>,
    pub trust_level: u8,
    pub memory_scope: MemoryScope,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl AgentPassport {
    /// Создать новый паспорт с текущим временем.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &str,
        display_name: &str,
        role: AgentRole,
        description: &str,
        skills: Vec<String>,
        tools: Vec<String>,
        constraints: Vec<String>,
        trust_level: u8,
        memory_scope: MemoryScope,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: crate::core::entity_id::EntityId::new().to_string(),
            name: name.to_string(),
            display_name: display_name.to_string(),
            role,
            description: description.to_string(),
            skills,
            tools,
            constraints,
            trust_level: trust_level.clamp(1, 10),
            memory_scope,
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// Репозиторий паспортов агентов.
#[async_trait::async_trait]
pub trait PassportRepository: Send + Sync {
    /// Создать или обновить паспорт по имени.
    async fn upsert(&self, passport: &AgentPassport) -> Result<()>;

    /// Получить паспорт по имени агента.
    async fn get_by_name(&self, name: &str) -> Result<Option<AgentPassport>>;

    /// Все паспорты (опционально только активные).
    async fn list(&self, active_only: bool) -> Result<Vec<AgentPassport>>;

    /// Активировать/деактивировать паспорт.
    async fn set_active(&self, name: &str, active: bool) -> Result<()>;

    /// Удалить паспорт.
    async fn delete(&self, name: &str) -> Result<()>;
}

/// Рендерит паспорт в компактный markdown-блок, который прикрепляется к
/// контекстному пакету или AGENTS.md. Чистая функция — тестируется без БД.
pub fn render_passport(passport: &AgentPassport) -> String {
    let mut out = String::with_capacity(1024);
    out.push_str(&format!("## Agent Passport: {}\n\n", passport.name));
    if !passport.display_name.is_empty() {
        out.push_str(&format!("**{}** — ", passport.display_name));
    }
    out.push_str(&format!(
        "role `{}`, memory scope `{}`, trust {}/10.\n\n",
        passport.role.as_str(),
        passport.memory_scope.as_str(),
        passport.trust_level
    ));
    if !passport.description.is_empty() {
        out.push_str(&format!("{}\n\n", passport.description));
    }
    if !passport.skills.is_empty() {
        out.push_str("Available skills:\n");
        for s in &passport.skills {
            out.push_str(&format!("- `{s}`\n"));
        }
        out.push('\n');
    }
    if !passport.tools.is_empty() {
        out.push_str("Allowed tools:\n");
        for t in &passport.tools {
            out.push_str(&format!("- `{t}`\n"));
        }
        out.push('\n');
    }
    if !passport.constraints.is_empty() {
        out.push_str("Constraints (must NOT do):\n");
        for c in &passport.constraints {
            out.push_str(&format!("- {c}\n"));
        }
        out.push('\n');
    }
    out
}

/// Паспорт по умолчанию для первого агента (opencode-primary): главный
/// исполнитель, который видит весь стек экосистемы.
pub fn default_primary_passport() -> AgentPassport {
    AgentPassport::new(
        "opencode-primary",
        "Primary Agent",
        AgentRole::Generalist,
        "Primary coding agent operating inside the Nexus ecosystem. Coordinates memory, skills, tools and context.",
        vec![
            "memory".to_string(),
            "graph".to_string(),
            "context".to_string(),
            "docs".to_string(),
            "code".to_string(),
        ],
        vec![],
        vec![
            "Never invent data; state explicitly when information is missing".to_string(),
            "Ground answers in Nexus memory and project docs when available".to_string(),
        ],
        5,
        MemoryScope::Project,
    )
}

/// Скоринг доверия: насколько паспорт согласован внутренне.
/// Чистая функция для тестов — проверяет, что доверие в диапазоне и имя не пустое.
pub fn trust_score(passport: &AgentPassport) -> u8 {
    let mut score = passport.trust_level.clamp(1, 10);
    if passport.name.trim().is_empty() {
        score = score.saturating_sub(3);
    }
    if passport.skills.is_empty() {
        score = score.saturating_sub(1);
    }
    score
}

// ── Identity ≠ Authorization (plan 4.6) ──────────────────────────────
//
// The passport answers "WHO is asking". It is identification, not a grant.
// A tool may be used only when it is EXPLICITLY listed in `passport.tools`,
// and a memory category may be read only when it is EXPLICITLY granted —
// neither the role, nor the name, nor the trust level, nor mere possession
// of a passport unlocks anything by itself. This is what "identity is not
// authorization" means and what the isolation tests below enforce.

/// Whether the agent may invoke the given tool.
///
/// Only an explicit entry in `passport.tools` grants the right. A high
/// `trust_level`, a `skills` entry or the agent's own `name` grant nothing.
pub fn can_use_tool(passport: &AgentPassport, tool: &str) -> bool {
    passport.tools.iter().any(|t| t == tool)
}

/// The memory categories the agent is authorized to read.
///
/// Derived strictly from explicitly granted tools (each authorized tool maps
/// to the categories it needs). Nothing is implied from role/name/trust.
pub fn authorized_categories(passport: &AgentPassport) -> Vec<String> {
    // Every explicitly allowed tool implies the categories it reads. The
    // mapping is the *only* way categories enter this list — a brand-new
    // tool name grants nothing.
    let mut categories = Vec::new();
    for tool in &passport.tools {
        match tool.as_str() {
            "nexus_memory_search" | "nexus_memory_read" | "nexus_context" => {
                push_unique(&mut categories, "architecture");
                push_unique(&mut categories, "code");
                push_unique(&mut categories, "decisions");
                push_unique(&mut categories, "documentation");
            }
            "nexus_memory_write" | "nexus_memory_update" => {
                push_unique(&mut categories, "write");
            }
            "nexus_secrets_read" => {
                push_unique(&mut categories, "secrets");
                push_unique(&mut categories, "personal");
            }
            _ => {} // unknown tool: grants nothing (fail closed)
        }
    }
    categories
}

fn push_unique(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.to_string());
    }
}

/// Full authorization decision for one memory access: identity (who) plus
/// explicit grants (what). Returns `true` only when the tool is allowed AND
/// the passport is active.
pub fn is_authorized(passport: &AgentPassport, tool: &str) -> bool {
    passport.is_active && can_use_tool(passport, tool)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AgentPassport {
        AgentPassport::new(
            "coder-alpha",
            "Coder Alpha",
            AgentRole::Coder,
            "Writes and refactors Rust code.",
            vec!["rust".to_string(), "test".to_string()],
            vec!["nexus_memory_search".to_string()],
            vec!["Never delete memories without confirmation".to_string()],
            8,
            MemoryScope::Project,
        )
    }

    #[test]
    fn role_str_roundtrip() {
        let roles = [
            AgentRole::Generalist,
            AgentRole::Coder,
            AgentRole::Researcher,
            AgentRole::Reviewer,
            AgentRole::Orchestrator,
            AgentRole::MemoryKeeper,
        ];
        for r in roles {
            assert_eq!(AgentRole::parse(r.as_str()), r);
        }
        assert_eq!(AgentRole::parse("bogus"), AgentRole::Generalist);
    }

    #[test]
    fn scope_str_roundtrip() {
        assert_eq!(MemoryScope::parse("personal"), MemoryScope::Personal);
        assert_eq!(MemoryScope::parse("project"), MemoryScope::Project);
        assert_eq!(MemoryScope::parse("team"), MemoryScope::Team);
        assert_eq!(MemoryScope::parse("global"), MemoryScope::Global);
        assert_eq!(MemoryScope::parse("x"), MemoryScope::Project);
    }

    #[test]
    fn new_passport_is_active_and_clamped() {
        let p = AgentPassport::new(
            "a",
            "",
            AgentRole::Generalist,
            "",
            vec![],
            vec![],
            vec![],
            99,
            MemoryScope::Global,
        );
        assert!(p.is_active);
        assert_eq!(p.trust_level, 10, "trust clamped to max");
        assert_eq!(p.memory_scope, MemoryScope::Global);
        assert!(!p.id.is_empty());
    }

    #[test]
    fn render_contains_sections() {
        let p = sample();
        let text = render_passport(&p);
        assert!(text.contains("Agent Passport: coder-alpha"));
        assert!(text.contains("role `coder`"));
        assert!(text.contains("trust 8/10"));
        assert!(text.contains("`rust`"));
        assert!(text.contains("nexus_memory_search"));
        assert!(text.contains("Constraints"));
    }

    #[test]
    fn render_omits_empty_sections() {
        let p = AgentPassport::new(
            "minimal",
            "",
            AgentRole::Generalist,
            "",
            vec![],
            vec![],
            vec![],
            5,
            MemoryScope::Project,
        );
        let text = render_passport(&p);
        assert!(!text.contains("Available skills"));
        assert!(!text.contains("Allowed tools"));
        assert!(!text.contains("Constraints"));
    }

    #[test]
    fn default_primary_has_identity() {
        let p = default_primary_passport();
        assert_eq!(p.name, "opencode-primary");
        assert_eq!(p.role, AgentRole::Generalist);
        assert_eq!(p.trust_level, 5);
        assert!(!p.skills.is_empty());
    }

    #[test]
    fn trust_score_reflects_integrity() {
        let good = sample();
        assert_eq!(trust_score(&good), 8);

        let mut empty_name = sample();
        empty_name.name = "".to_string();
        assert_eq!(trust_score(&empty_name), 5);

        let mut no_skills = sample();
        no_skills.skills = vec![];
        assert_eq!(trust_score(&no_skills), 7);
    }

    // ── Identity ≠ Authorization (plan 4.6) ─────────────────────────

    #[test]
    fn tool_requires_explicit_grant() {
        // The passport's identity (name, role, high trust) grants NOTHING —
        // only an explicit tools entry does.
        let p = sample(); // tools: ["nexus_memory_search"]
        assert!(can_use_tool(&p, "nexus_memory_search"));
        assert!(
            !can_use_tool(&p, "nexus_memory_write"),
            "write is not granted, even with trust 8 and a coder role"
        );
        assert!(
            !can_use_tool(&p, "nexus_secrets_read"),
            "secrets are never implied by identity"
        );
    }

    #[test]
    fn high_trust_does_not_imply_access() {
        // The strongest possible identity (trust 10, Generalist, active) still
        // cannot use a tool that was never granted.
        let mut p = sample();
        p.trust_level = 10;
        p.tools = vec![];
        assert_eq!(trust_score(&p), 10);
        assert!(
            !can_use_tool(&p, "nexus_memory_search"),
            "trust 10 with empty tools must deny everything"
        );
        assert!(!is_authorized(&p, "nexus_memory_search"));
    }

    #[test]
    fn role_and_skills_do_not_imply_categories() {
        // A coder with the "rust" skill is not automatically allowed to read
        // secrets or even architecture — categories come from explicit tools.
        let p = sample();
        let cats = authorized_categories(&p);
        assert!(cats.contains(&"architecture".to_string()));
        assert!(
            !cats.contains(&"secrets".to_string()),
            "secrets require an explicit grant, never role/skills"
        );
    }

    #[test]
    fn unknown_tool_grants_nothing_fail_closed() {
        // A brand-new tool name is NOT in the mapping → zero categories.
        let mut p = sample();
        p.tools = vec!["brand_new_tool".to_string()];
        assert!(
            can_use_tool(&p, "brand_new_tool"),
            "tool is allowed by name"
        );
        assert!(
            authorized_categories(&p).is_empty(),
            "unknown tool must map to NO categories (fail closed)"
        );
    }

    #[test]
    fn secrets_grant_comes_only_from_explicit_tool() {
        let mut p = sample();
        p.tools = vec!["nexus_secrets_read".to_string()];
        let cats = authorized_categories(&p);
        assert!(cats.contains(&"secrets".to_string()));
        assert!(cats.contains(&"personal".to_string()));
    }

    #[test]
    fn deactivated_passport_is_not_authorized() {
        let mut p = sample();
        p.is_active = false;
        assert!(
            !is_authorized(&p, "nexus_memory_search"),
            "an inactive identity must not be authorized even for granted tools"
        );
        assert!(
            can_use_tool(&p, "nexus_memory_search"),
            "grant survives, but…"
        );
        assert!(
            !is_authorized(&p, "nexus_memory_search"),
            "…authorization does not"
        );
    }

    #[test]
    fn write_permission_is_separate_from_read() {
        let mut reader = sample();
        reader.tools = vec!["nexus_memory_search".to_string()];
        let mut writer = sample();
        writer.tools = vec!["nexus_memory_write".to_string()];

        let reader_cats = authorized_categories(&reader);
        assert!(!reader_cats.contains(&"write".to_string()));
        let writer_cats = authorized_categories(&writer);
        assert!(writer_cats.contains(&"write".to_string()));
        // The writer cannot read what the reader can — both directions are
        // explicit, nothing is implied by sharing an identity shape.
        assert!(
            !writer_cats.contains(&"code".to_string()),
            "a write-only grant must not imply read access"
        );
    }
}
