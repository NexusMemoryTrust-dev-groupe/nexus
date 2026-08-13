//! Nexus Memory Score — «здоровье мозга проекта» (Knowledge Navigation 2.0).
//!
//! Не просто `importance = 0.72`, а полноценная панель здоровья памяти:
//!
//! ```text
//! Coverage          94%   — какая доля сущностей графа покрыта памятью
//! Freshness         88%   — насколько свежи знания (обновления/подтверждения)
//! Consistency       97%   — доля непротиворечивых записей
//! Trust             91%   — подтверждённость + уверенность + отсутствие «wrong»
//! Redundancy        12%   — доля дублирующихся записей (чем ниже, тем лучше)
//! Conflict           3%   — доля записей в конфликте (чем ниже, тем лучше)
//! Context Quality   93%   — зрелость знаний (доля Semantic/Procedural/Decision/Strategic)
//!
//! MEMORY HEALTH  ██████████████████░░ 92%
//! ```
//!
//! Все функции чистые и детерминированные: вход — срез `MemoryRecord`
//! (и опционально число сущностей графа), выход — структура показателей.
//! Так метрики тестируются без БД, а SQLite-слой лишь собирает данные.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::types::{MemoryLayer, MemoryState};

/// Одна метрика здоровья с описанием, что она означает.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ScoreMetric {
    /// 0.0–1.0, где 1.0 — идеально.
    pub value: f64,
}

impl ScoreMetric {
    pub fn pct(&self) -> u32 {
        (self.value.clamp(0.0, 1.0) * 100.0).round() as u32
    }
}

/// Полная панель здоровья памяти проекта.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryScore {
    /// Доля сущностей графа, покрытых памятью (1.0 когда покрыты все).
    pub coverage: ScoreMetric,
    /// Свежесть: насколько недавно память обновлялась / подтверждалась.
    pub freshness: ScoreMetric,
    /// Доля записей, не участвующих в противоречиях.
    pub consistency: ScoreMetric,
    /// Доверие: подтверждённость + уверенность + отсутствие негативной обратной связи.
    pub trust: ScoreMetric,
    /// Избыточность: доля дублирующихся записей (0.0 — нет дублей, идеал).
    pub redundancy: ScoreMetric,
    /// Конфликтность: доля записей в состоянии Conflicted (0.0 — идеал).
    pub conflict: ScoreMetric,
    /// Зрелость знаний: доля записей в Semantic/Procedural/Decision/Strategic.
    pub context_quality: ScoreMetric,
    /// Итоговое здоровье: взвешенная комбинация показателей (0.0–1.0).
    pub health: ScoreMetric,
    /// Сколько записей было проанализировано.
    pub records_analyzed: u32,
    /// Сколько сущностей графа учитывалось при расчёте покрытия.
    pub entities_total: u32,
}

/// Параметры расчёта. Все имеют разумные значения по умолчанию.
#[derive(Debug, Clone, Copy)]
pub struct ScoreOptions {
    /// Окно «свежести» в днях: запись, обновлённая внутри окна, считается свежей.
    pub freshness_window_days: i64,
    /// Максимальный возраст записи, после которого свежесть = 0 (в днях).
    pub max_age_days: i64,
}

impl Default for ScoreOptions {
    fn default() -> Self {
        Self {
            freshness_window_days: 30,
            max_age_days: 365,
        }
    }
}

/// Порог «длинной» записи для детекции избыточности.
const TITLE_SIMILARITY: f64 = 0.6;

/// Считает долю, защищаясь от деления на ноль.
fn ratio(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64
    }
}

/// Нормализованная свежесть записи: 1.0 внутри окна, плавный спад до 0.
fn freshness_of(record: &MemoryRecord, now: DateTime<Utc>, opts: ScoreOptions) -> f64 {
    let reference = record.updated_at.max(record.created_at);
    let age_days = (now - reference).num_days().max(0);
    if age_days <= opts.freshness_window_days {
        1.0
    } else {
        let span = (opts.max_age_days - opts.freshness_window_days).max(1) as f64;
        let over = (age_days - opts.freshness_window_days) as f64;
        (1.0 - over / span).clamp(0.0, 1.0)
    }
}

/// Доверие к одной записи: подтверждение + уверенность − негативный фидбек.
fn trust_of(record: &MemoryRecord) -> f64 {
    let mut t = record.confidence_score;
    match record.memory_state {
        MemoryState::UserConfirmed => t += 0.25,
        MemoryState::Inferred => t -= 0.10,
        MemoryState::Conflicted => t -= 0.30,
        _ => {}
    }
    if record.confirmed_at.is_some() {
        t += 0.10;
    }
    if record.feedback.wrong > 0 {
        t -= 0.20;
    }
    if record.feedback.irrelevant > 0 {
        t -= 0.10;
    }
    if record.feedback.useful > 0 {
        t += 0.10;
    }
    t.clamp(0.0, 1.0)
}

/// Косимметричное сходство заголовков: 1.0 — одинаковые, 0.0 — разные.
fn title_similarity(a: &str, b: &str) -> f64 {
    let norm = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let a = norm(a);
    let b = norm(b);
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let (shorter, longer) = if a.len() <= b.len() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    // Доля символов короткого заголовка, присутствующих в длинном (в порядке).
    // Работаем по char-индексам: байтовые индексы могут разрезать
    // многобайтовый символ (кириллица/эмодзи) → паника на slice.
    let longer_chars: Vec<char> = longer.chars().collect();
    let mut matches = 0usize;
    let mut idx = 0usize;
    for ch in shorter.chars() {
        if idx >= longer_chars.len() {
            break;
        }
        if let Some(pos) = longer_chars[idx..].iter().position(|&c| c == ch) {
            matches += 1;
            idx += pos + 1;
        }
    }
    matches as f64 / shorter.len() as f64
}

/// Считает полную панель здоровья для среза записей.
///
/// `entities_total` — число сущностей графа (для coverage); передайте 0, чтобы
/// coverage считался только по связанным сущностям внутри выборки.
pub fn compute_score(records: &[MemoryRecord], entities_total: u32) -> MemoryScore {
    compute_score_opts(records, entities_total, ScoreOptions::default())
}

/// Вариант с явными параметрами (для тестов и настройки).
pub fn compute_score_opts(
    records: &[MemoryRecord],
    entities_total: u32,
    opts: ScoreOptions,
) -> MemoryScore {
    let total = records.len();
    if total == 0 {
        return MemoryScore {
            coverage: ScoreMetric { value: 0.0 },
            freshness: ScoreMetric { value: 0.0 },
            consistency: ScoreMetric { value: 1.0 },
            trust: ScoreMetric { value: 0.0 },
            redundancy: ScoreMetric { value: 0.0 },
            conflict: ScoreMetric { value: 0.0 },
            context_quality: ScoreMetric { value: 0.0 },
            health: ScoreMetric { value: 0.0 },
            records_analyzed: 0,
            entities_total,
        };
    }

    let now = Utc::now();

    // Coverage: уникальные сущности, на которые ссылаются записи.
    let mut linked = std::collections::HashSet::new();
    for r in records {
        for e in &r.linked_entity_ids {
            linked.insert(e.as_str().to_string());
        }
    }
    let coverage_denominator = if entities_total > linked.len() as u32 {
        entities_total as usize
    } else {
        linked.len().max(1)
    };
    let coverage = ratio(linked.len(), coverage_denominator);

    // Freshness.
    let freshness_sum: f64 = records.iter().map(|r| freshness_of(r, now, opts)).sum();
    let freshness = freshness_sum / total as f64;

    // Consistency: не conflicted и не superseded.
    let inconsistent = records
        .iter()
        .filter(|r| {
            matches!(
                r.memory_state,
                MemoryState::Conflicted | MemoryState::Superseded
            )
        })
        .count();
    let consistency = 1.0 - ratio(inconsistent, total);

    // Trust.
    let trust_sum: f64 = records.iter().map(trust_of).sum();
    let trust = (trust_sum / total as f64).clamp(0.0, 1.0);

    // Redundancy: пары записей с похожими заголовками (не supersedes-цепочки).
    let mut redundant_pairs = 0usize;
    for i in 0..total {
        for j in (i + 1)..total {
            if title_similarity(&records[i].title, &records[j].title) >= TITLE_SIMILARITY {
                redundant_pairs += 1;
            }
        }
    }
    // Нормировка: максимум пар при total записях — total*(total-1)/2.
    let max_pairs = total.saturating_mul(total.saturating_sub(1)) / 2;
    let redundancy = if max_pairs == 0 {
        0.0
    } else {
        (redundant_pairs as f64 / max_pairs as f64).min(1.0)
    };

    // Conflict: доля записей в состоянии Conflicted.
    let conflicted = records
        .iter()
        .filter(|r| matches!(r.memory_state, MemoryState::Conflicted))
        .count();
    let conflict = ratio(conflicted, total);

    // Context quality: зрелые слои.
    let mature = records
        .iter()
        .filter(|r| {
            matches!(
                r.layer,
                MemoryLayer::Semantic
                    | MemoryLayer::Procedural
                    | MemoryLayer::Decision
                    | MemoryLayer::Strategic
            )
        })
        .count();
    let context_quality = ratio(mature, total);

    // Итоговое здоровье: покрытие + свежесть + согласованность + доверие +
    // отсутствие избыточности + отсутствие конфликтов + зрелость.
    let health = 0.20 * coverage
        + 0.15 * freshness
        + 0.15 * consistency
        + 0.20 * trust
        + 0.10 * (1.0 - redundancy)
        + 0.10 * (1.0 - conflict)
        + 0.10 * context_quality;

    MemoryScore {
        coverage: ScoreMetric { value: coverage },
        freshness: ScoreMetric { value: freshness },
        consistency: ScoreMetric { value: consistency },
        trust: ScoreMetric { value: trust },
        redundancy: ScoreMetric { value: redundancy },
        conflict: ScoreMetric { value: conflict },
        context_quality: ScoreMetric {
            value: context_quality,
        },
        health: ScoreMetric {
            value: health.clamp(0.0, 1.0),
        },
        records_analyzed: total as u32,
        entities_total,
    }
}

/// Краткая человекочитаемая сводка здоровья (для /help и контекстных пакетов).
pub fn render_score(score: &MemoryScore) -> String {
    let bar = |pct: u32| -> String {
        let filled = (pct as f32 / 100.0 * 20.0).round() as usize;
        let empty = 20usize.saturating_sub(filled);
        format!("{} {}", "█".repeat(filled), "░".repeat(empty))
    };
    let mut out = String::with_capacity(512);
    out.push_str(&format!(
        "MEMORY HEALTH {} {}%\n",
        bar(score.health.pct()),
        score.health.pct()
    ));
    out.push_str(&format!("Coverage        {:>3}%\n", score.coverage.pct()));
    out.push_str(&format!("Freshness       {:>3}%\n", score.freshness.pct()));
    out.push_str(&format!(
        "Consistency     {:>3}%\n",
        score.consistency.pct()
    ));
    out.push_str(&format!("Trust           {:>3}%\n", score.trust.pct()));
    out.push_str(&format!("Redundancy      {:>3}%\n", score.redundancy.pct()));
    out.push_str(&format!("Conflict        {:>3}%\n", score.conflict.pct()));
    out.push_str(&format!(
        "Context Quality {:>3}%\n",
        score.context_quality.pct()
    ));
    out.push_str(&format!("({} records analyzed)", score.records_analyzed));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::entity_id::EntityId;
    use crate::core::memory::memory_record::MemoryRecord;
    use crate::core::memory::types::{MemorySource, MemoryState};

    fn record(
        title: &str,
        state: MemoryState,
        layer: MemoryLayer,
        confidence: f64,
    ) -> MemoryRecord {
        let mut r = MemoryRecord::new(
            title.to_string(),
            format!("Content about {title}"),
            "tester".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.memory_state = state;
        r.layer = layer;
        r.confidence_score = confidence;
        r
    }

    fn link(r: &mut MemoryRecord, entity: &str) {
        r.linked_entity_ids
            .push(EntityId::parse(entity).unwrap_or_else(|_| EntityId::new()));
    }

    #[test]
    fn empty_pool_scores_zero_health() {
        let s = compute_score(&[], 0);
        assert_eq!(s.records_analyzed, 0);
        assert_eq!(s.health.pct(), 0);
        assert_eq!(s.consistency.pct(), 100, "пустой пул непротиворечив");
    }

    #[test]
    fn healthy_pool_scores_high() {
        let titles = [
            "Auth via JWT",
            "Dropped Redis on Aug 3",
            "Refresh tokens rotate",
            "DB stays local",
            "Middleware chain order",
            "Rate limiting at edge",
            "Logging via tracing",
            "CI runs cargo test",
            "Config in SQLite",
            "Versioning V1..Vn",
        ];
        let mut records = Vec::new();
        for (i, t) in titles.iter().enumerate() {
            let mut r = record(t, MemoryState::UserConfirmed, MemoryLayer::Decision, 0.9);
            link(&mut r, &format!("entity-{i}"));
            records.push(r);
        }
        let s = compute_score(&records, 10);
        assert!(s.health.pct() >= 80, "health={}", s.health.pct());
        assert_eq!(s.coverage.pct(), 100);
        assert_eq!(s.conflict.pct(), 0);
        assert_eq!(s.redundancy.pct(), 0);
        assert_eq!(s.context_quality.pct(), 100);
    }

    #[test]
    fn conflicted_and_superseded_hurt_consistency() {
        let mut records = vec![
            record("A", MemoryState::Current, MemoryLayer::Semantic, 0.7),
            record("B", MemoryState::Conflicted, MemoryLayer::Semantic, 0.7),
            record("C", MemoryState::Superseded, MemoryLayer::Semantic, 0.7),
        ];
        for r in &mut records {
            link(r, "entity-1");
        }
        let s = compute_score(&records, 1);
        assert_eq!(s.consistency.pct(), 33);
        assert_eq!(s.conflict.pct(), 33);
    }

    #[test]
    fn redundant_titles_raise_redundancy() {
        let records = vec![
            record(
                "Setup PostgreSQL",
                MemoryState::Current,
                MemoryLayer::Semantic,
                0.7,
            ),
            record(
                "setup postgresql",
                MemoryState::Current,
                MemoryLayer::Semantic,
                0.7,
            ),
            record(
                "Setup PostgreSQL again",
                MemoryState::Current,
                MemoryLayer::Semantic,
                0.7,
            ),
        ];
        let s = compute_score(&records, 0);
        assert!(
            s.redundancy.pct() >= 60,
            "redundancy={}",
            s.redundancy.pct()
        );
    }

    #[test]
    fn immature_layers_lower_context_quality() {
        let records = vec![
            record("W", MemoryState::Current, MemoryLayer::Working, 0.7),
            record("E", MemoryState::Current, MemoryLayer::Episodic, 0.7),
            record("S", MemoryState::Current, MemoryLayer::Semantic, 0.7),
        ];
        let s = compute_score(&records, 0);
        assert_eq!(s.context_quality.pct(), 33);
    }

    #[test]
    fn coverage_uses_entities_total() {
        let mut records = vec![record(
            "X",
            MemoryState::Current,
            MemoryLayer::Semantic,
            0.8,
        )];
        link(&mut records[0], "entity-1");
        let s = compute_score(&records, 4);
        assert_eq!(s.coverage.pct(), 25, "1 из 4 сущностей покрыта");
    }

    #[test]
    fn wrong_feedback_lowers_trust() {
        let mut good = record(
            "Good fact",
            MemoryState::UserConfirmed,
            MemoryLayer::Semantic,
            0.8,
        );
        good.confirmed_at = Some(Utc::now());
        let mut bad = record(
            "Bad fact",
            MemoryState::UserConfirmed,
            MemoryLayer::Semantic,
            0.8,
        );
        bad.confirmed_at = Some(Utc::now());
        bad.feedback.wrong = 2;
        let s_good = compute_score(&[good], 0);
        let s_bad = compute_score(&[bad], 0);
        assert!(
            s_good.trust.pct() > s_bad.trust.pct(),
            "good={}, bad={}",
            s_good.trust.pct(),
            s_bad.trust.pct()
        );
        assert_eq!(
            s_good.trust.pct(),
            100,
            "без wrong фидбека — полное доверие"
        );
        assert!(s_bad.trust.pct() < 100, "wrong фидбек снимает доверие");
    }

    #[test]
    fn old_records_have_lower_freshness() {
        let mut fresh = record("Fresh", MemoryState::Current, MemoryLayer::Semantic, 0.7);
        fresh.created_at = Utc::now() - chrono::Duration::days(2);
        fresh.updated_at = fresh.created_at;
        let mut old = record("Old", MemoryState::Current, MemoryLayer::Semantic, 0.7);
        old.created_at = Utc::now() - chrono::Duration::days(400);
        old.updated_at = old.created_at;
        let s = compute_score(&[fresh], 0);
        assert_eq!(s.freshness.pct(), 100);
        let s_old = compute_score(&[old], 0);
        assert_eq!(s_old.freshness.pct(), 0);
    }

    #[test]
    fn render_score_is_human_readable() {
        let records = vec![record(
            "D1",
            MemoryState::UserConfirmed,
            MemoryLayer::Decision,
            0.9,
        )];
        let s = compute_score(&records, 0);
        let text = render_score(&s);
        assert!(text.contains("MEMORY HEALTH"));
        assert!(text.contains("Coverage"));
        assert!(text.contains("Trust"));
    }
}
