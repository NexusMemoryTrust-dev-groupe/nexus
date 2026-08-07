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

/// Memory layer — represents the level of processing/abstraction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryLayer {
    Raw,
    Knowledge,
    Decision,
    Wisdom,
}

/// Current status of a memory record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryStatus {
    Active,
    Archived,
    Merged,
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
        let layer = MemoryLayer::Knowledge;
        let json = serde_json::to_string(&layer).unwrap();
        let decoded: MemoryLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(layer, decoded);
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
}
