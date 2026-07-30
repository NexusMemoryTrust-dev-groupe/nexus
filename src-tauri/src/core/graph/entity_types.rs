use serde::{Deserialize, Serialize};

/// Closed list of entity types in the knowledge graph.
/// Each represents a fundamental concept in the user's world model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntityType {
    Person,
    Organization,
    Project,
    Document,
    Meeting,
    Decision,
    Task,
    Technology,
    Incident,
    Repository,
    Service,
    Model,
    Conversation,
    Memory,
    Custom(String),
}

impl EntityType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Person => "Person",
            Self::Organization => "Organization",
            Self::Project => "Project",
            Self::Document => "Document",
            Self::Meeting => "Meeting",
            Self::Decision => "Decision",
            Self::Task => "Task",
            Self::Technology => "Technology",
            Self::Incident => "Incident",
            Self::Repository => "Repository",
            Self::Service => "Service",
            Self::Model => "Model",
            Self::Conversation => "Conversation",
            Self::Memory => "Memory",
            Self::Custom(s) => s,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Person" => Self::Person,
            "Organization" => Self::Organization,
            "Project" => Self::Project,
            "Document" => Self::Document,
            "Meeting" => Self::Meeting,
            "Decision" => Self::Decision,
            "Task" => Self::Task,
            "Technology" => Self::Technology,
            "Incident" => Self::Incident,
            "Repository" => Self::Repository,
            "Service" => Self::Service,
            "Model" => Self::Model,
            "Conversation" => Self::Conversation,
            "Memory" => Self::Memory,
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
            EntityType::Person,
            EntityType::Organization,
            EntityType::Project,
            EntityType::Document,
            EntityType::Meeting,
            EntityType::Decision,
            EntityType::Task,
            EntityType::Technology,
            EntityType::Incident,
            EntityType::Repository,
            EntityType::Service,
            EntityType::Model,
            EntityType::Conversation,
            EntityType::Memory,
        ];
        for t in &types {
            let s = t.as_str();
            assert!(!s.is_empty());
            assert_eq!(&EntityType::from_str(s), t);
        }
    }

    #[test]
    fn custom_type_roundtrip() {
        let ct = EntityType::Custom("MyType".to_string());
        assert_eq!(ct.as_str(), "MyType");
        assert_eq!(EntityType::from_str("MyType"), ct);
    }

    #[test]
    fn unknown_string_becomes_custom() {
        let result = EntityType::from_str("UnknownThing");
        assert_eq!(result, EntityType::Custom("UnknownThing".to_string()));
    }

    #[test]
    fn serialization_roundtrip() {
        for t in [EntityType::Person, EntityType::Custom("X".to_string())] {
            let json = serde_json::to_string(&t).unwrap();
            let decoded: EntityType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, decoded);
        }
    }
}
