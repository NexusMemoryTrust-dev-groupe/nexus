use serde::{Deserialize, Serialize};

/// Source of a memory record — where the information came from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemorySource {
    Manual,
    Git,
    Telegram,
    Email,
    Meeting,
    Document,
    AiGenerated,
    Compressed,
}

/// Visibility level — controls access to a memory record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryVisibility {
    Public,
    Private,
    Restricted,
}

/// How the memory was captured.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryCaptureMode {
    Passive,
    Assisted,
    Automatic,
}

/// Memory layer — cognitive tier of a memory record.
///
/// Six cognitive layers model how knowledge matures and how it is applied:
///
///   Working    — the active task, the hot zone ("fixing the auth bug now").
///   Episodic   — events, experiments, what was tried ("yesterday we tried...").
///   Semantic   — stable facts about the system or the world ("auth uses JWT").
///   Procedural — how things are done here ("refresh tokens rotate via X").
///   Decision   — a decision with its rationale ("on Aug 3 we dropped Redis").
///   Strategic  — principles and long-term direction ("everything stays local").
///
/// The classifier (`core::memory::layer`) assigns layers automatically on
/// create/update; a user override always wins and is recorded in history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryLayer {
    /// Active task — the hot working zone.
    Working,
    /// Events, experiments, what was tried.
    Episodic,
    /// Stable facts about the system or world.
    Semantic,
    /// How to do things — order of actions.
    Procedural,
    /// A decision with its rationale.
    Decision,
    /// Principles and long-term direction.
    Strategic,
}

impl MemoryLayer {
    /// Canonical string form, stored in the database.
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryLayer::Working => "Working",
            MemoryLayer::Episodic => "Episodic",
            MemoryLayer::Semantic => "Semantic",
            MemoryLayer::Procedural => "Procedural",
            MemoryLayer::Decision => "Decision",
            MemoryLayer::Strategic => "Strategic",
        }
    }

    /// Parse a stored string, mapping legacy layer names onto the cognitive
    /// taxonomy (V18 migration): Raw → Episodic, Knowledge → Semantic,
    /// Wisdom → Strategic. Decision is unchanged. Unknown values fall back to
    /// Episodic (raw capture is closest to an event).
    pub fn parse(s: &str) -> MemoryLayer {
        match s {
            "Working" => MemoryLayer::Working,
            "Episodic" => MemoryLayer::Episodic,
            "Semantic" => MemoryLayer::Semantic,
            "Procedural" => MemoryLayer::Procedural,
            "Decision" => MemoryLayer::Decision,
            "Strategic" => MemoryLayer::Strategic,
            // Legacy names (V1–V17 data).
            "Raw" => MemoryLayer::Episodic,
            "Knowledge" => MemoryLayer::Semantic,
            "Wisdom" => MemoryLayer::Strategic,
            _ => MemoryLayer::Episodic,
        }
    }
}

/// Who assigned the layer — provenance of the classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LayerAssignment {
    /// The user explicitly picked the layer.
    User,
    /// The built-in signature classifier assigned it.
    Classifier,
    /// The V18 migration remapped a legacy layer name.
    Migration,
    /// Not recorded (legacy rows before V18).
    Unknown,
}

impl LayerAssignment {
    pub fn as_str(&self) -> &'static str {
        match self {
            LayerAssignment::User => "user",
            LayerAssignment::Classifier => "classifier",
            LayerAssignment::Migration => "migration",
            LayerAssignment::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> LayerAssignment {
        match s {
            "user" => LayerAssignment::User,
            "classifier" => LayerAssignment::Classifier,
            "migration" => LayerAssignment::Migration,
            _ => LayerAssignment::Unknown,
        }
    }
}

/// One entry of the layer history — a single recorded layer change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerHistoryEntry {
    pub layer: MemoryLayer,
    /// Confidence of this assignment, 0.0–1.0 (1.0 for user overrides).
    pub confidence: f64,
    /// Short human-readable reason for the assignment.
    pub reason: String,
    /// ISO-8601 timestamp of the change.
    pub at: String,
    /// Who made the assignment.
    pub by: LayerAssignment,
}

impl LayerHistoryEntry {
    /// Newest-first ordering for history lists.
    pub fn sort_newest_first(list: &mut [LayerHistoryEntry]) {
        list.sort_by(|a, b| b.at.cmp(&a.at));
    }
}

/// Current status of a memory record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryStatus {
    Active,
    Archived,
    Merged,
}

impl MemoryStatus {
    /// Formal status machine (plan 3.6).
    ///
    /// A record may be archived exactly once (Active → Archived) or merged into
    /// a canonical entity exactly once (Active → Merged). Both Archived and
    /// Merged are terminal: an archived record cannot be silently revived and a
    /// merged record cannot be split back into its own duplicate — resurrection
    /// must go through an explicit, audited recovery path, never a plain state
    /// flip. This is the "ARCHIVED→ACTIVE запрещён" guarantee.
    pub fn can_transition(from: &MemoryStatus, to: &MemoryStatus) -> bool {
        match (from, to) {
            (MemoryStatus::Active, MemoryStatus::Active) => true, // no-op update
            (MemoryStatus::Active, MemoryStatus::Archived) => true,
            (MemoryStatus::Active, MemoryStatus::Merged) => true,
            (MemoryStatus::Archived, MemoryStatus::Archived) => true, // no-op
            (MemoryStatus::Merged, MemoryStatus::Merged) => true,     // no-op
            _ => false,
        }
    }
}

/// Memory Trust lifecycle state — насколько памяти можно доверять сейчас.
///
/// Это ядро идеи «управляемая, проверяемая память»: память не просто хранится,
/// она имеет явный статус достоверности, который пользователь может видеть
/// и менять (подтвердить, заменить, пометить устаревшей).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryState {
    /// Актуально: считается верным в данный момент.
    Current,
    /// Заменено более новой памятью (supersedes_id / superseded_by_id).
    Superseded,
    /// Противоречит другой памяти — требует решения пользователя.
    Conflicted,
    /// Подтверждено пользователем явно (confirmed_at / confirmed_by).
    UserConfirmed,
    /// Выведено моделью, но не подтверждено человеком.
    Inferred,
}

impl MemoryState {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryState::Current => "Current",
            MemoryState::Superseded => "Superseded",
            MemoryState::Conflicted => "Conflicted",
            MemoryState::UserConfirmed => "UserConfirmed",
            MemoryState::Inferred => "Inferred",
        }
    }

    pub fn parse(s: &str) -> MemoryState {
        match s {
            "Superseded" => MemoryState::Superseded,
            "Conflicted" => MemoryState::Conflicted,
            "UserConfirmed" => MemoryState::UserConfirmed,
            "Inferred" => MemoryState::Inferred,
            _ => MemoryState::Current,
        }
    }

    /// Formal memory-trust state machine (plan 3.6).
    ///
    /// Trust only moves forward: a record can be promoted (Inferred → anything),
    /// resolved upward (Conflicted → UserConfirmed/Current/Superseded) or retired
    /// (Current/UserConfirmed → Superseded). Reviving a retired record
    /// (Superseded → *), silently demoting a user confirmation
    /// (UserConfirmed → Current/Conflicted/Inferred) or degrading a resolved
    /// conflict back to Inferred are all forbidden — those need an explicit new
    /// record or a deliberate user action, never a plain state flip.
    pub fn can_transition(from: &MemoryState, to: &MemoryState) -> bool {
        if from == to {
            return true; // no-op update
        }
        match (from, to) {
            // Promotion of model-inferred records.
            (MemoryState::Inferred, MemoryState::Current)
            | (MemoryState::Inferred, MemoryState::UserConfirmed)
            | (MemoryState::Inferred, MemoryState::Conflicted)
            | (MemoryState::Inferred, MemoryState::Superseded) => true,
            // Conflict resolution.
            (MemoryState::Conflicted, MemoryState::UserConfirmed)
            | (MemoryState::Conflicted, MemoryState::Current)
            | (MemoryState::Conflicted, MemoryState::Superseded) => true,
            // Retirement: only forward, never backward.
            (MemoryState::Current, MemoryState::Superseded)
            | (MemoryState::Current, MemoryState::Conflicted)
            | (MemoryState::Current, MemoryState::UserConfirmed)
            | (MemoryState::UserConfirmed, MemoryState::Superseded) => true,
            _ => false,
        }
    }
}

/// Обратная связь пользователя по памяти («полезно / нерелевантно / неверно»).
///
/// Логика одного голоса: `voted` хранит kind активного голоса пользователя
/// (useful / irrelevant / wrong). Повторный клик по той же кнопке снимает
/// голос, клик по другой — переключает. Так счётчики не растут бесконечно.
/// `note` — объяснение пользователя, которое система (копилот/RAG) использует,
/// чтобы понимать, почему память полезна или нет.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MemoryFeedback {
    pub useful: u32,
    pub irrelevant: u32,
    pub wrong: u32,
    /// Kind активного голоса пользователя: "useful" | "irrelevant" | "wrong".
    #[serde(default)]
    pub voted: Option<String>,
    /// Объяснение пользователя (почему память полезна/нерелевантна/неверна).
    #[serde(default)]
    pub note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_source_serialization() {
        let sources = vec![
            MemorySource::Manual,
            MemorySource::Git,
            MemorySource::Telegram,
            MemorySource::Email,
            MemorySource::Meeting,
            MemorySource::Document,
            MemorySource::AiGenerated,
            MemorySource::Compressed,
        ];
        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            let decoded: MemorySource = serde_json::from_str(&json).unwrap();
            assert_eq!(source, decoded);
        }
    }

    #[test]
    fn memory_visibility_serialization() {
        let vis = MemoryVisibility::Private;
        let json = serde_json::to_string(&vis).unwrap();
        let decoded: MemoryVisibility = serde_json::from_str(&json).unwrap();
        assert_eq!(vis, decoded);
    }

    #[test]
    fn memory_layer_serialization() {
        let layers = [
            MemoryLayer::Working,
            MemoryLayer::Episodic,
            MemoryLayer::Semantic,
            MemoryLayer::Procedural,
            MemoryLayer::Decision,
            MemoryLayer::Strategic,
        ];
        for layer in layers {
            let json = serde_json::to_string(&layer).unwrap();
            let decoded: MemoryLayer = serde_json::from_str(&json).unwrap();
            assert_eq!(layer, decoded);
        }
    }

    #[test]
    fn memory_layer_parse_roundtrip() {
        assert_eq!(MemoryLayer::parse("Working"), MemoryLayer::Working);
        assert_eq!(MemoryLayer::parse("Episodic"), MemoryLayer::Episodic);
        assert_eq!(MemoryLayer::parse("Semantic"), MemoryLayer::Semantic);
        assert_eq!(MemoryLayer::parse("Procedural"), MemoryLayer::Procedural);
        assert_eq!(MemoryLayer::parse("Decision"), MemoryLayer::Decision);
        assert_eq!(MemoryLayer::parse("Strategic"), MemoryLayer::Strategic);
    }

    #[test]
    fn memory_layer_parse_legacy_names() {
        // V1–V17 data: legacy ladder names map onto the cognitive taxonomy.
        assert_eq!(MemoryLayer::parse("Raw"), MemoryLayer::Episodic);
        assert_eq!(MemoryLayer::parse("Knowledge"), MemoryLayer::Semantic);
        assert_eq!(MemoryLayer::parse("Wisdom"), MemoryLayer::Strategic);
        // Unknown values fall back to Episodic, never panic.
        assert_eq!(MemoryLayer::parse("TotallyUnknown"), MemoryLayer::Episodic);
        assert_eq!(MemoryLayer::parse(""), MemoryLayer::Episodic);
    }

    #[test]
    fn memory_layer_as_str_stable() {
        assert_eq!(MemoryLayer::Working.as_str(), "Working");
        assert_eq!(MemoryLayer::Episodic.as_str(), "Episodic");
        assert_eq!(MemoryLayer::Semantic.as_str(), "Semantic");
        assert_eq!(MemoryLayer::Procedural.as_str(), "Procedural");
        assert_eq!(MemoryLayer::Decision.as_str(), "Decision");
        assert_eq!(MemoryLayer::Strategic.as_str(), "Strategic");
    }

    #[test]
    fn memory_status_serialization() {
        let status = MemoryStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        let decoded: MemoryStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn memory_capture_mode_serialization() {
        let mode = MemoryCaptureMode::Assisted;
        let json = serde_json::to_string(&mode).unwrap();
        let decoded: MemoryCaptureMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, decoded);
    }

    // ── Formal state machines (plan 3.6) ──

    #[test]
    fn status_machine_archives_and_merges_once() {
        use MemoryStatus::*;
        assert!(MemoryStatus::can_transition(&Active, &Archived));
        assert!(MemoryStatus::can_transition(&Active, &Merged));
        // No-op updates on the same state are always allowed.
        assert!(MemoryStatus::can_transition(&Active, &Active));
        assert!(MemoryStatus::can_transition(&Archived, &Archived));
        assert!(MemoryStatus::can_transition(&Merged, &Merged));
    }

    #[test]
    fn status_machine_forbids_resurrection_and_split() {
        use MemoryStatus::*;
        // "ARCHIVED→ACTIVE запрещён" (plan 3.6).
        assert!(!MemoryStatus::can_transition(&Archived, &Active));
        assert!(!MemoryStatus::can_transition(&Merged, &Active));
        assert!(!MemoryStatus::can_transition(&Merged, &Archived));
        assert!(!MemoryStatus::can_transition(&Archived, &Merged));
    }

    #[test]
    fn state_machine_allows_forward_promotions() {
        use MemoryState::*;
        // Model-inferred records can be promoted anywhere.
        for to in [Current, UserConfirmed, Conflicted, Superseded] {
            assert!(
                MemoryState::can_transition(&Inferred, &to),
                "Inferred -> {to:?} must be allowed"
            );
        }
        // Conflict resolution is allowed in every direction that ends the doubt.
        assert!(MemoryState::can_transition(&Conflicted, &UserConfirmed));
        assert!(MemoryState::can_transition(&Conflicted, &Current));
        assert!(MemoryState::can_transition(&Conflicted, &Superseded));
        // Retirement only forward.
        assert!(MemoryState::can_transition(&Current, &Superseded));
        assert!(MemoryState::can_transition(&Current, &Conflicted));
        assert!(MemoryState::can_transition(&Current, &UserConfirmed));
        assert!(MemoryState::can_transition(&UserConfirmed, &Superseded));
        // No-op updates.
        assert!(MemoryState::can_transition(&Current, &Current));
        assert!(MemoryState::can_transition(&UserConfirmed, &UserConfirmed));
    }

    #[test]
    fn state_machine_forbids_revival_after_superseded() {
        use MemoryState::*;
        // A retired record is terminal — reviving it needs a new record.
        for to in [Current, UserConfirmed, Conflicted, Inferred] {
            assert!(
                !MemoryState::can_transition(&Superseded, &to),
                "Superseded -> {to:?} must be forbidden"
            );
        }
    }

    #[test]
    fn state_machine_forbids_silent_demotion_of_user_confirmed() {
        use MemoryState::*;
        // A record the user explicitly confirmed must not be silently demoted.
        assert!(!MemoryState::can_transition(&UserConfirmed, &Current));
        assert!(!MemoryState::can_transition(&UserConfirmed, &Conflicted));
        assert!(!MemoryState::can_transition(&UserConfirmed, &Inferred));
        // Conflicted → Inferred is a degradation of a resolved state.
        assert!(!MemoryState::can_transition(&Conflicted, &Inferred));
        assert!(!MemoryState::can_transition(&Current, &Inferred));
    }

    #[test]
    fn state_machine_total_no_panic() {
        // The transition function must be total for every (from, to) pair.
        use MemoryState::*;
        let all = [Current, Superseded, Conflicted, UserConfirmed, Inferred];
        for from in &all {
            for to in &all {
                let _ = MemoryState::can_transition(from, to);
            }
        }
    }
}
