-- V30: Add source_text to semantic fingerprint tables.
--
-- The original fingerprints stored only the embedding vector, so search was
-- pure cosine over vectors and could not use the lexical / path signal that
-- lives in the text itself (e.g. the file path inside a document title). This
-- migration keeps the original text next to its vector so the hybrid retriever
-- can combine cosine similarity with keyword / filename evidence.
--
-- ADD COLUMN is idempotent (duplicate-column errors are swallowed by the
-- migration runner).
ALTER TABLE memory_semantic_fingerprints ADD COLUMN source_text TEXT;
ALTER TABLE document_fingerprints ADD COLUMN source_text TEXT;
