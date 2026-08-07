//! Memory Trust lifecycle service.
//!
//! Implements the "управляемая, проверяемая память" idea: memories carry a
//! trust state (Current / Superseded / Conflicted / UserConfirmed / Inferred),
//! can replace each other explicitly, and can be flagged as conflicting when a
//! semantically close but factually different record appears.

use chrono::Utc;

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::MemoryState;
use crate::core::result::Result;
use crate::storage::sqlite::SqliteMemoryRepository;

/// Threshold (cosine similarity) above which two memories are considered
/// "about the same thing" and worth a conflict check.
pub const CONFLICT_SIMILARITY: f64 = 0.82;

/// Threshold below which we never flag a conflict (avoids noise on broad
/// topics like "project setup").
pub const CONFLICT_SIMILARITY_MIN: f64 = 0.60;

/// Semantic overlap between two texts — word-level Dice coefficient.
///
/// Robust for both English and Russian: two texts about the same topic share
/// most of their vocabulary even when phrased differently.
pub fn text_overlap(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let words = |s: &str| -> Vec<String> {
        s.split_whitespace()
            .map(|w| {
                w.chars()
                    .filter(|c| !c.is_ascii_punctuation())
                    .flat_map(|c| c.to_lowercase())
                    .collect::<String>()
            })
            .filter(|w| !w.is_empty())
            .collect()
    };
    let wa = words(a);
    let wb = words(b);
    if wa.is_empty() || wb.is_empty() {
        return 0.0;
    }

    let set_a: std::collections::HashSet<String> = wa.into_iter().collect();
    let set_b: std::collections::HashSet<String> = wb.into_iter().collect();
    let common = set_a.intersection(&set_b).count();

    let denom = set_a.len() + set_b.len();
    if denom == 0 {
        return 0.0;
    }
    2.0 * common as f64 / denom as f64
}

/// Significant words (≥3 chars, not stop words) that appear in exactly one of
/// the two texts — the factual divergence between two statements.
fn divergent_words(a: &str, b: &str) -> (Vec<String>, Vec<String>) {
    const STOP: &[&str] = &[
        "the", "and", "for", "with", "from", "that", "this", "have", "was", "are", "not", "but",
        "its", "has", "had", "will", "would", "should", "into", "than", "then", "them", "they",
        "you", "your", "our", "all", "can", "use", "used", "using", "also", "only", "just", "via",
    ];
    let words = |s: &str| -> std::collections::HashSet<String> {
        s.split_whitespace()
            .map(|w| {
                w.chars()
                    .filter(|c| !c.is_ascii_punctuation())
                    .flat_map(|c| c.to_lowercase())
                    .collect::<String>()
            })
            .filter(|w| w.len() >= 3 && !STOP.contains(&w.as_str()))
            .collect()
    };
    let wa = words(a);
    let wb = words(b);
    let only_a: Vec<String> = wa.difference(&wb).cloned().collect();
    let only_b: Vec<String> = wb.difference(&wa).cloned().collect();
    (only_a, only_b)
}

/// True when two texts are about the same topic but state different facts:
/// both sides have significant words the other does not.
pub fn is_conflicting_pair(a: &str, b: &str) -> bool {
    if text_overlap(a, b) < CONFLICT_SIMILARITY {
        return false;
    }
    let (only_a, only_b) = divergent_words(a, b);
    !only_a.is_empty() && !only_b.is_empty()
}

/// Semantic similarity between a candidate memory and all existing memories.
///
/// Returns (memory_id, similarity) pairs above `min_similarity`, best first.
/// Falls back to text-overlap when the semantic model is not loaded so the
/// lifecycle still works on machines without the embedding model.
pub async fn find_similar_memories(
    repo: &SqliteMemoryRepository,
    title: &str,
    content: &str,
    min_similarity: f64,
) -> Result<Vec<(EntityId, f64)>> {
    let mut results: Vec<(EntityId, f64)> = Vec::new();

    // 1. Semantic search — only when the model is available.
    if let Ok(conn) = crate::db::open_connection()
        && let Ok(search) = crate::core::context::semantic_search::SemanticSearch::new(conn)
        && search.is_model_loaded()
    {
        let combined = format!("{} {}", title, content);
        if let Ok(hits) = search.search(&combined, 10) {
            for (id, sim) in hits {
                if sim >= min_similarity {
                    results.push((id, sim));
                }
            }
        }
    }

    // 2. Text-overlap complement. Catches memories the semantic index does not
    //    know about yet (fresh inserts) and — crucially — can *raise* the score
    //    of a record the semantic model underestimated. Short, fact-dense texts
    //    (e.g. "Use PostgreSQL" vs "Use MySQL") can score lower on embedding
    //    cosine similarity than on word overlap, which would hide genuine
    //    conflicts from detect_and_mark_conflicts. Keep the stronger signal.
    if let Ok(records) = repo.list(10_000, 0).await {
        for rec in records {
            let overlap = text_overlap(
                &format!("{} {}", title, content),
                &format!("{} {}", rec.title, rec.content),
            );
            if overlap >= min_similarity {
                match results.iter_mut().find(|(id, _)| id == &rec.id) {
                    Some(slot) => {
                        if overlap > slot.1 {
                            slot.1 = overlap;
                        }
                    }
                    None => results.push((rec.id, overlap)),
                }
            }
        }
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(10);
    Ok(results)
}

/// Check a newly saved/updated memory against the existing pool and mark both
/// sides `Conflicted` when they are about the same thing but say different
/// things. Never conflicts a memory with itself, and never downgrades a
/// `UserConfirmed` record automatically — that requires a human decision.
pub async fn detect_and_mark_conflicts(
    repo: &SqliteMemoryRepository,
    candidate: &MemoryRecord,
) -> Result<()> {
    let similar = find_similar_memories(
        repo,
        &candidate.title,
        &candidate.content,
        CONFLICT_SIMILARITY_MIN,
    )
    .await?;

    let candidate_text = format!("{} {}", candidate.title, candidate.content);
    let mut changed: Vec<EntityId> = Vec::new();

    for (other_id, similarity) in similar {
        if other_id == candidate.id {
            continue;
        }
        if similarity < CONFLICT_SIMILARITY {
            continue;
        }

        let Some(mut other) = repo.get_by_id(&other_id).await? else {
            continue;
        };
        // Respect explicit human confirmations: never silently demote them.
        if other.memory_state == MemoryState::UserConfirmed {
            continue;
        }

        let other_text = format!("{} {}", other.title, other.content);
        // Same statement restated, not a conflict; only genuinely divergent
        // facts about the same topic trigger the flag.
        if !is_conflicting_pair(&candidate_text, &other_text) {
            continue;
        }

        // Mark both sides. The newer record is the candidate; the older one
        // gets Conflicted too so the trust UI surfaces the pair.
        if other.memory_state != MemoryState::Conflicted {
            other.memory_state = MemoryState::Conflicted;
            other.touch();
            repo.update(&other).await?;
            changed.push(other_id);
        }
    }

    // If we found a conflicting pair, the candidate is conflicted as well —
    // unless the caller explicitly confirmed it.
    if !changed.is_empty()
        && candidate.memory_state != MemoryState::UserConfirmed
        && candidate.memory_state != MemoryState::Conflicted
    {
        let mut candidate = candidate.clone();
        candidate.memory_state = MemoryState::Conflicted;
        candidate.touch();
        repo.update(&candidate).await?;
    }

    Ok(())
}

/// Age-aware demotion: memories with an `expires_at` in the past are marked
/// `Superseded` (their content should be re-verified). Returns how many were
/// demoted.
pub async fn demote_expired(repo: &SqliteMemoryRepository) -> Result<u64> {
    let records = repo.list(100_000, 0).await?;
    let now = Utc::now();
    let mut demoted = 0u64;

    for mut rec in records {
        let expired = rec.expires_at.map(|exp| exp < now).unwrap_or(false);
        if expired && rec.memory_state == MemoryState::Current {
            rec.memory_state = MemoryState::Superseded;
            rec.touch();
            repo.update(&rec).await?;
            demoted += 1;
        }
    }
    Ok(demoted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::types::MemorySource;

    fn repo() -> SqliteMemoryRepository {
        SqliteMemoryRepository::new_in_memory().unwrap()
    }

    #[test]
    fn overlap_detects_similar_text() {
        let a = "Use SQLite WAL mode for concurrent reads";
        let b = "We decided to enable SQLite WAL for reads";
        let c = "The cat sat on the mat";
        assert!(text_overlap(a, b) > 0.5);
        assert!(text_overlap(a, c) < 0.1);
    }

    #[tokio::test]
    async fn conflict_marks_both_sides() {
        let r = repo();
        let first = MemoryRecord::new(
            "Database choice".to_string(),
            "Use PostgreSQL as the primary database".to_string(),
            "alice".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.save(&first).await.unwrap();

        let second = MemoryRecord::new(
            "Database choice".to_string(),
            "Use MySQL as the primary database".to_string(),
            "bob".to_string(),
            MemorySource::Manual,
        )
        .unwrap();

        // Register the second via the repo so it has an id, then run the detector.
        r.save(&second).await.unwrap();
        detect_and_mark_conflicts(&r, &second).await.unwrap();

        let first_after = r.get_by_id(&first.id).await.unwrap().unwrap();
        let second_after = r.get_by_id(&second.id).await.unwrap().unwrap();
        assert_eq!(first_after.memory_state, MemoryState::Conflicted);
        assert_eq!(second_after.memory_state, MemoryState::Conflicted);
    }

    #[tokio::test]
    async fn same_statement_not_conflict() {
        let r = repo();
        let first = MemoryRecord::new(
            "Auth".to_string(),
            "Use JWT with 15 minute expiry".to_string(),
            "alice".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.save(&first).await.unwrap();

        let second = MemoryRecord::new(
            "Auth decision".to_string(),
            "Use JWT with 15 minute expiry (confirmed)".to_string(),
            "bob".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.save(&second).await.unwrap();

        detect_and_mark_conflicts(&r, &second).await.unwrap();

        let first_after = r.get_by_id(&first.id).await.unwrap().unwrap();
        assert_eq!(
            first_after.memory_state,
            MemoryState::Current,
            "near-identical restatement must not be flagged as a conflict"
        );
    }

    #[tokio::test]
    async fn user_confirmed_not_demoted() {
        let r = repo();
        let mut confirmed = MemoryRecord::new(
            "Port".to_string(),
            "Run the API on port 8080".to_string(),
            "alice".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        confirmed.memory_state = MemoryState::UserConfirmed;
        confirmed.confirmed_at = Some(Utc::now());
        r.save(&confirmed).await.unwrap();

        let challenger = MemoryRecord::new(
            "Port".to_string(),
            "Run the API on port 9090".to_string(),
            "bob".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.save(&challenger).await.unwrap();

        detect_and_mark_conflicts(&r, &challenger).await.unwrap();

        let confirmed_after = r.get_by_id(&confirmed.id).await.unwrap().unwrap();
        assert_eq!(confirmed_after.memory_state, MemoryState::UserConfirmed);
    }
}
