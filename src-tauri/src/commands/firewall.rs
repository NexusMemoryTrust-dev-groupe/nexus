use serde::Serialize;

use crate::core::memory::agent_permissions::{
    AgentAccessAssessment, AgentPolicy, Sensitivity, assess_agent_access, classify_categories,
    classify_sensitivity, render_policy,
};
use crate::core::memory::memory_firewall::{
    FirewallAction, FirewallAssessment, FirewallRepository, FirewallRule, FirewallScores,
    FirewallVerdict, QuarantineEntry, QuarantineStatus, assess_with_rules,
};
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::{MemoryLayer, MemoryVisibility};
use crate::storage::sqlite::SqliteFirewallRepository;

/// Serializable assessment for Tauri IPC (preview without persisting).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirewallAssessmentDto {
    pub verdict: String,
    pub toxicity: f64,
    pub spam: f64,
    pub injection: f64,
    pub pii: f64,
    pub reasons: Vec<String>,
    pub matched_rule_ids: Vec<String>,
}

impl From<&FirewallAssessment> for FirewallAssessmentDto {
    fn from(a: &FirewallAssessment) -> Self {
        Self {
            verdict: a.verdict.as_str().to_string(),
            toxicity: a.scores.toxicity,
            spam: a.scores.spam,
            injection: a.scores.injection,
            pii: a.scores.pii,
            reasons: a.reasons.clone(),
            matched_rule_ids: a.matched_rule_ids.clone(),
        }
    }
}

/// Serializable quarantine entry for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineEntryDto {
    pub id: String,
    pub title: String,
    pub content: String,
    pub author: String,
    pub source: String,
    pub reasons: Vec<String>,
    pub scores: FirewallScores,
    pub status: String,
    pub created_at: String,
    pub decided_at: Option<String>,
}

impl From<&QuarantineEntry> for QuarantineEntryDto {
    fn from(e: &QuarantineEntry) -> Self {
        Self {
            id: e.id.clone(),
            title: e.title.clone(),
            content: e.content.clone(),
            author: e.author.clone(),
            source: e.source.clone(),
            reasons: e.reasons.clone(),
            scores: e.scores,
            status: e.status.as_str().to_string(),
            created_at: e.created_at.clone(),
            decided_at: e.decided_at.clone(),
        }
    }
}

fn open_firewall_repo() -> Result<SqliteFirewallRepository, String> {
    let conn = crate::db::open_connection()?;
    SqliteFirewallRepository::new(conn).map_err(|e| e.to_string())
}

/// Предварительная проверка контента без записи в память. Ничего не
/// сохраняет — только возвращает вердикт и скоринги (для UI/MCP-превью).
#[tauri::command]
pub async fn firewall_check(
    title: String,
    content: String,
) -> Result<FirewallAssessmentDto, String> {
    let repo = open_firewall_repo()?;
    let rules = repo.list_rules().await.map_err(|e| e.to_string())?;
    let assessment = assess_with_rules(&title, &content, &rules);
    Ok(FirewallAssessmentDto::from(&assessment))
}

/// Список пользовательских правил файрвола.
#[tauri::command]
pub async fn firewall_rules() -> Result<Vec<FirewallRule>, String> {
    let repo = open_firewall_repo()?;
    repo.list_rules().await.map_err(|e| e.to_string())
}

/// Добавляет пользовательское правило: при совпадении паттерна контент
/// блокируется или уходит в карантин.
#[tauri::command]
pub async fn firewall_rule_add(
    pattern: String,
    action: String,
    reason: Option<String>,
) -> Result<FirewallRule, String> {
    if pattern.trim().is_empty() {
        return Err("Firewall rule pattern must not be empty".to_string());
    }
    let action_enum = match action.as_str() {
        "quarantine" => FirewallAction::Quarantine,
        "block" => FirewallAction::Block,
        other => {
            return Err(format!(
                "Unknown firewall action '{other}' (expected block|quarantine)"
            ));
        }
    };
    let repo = open_firewall_repo()?;
    let rule = FirewallRule {
        id: crate::core::entity_id::EntityId::new().as_str().to_string(),
        pattern: pattern.trim().to_string(),
        action: action_enum,
        enabled: true,
        reason: reason.unwrap_or_default(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    repo.add_rule(&rule).await.map_err(|e| e.to_string())?;
    Ok(rule)
}

/// Удаляет правило по id.
#[tauri::command]
pub async fn firewall_rule_delete(id: String) -> Result<(), String> {
    let repo = open_firewall_repo()?;
    repo.delete_rule(&id).await.map_err(|e| e.to_string())
}

/// Включает/выключает правило.
#[tauri::command]
pub async fn firewall_rule_set_enabled(id: String, enabled: bool) -> Result<(), String> {
    let repo = open_firewall_repo()?;
    repo.set_rule_enabled(&id, enabled)
        .await
        .map_err(|e| e.to_string())
}

/// Список карантина (опционально по статусу: pending|approved|rejected).
#[tauri::command]
pub async fn quarantine_list(status: Option<String>) -> Result<Vec<QuarantineEntryDto>, String> {
    let repo = open_firewall_repo()?;
    let filter = match status.as_deref() {
        None | Some("") | Some("all") => None,
        Some("pending") => Some(QuarantineStatus::Pending),
        Some("approved") => Some(QuarantineStatus::Approved),
        Some("rejected") => Some(QuarantineStatus::Rejected),
        Some(other) => return Err(format!("Unknown quarantine status '{other}'")),
    };
    let entries = repo
        .list_quarantine(filter)
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.iter().map(QuarantineEntryDto::from).collect())
}

/// Одобряет карантинную запись: создаёт из неё полноценную память
/// (минуя файрвол — пользователь явно подтвердил контент).
#[tauri::command]
pub async fn quarantine_approve(id: String) -> Result<crate::commands::memory::MemoryDto, String> {
    let fw_repo = open_firewall_repo()?;
    let entry = fw_repo
        .get_quarantine(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Quarantine entry {} not found", id))?;
    if entry.status != QuarantineStatus::Pending {
        return Err(format!(
            "Quarantine entry {} is already {}",
            id,
            entry.status.as_str()
        ));
    }

    // Recreate the memory exactly as the firewall would have allowed it.
    let memory_repo = open_memory_repo()?;
    let mut record = crate::core::memory::memory_record::MemoryRecord::new(
        entry.title.clone(),
        entry.content.clone(),
        entry.author.clone(),
        parse_memory_source(&entry.source),
    )
    .map_err(|e| e.to_string())?;
    crate::core::memory::memory_lifecycle::auto_classify(&mut record);
    memory_repo.save(&record).await.map_err(|e| e.to_string())?;

    // Conflict check + reconcile, mirroring create_memory.
    crate::core::memory::memory_lifecycle::detect_and_mark_conflicts(&memory_repo, &record)
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = crate::commands::conflict::sync_conflict_groups().await {
        eprintln!(
            "[nexus] conflict reconcile on quarantine approve failed: {}",
            e
        );
    }
    crate::core::context::indexer::spawn_index_memory(
        &record.id,
        &record.title,
        &record.summary,
        &record.content,
    );

    fw_repo
        .set_quarantine_status(&id, QuarantineStatus::Approved)
        .await
        .map_err(|e| e.to_string())?;

    Ok(crate::commands::memory::MemoryDto::from(record))
}

/// Отклоняет карантинную запись: контент не попадает в память.
#[tauri::command]
pub async fn quarantine_reject(id: String) -> Result<(), String> {
    let repo = open_firewall_repo()?;
    let entry = repo
        .get_quarantine(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Quarantine entry {} not found", id))?;
    if entry.status != QuarantineStatus::Pending {
        return Err(format!(
            "Quarantine entry {} is already {}",
            id,
            entry.status.as_str()
        ));
    }
    repo.set_quarantine_status(&id, QuarantineStatus::Rejected)
        .await
        .map_err(|e| e.to_string())
}

/// Экран входящего контента: вызывается перед сохранением памяти из
/// Tauri-команд и copilot.
///
/// * `Allow` — можно сохранять;
/// * `Block` — возвращает `Err` с причинами (жёсткая блокировка);
/// * `Quarantine` — сохраняет контент в карантин и возвращает `Err` с id
///   записи, чтобы вызывающий мог сообщить пользователю.
pub async fn screen_ingress(
    title: &str,
    content: &str,
    author: &str,
    source: &str,
) -> Result<(), String> {
    let repo = open_firewall_repo()?;
    let rules = repo.list_rules().await.map_err(|e| e.to_string())?;
    let assessment = assess_with_rules(title, content, &rules);

    match assessment.verdict {
        FirewallVerdict::Allow => Ok(()),
        FirewallVerdict::Block => Err(format!(
            "Memory Firewall: content blocked: {}",
            assessment.reasons.join("; ")
        )),
        FirewallVerdict::Quarantine => {
            let entry = QuarantineEntry::new(
                title.to_string(),
                content.to_string(),
                author.to_string(),
                source.to_string(),
                &assessment,
            );
            let qid = repo
                .add_quarantine(&entry)
                .await
                .map_err(|e| e.to_string())?;
            Err(format!(
                "Memory Firewall: content quarantined (id={}): {}. \
                 Approve via /quarantine approve {} to save as memory.",
                qid,
                assessment.reasons.join("; "),
                qid
            ))
        }
    }
}

fn open_memory_repo() -> Result<crate::storage::sqlite::SqliteMemoryRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteMemoryRepository::new(conn).map_err(|e| e.to_string())
}

fn parse_memory_source(s: &str) -> crate::core::memory::types::MemorySource {
    match s {
        "Manual" => crate::core::memory::types::MemorySource::Manual,
        "Git" => crate::core::memory::types::MemorySource::Git,
        "Telegram" => crate::core::memory::types::MemorySource::Telegram,
        "Email" => crate::core::memory::types::MemorySource::Email,
        "Meeting" => crate::core::memory::types::MemorySource::Meeting,
        "Document" => crate::core::memory::types::MemorySource::Document,
        "AiGenerated" => crate::core::memory::types::MemorySource::AiGenerated,
        "Compressed" => crate::core::memory::types::MemorySource::Compressed,
        _ => crate::core::memory::types::MemorySource::Manual,
    }
}

// ── Agent-level memory permissions (second Firewall ring) ───────────

/// Serializable agent policy for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPolicyDto {
    pub id: String,
    pub agent: String,
    pub role: String,
    pub allowed_visibility: Vec<String>,
    pub allowed_layers: Vec<String>,
    pub deny_patterns: Vec<String>,
    pub enabled: bool,
    pub created_at: String,
}

impl From<&AgentPolicy> for AgentPolicyDto {
    fn from(p: &AgentPolicy) -> Self {
        Self {
            id: p.id.clone(),
            agent: p.agent.clone(),
            role: p.role.clone(),
            allowed_visibility: p
                .allowed_visibility
                .iter()
                .map(|v| format!("{:?}", v))
                .collect(),
            allowed_layers: p
                .allowed_layers
                .iter()
                .map(|l| format!("{:?}", l))
                .collect(),
            deny_patterns: p.deny_patterns.clone(),
            enabled: p.enabled,
            created_at: p.created_at.clone(),
        }
    }
}

/// Serializable access assessment for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAccessAssessmentDto {
    pub verdict: String,
    pub reasons: Vec<String>,
    pub categories: Vec<String>,
    pub sensitivity: String,
}

impl From<&AgentAccessAssessment> for AgentAccessAssessmentDto {
    fn from(a: &AgentAccessAssessment) -> Self {
        Self {
            verdict: a.verdict.as_str().to_string(),
            reasons: a.reasons.clone(),
            categories: a.categories.clone(),
            sensitivity: a.sensitivity.as_str().to_string(),
        }
    }
}

fn parse_visibility(s: &str) -> Option<MemoryVisibility> {
    match s.trim().to_lowercase().as_str() {
        "public" => Some(MemoryVisibility::Public),
        "private" => Some(MemoryVisibility::Private),
        "restricted" => Some(MemoryVisibility::Restricted),
        _ => None,
    }
}

fn parse_layer(s: &str) -> Option<MemoryLayer> {
    match s.trim().to_lowercase().as_str() {
        "working" => Some(MemoryLayer::Working),
        "episodic" => Some(MemoryLayer::Episodic),
        "semantic" => Some(MemoryLayer::Semantic),
        "procedural" => Some(MemoryLayer::Procedural),
        "decision" => Some(MemoryLayer::Decision),
        "strategic" => Some(MemoryLayer::Strategic),
        _ => None,
    }
}

/// Create or update an agent policy (who may see what memory).
///
/// Visibility: public, private, restricted (comma-separated; empty = all).
/// Layers: working, episodic, semantic, procedural, decision, strategic
/// (comma-separated; empty = all).
/// Deny patterns: comma-separated substrings; any match in title/summary/
/// content → the agent is denied access.
#[tauri::command]
pub async fn agent_policy_add(
    agent: String,
    role: Option<String>,
    allowed_visibility: Option<String>,
    allowed_layers: Option<String>,
    deny_patterns: Option<String>,
) -> Result<AgentPolicyDto, String> {
    if agent.trim().is_empty() {
        return Err("agent name must not be empty".to_string());
    }
    let repo = open_firewall_repo()?;

    let vis: Vec<MemoryVisibility> = allowed_visibility
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            parse_visibility(s).ok_or_else(|| {
                format!(
                    "unknown visibility '{}' (expected public|private|restricted)",
                    s.trim()
                )
            })
        })
        .collect::<Result<_, _>>()?;
    let layers: Vec<MemoryLayer> = allowed_layers
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            parse_layer(s).ok_or_else(|| {
                format!(
                    "unknown layer '{}' (expected working|episodic|semantic|procedural|decision|strategic)",
                    s.trim()
                )
            })
        })
        .collect::<Result<_, _>>()?;
    let deny: Vec<String> = deny_patterns
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let existing = repo
        .get_policy_for_agent(&agent)
        .map_err(|e| e.to_string())?;
    let policy = match existing {
        Some(mut p) => {
            p.role = role.clone().unwrap_or(p.role);
            if !allowed_visibility.as_deref().unwrap_or("").is_empty() {
                p.allowed_visibility = vis;
            }
            if !allowed_layers.as_deref().unwrap_or("").is_empty() {
                p.allowed_layers = layers;
            }
            if !deny_patterns.as_deref().unwrap_or("").is_empty() {
                p.deny_patterns = deny;
            }
            p
        }
        None => AgentPolicy {
            id: crate::core::entity_id::EntityId::new().as_str().to_string(),
            agent: agent.trim().to_string(),
            role: role.clone().unwrap_or_else(|| "assistant".to_string()),
            allowed_visibility: vis,
            allowed_layers: layers,
            deny_patterns: deny,
            enabled: true,
            created_at: chrono::Utc::now().to_rfc3339(),
        },
    };
    repo.save_policy(&policy).map_err(|e| e.to_string())?;
    Ok(AgentPolicyDto::from(&policy))
}

/// List all agent policies (who may see what memory).
#[tauri::command]
pub async fn agent_policy_list() -> Result<Vec<AgentPolicyDto>, String> {
    let repo = open_firewall_repo()?;
    let policies = repo.list_policies().map_err(|e| e.to_string())?;
    Ok(policies.iter().map(AgentPolicyDto::from).collect())
}

/// Delete an agent policy by id.
#[tauri::command]
pub async fn agent_policy_delete(id: String) -> Result<(), String> {
    let repo = open_firewall_repo()?;
    repo.delete_policy(&id).map_err(|e| e.to_string())
}

/// Check whether an agent may see a given memory (by memory id).
#[tauri::command]
pub async fn agent_access_check(
    agent: String,
    memory_id: String,
) -> Result<AgentAccessAssessmentDto, String> {
    let firewall = open_firewall_repo()?;
    let policy = firewall
        .get_policy_for_agent(&agent)
        .map_err(|e| e.to_string())?;

    let mem_conn = crate::db::open_connection()?;
    let memory =
        crate::storage::sqlite::SqliteMemoryRepository::new(mem_conn).map_err(|e| e.to_string())?;
    let eid = crate::core::EntityId::parse(&memory_id).map_err(|e| e.to_string())?;
    let record = memory
        .get_by_id(&eid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory '{}' not found", memory_id))?;

    // Safety default: a missing or disabled policy means Deny, never silent allow.
    let assessment = match policy {
        Some(p) => assess_agent_access(&p, &record),
        None => {
            let categories = classify_categories(&record);
            let sensitivity = classify_sensitivity(&record);
            AgentAccessAssessment::deny(
                vec![format!(
                    "no policy configured for agent '{}' — denied by default",
                    agent
                )],
                categories,
                sensitivity,
            )
        }
    };
    Ok(AgentAccessAssessmentDto::from(&assessment))
}

/// Render the policies as text (for MCP/copilot).
#[tauri::command]
pub async fn render_agent_policies() -> Result<String, String> {
    let repo = open_firewall_repo()?;
    let policies = repo.list_policies().map_err(|e| e.to_string())?;
    if policies.is_empty() {
        return Ok(
            "No agent policies yet — add one via agent_policy_add (agent, visibility, layers, deny patterns)."
                .to_string(),
        );
    }
    let mut out = String::from("Agent-level memory permissions:\n");
    for p in &policies {
        out.push_str(&format!("  {}\n", render_policy(p)));
    }
    Ok(out)
}

/// Classify a memory's sensitivity level (for MCP/copilot).
#[tauri::command]
pub async fn memory_sensitivity(memory_id: String) -> Result<String, String> {
    let mem_conn = crate::db::open_connection()?;
    let memory =
        crate::storage::sqlite::SqliteMemoryRepository::new(mem_conn).map_err(|e| e.to_string())?;
    let eid = crate::core::EntityId::parse(&memory_id).map_err(|e| e.to_string())?;
    let record = memory
        .get_by_id(&eid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory '{}' not found", memory_id))?;
    let sens: Sensitivity = classify_sensitivity(&record);
    Ok(sens.as_str().to_string())
}
