import { useRef, useMemo, useState, useCallback, useEffect, memo } from 'react';
import { Canvas, useFrame, useThree } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import * as THREE from 'three';
import { useGraphStore } from '../../stores/graphStore';
import { useMemoryStore } from '../../stores/memoryStore';
import { useUiStore } from '../../stores/uiStore';
import { useLocale } from '../../stores/localeStore';
import { Network, Maximize2, Minimize2, Info } from 'lucide-react';

import {
  getColor, getEdgeColor,
  _dummy, _colorObj, _projVec, _lerpVec,
  sphereGeo, smallSphereGeo, tinySphereGeo,
} from './cosmic/constants';
import type { GraphNodeData, GraphEdgeData, OrbitCluster, SearchSuggestion } from './cosmic/types';
import { screenPos } from './cosmic/types';
import { getOrbitPos, resolveNodePosition } from './cosmic/utils';
import { buildClusters } from './cosmic/clusters';
import {
  LodTier, tierForDistanceSq,
  selectLabels, selectEdges, updateFrustum, isSphereVisible,
  thresholdsForNodeCount, labelBudgetForNodeCount,
} from './cosmic/lod';
import type { LabelCandidate } from './cosmic/lod';
import { Starfield, AmbientDust, PulseRings } from './cosmic/Decorations';
import { CosmicSun } from './cosmic/CosmicSun';
import { LabelLayer, ContextMenu, SearchBar, InfoPanel } from './cosmic/Panels';

// ═══════════════════════════════════════════════════════════════
// CORE NODE — IMPERATIVE position update for smooth drag
// ═══════════════════════════════════════════════════════════════
const CoreNode = memo(function CoreNode({
  node, onClick, onHover, onUnhover, onContextMenu, highlightedRef, highlightedIdsRef,
  corePositions, dragRef, filteredIds,
}: {
  node: GraphNodeData; onClick: () => void; onHover: () => void;
  onUnhover: () => void; onContextMenu: (e: MouseEvent) => void;
  highlightedRef: React.MutableRefObject<string | null>;
  highlightedIdsRef: React.MutableRefObject<Set<string>>;
  corePositions: React.MutableRefObject<Map<string, THREE.Vector3>>;
  dragRef: React.MutableRefObject<string | null>;
  filteredIds: Set<string> | null;
}) {
  const groupRef = useRef<THREE.Group>(null);
  const meshRef = useRef<THREE.Mesh>(null);
  const glow1Ref = useRef<THREE.Mesh>(null);
  const glow2Ref = useRef<THREE.Mesh>(null);
  const glow3Ref = useRef<THREE.Mesh>(null);
  const haloRef = useRef<THREE.Mesh>(null);
  const ring1Ref = useRef<THREE.Mesh>(null);
  const ring2Ref = useRef<THREE.Mesh>(null);
  const color = useMemo(() => new THREE.Color(node.color), [node.color]);
  const { camera, gl } = useThree();
  const dragPlane = useMemo(() => new THREE.Plane(), []);
  const dragOffset = useMemo(() => new THREE.Vector3(), []);
  const isDragging = useRef(false);

  // CRITICAL: Imperative position update every frame — smooth drag + physics
  useFrame((state) => {
    if (!groupRef.current) return;
    const t = state.clock.elapsedTime;

    // Position from physics/drag ref — updates every frame, no React re-render needed
    const pos = corePositions.current.get(node.id) || node.position;
    groupRef.current.position.set(pos.x, pos.y, pos.z);

    // Pulse + highlight
    if (!meshRef.current) return;
    const ids = highlightedIdsRef.current;
    const hasHighlight = ids.size > 0;
    const isHl = hasHighlight ? ids.has(node.id) : highlightedRef.current === node.id;
    const isDimmed = hasHighlight && !isHl;
    const isFiltered = filteredIds !== null && !filteredIds.has(node.id);
    const base = node.size * 0.7;
    const pulse = 1 + Math.sin(t * 0.4) * 0.03;
    const hl = isHl ? 1.2 : (isDimmed ? 0.85 : 1);
    const emissiveBoost = isHl ? 0.9 : (isDimmed ? 0.3 : 0.6);
    meshRef.current.scale.setScalar(base * pulse * hl);
    meshRef.current.visible = !isFiltered;
    (meshRef.current.material as THREE.MeshStandardMaterial).emissiveIntensity = emissiveBoost;
    if (glow1Ref.current) { glow1Ref.current.scale.setScalar(base * 1.4 * (1 + Math.sin(t * 0.5) * 0.04)); glow1Ref.current.visible = !isFiltered && isHl; }
    if (glow2Ref.current) { glow2Ref.current.scale.setScalar(base * 1.8 * (1 + Math.sin(t * 0.4 + 1) * 0.03)); glow2Ref.current.visible = !isFiltered && isHl; }
    if (glow3Ref.current) { glow3Ref.current.scale.setScalar(base * 2.3 * (1 + Math.sin(t * 0.3 + 2) * 0.025)); glow3Ref.current.visible = !isFiltered && isHl; }
    if (haloRef.current) { haloRef.current.rotation.z = t * 0.06; haloRef.current.visible = !isFiltered && isHl; }
    if (ring1Ref.current) { ring1Ref.current.rotation.z = t * 0.08; ring1Ref.current.rotation.x = t * 0.04; ring1Ref.current.visible = !isFiltered && (isHl || !hasHighlight); }
    if (ring2Ref.current) { ring2Ref.current.rotation.z = -t * 0.06; ring2Ref.current.rotation.y = t * 0.03; ring2Ref.current.visible = !isFiltered && (isHl || !hasHighlight); }
  });

  const startDrag = useCallback((nativeEvent: PointerEvent) => {
    if (nativeEvent.button !== 0) return;
    isDragging.current = true;
    dragRef.current = node.id;
    const pos = corePositions.current.get(node.id) || node.position;
    const camDir = new THREE.Vector3();
    camera.getWorldDirection(camDir);
    dragPlane.setFromNormalAndCoplanarPoint(camDir.negate(), pos);
    const raycaster = new THREE.Raycaster();
    raycaster.setFromCamera(new THREE.Vector2(
      nativeEvent.clientX / gl.domElement.clientWidth * 2 - 1,
      -(nativeEvent.clientY / gl.domElement.clientHeight) * 2 + 1,
    ), camera);
    const intersection = new THREE.Vector3();
    raycaster.ray.intersectPlane(dragPlane, intersection);
    dragOffset.copy(pos).sub(intersection);
    gl.domElement.setPointerCapture(nativeEvent.pointerId);
    const onMove = (me: PointerEvent) => {
      if (!isDragging.current) return;
      const mr = new THREE.Raycaster();
      mr.setFromCamera(new THREE.Vector2(
        me.clientX / gl.domElement.clientWidth * 2 - 1,
        -(me.clientY / gl.domElement.clientHeight) * 2 + 1,
      ), camera);
      const hit = new THREE.Vector3();
      mr.ray.intersectPlane(dragPlane, hit);
      if (hit) corePositions.current.set(node.id, hit.add(dragOffset));
    };
    const onUp = () => {
      isDragging.current = false;
      dragRef.current = null;
      gl.domElement.removeEventListener('pointermove', onMove);
      gl.domElement.removeEventListener('pointerup', onUp);
    };
    gl.domElement.addEventListener('pointermove', onMove);
    gl.domElement.addEventListener('pointerup', onUp);
  }, [camera, gl, node.id, node.position, dragPlane, dragOffset, corePositions, dragRef]);

  function getPos(): THREE.Vector3 {
    return corePositions.current.get(node.id) || node.position;
  }

  return (
    <group ref={groupRef} position={getPos()}>
      <mesh
        ref={meshRef} geometry={sphereGeo}
        onClick={(e) => { e.stopPropagation(); onClick(); }}
        onPointerEnter={(e) => {
          e.stopPropagation();
          if (dragRef.current) return; // Don't highlight during drag
          onHover();
          document.body.style.cursor = 'pointer';
        }}
        onPointerLeave={(e) => {
          e.stopPropagation();
          if (dragRef.current) return;
          onUnhover();
          document.body.style.cursor = 'auto';
        }}
        onPointerDown={(e) => {
          e.stopPropagation();
          if (e.button === 2) { onContextMenu(e.nativeEvent || e); return; }
          startDrag(e.nativeEvent);
        }}
      >
        <meshStandardMaterial color={color} emissive={color} emissiveIntensity={0.6} roughness={0.25} metalness={0.4} />
      </mesh>
      <mesh ref={glow1Ref} geometry={sphereGeo}>
        <meshBasicMaterial color={color} transparent opacity={0.15} blending={THREE.AdditiveBlending} depthWrite={false} />
      </mesh>
      <mesh ref={glow2Ref} geometry={sphereGeo}>
        <meshBasicMaterial color={color} transparent opacity={0.08} blending={THREE.AdditiveBlending} depthWrite={false} />
      </mesh>
      <mesh ref={glow3Ref} geometry={sphereGeo}>
        <meshBasicMaterial color={color} transparent opacity={0.03} blending={THREE.AdditiveBlending} depthWrite={false} />
      </mesh>
      <mesh ref={haloRef} rotation={[Math.PI / 2, 0, 0]}>
        <ringGeometry args={[node.size * 1.5, node.size * 2.2, 64]} />
        <meshBasicMaterial color={color} transparent opacity={0.06} blending={THREE.AdditiveBlending} depthWrite={false} side={THREE.DoubleSide} />
      </mesh>
      <mesh ref={ring1Ref} rotation={[Math.PI / 2, 0, 0]}>
        <torusGeometry args={[node.size * 2.0, 0.025, 6, 48]} />
        <meshBasicMaterial color={color} transparent opacity={0.2} blending={THREE.AdditiveBlending} depthWrite={false} />
      </mesh>
      <mesh ref={ring2Ref} rotation={[Math.PI / 3, Math.PI / 4, 0]}>
        <torusGeometry args={[node.size * 2.6, 0.015, 6, 48]} />
        <meshBasicMaterial color={color} transparent opacity={0.1} blending={THREE.AdditiveBlending} depthWrite={false} />
      </mesh>
      <PulseRings position={[0, 0, 0]} color={node.color} />
    </group>
  );
});

// ═══════════════════════════════════════════════════════════════
// SATELLITE INSTANCES (1 draw call)
// ═══════════════════════════════════════════════════════════════
const SatelliteInstances = memo(function SatelliteInstances({
  satellites, timeRef, onNodeClick, onNodeHover, onNodeUnhover,
  onNodeContextMenu, highlightedRef, highlightedIdsRef, filteredIds, corePositions, dragRef,
}: {
  satellites: GraphNodeData[]; timeRef: React.MutableRefObject<number>;
  onNodeClick: (n: GraphNodeData) => void; onNodeHover: (n: GraphNodeData) => void;
  onNodeUnhover: () => void; onNodeContextMenu: (n: GraphNodeData, e: MouseEvent) => void;
  highlightedRef: React.MutableRefObject<string | null>;
  highlightedIdsRef: React.MutableRefObject<Set<string>>;
  filteredIds: Set<string> | null;
  corePositions: React.MutableRefObject<Map<string, THREE.Vector3>>;
  dragRef: React.MutableRefObject<string | null>;
}) {
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const glowRef = useRef<THREE.InstancedMesh>(null);

  const instanceColors = useMemo(() => {
    const arr = new Float32Array(satellites.length * 3);
    for (let i = 0; i < satellites.length; i++) {
      _colorObj.set(satellites[i].color);
      arr[i * 3] = _colorObj.r; arr[i * 3 + 1] = _colorObj.g; arr[i * 3 + 2] = _colorObj.b;
    }
    return arr;
  }, [satellites]);

  const glowColors = useMemo(() => {
    const arr = new Float32Array(satellites.length * 3);
    for (let i = 0; i < satellites.length; i++) {
      _colorObj.set(satellites[i].color);
      arr[i * 3] = _colorObj.r; arr[i * 3 + 1] = _colorObj.g; arr[i * 3 + 2] = _colorObj.b;
    }
    return arr;
  }, [satellites]);

  useFrame(() => {
    if (!meshRef.current || satellites.length === 0) return;
    const t = timeRef.current;
    const isSearching = filteredIds !== null;
    const ids = highlightedIdsRef.current;
    const hasHighlight = ids.size > 0;
    for (let i = 0; i < satellites.length; i++) {
      const n = satellites[i];
      const isFiltered = isSearching && !filteredIds.has(n.id);
      const isHl = hasHighlight ? ids.has(n.id) || ids.has(n.coreId || '') : highlightedRef.current === n.coreId;
      const isDimmed = hasHighlight && !isHl;
      const corePos = n.coreId ? corePositions.current.get(n.coreId) : undefined;
      const wp = n.orbitRadius > 0 ? getOrbitPos(n, t, corePos) : (corePos || n.position);
      const scale = isHl ? 1.15 : (isDimmed ? 0.7 : 1);
      const sc = isFiltered ? 0.01 : n.size * 0.4 * scale;
      _dummy.position.copy(wp); _dummy.scale.setScalar(sc); _dummy.updateMatrix();
      meshRef.current.setMatrixAt(i, _dummy.matrix);
      _dummy.scale.setScalar(isFiltered ? 0.01 : n.size * 0.55 * scale); _dummy.updateMatrix();
      if (glowRef.current) glowRef.current.setMatrixAt(i, _dummy.matrix);
    }
    meshRef.current.instanceMatrix.needsUpdate = true;
    if (glowRef.current) glowRef.current.instanceMatrix.needsUpdate = true;
  });

  const { camera, raycaster, pointer } = useThree();
  const hoverIdx = useRef<number>(-1);
  useFrame(() => {
    if (!meshRef.current || satellites.length === 0) return;
    // Skip hover detection during drag
    if (dragRef.current) {
      if (hoverIdx.current >= 0) {
        onNodeUnhover();
        document.body.style.cursor = 'auto';
        hoverIdx.current = -1;
      }
      return;
    }
    raycaster.setFromCamera(pointer, camera);
    const intersects = raycaster.intersectObject(meshRef.current);
    const ni = intersects.length > 0 ? (intersects[0].instanceId ?? -1) : -1;
    if (ni !== hoverIdx.current) {
      if (hoverIdx.current >= 0 && hoverIdx.current < satellites.length) { onNodeUnhover(); document.body.style.cursor = 'auto'; }
      if (ni >= 0 && ni < satellites.length) { onNodeHover(satellites[ni]); document.body.style.cursor = 'pointer'; }
      hoverIdx.current = ni;
    }
  });
  const handleClick = useCallback(() => {
    if (hoverIdx.current >= 0 && hoverIdx.current < satellites.length) onNodeClick(satellites[hoverIdx.current]);
  }, [satellites, onNodeClick]);

  // Set instance colors imperatively after mount.
  //
  // This must stay above the early return below: React requires the same hooks
  // in the same order on every render, and returning first made this effect
  // conditional on there being satellites. Its own guard already handles the
  // empty case.
  useEffect(() => {
    if (!meshRef.current || satellites.length === 0) return;
    const mesh = meshRef.current;
    mesh.instanceColor = new THREE.InstancedBufferAttribute(instanceColors, 3);
    if (glowRef.current) {
      glowRef.current.instanceColor = new THREE.InstancedBufferAttribute(glowColors, 3);
    }
  }, [instanceColors, glowColors, satellites.length]);

  if (satellites.length === 0) return null;

  return (
    <>
      <instancedMesh ref={meshRef} args={[smallSphereGeo, undefined, satellites.length]}
        onClick={handleClick}
        onPointerDown={(e) => { if (e.button === 2 && hoverIdx.current >= 0) onNodeContextMenu(satellites[hoverIdx.current], e.nativeEvent || e); }}
        frustumCulled={false}>
        <meshStandardMaterial vertexColors emissive="#ffffff" emissiveIntensity={0.3} roughness={0.4} metalness={0.3} />
      </instancedMesh>
      <instancedMesh ref={glowRef} args={[smallSphereGeo, undefined, satellites.length]} frustumCulled={false}>
        <meshBasicMaterial vertexColors transparent opacity={0.07} blending={THREE.AdditiveBlending} depthWrite={false} />
      </instancedMesh>
    </>
  );
});

// ═══════════════════════════════════════════════════════════════
// DATA FLOW DOTS
// ═══════════════════════════════════════════════════════════════
const DataFlowDots = memo(function DataFlowDots({
  edges, nodeMap, timeRef, corePositions,
}: {
  edges: GraphEdgeData[]; nodeMap: Map<string, GraphNodeData>; timeRef: React.MutableRefObject<number>;
  corePositions: React.MutableRefObject<Map<string, THREE.Vector3>>;
}) {
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const edgeData = useMemo(() =>
    edges.filter(e => nodeMap.has(e.source) && nodeMap.has(e.target)).map((e, i) => ({
      src: nodeMap.get(e.source)!, tgt: nodeMap.get(e.target)!,
      speed: 0.12 + Math.random() * 0.08, phase: i * 0.1,
    })), [edges, nodeMap]);
  useFrame(() => {
    if (!meshRef.current || edgeData.length === 0) return;
    const t = timeRef.current;
    for (let i = 0; i < edgeData.length; i++) {
      const e = edgeData[i];
      const sp = resolveNodePosition(e.src, t, corePositions);
      const tp = resolveNodePosition(e.tgt, t, corePositions);
      const ph = ((t * e.speed + e.phase) % 1);
      _lerpVec.copy(sp).lerp(tp, ph);
      _dummy.position.copy(_lerpVec); _dummy.scale.setScalar(0.08); _dummy.updateMatrix();
      meshRef.current.setMatrixAt(i, _dummy.matrix);
    }
    meshRef.current.instanceMatrix.needsUpdate = true;
  });
  if (edgeData.length === 0) return null;
  return (
    <instancedMesh ref={meshRef} args={[tinySphereGeo, undefined, edgeData.length]} frustumCulled={false}>
      <meshBasicMaterial color="#ff8a5b" transparent opacity={0.5} blending={THREE.AdditiveBlending} depthWrite={false} />
    </instancedMesh>
  );
});

// ═══════════════════════════════════════════════════════════════
// EDGE LINES — color by relationship type, weight affects opacity
// ═══════════════════════════════════════════════════════════════
const _edgeColor = new THREE.Color();

const EdgeLines = memo(function EdgeLines({
  edges, nodeMap, timeRef, corePositions,
}: {
  edges: GraphEdgeData[]; nodeMap: Map<string, GraphNodeData>; timeRef: React.MutableRefObject<number>;
  corePositions: React.MutableRefObject<Map<string, THREE.Vector3>>;
}) {
  const lineRef = useRef<THREE.LineSegments>(null);
  const edgePairs = useMemo(() => edges.filter(e => nodeMap.has(e.source) && nodeMap.has(e.target)), [edges, nodeMap]);
  const geometry = useMemo(() => {
    const geo = new THREE.BufferGeometry();
    const count = Math.max(edgePairs.length * 6, 6);
    geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(count), 3));
    geo.setAttribute('color', new THREE.BufferAttribute(new Float32Array(count), 3));
    geo.setDrawRange(0, edgePairs.length * 2);
    return geo;
  }, [edgePairs.length]);

  useFrame(() => {
    if (!lineRef.current) return;
    const arr = (lineRef.current.geometry.getAttribute('position') as THREE.BufferAttribute).array as Float32Array;
    const col = (lineRef.current.geometry.getAttribute('color') as THREE.BufferAttribute).array as Float32Array;
    const t = timeRef.current;
    for (let i = 0; i < edgePairs.length; i++) {
      const e = edgePairs[i];
      const src = nodeMap.get(e.source); const tgt = nodeMap.get(e.target);
      if (!src || !tgt) continue;
      const sp = resolveNodePosition(src, t, corePositions);
      const tp = resolveNodePosition(tgt, t, corePositions);
      arr[i * 6] = sp.x; arr[i * 6 + 1] = sp.y; arr[i * 6 + 2] = sp.z;
      arr[i * 6 + 3] = tp.x; arr[i * 6 + 4] = tp.y; arr[i * 6 + 5] = tp.z;
      // Color per edge based on relationship type
      const hex = getEdgeColor(e.relationshipType);
      _edgeColor.set(hex);
      const w = Math.max(0.4, Math.min(1, e.weight));
      _edgeColor.multiplyScalar(w);
      col[i * 6] = _edgeColor.r; col[i * 6 + 1] = _edgeColor.g; col[i * 6 + 2] = _edgeColor.b;
      col[i * 6 + 3] = _edgeColor.r; col[i * 6 + 4] = _edgeColor.g; col[i * 6 + 5] = _edgeColor.b;
    }
    (lineRef.current.geometry.getAttribute('position') as THREE.BufferAttribute).needsUpdate = true;
    (lineRef.current.geometry.getAttribute('color') as THREE.BufferAttribute).needsUpdate = true;
  });
  if (edgePairs.length === 0) return null;
  return (
    <lineSegments ref={lineRef} geometry={geometry} frustumCulled={false}>
      <lineBasicMaterial vertexColors transparent opacity={0.5} blending={THREE.AdditiveBlending} depthWrite={false} />
    </lineSegments>
  );
});

// ═══════════════════════════════════════════════════════════════
// ORBIT PATHS
// ═══════════════════════════════════════════════════════════════
const OrbitPaths = memo(function OrbitPaths({ orbits }: { orbits: { position: THREE.Vector3; orbitRadius: number; color: string }[] }) {
  return (
    <>
      {orbits.map((orb, i) => (
        <mesh key={i} position={[orb.position.x, orb.position.y, orb.position.z]} rotation={[Math.PI / 2, 0, 0]}>
          <ringGeometry args={[orb.orbitRadius - 0.01, orb.orbitRadius + 0.01, 64]} />
          <meshBasicMaterial color={orb.color} transparent opacity={0.04} side={THREE.DoubleSide} blending={THREE.AdditiveBlending} depthWrite={false} />
        </mesh>
      ))}
    </>
  );
});

// ═══════════════════════════════════════════════════════════════
// SCREEN PROJECTION TRACKER
// ═══════════════════════════════════════════════════════════════
/**
 * Projects every node to screen space once per frame and, in the same pass,
 * decides how much detail it deserves.
 *
 * Combining the two is deliberate. The projection already computes a world
 * position and a camera-relative depth; classifying the level of detail here
 * costs one squared distance and reuses everything else. Doing it in the label
 * layer instead would mean walking every node a second time, in the DOM, at
 * 60 Hz.
 *
 * The results land in the shared `screenPos` map, which the label layer reads
 * without touching React state — a per-frame `setState` over thousands of nodes
 * would re-render the whole tree.
 */
function ScreenProjectionTracker({
  nodes, timeRef, corePositions, labelIdsRef, pinnedIdsRef,
}: {
  nodes: GraphNodeData[]; timeRef: React.MutableRefObject<number>;
  corePositions: React.MutableRefObject<Map<string, THREE.Vector3>>;
  labelIdsRef: React.MutableRefObject<Set<string>>;
  pinnedIdsRef: React.MutableRefObject<Set<string>>;
}) {
  const { camera, size } = useThree();

  // Thresholds and label budget tighten as the graph grows, so a 5000-node
  // workspace stays interactive without changing how a 50-node demo looks.
  const thresholds = useMemo(() => thresholdsForNodeCount(nodes.length), [nodes.length]);
  const labelBudget = useMemo(() => labelBudgetForNodeCount(nodes.length), [nodes.length]);

  // Reused across frames: allocating a candidate array per frame would hand the
  // garbage collector thousands of short-lived objects a second.
  const candidatesRef = useRef<LabelCandidate[]>([]);

  useFrame(() => {
    const t = timeRef.current;
    const frustum = updateFrustum(camera);
    const camPos = camera.position;

    const candidates = candidatesRef.current;
    candidates.length = 0;

    for (let i = 0; i < nodes.length; i++) {
      const n = nodes[i];
      const wp = resolveNodePosition(n, t, corePositions);

      const distanceSq = camPos.distanceToSquared(wp);
      const tier = tierForDistanceSq(distanceSq, thresholds);

      // Pad the cull radius by the node's own size so a large core whose centre
      // sits just off-screen does not blink out while its body still overlaps.
      const onScreen =
        tier !== LodTier.Culled && isSphereVisible(wp, n.size * 2, frustum);

      _projVec.copy(wp).project(camera);
      const visible = onScreen && _projVec.z < 1;

      screenPos.set(n.id, {
        x: (_projVec.x * 0.5 + 0.5) * size.width,
        y: (-_projVec.y * 0.5 + 0.5) * size.height,
        visible,
        distanceSq,
        tier,
      });

      candidates.push({
        id: n.id,
        distanceSq,
        isCore: n.orbitRadius === 0,
        visible,
        pinned: pinnedIdsRef.current.has(n.id),
      });
    }

    // One label decision per frame for the whole graph, rather than each label
    // deciding for itself.
    labelIdsRef.current = selectLabels(candidates, labelBudget);
  });
  return null;
}


// ═══════════════════════════════════════════════════════════════
// R3F SCENE — self-contained physics + drag
// ═══════════════════════════════════════════════════════════════
function Scene({
  clusters, allNodes, satellites, edges,
  onNodeClick, onNodeHover, onNodeUnhover, onNodeContextMenu,
  highlightedRef, highlightedIdsRef, filteredIds,
  labelIdsRef, pinnedIdsRef,
}: {
  clusters: OrbitCluster[]; allNodes: GraphNodeData[]; satellites: GraphNodeData[];
  edges: GraphEdgeData[];
  onNodeClick: (n: GraphNodeData) => void; onNodeHover: (n: GraphNodeData) => void;
  onNodeUnhover: () => void; onNodeContextMenu: (n: GraphNodeData, e: MouseEvent) => void;
  highlightedRef: React.MutableRefObject<string | null>;
  highlightedIdsRef: React.MutableRefObject<Set<string>>;
  filteredIds: Set<string> | null;
  /** Written by the projection pass, read by the DOM label layer. */
  labelIdsRef: React.MutableRefObject<Set<string>>;
  /** Ids that keep their label regardless of budget: selection, hover, search. */
  pinnedIdsRef: React.MutableRefObject<Set<string>>;
}) {
  const timeRef = useRef(0);
  const dragRef = useRef<string | null>(null);
  const corePositions = useRef<Map<string, THREE.Vector3>>(new Map());
  const initKey = clusters.map(c => c.coreId).join(',');
  useMemo(() => {
    const m = new Map<string, THREE.Vector3>();
    clusters.forEach(c => m.set(c.coreId, c.center.clone()));
    corePositions.current = m;
  }, [initKey]); // eslint-disable-line react-hooks/exhaustive-deps

  const nodeMap = useMemo(() => {
    const m = new Map<string, GraphNodeData>(); allNodes.forEach(n => m.set(n.id, n)); return m;
  }, [allNodes]);

  // Edges actually handed to the renderer, capped at the budget.
  //
  // Only the *drawing* is trimmed: hover propagation below still walks the full
  // edge list, so dropping a faint link from the picture never changes which
  // neighbours light up when a node is hovered.
  const drawnEdges = useMemo(() => selectEdges(edges), [edges]);

  // Adjacency list for hover propagation
  const adjMap = useMemo(() => {
    const m = new Map<string, Set<string>>();
    allNodes.forEach(n => m.set(n.id, new Set()));
    edges.forEach(e => { m.get(e.source)?.add(e.target); m.get(e.target)?.add(e.source); });
    return m;
  }, [allNodes, edges]);

  // 30 FPS physics — repulsion + centering + damping + minimum distance
  const PHYSICS_INTERVAL = 1 / 30;
  const lastPhysicsRef = useRef(0);
  const velocitiesRef = useRef<Map<string, THREE.Vector3>>(new Map());
  const REPULSION = 400;
  const MIN_DIST = 18;
  const CENTERING = 0.001;
  const DAMPING = 0.82;

  useFrame((_, dt) => {
    timeRef.current += dt;
    if (timeRef.current - lastPhysicsRef.current >= PHYSICS_INTERVAL) {
      lastPhysicsRef.current = timeRef.current;
      const pos = corePositions.current;
      const vel = velocitiesRef.current;
      // Build a lookup from coreId to cluster for centering
      const clusterMap = new Map<string, OrbitCluster>();
      clusters.forEach(c => clusterMap.set(c.coreId, c));

      // Repulsion between ALL pairs
      for (let i = 0; i < clusters.length; i++) {
        const ci = pos.get(clusters[i].coreId);
        if (!ci) continue;
        if (!vel.has(clusters[i].coreId)) vel.set(clusters[i].coreId, new THREE.Vector3());
        for (let j = i + 1; j < clusters.length; j++) {
          const cj = pos.get(clusters[j].coreId);
          if (!cj) continue;
          const dx = ci.x - cj.x; const dy = ci.y - cj.y; const dz = ci.z - cj.z;
          const distSq = dx * dx + dy * dy + dz * dz;
          const dist = Math.sqrt(distSq) || 1;
          const effectiveDist = Math.max(dist, MIN_DIST);
          const force = REPULSION / (effectiveDist * effectiveDist);
          const fx = (dx / dist) * force * PHYSICS_INTERVAL;
          const fy = (dy / dist) * force * PHYSICS_INTERVAL;
          const fz = (dz / dist) * force * PHYSICS_INTERVAL;
          const vi = vel.get(clusters[i].coreId)!;
          const vj = vel.get(clusters[j].coreId)!;
          if (dragRef.current !== clusters[i].coreId) { vi.x += fx; vi.y += fy; vi.z += fz; }
          if (dragRef.current !== clusters[j].coreId) { vj.x -= fx; vj.y -= fy; vj.z -= fz; }
        }
      }
      // Centering + damping + integrate
      for (let i = 0; i < clusters.length; i++) {
        if (dragRef.current === clusters[i].coreId) continue;
        const ci = pos.get(clusters[i].coreId);
        const vi = vel.get(clusters[i].coreId);
        if (!ci || !vi) continue;
        const orig = clusterMap.get(clusters[i].coreId)?.center || clusters[i].center;
        vi.x += (orig.x - ci.x) * CENTERING;
        vi.y += (orig.y - ci.y) * CENTERING;
        vi.z += (orig.z - ci.z) * CENTERING;
        vi.multiplyScalar(DAMPING);
        ci.x += vi.x; ci.y += vi.y; ci.z += vi.z;
      }
    }
  });

  // Ref type comes from drei's own forwardRef signature, so it stays correct if
  // drei swaps its underlying OrbitControls implementation.
  const orbitControlsRef = useRef<React.ComponentRef<typeof OrbitControls> | null>(null);
  useFrame(() => {
    if (orbitControlsRef.current) orbitControlsRef.current.enabled = dragRef.current === null;
  });

  return (
    <>
      <OrbitControls ref={orbitControlsRef} enableDamping dampingFactor={0.08} rotateSpeed={0.6} zoomSpeed={0.8} panSpeed={0.5} minDistance={5} maxDistance={200} makeDefault />
      <Starfield count={600} />
      <AmbientDust count={180} />
      <CosmicSun />
      <ambientLight intensity={0.15} />
      <pointLight position={[0, 20, 0]} intensity={0.8} color="#ff8a5b" distance={100} />
      <pointLight position={[-30, 10, 20]} intensity={0.3} color="#a99cf8" distance={80} />
      <pointLight position={[25, -5, -15]} intensity={0.3} color="#63d8d2" distance={80} />

      {clusters.map(c => {
        const n = allNodes.find(nd => nd.id === c.coreId);
        if (!n) return null;
        return (
          <CoreNode key={c.coreId} node={n}
            onClick={() => onNodeClick(n)}
            onHover={() => {
              onNodeHover(n);
              highlightedRef.current = c.coreId;
              // BFS: highlight this node + all directly connected nodes
              const ids = new Set<string>([c.coreId]);
              const neighbors = adjMap.get(c.coreId);
              if (neighbors) neighbors.forEach(nid => ids.add(nid));
              highlightedIdsRef.current = ids;
            }}
            onUnhover={() => {
              onNodeUnhover();
              highlightedRef.current = null;
              highlightedIdsRef.current = new Set();
            }}
            onContextMenu={(e) => onNodeContextMenu(n, e)}
            highlightedRef={highlightedRef}
            highlightedIdsRef={highlightedIdsRef}
            corePositions={corePositions} dragRef={dragRef} filteredIds={filteredIds}
          />
        );
      })}

      <SatelliteInstances satellites={satellites} timeRef={timeRef}
        onNodeClick={onNodeClick}
        onNodeHover={(n) => {
          onNodeHover(n);
          highlightedRef.current = n.id;
          // BFS: highlight satellite + its parent core + all neighbors of the core
          const ids = new Set<string>([n.id]);
          if (n.coreId) {
            ids.add(n.coreId);
            const coreNeighbors = adjMap.get(n.coreId);
            if (coreNeighbors) coreNeighbors.forEach(nid => ids.add(nid));
          }
          highlightedIdsRef.current = ids;
        }}
        onNodeUnhover={() => {
          onNodeUnhover();
          highlightedRef.current = null;
          highlightedIdsRef.current = new Set();
        }}
        onNodeContextMenu={onNodeContextMenu}
        highlightedRef={highlightedRef} highlightedIdsRef={highlightedIdsRef}
        filteredIds={filteredIds}
        corePositions={corePositions} dragRef={dragRef} />

      <OrbitPaths orbits={satellites.map(s => ({ position: s.position, orbitRadius: s.orbitRadius, color: s.color }))} />
      <EdgeLines edges={drawnEdges} nodeMap={nodeMap} timeRef={timeRef} corePositions={corePositions} />
      <DataFlowDots edges={drawnEdges} nodeMap={nodeMap} timeRef={timeRef} corePositions={corePositions} />
      <ScreenProjectionTracker nodes={allNodes} timeRef={timeRef} corePositions={corePositions}
        labelIdsRef={labelIdsRef} pinnedIdsRef={pinnedIdsRef} />
    </>
  );
}


// ═══════════════════════════════════════════════════════════════
// MAIN EXPORT
// ═══════════════════════════════════════════════════════════════
export function CosmicGraphView() {
  const { nodes, edges, fetchGraph, isLoading } = useGraphStore();
  const { selectMemory, memories, fetchMemories } = useMemoryStore();
  const { copilotOpen, toggleCopilot, setActiveView } = useUiStore();
  const { t } = useLocale();
  const [selectedNode, setSelectedNode] = useState<GraphNodeData | null>(null);
  const [hoveredNode, setHoveredNode] = useState<GraphNodeData | null>(null);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; node: GraphNodeData } | null>(null);
  const [hiddenIds, setHiddenIds] = useState<Set<string>>(new Set());
  const containerRef = useRef<HTMLDivElement>(null);
  const hoveredIdRef = useRef<string | null>(null);
  const highlightedRef = useRef<string | null>(null);
  const highlightedIdsRef = useRef<Set<string>>(new Set());
  const nodeClickedRef = useRef(false);

  // Level-of-detail hand-off between the R3F frame loop and the DOM label layer.
  //
  // Refs rather than state: the projection pass rewrites `labelIdsRef` every
  // frame, and routing that through setState would re-render the whole tree 60
  // times a second — the exact cost the label budget exists to avoid.
  const labelIdsRef = useRef<Set<string>>(new Set());
  const pinnedIdsRef = useRef<Set<string>>(new Set());

  useEffect(() => { fetchGraph(); fetchMemories(); }, [fetchGraph, fetchMemories]);

  const { clusters, allNodes, uniqueEdges, satellites } = useMemo(
    () => buildClusters(nodes.filter(n => !hiddenIds.has(n.id)), edges), [nodes, edges, hiddenIds],
  );

  // Search suggestions — entities + memories (match title, type, and description)
  const searchSuggestions = useMemo<SearchSuggestion[]>(() => {
    if (searchQuery.length < 2) return [];
    const q = searchQuery.toLowerCase();
    const entityMatches: SearchSuggestion[] = allNodes
      .filter(n => n.title.toLowerCase().includes(q) || n.entityType.toLowerCase().includes(q) || n.description?.toLowerCase().includes(q))
      .slice(0, 5)
      .map(n => ({ id: n.id, title: n.title, type: n.entityType, color: n.color, kind: 'entity' as const, description: n.description, connectionCount: n.connectionCount }));
    const memoryMatches: SearchSuggestion[] = memories
      .filter(m => m.title.toLowerCase().includes(q) || m.summary?.toLowerCase().includes(q))
      .slice(0, 3)
      .map(m => ({ id: m.id, title: m.title, type: 'Memory', color: '#f472b6', kind: 'memory' as const, description: m.summary }));
    return [...entityMatches, ...memoryMatches].slice(0, 8);
  }, [searchQuery, allNodes, memories]);

  const filteredIds = useMemo(() => {
    if (!searchQuery.trim()) return null;
    const q = searchQuery.toLowerCase();
    const ids = new Set<string>();
    allNodes.forEach(n => {
      if (n.title.toLowerCase().includes(q) || n.entityType.toLowerCase().includes(q) || n.description?.toLowerCase().includes(q))
        ids.add(n.id);
    });
    return ids;
  }, [searchQuery, allNodes]);

  // Labels that survive the budget no matter how far the camera moves.
  //
  // Without this, orbiting away from a node you just clicked silently drops its
  // name — the label budget cannot distinguish "far" from "the one thing the
  // user is looking at". Search hits are pinned for the same reason: a match the
  // user typed must stay legible even in a dense graph.
  useEffect(() => {
    const pinned = new Set<string>();
    if (selectedNode) pinned.add(selectedNode.id);
    if (hoveredNode) pinned.add(hoveredNode.id);
    if (filteredIds) filteredIds.forEach(id => pinned.add(id));
    pinnedIdsRef.current = pinned;
  }, [selectedNode, hoveredNode, filteredIds]);

  const memoryCountMap = useMemo(() => {
    const m = new Map<string, number>();
    memories.forEach(mem => { mem.linkedEntityIds?.forEach(eid => m.set(eid, (m.get(eid) || 0) + 1)); });
    return m;
  }, [memories]);

  // Related memories for selected node
  const relatedMemories = useMemo(() => {
    if (!selectedNode) return [];
    return memories.filter(m =>
      m.linkedEntityIds?.includes(selectedNode.id) ||
      m.title.toLowerCase().includes(selectedNode.title.toLowerCase())
    ).slice(0, 5);
  }, [selectedNode, memories]);

  // Connected nodes for selected node
  const connectedNodes = useMemo(() => {
    if (!selectedNode) return [];
    const connectedIds = new Set<string>();
    uniqueEdges.forEach(e => {
      if (e.source === selectedNode.id) connectedIds.add(e.target);
      if (e.target === selectedNode.id) connectedIds.add(e.source);
    });
    return allNodes.filter(n => connectedIds.has(n.id));
  }, [selectedNode, uniqueEdges, allNodes]);

  const handleNodeClick = useCallback((n: GraphNodeData) => {
    nodeClickedRef.current = true;
    setTimeout(() => { nodeClickedRef.current = false; }, 0);
    setSelectedNode(prev => prev?.id === n.id ? null : n);
  }, []);
  const handleNodeHover = useCallback((n: GraphNodeData) => setHoveredNode(n), []);
  const handleNodeUnhover = useCallback(() => setHoveredNode(null), []);
  const handleNodeContextMenu = useCallback((n: GraphNodeData, e: MouseEvent) => {
    setContextMenu({ x: e.clientX || 0, y: e.clientY || 0, node: n });
  }, []);

  // Ask Copilot — build entity context and send
  const handleAskCopilot = useCallback((n: GraphNodeData) => {
    const entityEdges = uniqueEdges.filter(e => e.source === n.id || e.target === n.id);
    const relatedMems = memories.filter(m =>
      m.linkedEntityIds?.includes(n.id) || m.title.toLowerCase().includes(n.title.toLowerCase())
    );
    const connectedTitles = entityEdges.map(e => {
      const otherId = e.source === n.id ? e.target : e.source;
      const other = nodes.find(nd => nd.id === otherId);
      return other ? `${other.title} (${e.relationshipType})` : e.relationshipType;
    });

    let contextMessage = `Tell me about the entity "${n.title}" (${n.entityType})`;
    if (n.description) contextMessage += `. Description: ${n.description}`;
    if (connectedTitles.length > 0) contextMessage += `. It connects to: ${connectedTitles.join(', ')}`;
    if (relatedMems.length > 0) contextMessage += `. Related memories: ${relatedMems.map(m => m.title).join(', ')}`;
    contextMessage += '. Provide insights about relationships, importance, and suggestions.';

    // Pre-fill copilot and open
    selectMemory(relatedMems[0] || null);
    if (!copilotOpen) toggleCopilot();
    // Small delay to let copilot mount, then set input via DOM event
    setTimeout(() => {
      const input = document.querySelector('.ai-input') as HTMLInputElement | null;
      if (input) {
        const nativeInputValueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
        nativeInputValueSetter?.call(input, contextMessage);
        input.dispatchEvent(new Event('input', { bubbles: true }));
      }
    }, 100);
  }, [uniqueEdges, memories, nodes, selectMemory, copilotOpen, toggleCopilot]);

  const handleViewMemory = useCallback((n: GraphNodeData) => {
    const matching = memories.find(m => m.title === n.title || m.linkedEntityIds?.includes(n.id));
    if (matching) selectMemory(matching);
  }, [memories, selectMemory]);

  const handleHideNode = useCallback((id: string) => { setHiddenIds(prev => new Set(prev).add(id)); setSelectedNode(null); }, []);

  const toggleFullscreen = useCallback(() => {
    if (!containerRef.current) return;
    if (!document.fullscreenElement) { containerRef.current.requestFullscreen?.(); setIsFullscreen(true); }
    else { document.exitFullscreen?.(); setIsFullscreen(false); }
  }, []);

  useEffect(() => {
    const h = () => setIsFullscreen(!!document.fullscreenElement);
    document.addEventListener('fullscreenchange', h);
    return () => document.removeEventListener('fullscreenchange', h);
  }, []);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const h = (e: Event) => e.preventDefault();
    el.addEventListener('contextmenu', h);
    return () => el.removeEventListener('contextmenu', h);
  }, []);

  // All entities as search suggestions (for initial dropdown) — MUST be before early returns (Rules of Hooks)
  const allSearchEntities = useMemo<SearchSuggestion[]>(() => {
    const entities: SearchSuggestion[] = allNodes
      .filter(n => n.orbitRadius === 0) // core nodes only
      .map(n => ({ id: n.id, title: n.title, type: n.entityType, color: n.color, kind: 'entity' as const, description: n.description, connectionCount: n.connectionCount }));
    const mems: SearchSuggestion[] = memories.slice(0, 20).map(m => ({
      id: m.id, title: m.title, type: 'Memory', color: '#f472b6', kind: 'memory' as const, description: m.summary,
    }));
    return [...entities, ...mems];
  }, [allNodes, memories]);

  if (isLoading) return <div className="empty-state"><div className="empty-state-title">{t('graph.loading')}</div></div>;
  if (nodes.length === 0) return (
    <div className="empty-state">
      <Network size={48} className="empty-state-icon" />
      <div className="empty-state-title">{t('graph.empty')}</div>
      <div className="empty-state-desc">Entities and relationships will appear here</div>
    </div>
  );

  const uniqueTypes = [...new Set(nodes.map(n => n.entityType))];

  const entityIcons: Record<string, string> = {
    person: '👤', project: '📁', decision: '⚖️', task: '✓',
    technology: '🔧', file: '📄', organization: '🏢', meeting: '📅',
    concept: '💡', document: '📝', default: '●',
  };

  return (
    <div ref={containerRef} className="cosmic-graph-wrapper" style={{ background: '#06070b' }}>
      <Canvas camera={{ position: [0, 10, 40], fov: 55, near: 0.1, far: 700 }}
        gl={{ antialias: true, alpha: false, powerPreference: 'high-performance' }}
        dpr={[1, 1.5]} style={{ position: 'absolute', inset: 0 }}
        onPointerMissed={() => {
          if (!nodeClickedRef.current) {
            setSelectedNode(null);
            setContextMenu(null);
          }
        }}>
        <color attach="background" args={['#06070b']} />
        <Scene clusters={clusters} allNodes={allNodes} satellites={satellites} edges={uniqueEdges}
          onNodeClick={handleNodeClick} onNodeHover={handleNodeHover}
          onNodeUnhover={handleNodeUnhover} onNodeContextMenu={handleNodeContextMenu}
          highlightedRef={highlightedRef} highlightedIdsRef={highlightedIdsRef}
          filteredIds={filteredIds}
          labelIdsRef={labelIdsRef} pinnedIdsRef={pinnedIdsRef} />
      </Canvas>

      <LabelLayer nodes={allNodes} hoveredIdRef={hoveredIdRef}
        highlightedIdsRef={highlightedIdsRef} filteredIds={filteredIds}
        labelIdsRef={labelIdsRef} />
      <SearchBar value={searchQuery} onChange={setSearchQuery} onClear={() => setSearchQuery('')}
        suggestions={searchSuggestions}
        allEntities={allSearchEntities}
        onSelectSuggestion={(s) => {
          if (s.kind === 'memory') {
            const mem = memories.find(m => m.id === s.id);
            if (mem) { selectMemory(mem); setSearchQuery(''); setActiveView('memory'); }
          } else {
            const node = allNodes.find(n => n.id === s.id);
            if (node) {
              // Unhide if hidden
              setHiddenIds(prev => { if (!prev.has(s.id)) return prev; const next = new Set(prev); next.delete(s.id); return next; });
              // Highlight selected node + its connected neighbors
              const ids = new Set<string>([s.id]);
              uniqueEdges.forEach(e => {
                if (e.source === s.id) ids.add(e.target);
                if (e.target === s.id) ids.add(e.source);
              });
              highlightedIdsRef.current = ids;
              highlightedRef.current = s.id;
              setSelectedNode(node);
              setSearchQuery('');
              // Clear highlight after 3 seconds
              setTimeout(() => { highlightedIdsRef.current = new Set(); highlightedRef.current = null; }, 3000);
            }
          }
        }} />

      <div className="graph-controls">
        <button className="graph-control-btn" onClick={toggleFullscreen} title="Fullscreen">
          {isFullscreen ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
        </button>
      </div>

      <div className="graph-legend">
        {uniqueTypes.map(type => (
          <div key={type} className="graph-legend-item">
            <div className="graph-legend-dot" style={{ background: getColor(type), boxShadow: `0 0 8px ${getColor(type)}44` }} />
            <span>{entityIcons[type.toLowerCase()] || ''} {type}</span>
          </div>
        ))}
      </div>

      <div className="graph-stats">
        <span>{clusters.length} clusters</span>
        <span>{satellites.length} satellites</span>
        <span>{uniqueEdges.length} connections</span>
        {hoveredNode && <span style={{ color: hoveredNode.color }}>{hoveredNode.title}</span>}
      </div>

      {hoveredNode && !selectedNode && !contextMenu && (
        <div className="cosmic-hover-tooltip" style={{ position: 'absolute', bottom: 60, left: '50%', transform: 'translateX(-50%)' }}>
          <span style={{ color: hoveredNode.color }}>{hoveredNode.title}</span>
          <span style={{ color: 'var(--muted-2)', marginLeft: 8 }}>{hoveredNode.entityType}</span>
          {hoveredNode.connectionCount > 0 && <span style={{ color: 'var(--muted-3)', marginLeft: 8 }}>{hoveredNode.connectionCount} conn</span>}
        </div>
      )}

      <InfoPanel node={selectedNode} onClose={() => setSelectedNode(null)} onAskCopilot={handleAskCopilot}
        relatedMemoryCount={selectedNode ? (memoryCountMap.get(selectedNode.id) || 0) : 0}
        relatedMemories={relatedMemories}
        connectedNodes={connectedNodes}
        allEdges={uniqueEdges}
        onSelectNode={(n) => setSelectedNode(n)}
        onViewMemory={(id) => { const mem = memories.find(m => m.id === id); if (mem) selectMemory(mem); }} />

      <ContextMenu data={contextMenu} onClose={() => setContextMenu(null)} onAskCopilot={handleAskCopilot}
        onFocus={(n) => setSelectedNode(n)} onHide={handleHideNode} onViewMemory={handleViewMemory} />

      <div className="graph-info-badge">
        <Info size={12} />
        <span>Scroll zoom · Drag rotate · Right-click for menu</span>
      </div>
    </div>
  );
}