// The single React⇄sim seam. Two slices, two cadences (AGENTS "two clocks, never merged"):
//   • stats  — the low-frequency snapshot, PUSHED from the existing ~3 Hz interval (the chosen
//              "Context + interval setState"); also drives game.setStats → deck overlay halos.
//   • ui     — selection/mode/tool/transport/etc, updated on game.onChange so player actions
//              reflect sub-100 ms (UX non-negotiable). Filtered by shallow-compare so the 3 Hz
//              stats churn (onChange also fires on each setStats) doesn't cause redundant renders.
// React NEVER touches the rAF render loop or the deck.gl overlay — those stay imperative in
// GameLoop/Game. Commands still flow only through Game methods (which wrap SimBridge).
import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import type { ContextMenuState, Game } from "../../game";
import type { GameLoop } from "../../sim/GameLoop";
import type { Stats } from "../../types";
import { audio } from "../../fx/audio";

export interface GameUI {
  mode: Game["mode"];
  tool: Game["tool"];
  /** The loaded ruleset ("transit" | "arcadia") — drives mode-aware chrome (fantasy build tools). */
  ruleset: string;
  transport: number;
  enabledModes: number[]; // sorted snapshot of the Set (stable array for render deps)
  showDemand: boolean;
  showReach: boolean;
  showRoads: boolean;
  showPeeps: boolean;
  lens: Game["lens"];
  selectedLine: number | null;
  selectedStation: number | null;
  notice: string | null;
  contextMenu: ContextMenuState | null;
  /** A station placement is awaiting confirm (fantasy "confirm build") — drives the confirm bar. */
  pendingStation: boolean;
  /** History depths, so the Undo/Redo chrome re-renders exactly when they move. */
  historyLen: number;
  redoLen: number;
}

function snapUI(g: Game): GameUI {
  return {
    mode: g.mode,
    tool: g.tool,
    ruleset: g.ruleset,
    transport: g.transport,
    enabledModes: [...g.enabledModes].sort((a, b) => a - b),
    showDemand: g.showDemand,
    showReach: g.showReach,
    showRoads: g.showRoads,
    showPeeps: g.showPeeps,
    lens: g.lens,
    selectedLine: g.selectedLine,
    selectedStation: g.selectedStation,
    notice: g.notice,
    contextMenu: g.contextMenu,
    pendingStation: g.pendingStation !== null,
    historyLen: g.bridge.log.length,
    redoLen: g.bridge.canRedo() ? 1 : 0, // depth doesn't matter to the chrome, availability does
  };
}

function uiEqual(a: GameUI, b: GameUI): boolean {
  return (
    a.mode === b.mode &&
    a.tool === b.tool &&
    a.ruleset === b.ruleset &&
    a.transport === b.transport &&
    a.showDemand === b.showDemand &&
    a.showReach === b.showReach &&
    a.showRoads === b.showRoads &&
    a.showPeeps === b.showPeeps &&
    a.lens === b.lens &&
    a.selectedLine === b.selectedLine &&
    a.selectedStation === b.selectedStation &&
    a.notice === b.notice &&
    a.contextMenu === b.contextMenu &&
    a.pendingStation === b.pendingStation &&
    a.historyLen === b.historyLen &&
    a.redoLen === b.redoLen &&
    a.enabledModes.length === b.enabledModes.length &&
    a.enabledModes.every((v, i) => v === b.enabledModes[i])
  );
}

interface Ctx {
  game: Game;
  loop: GameLoop;
  ui: GameUI;
  stats: Stats;
}

const GameCtx = createContext<Ctx | null>(null);

export function GameProvider({
  game,
  loop,
  children,
}: {
  game: Game;
  loop: GameLoop;
  children: ReactNode;
}) {
  const [ui, setUi] = useState<GameUI>(() => snapUI(game));
  const [stats, setStats] = useState<Stats>(() => game.bridge.stats());
  const uiRef = useRef(ui);

  useEffect(() => {
    // Immediate UI slice: re-render only when a UI field actually changed.
    const onChange = () => {
      const next = snapUI(game);
      if (!uiEqual(uiRef.current, next)) {
        uiRef.current = next;
        setUi(next);
      }
    };
    game.onChange.push(onChange);

    // Low-frequency stats slice (the ~3 Hz throttle). game.setStats also refreshes the deck
    // overlay's waiting-pax halos — that lives OUTSIDE React, in Game/deck.
    const id = window.setInterval(() => {
      const s = game.bridge.stats();
      setStats(s);
      game.setStats(s);
      // Day/night mood wash (two-clocks: rides the 3 Hz slice, not rAF). NOT in arcadia — the fantasy
      // world reads in VALUE and reserves warmth for the empire; a time-varying hue wash would muddy the
      // warmth-vs-decadence read and fight the tide's own cold-violet.
      if (game.ruleset !== "arcadia") game.sky.set(s.simHour);
    }, 333);

    loop.start();

    // WebAudio can only start inside a user gesture — unlock the kit on the first pointer/key
    // input, then drop the listeners (they're `once`). Until then every cue is a silent no-op.
    const unlock = () => audio.unlock();
    window.addEventListener("pointerdown", unlock, { once: true });
    window.addEventListener("keydown", unlock, { once: true });

    return () => {
      window.clearInterval(id);
      const i = game.onChange.indexOf(onChange);
      if (i >= 0) game.onChange.splice(i, 1);
      loop.stop();
      window.removeEventListener("pointerdown", unlock);
      window.removeEventListener("keydown", unlock);
    };
  }, [game, loop]);

  return <GameCtx.Provider value={{ game, loop, ui, stats }}>{children}</GameCtx.Provider>;
}

function useCtx(): Ctx {
  const c = useContext(GameCtx);
  if (!c) throw new Error("useGame* must be used within <GameProvider>");
  return c;
}

/** The Game instance — call its methods (the only write path) from event handlers. Stable. */
export function useGame(): Game {
  return useCtx().game;
}
/** The GameLoop — for speed knobs (speed is a loop knob, never a Command). Stable. */
export function useLoop(): GameLoop {
  return useCtx().loop;
}
/** Immediate UI-state slice (selection/mode/tool/transport/enabledModes/showDemand). */
export function useGameUI(): GameUI {
  return useCtx().ui;
}
/** The ~3 Hz Stats snapshot (ridership, per-line, coverage, money, time-of-day, …). */
export function useStats(): Stats {
  return useCtx().stats;
}
