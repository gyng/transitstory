// Rolling-number tween for the headline counters/gauge — the "reacting gauge" juice AGENTS lists.
// It runs on its OWN short-lived rAF and writes textContent through a ref, so it causes ZERO React
// re-renders during the tween (no per-frame React churn) and is fully decoupled from the sim/deck
// render loop. Each new target (arriving on the ~3 Hz stats slice) re-aims an ease-out toward it.
// Honours prefers-reduced-motion (snaps), and snaps under the e2e hook so tests read exact values.
import { useEffect, useRef } from "react";

export function useTweenedNumber(
  target: number,
  fmt: (n: number) => string,
  ms = 480,
): React.RefObject<HTMLElement | null> {
  const ref = useRef<HTMLElement | null>(null);
  const cur = useRef(target);
  const raf = useRef(0);

  useEffect(() => {
    const node = ref.current;
    if (node) node.textContent = fmt(cur.current); // ensure content exists on first paint
    const reduce =
      typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const testing = "__ot_test" in window; // deterministic exact values for e2e
    if (reduce || testing || ms <= 0 || cur.current === target) {
      cur.current = target;
      if (node) node.textContent = fmt(target);
      return;
    }
    const from = cur.current;
    const start = performance.now();
    const tick = (now: number): void => {
      const t = Math.min(1, (now - start) / ms);
      const k = 1 - (1 - t) * (1 - t) * (1 - t); // ease-out cubic
      cur.current = from + (target - from) * k;
      if (ref.current) ref.current.textContent = fmt(cur.current);
      if (t < 1) raf.current = requestAnimationFrame(tick);
      else cur.current = target;
    };
    raf.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf.current);
  }, [target, ms, fmt]);

  return ref;
}
