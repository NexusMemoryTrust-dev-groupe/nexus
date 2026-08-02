import * as THREE from 'three';
import type { GraphNodeData, GraphEdgeData, OrbitCluster } from './types';
import { getColor, ORBIT_SPEED_MULT, ORBIT_SPREAD } from './constants';
import { formatNodeName } from './utils';

/** Raw entity shape as it arrives from the graph store. */
interface RawNode {
  id: string;
  entityType: string;
  title: string;
  description: string;
}

/** Raw relationship shape as it arrives from the graph store. */
interface RawEdge {
  id: string;
  sourceEntityId: string;
  targetEntityId: string;
  relationshipType: string;
  weight: number;
}

export interface ClusterResult {
  clusters: OrbitCluster[];
  allNodes: GraphNodeData[];
  uniqueEdges: GraphEdgeData[];
  satellites: GraphNodeData[];
}

/**
 * Even distribution on a golden-angle spiral.
 *
 * A grid would betray the metaphor (knowledge is not tabular) and concentric
 * rings visibly repeat; the golden angle never aligns, so clusters read as an
 * organic field however many there are.
 */
function generateCenter(index: number, totalNodes: number): THREE.Vector3 {
  const spacing = Math.max(30, Math.sqrt(totalNodes) * 8);
  const goldenAngle = Math.PI * (3 - Math.sqrt(5));
  const angle = index * goldenAngle;
  const radius = spacing * Math.sqrt(index + 1) * 0.4;
  return new THREE.Vector3(
    Math.cos(angle) * radius,
    Math.sin(index * 1.7) * spacing * 0.15,
    Math.sin(angle) * radius,
  );
}

/**
 * Group the graph into orbital clusters: well-connected entities become cores,
 * their neighbours become satellites, and anything left over becomes its own
 * core so nothing is hidden from the user.
 *
 * Adjacency is built once into a `Map` and neighbour lookups go through an
 * index rather than `Array.find`, because the original did a linear scan per
 * neighbour — quadratic in node count, which is what made large graphs stall
 * before a single frame was drawn.
 */
export function buildClusters(nodes: RawNode[], edges: RawEdge[]): ClusterResult {
  // Collapse duplicate relationships. Two entities linked twice would otherwise
  // draw overlapping lines and count double towards importance.
  const seenEdges = new Set<string>();
  const uniqueEdges: GraphEdgeData[] = [];
  for (const e of edges) {
    const key = `${e.sourceEntityId}->${e.targetEntityId}`;
    if (seenEdges.has(key)) continue;
    seenEdges.add(key);
    uniqueEdges.push({
      id: e.id,
      source: e.sourceEntityId,
      target: e.targetEntityId,
      relationshipType: e.relationshipType,
      weight: e.weight,
    });
  }

  // Index by id once: neighbour resolution below is O(1) instead of O(n).
  const byId = new Map<string, RawNode>();
  for (const n of nodes) byId.set(n.id, n);

  const adj = new Map<string, Set<string>>();
  for (const n of nodes) adj.set(n.id, new Set());
  for (const e of uniqueEdges) {
    adj.get(e.source)?.add(e.target);
    adj.get(e.target)?.add(e.source);
  }

  // Importance is degree: how many other things reference this one.
  const importance = new Map<string, number>();
  for (const n of nodes) importance.set(n.id, adj.get(n.id)?.size ?? 0);

  // Descending, so the most connected entity claims its neighbours first and
  // becomes a visual anchor rather than someone else's satellite.
  const sorted = [...nodes].sort(
    (a, b) => (importance.get(b.id) ?? 0) - (importance.get(a.id) ?? 0),
  );

  const assigned = new Set<string>();
  const clusters: OrbitCluster[] = [];
  const maxCores = Math.min(nodes.length, Math.ceil(nodes.length / 2));

  for (const core of sorted) {
    if (clusters.length >= maxCores) break;
    if (assigned.has(core.id)) continue;

    const neighbours = adj.get(core.id) ?? new Set<string>();
    const center = generateCenter(clusters.length, nodes.length);
    const sats: GraphNodeData[] = [];
    let si = 0;

    for (const nid of neighbours) {
      if (assigned.has(nid)) continue;
      const sn = byId.get(nid);
      if (!sn) continue;
      sats.push({
        id: sn.id,
        title: formatNodeName(sn.title, sn.entityType),
        entityType: sn.entityType,
        color: getColor(sn.entityType),
        position: center.clone(),
        orbitRadius: 4 + si * ORBIT_SPREAD,
        // Outer satellites orbit slower, which reads as depth rather than
        // everything spinning in lockstep.
        orbitSpeed: ORBIT_SPEED_MULT * (1 - si * 0.04),
        orbitOffset: (si * Math.PI * 2) / Math.max(neighbours.size, 1),
        coreId: core.id,
        size: 0.8 + (importance.get(sn.id) ?? 0) * 0.15,
        connectionCount: importance.get(sn.id) ?? 0,
        description: sn.description,
      });
      assigned.add(nid);
      si++;
    }

    clusters.push({
      coreId: core.id,
      coreTitle: formatNodeName(core.title, core.entityType),
      coreColor: getColor(core.entityType),
      satellites: sats,
      center,
    });
    assigned.add(core.id);
  }

  // Unconnected entities still deserve a place: each becomes its own core on
  // the spiral. Dropping them would quietly lose data from the view.
  for (const n of nodes) {
    if (assigned.has(n.id)) continue;
    clusters.push({
      coreId: n.id,
      coreTitle: formatNodeName(n.title, n.entityType),
      coreColor: getColor(n.entityType),
      satellites: [],
      center: generateCenter(clusters.length, nodes.length),
    });
    assigned.add(n.id);
  }

  const allNodes: GraphNodeData[] = [];
  const satellites: GraphNodeData[] = [];
  for (const c of clusters) {
    const cn = byId.get(c.coreId);
    if (cn) {
      allNodes.push({
        id: cn.id,
        title: formatNodeName(cn.title, cn.entityType),
        entityType: cn.entityType,
        color: getColor(cn.entityType),
        position: c.center.clone(),
        orbitRadius: 0,
        orbitSpeed: 0,
        orbitOffset: 0,
        coreId: null,
        size: 1.5 + (importance.get(cn.id) ?? 0) * 0.2,
        connectionCount: importance.get(cn.id) ?? 0,
        description: cn.description,
      });
    }
    satellites.push(...c.satellites);
    allNodes.push(...c.satellites);
  }

  return { clusters, allNodes, uniqueEdges, satellites };
}
