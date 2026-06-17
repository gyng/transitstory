# transitstory — GSG/tycoon UI reorg (implementation spec)

**Status:** APPROVED (2026-06-18). Produced by a scout + a 3-lens design panel (tycoon/city-builder, GSG,
cogsci/IA) → adversarial synthesis, then owner sign-off on the three open forks. This is the
green-at-every-commit implementation spec. Genres referenced: OpenTTD/Transport Tycoon (rail
construction sub-toolbar), Cities:Skylines / SimCity 2013 (info-view/data-layer lenses), Transport Fever
/ Anno / Factorio (resource strip + alert pings), Paradox CK3/EU4/HOI4/Stellaris (top strip + alert tray
+ outliner).

## Owner-confirmed forks
1. **Bottom dock = ROSTER + event TICKER hybrid** (roster ⅔ left: Roster/Fleet/Report tabs; ticker ⅓
   right: passive event log + day-report digest + objective).
2. **Transport-MODE picker lives INSIDE the RAIL flyout** (nested under RAIL with Track/Service/Station).
3. **Below 1024px the Inspector OVERLAYS the lens rail** (modal, preserves map width).

## The motivating bug
The single bottom `#transport-bar` crammed ~6 groups (transport modes + Build/Run + speeds + layer
toggles + arcadia lens-bar + ⚙) into one `flex-wrap` row at `maxWidth:96vw` — ~946px transit / ~1242px
fantasy — overflowing/mis-centering below ~1024px. The reorg DISSOLVES that bar; each group migrates to a
region that owns one concept, so no row ever reassembles the union (the overflow becomes structurally
impossible, not patched).

## Full-screen layout (4 rigid edges — geometry never relayouts between rulesets; contents swap)
```
┌──────────────────────────────────────────────────────────────────────────┐
│ TOP STRIP  grid[auto | 1fr | auto] ~44px                                   │
│  RESOURCES (L)            ALERTS (C, clickable→fly-to)        TIME (R)      │
│  money·riders·●coverage   ⚠ left-behind·overbook·war·notice   ⏱·▶Run·1248max│
├────┬──────────────────────────────────────────────────┬─────────┬─────────┤
│ L  │ ┌RAIL flyout (anchored, ONE open)──────────────┐  │ INSPEC- │  LENS   │
│ E  │ │ ╱Track ◉Station 🚆Service ‖ mode 🚆🚌⛴✈🚚    │  │  TOR    │  rail   │
│ F  │ └───────────────────────────────────────────────┘  │ (on     │  🌡🕐🛣 │
│ T  │  ▸RAIL ▸MILITARY ▸BULLDOZE ▸TECH ▸BOUNTY            │ select) │  🧍🚦   │
│    │            #MAP (deck.gl — SPATIAL TRUTH)           │  name   │  ──    │
│ ↶↷ │            ⌖ draft/confirm/build-hud float @cursor  │  headwy │  ◉realm│
│ 📊⚙│                                                     │  track  │  ☠decd │
├────┴──────────────────────────────────────────────────┴─────────┴─────────┤
│ BOTTOM DOCK ~92px  [ ROSTER | Fleet | Report ] lines·load·hdwy ‖ [ ticker ] │
└──────────────────────────────────────────────────────────────────────────┘
```

## Regions (contents · interaction · state-binding · ruleset-swap)
- **TOP-LEFT resources** (`StatsBar`→strip): transit `money`/`ridership`/`coverage`(+gauge, oversized for
  glanceability)/`waiting`/`left-behind`/`net-load`; fantasy `tribute`/`mana`/`manpower`/`standing`(gauge)/
  `decadence`(gauge)/`towns-captured`/`armies`/`raiders`. Each scalar an `.ot-readout` well. Binds `stats`
  (3Hz); `ui.ruleset` picks the set. Gauge click → `open-dashboard`.
- **TOP-CENTER alerts** (NEW `AlertCluster`, absorbs floating `notice`): clickable severity-sorted pings
  (CB-safe icon+count+label, `.ot-led`): left-behind, overbooked/starved stations, congestion, water,
  `milestone`, `notice`; fantasy `decadence-breach`/`decadence-eta`/`realm-lost`/`raiders`. Click →
  `game.flyToAlert(id)` (imperative deck flyTo — NOT a React rAF) + arms the relevant tool. **HARD RULE:**
  every alert derives ONLY from the sim's `stats.perStation[]`/`perLine[]` pressure fields — never a
  parallel JS heuristic — so badge + fly-to always agree with the map. Binds `stats` (3Hz).
- **TOP-RIGHT time** (NEW `TimeCluster`, lifted from `Toolbar`): Build/Run segmented `.ot-key` (cyan=Run,
  the hard wall; reads `ui.mode`, writes `game.setMode`), speed ladder `speed-1/2/4/8/100` (LOCAL state →
  `loop.setSpeed`, NOT a Command), `clock`/`period`/`tod-glyph` (reads `stats`).
- **LEFT construction** (NEW `ConstructionRail`, from `Menu` build portion + `Toolbar` tools/modes):
  L1 = 5 radio category keys RAIL/MILITARY/BULLDOZE/TECH/BOUNTY; L2 = horizontal flyout anchored to the
  armed key (OpenTTD sub-toolbar). RAIL→`tool-line`(Track)·`tool-service`·`tool-station` ‖ MODE segment
  `mode-transport-*` (gated by `enabledModes`; `aircraft-picker` here) + `tool-select` at head. MILITARY→
  `tool-barracks` (fantasy). BULLDOZE→arms `tool-bulldozer` (danger tone, no submenu). TECH→`tech-launcher`/
  `tech-panel` popover. BOUNTY→`tool-bounty` (fantasy). Bottom-left CORNER cluster (Fitts): undo/redo,
  `open-dashboard`, `open-settings`. Binds `ui` (immediate). Tool hints render in the flyout.
- **RIGHT lens rail** (NEW `LensRail`, from `Toolbar` layers + arcadia lens-bar): additive overlay toggles
  `layer-demand`/`layer-reach`/`layer-roads`/`layer-peeps`/`layer-signals`; fantasy exclusive radio
  `lens-realm`/`lens-supply`/`lens-military`/`lens-decadence` (the `lens-bar`). Icon+label, `.ot-key.on`.
  Toggles ONLY flip overlay visibility / recolor existing geometry — never new geometry. Binds `ui`.
- **RIGHT inspector** (consolidate `Panels` editor + `Fleet` detail), docks immediately LEFT of the lens
  rail: `editor-panel` (name/color/`trains-input`/`assign-trainset`/`headway-slider`+`headway-label`/
  track-type/build-mode/`extend-*`/branches), `station-editor` (`platform-stepper`/`platform-±`/
  `platform-count`), hover tips, `follow-card`/`commuter-card`. **Progressive disclosure: empty until
  selection.** Binds `ui.selection` (mount) + `stats` (readouts). **<1024px: OVERLAYS the lens rail.**
  **Headway commits on native `change` only** (preview on `input` via local `useState`).
- **BOTTOM dock** (NEW `Outliner` wrapping `Panels` LineList + `Fleet` + `ServiceReport` + `Objectives`):
  LEFT ⅔ = roster (tabs Roster/Fleet/Report): `line-list` keyed swatch·name·`line-load`·headway·
  `line-performance`/`line-impact`; click→select→drives the right inspector. Fantasy adds `spell-bar`/
  `spells`/`autocast-toggle` as a roster-adjacent fan. RIGHT ⅓ = ticker: scrolling event log + `day-report`
  digest + `objectives`/`objective-banner`/`onboarding` one-liner (read-only). Roster binds `stats`
  (`perLine[]` 3Hz); ticker binds `ui.notice`/history + `stats`.
- **FLOATING near cursor (NOT in the dock):** `build-hud`*, `draft-controls`/`draft-*`, `station-confirm`*
  — sub-100ms client-side off the build gesture. Freeing bottom-center is what lets these float without
  collision (the structural overflow fix).

## Overflow fix (old #transport-bar → new homes)
modes→RAIL flyout · tools→left categories/flyouts · Build/Run+speeds→TimeCluster(top-R) · layer toggles +
lens-bar→LensRail(right, VERTICAL — height-bounded, never wraps) · ⚙→bottom-left corner. Deepest nesting
= `category→≤3 items`; the union never co-renders. **Responsive <1024px:** left rail icon-only; mode
picker→single popover if needed; inspector OVERLAYS lens rail (modal); lens rail→single "lens" key+flyout;
ticker→single line; top-strip resources wrap to 2 rows in the L cell only (C/R hold).

## Testid discipline
**Every existing testid is an e2e contract — RE-HOME, never drop.** Full map in the panel synthesis; key
moves: resources(money/ridership/coverage/tribute/mana/manpower/standing/decadence/towns/armies/raiders)→
top-L; alerts(notice/milestone/water-warning/decadence-breach/-eta/realm-lost)→top-C; time(clock/period/
mode-toggle|start|resume/speed-*/mode-controls)→top-R; tools(tool-*/mode-transport-*/aircraft-picker/
tech-*)→left; lenses(layer-*/lens-*)→right rail; editor(editor-panel/headway-slider/trains-input/
platform-*/extend-*)→right inspector; roster(line-list/line-load/line-performance/line-impact)+fleet
(fleet-*)+report(service-*)+spells→bottom dock; objectives/day-report/onboarding→bottom ticker;
build-hud/draft-*/station-confirm→floating; settings(setting-*)→`settings-panel` from corner ⚙.

## Component/CSS plan
- NEW: `AlertCluster`, `TimeCluster`, `ConstructionRail`, `LensRail`, `Outliner`. Refactor: `StatsBar`→
  resource strip; `Panels`→split (Editor→right Inspector, LineList→bottom Outliner); `Toolbar`→REMOVED
  (groups distributed) — keep its `Button`/`BigModeButton` `.ot-key` primitives, extract to `shared.ts`.
  Re-parented only: `BuildHud`, `DraftControls`, `StationConfirmBar`, `ContextMenu`, `Follow/CommuterCard`,
  `TechPanel`, `SpellBar`, `Settings`, `StatsDashboard`, `Onboarding`.
- App shell = a CSS GRID pinned to the viewport over `#map`: `grid-template-rows: 44px 1fr 92px;
  grid-template-columns: auto 1fr auto`. Top spans all cols; edges occupy edge cells. Wrapper
  `pointer-events:none`, re-enabled on `.ot-console` children (map drags pass through gaps — existing
  pattern). Reuse `--ot-con-*` tokens + `.ot-console`/`.ot-key`(`.on`/`.on-good`/`.on-danger`)/`.ot-readout`/
  `.ot-led` AS-IS. Micro-motion preserved.
- Constraints: React owns DOM chrome only; #map/deck/rAF untouched, center cell, z-below. All reads via
  `useStats`/`useGameUI`; writes via `Game` methods. DOM on the 3Hz stats slice. fly-to via the imperative
  map seam. Per-line color via the single `hex()` path to roster swatch + inspector.

## Staged build (each stage = ONE commit, app working + tsc/vitest green; testids re-homed not dropped)
1. **Grid shell, no behavior change** — viewport grid + region wrappers; render existing Toolbar/StatsBar/
   Panels UNCHANGED inside cells. Prove grid + pointer-events scoping. All testids still resolve.
2. **Build/Run + speed → TimeCluster (top-right)** — drop from `#transport-bar`.
3. **layer toggles + lens-bar → LensRail (right edge, vertical).**
4. **tools + transport-MODE → ConstructionRail (left categories+flyout); DELETE `#transport-bar`.** The
   overflow is now structurally impossible. Playwright resize to 1000px → no overflow.
5. **Split StatsBar → resource strip (L) + AlertCluster (C);** wire alerts to `stats` pressure + `flyToAlert`.
6. **Bottom Outliner: roster/fleet/report tabs + ticker;** frees bottom-center for floating controls.
7. **Right Inspector consolidation** (editor+station+tips+fleet detail, progressive on selection); corner
   utility cluster.
8. **Fantasy parity + responsive (<1024px) + console-theme polish.** Arcadia loop turn; no overflow at
   1000/1280px.

Each stage keeps the old component mounted until its replacement asserts green, so the e2e suite stays
green throughout.
