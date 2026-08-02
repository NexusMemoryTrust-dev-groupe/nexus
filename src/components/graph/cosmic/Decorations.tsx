import { useRef, useMemo, memo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';

import { entityColors, _colorObj } from './constants';

/**
 * Purely decorative scene layers: the starfield backdrop, drifting dust, and
 * the pulse rings that mark a highlighted node.
 *
 * Extracted from the 81 KB single-file view because none of this participates in
 * graph logic — it never reads a node, an edge, or a score. Keeping it next to
 * the physics and interaction code made both harder to reason about, and made
 * the file too large to review as a unit.
 */

// ── Starfield ───────────────────────────────────────────────────────────────

/**
 * Static star backdrop.
 *
 * One `points` draw call with a fixed buffer: the positions are generated once
 * and never touched again, so the per-frame cost is a single rotation on the
 * transform. Rotating the object rather than rewriting vertices is what keeps
 * 600 stars free.
 */
export const Starfield = memo(function Starfield({ count = 600 }: { count?: number }) {
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
  useFrame((_, dt) => {
    if (ref.current) ref.current.rotation.y += dt * 0.001;
  });

  return (
    <points ref={ref}>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
      </bufferGeometry>
      <pointsMaterial
        color="#ffffff"
        size={0.35}
        transparent
        opacity={0.4}
        sizeAttenuation
        depthWrite={false}
      />
    </points>
  );
});

// ── Ambient dust ────────────────────────────────────────────────────────────

/**
 * Slow-drifting coloured motes, tinted from the entity palette so the
 * background echoes the graph's own colours.
 *
 * Unlike the starfield this *does* rewrite vertex positions each frame, which is
 * why the count is deliberately an order of magnitude lower.
 */
export const AmbientDust = memo(function AmbientDust({ count = 180 }: { count?: number }) {
  const [positions, colors, speeds] = useMemo(() => {
    const pos = new Float32Array(count * 3);
    const col = new Float32Array(count * 3);
    const spd = new Float32Array(count);
    // `slice(0, -1)` drops the `default` entry: it is a fallback colour, not
    // part of the visual identity, and including it muddies the palette.
    const palette = Object.values(entityColors).slice(0, -1);

    for (let i = 0; i < count; i++) {
      pos[i * 3] = (Math.random() - 0.5) * 120;
      pos[i * 3 + 1] = (Math.random() - 0.5) * 80;
      pos[i * 3 + 2] = (Math.random() - 0.5) * 120;
      _colorObj.set(palette[Math.floor(Math.random() * palette.length)]);
      col[i * 3] = _colorObj.r;
      col[i * 3 + 1] = _colorObj.g;
      col[i * 3 + 2] = _colorObj.b;
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
      <pointsMaterial
        vertexColors
        size={0.2}
        transparent
        opacity={0.35}
        sizeAttenuation
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </points>
  );
});

// ── Pulse rings ─────────────────────────────────────────────────────────────

/**
 * Three expanding rings that fade as they grow — the "this node is active"
 * signal.
 *
 * Each ring needs its own material because opacity is animated per ring; a
 * shared material would make all three fade in lockstep and lose the ripple.
 */
export function PulseRings({
  position,
  color,
}: {
  position: [number, number, number];
  color: string;
}) {
  const ring1Ref = useRef<THREE.Mesh>(null);
  const ring2Ref = useRef<THREE.Mesh>(null);
  const ring3Ref = useRef<THREE.Mesh>(null);

  useFrame((state) => {
    const t = state.clock.elapsedTime;
    [ring1Ref, ring2Ref, ring3Ref].forEach((ref, idx) => {
      if (!ref.current) return;
      // Offsetting each ring by a third of the cycle produces the ripple.
      const p = (t * 0.15 + idx * 0.33) % 1;
      ref.current.scale.setScalar(2.0 + p * 5.0);
      (ref.current.material as THREE.MeshBasicMaterial).opacity = 0.2 * (1 - p);
    });
  });

  const mats = useMemo(() => {
    const base = new THREE.MeshBasicMaterial({
      color: new THREE.Color(color),
      transparent: true,
      opacity: 0,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
      side: THREE.DoubleSide,
    });
    return [base, base.clone(), base.clone()];
  }, [color]);

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
