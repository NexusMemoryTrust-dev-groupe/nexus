//! Memory Trust lifecycle service.
//!
//! Implements the "управляемая, проверяемая память" idea: memories carry a
//! trust state (Current / Superseded / Conflicted / UserConfirmed / Inferred),
//! can replace each other explicitly, and can be flagged as conflicting when a
//! semantically close but factually different record appears.

use chrono::Utc;

use crate::core::entity_id::EntityId;
use crate::core::memory::layer::LayerClassifier;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::{LayerAssignment, LayerHistoryEntry, MemoryState};
use crate::core::result::Result;
use crate::storage::sqlite::SqliteMemoryRepository;

/// Threshold (cosine similarity) above which two memories are considered
/// "about the same thing" and worth a conflict check.
pub const CONFLICT_SIMILARITY: f64 = 0.82;

/// Threshold below which we never flag a conflict (avoids noise on broad
/// topics like "project setup").
pub const CONFLICT_SIMILARITY_MIN: f64 = 0.60;

/// Semantic (embedding-cosine) threshold for a *paraphrased* conflict: two
/// records that share almost no vocabulary (Dice below [`CONFLICT_SIMILARITY`])
/// can still be about the same fact and say different things. The Dice gate
/// alone misses "PostgreSQL is the primary database" vs "we migrated from
/// PostgreSQL to SQLite" (overlap ≈ 0.44–0.67). This cosine gate catches the
/// paraphrase while [`CONFLICT_SIMILARITY_MIN`] still filters out broad topics.
///
/// Calibrated against real all-MiniLM-L6-v2 measurements (nexus_bench):
/// genuine paraphrased conflicts scored 0.627–0.726, so 0.62 is the ceiling
/// that still catches the weakest real conflict. Compatibility with
/// "similarity ≠ contradiction" (primary DB vs used-for-analytics) is enforced
/// separately by the shared-stems requirement in [`is_conflicting_pair`].
pub const CONFLICT_SEMANTIC_SIMILARITY: f64 = 0.62;

/// Run the signature classifier on a record unless the layer is pinned by an
/// explicit user choice (the last history entry is `user`). Fills the layer
/// provenance: layer, confidence, reason, updated_at and a history entry.
///
/// Used by every write path (Tauri commands, copilot, MCP) so no memory can
/// enter the store without a cognitive layer.
pub fn auto_classify(record: &mut MemoryRecord) {
    let pinned_by_user = record
        .layer_history
        .last()
        .map(|e| e.by == LayerAssignment::User)
        .unwrap_or(false);
    if pinned_by_user {
        return;
    }

    let classification = LayerClassifier::classify(
        &record.title,
        &record.content,
        record.source.clone(),
        record.memory_state.clone(),
        record.importance_score,
    );
    record.layer = classification.layer;
    record.layer_confidence = classification.confidence;
    record.layer_reason = classification.reason;
    record.layer_updated_at = Some(Utc::now());
    record.layer_history.push(LayerHistoryEntry {
        layer: record.layer.clone(),
        confidence: record.layer_confidence,
        reason: record.layer_reason.clone(),
        at: Utc::now().to_rfc3339(),
        by: LayerAssignment::Classifier,
    });
}

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

/// Significant words (≥3 chars, not stop words) in a text — the vocabulary
/// that carries factual meaning ("postgresql", "database", "migrated").
fn significant_words(s: &str) -> std::collections::HashSet<String> {
    const STOP: &[&str] = &[
        "the", "and", "for", "with", "from", "that", "this", "have", "was", "are", "not", "but",
        "its", "has", "had", "will", "would", "should", "into", "than", "then", "them", "they",
        "you", "your", "our", "all", "can", "use", "used", "using", "also", "only", "just", "via",
    ];
    s.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| !c.is_ascii_punctuation())
                .flat_map(|c| c.to_lowercase())
                .collect::<String>()
        })
        .filter(|w| w.len() >= 3 && !STOP.contains(&w.as_str()))
        .collect()
}

/// Significant words that appear in exactly one of the two texts — the
/// factual divergence between two statements.
fn divergent_words(a: &str, b: &str) -> (Vec<String>, Vec<String>) {
    let wa = significant_words(a);
    let wb = significant_words(b);
    let only_a: Vec<String> = wa.difference(&wb).cloned().collect();
    let only_b: Vec<String> = wb.difference(&wa).cloned().collect();
    (only_a, only_b)
}

/// Lightweight stem: first 4 characters of a significant word. Aligns
/// inflections without a full stemmer ("deployed"→"depl", "deployments"→"depl")
/// while keeping unrelated words apart ("primary"→"prim" vs "production"→"prod").
fn stem(word: &str) -> String {
    word.chars().take(4).collect()
}

/// Number of shared significant word-stems between two texts. A genuine
/// paraphrase conflict shares the claim skeleton ("deploy…aws" vs
/// "deployments…aws" share `depl`+`aws`), whereas merely-compatible statements
/// about the same entity share only that entity ("postgresql") — 1 stem. This
/// is what separates real conflicts from "similarity ≠ contradiction" cases.
fn shared_significant_stems(a: &str, b: &str) -> usize {
    let stems_a: std::collections::HashSet<String> =
        significant_words(a).iter().map(|w| stem(w)).collect();
    let stems_b: std::collections::HashSet<String> =
        significant_words(b).iter().map(|w| stem(w)).collect();
    stems_a.intersection(&stems_b).count()
}

/// True when two texts are about the same topic but state different facts:
/// both sides have significant words the other does not.
///
/// Two channels decide whether the pair is worth the divergence check:
/// - lexical: Dice text-overlap ≥ [`CONFLICT_SIMILARITY`];
/// - semantic: embedding cosine ≥ [`CONFLICT_SEMANTIC_SIMILARITY`] **and** at
///   least two shared significant stems. The cosine catches paraphrases that
///   share almost no vocabulary ("PostgreSQL is the primary database" vs "we
///   migrated from PostgreSQL to SQLite"); the stems requirement blocks
///   semantically-close but *compatible* statements ("PostgreSQL is the primary
///   database" vs "PostgreSQL is used for analytics") — similarity alone is
///   NOT a contradiction.
///
/// The caller passes `semantic` (e.g. the cosine from `find_similar_memories`);
/// pass `None` when only the lexical channel is available (model not loaded).
/// Either channel being strong is enough to proceed with the divergence check —
/// the divergence check is what decides.
pub fn is_conflicting_pair(a: &str, b: &str, semantic: Option<f64>) -> bool {
    let lexical_ok = text_overlap(a, b) >= CONFLICT_SIMILARITY;
    let semantic_ok = semantic
        .map(|s| s >= CONFLICT_SEMANTIC_SIMILARITY && shared_significant_stems(a, b) >= 2)
        .unwrap_or(false);
    if !lexical_ok && !semantic_ok {
        return false;
    }
    let (only_a, only_b) = divergent_words(a, b);
    !only_a.is_empty() && !only_b.is_empty()
}

/// One hit of [`find_similar_memories`]: keeps the two similarity signals
/// separate so the caller can apply per-channel thresholds instead of a single
/// merged score that hides a strong semantic match behind a weak lexical one.
pub struct SimilarityHit {
    pub id: EntityId,
    /// Embedding cosine similarity (0.0 when the model is unavailable).
    pub semantic: f64,
    /// Dice text-overlap similarity.
    pub lexical: f64,
}

/// Semantic similarity between a candidate memory and all existing memories.
///
/// Returns [`SimilarityHit`]s where at least one channel is above
/// `min_similarity`, best combined score first. The semantic channel only
/// fires when the embedding model is loaded; the lexical channel always runs
/// (catches fresh inserts the semantic index does not know yet).
pub async fn find_similar_memories(
    repo: &SqliteMemoryRepository,
    title: &str,
    content: &str,
    min_similarity: f64,
) -> Result<Vec<SimilarityHit>> {
    let mut results: Vec<SimilarityHit> = Vec::new();

    // 1. Semantic search — only when the model is available.
    //
    // Plan 7.6: `hybrid_retrieval` gates this channel. ON (default) merges
    // embeddings with lexical overlap (step 2) for the hybrid signal; OFF
    // drops the semantic index so similarity comes from embeddings alone.
    if crate::core::config::is_enabled(crate::core::config::FEATURE_HYBRID_RETRIEVAL)
        && let Ok(conn) = crate::db::open_connection()
        && let Ok(search) = crate::core::context::semantic_search::SemanticSearch::new(conn)
        && search.is_model_loaded()
    {
        let combined = format!("{} {}", title, content);
        if let Ok(hits) = search.search(&combined, 10) {
            for (id, sim) in hits {
                if sim >= min_similarity {
                    results.push(SimilarityHit {
                        id,
                        semantic: sim,
                        lexical: 0.0,
                    });
                }
            }
        }
    }

    // 2. Text-overlap complement. Catches memories the semantic index does not
    //    know about yet (fresh inserts) and — crucially — can *raise* the
    //    lexical score of a record the semantic model underestimated. Short,
    //    fact-dense texts (e.g. "Use PostgreSQL" vs "Use MySQL") can score lower
    //    on embedding cosine similarity than on word overlap, which would hide
    //    genuine conflicts from detect_and_mark_conflicts. Keep both signals.
    if let Ok(records) = repo.list(10_000, 0).await {
        for rec in records {
            let overlap = text_overlap(
                &format!("{} {}", title, content),
                &format!("{} {}", rec.title, rec.content),
            );
            if overlap >= min_similarity {
                match results.iter_mut().find(|h| h.id == rec.id) {
                    Some(slot) => {
                        if overlap > slot.lexical {
                            slot.lexical = overlap;
                        }
                    }
                    None => results.push(SimilarityHit {
                        id: rec.id,
                        semantic: 0.0,
                        lexical: overlap,
                    }),
                }
            }
        }
    }

    results.sort_by(|a, b| {
        b.semantic
            .max(b.lexical)
            .partial_cmp(&a.semantic.max(a.lexical))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
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

    for hit in similar {
        if hit.id == candidate.id {
            continue;
        }
        // Either channel strong enough to warrant a conflict check.
        if hit.semantic < CONFLICT_SEMANTIC_SIMILARITY && hit.lexical < CONFLICT_SIMILARITY {
            continue;
        }

        let Some(mut other) = repo.get_by_id(&hit.id).await? else {
            continue;
        };
        // Respect explicit human confirmations: never silently demote them.
        if other.memory_state == MemoryState::UserConfirmed {
            continue;
        }

        let other_text = format!("{} {}", other.title, other.content);
        // Same statement restated, not a conflict; only genuinely divergent
        // facts about the same topic trigger the flag. The semantic channel
        // handles paraphrases that barely share vocabulary.
        //
        // Plan 7.6: `semantic_conflict_v2` gates the embedding channel. OFF
        // rolls back to lexical-only verdicts (cheaper, weaker on paraphrases);
        // ON (default) keeps the hybrid semantic+lexical judgement.
        let semantic =
            if crate::core::config::is_enabled(crate::core::config::FEATURE_SEMANTIC_CONFLICT_V2) {
                Some(hit.semantic)
            } else {
                None
            };
        if !is_conflicting_pair(&candidate_text, &other_text, semantic) {
            continue;
        }

        // Mark both sides. The newer record is the candidate; the older one
        // gets Conflicted too so the trust UI surfaces the pair.
        if other.memory_state != MemoryState::Conflicted {
            other.memory_state = MemoryState::Conflicted;
            other.touch();
            repo.update(&other).await?;
            changed.push(hit.id);
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

    #[test]
    fn auto_classify_assigns_a_layer_and_provenance() {
        let mut record = MemoryRecord::new(
            "Deployment decision".to_string(),
            "We decided to run the API on port 8080 with blue-green deploys".to_string(),
            "alice".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        assert_eq!(record.layer_history.len(), 0);

        auto_classify(&mut record);

        // A decision-flavoured record must land on the Decision layer, with
        // confidence, a human-readable reason and a classifier history entry.
        assert_eq!(
            record.layer,
            crate::core::memory::types::MemoryLayer::Decision,
            "auto_classify must pick the Decision layer for decision text"
        );
        assert!(
            (0.0..=1.0).contains(&record.layer_confidence),
            "confidence must be bounded"
        );
        assert!(!record.layer_reason.is_empty());
        assert!(record.layer_updated_at.is_some());
        assert_eq!(record.layer_history.len(), 1);
        assert_eq!(
            record.layer_history[0].by,
            LayerAssignment::Classifier,
            "the history entry must record the classifier as the author"
        );
    }

    #[test]
    fn auto_classify_respects_user_pinned_layer() {
        let mut record = MemoryRecord::new(
            "Journal".to_string(),
            "Today I refactored the conflict engine and wrote tests".to_string(),
            "alice".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        // The user pinned the layer explicitly — the classifier must not
        // overwrite it, even though the text reads as an episodic journal.
        record.layer = crate::core::memory::types::MemoryLayer::Working;
        record.layer_history.push(LayerHistoryEntry {
            layer: record.layer.clone(),
            confidence: 1.0,
            reason: "user pinned".into(),
            at: Utc::now().to_rfc3339(),
            by: LayerAssignment::User,
        });

        auto_classify(&mut record);

        assert_eq!(
            record.layer,
            crate::core::memory::types::MemoryLayer::Working,
            "a user-pinned layer must survive auto_classify"
        );
        assert_eq!(
            record.layer_history.len(),
            1,
            "no classifier entry may be appended over a user pin"
        );
    }

    // ── Hybrid conflict channels ──

    #[test]
    fn is_conflicting_pair_catches_paraphrase_via_semantic_channel() {
        // The benchmark failure pair: low Dice overlap, high semantic similarity.
        // The old Dice-only gate rejected it (0/2 paraphrases caught).
        let a = "PostgreSQL is the primary production database for all services";
        let b = "We migrated the production database from PostgreSQL to SQLite for simplicity";

        let dice = text_overlap(a, b);
        assert!(
            dice < CONFLICT_SIMILARITY,
            "test precondition: Dice {dice:.3} must be below the lexical gate"
        );

        // Semantic channel alone (high cosine) is enough to reach the
        // divergence check, and the divergence is real (migrated/sqlite vs
        // primary/services).
        assert!(
            is_conflicting_pair(a, b, Some(CONFLICT_SEMANTIC_SIMILARITY + 0.05)),
            "paraphrased conflict must be caught via the semantic channel"
        );
    }

    #[test]
    fn is_conflicting_pair_rejects_compatible_statements_even_at_high_cosine() {
        // The user's exact guidance case: "PostgreSQL is the primary DB" and
        // "PostgreSQL is used for analytics" are semantically very close
        // (real cosine ~0.8 on MiniLM) but describe *compatible* facts — the
        // database can be primary AND used for analytics. The shared-stems
        // requirement (≥2) is what blocks this false positive: the pair shares
        // only the entity stem ("postg"), not a claim skeleton.
        let a = "PostgreSQL is the primary database";
        let b = "PostgreSQL is used for analytics";
        assert_eq!(
            shared_significant_stems(a, b),
            1,
            "compatible pair must share only the entity stem"
        );
        assert!(
            !is_conflicting_pair(a, b, Some(0.85)),
            "high cosine must NOT flag compatible statements (similarity ≠ contradiction)"
        );
    }

    #[test]
    fn is_conflicting_pair_rejects_semantic_below_threshold() {
        let a = "PostgreSQL is the primary database";
        let b = "We migrated the production database from PostgreSQL to SQLite for simplicity";
        assert!(
            !is_conflicting_pair(a, b, Some(CONFLICT_SEMANTIC_SIMILARITY - 0.10)),
            "a cosine below the semantic gate must not trigger the conflict"
        );
    }

    #[test]
    fn is_conflicting_pair_keeps_lexical_channel_without_model() {
        // No model (semantic = None): the lexical channel must still catch the
        // classic near-dup pair.
        let a = "Use PostgreSQL as the primary database";
        let b = "Use MySQL as the primary database";
        assert!(is_conflicting_pair(a, b, None));
    }

    #[tokio::test]
    async fn find_similar_memories_keeps_channels_separate() {
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
        r.save(&second).await.unwrap();

        // Dice for this near-dup pair ≈ 0.75 ≥ CONFLICT_SIMILARITY_MIN, so the
        // lexical channel must surface it as a candidate even without a model;
        // the semantic field must stay 0.0 (no bogus guess).
        let hits =
            find_similar_memories(&r, &second.title, &second.content, CONFLICT_SIMILARITY_MIN)
                .await
                .unwrap();
        let hit = hits
            .iter()
            .find(|h| h.id == first.id)
            .expect("the other record must be a candidate");
        assert_eq!(hit.semantic, 0.0, "no model loaded → semantic must be 0.0");
        assert!(
            hit.lexical >= CONFLICT_SIMILARITY_MIN,
            "lexical channel must keep the near-dup as a candidate"
        );
    }
}
