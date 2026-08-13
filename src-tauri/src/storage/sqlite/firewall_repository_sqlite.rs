use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::memory::agent_permissions::AgentPolicy;
use crate::core::memory::memory_firewall::{
    FirewallAction, FirewallRepository, FirewallRule, FirewallScores, QuarantineEntry,
    QuarantineStatus,
};
use crate::core::result::Result;
use crate::storage::sqlite::schema;

/// SQLite-backed implementation of the Memory Firewall repository
/// (rules + quarantine, System 4). Same Mutex<Connection> pattern as the
/// other repositories.
pub struct SqliteFirewallRepository {
    conn: Mutex<Connection>,
}

impl SqliteFirewallRepository {
    pub fn new(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        schema::apply_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::new(conn)
    }
}

fn row_to_rule(row: &rusqlite::Row) -> rusqlite::Result<FirewallRule> {
    let id: String = row.get(0)?;
    let pattern: String = row.get(1)?;
    let action: String = row.get(2)?;
    let enabled: i64 = row.get(3)?;
    let reason: String = row.get(4)?;
    let created_at: String = row.get(5)?;
    Ok(FirewallRule {
        id,
        pattern,
        action: match action.as_str() {
            "quarantine" => FirewallAction::Quarantine,
            _ => FirewallAction::Block,
        },
        enabled: enabled != 0,
        reason,
        created_at,
    })
}

fn row_to_quarantine(row: &rusqlite::Row) -> rusqlite::Result<QuarantineEntry> {
    let id: String = row.get(0)?;
    let title: String = row.get(1)?;
    let content: String = row.get(2)?;
    let author: String = row.get(3)?;
    let source: String = row.get(4)?;
    let reasons_json: String = row.get(5)?;
    let scores_json: String = row.get(6)?;
    let status: String = row.get(7)?;
    let created_at: String = row.get(8)?;
    let decided_at: Option<String> = row.get(9)?;

    let reasons: Vec<String> = serde_json::from_str(&reasons_json).unwrap_or_default();
    let scores: FirewallScores = serde_json::from_str(&scores_json).unwrap_or_default();

    Ok(QuarantineEntry {
        id,
        title,
        content,
        author,
        source,
        reasons,
        scores,
        status: QuarantineStatus::parse(&status),
        created_at,
        decided_at,
    })
}

#[async_trait]
impl FirewallRepository for SqliteFirewallRepository {
    async fn add_rule(&self, rule: &FirewallRule) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO firewall_rules (id, pattern, action, enabled, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                rule.id,
                rule.pattern,
                rule.action.as_str(),
                rule.enabled as i64,
                rule.reason,
                rule.created_at,
            ],
        )?;
        Ok(rule.id.clone())
    }

    async fn list_rules(&self) -> Result<Vec<FirewallRule>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, pattern, action, enabled, reason, created_at
             FROM firewall_rules ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], row_to_rule)?;
        let mut rules = Vec::new();
        for row in rows {
            rules.push(row?);
        }
        Ok(rules)
    }

    async fn delete_rule(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM firewall_rules WHERE id = ?1", params![id])?;
        Ok(())
    }

    async fn set_rule_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE firewall_rules SET enabled = ?2 WHERE id = ?1",
            params![id, enabled as i64],
        )?;
        Ok(())
    }

    async fn add_quarantine(&self, entry: &QuarantineEntry) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO quarantine_entries
                 (id, title, content, author, source, reasons_json, scores_json, status, created_at, decided_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.id,
                entry.title,
                entry.content,
                entry.author,
                entry.source,
                serde_json::to_string(&entry.reasons).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&entry.scores).unwrap_or_else(|_| "{}".to_string()),
                entry.status.as_str(),
                entry.created_at,
                entry.decided_at,
            ],
        )?;
        Ok(entry.id.clone())
    }

    async fn list_quarantine(
        &self,
        status: Option<QuarantineStatus>,
    ) -> Result<Vec<QuarantineEntry>> {
        let conn = self.conn.lock().unwrap();
        let (sql, param): (&str, Option<String>) = match status {
            Some(s) => (
                "SELECT id, title, content, author, source, reasons_json, scores_json,
                        status, created_at, decided_at
                 FROM quarantine_entries WHERE status = ?1 ORDER BY created_at DESC",
                Some(s.as_str().to_string()),
            ),
            None => (
                "SELECT id, title, content, author, source, reasons_json, scores_json,
                        status, created_at, decided_at
                 FROM quarantine_entries ORDER BY created_at DESC",
                None,
            ),
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = match param {
            Some(p) => stmt.query_map(params![p], row_to_quarantine)?,
            None => stmt.query_map([], row_to_quarantine)?,
        };
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    async fn get_quarantine(&self, id: &str) -> Result<Option<QuarantineEntry>> {
        let conn = self.conn.lock().unwrap();
        let entry = conn
            .query_row(
                "SELECT id, title, content, author, source, reasons_json, scores_json,
                        status, created_at, decided_at
                 FROM quarantine_entries WHERE id = ?1",
                params![id],
                row_to_quarantine,
            )
            .optional()?;
        Ok(entry)
    }

    async fn set_quarantine_status(&self, id: &str, status: QuarantineStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE quarantine_entries
             SET status = ?2, decided_at = ?3
             WHERE id = ?1",
            params![id, status.as_str(), chrono::Utc::now().to_rfc3339(),],
        )?;
        Ok(())
    }
}

// ── Agent-level memory permissions (second Firewall ring) ──────────

fn policy_row(row: &rusqlite::Row) -> rusqlite::Result<AgentPolicy> {
    let vis_json: String = row.get(3)?;
    let layers_json: String = row.get(4)?;
    let deny_json: String = row.get(5)?;
    let enabled: i64 = row.get(6)?;
    Ok(AgentPolicy {
        id: row.get(0)?,
        agent: row.get(1)?,
        role: row.get(2)?,
        allowed_visibility: serde_json::from_str(&vis_json).unwrap_or_default(),
        allowed_layers: serde_json::from_str(&layers_json).unwrap_or_default(),
        deny_patterns: serde_json::from_str(&deny_json).unwrap_or_default(),
        enabled: enabled != 0,
        created_at: row.get(7)?,
    })
}

impl SqliteFirewallRepository {
    /// Upsert an agent policy (by id).
    pub fn save_policy(&self, policy: &AgentPolicy) -> Result<()> {
        let vis_json = serde_json::to_string(&policy.allowed_visibility)?;
        let layers_json = serde_json::to_string(&policy.allowed_layers)?;
        let deny_json = serde_json::to_string(&policy.deny_patterns)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO agent_policies
                (id, agent, role, allowed_visibility, allowed_layers, deny_patterns, enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                agent=excluded.agent,
                role=excluded.role,
                allowed_visibility=excluded.allowed_visibility,
                allowed_layers=excluded.allowed_layers,
                deny_patterns=excluded.deny_patterns,
                enabled=excluded.enabled",
            params![
                policy.id,
                policy.agent,
                policy.role,
                vis_json,
                layers_json,
                deny_json,
                policy.enabled as i64,
                policy.created_at,
            ],
        )?;
        Ok(())
    }

    /// All agent policies.
    pub fn list_policies(&self) -> Result<Vec<AgentPolicy>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, agent, role, allowed_visibility, allowed_layers, deny_patterns, enabled, created_at
             FROM agent_policies ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], policy_row)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Policy for a specific agent (None if not configured).
    pub fn get_policy_for_agent(&self, agent: &str) -> Result<Option<AgentPolicy>> {
        let conn = self.conn.lock().unwrap();
        let policy = conn
            .query_row(
                "SELECT id, agent, role, allowed_visibility, allowed_layers, deny_patterns, enabled, created_at
                 FROM agent_policies WHERE agent = ?1 LIMIT 1",
                params![agent],
                policy_row,
            )
            .optional()?;
        Ok(policy)
    }

    /// Delete a policy by id.
    pub fn delete_policy(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM agent_policies WHERE id = ?1", params![id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::memory_firewall::{FirewallVerdict, assess_with_rules};

    fn rule(id: &str, pattern: &str) -> FirewallRule {
        FirewallRule {
            id: id.to_string(),
            pattern: pattern.to_string(),
            action: FirewallAction::Block,
            enabled: true,
            reason: "test".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn rules_round_trip() {
        let repo = SqliteFirewallRepository::new_in_memory().unwrap();
        let r = rule("r1", "confidential");
        repo.add_rule(&r).await.unwrap();

        let rules = repo.list_rules().await.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "r1");
        assert_eq!(rules[0].pattern, "confidential");
        assert!(rules[0].enabled);
    }

    #[tokio::test]
    async fn delete_rule_removes_it() {
        let repo = SqliteFirewallRepository::new_in_memory().unwrap();
        repo.add_rule(&rule("r1", "x")).await.unwrap();
        repo.add_rule(&rule("r2", "y")).await.unwrap();
        repo.delete_rule("r1").await.unwrap();

        let rules = repo.list_rules().await.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "r2");
    }

    #[tokio::test]
    async fn set_rule_enabled_toggles() {
        let repo = SqliteFirewallRepository::new_in_memory().unwrap();
        repo.add_rule(&rule("r1", "x")).await.unwrap();
        repo.set_rule_enabled("r1", false).await.unwrap();

        let rules = repo.list_rules().await.unwrap();
        assert!(!rules[0].enabled);

        repo.set_rule_enabled("r1", true).await.unwrap();
        let rules = repo.list_rules().await.unwrap();
        assert!(rules[0].enabled);
    }

    #[tokio::test]
    async fn quarantine_round_trip() {
        let repo = SqliteFirewallRepository::new_in_memory().unwrap();
        let assessment = assess_with_rules(
            "Контакты",
            "Почта: john.doe@example.com, телефон +7 912 345 67 89",
            &[],
        );
        assert_eq!(assessment.verdict, FirewallVerdict::Quarantine);
        let entry = QuarantineEntry::new(
            "Контакты".to_string(),
            "Почта: john.doe@example.com".to_string(),
            "user".to_string(),
            "manual".to_string(),
            &assessment,
        );
        repo.add_quarantine(&entry).await.unwrap();

        let entries = repo.list_quarantine(None).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, entry.id);
        assert_eq!(entries[0].status, QuarantineStatus::Pending);
        assert_eq!(entries[0].reasons.len(), 1);
        assert!(entries[0].reasons[0].starts_with("pii"));
    }

    #[tokio::test]
    async fn quarantine_filters_by_status() {
        let repo = SqliteFirewallRepository::new_in_memory().unwrap();
        let a = assess_with_rules("t", "a@b.com", &[]);
        let e1 = QuarantineEntry::new("t1".into(), "a@b.com".into(), "u".into(), "m".into(), &a);
        repo.add_quarantine(&e1).await.unwrap();
        repo.set_quarantine_status(&e1.id, QuarantineStatus::Approved)
            .await
            .unwrap();

        let pending = repo
            .list_quarantine(Some(QuarantineStatus::Pending))
            .await
            .unwrap();
        let approved = repo
            .list_quarantine(Some(QuarantineStatus::Approved))
            .await
            .unwrap();
        assert!(pending.is_empty());
        assert_eq!(approved.len(), 1);
        assert_eq!(approved[0].status, QuarantineStatus::Approved);
        assert!(approved[0].decided_at.is_some());
    }

    #[tokio::test]
    async fn get_quarantine_nonexistent_returns_none() {
        let repo = SqliteFirewallRepository::new_in_memory().unwrap();
        assert!(repo.get_quarantine("missing").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn agent_policy_round_trip() {
        let repo = SqliteFirewallRepository::new_in_memory().unwrap();
        let p = AgentPolicy {
            id: "pol-1".to_string(),
            agent: "claude-code".to_string(),
            role: "assistant".to_string(),
            allowed_visibility: vec![
                crate::core::memory::types::MemoryVisibility::Public,
                crate::core::memory::types::MemoryVisibility::Restricted,
            ],
            allowed_layers: vec![crate::core::memory::types::MemoryLayer::Semantic],
            deny_patterns: vec!["api key".to_string(), "password".to_string()],
            enabled: true,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        repo.save_policy(&p).unwrap();

        let policies = repo.list_policies().unwrap();
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].agent, "claude-code");
        assert_eq!(policies[0].allowed_visibility.len(), 2);
        assert_eq!(policies[0].deny_patterns.len(), 2);

        let by_agent = repo.get_policy_for_agent("claude-code").unwrap();
        assert!(by_agent.is_some());
        assert_eq!(by_agent.unwrap().role, "assistant");
        assert!(repo.get_policy_for_agent("missing").unwrap().is_none());
    }

    #[tokio::test]
    async fn agent_policy_upsert_updates_existing() {
        let repo = SqliteFirewallRepository::new_in_memory().unwrap();
        let mut p = AgentPolicy {
            id: "pol-1".to_string(),
            agent: "copilot".to_string(),
            role: "assistant".to_string(),
            allowed_visibility: Vec::new(),
            allowed_layers: Vec::new(),
            deny_patterns: Vec::new(),
            enabled: true,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        repo.save_policy(&p).unwrap();
        p.enabled = false;
        p.deny_patterns = vec!["secret".to_string()];
        repo.save_policy(&p).unwrap();

        let policies = repo.list_policies().unwrap();
        assert_eq!(policies.len(), 1, "upsert must not duplicate");
        assert!(!policies[0].enabled);
        assert_eq!(policies[0].deny_patterns, vec!["secret".to_string()]);
    }

    #[tokio::test]
    async fn agent_policy_delete_removes() {
        let repo = SqliteFirewallRepository::new_in_memory().unwrap();
        let p = AgentPolicy {
            id: "pol-1".to_string(),
            agent: "copilot".to_string(),
            role: "assistant".to_string(),
            allowed_visibility: Vec::new(),
            allowed_layers: Vec::new(),
            deny_patterns: Vec::new(),
            enabled: true,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        repo.save_policy(&p).unwrap();
        repo.delete_policy("pol-1").unwrap();
        assert!(repo.list_policies().unwrap().is_empty());
        // Deleting a missing policy is a no-op (no error).
        repo.delete_policy("pol-1").unwrap();
    }
}
