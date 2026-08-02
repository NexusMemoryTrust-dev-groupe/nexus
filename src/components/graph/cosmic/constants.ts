import * as THREE from 'three';

/**
 * Palette, shared geometry and orbit tuning for the cosmic graph.
 *
 * Geometry and scratch objects are created once at module scope: allocating a
 * `Vector3` or a `SphereGeometry` inside a frame callback is the classic way to
 * turn a 60 fps scene into a garbage-collection sawtooth.
 */

export const entityColors: Record<string, string> = {
  project: '#ff8a5b',
  file: '#a99cf8',
  person: '#63d8d2',
  concept: '#ddbb65',
  task: '#f472b6',
  decision: '#818cf8',
  organization: '#f59e0b',
  meeting: '#34d399',
  document: '#c084fc',
  technology: '#60a5fa',
  memory: '#f472b6',
  default: '#93c5fd',
};

export function getColor(entityType: string): string {
  const lower = entityType.toLowerCase();
  for (const [key, val] of Object.entries(entityColors)) {
    if (lower.includes(key)) return val;
  }
  return entityColors.default;
}

export const EDGE_TYPE_COLORS: Record<string, string> = {
  uses: '#60a5fa',
  dependson: '#f472b6',
  relatedto: '#93c5fd',
  contains: '#ddbb65',
  createdby: '#63d8d2',
  references: '#a99cf8',
  default: '#4a5568',
};

export function getEdgeColor(relationshipType: string): string {
  const lower = relationshipType.toLowerCase().replace(/[^a-z]/g, '');
  return EDGE_TYPE_COLORS[lower] ?? EDGE_TYPE_COLORS.default;
}

// ── Scratch objects, reused every frame ──
export const _dummy = new THREE.Object3D();
export const _colorObj = new THREE.Color();
export const _projVec = new THREE.Vector3();
export const _lerpVec = new THREE.Vector3();

// ── Shared geometry, three levels of detail ──
export const sphereGeo = new THREE.SphereGeometry(1, 24, 24);
export const smallSphereGeo = new THREE.SphereGeometry(1, 12, 12);
export const tinySphereGeo = new THREE.SphereGeometry(1, 8, 8);

export const ORBIT_SPEED_MULT = 0.012;
export const ORBIT_SPREAD = 1.6;

/**
 * Level-of-detail thresholds, in world units from the camera.
 *
 * Rationale: a node 400 units away occupies a couple of pixels, so a 24×24
 * sphere and a DOM label are pure cost. Culling and simplifying by distance is
 * what keeps a graph of thousands of nodes interactive instead of turning the
 * whole scene into a slideshow.
 */
export const LOD = {
  /** Beyond this, draw nothing at all. */
  cullDistance: 900,
  /** Beyond this, use the 8-segment sphere. */
  tinyDistance: 420,
  /** Beyond this, use the 12-segment sphere. */
  smallDistance: 200,
  /** Beyond this, stop rendering DOM labels. */
  labelDistance: 260,
  /** Hard cap on simultaneously rendered DOM labels. */
  maxLabels: 60,
} as const;
