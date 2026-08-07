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
  // Memory Trust lifecycle (V12)
  memoryState: string;
  supersedesId: string | null;
  supersededById: string | null;
  confirmedAt: string | null;
  confirmedBy: string | null;
  expiresAt: string | null;
  feedback: MemoryFeedback;
}

export interface MemoryFeedback {
  useful: number;
  irrelevant: number;
  wrong: number;
  /** Active vote kind: 'useful' | 'irrelevant' | 'wrong' | null */
  voted?: string | null;
  /** User's explanation of why the verdict was given */
  note?: string | null;
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
export type ActiveView = 'memory' | 'graph' | 'timeline' | 'context' | 'settings' | 'projects' | 'savings' | 'radar' | 'team' | 'audit';

// ── Memory Radar ───────────────────────────────────────────────────────────
//
// Mirrors `core::memory::memory_radar` on the Rust side. The radar is the
// proactive recall layer: instead of waiting for a query it scans the pool and
// reports what needs attention right now.

export interface RadarCounts {
  total: number;
  newSinceLastScan: number;
  updatedSinceLastScan: number;
  conflicted: number;
  superseded: number;
  inferred: number;
  expiring: number;
  unconfirmed: number;
}

export interface RadarItem {
  id: string;
  title: string;
  summary: string;
  /** resolve | recheck | confirm | review */
  action: string;
  importance: number;
  confidence: number;
  memoryState: string;
  createdAt: string;
  updatedAt: string;
  expiresAt: string | null;
  reason: string;
}

export interface RadarSnapshot {
  generatedAt: string;
  /** Timestamp of the previous scan (null on first run). */
  since: string | null;
  counts: RadarCounts;
  items: RadarItem[];
  attentionScore: number;
}

// ── Team Memory (shared trusted layer) ─────────────────────────────────────
//
// Mirrors `core::team` on the Rust side. The trusted decision layer answers
// "who confirmed what, what went stale, what is in conflict" — the things a
// team cannot recover from chat history.

export interface TeamMember {
  id: string;
  name: string;
  /** admin | member | viewer */
  role: string;
  active: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface MemberActivity {
  member: TeamMember;
  authored: number;
  confirmed: number;
  updated: number;
}

export interface DecisionItem {
  memoryId: string;
  title: string;
  /** Who performed the decision (member name, when known). */
  by: string | null;
  /** When it happened (RFC3339, when known). */
  at: string | null;
  /** Extra context — e.g. which memory replaced this one. */
  detail: string | null;
}

export interface TeamTotals {
  members: number;
  active: number;
  confirmed: number;
  superseded: number;
  conflicted: number;
  authored: number;
}

export interface TeamOverview {
  members: MemberActivity[];
  confirmedDecisions: DecisionItem[];
  supersededDecisions: DecisionItem[];
  conflicted: DecisionItem[];
  totals: TeamTotals;
}

// ── Audit Memory (decision chain / compliance) ─────────────────────────────
//
// Mirrors `core::audit` on the Rust side. The answer to "why did we decide
// this?" — context, alternatives considered, who confirmed, what replaced it.
// The compliance door: prove the team knew and why it decided so.

export interface AuditEvent {
  id: string;
  memoryId: string;
  /** Created | Alternative | Confirmed | Superseded | Note */
  eventType: string;
  actor: string;
  detail: string | null;
  relatedMemoryId: string | null;
  createdAt: string;
}

export interface DecisionAlternative {
  title: string;
  reason: string;
}

export interface AuditVersion {
  version: number;
  changeType: string;
  by: string;
  at: string;
  reason: string | null;
}

export interface AuditTrail {
  memoryId: string;
  title: string;
  state: string;
  author: string;
  createdAt: string;
  updatedAt: string;
  reason: string | null;
  confirmedBy: string | null;
  confirmedAt: string | null;
  supersedes: string | null;
  supersededBy: string | null;
  alternatives: DecisionAlternative[];
  events: AuditEvent[];
  versions: AuditVersion[];
}

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
