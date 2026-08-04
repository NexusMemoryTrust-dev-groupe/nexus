import { useEffect, useRef, useState } from 'react';

/**
 * Motion-aware primitives.
 *
 * These live apart from the components that use them for two reasons: Fast
 * Refresh only reloads a module that exports components exclusively, and the
 * animated numerals on the Context page need the counter without pulling in the
 * whole instrument set.
 */

const QUERY = '(prefers-reduced-motion: reduce)';

/**
 * Live subscription to the OS motion setting.
 *
 * Reading once at mount would cover almost every case, but the listener costs
 * nothing and means toggling the system setting takes effect without an app
 * restart.
 */
export function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(
    () => typeof window !== 'undefined' && window.matchMedia(QUERY).matches,
  );

  useEffect(() => {
    const mq = window.matchMedia(QUERY);
    const onChange = (e: MediaQueryListEvent) => setReduced(e.matches);
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, []);

  return reduced;
}

/**
 * Count from zero up to `target` on mount.
 *
 * Driven by rAF rather than a CSS transition because the animated thing is
 * text. The easing matters: `easeOutCubic` makes the figure land instead of
 * crawling to a stop. Cancels on unmount so switching views mid-animation
 * cannot write into a dead component, and skips straight to the value when the
 * user has asked for less motion.
 */
export function useCountUp(target: number, ms = 780): number {
  const reduced = useReducedMotion();
  const [value, setValue] = useState(reduced ? target : 0);
  const frame = useRef(0);

  useEffect(() => {
    if (reduced) {
      setValue(target);
      return;
    }

    const start = performance.now();

    const tick = (now: number) => {
      const p = Math.min((now - start) / ms, 1);
      const eased = 1 - Math.pow(1 - p, 3);
      setValue(Math.round(target * eased));
      if (p < 1) frame.current = requestAnimationFrame(tick);
    };

    frame.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame.current);
  }, [target, ms, reduced]);

  return value;
}
