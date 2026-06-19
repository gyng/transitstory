// #10 Modal-dialog focus management — the a11y machinery the StatsDashboard + Settings modals were missing:
// focus moves INTO the panel on open, a Tab/Shift+Tab wrap traps focus inside it (so a keyboard user can't tab
// behind the scrim into live chrome), Escape closes it, and focus is RESTORED to the trigger on close. Pair the
// returned ref with role="dialog" aria-modal="true" aria-labelledby on the panel. React owns DOM chrome; this is
// pure DOM focus wiring (no sim, no render loop).
import { useEffect, useRef } from "react";

export function useDialog(open: boolean, onClose: () => void): React.RefObject<HTMLDivElement | null> {
  const ref = useRef<HTMLDivElement>(null);
  // Keep onClose current without re-running the effect each render (callers pass a fresh arrow) — else the
  // panel would re-focus on every render.
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useEffect(() => {
    if (!open) return;
    const prevFocus = document.activeElement as HTMLElement | null; // the trigger, to restore on close
    const panel = ref.current;
    const focusables = (): HTMLElement[] =>
      panel
        ? Array.from(
            panel.querySelectorAll<HTMLElement>(
              'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
            ),
          )
        : [];
    (focusables()[0] ?? panel)?.focus(); // move focus IN

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        closeRef.current();
        return;
      }
      if (e.key !== "Tab" || !panel) return;
      const f = focusables();
      if (f.length === 0) {
        e.preventDefault();
        return;
      }
      const first = f[0];
      const last = f[f.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && active === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      prevFocus?.focus?.(); // restore focus to the trigger
    };
  }, [open]);

  return ref;
}
