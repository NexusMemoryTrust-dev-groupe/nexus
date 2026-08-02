import * as THREE from 'three';

/**
 * Level-of-detail and culling decisions for the cosmic graph.
 *
 * Why this exists
 * ---------------
 * Every node used to be drawn every frame with `frustumCulled={false}`, at full
 * geometry, with a DOM label attached. That is fine for the fifty-odd entities a
 * demo database holds and falls apart well before the thousands a real workspace
 * accumulates: the label layer alone forces a reflow per node per frame.
 *
 * The functions here are deliberately pure so the decisions can be tested
 * without a WebGL context. The renderer asks "should I draw this, and how
 * cheaply?" and gets an answer that depends only on numbers.
 */

// ── Tiers ───────────────────────────────────────────────────────────────────

/**
 * How much detail an item gets. Ordered from most to least expensive so a
 * comparison like `tier <= LodTier.Reduced` reads naturally.
 */
export enum LodTier {
  /** Close to the camera: full geometry, label, glow, pulse rings. */
  Full = 0,
  /** Mid-distance: slightly smaller, label if the budget allows, no decoration. */
  Reduced = 1,
  /** Far: a point of light. No label, no decoration. */
  Minimal = 2,
  /** Outside the frustum or beyond the draw distance: not drawn at all. */
  Culled = 3,
}

/** Distance thresholds, in world units, that separate the tiers. */
export interface LodThresholds {
  /** At or beyond this distance an item drops out of {@link LodTier.Full}. */
  full: number;
  /** At or beyond this distance an item drops to {@link LodTier.Minimal}. */
  reduced: number;
  /** At or beyond this distance an item is {@link LodTier.Culled}. */
  draw: number;
}

/**
 * Defaults chosen against the actual camera setup: the graph sits within roughly
 * 200 units of the origin and the camera starts at ~120 units out, so `full`
 * covers the cluster the user is looking at, `reduced` the surrounding ones, and
 * `draw` is generous enough that nothing pops in while orbiting.
 */
export const DEFAULT_THRESHOLDS: LodThresholds = {
  full: 90,
  reduced: 220,
  draw: 900,
};

/**
 * Maximum number of DOM labels rendered at once.
 *
 * This is the single most important budget in the view. Labels are real DOM
 * nodes positioned from a per-frame projection; past a hundred or so the layout
 * cost dominates everything else the frame does, and they overlap into an
 * unreadable smear long before that. Capping at the nearest N keeps the ones a
 * user can actually read.
 */
export const MAX_LABELS = 64;

/**
 * Maximum number of edges drawn.
 *
 * Edges are one draw call regardless of count, but each one costs a position
 * update per frame on the CPU. Dense graphs also turn into visual noise, so the
 * strongest connections are kept and the rest dropped.
 */
export const MAX_EDGES = 1500;

// ── Distance classification ─────────────────────────────────────────────────

/**
 * Classify a squared distance into a tier.
 *
 * Takes the *squared* distance because that is what `Vector3.distanceToSquared`
 * returns, and skipping the square root matters when this runs for every node on
 * every frame.
 *
 * Boundaries belong to the coarser tier (`>=`, not `>`). The direction is
 * arbitrary but must be fixed and tested: a node parked exactly on a threshold
 * would otherwise flip tiers on floating-point noise and visibly pop.
 */
export function tierForDistanceSq(
  distanceSq: number,
  thresholds: LodThresholds = DEFAULT_THRESHOLDS,
): LodTier {
  if (distanceSq >= thresholds.draw * thresholds.draw) return LodTier.Culled;
  if (distanceSq >= thresholds.reduced * thresholds.reduced) return LodTier.Minimal;
  if (distanceSq >= thresholds.full * thresholds.full) return LodTier.Reduced;
  return LodTier.Full;
}

/** Convenience wrapper for callers that already have a plain distance. */
export function tierForDistance(
  distance: number,
  thresholds: LodThresholds = DEFAULT_THRESHOLDS,
): LodTier {
  return tierForDistanceSq(distance * distance, thresholds);
}

/**
 * Scale multiplier applied to a node's radius at each tier.
 *
 * Decreases monotonically, and never reaches zero for a tier that is still
 * drawn: a zero scale makes a node vanish without being culled, which reads as a
 * rendering bug rather than as distance falloff.
 */
export function tierScale(tier: LodTier): number {
  switch (tier) {
    case LodTier.Full:
      return 1;
    case LodTier.Reduced:
      return 0.85;
    case LodTier.Minimal:
      return 0.6;
    case LodTier.Culled:
      return 0;
  }
}

/** Whether a tier is allowed to carry a DOM label at all. */
export function tierAllowsLabel(tier: LodTier): boolean {
  return tier === LodTier.Full || tier === LodTier.Reduced;
}

/** Whether a tier gets decorative extras: glow shells, pulse rings, orbit paths. */
export function tierAllowsDecoration(tier: LodTier): boolean {
  return tier === LodTier.Full;
}

// ── Label budget ────────────────────────────────────────────────────────────

/** Minimal shape the label selector needs. */
export interface LabelCandidate {
  id: string;
  /** Squared distance from the camera. */
  distanceSq: number;
  /** Cluster cores carry the names a user navigates by. */
  isCore: boolean;
  /** False when frustum-culled or beyond the draw distance. */
  visible: boolean;
  /** Pinned candidates bypass the budget: selection, hover, search hits. */
  pinned?: boolean;
}

/**
 * How much of a distance advantage a cluster core gets over a satellite.
 *
 * Cores are the labels a user navigates by, so a core competes as if it were
 * half the distance away (0.25 on squared distances). A plain tiebreak would
 * only help at *exactly* equal distances, which never happens in practice.
 */
const CORE_DISTANCE_BONUS = 0.25;

/**
 * Choose which items get a label.
 *
 * Order of operations matters: invisible candidates are discarded *before* the
 * budget is applied. Truncating first would let off-screen nodes consume slots
 * and leave visible ones unlabelled — which is exactly the bug the regression
 * test below pins.
 *
 * Returns a `Set` because the caller tests membership per item while rendering.
 */
export function selectLabels(
  candidates: LabelCandidate[],
  maxLabels: number = MAX_LABELS,
): Set<string> {
  const chosen = new Set<string>();
  if (maxLabels <= 0) return chosen;

  const eligible: LabelCandidate[] = [];
  for (const c of candidates) {
    if (!c.visible) continue;
    if (c.pinned) {
      // Pinned items bypass the budget entirely: hiding the node the user just
      // selected would be worse than one label over the cap.
      chosen.add(c.id);
      continue;
    }
    eligible.push(c);
  }

  const remaining = maxLabels - chosen.size;
  if (remaining <= 0) return chosen;

  // Copied before sorting: the caller reuses its candidate buffer across
  // frames, and sorting in place would scramble the render order it depends on.
  const ranked = [...eligible].sort(
    (a, b) => effectiveDistance(a) - effectiveDistance(b),
  );

  for (let i = 0; i < Math.min(remaining, ranked.length); i++) {
    chosen.add(ranked[i].id);
  }
  return chosen;
}

function effectiveDistance(c: LabelCandidate): number {
  return c.isCore ? c.distanceSq * CORE_DISTANCE_BONUS : c.distanceSq;
}

// ── Frustum culling ─────────────────────────────────────────────────────────

/**
 * Reusable frustum, matrix and sphere, module-level to avoid allocating per
 * frame.
 *
 * Safe because the render loop is single-threaded and each `updateFrustum` call
 * fully overwrites the previous state before it is read.
 */
const _frustum = new THREE.Frustum();
const _viewProjection = new THREE.Matrix4();
const _sphere = new THREE.Sphere();

/** Recompute the cached frustum from a camera. Call once per frame. */
export function updateFrustum(camera: THREE.Camera): THREE.Frustum {
  _viewProjection.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
  _frustum.setFromProjectionMatrix(_viewProjection);
  return _frustum;
}

/**
 * Whether a sphere is at least partly inside the cached frustum.
 *
 * Tests the sphere rather than the centre point on purpose: culling on the
 * centre alone makes large cluster cores blink out while their body still
 * visibly overlaps the screen edge. The caller pads `radius` for glow shells.
 */
export function isSphereVisible(
  center: THREE.Vector3,
  radius: number,
  frustum: THREE.Frustum = _frustum,
): boolean {
  _sphere.center.copy(center);
  _sphere.radius = radius;
  return frustum.intersectsSphere(_sphere);
}

// ── Edge budget ─────────────────────────────────────────────────────────────

/** Minimal shape the edge selector needs. */
export interface EdgeCandidate {
  id: string;
  weight: number;
}

/**
 * Trim edges to the budget, keeping the strongest.
 *
 * Returns the input untouched when it already fits, so the common case costs
 * nothing. Above the budget, weight is the right discriminator: a 0.9-weight
 * relationship carries more meaning than a 0.1 one, and drawing every faint link
 * in a dense graph produces a grey haze that hides the structure.
 */
export function selectEdges<T extends EdgeCandidate>(
  edges: T[],
  maxEdges: number = MAX_EDGES,
): T[] {
  if (edges.length <= maxEdges) return edges;
  return [...edges].sort((a, b) => b.weight - a.weight).slice(0, maxEdges);
}

// ── Adaptive quality ────────────────────────────────────────────────────────

/**
 * Scale the thresholds to the size of the graph.
 *
 * A fifty-node graph should look exactly as rich as it does today; a five
 * thousand node graph cannot afford the same generosity. Shrinking the `full`
 * and `reduced` radii as the node count grows keeps the number of
 * fully-detailed items roughly constant instead of letting it grow linearly.
 */
export function thresholdsForNodeCount(nodeCount: number): LodThresholds {
  if (nodeCount <= 150) return DEFAULT_THRESHOLDS;

  // Falls off as 1/sqrt(n): quadrupling the nodes halves the detail radius,
  // which keeps the *area* of full detail — and so the item count — stable.
  const factor = Math.sqrt(150 / nodeCount);

  // Floors are ordered (25 < 60 < draw) so the tiers can never invert, which
  // would classify everything as culled and blank the scene on the largest
  // graphs — the exact case this function exists to rescue.
  return {
    full: Math.max(25, DEFAULT_THRESHOLDS.full * factor),
    reduced: Math.max(60, DEFAULT_THRESHOLDS.reduced * factor),
    draw: DEFAULT_THRESHOLDS.draw,
  };
}

/** Label budget for a given graph size. Never drops below a readable minimum. */
export function labelBudgetForNodeCount(nodeCount: number): number {
  if (nodeCount <= MAX_LABELS) return nodeCount;
  if (nodeCount <= 500) return MAX_LABELS;
  // Very large graphs: fewer labels, because at that density they overlap into
  // noise anyway and the DOM cost is the dominant frame expense.
  return 40;
}
