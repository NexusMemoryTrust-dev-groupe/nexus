import { useRef, useMemo, memo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';

/**
 * The background star: emissive core, three corona shells, radiating light
 * rays, and the two point lights that actually illuminate the graph.
 *
 * Self-contained — it takes a position and nothing else, and reads no graph
 * state. That is precisely why it belongs in its own module rather than sitting
 * in the middle of the view's interaction code.
 */
export const CosmicSun = memo(function CosmicSun({
  position = [-80, 35, -90] as [number, number, number],
}: {
  position?: [number, number, number];
}) {
  const coreRef = useRef<THREE.Mesh>(null);
  const corona1Ref = useRef<THREE.Mesh>(null);
  const corona2Ref = useRef<THREE.Mesh>(null);
  const corona3Ref = useRef<THREE.Mesh>(null);
  const raysRef = useRef<THREE.Mesh>(null);
  const groupRef = useRef<THREE.Group>(null);

  // Materials are created once and mutated in the frame loop. Rebuilding them
  // per frame would recompile shaders and stall the render.
  const coreMat = useMemo(
    () => new THREE.MeshBasicMaterial({ color: new THREE.Color('#fff4e0'), toneMapped: false }),
    [],
  );

  /** Corona shells share a shape and differ only in size, colour and opacity. */
  const coronaMat = useMemo(
    () =>
      new THREE.MeshBasicMaterial({
        color: new THREE.Color('#ffaa44'),
        transparent: true,
        opacity: 0.18,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
        // BackSide: rendering the inside of the sphere gives a limb-glow that
        // reads as atmosphere rather than as a solid ball.
        side: THREE.BackSide,
      }),
    [],
  );
  const coronaMat2 = useMemo(
    () =>
      new THREE.MeshBasicMaterial({
        color: new THREE.Color('#ff8833'),
        transparent: true,
        opacity: 0.08,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
        side: THREE.BackSide,
      }),
    [],
  );
  const coronaMat3 = useMemo(
    () =>
      new THREE.MeshBasicMaterial({
        color: new THREE.Color('#ffcc66'),
        transparent: true,
        opacity: 0.04,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
        side: THREE.BackSide,
      }),
    [],
  );

  /**
   * Twelve flat quads radiating outward, built as raw triangles.
   *
   * Randomised length and width per ray so the corona does not look like a
   * mechanical gear. Generated once: the animation only rotates the mesh.
   */
  const raysGeo = useMemo(() => {
    const count = 12;
    const positions = new Float32Array(count * 6 * 3); // 6 verts = 2 triangles
    for (let i = 0; i < count; i++) {
      const angle = (i / count) * Math.PI * 2;
      const innerR = 2.2;
      const outerR = 8 + Math.random() * 6;
      const halfW = 0.15 + Math.random() * 0.15;
      const cos = Math.cos(angle);
      const sin = Math.sin(angle);
      const perpCos = Math.cos(angle + Math.PI / 2);
      const perpSin = Math.sin(angle + Math.PI / 2);
      const base = i * 18;

      // Triangle 1: inner edge to outer tip.
      positions[base] = cos * innerR + perpCos * halfW;
      positions[base + 1] = sin * innerR + perpSin * halfW;
      positions[base + 2] = 0;
      positions[base + 3] = cos * innerR - perpCos * halfW;
      positions[base + 4] = sin * innerR - perpSin * halfW;
      positions[base + 5] = 0;
      positions[base + 6] = cos * outerR;
      positions[base + 7] = sin * outerR;
      positions[base + 8] = 0;

      // Triangle 2: closes the quad, tapering toward the tip.
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

  const raysMat = useMemo(
    () =>
      new THREE.MeshBasicMaterial({
        color: new THREE.Color('#ffcc88'),
        transparent: true,
        opacity: 0.12,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
        side: THREE.DoubleSide,
      }),
    [],
  );

  useFrame((state) => {
    const t = state.clock.elapsedTime;

    // Each layer breathes at its own rate; synchronised pulses would look
    // mechanical instead of alive.
    if (coreRef.current) coreRef.current.scale.setScalar(1.0 + Math.sin(t * 1.5) * 0.05);

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
    if (raysRef.current) {
      raysRef.current.rotation.z = t * 0.03;
      raysMat.opacity = 0.08 + Math.sin(t * 0.5) * 0.06;
    }
    if (groupRef.current) {
      groupRef.current.rotation.y = Math.sin(t * 0.05) * 0.02;
    }
  });

  return (
    <group ref={groupRef} position={position}>
      <mesh ref={coreRef} material={coreMat}>
        <sphereGeometry args={[1.8, 32, 32]} />
      </mesh>
      <mesh ref={corona1Ref} material={coronaMat}>
        <sphereGeometry args={[2.8, 32, 32]} />
      </mesh>
      <mesh ref={corona2Ref} material={coronaMat2}>
        <sphereGeometry args={[4.5, 32, 32]} />
      </mesh>
      <mesh ref={corona3Ref} material={coronaMat3}>
        <sphereGeometry args={[7.0, 32, 32]} />
      </mesh>
      <mesh ref={raysRef} geometry={raysGeo} material={raysMat} />
      {/* Two lights: a wide warm fill and a tighter hotter core, so nodes near
          the sun pick up a visible falloff instead of flat illumination. */}
      <pointLight color="#ffcc88" intensity={15} distance={200} decay={1.5} />
      <pointLight color="#ff9944" intensity={5} distance={120} decay={2} />
    </group>
  );
});
