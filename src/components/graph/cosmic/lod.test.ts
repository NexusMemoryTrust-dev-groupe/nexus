import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import {
  LodTier,
  DEFAULT_THRESHOLDS,
  MAX_LABELS,
  MAX_EDGES,
  tierForDistance,
  tierForDistanceSq,
  tierScale,
  tierAllowsLabel,
  tierAllowsDecoration,
  selectLabels,
  selectEdges,
  updateFrustum,
  isSphereVisible,
  thresholdsForNodeCount,
  labelBudgetForNodeCount,
} from './lod';

// ── Distance classification ─────────────────────────────────────────────────

describe('tier classification', () => {
  it('puts nearby nodes in the full-detail tier', () => {
    expect(tierForDistance(0)).toBe(LodTier.Full);
    expect(tierForDistance(DEFAULT_THRESHOLDS.full - 1)).toBe(LodTier.Full);
  });

  it('degrades monotonically as distance grows', () => {
    const tiers = [10, 100, 300, 2000].map((d) => tierForDistance(d));
    for (let i = 1; i < tiers.length; i++) {
      expect(tiers[i]).toBeGreaterThanOrEqual(tiers[i - 1]);
    }
  });

  it('culls beyond the draw distance', () => {
    expect(tierForDistance(DEFAULT_THRESHOLDS.draw + 1)).toBe(LodTier.Culled);
  });

  it('agrees between the squared and plain distance forms', () => {
    // The renderer uses the squared form to avoid a sqrt per node per frame;
    // if the two ever disagree, the hot path silently renders a different tier
    // from the one the tests cover.
    for (const d of [0, 50, 89, 91, 219, 221, 899, 901, 5000]) {
      expect(tierForDistanceSq(d * d)).toBe(tierForDistance(d));
    }
  });

  it('treats threshold boundaries as belonging to the coarser tier', () => {
    // Exactly at `full` must not claim full detail: the comparison is `<`, so a
    // node sitting on the boundary drops one tier. Pinned because an off-by-one
    // here produces a visible pop as the camera moves.
    expect(tierForDistance(DEFAULT_THRESHOLDS.full)).toBe(LodTier.Reduced);
    expect(tierForDistance(DEFAULT_THRESHOLDS.reduced)).toBe(LodTier.Minimal);
    expect(tierForDistance(DEFAULT_THRESHOLDS.draw)).toBe(LodTier.Culled);
  });
});

describe('tier capabilities', () => {
  it('scales geometry down as the tier coarsens', () => {
    expect(tierScale(LodTier.Full)).toBe(1);
    expect(tierScale(LodTier.Reduced)).toBeLessThan(tierScale(LodTier.Full));
    expect(tierScale(LodTier.Minimal)).toBeLessThan(tierScale(LodTier.Reduced));
  });

  it('never scales to zero for a visible tier', () => {
    // A zero scale would make a node vanish without being culled, which reads
    // as a rendering bug rather than as distance falloff.
    for (const tier of [LodTier.Full, LodTier.Reduced, LodTier.Minimal]) {
      expect(tierScale(tier)).toBeGreaterThan(0);
    }
  });

  it('allows labels only for the two closest tiers', () => {
    expect(tierAllowsLabel(LodTier.Full)).toBe(true);
    expect(tierAllowsLabel(LodTier.Reduced)).toBe(true);
    expect(tierAllowsLabel(LodTier.Minimal)).toBe(false);
    expect(tierAllowsLabel(LodTier.Culled)).toBe(false);
  });

  it('allows decoration only at full detail', () => {
    // Pulse rings and glow are the expensive extras; they are what makes a
    // thousand-node graph drop frames, so they stay nearest-only.
    expect(tierAllowsDecoration(LodTier.Full)).toBe(true);
    expect(tierAllowsDecoration(LodTier.Reduced)).toBe(false);
  });
});

// ── Label budget ────────────────────────────────────────────────────────────

describe('selectLabels', () => {
  const candidate = (id: string, distanceSq: number, isCore = false) => ({
    id,
    distanceSq,
    isCore,
    visible: true,
  });

  it('returns every candidate when under budget', () => {
    const picked = selectLabels([candidate('a', 1), candidate('b', 2)], 10);
    expect(picked.size).toBe(2);
  });

  it('never exceeds the budget', () => {
    const many = Array.from({ length: 500 }, (_, i) => candidate(`n${i}`, i));
    expect(selectLabels(many, 64).size).toBe(64);
  });

  it('keeps the nearest candidates when over budget', () => {
    const picked = selectLabels(
      [candidate('far', 1000), candidate('near', 1), candidate('mid', 100)],
      2,
    );
    expect(picked.has('near')).toBe(true);
    expect(picked.has('mid')).toBe(true);
    expect(picked.has('far')).toBe(false);
  });

  it('prefers cores over satellites at equal distance', () => {
    // Cluster cores carry the names a user navigates by; dropping a core label
    // while keeping an orbiting satellite makes the graph unreadable.
    const picked = selectLabels(
      [candidate('satellite', 50, false), candidate('core', 50, true)],
      1,
    );
    expect(picked.has('core')).toBe(true);
    expect(picked.has('satellite')).toBe(false);
  });

  it('excludes off-screen candidates entirely', () => {
    const picked = selectLabels(
      [{ id: 'behind', distanceSq: 1, isCore: true, visible: false }],
      10,
    );
    expect(picked.size).toBe(0);
  });

  it('does not spend budget on invisible nodes', () => {
    // Regression: an early version filtered *after* truncating, so off-screen
    // nodes consumed slots and visible ones lost their labels.
    const candidates = [
      { id: 'hidden1', distanceSq: 1, isCore: true, visible: false },
      { id: 'hidden2', distanceSq: 2, isCore: true, visible: false },
      { id: 'shown', distanceSq: 3, isCore: true, visible: true },
    ];
    const picked = selectLabels(candidates, 2);
    expect(picked.has('shown')).toBe(true);
  });

  it('handles an empty candidate list', () => {
    expect(selectLabels([], 10).size).toBe(0);
  });

  it('handles a zero budget', () => {
    expect(selectLabels([candidate('a', 1)], 0).size).toBe(0);
  });

  it('does not mutate the input array', () => {
    // The caller reuses its candidate buffer across frames; sorting in place
    // would scramble the render order it depends on.
    const input = [candidate('c', 3), candidate('a', 1), candidate('b', 2)];
    const order = input.map((c) => c.id);
    selectLabels(input, 2);
    expect(input.map((c) => c.id)).toEqual(order);
  });
});

// ── Edge budget ─────────────────────────────────────────────────────────────

describe('selectEdges', () => {
  it('returns every edge when under budget', () => {
    const edges = [
      { id: 'a', weight: 0.1 },
      { id: 'b', weight: 0.9 },
    ];
    expect(selectEdges(edges, 10)).toHaveLength(2);
  });

  it('keeps the heaviest edges when over budget', () => {
    const edges = [
      { id: 'weak', weight: 0.1 },
      { id: 'strong', weight: 0.9 },
      { id: 'mid', weight: 0.5 },
    ];
    const kept = selectEdges(edges, 2).map((e) => e.id);
    expect(kept).toContain('strong');
    expect(kept).toContain('mid');
    expect(kept).not.toContain('weak');
  });

  it('never exceeds the budget', () => {
    const edges = Array.from({ length: 5000 }, (_, i) => ({
      id: `e${i}`,
      weight: Math.random(),
    }));
    expect(selectEdges(edges, MAX_EDGES)).toHaveLength(MAX_EDGES);
  });

  it('does not mutate the input array', () => {
    const edges = [
      { id: 'a', weight: 0.1 },
      { id: 'b', weight: 0.9 },
    ];
    const order = edges.map((e) => e.id);
    selectEdges(edges, 1);
    expect(edges.map((e) => e.id)).toEqual(order);
  });

  it('handles an empty edge list', () => {
    expect(selectEdges([], 10)).toEqual([]);
  });
});

// ── Frustum culling ─────────────────────────────────────────────────────────

describe('frustum culling', () => {
  function cameraLookingDownNegativeZ(): THREE.PerspectiveCamera {
    const cam = new THREE.PerspectiveCamera(60, 1, 0.1, 1000);
    cam.position.set(0, 0, 0);
    cam.lookAt(0, 0, -1);
    cam.updateMatrixWorld(true);
    cam.updateProjectionMatrix();
    return cam;
  }

  it('sees a sphere directly ahead', () => {
    const cam = cameraLookingDownNegativeZ();
    updateFrustum(cam);
    expect(isSphereVisible(new THREE.Vector3(0, 0, -50), 2)).toBe(true);
  });

  it('rejects a sphere behind the camera', () => {
    const cam = cameraLookingDownNegativeZ();
    updateFrustum(cam);
    expect(isSphereVisible(new THREE.Vector3(0, 0, 50), 2)).toBe(false);
  });

  it('rejects a sphere far off to the side', () => {
    const cam = cameraLookingDownNegativeZ();
    updateFrustum(cam);
    expect(isSphereVisible(new THREE.Vector3(5000, 0, -50), 2)).toBe(false);
  });

  it('keeps a large sphere whose centre is outside but whose body intrudes', () => {
    // Radius matters: culling on the centre alone makes big cluster cores blink
    // out while still visibly overlapping the screen edge.
    const cam = cameraLookingDownNegativeZ();
    updateFrustum(cam);
    const justOutside = new THREE.Vector3(0, 0, 20);
    expect(isSphereVisible(justOutside, 0.1)).toBe(false);
    expect(isSphereVisible(justOutside, 200)).toBe(true);
  });
});

// ── Adaptive budgets ────────────────────────────────────────────────────────

describe('adaptive thresholds', () => {
  it('leaves small graphs at full detail radius', () => {
    expect(thresholdsForNodeCount(50)).toEqual(DEFAULT_THRESHOLDS);
    expect(thresholdsForNodeCount(150)).toEqual(DEFAULT_THRESHOLDS);
  });

  it('tightens the detail radius as the graph grows', () => {
    const small = thresholdsForNodeCount(150);
    const large = thresholdsForNodeCount(2000);
    expect(large.full).toBeLessThan(small.full);
    expect(large.reduced).toBeLessThan(small.reduced);
  });

  it('never collapses the thresholds to zero or inverts them', () => {
    // An inverted ordering would classify everything as culled and blank the
    // scene on the largest graphs — the exact case this is meant to rescue.
    for (const n of [200, 1000, 10_000, 100_000]) {
      const t = thresholdsForNodeCount(n);
      expect(t.full).toBeGreaterThan(0);
      expect(t.reduced).toBeGreaterThan(t.full);
      expect(t.draw).toBeGreaterThan(t.reduced);
    }
  });

  it('shrinks the label budget for dense graphs', () => {
    expect(labelBudgetForNodeCount(10)).toBe(10);
    expect(labelBudgetForNodeCount(400)).toBe(MAX_LABELS);
    expect(labelBudgetForNodeCount(5000)).toBeLessThan(MAX_LABELS);
  });

  it('always leaves at least one label', () => {
    expect(labelBudgetForNodeCount(1_000_000)).toBeGreaterThan(0);
  });
});
