import { useRef, useMemo, useState, useCallback, useEffect, memo } from 'react';
import { Canvas, useFrame, useThree } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import * as THREE from 'three';
import { useGraphStore } from '../../stores/graphStore';
import { useMemoryStore } from '../../stores/memoryStore';
import { useUiStore } from '../../stores/uiStore';
import { useLocale } from '../../stores/localeStore';
import {
  Network, Maximize2, Minimize2, Info, MessageCircle, Search, X,
  Brain, Link2, Eye, Focus, ChevronRight, Sparkles,
} from 'lucide-react';

// ═══════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════
const entityColors: Record<string, string> = {
  project: '#ff8a5b', file: '#a99cf8', person: '#63d8d2',
  concept: '#ddbb65', task: '#f472b6', decision: '#818cf8',
  organization: '#f59e0b', meeting: '#34d399', document: '#c084fc',
  technology: '#60a5fa', memory: '#f472b6', default: '#93c5fd',
};

function getColor(entityType: string): string {
  const lower = entityType.toLowerCase();
  for (const [key, val] of Object.entries(entityColors)) {
    if (lower.includes(key)) return val;
  }
  return entityColors.default;
}

const _dummy = new THREE.Object3D();
const _colorObj = new THREE.Color();
const _projVec = new THREE.Vector3();
const _lerpVec = new THREE.Vector3();

const sphereGeo = new THREE.SphereGeometry(1, 24, 24);
const smallSphereGeo = new THREE.SphereGeometry(1, 12, 12);
const tinySphereGeo = new THREE.SphereGeometry(1, 8, 8);

const ORBIT_SPEED_MULT = 0.012;
const ORBIT_SPREAD = 1.6;

// ═══════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════
interface GraphNodeData {
  id: string; title: string; entityType: string; color: string;
  position: THREE.Vector3; orbitRadius: number; orbitSpeed: number;
  orbitOffset: number; coreId: string | null; size: number;
  connectionCount: number; description?: string;
}
interface GraphEdgeData {
  id: string; source: string; target: string;
  relationshipType: string; weight: number;
}
interface OrbitCluster {
  coreId: string; coreTitle: string; coreColor: string;
  satellites: GraphNodeData[]; center: THREE.Vector3;
}
interface SearchSuggestion {
  id: string; title: string; type: string; color: string; kind: 'entity' | 'memory';
  description?: string; connectionCount?: number;
}

const screenPos = new Map<string, { x: number; y: number; visible: boolean }>();

// ═══════════════════════════════════════════════════════════════
// UTILITIES
// ═══════════════════════════════════════════════════════════════
function getOrbitPos(n: GraphNodeData, t: number, corePos?: THREE.Vector3): THREE.Vector3 {
  const a = t * n.orbitSpeed + n.orbitOffset;
  const base = corePos || n.position;
  return new THREE.Vector3(
    base.x + Math.cos(a) * n.orbitRadius,
    base.y + Math.sin(a * 0.5) * n.orbitRadius * 0.3,
    base.z + Math.sin(a) * n.orbitRadius,
  );
}

// Resolve LIVE position for any node — core or satellite
// Core nodes (coreId=null) → corePositions.get(n.id)
// Satellites (coreId set) → orbit around parent core position
function resolveNodePosition(
  n: GraphNodeData, t: number,
  corePositions: React.MutableRefObject<Map<string, THREE.Vector3>>,
): THREE.Vector3 {
  if (n.orbitRadius > 0 && n.coreId) {
    const corePos = corePositions.current.get(n.coreId);
    return getOrbitPos(n, t, corePos);
  }
  // Core node or satellite without core — use live physics position
  const livePos = corePositions.current.get(n.id);
  return livePos || n.position;
}

// Format node name based on entity type — makes names descriptive and concise
function formatNodeName(title: string, entityType: string): string {
  const lower = entityType.toLowerCase();

  // Files: extract filename, ensure extension
  if (lower.includes('file')) {
    // Extract last path segment
    const parts = title.replace(/\\/g, '/').split('/');
    let name = parts[parts.length - 1] || title;
    // If no extension, add a type hint
    if (!name.includes('.')) {
      const ext = lower.includes('config') ? '.config' : lower.includes('test') ? '.test' : '.file';
      name += ext;
    }
    return name.length > 28 ? name.slice(0, 26) + '…' : name;
  }

  // Code / function / method: show short signature
  if (lower.includes('function') || lower.includes('method') || lower.includes('class')) {
    // Remove full path, keep just the identifier
    const parts = title.replace(/\\/g, '/').split('/');
    const name = parts[parts.length - 1] || title;
    return name.length > 28 ? name.slice(0, 26) + '…' : name;
  }

  // Tasks: keep concise
  if (lower.includes('task')) {
    return title.length > 25 ? title.slice(0, 23) + '…' : title;
  }

  // Concepts / decisions: extract key phrase, single-word if possible
  if (lower.includes('concept') || lower.includes('decision')) {
    // Remove common filler words for a more concise label
    const cleaned = title
      .replace(/\b(implementation|concept|decision|approach|strategy|pattern|architecture)\b/gi, '')
      .replace(/\s{2,}/g, ' ')
      .trim();
    if (cleaned.length > 3 && cleaned.length <= 28) return cleaned;
    return title.length > 28 ? title.slice(0, 26) + '…' : title;
  }

  // Default: truncate long names
  return title.length > 28 ? title.slice(0, 26) + '…' : title;
}

// ═══════════════════════════════════════════════════════════════
// STARFIELD
// ═══════════════════════════════════════════════════════════════
const Starfield = memo(function Starfield({ count = 600 }: { count?: number }) {
  const positions = useMemo(() => {
    const a = new Float32Array(count * 3);
    for (let i = 0; i < count; i++) {
      a[i * 3] = (Math.random() - 0.5) * 500;
      a[i * 3 + 1] = (Math.random() - 0.5) * 500;
      a[i * 3 + 2] = (Math.random() - 0.5) * 500;
    }
    return a;
  }, [count]);
  const ref = useRef<THREE.Points>(null);
  useFrame((_, dt) => { if (ref.current) ref.current.rotation.y += dt * 0.001; });
  return (
    <points ref={ref}>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
      </bufferGeometry>
      <pointsMaterial color="#ffffff" size={0.35} transparent opacity={0.4} sizeAttenuation depthWrite={false} />
    </points>
  );
});

// ═══════════════════════════════════════════════════════════════
// AMBIENT DUST
// ═══════════════════════════════════════════════════════════════
const AmbientDust = memo(function AmbientDust({ count = 180 }: { count?: number }) {
  const [positions, colors, speeds] = useMemo(() => {
    const pos = new Float32Array(count * 3);
    const col = new Float32Array(count * 3);
    const spd = new Float32Array(count);
    const palette = Object.values(entityColors).slice(0, -1);
    for (let i = 0; i < count; i++) {
      pos[i * 3] = (Math.random() - 0.5) * 120;
      pos[i * 3 + 1] = (Math.random() - 0.5) * 80;
      pos[i * 3 + 2] = (Math.random() - 0.5) * 120;
      _colorObj.set(palette[Math.floor(Math.random() * palette.length)]);
      col[i * 3] = _colorObj.r; col[i * 3 + 1] = _colorObj.g; col[i * 3 + 2] = _colorObj.b;
      spd[i] = 0.1 + Math.random() * 0.3;
    }
    return [pos, col, spd] as const;
  }, [count]);
  const ref = useRef<THREE.Points>(null);
  useFrame((state) => {
    if (!ref.current) return;
    const t = state.clock.elapsedTime;
    const arr = ref.current.geometry.getAttribute('position') as THREE.BufferAttribute;
    const a = arr.array as Float32Array;
    for (let i = 0; i < count; i++) {
      a[i * 3 + 1] += Math.sin(t * speeds[i] + i) * 0.003;
      a[i * 3] += Math.cos(t * speeds[i] * 0.5 + i * 0.7) * 0.002;
    }
    arr.needsUpdate = true;
  });
  return (
    <points ref={ref}>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
        <bufferAttribute attach="attributes-color" args={[colors, 3]} />
      </bufferGeometry>
      <pointsMaterial vertexColors size={0.2} transparent opacity={0.35} sizeAttenuation depthWrite={false} blending={THREE.AdditiveBlending} />
    </points>
  );
});

// ═══════════════════════════════════════════════════════════════
// PULSE RINGS
// ═══════════════════════════════════════════════════════════════
function PulseRings({ position, color }: { position: [number, number, number]; color: string }) {
  const ring1Ref = useRef<THREE.Mesh>(null);
  const ring2Ref = useRef<THREE.Mesh>(null);
  const ring3Ref = useRef<THREE.Mesh>(null);
  useFrame((state) => {
    const t = state.clock.elapsedTime;
    [ring1Ref, ring2Ref, ring3Ref].forEach((ref, idx) => {
      if (!ref.current) return;
      const p = ((t * 0.15 + idx * 0.33) % 1);
      ref.current.scale.setScalar(2.0 + p * 5.0);
      (ref.current.material as THREE.MeshBasicMaterial).opacity = 0.2 * (1 - p);
    });
  });
  const mat = useMemo(() => new THREE.MeshBasicMaterial({
    color: new THREE.Color(color), transparent: true, opacity: 0,
    blending: THREE.AdditiveBlending, depthWrite: false, side: THREE.DoubleSide,
  }), [color]);
  // Create 3 separate materials for the 3 rings
  const mats = useMemo(() => [
    mat,
    mat.clone(),
    mat.clone(),
  ], [mat]);

  return (
    <group position={position}>
      {[ring1Ref, ring2Ref, ring3Ref].map((ref, i) => (
        <mesh key={i} ref={ref} rotation={[Math.PI / 2, 0, 0]} material={mats[i]}>
          <ringGeometry args={[0.95, 1.05, 64]} />
        </mesh>
      ))}
    </group>
  );
}

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
  }, [camera, gl, node.id, node.position, getPos, dragPlane, dragOffset, corePositions, dragRef]);

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

  if (satellites.length === 0) return null;
  // Set instance colors imperatively after mount
  useEffect(() => {
    if (!meshRef.current || satellites.length === 0) return;
    const mesh = meshRef.current;
    mesh.instanceColor = new THREE.InstancedBufferAttribute(instanceColors, 3);
    if (glowRef.current) {
      glowRef.current.instanceColor = new THREE.InstancedBufferAttribute(glowColors, 3);
    }
  }, [instanceColors, glowColors, satellites.length]);

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
const EDGE_TYPE_COLORS: Record<string, string> = {
  RelatedTo: '#63d8d2', Uses: '#ff8a5b', Implements: '#a99cf8',
  DependsOn: '#f472b6', PartOf: '#ddbb65', ConflictsWith: '#ef4444',
  default: '#6b7280',
};
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
      const hex = EDGE_TYPE_COLORS[e.relationshipType] || EDGE_TYPE_COLORS.default;
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
function ScreenProjectionTracker({
  nodes, timeRef, corePositions,
}: {
  nodes: GraphNodeData[]; timeRef: React.MutableRefObject<number>;
  corePositions: React.MutableRefObject<Map<string, THREE.Vector3>>;
}) {
  const { camera, size } = useThree();
  useFrame(() => {
    const t = timeRef.current;
    for (let i = 0; i < nodes.length; i++) {
      const n = nodes[i];
      const wp = resolveNodePosition(n, t, corePositions);
      _projVec.copy(wp).project(camera);
      screenPos.set(n.id, { x: (_projVec.x * 0.5 + 0.5) * size.width, y: (-_projVec.y * 0.5 + 0.5) * size.height, visible: _projVec.z < 1 });
    }
  });
  return null;
}

// ═══════════════════════════════════════════════════════════════
// BUILD CLUSTERS — each orphan is its own core, dynamic spread
// ═══════════════════════════════════════════════════════════════
function buildClusters(
  nodes: Array<{ id: string; entityType: string; title: string; description: string }>,
  edges: Array<{ sourceEntityId: string; targetEntityId: string; relationshipType: string; weight: number; id: string }>,
): { clusters: OrbitCluster[]; allNodes: GraphNodeData[]; uniqueEdges: GraphEdgeData[]; satellites: GraphNodeData[] } {
  const seenEdges = new Set<string>();
  const uniqueEdges: GraphEdgeData[] = edges.filter(e => {
    const k = `${e.sourceEntityId}→${e.targetEntityId}`;
    if (seenEdges.has(k)) return false; seenEdges.add(k); return true;
  }).map(e => ({ id: e.id, source: e.sourceEntityId, target: e.targetEntityId, relationshipType: e.relationshipType, weight: e.weight }));

  const adj = new Map<string, Set<string>>();
  nodes.forEach(n => adj.set(n.id, new Set()));
  uniqueEdges.forEach(e => { adj.get(e.source)?.add(e.target); adj.get(e.target)?.add(e.source); });

  const importance = new Map<string, number>();
  nodes.forEach(n => importance.set(n.id, adj.get(n.id)?.size || 0));

  const sorted = [...nodes].sort((a, b) => (importance.get(a.id) || 0) - (importance.get(b.id) || 0));
  const assigned = new Set<string>();
  const clusters: OrbitCluster[] = [];

  // Generate dynamic spread positions — golden angle spiral for even distribution
  function generateCenter(index: number): THREE.Vector3 {
    const totalNodes = nodes.length;
    const spacing = Math.max(30, Math.sqrt(totalNodes) * 8);
    // Golden angle spiral in XZ plane with Y variation
    const goldenAngle = Math.PI * (3 - Math.sqrt(5));
    const angle = index * goldenAngle;
    const radius = spacing * Math.sqrt(index + 1) * 0.4;
    return new THREE.Vector3(
      Math.cos(angle) * radius,
      (Math.sin(index * 1.7) * spacing * 0.15),
      Math.sin(angle) * radius,
    );
  }

  // Build main clusters from high-importance nodes
  let clusterIdx = 0;
  for (let i = sorted.length - 1; i >= 0 && clusterIdx < Math.min(nodes.length, Math.ceil(nodes.length / 2)); i--) {
    const core = sorted[i];
    if (assigned.has(core.id)) continue;
    const neighbors = adj.get(core.id) || new Set();
    const sats: GraphNodeData[] = [];
    const center = generateCenter(clusterIdx);
    let si = 0;
    neighbors.forEach((nid) => {
      if (!assigned.has(nid)) {
        const sn = nodes.find(n => n.id === nid);
        if (sn) {
          sats.push({
            id: sn.id, title: formatNodeName(sn.title, sn.entityType), entityType: sn.entityType, color: getColor(sn.entityType),
            position: center.clone(), orbitRadius: 4 + si * ORBIT_SPREAD,
            orbitSpeed: ORBIT_SPEED_MULT * (1 - si * 0.04),
            orbitOffset: (si * Math.PI * 2) / Math.max(neighbors.size, 1),
            coreId: core.id, size: 0.8 + (importance.get(sn.id) || 0) * 0.15,
            connectionCount: importance.get(sn.id) || 0, description: sn.description,
          });
          assigned.add(nid); si++;
        }
      }
    });
    clusters.push({ coreId: core.id, coreTitle: formatNodeName(core.title, core.entityType), coreColor: getColor(core.entityType), satellites: sats, center });
    assigned.add(core.id);
    clusterIdx++;
  }

  // Orphans: each gets its own position on the spiral
  let orphanIdx = 0;
  const orphans = nodes.filter(n => !assigned.has(n.id));
  orphans.forEach((n) => {
    const center = generateCenter(clusters.length + orphanIdx);
    clusters.push({
      coreId: n.id, coreTitle: formatNodeName(n.title, n.entityType), coreColor: getColor(n.entityType),
      satellites: [], center,
    });
    assigned.add(n.id); orphanIdx++;
  });

  const allNodes: GraphNodeData[] = [];
  const allSatellites: GraphNodeData[] = [];
  clusters.forEach(c => {
    const cn = nodes.find(n => n.id === c.coreId);
    if (cn) {
      allNodes.push({
        id: cn.id, title: formatNodeName(cn.title, cn.entityType), entityType: cn.entityType, color: getColor(cn.entityType),
        position: c.center.clone(), orbitRadius: 0, orbitSpeed: 0, orbitOffset: 0, coreId: null,
        size: 1.5 + (importance.get(cn.id) || 0) * 0.2, connectionCount: importance.get(cn.id) || 0,
        description: cn.description,
      });
    }
    allSatellites.push(...c.satellites);
    allNodes.push(...c.satellites);
  });
  return { clusters, allNodes, uniqueEdges, satellites: allSatellites };
}

// ═══════════════════════════════════════════════════════════════
// COSMIC SUN — glowing star with corona, rays, and light
// ═══════════════════════════════════════════════════════════════
const CosmicSun = memo(function CosmicSun({ position = [-80, 35, -90] as [number, number, number] }: { position?: [number, number, number] }) {
  const coreRef = useRef<THREE.Mesh>(null);
  const corona1Ref = useRef<THREE.Mesh>(null);
  const corona2Ref = useRef<THREE.Mesh>(null);
  const corona3Ref = useRef<THREE.Mesh>(null);
  const raysRef = useRef<THREE.Mesh>(null);
  const groupRef = useRef<THREE.Group>(null);

  // Animated pulsing materials
  const coreMat = useMemo(() => new THREE.MeshBasicMaterial({
    color: new THREE.Color('#fff4e0'),
    toneMapped: false,
  }), []);
  const coronaMat = useMemo(() => new THREE.MeshBasicMaterial({
    color: new THREE.Color('#ffaa44'),
    transparent: true,
    opacity: 0.18,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
    side: THREE.BackSide,
  }), []);
  const coronaMat2 = useMemo(() => new THREE.MeshBasicMaterial({
    color: new THREE.Color('#ff8833'),
    transparent: true,
    opacity: 0.08,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
    side: THREE.BackSide,
  }), []);
  const coronaMat3 = useMemo(() => new THREE.MeshBasicMaterial({
    color: new THREE.Color('#ffcc66'),
    transparent: true,
    opacity: 0.04,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
    side: THREE.BackSide,
  }), []);

  // Light rays geometry — 12 flat quads radiating outward
  const raysGeo = useMemo(() => {
    const count = 12;
    const positions = new Float32Array(count * 6 * 3); // 6 verts per ray (2 triangles)
    for (let i = 0; i < count; i++) {
      const angle = (i / count) * Math.PI * 2;
      const innerR = 2.2;
      const outerR = 8 + Math.random() * 6;
      const halfW = 0.15 + Math.random() * 0.15;
      const cos = Math.cos(angle);
      const sin = Math.sin(angle);
      const perpCos = Math.cos(angle + Math.PI / 2);
      const perpSin = Math.sin(angle + Math.PI / 2);
      // Two triangles per ray
      const base = i * 18;
      // Triangle 1
      positions[base] = cos * innerR + perpCos * halfW;
      positions[base + 1] = sin * innerR + perpSin * halfW;
      positions[base + 2] = 0;
      positions[base + 3] = cos * innerR - perpCos * halfW;
      positions[base + 4] = sin * innerR - perpSin * halfW;
      positions[base + 5] = 0;
      positions[base + 6] = cos * outerR;
      positions[base + 7] = sin * outerR;
      positions[base + 8] = 0;
      // Triangle 2
      positions[base + 9] = cos * outerR;
      positions[base + 10] = sin * outerR;
      positions[base + 11] = 0;
      positions[base + 12] = cos * innerR - perpCos * halfW;
      positions[base + 13] = sin * innerR - perpSin * halfW;
      positions[base + 14] = 0;
      positions[base + 15] = cos * outerR + perpCos * halfW * 0.3;
      positions[base + 16] = sin * outerR + perpSin * halfW * 0.3;
      positions[base + 17] = 0;
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    return geo;
  }, []);
  const raysMat = useMemo(() => new THREE.MeshBasicMaterial({
    color: new THREE.Color('#ffcc88'),
    transparent: true,
    opacity: 0.12,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
    side: THREE.DoubleSide,
  }), []);

  useFrame((state) => {
    const t = state.clock.elapsedTime;
    // Core pulse
    const corePulse = 1.0 + Math.sin(t * 1.5) * 0.05;
    if (coreRef.current) coreRef.current.scale.setScalar(corePulse);
    // Corona layers pulse independently
    if (corona1Ref.current) {
      corona1Ref.current.scale.setScalar(1.0 + Math.sin(t * 0.8) * 0.1);
      coronaMat.opacity = 0.14 + Math.sin(t * 1.2) * 0.06;
    }
    if (corona2Ref.current) {
      corona2Ref.current.scale.setScalar(1.0 + Math.sin(t * 0.6 + 1) * 0.12);
      coronaMat2.opacity = 0.06 + Math.sin(t * 0.9 + 0.5) * 0.04;
    }
    if (corona3Ref.current) {
      corona3Ref.current.scale.setScalar(1.0 + Math.sin(t * 0.4 + 2) * 0.15);
      coronaMat3.opacity = 0.03 + Math.sin(t * 0.7 + 1) * 0.02;
    }
    // Rays slow rotation + opacity pulse
    if (raysRef.current) {
      raysRef.current.rotation.z = t * 0.03;
      raysMat.opacity = 0.08 + Math.sin(t * 0.5) * 0.06;
    }
    // Gentle group wobble
    if (groupRef.current) {
      groupRef.current.rotation.y = Math.sin(t * 0.05) * 0.02;
    }
  });

  return (
    <group ref={groupRef} position={position}>
      {/* Core — bright emissive sphere */}
      <mesh ref={coreRef} material={coreMat}>
        <sphereGeometry args={[1.8, 32, 32]} />
      </mesh>
      {/* Corona layer 1 — tight glow */}
      <mesh ref={corona1Ref} material={coronaMat}>
        <sphereGeometry args={[2.8, 32, 32]} />
      </mesh>
      {/* Corona layer 2 — mid glow */}
      <mesh ref={corona2Ref} material={coronaMat2}>
        <sphereGeometry args={[4.5, 32, 32]} />
      </mesh>
      {/* Corona layer 3 — wide diffuse */}
      <mesh ref={corona3Ref} material={coronaMat3}>
        <sphereGeometry args={[7.0, 32, 32]} />
      </mesh>
      {/* Light rays — radiating quads */}
      <mesh ref={raysRef} geometry={raysGeo} material={raysMat} />
      {/* Point light from the sun */}
      <pointLight color="#ffcc88" intensity={15} distance={200} decay={1.5} />
      <pointLight color="#ff9944" intensity={5} distance={120} decay={2} />
    </group>
  );
});

// ═══════════════════════════════════════════════════════════════
// R3F SCENE — self-contained physics + drag
// ═══════════════════════════════════════════════════════════════
function Scene({
  clusters, allNodes, satellites, edges,
  onNodeClick, onNodeHover, onNodeUnhover, onNodeContextMenu,
  highlightedRef, highlightedIdsRef, filteredIds,
}: {
  clusters: OrbitCluster[]; allNodes: GraphNodeData[]; satellites: GraphNodeData[];
  edges: GraphEdgeData[];
  onNodeClick: (n: GraphNodeData) => void; onNodeHover: (n: GraphNodeData) => void;
  onNodeUnhover: () => void; onNodeContextMenu: (n: GraphNodeData, e: MouseEvent) => void;
  highlightedRef: React.MutableRefObject<string | null>;
  highlightedIdsRef: React.MutableRefObject<Set<string>>;
  filteredIds: Set<string> | null;
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

  const orbitControlsRef = useRef<any>(null);
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
      <EdgeLines edges={edges} nodeMap={nodeMap} timeRef={timeRef} corePositions={corePositions} />
      <DataFlowDots edges={edges} nodeMap={nodeMap} timeRef={timeRef} corePositions={corePositions} />
      <ScreenProjectionTracker nodes={allNodes} timeRef={timeRef} corePositions={corePositions} />
    </>
  );
}

// ═══════════════════════════════════════════════════════════════
// DOM: LABEL LAYER
// ═══════════════════════════════════════════════════════════════
const LabelLayer = memo(function LabelLayer({
  nodes, hoveredIdRef, highlightedIdsRef, filteredIds,
}: {
  nodes: GraphNodeData[]; hoveredIdRef: React.MutableRefObject<string | null>;
  highlightedIdsRef: React.MutableRefObject<Set<string>>; filteredIds: Set<string> | null;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const rafRef = useRef<number>(0);
  useEffect(() => {
    const update = () => {
      const c = containerRef.current;
      if (!c) { rafRef.current = requestAnimationFrame(update); return; }
      const ch = c.children;
      const isSearching = filteredIds !== null;
      const ids = highlightedIdsRef.current;
      const hasHighlight = ids.size > 0;
      for (let i = 0; i < nodes.length && i < ch.length; i++) {
        const n = nodes[i]; const el = ch[i] as HTMLElement; const sp = screenPos.get(n.id);
        const isH = hasHighlight ? ids.has(n.id) : hoveredIdRef.current === n.id;
        const isDimmed = hasHighlight && !isH;
        const isCore = n.orbitRadius === 0;
        const isF = isSearching && !filteredIds.has(n.id);
        if (!sp || !sp.visible || isF) { el.style.visibility = 'hidden'; }
        else {
          el.style.visibility = 'visible'; el.style.left = `${sp.x}px`; el.style.top = `${sp.y}px`;
          el.style.transform = `translate(-50%, -100%) scale(${isH ? 1.15 : 1})`;
          el.style.opacity = isDimmed ? '0.2' : (isCore ? '1' : (isH ? '1' : '0.55'));
          el.style.fontSize = isCore ? '12px' : '10px';
          el.style.fontWeight = isCore ? '700' : (isH ? '600' : '400');
          el.style.color = isH ? n.color : (isDimmed ? '#333' : (isCore ? '#e8edf3' : '#7888a0'));
        }
      }
      rafRef.current = requestAnimationFrame(update);
    };
    rafRef.current = requestAnimationFrame(update);
    return () => cancelAnimationFrame(rafRef.current);
  }, [nodes, hoveredIdRef, highlightedIdsRef, filteredIds]);

  return (
    <div ref={containerRef} style={{ position: 'absolute', inset: 0, pointerEvents: 'none', overflow: 'hidden' }}>
      {nodes.map(n => (
        <div key={n.id} style={{
          position: 'absolute', transform: 'translate(-50%, -100%)',
          fontFamily: 'system-ui, -apple-system, sans-serif',
          textShadow: '0 1px 4px rgba(0,0,0,0.9)', whiteSpace: 'nowrap',
          pointerEvents: 'none', userSelect: 'none', willChange: 'transform',
          transition: 'color 0.2s, opacity 0.2s, font-size 0.15s',
        }}>
          {n.title}
        </div>
      ))}
    </div>
  );
});

// ═══════════════════════════════════════════════════════════════
// DOM: CONTEXT MENU
// ═══════════════════════════════════════════════════════════════
function ContextMenu({
  data, onClose, onAskCopilot, onFocus, onHide, onViewMemory,
}: {
  data: { x: number; y: number; node: GraphNodeData } | null;
  onClose: () => void; onAskCopilot: (n: GraphNodeData) => void;
  onFocus: (n: GraphNodeData) => void; onHide: (id: string) => void;
  onViewMemory: (n: GraphNodeData) => void;
}) {
  useEffect(() => {
    if (!data) return;
    const h = () => onClose();
    document.addEventListener('click', h);
    document.addEventListener('contextmenu', h);
    return () => { document.removeEventListener('click', h); document.removeEventListener('contextmenu', h); };
  }, [data, onClose]);
  if (!data) return null;
  return (
    <div className="cosmic-context-menu" style={{ left: data.x, top: data.y }} onClick={e => e.stopPropagation()}>
      <div className="cosmic-context-header">
        <div className="cosmic-context-dot" style={{ background: data.node.color }} />
        <span>{data.node.title}</span>
      </div>
      <button onClick={() => { onAskCopilot(data.node); onClose(); }}><Brain size={14} /> Ask Copilot</button>
      <button onClick={() => { onViewMemory(data.node); onClose(); }}><MessageCircle size={14} /> View memories</button>
      <button onClick={() => { onFocus(data.node); onClose(); }}><Focus size={14} /> Focus cluster</button>
      <div className="cosmic-context-separator" />
      <button onClick={() => { onHide(data.node.id); onClose(); }}><Eye size={14} /> Hide node</button>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════
// DOM: SEARCH BAR with autocomplete
// ═══════════════════════════════════════════════════════════════
function SearchBar({
  value, onChange, onClear, suggestions, onSelectSuggestion, allEntities,
}: {
  value: string; onChange: (v: string) => void; onClear: () => void;
  suggestions: SearchSuggestion[]; onSelectSuggestion: (s: SearchSuggestion) => void;
  allEntities: SearchSuggestion[];
}) {
  const [focused, setFocused] = useState(false);
  const [activeIdx, setActiveIdx] = useState(-1);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const showDropdown = focused;
  // When empty query: show all entities grouped by type. When typing: show filtered suggestions
  const displayItems = value.length >= 2 ? suggestions : allEntities;
  // Flat list for keyboard nav
  const flatItems = useMemo(() => {
    if (value.length >= 2) return displayItems;
    const flat: SearchSuggestion[] = [];
    displayItems.forEach(s => flat.push(s));
    return flat;
  }, [displayItems, value]);
  // Group by type for empty query
  const grouped = useMemo(() => {
    if (value.length >= 2) return null;
    const groups = new Map<string, SearchSuggestion[]>();
    displayItems.forEach(s => {
      const key = s.kind === 'memory' ? 'Memories' : s.type;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(s);
    });
    return groups;
  }, [displayItems, value]);

  // Reset active index when query changes
  useEffect(() => { setActiveIdx(-1); }, [value]);

  // Scroll active item into view
  useEffect(() => {
    if (activeIdx < 0 || !dropdownRef.current) return;
    const item = dropdownRef.current.querySelector('.cosmic-search-item-active');
    if (item) item.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }, [activeIdx]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!showDropdown || flatItems.length === 0) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIdx(prev => (prev + 1) % flatItems.length);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIdx(prev => (prev - 1 + flatItems.length) % flatItems.length);
    } else if (e.key === 'Enter' && activeIdx >= 0 && activeIdx < flatItems.length) {
      e.preventDefault();
      onSelectSuggestion(flatItems[activeIdx]);
      setActiveIdx(-1);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      setFocused(false);
      setActiveIdx(-1);
    }
  }, [showDropdown, flatItems, activeIdx, onSelectSuggestion]);

  return (
    <div className="cosmic-search">
      <Search size={13} className="cosmic-search-icon" />
      <input type="text" placeholder="Search..." value={value}
        onChange={e => onChange(e.target.value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setTimeout(() => { setFocused(false); setActiveIdx(-1); }, 200)}
        onKeyDown={handleKeyDown}
        className="cosmic-search-input" />
      {value && <button className="cosmic-search-clear" onClick={onClear}><X size={11} /></button>}
      {showDropdown && (
        <div className="cosmic-search-dropdown" ref={dropdownRef}>
          {value.length < 2 && grouped ? (
            // Show grouped categories when no query
            (() => {
              let globalIdx = 0;
              return Array.from(grouped.entries()).map(([category, items]) => (
                <div key={category}>
                  <div className="cosmic-search-category">{category}</div>
                  {items.slice(0, 4).map(s => {
                    const idx = globalIdx++;
                    return <SearchSuggestionItem key={s.id} s={s} onSelect={onSelectSuggestion} isActive={idx === activeIdx} />;
                  })}
                </div>
              ));
            })()
          ) : (
            // Show filtered results when typing
            displayItems.map((s, idx) => <SearchSuggestionItem key={s.id} s={s} onSelect={onSelectSuggestion} isActive={idx === activeIdx} />)
          )}
          {value.length >= 2 && displayItems.length === 0 && (
            <div className="cosmic-search-empty">No results for "{value}"</div>
          )}
        </div>
      )}
    </div>
  );
}

function SearchSuggestionItem({ s, onSelect, isActive }: { s: SearchSuggestion; onSelect: (s: SearchSuggestion) => void; isActive?: boolean }) {
  const entityIcons: Record<string, string> = {
    person: '👤', project: '📁', decision: '⚖️', task: '✓',
    technology: '🔧', file: '📄', organization: '🏢', meeting: '📅',
    concept: '💡', document: '📝', default: '●',
  };
  const icon = entityIcons[s.type.toLowerCase()] || entityIcons.default;
  const descSnippet = s.description ? s.description.slice(0, 60) + (s.description.length > 60 ? '…' : '') : '';
  return (
    <button className={`cosmic-search-item${isActive ? ' cosmic-search-item-active' : ''}`}
      onMouseDown={(e) => {
        e.preventDefault(); // prevent input blur from killing dropdown
        e.stopPropagation();
        onSelect(s); // fire directly in mousedown — safe across all browsers/webview
      }}>
      <span className="cosmic-search-item-icon" style={{ color: s.color }}>{icon}</span>
      <div className="cosmic-search-item-text">
        <div className="cosmic-search-item-title">{s.title}</div>
        <div className="cosmic-search-item-type" style={{ color: s.color }}>
          {s.type}{s.connectionCount != null ? ` · ${s.connectionCount} conn` : ''}
        </div>
        {descSnippet && <div className="cosmic-search-item-desc">{descSnippet}</div>}
      </div>
      <span className="cosmic-search-item-badge" style={{ color: s.color, background: `${s.color}15` }}>
        {s.kind === 'entity' ? 'Entity' : 'Memory'}
      </span>
    </button>
  );
}

// ═══════════════════════════════════════════════════════════════
// DOM: INFO PANEL — rich display with backend data
// ═══════════════════════════════════════════════════════════════
const InfoPanel = memo(function InfoPanel({
  node, onClose, onAskCopilot, relatedMemoryCount, relatedMemories,
  connectedNodes, allEdges, onSelectNode, onViewMemory,
}: {
  node: GraphNodeData | null; onClose: () => void;
  onAskCopilot: (n: GraphNodeData) => void; relatedMemoryCount: number;
  relatedMemories: Array<{ id: string; title: string; summary: string; confidenceScore: number; importanceScore: number }>;
  connectedNodes: GraphNodeData[]; allEdges: GraphEdgeData[];
  onSelectNode: (n: GraphNodeData) => void; onViewMemory: (id: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  if (!node) return null;
  const nodeEdges = allEdges.filter(e => e.source === node.id || e.target === node.id);
  const entityIcons: Record<string, string> = {
    person: '👤', project: '📁', decision: '⚖️', task: '✓',
    technology: '🔧', file: '📄', organization: '🏢', meeting: '📅',
    concept: '💡', document: '📝', default: '●',
  };
  const icon = entityIcons[node.entityType.toLowerCase()] || entityIcons.default;
  return (
    <div className="cosmic-info-panel" style={{ animation: 'slideInRight 0.25s ease-out' }}>
      <div className="cosmic-info-header">
        <div className="cosmic-info-icon" style={{ background: `${node.color}22`, color: node.color, boxShadow: `0 0 16px ${node.color}33`, fontSize: 18 }}>
          {icon}
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="cosmic-info-title">{node.title}</div>
          <div className="cosmic-info-type" style={{ color: node.color }}>{node.entityType}</div>
        </div>
        <button className="cosmic-info-close" onClick={onClose}>×</button>
      </div>
      <div className="cosmic-info-body" style={{ maxHeight: expanded ? 500 : 280, overflowY: 'auto', transition: 'max-height 0.3s ease' }}>
        {node.description && (
          <div className="cosmic-info-desc">{node.description}</div>
        )}
        <div className="cosmic-info-stats">
          <div className="cosmic-info-stat"><Link2 size={12} /><span>{node.connectionCount} connections</span></div>
          {relatedMemoryCount > 0 && <div className="cosmic-info-stat"><Brain size={12} /><span>{relatedMemoryCount} related memories</span></div>}
        </div>

        {/* Connected entities */}
        {nodeEdges.length > 0 && (
          <div className="cosmic-info-section">
            <div className="cosmic-info-section-title"><Link2 size={10} /> Connections</div>
            {nodeEdges.slice(0, expanded ? 12 : 5).map(e => {
              const otherId = e.source === node.id ? e.target : e.source;
              const other = connectedNodes.find(n => n.id === otherId);
              if (!other) return null;
              const relColors: Record<string, string> = {
                RelatedTo: '#63d8d2', Uses: '#ff8a5b', Implements: '#a99cf8',
                DependsOn: '#f472b6', PartOf: '#ddbb65', ConflictsWith: '#ef4444',
              };
              const relColor = relColors[e.relationshipType] || '#6b7280';
              return (
                <div key={e.id} className="cosmic-info-connection" onClick={() => onSelectNode(other)}
                  style={{ cursor: 'pointer' }}>
                  <div style={{ width: 6, height: 6, borderRadius: '50%', background: other.color, flexShrink: 0 }} />
                  <span className="cosmic-info-connection-name">{other.title}</span>
                  <span className="cosmic-info-connection-rel" style={{ color: relColor, background: `${relColor}15` }}>{e.relationshipType}</span>
                </div>
              );
            })}
          </div>
        )}

        {/* Related memories */}
        {relatedMemories.length > 0 && (
          <div className="cosmic-info-section">
            <div className="cosmic-info-section-title"><Brain size={10} /> Related Memories</div>
            {relatedMemories.slice(0, expanded ? 6 : 3).map(m => (
              <div key={m.id} className="cosmic-info-memory-card" onClick={() => onViewMemory(m.id)}
                style={{ cursor: 'pointer' }}>
                <div className="cosmic-info-memory-title">{m.title}</div>
                <div className="cosmic-info-memory-summary">
                  {m.summary?.slice(0, 80)}{m.summary?.length > 80 ? '…' : ''}
                </div>
                <div className="cosmic-info-memory-bars">
                  <div className="cosmic-info-score-bar">
                    <span className="cosmic-info-score-label">Conf</span>
                    <div className="cosmic-info-score-track">
                      <div className="cosmic-info-score-fill" style={{ width: `${Math.round(m.confidenceScore * 100)}%`, background: '#63d8d2' }} />
                    </div>
                    <span className="cosmic-info-score-value">{Math.round(m.confidenceScore * 100)}%</span>
                  </div>
                  <div className="cosmic-info-score-bar">
                    <span className="cosmic-info-score-label">Imp</span>
                    <div className="cosmic-info-score-track">
                      <div className="cosmic-info-score-fill" style={{ width: `${Math.round(m.importanceScore * 100)}%`, background: '#ddbb65' }} />
                    </div>
                    <span className="cosmic-info-score-value">{Math.round(m.importanceScore * 100)}%</span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}

        <button onClick={() => setExpanded(!expanded)} className="cosmic-info-expand">
          {expanded ? 'Show less' : `Show more`} <ChevronRight size={10} style={{ transform: expanded ? 'rotate(90deg)' : 'none', transition: 'transform 0.2s' }} />
        </button>

        <button className="cosmic-info-copilot-btn" onClick={() => onAskCopilot(node)}>
          <Sparkles size={14} /> Ask Copilot
        </button>
      </div>
    </div>
  );
});

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
          filteredIds={filteredIds} />
      </Canvas>

      <LabelLayer nodes={allNodes} hoveredIdRef={hoveredIdRef}
        highlightedIdsRef={highlightedIdsRef} filteredIds={filteredIds} />
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
