//! Predictive Context (System 8) — Nexus предсказывает следующий запрос.
//!
//! Люди работают паттернами: спросил «как работает auth» → следующим почти
//! наверняка будет «где хранятся сессии» или «как добавить токен». Nexus
//! запоминает последовательности запросов и по текущему вопросу предсказывает:
//! * какой запрос будет следующим (`suggested_query`),
//! * с какой вероятностью (`confidence`),
//! * какие сущности/память понадобятся (`entities`) — их можно заранее
//!   прогреть в кэше контекста, чтобы следующий ответ пришёл мгновенно.
//!
//! Чистые функции здесь тестируются без БД: нормализация, марковские
//! переходы первого порядка, сглаживание вероятностей. Хранение истории
//! запросов — в `query_history` (миграция V26).

use serde::{Deserialize, Serialize};

/// Одна запись истории запросов.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryLogEntry {
    pub query: String,
    pub intent_type: String,
    /// Сущности/память, задействованные в ответе (id).
    pub entities: Vec<String>,
    pub created_at: String,
}

/// Предсказание следующего запроса.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Prediction {
    pub suggested_query: String,
    /// 0.0–1.0: как часто этот переход встречался в истории.
    pub confidence: f64,
    /// Интент следующего запроса (если известен).
    pub intent_type: String,
    /// Сущности, которые понадобятся следующему запросу — кандидаты на прогрев.
    pub entities: Vec<String>,
    /// Сколько переходов с текущим запросом было в истории.
    pub matches: usize,
}

/// Нормализация запроса: lowercase, без пунктуации, стоп-слов нет — только
/// значимые слова, отсортированные (порядок слов не важен для схожести).
pub fn normalize(query: &str) -> String {
    let mut words: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && w.len() > 1)
        .map(|w| w.to_string())
        .collect();
    words.sort();
    words.dedup();
    words.join(" ")
}

/// Похожесть двух нормализованных запросов (Jaccard по словам).
pub fn jaccard(a: &str, b: &str) -> f64 {
    let wa: Vec<&str> = a.split_whitespace().collect();
    let wb: Vec<&str> = b.split_whitespace().collect();
    if wa.is_empty() || wb.is_empty() {
        return 0.0;
    }
    let inter = wa.iter().filter(|w| wb.contains(w)).count();
    let union = wa.len() + wb.len() - inter;
    if union == 0 {
        return 0.0;
    }
    inter as f64 / union as f64
}

/// Порог схожести для «тот же запрос»: 0.6 — запросы, делящие большинство слов.
const SAME_QUERY_THRESHOLD: f64 = 0.6;

/// Предсказать следующий запрос по истории (марковская цепь первого порядка).
///
/// Для каждого вхождения текущего запроса (по нормализованной схожести) смотрим,
/// что шло следующим, и агрегируем частоты. Возвращает топ `top_k` кандидатов
/// с уверенностью = доля переходов на этот следующий запрос.
pub fn predict_next(history: &[QueryLogEntry], current: &str, top_k: usize) -> Vec<Prediction> {
    let cur = normalize(current);
    if cur.is_empty() {
        return Vec::new();
    }

    // Сопоставляем позиции в истории с текущим запросом.
    let mut transitions: Vec<&QueryLogEntry> = Vec::new(); // следующие записи
    let mut matches = 0usize;
    for (i, entry) in history.iter().enumerate() {
        if i + 1 < history.len() {
            let sim = jaccard(&cur, &normalize(&entry.query));
            if sim >= SAME_QUERY_THRESHOLD {
                matches += 1;
                transitions.push(&history[i + 1]);
            }
        }
    }
    if transitions.is_empty() {
        return Vec::new();
    }

    // Группируем переходы по нормализованному следующему запросу.
    use std::collections::BTreeMap;
    struct Acc {
        query: String,
        intent: String,
        entities: Vec<String>,
        count: usize,
    }
    let mut map: BTreeMap<String, Acc> = BTreeMap::new();
    for next in transitions {
        let key = normalize(&next.query);
        let acc = map.entry(key).or_insert_with(|| Acc {
            query: next.query.clone(),
            intent: next.intent_type.clone(),
            entities: Vec::new(),
            count: 0,
        });
        acc.count += 1;
        for e in &next.entities {
            if !acc.entities.contains(e) && acc.entities.len() < 10 {
                acc.entities.push(e.clone());
            }
        }
    }

    let total = matches as f64;
    let mut out: Vec<Prediction> = map
        .into_values()
        .map(|a| Prediction {
            suggested_query: a.query,
            confidence: a.count as f64 / total,
            intent_type: a.intent,
            entities: a.entities,
            matches: a.count,
        })
        .collect();
    out.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(top_k);
    out
}

/// Прогрев: сущности, которые с наибольшей вероятностью понадобятся сразу
/// после текущего запроса. Объединяет предсказания, ранжирует по уверенности.
pub fn prewarm_entities(predictions: &[Prediction]) -> Vec<String> {
    use std::collections::BTreeMap;
    let mut scores: BTreeMap<String, f64> = BTreeMap::new();
    for p in predictions {
        for e in &p.entities {
            *scores.entry(e.clone()).or_insert(0.0) += p.confidence;
        }
    }
    let mut v: Vec<(String, f64)> = scores.into_iter().collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    v.into_iter().map(|(e, _)| e).collect()
}

/// Человекочитаемый рендер предсказаний.
pub fn render_predictions(predictions: &[Prediction]) -> String {
    if predictions.is_empty() {
        return "No predictions yet — history is too small. Keep asking questions.".to_string();
    }
    let mut out = String::with_capacity(256);
    for p in predictions {
        out.push_str(&format!(
            "  {:.0}% — \"{}\" (intent {}, {} transition(s))",
            p.confidence * 100.0,
            p.suggested_query,
            p.intent_type,
            p.matches,
        ));
        if !p.entities.is_empty() {
            out.push_str(&format!(
                " → prewarm: {}",
                p.entities
                    .iter()
                    .take(4)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(query: &str, intent: &str, entities: &[&str]) -> QueryLogEntry {
        QueryLogEntry {
            query: query.to_string(),
            intent_type: intent.to_string(),
            entities: entities.iter().map(|s| s.to_string()).collect(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn history() -> Vec<QueryLogEntry> {
        vec![
            entry("how does auth work", "explain", &["e-auth", "e-token"]),
            entry(
                "where are sessions stored",
                "explore",
                &["e-session", "e-db"],
            ),
            entry("how does auth work", "explain", &["e-auth"]),
            entry("rotate the jwt secret", "change", &["e-config"]),
            entry("how does auth work", "explain", &["e-auth", "e-jwt"]),
            entry("where are sessions stored", "explore", &["e-session"]),
            entry("check the config file", "explore", &["e-config"]),
        ]
    }

    #[test]
    fn normalize_lowercases_and_sorts() {
        assert_eq!(normalize("How Does AUTH work??"), "auth does how work");
        assert_eq!(normalize("auth auth auth"), "auth");
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("!@#$%^"), "");
    }

    #[test]
    fn jaccard_measures_overlap() {
        assert_eq!(jaccard("auth work", "auth work"), 1.0);
        // Общее {auth}, объединение {auth, work, sessions} → 1/3.
        assert!((jaccard("auth work", "auth sessions") - 1.0 / 3.0).abs() < 0.001);
        assert_eq!(jaccard("auth work", "sessions db"), 0.0);
        assert_eq!(jaccard("", "auth"), 0.0);
    }

    #[test]
    fn predicts_most_common_next_query() {
        let preds = predict_next(&history(), "how does auth work", 5);
        assert!(!preds.is_empty());
        // "where are sessions stored" шёл 2 раза после "auth" — топ-1.
        assert_eq!(preds[0].suggested_query, "where are sessions stored");
        assert_eq!(preds[0].matches, 2);
        assert!((preds[0].confidence - 2.0 / 3.0).abs() < 0.001);
        // Интент и сущности предсказанного запроса проброшены для прогрева.
        assert_eq!(preds[0].intent_type, "explore");
        assert!(preds[0].entities.contains(&"e-session".to_string()));
    }

    #[test]
    fn similar_query_uses_jaccard_threshold() {
        let mut h = history();
        // Вариация формулировки: "auth" → "sessions" только 1 раз.
        h.push(entry(
            "explain the auth flow please",
            "explain",
            &["e-auth"],
        ));
        h.push(entry("list all sessions", "explore", &["e-session"]));
        let preds = predict_next(&h, "how does auth work", 5);
        assert_eq!(preds[0].suggested_query, "where are sessions stored");
    }

    #[test]
    fn empty_or_unseen_query_returns_nothing() {
        assert!(predict_next(&history(), "totally unrelated topic xyz", 3).is_empty());
        assert!(predict_next(&[], "how does auth work", 3).is_empty());
    }

    #[test]
    fn confidence_sums_to_one_for_top_k() {
        let preds = predict_next(&history(), "how does auth work", 10);
        let total: f64 = preds.iter().map(|p| p.confidence).sum();
        assert!((total - 1.0).abs() < 0.001, "sum={total}");
    }

    #[test]
    fn prewarm_ranks_entities_by_confidence() {
        let preds = predict_next(&history(), "how does auth work", 5);
        let entities = prewarm_entities(&preds);
        assert!(!entities.is_empty());
        // e-session и e-db — сущности самого частого предсказанного запроса —
        // должны быть в топе (первые два балла выше остальных).
        assert!(entities[0] == "e-session" || entities[0] == "e-db");
        assert!(entities.contains(&"e-session".to_string()));
        assert!(entities.contains(&"e-config".to_string()));
    }

    #[test]
    fn render_shows_predictions() {
        let preds = predict_next(&history(), "how does auth work", 3);
        let text = render_predictions(&preds);
        assert!(text.contains('%'));
        assert!(text.contains("sessions"));
        assert!(render_predictions(&[]).contains("No predictions"));
    }
}
