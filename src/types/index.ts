export interface Memory {
  id: string;
  title: string;
  summary: string;
  content: string;
  createdAt: string;
  updatedAt: string;
  author: string;
  source: string;
  confidenceScore: number;
  importanceScore: number;
  visibility: string;
  captureMode: string;
  projectSpaceId: string | null;
  linkedEntityIds: string[];
  latestVersionId: string | null;
  status: string;
  layer: string;
  attachedFiles: AttachedFile[];
}

export interface AttachedFile {
  name: string;
  path: string;
  sizeBytes: number;
  mimeType: string;
}

export interface GraphNode {
  id: string;
  entityType: string;
  title: string;
  description: string;
  status: string;
  createdAt: string;
  updatedAt: string;
}

export interface GraphEdge {
  id: string;
  sourceEntityId: string;
  targetEntityId: string;
  relationshipType: string;
  weight: number;
  createdAt: string;
}

export interface ContextPackage {
  query: string;
  intentType: string;
  confidence: number;
  entities: GraphNode[];
  memoryRecords: Memory[];
  relationships: GraphEdge[];
  tokenCount: number;
  /// Per-item explanation of why it is present. Empty when the backend
  /// predates provenance, so the panel degrades to hidden rather than broken.
  provenance: ContextTrace[];
}

export type AppMode = 'explorer' | 'operator';
export type ActiveView = 'memory' | 'graph' | 'timeline' | 'context' | 'settings' | 'projects' | 'savings';

export interface FileEntry {
  name: string;
  path: string;
  isDir: boolean;
  sizeBytes: number;
  mimeType: string;
  children?: FileEntry[];
}

// ── Context provenance ──────────────────────────────────────────────────────
//
// Mirrors `core::context::provenance` on the Rust side. The shapes are tagged
// unions so a new reason added in the backend surfaces here as a type error
// rather than as a silently blank row in the panel.

export type ContextReason =
  | { kind: 'queryMatch'; query: string }
  | { kind: 'keywordMatch'; keyword: string }
  | { kind: 'graphExpansion'; fromId: string; fromTitle: string; hops: number }
  | { kind: 'memorySearch'; query: string }
  | { kind: 'recentActivity'; ageDays: number }
  | { kind: 'highImportance'; importance: number };

export type ContextDropCause =
  | { kind: 'belowRelevance'; score: number; floor: number }
  | { kind: 'tokenBudget'; limit: number }
  | { kind: 'entityCap'; cap: number };

export interface ContextScorePart {
  component: string;
  points: number;
}

export interface ContextTrace {
  id: string;
  kind: 'entity' | 'memory';
  title: string;
  reasons: ContextReason[];
  score: number | null;
  scoreParts: ContextScorePart[];
  tokens: number;
  included: boolean;
  dropped: ContextDropCause | null;
}

export interface ContextProvenance {
  traces: ContextTrace[];
}

export interface ContextExport {
  content: string;
  format: 'markdown' | 'json' | 'plain';
  tokens: number;
  tokenMethod: string;
  filename: string;
}
