use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::memory::memory_record::MemoryRecord;
use crate::core::result::Result;

/// Compressed representation of multiple memory records.
/// Used for summarization and strategic-level memory layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedMemory {
    /// Human-readable summary of the compressed content.
    pub summary: String,
    /// Extracted key facts from the source records.
    pub key_facts: Vec<String>,
    /// When the compression was performed.
    pub compressed_at: DateTime<Utc>,
    /// Number of source records that were compressed.
    pub source_count: u32,
}

/// Service trait for memory compression.
/// Compresses multiple memory records into a summary with key facts.
/// Supports decompression for recovery.
#[async_trait]
pub trait MemoryCompressionService: Send + Sync {
    /// Compress a set of memory records into a compact representation.
    async fn compress(&self, records: &[MemoryRecord]) -> Result<CompressedMemory>;

    /// Decompress a compressed memory back into its constituent records.
    async fn decompress(&self, compressed: &CompressedMemory) -> Result<Vec<MemoryRecord>>;
}

/// Simple in-memory compression service.
/// Extracts titles and content as key facts, creates a basic summary.
pub struct SimpleCompressionService;

#[async_trait]
impl MemoryCompressionService for SimpleCompressionService {
    async fn compress(&self, records: &[MemoryRecord]) -> Result<CompressedMemory> {
        if records.is_empty() {
            return Err(crate::core::result::AppError::Validation(
                "Cannot compress empty records".to_string(),
            ));
        }

        let key_facts: Vec<String> = records
            .iter()
            .map(|r| format!("{}: {}", r.title, r.summary))
            .collect();

        let summary = format!(
            "Compressed summary of {} memory records",
            records.len()
        );

        Ok(CompressedMemory {
            summary,
            key_facts,
            compressed_at: Utc::now(),
            source_count: records.len() as u32,
        })
    }

    async fn decompress(&self, compressed: &CompressedMemory) -> Result<Vec<MemoryRecord>> {
        let mut records = Vec::new();
        for fact in &compressed.key_facts {
            // Parse "title: summary" format used by compress
            let (title, summary) = if let Some(idx) = fact.find(": ") {
                (fact[..idx].to_string(), fact[idx + 2..].to_string())
            } else {
                (fact.clone(), String::new())
            };
            let record = MemoryRecord::new(
                title,
                format!("Decompressed from summary: {}", summary),
                "compressed".to_string(),
                crate::core::memory::types::MemorySource::Compressed,
            )?;
            records.push(record);
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::types::MemorySource;

    fn sample_record(title: &str) -> MemoryRecord {
        MemoryRecord::new(
            title.to_string(),
            format!("Content for {}", title),
            "author".to_string(),
            MemorySource::Manual,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn compress_records() {
        let svc = SimpleCompressionService;
        let records = vec![
            sample_record("Record 1"),
            sample_record("Record 2"),
            sample_record("Record 3"),
        ];
        let result = svc.compress(&records).await.unwrap();
        assert_eq!(result.source_count, 3);
        assert_eq!(result.key_facts.len(), 3);
        assert!(!result.summary.is_empty());
    }

    #[tokio::test]
    async fn compress_empty_fails() {
        let svc = SimpleCompressionService;
        let result = svc.compress(&[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn decompress_roundtrip() {
        let svc = SimpleCompressionService;
        let records = vec![
            sample_record("Alpha"),
            sample_record("Beta"),
        ];
        let compressed = svc.compress(&records).await.unwrap();
        let decompressed = svc.decompress(&compressed).await.unwrap();
        assert_eq!(decompressed.len(), 2);
        assert_eq!(decompressed[0].title, "Alpha");
        assert_eq!(decompressed[1].title, "Beta");
    }

    #[tokio::test]
    async fn decompress_empty_key_facts() {
        let svc = SimpleCompressionService;
        let compressed = CompressedMemory {
            summary: "empty".to_string(),
            key_facts: vec![],
            compressed_at: Utc::now(),
            source_count: 0,
        };
        let result = svc.decompress(&compressed).await.unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn compressed_memory_serialization() {
        let cm = CompressedMemory {
            summary: "Summary".to_string(),
            key_facts: vec!["fact1".to_string(), "fact2".to_string()],
            compressed_at: Utc::now(),
            source_count: 5,
        };
        let json = serde_json::to_string(&cm).unwrap();
        let decoded: CompressedMemory = serde_json::from_str(&json).unwrap();
        assert_eq!(cm.summary, decoded.summary);
        assert_eq!(cm.key_facts.len(), decoded.key_facts.len());
        assert_eq!(cm.source_count, decoded.source_count);
    }
}
