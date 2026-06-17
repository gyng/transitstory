// React root. Phase machine: menu → (boot) → playing. `boot()` builds the imperative world
// (map, deck overlay, sim, Game, GameLoop) exactly as the old main.ts did — React only owns
// the floating chrome inside #ui. The map/deck overlay live in the separate #map div and the
// rAF loop runs entirely outside React (AGENTS render-hot-path / two-clocks rules).
import { useCallback, useEffect, useRef, useState } from "react";
import { applyArcadiaBasemap, createMap } from "../../map/basemap";
import { createOverlay } from "../../map/overlay";
import { loadCity } from "../../sim/city";
import { mmToLngLat } from "../../coords/geo";
import { loadNetwork, networkFromSupplyGraph } from "../../sim/network";
import { cityById, type CityEntry } from "../../sim/cities";
import { SimBridge } from "../../sim/SimBridge";
import { Buildability } from "../../sim/buildability";
import { writeSave, type SaveBlob } from "../../sim/save";
import { Game } from "../../game";
import { GameLoop } from "../../sim/GameLoop";
import { attachPointer } from "../../tools/pointer";
import { installTestHooks } from "../../testhooks";
import { GameProvider, useGame, useGameUI } from "./GameContext";
import { AppShell } from "./AppShell";
import { TimeCluster } from "./TimeCluster";
import { Menu } from "./Menu";
import { StatsBar } from "./StatsBar";
import { Panels } from "./Panels";
import { Toolbar } from "./Toolbar";
import { OnboardingCoach } from "./Onboarding";
import { ObjectivePanel } from "./Objectives";
import { TechPanel } from "./TechPanel";
import { SpellBar } from "./SpellBar";
import { ServiceReport } from "./ServiceReport";
import { Fleet } from "./Fleet";
import { DraftControls } from "./DraftControls";
import { CommuterCard } from "./CommuterCard";
import { FollowCard } from "./FollowCard";
import { StatsDashboard } from "./StatsDashboard";
import { StatsRecorder } from "./statsHistory";
import { ContextMenu } from "./ContextMenu";
import { StationConfirmBar } from "./StationConfirmBar";
import { DayReport, Milestones } from "./Beats";
import { getScenario } from "../../objectives";

interface BootedWorld {
  game: Game;
  loop: GameLoop;
  cityName: string;
}

/** Build the imperative world for one city. Mirrors the old main.ts boot() — sans the UI
 *  mounts (React owns those) and the 3 Hz stats interval (the GameProvider runs it). When
 *  `resume` is given, the saved command log is replayed instead of the real network. */
async function boot(manifestPath: string, withNetwork: boolean, resume?: SaveBlob): Promise<BootedWorld> {
  const city = await loadCity(manifestPath); // sets the session coordinate origin

  const map = createMap("map", city.raw.center, city.raw.zoom);
  const overlay = createOverlay();
  map.addControl(overlay);

  const bridge = new SimBridge(city.seed, city.coreCityJson);
  const game = new Game(bridge, map, overlay, new Buildability(city.buildability));
  game.ruleset = city.raw.ruleset ?? "transit"; // mode-aware chrome (fantasy build tools etc.)
  // S11 rail-gate: arcadia builds RAIL only (+ Heavy Rail once teched). Enable rail + heavy here so the
  // chord/settings can't select bus/ferry/plane; the toolbar only SHOWS heavy once HEAVY_RAIL is unlocked,
  // and the sim rejects an un-teched heavy line regardless (the source of truth).
  if (game.ruleset === "arcadia") {
    game.enabledModes = new Set([0, 4]);
    applyArcadiaBasemap(map); // dead ash-grey void under the baked continent (figure-ground)
    game.sky.setEnabled(false); // no day/night hue wash in the value-not-hue fantasy world
    // #3d-trees diorama: tilt the camera so the lowpoly pines (+ terrain) read as a TTD-style 3D scene.
    // A modest pitch keeps the strategic overview legible; the player can still pan/zoom freely.
    map.setMaxPitch(60);
    map.setPitch(45);
  }
  game.demandHeat = city.demandHeat; // travel-demand heat overlay source
  game.demandCellM = city.demandCellM; // sizes the demand-heat hexagons to the grid pitch
  // Fantasy baked terrain IS the map: feed the raw buildability cells (exact hex centres — NOT the
  // square-binned Buildability lookup) straight to the terrain layer. Fantasy only, so transit cities
  // never draw it. terrainCellM = the hex circumradius (gridCellMm/1000).
  if (game.ruleset === "arcadia" && city.buildability) {
    game.terrain = city.buildability.cells.map((c) => ({ lng: c.lon, lat: c.lat, c: c.c }));
    game.terrainCellM = (city.raw.gridCellMm ?? 0) / 1000;
    // Baked supply-chain source nodes: i64 mm (= hexgrid::center_of) → lng/lat via the one geo boundary.
    const sg = city.raw.supplyGraph;
    if (sg) {
      game.resources = sg.resources.map((r) => {
        const [lng, lat] = mmToLngLat([r.xMm, r.yMm]);
        return { lng, lat, kind: r.kind, yield: r.yield };
      });
      game.towns = (sg.towns ?? []).map((t) => {
        const [lng, lat] = mmToLngLat([t.xMm, t.yMm]);
        // recipe → chain: grain(1)+fuel(3) = BREAD, ore(0)+aether(2) = ARMS (rings the town accordingly).
        const r = t.recipe ?? [];
        const chain = r.includes(1) || r.includes(3) ? "bread" : r.includes(0) || r.includes(2) ? "arms" : "";
        return { lng, lat, kind: t.kind, value: t.value, decadence: t.decadence, chain };
      });
      game.decadenceAnchors = (sg.decadenceSeed?.reservoir ?? []).map((a) => {
        const [lng, lat] = mmToLngLat([a.xMm, a.yMm]);
        return { lng, lat };
      });
      game.influenceHops = sg.decadenceSeed?.influenceHops ?? 0; // #infrastructure: >0 arms the connected-rail gate (drives the frontier halos)
      game.buildAmbientTrade(); // #living — ambient ox-cart trade routes between the baked nodes (render-only)
    }
    // Baked rivers (additive top-level manifest field): i64-mm cell-centre endpoints → lng/lat via the one
    // coordinate boundary. Render-only cold water; never reaches the core.
    game.rivers = (city.raw.rivers ?? []).map((rv) => {
      const [flng, flat] = mmToLngLat([rv.x0Mm, rv.y0Mm]);
      const [tlng, tlat] = mmToLngLat([rv.x1Mm, rv.y1Mm]);
      return { from: [flng, flat] as [number, number], to: [tlng, tlat] as [number, number], wclass: rv.wclass, ford: rv.ford };
    });
    game.buildTrees(); // #3d-trees — scatter lowpoly pines on the forest hexes for the 3D diorama
  }
  const loop = new GameLoop(game);
  attachPointer(game);
  installTestHooks(game, loop);

  if (resume) {
    bridge.loadLog(resume.log); // replays seed + saved log through the same Command path
    game.mode = bridge.stats().running ? "run" : "build";
  } else if (withNetwork && city.raw.networkPath) {
    try {
      game.applyNetwork(await loadNetwork(city.raw.networkPath));
    } catch (e) {
      console.warn("network load failed; starting empty", e);
    }
  } else if (game.ruleset === "arcadia" && city.raw.supplyGraph) {
    // The baked supply graph IS the world's fixed nodes (resources = sources, towns = sinks, capital =
    // barracks). Place them via the command path; the player draws the rail connecting the chains. Not
    // gated on `withNetwork` — these are map features, not an optional starting metro.
    game.applyNetwork(networkFromSupplyGraph(city.raw.supplyGraph));
  }

  // Autosave from the first PLAYER action onward — wiring onCommit after the pre-seeded network
  // / resume replay means we don't re-save the baseline; the log always holds the full state.
  bridge.onCommit = () =>
    writeSave({ v: 1, cityId: city.raw.id, cityName: city.raw.name, seed: city.seed, log: [...bridge.log.all()] });

  map.once("load", () => game.refresh());
  game.refresh();

  // Cinematic intro (arcadia, fresh start only): establish on the whole continent, then fly INTO the
  // capital so the player starts at their seat. The flyTo keeps the map non-idle, so __MAP_READY (which
  // fires on 'idle') only resolves AFTER it settles — e2e waiting on __MAP_READY is unaffected. Skipped on
  // resume + when the player prefers reduced motion (jump straight in).
  if (game.ruleset === "arcadia" && !resume) {
    const cap = game.towns.find((t) => t.kind === "capital");
    if (cap) {
      const wide = (city.raw.zoom ?? 10) - 2;
      const near = (city.raw.zoom ?? 10) + 3; // past DETAIL_ZOOM → the capital's detailed seat (icons/nodes show)
      const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches;
      map.once("load", () => {
        if (reduce) {
          map.jumpTo({ center: [cap.lng, cap.lat], zoom: near });
        } else {
          map.jumpTo({ center: city.raw.center, zoom: wide }); // full-continent establishing shot
          map.flyTo({ center: [cap.lng, cap.lat], zoom: near, duration: 2600, essential: true });
        }
      });
    }
  }

  window.__ot = { map, bridge, city, overlay, game };
  window.__APP_READY = true;
  return { game, loop, cityName: city.raw.name };
}

function Title({ name }: { name: string }) {
  return (
    <div
      id="app-title"
      style={{
        margin: 0,
        padding: "4px 10px",
        borderRadius: 8,
        background: "rgba(255,255,255,.85)",
        font: "600 14px system-ui,sans-serif",
        color: "#1c2024",
        boxShadow: "0 2px 10px rgba(0,0,0,.12)",
        whiteSpace: "nowrap",
      }}
    >
      Transit Story · {name}
    </div>
  );
}

/** Top-left chrome row: title · Undo · Stats laid out by flex, so a long city name pushes its
 *  neighbours along instead of sliding under them (the old fixed `left` offsets overlapped). */
function TopLeftBar({ children }: { children: React.ReactNode }) {
  return (
    <div style={{ position: "fixed", top: 10, left: 14, zIndex: 10, display: "flex", alignItems: "stretch", gap: 8 }}>
      {children}
    </div>
  );
}

export function App() {
  const [world, setWorld] = useState<BootedWorld | null>(null);
  const [booting, setBooting] = useState(false);
  const [scenarioId, setScenarioId] = useState<string | null>(null);
  const started = useRef(false);

  const startBoot = useCallback((manifestPath: string, withNetwork: boolean, scenario: string | null = null) => {
    setScenarioId(scenario);
    setBooting(true);
    void boot(manifestPath, withNetwork).then(setWorld);
  }, []);

  const startResume = useCallback((save: SaveBlob) => {
    setScenarioId(null);
    setBooting(true);
    void boot(cityById(save.cityId).manifest, false, save).then(setWorld);
  }, []);

  // Deep-link / e2e: `?city=<id>&network=0|1&scenario=<id>` skips the menu.
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    const params = new URLSearchParams(location.search);
    const cityParam = params.get("city");
    if (cityParam) {
      const entry = cityById(cityParam);
      // The globe's "network" is its cities — always load it (the board is empty without them).
      startBoot(entry.manifest, entry.id === "globe" || params.get("network") === "1", params.get("scenario"));
    }
  }, [startBoot]);

  // Ctrl/Cmd-Z = undo (rebuild from seed + log[..-1]); Ctrl/Cmd-Shift-Z or Ctrl-Y = redo.
  useEffect(() => {
    if (!world) return;
    const onKey = (e: KeyboardEvent) => {
      const k = e.key.toLowerCase();
      if ((e.ctrlKey || e.metaKey) && (k === "y" || (k === "z" && e.shiftKey))) {
        e.preventDefault();
        world.game.redo();
      } else if ((e.ctrlKey || e.metaKey) && !e.shiftKey && k === "z") {
        e.preventDefault();
        world.game.undo();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [world]);

  // Game-feel CAMERA: WASD / arrow keys pan (held-key rAF for smooth continuous motion), Q/E (and
  // =/+ / -) zoom. The map + this rAF live OUTSIDE React (AGENTS two-clocks) — this drives the MapLibre
  // camera imperatively, never the sim. We disable MapLibre's native keyboard handler and own all of it
  // (else arrows double-pan once the canvas has focus). Ignores typing in inputs + OS/undo chords.
  useEffect(() => {
    if (!world) return;
    const map = world.game.map;
    map.keyboard.disable(); // own keyboard nav fully (no native arrow/-/+ double-handling)
    const held = new Set<string>();
    const PAN = 12; // px per frame per axis
    const DIRS: Record<string, [number, number]> = {
      w: [0, -1], s: [0, 1], a: [-1, 0], d: [1, 0],
      arrowup: [0, -1], arrowdown: [0, 1], arrowleft: [-1, 0], arrowright: [1, 0],
    };
    let raf = 0;
    const tick = () => {
      let dx = 0, dy = 0;
      for (const k of held) { const v = DIRS[k]; if (v) { dx += v[0]; dy += v[1]; } }
      if (dx || dy) {
        const len = Math.hypot(dx, dy) || 1; // normalise so a diagonal isn't ~41% faster than a cardinal
        map.panBy([(dx / len) * PAN, (dy / len) * PAN], { duration: 0 });
        raf = requestAnimationFrame(tick);
      } else {
        raf = 0;
      }
    };
    const onDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.ctrlKey || e.metaKey || e.altKey) return; // let undo/redo/OS chords pass through
      const k = e.key.toLowerCase();
      if (k === "q" || k === "-" || k === "_") { e.preventDefault(); map.easeTo({ zoom: map.getZoom() - 0.6, duration: 140 }); return; }
      if (k === "e" || k === "=" || k === "+") { e.preventDefault(); map.easeTo({ zoom: map.getZoom() + 0.6, duration: 140 }); return; }
      if (!(k in DIRS)) return;
      e.preventDefault();
      held.add(k);
      if (!raf) raf = requestAnimationFrame(tick);
    };
    const onUp = (e: KeyboardEvent) => { held.delete(e.key.toLowerCase()); };
    const stop = () => { held.clear(); if (raf) { cancelAnimationFrame(raf); raf = 0; } };
    window.addEventListener("keydown", onDown);
    window.addEventListener("keyup", onUp);
    window.addEventListener("blur", stop); // tab-switch mid-press must not strand a key (camera drift)
    return () => {
      window.removeEventListener("keydown", onDown);
      window.removeEventListener("keyup", onUp);
      window.removeEventListener("blur", stop);
      if (raf) cancelAnimationFrame(raf);
    };
  }, [world]);

  if (!world) {
    if (booting) return null; // map is being built; chrome appears once the world is ready
    return (
      <Menu
        onStart={(c: CityEntry, withNet: boolean, scenario: string | null) => {
          // Mirror the start into the URL (the same deep-link the e2e uses) so a refresh — and
          // the objectives' "Retry challenge" reload — re-boots this exact setup, not the menu.
          const q = new URLSearchParams({ city: c.id });
          if (withNet) q.set("network", "1");
          if (scenario) q.set("scenario", scenario);
          history.replaceState(null, "", `?${q.toString()}`);
          startBoot(c.manifest, c.id === "globe" || withNet, scenario);
        }}
        onResume={startResume}
      />
    );
  }

  const scenario = getScenario(scenarioId);

  return (
    <GameProvider game={world.game} loop={world.loop}>
      {/* The viewport chrome shell: a CSS grid over #map owning the four rigid edges. STAGE 1 hosts
          the EXISTING (position:fixed) chrome inside their current-equivalent region cells unchanged
          — fixed positioning keeps them visually pinned exactly where they were, so this proves the
          grid + pointer scoping with zero behaviour change. Later stages flow each group into its cell. */}
      <AppShell
        top={
          <>
            <StatsBar />
            {/* Time cluster pinned to the top-right of the strip: Build/Run + speed + clock. The
                spacer pushes it right; StatsBar is position:fixed (top-centre) so it ignores flow. */}
            <div style={{ flex: 1, pointerEvents: "none" }} />
            <div style={{ display: "flex", alignItems: "flex-start", padding: "8px 14px 0 0", pointerEvents: "none" }}>
              <TimeCluster />
            </div>
          </>
        }
        left={<Panels />}
        right={
          <>
            {scenario && <ObjectivePanel scenario={scenario} />}
          </>
        }
        bottom={<Toolbar />}
      />
      {/* Floating / transient overlays — kept outside the shell so they retain their own fixed
          position + z-order (re-parented into regions in later stages where the spec calls for it). */}
      <TopLeftBar>
        <Title name={world.cityName} />
        <UndoControl />
        <DashboardControl />
      </TopLeftBar>
      <Toast />
      <OnboardingCoach />
      <TechPanel />
      <SpellBar />
      <Fleet />
      <ServiceReport />
      <DraftControls />
      <CommuterCard />
      <FollowCard />
      <StatsRecorder />
      <DayReport />
      <Milestones />
      <ContextMenu />
      <StationConfirmBar />
    </GameProvider>
  );
}

/** The prominent "📊 Stats" entry point + the dashboard it opens (ledger + detailed stats +
 *  charts). The button sits top-left next to Undo; the dashboard is a centred overlay. State is
 *  local — the always-mounted StatsRecorder keeps history accruing whether or not it's open. */
function DashboardControl() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        data-testid="open-dashboard"
        onClick={() => setOpen(true)}
        title="Network dashboard — ledger, ridership, satisfaction, trend charts"
        style={{
          padding: "4px 12px",
          borderRadius: 8,
          border: "0",
          background: "#1c2024",
          color: "#fff",
          font: "700 13px system-ui,sans-serif",
          cursor: "pointer",
          boxShadow: "0 2px 10px rgba(0,0,0,.18)",
        }}
      >
        📊 Stats
      </button>
      <StatsDashboard open={open} onClose={() => setOpen(false)} />
    </>
  );
}

/** Transient notice (e.g. an afford-gate rejection) so a gated Command has a visible echo
 *  (AGENTS: every Command needs an on-map/HUD echo). Auto-dismisses after a few seconds. */
function Toast() {
  const game = useGame();
  const { notice } = useGameUI();
  useEffect(() => {
    if (!notice) return;
    const id = window.setTimeout(() => game.dismissNotice(), 3000);
    return () => window.clearTimeout(id);
  }, [notice, game]);
  if (!notice) return null;
  return (
    <div
      data-testid="notice"
      onClick={() => game.dismissNotice()}
      style={{
        position: "fixed",
        top: 56,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 20,
        padding: "8px 16px",
        borderRadius: 8,
        background: "var(--ot-gauge-bad, #d62828)",
        color: "#fff",
        font: "600 13px system-ui,sans-serif",
        boxShadow: "0 4px 16px rgba(0,0,0,.3)",
        cursor: "pointer",
      }}
    >
      {notice}
    </div>
  );
}

/** Undo/redo affordances next to the title (AGENTS UX "reversible by construction"). Ctrl-Z /
 *  Ctrl-Shift-Z are the primary paths; these make them discoverable. Re-render on the UI slice
 *  so the disabled states track the history boundaries. */
function HistoryButton({ testid, label, hint, enabled, onClick }: { testid: string; label: string; hint: string; enabled: boolean; onClick: () => void }) {
  return (
    <button
      data-testid={testid}
      onClick={onClick}
      disabled={!enabled}
      title={hint}
      style={{
        padding: "4px 10px",
        borderRadius: 8,
        border: "0",
        background: enabled ? "rgba(255,255,255,.85)" : "rgba(255,255,255,.4)",
        color: enabled ? "#1c2024" : "#9aa3ad",
        font: "600 13px system-ui,sans-serif",
        cursor: enabled ? "pointer" : "default",
        boxShadow: "0 2px 10px rgba(0,0,0,.12)",
      }}
    >
      {label}
    </button>
  );
}

function UndoControl() {
  const game = useGame();
  useGameUI(); // subscribe: re-render when selection/mode change (covers the empty↔non-empty edge)
  return (
    <>
      <HistoryButton testid="undo" label="↶ Undo" hint="Undo last action (Ctrl-Z)" enabled={game.canUndo()} onClick={() => game.undo()} />
      {/* Redo renders only when there IS something to redo — dead chrome otherwise. */}
      {game.canRedo() && (
        <HistoryButton testid="redo" label="↷ Redo" hint="Redo (Ctrl-Shift-Z / Ctrl-Y)" enabled onClick={() => game.redo()} />
      )}
    </>
  );
}
