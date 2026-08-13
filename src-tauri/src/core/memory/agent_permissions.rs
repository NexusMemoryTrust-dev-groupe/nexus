//! Agent-level memory permissions (Система 4, Firewall — второй контур).
//!
//! Спецификация: файрвол работает не только по данным (toxicity/spam/injection/
//! pii) — он понимает, **какому агенту какую память можно видеть**:
//!
//!   Claude Code:  architecture ✓  code ✓  decisions ✓  secrets ✗  personal ✗
//!   Другой агент: documentation ✓  architecture ✓  implementation ✗
//!
//! Это локальный policy engine (enterprise-grade, а не просто ACL): каждая
//! политика привязывает агента к набору разрешённых видимостей, слоёв памяти
//! и запрещённых паттернов (секреты, личные данные). Перед тем как память
//! попадёт в контекст LLM, конвейер становится:
//!
//!   Memory → Trust → Sensitivity → Scope → Freshness → Permission → LLM
//!
//! Модуль чистый: классификация чувствительности и вердикт доступа — чистые
//! функции над записью памяти, полностью юнит-тестируемые.

use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::types::{MemoryLayer, MemoryVisibility};

// ── Константы категорий чувствительности ──────────────────────────

/// Секреты: API-ключи, пароли, токены, credentials.
pub const CATEGORY_SECRETS: &str = "secrets";
/// Личные данные: email, телефоны, персональные заметки.
pub const CATEGORY_PERSONAL: &str = "personal";
/// Внутренняя архитектура: схемы, структура модулей, design decisions.
pub const CATEGORY_ARCHITECTURE: &str = "architecture";
/// Код и реализация: сниппеты, алгоритмы, конфиги.
pub const CATEGORY_CODE: &str = "code";
/// Технические решения: решения команды, ADR.
pub const CATEGORY_DECISIONS: &str = "decisions";
/// Документация: гайды, заметки, описания.
pub const CATEGORY_DOCUMENTATION: &str = "documentation";
/// Прочее.
pub const CATEGORY_OTHER: &str = "other";

/// Паттерны, указывающие на секреты/credentials (регистронезависимо).
const SECRET_PATTERNS: &[&str] = &[
    "api key",
    "api_key",
    "apikey",
    "secret",
    "password",
    "passwd",
    "token",
    "credential",
    "credential",
    "client secret",
    "bearer ",
    "private key",
    "access key",
    "auth secret",
    "секрет",
    "пароль",
    "ключ доступа",
    "токен",
    "авторизационные данные",
];

/// Паттерны, указывающие на личные данные (помимо pii-эвристик файрвола).
const PERSONAL_PATTERNS: &[&str] = &[
    "паспорт",
    "паспорта",
    "номер паспорта",
    "personal",
    "моё здоровье",
    "мой телефон",
    "моя почта",
    "мой адрес",
    "личное",
    "личный дневник",
    "private journal",
    "my address",
    "my phone",
    "my email",
];

/// Форма-детекция личных данных (email/телефон/паспорт) — те же сигналы,
/// что и в файрволе (`pii_score`), чтобы классификация совпадала с
/// фильтрацией при записи.
fn has_personal_shape(text: &str) -> bool {
    // Email: что-то@что-то.что-то (тримим завершающую пунктуацию).
    let has_email = text.split_whitespace().any(|w| {
        let trimmed = w.trim_end_matches(|c: char| !(c.is_alphanumeric() || c == '@'));
        trimmed.contains('@') && trimmed.contains('.')
    });
    if has_email {
        return true;
    }

    // Телефон: "+7" маркер или "8" + 11+ цифр суммарно.
    let all_digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    if all_digits.len() >= 11 && (text.contains("+7") || all_digits.starts_with('8')) {
        return true;
    }

    // Паспорт РФ: серия 4 цифры + номер 6 цифр (разделители пробел/тире).
    let norm = text.replace(['-', '\u{2013}'], " ");
    let words: Vec<&str> = norm.split_whitespace().collect();
    for w in words.windows(2) {
        let a_digits: String = w[0].chars().filter(|c| c.is_ascii_digit()).collect();
        let b_digits: String = w[1].chars().filter(|c| c.is_ascii_digit()).collect();
        if (a_digits.len() == 4 && b_digits.len() == 6)
            || (a_digits.len() == 6 && b_digits.len() == 4)
        {
            return true;
        }
    }

    false
}

/// Уровень чувствительности памяти (близко к спецификации
/// PRIVATE / RESTRICTED / PROJECT / PUBLIC).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Sensitivity {
    Public,
    Project,
    Restricted,
    Private,
}

impl Sensitivity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Sensitivity::Public => "public",
            Sensitivity::Project => "project",
            Sensitivity::Restricted => "restricted",
            Sensitivity::Private => "private",
        }
    }
}

/// Вердикт доступа агента к конкретной памяти.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessVerdict {
    Allow,
    Deny,
}

impl AccessVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccessVerdict::Allow => "allow",
            AccessVerdict::Deny => "deny",
        }
    }
}

/// Политика агента: какие видимости/слои/паттерны ему разрешены.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentPolicy {
    pub id: String,
    /// Имя агента, к которому привязана политика ("claude-code", "copilot").
    pub agent: String,
    /// Роль агента для отчёта ("assistant", "reviewer", "automation").
    pub role: String,
    /// Разрешённые видимости памяти. Пусто = всё разрешено.
    pub allowed_visibility: Vec<MemoryVisibility>,
    /// Разрешённые слои памяти. Пусто = всё разрешено.
    pub allowed_layers: Vec<MemoryLayer>,
    /// Запрещённые паттерны в title/summary/content (регистронезависимо).
    /// Срабатывание любого → Deny.
    pub deny_patterns: Vec<String>,
    pub enabled: bool,
    pub created_at: String,
}

/// Результат проверки доступа агента к памяти.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentAccessAssessment {
    pub verdict: AccessVerdict,
    /// Человекочитаемые причины (что именно запретило доступ).
    pub reasons: Vec<String>,
    /// Категории, к которым отнесена память (secrets/personal/...).
    pub categories: Vec<String>,
    /// Уровень чувствительности памяти.
    pub sensitivity: Sensitivity,
}

impl AgentAccessAssessment {
    pub fn allow(categories: Vec<String>, sensitivity: Sensitivity) -> Self {
        Self {
            verdict: AccessVerdict::Allow,
            reasons: Vec::new(),
            categories,
            sensitivity,
        }
    }

    pub fn deny(
        mut reasons: Vec<String>,
        categories: Vec<String>,
        sensitivity: Sensitivity,
    ) -> Self {
        reasons.sort();
        reasons.dedup();
        Self {
            verdict: AccessVerdict::Deny,
            reasons,
            categories,
            sensitivity,
        }
    }
}

/// Классифицирует память по категориям чувствительности.
///
/// Логика: секреты и личные данные определяются паттернами в содержимом;
/// архитектура/код/решения/документация — по слою памяти. Видимость
/// учитывается как верхний уровень.
pub fn classify_categories(record: &MemoryRecord) -> Vec<String> {
    let haystack = format!("{} {} {}", record.title, record.summary, record.content).to_lowercase();

    let mut cats: Vec<String> = Vec::new();
    let has_secret = SECRET_PATTERNS
        .iter()
        .any(|p| haystack.contains(&p.to_lowercase()));
    let has_personal = PERSONAL_PATTERNS
        .iter()
        .any(|p| haystack.contains(&p.to_lowercase()))
        || has_personal_shape(&haystack);

    if has_secret {
        cats.push(CATEGORY_SECRETS.to_string());
    }
    if has_personal {
        cats.push(CATEGORY_PERSONAL.to_string());
    }

    match record.layer {
        MemoryLayer::Working | MemoryLayer::Episodic => {}
        MemoryLayer::Semantic => {
            if !has_secret && !has_personal {
                cats.push(CATEGORY_ARCHITECTURE.to_string());
            }
        }
        MemoryLayer::Procedural => cats.push(CATEGORY_CODE.to_string()),
        MemoryLayer::Decision | MemoryLayer::Strategic => cats.push(CATEGORY_DECISIONS.to_string()),
    }

    if cats.is_empty() {
        cats.push(CATEGORY_DOCUMENTATION.to_string());
    }
    cats
}

/// Определяет уровень чувствительности памяти.
pub fn classify_sensitivity(record: &MemoryRecord) -> Sensitivity {
    // Видимость задаёт жёсткий потолок.
    match record.visibility {
        MemoryVisibility::Private => return Sensitivity::Private,
        MemoryVisibility::Restricted => return Sensitivity::Restricted,
        MemoryVisibility::Public => {}
    }
    // Внутри Public — категории секретов/личного поднимают уровень.
    let cats = classify_categories(record);
    if cats
        .iter()
        .any(|c| c == CATEGORY_SECRETS || c == CATEGORY_PERSONAL)
    {
        return Sensitivity::Private;
    }
    match record.layer {
        MemoryLayer::Decision | MemoryLayer::Strategic | MemoryLayer::Semantic => {
            Sensitivity::Restricted
        }
        _ => Sensitivity::Project,
    }
}

/// Проверяет доступ агента к памяти по его политике.
///
/// Вердикт:
/// - политика отключена → Deny (безопасность по умолчанию);
/// - видимость памяти не входит в разрешённые → Deny;
/// - слой памяти не входит в разрешённые (если список не пуст) → Deny;
/// - любой deny-паттерн найден в содержимом → Deny;
/// - иначе → Allow.
pub fn assess_agent_access(policy: &AgentPolicy, record: &MemoryRecord) -> AgentAccessAssessment {
    let categories = classify_categories(record);
    let sensitivity = classify_sensitivity(record);
    let mut reasons: Vec<String> = Vec::new();

    if !policy.enabled {
        reasons.push(format!("policy for agent '{}' is disabled", policy.agent));
    }

    if !policy.allowed_visibility.is_empty()
        && !policy.allowed_visibility.contains(&record.visibility)
    {
        reasons.push(format!(
            "visibility {:?} not allowed for agent '{}'",
            record.visibility, policy.agent
        ));
    }

    if !policy.allowed_layers.is_empty() && !policy.allowed_layers.contains(&record.layer) {
        reasons.push(format!(
            "layer {:?} not allowed for agent '{}'",
            record.layer, policy.agent
        ));
    }

    if !policy.deny_patterns.is_empty() {
        let haystack =
            format!("{} {} {}", record.title, record.summary, record.content).to_lowercase();
        for p in &policy.deny_patterns {
            if p.is_empty() {
                continue;
            }
            if haystack.contains(&p.to_lowercase()) {
                reasons.push(format!(
                    "deny pattern '{}' matched for agent '{}'",
                    p, policy.agent
                ));
            }
        }
    }

    if reasons.is_empty() {
        AgentAccessAssessment::allow(categories, sensitivity)
    } else {
        AgentAccessAssessment::deny(reasons, categories, sensitivity)
    }
}

/// Текстовый отчёт о политике (для MCP/copilot).
pub fn render_policy(policy: &AgentPolicy) -> String {
    let vis: Vec<String> = policy
        .allowed_visibility
        .iter()
        .map(|v| format!("{:?}", v).to_lowercase())
        .collect();
    let layers: Vec<String> = policy
        .allowed_layers
        .iter()
        .map(|l| format!("{:?}", l).to_lowercase())
        .collect();
    format!(
        "Agent '{}' ({}): visibility [{}], layers [{}], deny [{}], enabled {}",
        policy.agent,
        policy.role,
        if vis.is_empty() {
            "all".to_string()
        } else {
            vis.join(", ")
        },
        if layers.is_empty() {
            "all".to_string()
        } else {
            layers.join(", ")
        },
        if policy.deny_patterns.is_empty() {
            "none".to_string()
        } else {
            policy.deny_patterns.join(", ")
        },
        policy.enabled
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::memory_record::MemoryRecord;
    use crate::core::memory::types::MemorySource;

    fn record(
        title: &str,
        summary: &str,
        layer: MemoryLayer,
        vis: MemoryVisibility,
    ) -> MemoryRecord {
        let mut r = MemoryRecord::new(
            title.to_string(),
            summary.to_string(),
            "tester".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.summary = summary.to_string();
        r.layer = layer;
        r.visibility = vis;
        r
    }

    fn policy(agent: &str) -> AgentPolicy {
        AgentPolicy {
            id: "p1".to_string(),
            agent: agent.to_string(),
            role: "assistant".to_string(),
            allowed_visibility: vec![MemoryVisibility::Public, MemoryVisibility::Restricted],
            allowed_layers: vec![
                MemoryLayer::Semantic,
                MemoryLayer::Decision,
                MemoryLayer::Procedural,
            ],
            deny_patterns: vec![
                "api key".to_string(),
                "password".to_string(),
                "паспорт".to_string(),
            ],
            enabled: true,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn secrets_are_classified_as_secrets_category() {
        let r = record(
            "Auth",
            "The api key is stored in the vault",
            MemoryLayer::Semantic,
            MemoryVisibility::Public,
        );
        let cats = classify_categories(&r);
        assert!(cats.iter().any(|c| c == CATEGORY_SECRETS));
        assert_eq!(classify_sensitivity(&r), Sensitivity::Private);
    }

    #[test]
    fn personal_data_is_classified_as_personal() {
        let r = record(
            "Заметка",
            "Мой паспорт 4508 123456",
            MemoryLayer::Episodic,
            MemoryVisibility::Public,
        );
        let cats = classify_categories(&r);
        assert!(cats.iter().any(|c| c == CATEGORY_PERSONAL));
        assert_eq!(classify_sensitivity(&r), Sensitivity::Private);
    }

    #[test]
    fn phone_and_email_shapes_are_classified_as_personal() {
        // Телефон по форме (+7 + 11 цифр), без слова "мой".
        let phone = record(
            "HR note",
            "employee phone +7 900 123-45-67",
            MemoryLayer::Episodic,
            MemoryVisibility::Public,
        );
        let cats = classify_categories(&phone);
        assert!(
            cats.iter().any(|c| c == CATEGORY_PERSONAL),
            "phone shape must be personal, got {:?}",
            cats
        );

        // Email по форме (x@y.z), без слова "my".
        let email = record(
            "Contact",
            "reach alice@example.com anytime",
            MemoryLayer::Episodic,
            MemoryVisibility::Public,
        );
        let cats = classify_categories(&email);
        assert!(
            cats.iter().any(|c| c == CATEGORY_PERSONAL),
            "email shape must be personal, got {:?}",
            cats
        );
    }

    #[test]
    fn architecture_decision_is_restricted() {
        let r = record(
            "ADR",
            "We chose SQLite for storage",
            MemoryLayer::Decision,
            MemoryVisibility::Public,
        );
        let cats = classify_categories(&r);
        assert!(cats.iter().any(|c| c == CATEGORY_DECISIONS));
        assert_eq!(classify_sensitivity(&r), Sensitivity::Restricted);
    }

    #[test]
    fn code_layer_is_code_category() {
        let r = record(
            "Snippet",
            "fn main() { println!() }",
            MemoryLayer::Procedural,
            MemoryVisibility::Public,
        );
        let cats = classify_categories(&r);
        assert!(cats.iter().any(|c| c == CATEGORY_CODE));
    }

    #[test]
    fn private_visibility_is_never_visible() {
        let r = record(
            "Личное",
            "Мои мысли",
            MemoryLayer::Episodic,
            MemoryVisibility::Private,
        );
        let p = policy("claude-code");
        let a = assess_agent_access(&p, &r);
        assert_eq!(a.verdict, AccessVerdict::Deny);
        assert!(
            a.reasons.iter().any(|r| r.contains("visibility")),
            "private visibility must be denied, got {:?}",
            a.reasons
        );
    }

    #[test]
    fn deny_pattern_blocks_access() {
        let r = record(
            "Server config",
            "The default password for the admin is strong",
            MemoryLayer::Semantic,
            MemoryVisibility::Public,
        );
        let p = policy("claude-code");
        let a = assess_agent_access(&p, &r);
        assert_eq!(a.verdict, AccessVerdict::Deny);
        assert!(a.reasons.iter().any(|r| r.contains("deny pattern")));
    }

    #[test]
    fn allowed_memory_is_visible() {
        let r = record(
            "Decisions",
            "Authentication uses JWT access tokens",
            MemoryLayer::Decision,
            MemoryVisibility::Restricted,
        );
        let p = policy("claude-code");
        let a = assess_agent_access(&p, &r);
        assert_eq!(a.verdict, AccessVerdict::Allow);
        assert!(a.reasons.is_empty());
        assert_eq!(a.sensitivity, Sensitivity::Restricted);
    }

    #[test]
    fn disabled_policy_denies_everything() {
        let r = record(
            "Docs",
            "Some documentation",
            MemoryLayer::Semantic,
            MemoryVisibility::Public,
        );
        let mut p = policy("copilot");
        p.enabled = false;
        let a = assess_agent_access(&p, &r);
        assert_eq!(a.verdict, AccessVerdict::Deny);
    }

    #[test]
    fn empty_visibility_list_allows_all_visibilities() {
        let r = record(
            "Note",
            "A simple note",
            MemoryLayer::Episodic,
            MemoryVisibility::Private,
        );
        let mut p = policy("trusted");
        p.allowed_visibility.clear();
        p.allowed_layers.clear();
        p.deny_patterns.clear();
        let a = assess_agent_access(&p, &r);
        assert_eq!(a.verdict, AccessVerdict::Allow);
    }

    #[test]
    fn layer_restriction_blocks_wrong_layer() {
        let r = record(
            "Чужая память",
            "Working layer note",
            MemoryLayer::Working,
            MemoryVisibility::Public,
        );
        let p = policy("claude-code");
        let a = assess_agent_access(&p, &r);
        assert_eq!(a.verdict, AccessVerdict::Deny);
        assert!(a.reasons.iter().any(|r| r.contains("layer")));
    }
}
