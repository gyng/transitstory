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
import { AlertCluster } from "./AlertCluster";
import { LensRail } from "./LensRail";
import { ConstructionRail } from "./ConstructionRail";
import { CornerCluster } from "./CornerCluster";
import { Settings } from "./Settings";
import { StatsDashboard } from "./StatsDashboard";
import { BuildHud } from "./BuildHud";
import { Menu } from "./Menu";
import { StatsBar } from "./StatsBar";
import { Inspector } from "./Inspector";
import { Outliner } from "./Outliner";
import { OnboardingCoach } from "./Onboarding";
import { DraftControls } from "./DraftControls";
import { CommuterCard } from "./CommuterCard";
import { FollowCard } from "./FollowCard";
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

  // Pass the ruleset so the OSM AttributionControl mounts for real-OSM cities only (the ODbL release
  // gate) — the baked fantasy/arcadia continent is non-OSM, so it carries no (false) OSM credit.
  const ruleset = city.raw.ruleset ?? "transit";
  const map = createMap("map", city.raw.center, city.raw.zoom, ruleset);
  const overlay = createOverlay();
  map.addControl(overlay);

  const bridge = new SimBridge(city.seed, city.coreCityJson);
  const game = new Game(bridge, map, overlay, new Buildability(city.buildability));
  game.ruleset = ruleset; // mode-aware chrome (fantasy build tools etc.)
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

/** The app title (city name). Folded into the grid TopStrip's LEFT cell as the first flex element,
 *  ordered BEFORE the resource strip in the SAME flex context — so a long city name pushes the
 *  resources along instead of the old `position:fixed` bar sliding UNDER them (the overlap fix).
 *  Keeps id `app-title` (e2e contract: load/menu specs assert its text). */
function Title({ name }: { name: string }) {
  return (
    <div
      id="app-title"
      className="ot-console"
      title={`Transit Story · ${name}`}
      style={{
        margin: "7px 0 0 0",
        padding: "7px 12px",
        font: "600 14px system-ui,sans-serif",
        color: "var(--ot-con-ink)",
        whiteSpace: "nowrap",
        alignSelf: "flex-start",
        pointerEvents: "auto",
      }}
    >
      Transit Story · {name}
    </div>
  );
}

export function App() {
  const [world, setWorld] = useState<BootedWorld | null>(null);
  const [booting, setBooting] = useState(false);
  const [scenarioId, setScenarioId] = useState<string | null>(null);
  // Dashboard + settings modal open flags live here (App level) so the modals render as shell siblings
  // at #ui-level z, above the grid shell's stacking context — opened from the bottom-left CornerCluster.
  const [dashOpen, setDashOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
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
          // Top strip — three flow cells: resources (L) · alerts (C, clickable→fly-to) · time (R).
          // StatsBar + AlertCluster + TimeCluster are real grid-flowed children now (stage 5 dissolved
          // the position:fixed centred bar). The L cell holds the resource strip; the centre flexes so
          // the alert tray stays centred between them; the R cell pins the time cluster to the corner.
          <>
            {/* Title → resources, in ONE left-cell flex context (Title first), so they can never
                overlap: a long city name pushes the resource strip along instead of sliding under it.
                The 14px left padding matches the other shell edges' inset. */}
            <div style={{ flex: "0 1 auto", display: "flex", alignItems: "flex-start", gap: 8, minWidth: 0, padding: "0 0 0 14px", pointerEvents: "none" }}>
              <Title name={world.cityName} />
              <StatsBar />
            </div>
            <div style={{ flex: 1, display: "flex", justifyContent: "center", alignItems: "flex-start", padding: "7px 0 0", pointerEvents: "none" }}>
              <AlertCluster />
            </div>
            <div style={{ flex: "0 0 auto", display: "flex", alignItems: "flex-start", padding: "8px 14px 0 0", pointerEvents: "none" }}>
              <TimeCluster />
            </div>
          </>
        }
        left={
          // Left edge: the construction rail (categories + flyout) anchored top, and the corner
          // utility cluster (undo/redo · dashboard · settings) pinned to the bottom-left corner (Fitts).
          // No raised z needed any more — all the legacy fixed panels migrated into the shell (stages 6-7),
          // so the rail/corner stack naturally within the shell's own context.
          <div style={{ height: "100%", display: "flex", flexDirection: "column", justifyContent: "space-between", alignItems: "flex-start", padding: "0 0 14px 14px", pointerEvents: "none", position: "relative" }}>
            <div style={{ paddingTop: 0, pointerEvents: "none" }}>
              <ConstructionRail />
            </div>
            <CornerCluster onOpenDashboard={() => setDashOpen(true)} onToggleSettings={() => setSettingsOpen((o) => !o)} />
          </div>
        }
        right={
          // Right edge (stage 7): a flex ROW — the Inspector docked INBOARD (left) of the LensRail, so
          // the two never share a column (resolves the stage 1-4 right:14 overlap). Both vertically
          // centred. The Inspector is progressive (empty until selection). <1024px the inspector
          // OVERLAYS the lens rail (modal) — handled by the .ot-right-edge responsive rules in styles.css.
          <div className="ot-right-edge" style={{ height: "100%", display: "flex", flexDirection: "row", alignItems: "center", justifyContent: "flex-end", gap: 10, padding: "0 14px", pointerEvents: "none", position: "relative" }}>
            <Inspector />
            <LensRail />
          </div>
        }
        bottom={
          // Bottom dock: the Outliner (roster/fleet/report tabs ⅔ + event ticker ⅓) fills the row; the
          // live BuildHud floats bottom-centre OVER it (sub-100ms client-side route readout), now that the
          // dock no longer crams bottom-centre (the structural overflow fix — stage 6).
          <div style={{ height: "100%", position: "relative", pointerEvents: "none" }}>
            <Outliner scenario={scenario} />
            <div style={{ position: "absolute", inset: 0, display: "flex", alignItems: "flex-end", justifyContent: "center", padding: "0 0 8px", pointerEvents: "none" }}>
              <BuildHud />
            </div>
          </div>
        }
      />
      {/* Floating / transient overlays — kept outside the shell so they retain their own fixed
          position + z-order (build HUD, draft, cards, beats). The roster migrated to the bottom
          Outliner (stage 6), the editor to the right Inspector (stage 7), and the title into the
          top-strip L cell (folded in with the resource strip so they can't overlap — item 1). */}
      <NoticeAutoDismiss />
      <OnboardingCoach />
      <DraftControls />
      <CommuterCard />
      <FollowCard />
      <StatsRecorder />
      <DayReport />
      <Milestones />
      <ContextMenu />
      <StationConfirmBar />
      {/* Settings + dashboard modals: shell siblings so they float above the grid shell (their own
          higher z) — triggered from the bottom-left CornerCluster. */}
      <Settings open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <StatsDashboard open={dashOpen} onClose={() => setDashOpen(false)} />
    </GameProvider>
  );
}

/** A transient notice (e.g. an afford-gate rejection) now renders as a ping in the top-centre
 *  AlertCluster (`notice` testid lives there) — this hook-only component preserves the toast's
 *  auto-dismiss after a few seconds, so a gated Command's visible echo still clears itself. */
function NoticeAutoDismiss() {
  const game = useGame();
  const { notice } = useGameUI();
  useEffect(() => {
    if (!notice) return;
    const id = window.setTimeout(() => game.dismissNotice(), 3000);
    return () => window.clearTimeout(id);
  }, [notice, game]);
  return null;
}

