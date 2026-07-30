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
}

export type AppMode = 'explorer' | 'operator';
export type ActiveView = 'memory' | 'graph' | 'timeline' | 'context' | 'settings' | 'projects';

export interface FileEntry {
  name: string;
  path: string;
  isDir: boolean;
  sizeBytes: number;
  mimeType: string;
  children?: FileEntry[];
}
