// Persistence = seed + command log (the documented save artifact). We never serialize World
// state; resuming replays the log through the same applyCommandJson path that produced it, so
// the save is small and the determinism guarantee carries it. localStorage holds one autosave
// per browser; the network/lines are restored (sim time resets — the slice doesn't persist the
// clock). This is strictly outer-ring: the core is untouched.
import type { Command } from "../types";

const KEY = "transitstory.save.v1";

export interface SaveBlob {
  v: 1;
  cityId: string;
  cityName: string;
  seed: number;
  log: Command[];
}

export function writeSave(blob: SaveBlob): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(blob));
  } catch {
    /* quota exceeded / private mode — saving is best-effort, never fatal */
  }
}

export function readSave(): SaveBlob | null {
  try {
    const s = localStorage.getItem(KEY);
    if (!s) return null;
    const b = JSON.parse(s) as Partial<SaveBlob>;
    if (b && b.v === 1 && typeof b.cityId === "string" && typeof b.seed === "number" && Array.isArray(b.log)) {
      return { v: 1, cityId: b.cityId, cityName: b.cityName ?? b.cityId, seed: b.seed, log: b.log as Command[] };
    }
  } catch {
    /* corrupt blob — ignore, start fresh */
  }
  return null;
}

export function clearSave(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    /* ignore */
  }
}
