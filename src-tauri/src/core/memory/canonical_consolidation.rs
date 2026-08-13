//! Canonical Consolidation — «ночная» консолидация повторяющихся записей (Система 3).
//!
//! Спецификация: за неделю агент накопил 17 сообщений, 9 решений, 4 изменения
//! архитектуры — и 7 записей фактически говорят об одном и том же. Nexus ночью
//! обнаруживает повторы и превращает их в одну Canonical Memory:
//!
//!   "Authentication uses JWT access tokens + rotating refresh tokens."
//!
//! При этом сохраняется provenance всех исходных событий (`derived_from`).
//!
//! Механика (чистые функции, без базы данных):
//! - `tokenize` — нормализация текста в набор токенов (lowercase, стоп-слова);
//! - `similarity` — Jaccard-схожесть двух записей по title+summary;
//! - `find_clusters` — жадная кластеризация: записи, говорящие об одном и том же,
//!   попадают в один кластер (порог `SIMILARITY_THRESHOLD`, минимум 2 участника);
//! - `build_canonical` — синтез канонической записи: заголовок самого весомого
//!   участника, объединённое резюме, importance/confidence усилены повторением,
//!   `derived_from` = все исходные id (provenance).
//!
//! Исходные записи не удаляются — они помечаются `MemoryStatus::Merged` с
//! указанием `superseded_by_id` на каноническую. История сохраняется.

use std::collections::HashSet;

use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::types::{MemorySource, MemoryState, MemoryStatus};

/// Минимальный Jaccard-порог для того, чтобы две записи считались повтором.
pub const SIMILARITY_THRESHOLD: f64 = 0.40;

/// Минимальное число участников, чтобы кластер стал канонической записью.
pub const MIN_CLUSTER_SIZE: usize = 2;

/// Усиление важности за каждый дополнительный повтор (cap 1.0).
pub const IMPORTANCE_BOOST_PER_REPEAT: f64 = 0.05;

/// Усиление уверенности за повторение (cap 1.0).
pub const CONFIDENCE_BOOST_PER_REPEAT: f64 = 0.04;

/// Максимальная длина синтезированного резюме (символов).
pub const MAX_CANONICAL_SUMMARY_CHARS: usize = 600;

/// Один кластер повторяющихся записей.
#[derive(Debug, Clone)]
pub struct Cluster {
    /// Ids участников (в порядке важности).
    pub member_ids: Vec<String>,
    /// Titles участников (для отчёта).
    pub member_titles: Vec<String>,
    /// Средняя попарная схожесть внутри кластера (0.0–1.0).
    pub cohesion: f64,
}

/// Итог одного консолидационного прохода.
#[derive(Debug, Clone)]
pub struct ConsolidationResult {
    pub clusters_found: usize,
    /// Кластеры, которые превратились в канонические записи.
    pub canonical_count: usize,
    /// Id созданных канонических записей.
    pub canonical_ids: Vec<String>,
    /// Сколько исходных записей было помечено как Merged.
    pub merged_members: usize,
    /// Кластеры ниже порога участников (показаны в отчёте как «почти»).
    pub near_clusters: usize,
}

/// Нормализация текста в набор токенов для сравнения.
pub fn tokenize(text: &str) -> HashSet<String> {
    const STOP: &[&str] = &[
        "and", "or", "the", "a", "an", "to", "of", "in", "on", "for", "with", "from", "by", "at",
        "as", "is", "are", "was", "were", "be", "been", "it", "its", "this", "that", "we", "our",
        "you", "your", "they", "their", "i", "my", "me", "not", "no", "but", "do", "does", "did",
        "have", "has", "had", "will", "would", "can", "could", "should", "using", "uses", "used",
        "use", "new", "via", "per", "и", "в", "на", "для", "с", "по", "из", "не", "это", "что",
        "как", "мы", "наш", "вы", "ваш", "они", "их",
    ];
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| !w.is_empty())
        .filter(|w| w.len() > 1)
        .filter(|w| !STOP.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Jaccard-схожесть двух записей по title+summary (0.0 – 1.0).
pub fn similarity(a: &MemoryRecord, b: &MemoryRecord) -> f64 {
    let ta = tokenize(&format!("{} {}", a.title, a.summary));
    let tb = tokenize(&format!("{} {}", b.title, b.summary));
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let intersection = ta.intersection(&tb).count();
    let union = ta.union(&tb).count();
    intersection as f64 / union as f64
}

/// Токенная Jaccard-схожесть для двух уже-токенизированных наборов.
fn token_similarity(ta: &HashSet<String>, tb: &HashSet<String>) -> f64 {
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let intersection = ta.intersection(tb).count();
    let union = ta.union(tb).count();
    intersection as f64 / union as f64
}

/// Жадная кластеризация записей, говорящих об одном и том же.
///
/// Каждая запись сравнивается с представителем каждого существующего кластера
/// (первый участник); при совпадении выше порога — присоединяется к самому
/// похожему кластеру, иначе открывает новый. Conflicted/Superseded записи и
/// уже Merged не участвуют — у них свои жизненные циклы.
pub fn find_clusters(records: &[MemoryRecord]) -> Vec<Cluster> {
    // Индексы участников: только актуальная память, достойная консолидации.
    let participants: Vec<&MemoryRecord> = records
        .iter()
        .filter(|r| {
            !matches!(
                r.memory_state,
                MemoryState::Conflicted | MemoryState::Superseded
            ) && r.status != MemoryStatus::Merged
        })
        .collect();

    // Кэш токенов, чтобы не токенизировать повторно.
    let tokens: Vec<HashSet<String>> = participants
        .iter()
        .map(|r| tokenize(&format!("{} {}", r.title, r.summary)))
        .collect();

    let mut clusters: Vec<(Vec<usize>, Vec<f64>)> = Vec::new(); // (member_idxs, similarities)
    for (idx, _rec) in participants.iter().enumerate() {
        let mut best: Option<(usize, f64)> = None;
        for (ci, (members, _)) in clusters.iter().enumerate() {
            let sim = token_similarity(&tokens[idx], &tokens[members[0]]);
            if sim >= SIMILARITY_THRESHOLD && best.map(|(_, s)| sim > s).unwrap_or(true) {
                best = Some((ci, sim));
            }
        }
        match best {
            Some((ci, sim)) => {
                clusters[ci].0.push(idx);
                clusters[ci].1.push(sim);
            }
            None => {
                clusters.push((vec![idx], vec![1.0]));
            }
        }
    }

    let mut out = Vec::new();
    for (members, sims) in clusters {
        if members.len() < 2 {
            continue; // одиночные записи — не повторы
        }
        // Сортируем по важности: самый весомый участник становится заголовком.
        let mut ordered = members;
        ordered.sort_by(|&a, &b| {
            participants[b]
                .importance_score
                .partial_cmp(&participants[a].importance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let cohesion = sims.iter().sum::<f64>() / sims.len() as f64;
        out.push(Cluster {
            member_ids: ordered
                .iter()
                .map(|&i| participants[i].id.as_str().to_string())
                .collect(),
            member_titles: ordered
                .iter()
                .map(|&i| participants[i].title.clone())
                .collect(),
            cohesion,
        });
    }
    // Самые связные кластеры первыми.
    out.sort_by(|a, b| {
        b.cohesion
            .partial_cmp(&a.cohesion)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Синтез канонической записи из кластера.
///
/// - Заголовок: самого важного участника (наиболее каноничная формулировка).
/// - Резюме: объединение уникальных summary участников, обрезанное по лимиту.
/// - Content: перечень фактов с указанием источника каждого.
/// - Importance/confidence: максимальное значение участников + буст за каждый
///   дополнительный повтор — повторение подтверждает истину.
/// - `derived_from`: все исходные id (provenance сохранён).
pub fn build_canonical(
    cluster: &Cluster,
    records: &[MemoryRecord],
    author: &str,
) -> Option<MemoryRecord> {
    let members: Vec<&MemoryRecord> = cluster
        .member_ids
        .iter()
        .filter_map(|id| records.iter().find(|r| r.id.as_str() == id))
        .collect();
    if members.len() < MIN_CLUSTER_SIZE {
        return None;
    }

    // Самый важный участник — основа заголовка и layer.
    let anchor = members[0];
    let repeats = (members.len() - 1) as f64;

    let mut summary_parts: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for m in &members {
        let mut key_tokens: Vec<String> = tokenize(&m.summary).into_iter().collect();
        key_tokens.sort();
        let key = key_tokens.join(" ");
        if key.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        let s = m.summary.trim();
        if !s.is_empty() {
            summary_parts.push(s.to_string());
        }
    }
    let mut summary = summary_parts.join(" | ");
    if summary.chars().count() > MAX_CANONICAL_SUMMARY_CHARS {
        let cut: String = summary.chars().take(MAX_CANONICAL_SUMMARY_CHARS).collect();
        summary = format!("{}…", cut.trim_end());
    }

    let mut content_lines = Vec::new();
    for m in &members {
        content_lines.push(format!(
            "— {} ({}; importance {:.2})",
            m.summary.trim(),
            m.id.as_str(),
            m.importance_score
        ));
    }
    let content = content_lines.join("\n");

    let mut canonical = MemoryRecord::new(
        anchor.title.clone(),
        content,
        author.to_string(),
        MemorySource::Compressed,
    )
    .ok()?;
    canonical.summary = summary;
    canonical.derived_from = cluster.member_ids.clone();
    canonical.importance_score =
        (anchor.importance_score + IMPORTANCE_BOOST_PER_REPEAT * repeats).min(1.0);
    canonical.confidence_score = (members
        .iter()
        .map(|m| m.confidence_score)
        .fold(0.0_f64, f64::max)
        + CONFIDENCE_BOOST_PER_REPEAT * repeats)
        .min(1.0);
    canonical.layer = anchor.layer.clone();
    canonical.memory_state = MemoryState::Current;
    canonical.reason = Some(format!(
        "Consolidated from {} records stating the same fact",
        members.len()
    ));
    canonical.rehearsal_count = 0;
    Some(canonical)
}

/// Текстовый отчёт о найденных кластерах (для MCP/copilot).
pub fn render_clusters(clusters: &[Cluster]) -> String {
    if clusters.is_empty() {
        return "No repetitions found — every record is unique.".to_string();
    }
    let mut out = String::from("Canonical consolidation — repeated records:\n");
    for (i, c) in clusters.iter().enumerate() {
        out.push_str(&format!(
            "{}. {} records about the same thing (cohesion {:.2}):\n",
            i + 1,
            c.member_ids.len(),
            c.cohesion
        ));
        for t in &c.member_titles {
            out.push_str(&format!("     • {t}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::memory_record::MemoryRecord;
    use crate::core::memory::types::MemorySource;

    fn record(title: &str, summary: &str, importance: f64) -> MemoryRecord {
        let mut r = MemoryRecord::new(
            title.to_string(),
            summary.to_string(),
            "tester".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.summary = summary.to_string();
        r.importance_score = importance;
        r
    }

    #[test]
    fn tokenize_drops_stop_words_and_normalizes_case() {
        let t = tokenize("Authentication uses JWT access tokens AND rotating refresh tokens");
        assert!(t.contains("jwt"));
        assert!(t.contains("tokens"));
        assert!(!t.contains("and"));
        assert!(!t.contains("uses"));
        // Русский тоже нормализуется.
        let ru = tokenize("Авторизация использует JWT токены и refresh токены");
        assert!(ru.contains("авторизация"));
        assert!(ru.contains("jwt"));
        assert!(!ru.contains("и"));
    }

    #[test]
    fn identical_records_have_similarity_one() {
        let a = record("Auth", "Authentication uses JWT access tokens", 0.8);
        let b = record("Auth", "Authentication uses JWT access tokens", 0.8);
        assert_eq!(similarity(&a, &b), 1.0);
    }

    #[test]
    fn unrelated_records_have_low_similarity() {
        let a = record("Auth", "JWT authentication tokens", 0.8);
        let b = record("Billing", "Invoice payment monthly plan", 0.8);
        assert!(similarity(&a, &b) < SIMILARITY_THRESHOLD);
    }

    #[test]
    fn repeated_records_form_one_cluster() {
        let a = record("Auth", "Authentication uses JWT access tokens", 0.7);
        let b = record(
            "Auth",
            "Authentication uses JWT access tokens and refresh tokens",
            0.8,
        );
        let c = record("Billing", "Payment plan and invoice", 0.6);
        let clusters = find_clusters(&[a, b, c]);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].member_ids.len(), 2);
        assert!(clusters[0].cohesion >= SIMILARITY_THRESHOLD);
    }

    #[test]
    fn conflicted_and_merged_do_not_cluster() {
        let mut a = record("Auth", "Authentication uses JWT tokens", 0.7);
        a.memory_state = MemoryState::Conflicted;
        let mut b = record("Auth", "Authentication uses JWT tokens", 0.7);
        b.status = MemoryStatus::Merged;
        let clusters = find_clusters(&[a, b]);
        assert!(
            clusters.is_empty(),
            "conflicted/merged records never consolidate"
        );
    }

    #[test]
    fn canonical_preserves_provenance_and_boosts_importance() {
        let a = record("Auth", "Authentication uses JWT access tokens", 0.7);
        let b = record(
            "Auth details",
            "Authentication uses JWT access tokens and rotating refresh tokens",
            0.8,
        );
        let clusters = find_clusters(&[a.clone(), b.clone()]);
        assert_eq!(clusters.len(), 1);
        let canon = build_canonical(&clusters[0], &[a.clone(), b.clone()], "nexus").unwrap();
        // Provenance: все исходные id.
        assert_eq!(canon.derived_from.len(), 2);
        assert!(canon.derived_from.contains(&a.id.as_str().to_string()));
        assert!(canon.derived_from.contains(&b.id.as_str().to_string()));
        // Повторение усилило важность.
        assert!(canon.importance_score > a.importance_score);
        assert!(canon.importance_score <= 1.0);
        // Резюме объединено из обоих.
        assert!(canon.summary.contains("access tokens"));
    }

    #[test]
    fn cluster_orders_by_importance_and_uses_top_as_title() {
        let low = record("Auth", "Authentication uses JWT tokens", 0.4);
        let high = record(
            "Auth",
            "Authentication uses JWT access tokens and refresh tokens",
            0.95,
        );
        let clusters = find_clusters(&[low.clone(), high.clone()]);
        let canon = build_canonical(&clusters[0], &[low, high], "nexus").unwrap();
        assert!(canon.title.contains("Auth"));
        assert!(
            canon.importance_score >= 0.95,
            "anchor importance preserved"
        );
    }
}
