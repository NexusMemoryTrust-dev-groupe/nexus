use serde::{Deserialize, Serialize};

/// Closed list of relationship types between entities.
/// Each represents a fundamental way two entities can be connected.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RelationshipType {
    Created,
    Modified,
    ParticipatedIn,
    DependsOn,
    CausedBy,
    RelatedTo,
    Owns,
    Uses,
    Mentions,
    DerivedFrom,
    BlockedBy,
    ReplacedBy,
    Custom(String),
}

impl RelationshipType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Created => "Created",
            Self::Modified => "Modified",
            Self::ParticipatedIn => "ParticipatedIn",
            Self::DependsOn => "DependsOn",
            Self::CausedBy => "CausedBy",
            Self::RelatedTo => "RelatedTo",
            Self::Owns => "Owns",
            Self::Uses => "Uses",
            Self::Mentions => "Mentions",
            Self::DerivedFrom => "DerivedFrom",
            Self::BlockedBy => "BlockedBy",
            Self::ReplacedBy => "ReplacedBy",
            Self::Custom(s) => s,
        }
    }
}

// Infallible conversion — unknown names become `Custom`, so this is `From`
// rather than `FromStr` (see the note on `EntityType`).
impl From<&str> for RelationshipType {
    fn from(s: &str) -> Self {
        match s {
            "Created" => Self::Created,
            "Modified" => Self::Modified,
            "ParticipatedIn" => Self::ParticipatedIn,
            "DependsOn" => Self::DependsOn,
            "CausedBy" => Self::CausedBy,
            "RelatedTo" => Self::RelatedTo,
            "Owns" => Self::Owns,
            "Uses" => Self::Uses,
            "Mentions" => Self::Mentions,
            "DerivedFrom" => Self::DerivedFrom,
            "BlockedBy" => Self::BlockedBy,
            "ReplacedBy" => Self::ReplacedBy,
            other => Self::Custom(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtin_types_as_str() {
        let types = [
            RelationshipType::Created,
            RelationshipType::Modified,
            RelationshipType::ParticipatedIn,
            RelationshipType::DependsOn,
            RelationshipType::CausedBy,
            RelationshipType::RelatedTo,
            RelationshipType::Owns,
            RelationshipType::Uses,
            RelationshipType::Mentions,
            RelationshipType::DerivedFrom,
            RelationshipType::BlockedBy,
            RelationshipType::ReplacedBy,
        ];
        for t in &types {
            let s = t.as_str();
            assert!(!s.is_empty());
            assert_eq!(&RelationshipType::from(s), t);
        }
    }

    #[test]
    fn custom_type_roundtrip() {
        let ct = RelationshipType::Custom("MyRel".to_string());
        assert_eq!(ct.as_str(), "MyRel");
        assert_eq!(RelationshipType::from("MyRel"), ct);
    }

    #[test]
    fn unknown_string_becomes_custom() {
        let result = RelationshipType::from("UnknownRel");
        assert_eq!(result, RelationshipType::Custom("UnknownRel".to_string()));
    }

    #[test]
    fn serialization_roundtrip() {
        for t in [
            RelationshipType::Created,
            RelationshipType::Custom("X".to_string()),
        ] {
            let json = serde_json::to_string(&t).unwrap();
            let decoded: RelationshipType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, decoded);
        }
    }
}
