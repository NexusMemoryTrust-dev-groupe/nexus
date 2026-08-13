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
  // Cognitive layer provenance (V18)
  layerConfidence: number;
  layerReason: string;
  layerUpdatedAt: string | null;
  layerHistory: LayerHistoryEntry[];
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

/** One entry of the layer provenance trail. Newest first. */
export interface LayerHistoryEntry {
  layer: string;
  confidence: number;
  reason: string;
  /** ISO timestamp of the (re)assignment. */
  at: string;
  /** Who assigned it: 'user' | 'classifier' | 'migration' | 'unknown'. */
  by: string;
}

/** Aggregated layer statistics, from the `get_layer_stats` command. */
export interface LayerStat {
  layer: string;
  count: number;
  meanConfidence: number;
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
export type ActiveView = 'memory' | 'graph' | 'timeline' | 'context' | 'settings' | 'projects' | 'savings' | 'radar' | 'team' | 'audit' | 'conflict' | 'flight' | 'rehearsal' | 'firewall' | 'contextlab' | 'passport' | 'skills' | 'predictive' | 'knowledge' | 'diagnostics';

// ── Flight Recorder (operation black-box, System 5) ────────────────────────
//
// Mirrors `core::flight::flight_recorder` on the Rust side. The black box
// logs every significant operation of the ecosystem: memory creation,
// conflicts, quarantine, rehearsal, skill and MCP calls — replayable per
// entity and summarised into stats.

export interface FlightSession {
  id: string;
  title: string;
  purpose: string;
  actor: string;
  source: string;
  /** active | closed */
  status: string;
  startedAt: string;
  endedAt: string | null;
}

export interface FlightRecord {
  id: string;
  sessionId: string | null;
  recordedAt: string;
  actor: string;
  /** memory | conflict | firewall | rehearsal | radar | skill | context | team | versioning | mcp | system */
  category: string;
  action: string;
  entityType: string;
  entityId: string;
  summary: string;
  details: Record<string, unknown>;
  durationMs: number;
  /** success | error | blocked | skipped */
  outcome: string;
}

export interface FlightStats {
  totalRecords: number;
  totalSessions: number;
  activeSessions: number;
  byCategory: Record<string, number>;
  byOutcome: Record<string, number>;
  /** System 5: recorded context chains (why-chains). */
  contextChains: number;
}

/** One recorded "why did the AI say this" chain (System 5). */
export interface ContextChain {
  id: string;
  sessionId: string | null;
  actor: string;
  query: string;
  intent: string;
  answerConfidence: number;
  answer: string;
  totalTokens: number;
  createdAt: string;
  /** Rendered "why" breakdown with ASCII bars. */
  why: string;
  /** Rendered pipeline stages chronology. */
  pipeline: string;
}

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

// ── Memory Conflict Engine (Система 2) ──────────────────────────────────────
//
// Mirrors `core::memory::conflict` on the Rust side. A conflict group ties the
// two sides of a semantic contradiction together; the Current Truth Engine
// computes which side wins right now, and `resolveConflict` settles it
// (winner → Current, losers → Superseded).

export interface ConflictResolution {
  winnerId: string;
  /** Normalized 0.0–1.0. */
  confidence: number;
  /** Human-readable reasons, e.g. "+ recent source". */
  reasons: string[];
  /** Who decided: 'user' | 'engine'. */
  by: string;
  /** ISO timestamp of the resolution. */
  at: string;
}

export interface ConflictGroup {
  id: string;
  topic: string;
  memberIds: string[];
  /** ISO timestamp when the contradiction was detected. */
  detectedAt: string;
  /** 'open' | 'resolved'. */
  status: string;
  resolvedAt: string | null;
  resolution: ConflictResolution | null;
}

/** The Current Truth Engine's read-only verdict over one conflict. */
export interface TruthVerdict {
  winnerId: string;
  confidence: number;
  reasons: string[];
}

// ── Memory Rehearsal & Canonical Consolidation (Система 3) ─────────────────
//
// Mirrors `commands::rehearsal` DTOs. Rehearsal strengthens memories that are
// due for review; canonical consolidation collapses repeated records into one
// canonical fact, keeping full provenance.

export interface RehearsalItem {
  id: string;
  title: string;
  summary: string;
  importance: number;
  confidence: number;
  rehearsalCount: number;
  lastRehearsedAt: string | null;
  dueAt: string;
  overdueDays: number;
}

export interface RehearsalCounts {
  total: number;
  dueNow: number;
  rehearsedAtLeastOnce: number;
  neverRehearsed: number;
  scheduled: number;
}

export interface RehearsalPlan {
  generatedAt: string;
  counts: RehearsalCounts;
  items: RehearsalItem[];
}

export interface RehearsalCycleReport {
  ranAt: string;
  rehearsed: number;
  scheduledFirst: number;
  decayed: number;
  skipped: number;
  total: number;
}

export interface CanonicalMemory {
  id: string;
  title: string;
  summary: string;
  memberIds: string[];
  memberCount: number;
  cohesion: number;
  importanceScore: number;
  confidenceScore: number;
  layer: string;
  createdAt: string;
}

export interface ConsolidationReport {
  ranAt: string;
  clustersFound: number;
  canonicalCreated: number;
  mergedMembers: number;
  skippedExisting: number;
  totalCanonical: number;
}

// ── Memory Firewall & Agent Policies (Система 4) ───────────────────────────

export interface FirewallScores {
  toxicity: number;
  spam: number;
  injection: number;
  pii: number;
}

export interface FirewallRule {
  id: string;
  pattern: string;
  /** block | quarantine */
  action: string;
  enabled: boolean;
  reason: string;
  createdAt: string;
}

export interface FirewallAssessment {
  verdict: string;
  toxicity: number;
  spam: number;
  injection: number;
  pii: number;
  reasons: string[];
  matchedRuleIds: string[];
}

export interface QuarantineEntry {
  id: string;
  title: string;
  content: string;
  author: string;
  source: string;
  reasons: string[];
  scores: FirewallScores;
  /** pending | approved | rejected */
  status: string;
  createdAt: string;
  decidedAt: string | null;
}

export interface AgentPolicy {
  id: string;
  agent: string;
  role: string;
  allowedVisibility: string[];
  allowedLayers: string[];
  denyPatterns: string[];
  enabled: boolean;
  createdAt: string;
}

export interface AgentAccessAssessment {
  verdict: string;
  reasons: string[];
  categories: string[];
  sensitivity: string;
}

// ── Context Lab (Система 6) ────────────────────────────────────────────────
//
// One query is assembled by several strategies (compact / balanced / rich);
// each takes metrics — how much memory and how many entities fit, tokens,
// layer maturity, predicted answer accuracy. The winner and full history are
// persisted in `context_lab_runs`.

export interface LabResult {
  strategy: string;
  memories: number;
  entities: number;
  tokens: number;
  baselineTokens: number;
  avgRelevance: number;
  maturity: number;
  accuracy: number;
  efficiencyPerKToken: number;
  buildMs: number;
}

export interface LabExperiment {
  query: string;
  createdAt: string;
  results: LabResult[];
  bestStrategy: string;
  summary: string;
}

// ── Agent Passport (Система 7) ─────────────────────────────────────────────

export interface AgentPassport {
  name: string;
  displayName: string;
  role: string;
  description: string;
  skills: string[];
  tools: string[];
  constraints: string[];
  trustLevel: number;
  memoryScope: string;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
}

// ── Skills & Skill Genesis (Система 8) ─────────────────────────────────────

export interface Skill {
  id: string;
  name: string;
  description: string;
  command: string;
  /** snake_case: mirrors `core::knowledge::skills::Skill` (no rename_all). */
  script_path: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface SkillOutput {
  success: boolean;
  stdout: string;
  stderr: string;
  exit_code: number | null;
  duration_ms: number;
  timed_out: boolean;
}

export interface SkillProposal {
  id: string;
  category: string;
  action: string;
  occurrences: number;
  name: string;
  description: string;
  /** proposed | approved | rejected */
  status: string;
  createdAt: string;
}

// ── Predictive Context (Система 9) ─────────────────────────────────────────

export interface Prediction {
  suggestedQuery: string;
  confidence: number;
  intentType: string;
  entities: string[];
  matches: number;
}

export interface PredictiveResponse {
  query: string;
  predictions: Prediction[];
  /** snake_case: mirrors the raw `serde_json::json!` envelope from predictive_predict. */
  prewarm_entities: string[];
  history_size: number;
}

// ── Knowledge Map (Система 10) ─────────────────────────────────────────────

export interface MapItem {
  ring: string;
  kind: string;
  id: string;
  title: string;
  layer: string;
  weight: number;
  owner: string;
}

export interface KnowledgeMap {
  entityId: string;
  entityTitle: string;
  mission: MapItem[];
  relevant: MapItem[];
  supporting: MapItem[];
  historical: MapItem[];
  total: number;
  rendered: string;
}

// ── Diagnostics (Production Readiness 0.5) ─────────────────────────────────
//
// Mirrors `commands::diagnostics` on the Rust side: the same health battery
// `nexus doctor` runs, surfaced in the UI. PII-free by design — check names,
// statuses and aggregate counts only.

export interface DiagnosticCheck {
  name: string;
  /** "ok" | "warning" | "error" */
  status: string;
  message: string;
}

export interface DiagnosticsReport {
  runAt: string;
  healthy: boolean;
  checks: DiagnosticCheck[];
}

export interface DiagnosticsExport {
  content: string;
  filename: string;
}
