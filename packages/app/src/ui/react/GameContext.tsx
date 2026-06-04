// The single React⇄sim seam. Two slices, two cadences (AGENTS "two clocks, never merged"):
//   • stats  — the low-frequency snapshot, PUSHED from the existing ~3 Hz interval (the chosen
//              "Context + interval setState"); also drives game.setStats → deck overlay halos.
//   • ui     — selection/mode/tool/transport/etc, updated on game.onChange so player actions
//              reflect sub-100 ms (UX non-negotiable). Filtered by shallow-compare so the 3 Hz
//              stats churn (onChange also fires on each setStats) doesn't cause redundant renders.
// React NEVER touches the rAF render loop or the deck.gl overlay — those stay imperative in
// GameLoop/Game. Commands still flow only through Game methods (which wrap SimBridge).
import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import type { Game } from "../../game";
import type { GameLoop } from "../../sim/GameLoop";
import type { Stats } from "../../types";

export interface GameUI {
  mode: Game["mode"];
  tool: Game["tool"];
  transport: number;
  enabledModes: number[]; // sorted snapshot of the Set (stable array for render deps)
  showDemand: boolean;
  selectedLine: number | null;
  selectedStation: number | null;
}

function snapUI(g: Game): GameUI {
  return {
    mode: g.mode,
    tool: g.tool,
    transport: g.transport,
    enabledModes: [...g.enabledModes].sort((a, b) => a - b),
    showDemand: g.showDemand,
    selectedLine: g.selectedLine,
    selectedStation: g.selectedStation,
  };
}

function uiEqual(a: GameUI, b: GameUI): boolean {
  return (
    a.mode === b.mode &&
    a.tool === b.tool &&
    a.transport === b.transport &&
    a.showDemand === b.showDemand &&
    a.selectedLine === b.selectedLine &&
    a.selectedStation === b.selectedStation &&
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
    }, 333);

    loop.start();

    return () => {
      window.clearInterval(id);
      const i = game.onChange.indexOf(onChange);
      if (i >= 0) game.onChange.splice(i, 1);
      loop.stop();
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
