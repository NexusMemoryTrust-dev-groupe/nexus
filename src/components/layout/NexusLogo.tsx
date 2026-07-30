/**
 * Animated Nexus logo — shrunk from the designer's 500×500 SVG.
 * All rotations / pulses are preserved; only the container scales down.
 * Unique ID prefix prevents collisions when multiple instances exist.
 */
let _nextId = 0;

export function NexusLogo({ size = 40 }: { size?: number }) {
  const uid = `nx${_nextId++}`;

  return (
    <svg
      viewBox="0 0 500 500"
      width={size}
      height={size}
      className="nexus-logo-svg"
      xmlns="http://www.w3.org/2000/svg"
      style={{ overflow: 'visible' }}
    >
      <defs>
        <linearGradient id={`${uid}-primaryGrad`} x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#FFB380" />
          <stop offset="50%" stopColor="#FF6B35" />
          <stop offset="100%" stopColor="#C2360E" />
        </linearGradient>
        <linearGradient id={`${uid}-fadeGrad`} x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stopColor="#FF9853" stopOpacity="0.8" />
          <stop offset="100%" stopColor="#E04616" stopOpacity="0" />
        </linearGradient>
        <filter id={`${uid}-glow`} x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="3" result="b1" />
          <feGaussianBlur stdDeviation="8" result="b2" />
          <feMerge>
            <feMergeNode in="b2" />
            <feMergeNode in="b1" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
        <filter id={`${uid}-core`} x="-100%" y="-100%" width="300%" height="300%">
          <feGaussianBlur stdDeviation="4" result="b1" />
          <feGaussianBlur stdDeviation="10" result="b2" />
          <feGaussianBlur stdDeviation="20" result="b3" />
          <feMerge>
            <feMergeNode in="b3" />
            <feMergeNode in="b2" />
            <feMergeNode in="b1" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      {/* ── Outer ring ── */}
      <circle cx="250" cy="250" r="240" fill="none" stroke="#E04616" strokeWidth="2" opacity="0.4" />

      {/* ── Tick marks (slow rotate 120s) ── */}
      <g opacity="0.4">
        <animateTransform
          attributeName="transform"
          type="rotate"
          from="0 250 250"
          to="360 250 250"
          dur="120s"
          repeatCount="indefinite"
        />
        {Array.from({ length: 12 }, (_, i) => i * 30).map((deg) => (
          <line
            key={`t${deg}`}
            x1="250" y1="10" x2="250" y2="24"
            stroke="#FF9853"
            strokeWidth="3"
            transform={`rotate(${deg} 250 250)`}
          />
        ))}
        {Array.from({ length: 48 }, (_, i) => i * 7.5)
          .filter((d) => d % 30 !== 0)
          .map((deg) => (
            <line
              key={`s${deg}`}
              x1="250" y1="10" x2="250" y2="18"
              stroke="#FF6B35"
              strokeWidth="1.5"
              transform={`rotate(${deg} 250 250)`}
            />
          ))}
      </g>

      {/* ── Arc dashes (counter-rotate 45s) ── */}
      <g opacity="0.6">
        <animateTransform
          attributeName="transform"
          type="rotate"
          from="360 250 250"
          to="0 250 250"
          dur="45s"
          repeatCount="indefinite"
        />
        {Array.from({ length: 12 }, (_, i) => i * 30).map((deg) => (
          <path
            key={`a${deg}`}
            d="M 250 30 A 220 220 0 0 1 350 48"
            fill="none"
            stroke={`url(#${uid}-primaryGrad)`}
            strokeWidth="4"
            strokeDasharray="40 80"
            transform={`rotate(${deg} 250 250)`}
          />
        ))}
        {[15, 75, 135, 195, 255, 315].map((deg) => (
          <circle
            key={`d${deg}`}
            cx="250" cy="30" r="5"
            fill="#FFFFFF"
            filter={`url(#${uid}-glow)`}
            transform={`rotate(${deg} 250 250)`}
          />
        ))}
      </g>

      {/* ── Rotating tech text (200s) ── */}
      <g opacity="0.3" fill="#FF9853" fontSize="6" fontFamily="monospace" letterSpacing="2">
        <animateTransform
          attributeName="transform"
          type="rotate"
          from="0 250 250"
          to="-360 250 250"
          dur="200s"
          repeatCount="indefinite"
        />
        <text x="235" y="45" transform="rotate(0 250 250)">SYS.MEM.0X4A</text>
        <text x="235" y="45" transform="rotate(90 250 250)">NXS.CPL.V9.1</text>
        <text x="235" y="45" transform="rotate(180 250 250)">INTEL.SYNC.OK</text>
        <text x="235" y="45" transform="rotate(270 250 250)">NEURAL.CAP.MAX</text>
      </g>

      {/* ── Triangles + hexagons (90s) ── */}
      <g opacity="0.4">
        <animateTransform
          attributeName="transform"
          type="rotate"
          from="0 250 250"
          to="360 250 250"
          dur="90s"
          repeatCount="indefinite"
        />
        {[0, 30, 60, 90].map((deg) => (
          <polygon
            key={`tri${deg}`}
            points="250,70 405,340 95,340"
            fill="none"
            stroke="#FF6B35"
            strokeWidth="2.5"
            transform={`rotate(${deg} 250 250)`}
          />
        ))}
        {[0, 45].map((deg) => (
          <polygon
            key={`hex${deg}`}
            points="250,110 371,180 371,320 250,390 129,320 129,180"
            fill="none"
            stroke="#FF9853"
            strokeWidth="2"
            strokeDasharray="6 10"
            transform={`rotate(${deg} 250 250)`}
          />
        ))}
      </g>

      {/* ── Tendrils from center (60s reverse) ── */}
      <g>
        <animateTransform
          attributeName="transform"
          type="rotate"
          from="360 250 250"
          to="0 250 250"
          dur="60s"
          repeatCount="indefinite"
        />
        {Array.from({ length: 24 }, (_, i) => i * 15).map((deg) => {
          const bright = deg % 60 === 0;
          return (
            <g key={`te${deg}`} transform={`rotate(${deg} 250 250)`}>
              <path
                d="M 250 250 C 350 180, 200 100, 250 70"
                fill="none"
                stroke={`url(#${uid}-fadeGrad)`}
                strokeWidth="3"
                opacity={bright ? 0.8 : 0.3}
              />
              <path
                d="M 250 250 Q 150 180, 250 110"
                fill="none"
                stroke="#FF6B35"
                strokeWidth="1.5"
                opacity="0.5"
              />
              {bright && (
                <>
                  <circle cx="250" cy="70" r="4" fill="#FFFFFF" filter={`url(#${uid}-glow)`} />
                  {deg % 120 === 0 && (
                    <circle cx="250" cy="110" r="3" fill="#FF9853" />
                  )}
                </>
              )}
            </g>
          );
        })}
      </g>

      {/* ── Inner spinning ring (20s — fastest) ── */}
      <g>
        <animateTransform
          attributeName="transform"
          type="rotate"
          from="0 250 250"
          to="-360 250 250"
          dur="20s"
          repeatCount="indefinite"
        />
        <circle cx="250" cy="250" r="50" fill="none" stroke="#E04616" strokeWidth="6" strokeDasharray="3 14" opacity="0.8" />
        <circle cx="250" cy="250" r="45" fill="none" stroke="#FF9853" strokeWidth="2" strokeDasharray="25 50" opacity="0.9" />
        {[0, 45, 90, 135, 180, 225, 270, 315].map((deg) => (
          <rect
            key={`bar${deg}`}
            x="247" y="183" width="6" height="16" rx="3"
            fill="#FF6B35"
            transform={`rotate(${deg} 250 250)`}
          />
        ))}
      </g>

      {/* ── Pulsing core + diamond ── */}
      <g>
        <circle cx="250" cy="250" r="24" fill={`url(#${uid}-primaryGrad)`} filter={`url(#${uid}-core)`}>
          <animate attributeName="r" values="24;30;24" dur="3s" repeatCount="indefinite" />
          <animate attributeName="opacity" values="0.8;1;0.8" dur="3s" repeatCount="indefinite" />
        </circle>
        <g filter={`url(#${uid}-glow)`}>
          <animateTransform
            attributeName="transform"
            type="rotate"
            from="0 250 250"
            to="360 250 250"
            dur="10s"
            repeatCount="indefinite"
          />
          <polygon points="250,218 274,250 250,282 226,250" fill="#FFFFFF" opacity="0.9" />
          <polygon points="250,228 264,250 250,272 236,250" fill="#FFB380" />
        </g>
        <circle cx="250" cy="250" r="5" fill="#FFFFFF" filter={`url(#${uid}-core)`} />
      </g>

      {/* ── Corner brackets ── */}
      <g filter={`url(#${uid}-glow)`}>
        <path d="M 30 55 L 30 30 L 55 30" fill="none" stroke="#FF6B35" strokeWidth="5" opacity="0.7" />
        <path d="M 470 55 L 470 30 L 445 30" fill="none" stroke="#FF6B35" strokeWidth="5" opacity="0.7" />
        <path d="M 30 445 L 30 470 L 55 470" fill="none" stroke="#FF6B35" strokeWidth="5" opacity="0.7" />
        <path d="M 470 445 L 470 470 L 445 470" fill="none" stroke="#FF6B35" strokeWidth="5" opacity="0.7" />
      </g>
    </svg>
  );
}
