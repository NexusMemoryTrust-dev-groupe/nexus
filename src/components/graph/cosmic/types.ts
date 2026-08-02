import * as THREE from 'three';

/**
 * Shared shapes for the cosmic graph view.
 *
 * Extracted from the original 81 KB single-file component: every sub-module
 * needs these, and re-declaring them locally is how the two halves of a
 * renderer drift apart.
 */

export interface GraphNodeData {
  id: string;
  title: string;
  entityType: string;
  color: string;
  position: THREE.Vector3;
  orbitRadius: number;
  orbitSpeed: number;
  orbitOffset: number;
  coreId: string | null;
  size: number;
  connectionCount: number;
  description?: string;
}

export interface GraphEdgeData {
  id: string;
  source: string;
  target: string;
  relationshipType: string;
  weight: number;
}

export interface OrbitCluster {
  coreId: string;
  coreTitle: string;
  coreColor: string;
  satellites: GraphNodeData[];
  center: THREE.Vector3;
}

export interface SearchSuggestion {
  id: string;
  title: string;
  type: string;
  color: string;
  kind: 'entity' | 'memory';
  description?: string;
  connectionCount?: number;
}

/**
 * Live screen-space projection of one node.
 *
 * `distanceSq` and `tier` are carried alongside the pixel coordinates because
 * the label layer needs them to decide *which* labels are worth drawing, and
 * recomputing a camera distance in the DOM layer would duplicate work the
 * projection pass has already done.
 */
export interface ScreenProjection {
  x: number;
  y: number;
  /** Inside the view frustum and in front of the camera. */
  visible: boolean;
  /** Squared distance from the camera, for level-of-detail decisions. */
  distanceSq: number;
  /** Level-of-detail tier, as computed by `cosmic/lod`. */
  tier: number;
}

/**
 * Live screen-space projection of every node, written by the projection
 * tracker inside the R3F frame loop and read by the DOM label layer.
 *
 * A module-level map rather than React state on purpose: this updates every
 * frame, and routing 60 Hz through setState would re-render the whole tree.
 */
export const screenPos = new Map<string, ScreenProjection>();
