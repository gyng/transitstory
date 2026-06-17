//! The deterministic world state and its two pure entry points: `apply(Command)` for
//! mutations and `tick(dt_ms)` for advancement. Holds the seeded RNG, the command log,
//! and `state_hash()` for the determinism test. No clock/thread/HashMap-iteration/float
//! in state-affecting paths.
use crate::city::CityData;
use crate::command::{Command, Event};
use crate::geo_local::PointMm;
use crate::hash::fnv1a;
use crate::ids::{LineId, StationId};
use crate::line::Line;
use crate::station::Station;
use crate::stats::{AccessLink, LineStat, LineView, OdLink, ShedCell, StationStat, StatsSnapshot, StationView};
use crate::tick;
use crate::trainset::TrainsetAssignment;
use crate::vehicle::VehicleSoA;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Frequency/capacity guardrails. `count` is clamped so the (future) pre-sized SoA
/// vehicle buffers can never be exceeded; headway has a sane floor. CLOCK-FRAME (see
/// `tod::CLOCK_SCALE`): values are sim-ms, labels are what the in-game clock observes —
/// 1 clock-minute = 2_000 sim-ms.
pub const MAX_TRAINS_PER_LINE: u16 = 24;
pub const MIN_HEADWAY_MS: i64 = 2_000; // 1 clock-min
pub const MAX_HEADWAY_MS: i64 = 120_000; // 60 clock-min
pub const DEFAULT_HEADWAY_MS: i64 = 10_000; // 5 clock-min

// Economy (optional, NIMBY-style). Dollars. Construction is a one-time capital cost; fares
// accrue per boarding. The disruption metric feeds the surface land-taking premium.
pub const START_BUDGET: i64 = 2_000_000_000;
pub const FARE: i64 = 2; // $ per boarding
/// GOLD to post a bounty (fantasy/arcadia, V3 — steering the legions costs the realm's treasury). A flat
/// decree cost; clearing a bounty (amount 0) is free. The bounty `amount` is the steering WEIGHT, not gold.
const BOUNTY_COST: i64 = 10;
const PER_KM_SURFACE: i64 = 8_000_000;
const PER_KM_ELEVATED: i64 = 30_000_000;
const PER_KM_TUNNEL: i64 = 90_000_000;
// Heavy / high-speed rail needs dedicated, grade-separated right-of-way: far pricier per km.
const PER_KM_HSR_SURFACE: i64 = 24_000_000;
const PER_KM_HSR_ELEVATED: i64 = 60_000_000;
const PER_KM_HSR_TUNNEL: i64 = 180_000_000;
const TAKING_PER_KM_BUILT: i64 = 6_000_000;
/// P2: single track costs this percent of double-track per-km capital (one rail pair, not two).
const SINGLE_TRACK_PCT: i64 = 55;
const TRAIN_COST: i64 = 15_000_000;
// TTD L5d: capital per player-placed block signal (~¼ km of surface track). A signal raises a
// single-track span's same-direction throughput, so it's a cheaper alternative to double-tracking
// (which adds ~45% of PER_KM_SURFACE per km) — but it's not free, so signalling is an economic
// tradeoff in the fantasy economy. 0 signals ⇒ 0 added ⇒ the goldens (no signals) stay byte-identical.
const SIGNAL_COST: i64 = 2_000_000;
// Recurring maintenance (opex), accrued only while the economy is ON and running. A slow drain
// that fares must outrun — the second pressure axis alongside waiting. Tunable game balance.
const DAY_MS: i64 = 86_400_000;
const OPEX_PER_TRAIN_DAY: i64 = 200_000;
const OPEX_PER_KM_DAY: i64 = 50_000;
/// Fantasy gold UPKEEP (#economy): a train costs this many KM-equivalents of upkeep (rolling stock is
/// pricier to keep than track). Daily gold drain = `(track_km + trains×this) × gold_upkeep_per_day /
/// GOLD_UPKEEP_DIVISOR`. Tunable; 0 baked rate disables it (golden-neutral default).
const GOLD_UPKEEP_TRAIN_KM: i64 = 4;
const GOLD_UPKEEP_DIVISOR: i64 = 100;

/// One TTD-style SIGNAL marker (render-only): the state of a single-track span (or the gate a held cart
/// waits at). `status`: 1 = OCCUPIED (a cart is in the span — red), 2 = WAITING (a cart is held at this
/// gate for the block ahead — amber). `mid_mm` is the arc-length on `(line, path)` to draw it at — so
/// `render_buf` self-positions via `point_at` with no extra lookup. Carries `span` for tests/dedup.
#[derive(Clone, Copy, Debug)]
pub struct SignalOccupancy {
    pub line: u32,
    pub path: u8,
    pub span: u32,
    pub mid_mm: i64,
    pub status: u8,
}

/// TTD L5a — a PLAYER-PLACED block signal: a passing-place gate at arc-length `at_mm` strictly inside
/// span `span` of `(line, path)`. Placed/removed via `Command::PlaceSignal`/`RemoveSignal`; AUTHORITATIVE
/// player state (hashed in `Canonical`, unlike the derived scratch `SignalOccupancy` readout above).
/// In L5a it is RECORDED + replayable but does NOT yet re-key occupancy — L5b makes a signal subdivide a
/// single span into sub-blocks (a mid-span meet point). Integer-only (i64 mm) ⇒ determinism-safe; the
/// store is kept canonically SORTED + deduped so the hash is command-order-independent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Signal {
    pub line: LineId,
    pub path: u8,
    pub span: u32,
    pub at_mm: i64,
}

/// The canonical total-order key for a `Signal` — `(line, path, span, at_mm)`. The store is kept sorted
/// by this so it is deduped and its serialization is command-order-independent (placement order can't
/// change the hash), mirroring the topology-pure `track_segments` discipline.
fn signal_key(s: &Signal) -> (u32, u8, u32, i64) {
    (s.line.0, s.path, s.span, s.at_mm)
}

pub struct World {
    pub seed: u64,
    pub clock_ms: i64,
    pub running: bool,
    pub rng: ChaCha8Rng,
    pub stations: Vec<Station>,
    pub lines: Vec<Line>,
    pub vehicles: VehicleSoA,
    pub city: CityData,
    pub cmd_log: Vec<Command>,
    /// Set when a line/trainset/headway/running change requires the dispatcher to rebuild
    /// vehicles; cleared after a rebuild so steady running does no work.
    pub dispatch_dirty: bool,

    // --- demand / ridership (T16) ---
    /// Per-station captured origin (resident) and destination (job) weight from the grid.
    pub captured_origin: Vec<f32>,
    pub captured_dest: Vec<f32>,
    /// Fantasy (arcadia) S7e multi-stage: per-station captured DEST weight broken down BY commodity, flat
    /// `station * N_COMMODITIES + commodity`. Lets the router send a cart to a node that WANTS its
    /// commodity (a raw → its processor, a mid → its final sink) instead of the highest-TOTAL-dest node.
    /// DERIVED in `prepare` (not hashed, a read-cache). Only consulted when `has_multistage`, so raw-only
    /// worlds (transit, the current baked world, the golden fixtures) route exactly as before.
    pub dest_by_comm: Vec<f32>,
    /// Fantasy (arcadia) S7e multi-stage: true iff this world uses any PROCESSED good (a station whose
    /// output commodity is ≥ `forge::FIRST_MID`, i.e. a processor). Gates commodity-aware routing so a
    /// raw-only world is byte-identical (no re-pin, no balance change); set in `prepare`. NOT hashed.
    pub has_multistage: bool,
    /// Fantasy (arcadia) S7e: per-station OUTPUT commodity — the dominant origin-commodity of a station's
    /// captured cells (ORE=0 default). A net-source node produces THIS commodity (not always ORE). DERIVED
    /// from the demand grid in `prepare`, like `captured_origin`; NOT hashed (a read-cache, golden-neutral).
    pub station_commodity: Vec<u8>,
    /// Fantasy (arcadia) S7e-2: per-station RECIPE — the distinct commodities a sink REQUIRES (the commodities
    /// it captures DEST weight of). A sink with ≥2 required commodities consumes them by LIEBIG (output =
    /// min input; the scarcer throttles), so a BREAD town needs grain+fuel and an ARMS barracks ore+aether.
    /// A single/empty recipe ⇒ consume-all (the S7e-1 behaviour) ⇒ commodity-0 worlds are byte-identical.
    /// DERIVED in `prepare`; NOT hashed (golden-neutral).
    pub station_recipe: Vec<Vec<u8>>,
    /// Fractional passenger-spawn accumulator per station (deterministic count).
    pub spawn_accum: Vec<f32>,
    /// Forge-Line per-node commodity BUFFERS (fantasy, S7): flat `station * N_COMMODITIES + commodity`
    /// = units held. **Hashed** (folded into `Canonical`) — the first fantasy state. EMPTY for transit
    /// (`GravityDemand`/`AgentDemand` never call `forge::produce`), so transit serialises a length-0
    /// vec (one re-pin, then byte-identical). Sized lazily by `produce` on the arcadia path.
    pub forge_stock: Vec<i64>,
    /// Sub-unit (µ-unit) production remainder per node — the integer fixed-point accumulator that keeps
    /// `forge_stock` accrual exact. Derived/transient like `spawn_accum` (regenerated bit-identically on
    /// replay from the same tick sequence), so NOT folded into `Canonical`.
    pub forge_accum: Vec<i64>,
    /// Sub-unit (µ-unit) OFF-RAIL backstop remainder per node (fantasy #11) — the integer fixed-point
    /// accumulator for the slow walking-goods trickle into a starved town. Derived/transient like
    /// `forge_accum` (regenerated bit-identically on replay), so NOT folded into `Canonical`.
    pub walk_accum: Vec<i64>,
    /// The war machine's legions (fantasy, S8) — a SEPARATE SoA from `vehicles` so `dispatch`'s
    /// `v.clear()` (every `SetHeadway`) can't teleport a marching army (binding condition #2). Its
    /// authoritative fields are hashed; empty for transit. See [`crate::army`].
    pub armies: crate::army::ArmySoA,
    /// The RIVAL (fantasy, S11): decadence raiders — a separate hashed SoA (free 2-D off-rail position),
    /// fielded from the reservoir, marching the capital. Empty for transit/demo (no reservoir).
    pub raiders: crate::raider::RaiderSoA,
    /// Raider spawn-cadence accumulator (ms) + the reservoir cursor — hashed (gameplay-causal), but no rng:
    /// a fixed accumulator + a cycling counter keep the rival deterministic. 0 for transit/demo.
    pub raider_spawn_accum_ms: i64,
    pub raider_cursor: u32,
    /// Lose-meter floor-raise from raiders that REACHED the capital (S11). The field-derived `decadence`
    /// is overwritten each tick, so raiders accumulate their damage HERE; the field step adds it back on
    /// top (bounded by the capital threshold). DECAYS toward 0 via `heal_breach` (the realm recovers when
    /// the network holds — no point-of-no-return). 0 for transit/demo (no raiders). Hashed.
    pub raider_breach: i64,
    /// Sub-unit accumulator for the breach HEAL (so a slow per-tick decay rate isn't truncated to 0). 0
    /// for transit/demo. Hashed.
    pub raider_breach_heal_accum: i64,
    /// Rail-attack (#war): per-LINE "disabled until" clock-ms — a raider that reaches a line's track CUTS
    /// it for `RAIL_DISABLE_MS`; while `clock_ms < disabled_until`, the line is RAIDED: its trains freeze
    /// in place (no advance, no delivery) until the timer lapses. **Hashed** (gameplay-causal). Lazily
    /// grown ONLY when a raid lands (index = LineId), so it stays EMPTY for transit + both golden fixtures
    /// (zero raiders ⇒ no raid ⇒ no growth) — appended-last + empty ⇒ byte-identical, no golden re-pin.
    pub line_disabled_until_ms: Vec<i64>,
    /// Per-town resistance (siege HP, fantasy S8b): a defended town grinds down under siege; 0 = fallen.
    /// **Hashed.** Lazily sized to the node count (empty for transit). Index = StationId.
    pub town_value: Vec<i64>,
    /// Count of towns captured this game (fantasy S8b) — the conquest score. **Hashed.** 0 for transit.
    pub towns_captured: i64,
    /// Per-station BARRACKS flag (fantasy S8): legions launch only from a barracks on a built route.
    /// **Hashed** (set by `PlaceBarracks`, a pure function of the command log). Empty for transit.
    pub is_barracks: Vec<bool>,
    /// Per-town BOUNTY (fantasy S8 — the Majesty steering lever): a posted bounty pulls AI legions
    /// toward that town (the highest-bounty uncaptured town on a route becomes the target). **Hashed**
    /// (set by `PostBounty`). Empty for transit. Steering only for now; the payout economics are S11.
    pub bounty: Vec<i64>,
    /// Global DECADENCE (fantasy, S9): the spreading-corruption pressure — the lose condition. Grows
    /// while running, pushed back by conquest; reaching the capital threshold = the realm falls.
    /// **Hashed.** 0 for transit (never runs `war_step`). See [`crate::decadence`].
    pub decadence: i64,
    /// Sub-unit (milli-unit) decadence remainder — the integer fixed-point accumulator that keeps the
    /// per-tick `net·dt/1000` growth EXACT instead of truncating any rate below ~20/s to zero (the bug
    /// that froze the gentle baked continent's lose meter). Derived/transient like `forge_accum` /
    /// `spawn_accum` (regenerated bit-identically on replay from the same tick sequence), so NOT folded
    /// into `Canonical`; only the whole-unit `decadence` is authoritative state.
    pub decadence_accum: i64,
    /// Global TRIBUTE (fantasy, S7d): the supply score — accumulated as towns (sink nodes) consume the
    /// commodity delivered to them (the game's core payoff: feed towns → tribute). **Hashed** (in
    /// `Canonical`). Always 0 for transit (gravity never consumes commodities), so it adds one i64 to
    /// the transit hash (a re-pin) then stays byte-identical. The S11 economy splits this into
    /// gold/mana/manpower channels behind this same accumulator.
    pub tribute: i64,
    /// The S11 ECONOMY SPLIT — two SPECIALISED channels minted ALONGSIDE gold (`tribute`) by WHICH
    /// commodity a town consumes: `mana` from AETHER chains, `manpower` from INGOT/ARMS chains. Gold is
    /// still minted by every delivery (unchanged), so the war-chest balance is untouched; mana/manpower
    /// are ADDITIVE bonuses that gate channel-specific tech (so the COMPOSITION of your supply network
    /// matters, not just its volume). **Hashed** (in `Canonical`). 0 for transit + any world that delivers
    /// no aether/ingot (the demo arcadia golden ⇒ appended-zero re-pin, behaviour byte-identical).
    pub mana: i64,
    pub manpower: i64,
    /// Cumulative SPELLS cast (fantasy, S11 — the mana spell arm). Hashed (deterministic counter); 0 for
    /// transit + any realm without SPELLCRAFT. The HUD reads it; `spell::step` increments it on each cast.
    pub spells_cast: u32,
    /// Recent spell FLASHES (render-only — a brief burst at each cast site, aged + retired in `spell::step`).
    /// NOT hashed (like the army/raider cartesian render fields).
    pub spell_flashes: Vec<crate::spell::SpellFlash>,
    /// TTD-style SIGNAL occupancy (render-only): the per-tick block state of single-track spans, so the
    /// player SEES why a cart waits at a meet. Re-derived every tick in `vehicle::advance` from the same
    /// occupancy the meet protocol builds — write-only scratch consumed only by the render copy-out, so
    /// it can't perturb the next tick. Deliberately NOT in `Canonical`/`state_hash` (like `spell_flashes`/
    /// `forge_accum`) ⇒ golden-neutral, regenerated bit-identically on replay.
    pub signal_occupancy: Vec<SignalOccupancy>,
    /// TTD L5a — the AUTHORITATIVE store of player-placed block signals. HASHED (in `Canonical`, appended
    /// LAST) — distinct from the scratch `signal_occupancy` readout above. Kept canonically sorted+deduped
    /// so the hash is command-order-independent. EMPTY for transit + the arcadia golden (no signal ever
    /// placed) ⇒ a one-time length-0 append re-pin, then byte-identical; motion unaffected in L5a.
    pub signals: Vec<Signal>,
    /// AUTOCAST toggle (fantasy, S11): off (default) ⇒ spells fire only on `Command::CastSpell` (the player
    /// picks WHEN, the invest-vs-cast tradeoff); on ⇒ `spell::step` auto-fires the battery each tick. Set by
    /// `Command::SetAutocast`. **Deliberately NOT in `Canonical`**: a pure input toggle whose EVERY effect
    /// (mana / spells_cast / decadence_cells / raider.state / army.strength) is already hashed, and it is
    /// only ever set by a deterministic command — so two replays of one log set it identically ⇒ excluding
    /// it can't mask divergence, and the goldens (autocast never set) stay byte-identical with NO re-pin.
    pub autocast: bool,
    /// Unlocked-tech bitset (fantasy, S11): bit `TECHS[id].bit` set ⇒ that upgrade is active. Bought with
    /// tribute via `Command::UnlockTech`; each bit gates a buff to an existing lever (forge rate / legion
    /// cost / decadence creep). **Hashed** (in `Canonical`). Always 0 for transit (the ruleset rejects
    /// `UnlockTech`) and the arcadia golden (its log predates tech), so it adds one u32 to both hashes
    /// (a one-time re-pin) then stays byte-identical.
    pub tech_unlocked: u32,
    /// Per-station FIFO queue of waiting passengers (each carrying a multi-leg route).
    pub waiting: Vec<VecDeque<crate::pax::Pax>>,
    /// Per-station lines serving it (operational only); rebuilt by the dispatcher for routing.
    pub serving: Vec<Vec<LineId>>,
    /// Coalesced divergence/convergence switch-clusters of branched lines (P4,
    /// docs/capacity-roadmap.md). Re-derived in `dispatch` on `dispatch_dirty` (same trigger as
    /// `serving`); a pure function of the already-hashed line topology, so it is **never hashed**
    /// (transient, like `serving`). Empty for an all-non-branched network ⇒ the junction-mutex
    /// passes are inert and motion is byte-identical to pre-P4.
    pub junctions: Vec<Junction>,
    /// Cross-LINE shared physical-rail blocks (Phase 2, docs/shared-rail.md). Re-derived in `dispatch`
    /// on `dispatch_dirty` for GRID lines only; **never hashed** (transient, like `junctions`). Empty
    /// for a continuous / non-grid / non-shared network ⇒ the cross-line mutex is inert (zero re-pins).
    pub cross_blocks: Vec<CrossBlock>,
    /// Derived shared-infrastructure TrackGraph (TTD L1, docs/ttd-track-model.md): the per-line polylines
    /// abstracted into nodes (stations/junctions/termini) + segments (maximal grid-edge runs). Re-derived
    /// in `dispatch` for GRID lines only; **never hashed** (transient, a pure fn of `lines`/`stations`,
    /// like `cross_blocks`). Empty for continuous / non-grid networks ⇒ zero re-pins. The spine L2+ key off.
    pub track_graph: crate::track_graph::TrackGraph,
    /// Inter-station footpaths: per station, the nearby stations reachable on foot within
    /// `FOOTPATH_MM`, each with its integer walk time (ms). Derived from positions, rebuilt with
    /// the catchment when stations change. Lets RAPTOR transfer between unconnected lines whose
    /// stops are close (an interchange by foot); board_alight delays the rider by the walk time.
    pub footpaths: Vec<Vec<(u32, i64)>>,
    /// Route cache (origin,dest)->legs, so BFS isn't rerun per spawn on large networks.
    /// A derived cache (not hashed); cleared when the network changes. Lookup-only, so no
    /// HashMap-iteration determinism hazard.
    pub route_cache: rustc_hash::FxHashMap<(u32, u32), Option<Vec<crate::routing::Leg>>>,
    /// Accessibility cache: origin station → one-to-all transit travel time (ms) to every station
    /// (`i64::MAX` = unreachable), from `Router::reachable`. Lets the demand model weight a trip's
    /// destination by how fast the network reaches it. Derived (not hashed); cleared on network
    /// change alongside `route_cache`; lookup-only, so no HashMap-iteration determinism hazard.
    pub access_cache: rustc_hash::FxHashMap<u32, Vec<i64>>,
    /// Buildability lookup: (cell_x, cell_y) -> class code. Built once from CityData; lookup-only.
    pub build_lookup: rustc_hash::FxHashMap<(i32, i32), u8>,
    pub build_cell_mm: i64,
    /// Fantasy (arcadia) S10: STATIC topology of the decadence area-control field — the hex-cell domain,
    /// adjacency, creep-to-capital gradient, and reservoir seed. Built once from `CityData` (a pure
    /// function of it, reconstructible on replay), so NOT hashed. Empty for transit / demo arcadia. The
    /// dynamic, hashed per-cell tide values (S10b) layer on top of this board.
    pub decadence_field: crate::decadence_field::DecadenceField,
    /// Fantasy (arcadia) S10b: the DYNAMIC decadence tide — per-domain-cell corruption (dense over
    /// `decadence_field.cells`, 0..`DECAD_MAX`), evolved by the double-buffered creep CA. **Hashed**
    /// (RNG-/gameplay-causal, can't reconstruct from seed alone), appended LAST in `Canonical`. EMPTY for
    /// transit / demo arcadia (no terrain ⇒ the CA never runs), so it adds a length-0 slice to the hash
    /// (one re-pin, then byte-identical). Sized lazily by `decadence_field::step` on first arcadia tick.
    pub decadence_cells: Vec<i32>,
    /// Sub-unit (milli-gain) decadence-creep remainder — the integer fixed-point accumulator that makes
    /// the tide creep rate CONTINUOUS: a slow `creep_per_s` that would truncate `creep·dt/1000` to 0/tick
    /// instead accrues across ticks. The per-tick gain is uniform across advancing cells, so a single
    /// scalar. Derived/transient like `forge_accum` (regenerated bit-identically on replay), so NOT
    /// folded into `Canonical`. A rate yielding an exact integer gain (e.g. the default/baked rates)
    /// leaves this 0 ⇒ every current world is byte-identical to the old gain-floor.
    pub decadence_gain_accum: i64,
    /// Cumulative boardings (the headline ridership counter).
    pub ridership_total: u64,
    pub boardings: Vec<u64>,
    pub alightings: Vec<u64>,
    // --- passenger lifecycle telemetry (service-quality legibility) ---
    /// Σ end-to-end trip time (ms) over completed trips, and the completed-trip count.
    pub total_journey_ms: u64,
    pub journey_samples: u64,
    /// Σ platform wait (ms) over boardings, and the boarding count (one sample per board).
    pub total_wait_ms: u64,
    pub wait_samples: u64,
    /// Cumulative times a rider wanting a line was passed by a full vehicle (the real
    /// "left behind" pressure — distinct from the live waiting-queue depth).
    pub denied_boardings: u64,
    /// Cumulative riders who gave up waiting (renege) because service was too infrequent.
    pub abandoned: u64,
    /// `denied_boardings`/`abandoned` bucketed PER STATION (where the loss happened). Index-stable
    /// with `stations`; sums equal the global totals. Surfaced as the per-platform starvation
    /// signal. Folded into state_hash (deterministic, derived from the same command/tick sequence).
    pub denied_at: Vec<u64>,
    pub abandoned_at: Vec<u64>,
    /// Set when stations change (catchment capture needs recompute).
    pub demand_dirty: bool,
    /// The last in-game day `demand::grow` ran for (clock_ms / DAY). Pure function of the clock,
    /// so replays reconstruct it — not hashed (like the other derived markers).
    pub last_growth_day: i64,
    /// The last in-game day gold UPKEEP was charged (fantasy #economy). Same clock-derived, replay-
    /// reconstructed, NOT-hashed pattern as `last_growth_day` — the drain it triggers mutates the hashed
    /// `tribute`, but the cursor itself stays out of `Canonical` (golden-neutral; 0 by default).
    pub last_upkeep_day: i64,
    /// Per-cell weight ceiling for demand growth: 2× the city's strongest initial cell, computed
    /// once at boot — dataset-agnostic (a metro-population globe cell and a 0–8 city cell both
    /// get headroom without runaway).
    pub growth_cap_w: f32,
    /// Optional economy (NIMBY-style): when OFF (the default), money is informational only —
    /// when ON, construction you can't afford is rejected and opex drains the balance.
    pub economy_enabled: bool,
    /// Cumulative maintenance (opex) charged so far, and the sub-day remainder (exact integer
    /// accrual). Affects `balance` → the afford-gate, so both are folded into state_hash.
    pub opex_accrued: i64,
    pub opex_rem: i64,
    /// The trip-planning strategy (the routing seam). `BfsRouter` ships; RAPTOR swaps in here.
    pub router: Box<dyn crate::routing::Router>,
    /// The game-mode seam (fantasy-fork.md): selected from `CityData.ruleset` at construction and
    /// FROZEN — never a Command. Owns scoring + command validity (+ fantasy's per-tick trailer).
    /// **Not hashed** (a construction-time selector, not evolving state) and, until S2, **not yet
    /// called** (the transit logic still lives in `world.rs`/`demand.rs`), so the golden pin is
    /// byte-identical. Mirrors `router`.
    pub ruleset: Box<dyn crate::ruleset::Ruleset>,
    /// The demand model (gravity vs agents vs supply-chain) behind the same seam. Default-constructed
    /// to `GravityDemand`; `SetDemandMode` swaps `AgentDemand` in at S2. Inert until S2, like `ruleset`.
    pub demand: Box<dyn crate::ruleset::Demand>,
    /// Max legs (transfers + 1) a routed trip may use (from CityData, or the routing default).
    pub max_legs: usize,
    /// Demand model: when `true`, trips come from `population` (agents) instead of gravity flow.
    /// Command-derived (SetDemandMode), so it isn't hashed; the trips it causes are.
    pub agent_demand: bool,
    /// The seed-derived citizen population (Some when agent demand is on). NOT folded into
    /// state_hash — it is a pure function of (seed, grid), regenerated on enable / replay.
    pub population: Option<crate::agents::Population>,
    /// Set when the served-station set changes (dispatch rebuild) so the agent population refreshes
    /// its cell→nearest-station map. Derived/transient (not hashed), like `dispatch_dirty`.
    pub cell_station_dirty: bool,
    /// Render-only breadcrumbs of recently-completed trips (the "peep walking out of the station"
    /// animation source). A bounded, age-pruned ring buffer written in `board_alight` and read by
    /// `render_buf::fill_peeps`. **Deliberately excluded from `Canonical`** — purely cosmetic and
    /// regenerated by replay, so it costs the determinism gate nothing. See [`RecentAlight`].
    pub recent_alight: std::collections::VecDeque<RecentAlight>,
}

/// A coalesced set of branch divergence/convergence POINTS of one line that lie within one
/// consist-length of each other on the trunk — a single atomic mutex (the "switch cluster", P4,
/// docs/capacity-roadmap.md). Keyed on the group id (line + lowest member station). Re-derived on
/// `dispatch_dirty` (same trigger as `serving`), never per-tick; **not** in `state_hash` (a pure
/// function of the already-hashed line topology, regenerated bit-for-bit on replay). Coalescing is
/// the load-bearing liveness fix: two switches within one consist-length form a 2-cycle deadlock
/// under a naive point-mutex, so they are merged into one atomic group (one owner ⇒ acyclic).
#[derive(Clone)]
pub struct Junction {
    pub line: LineId,
    /// The group's identity station = the lowest `StationId` among member points (index-stable key,
    /// independent of the order branches were added).
    pub key_station: StationId,
    /// For every service PATH that passes through ANY member point: the group's arc-length SPAN
    /// `[lo, hi]` on THAT path (`lo` = min member arclen, `hi` = max member arclen on that path),
    /// as `(path, lo_mm, hi_mm)`, sorted by path index. A single-point group has `lo == hi`. Each
    /// path supplies its OWN arc-lengths — the clean answer to per-path Catmull-Rom inflation: the
    /// mutex never keys on a shared scalar.
    pub span_by_path: Vec<(u8, i64, i64)>,
}

/// A CROSS-LINE shared physical-rail block (Phase 2, docs/shared-rail.md): a maximal run of
/// physically-SINGLE grid edges traversed by **>=2 distinct lines**, between common passing places.
/// Keyed on a line-INDEPENDENT `block_id` so two lines on one physical rail land in the SAME mutex row
/// (unlike the line-scoped `Junction`). Re-derived on `dispatch_dirty` for grid lines only; **never
/// hashed** (transient, like `junctions`/`serving`). Inert unless a grid network actually shares a
/// single edge ⇒ continuous / non-shared networks are byte-identical.
#[derive(Clone)]
pub struct CrossBlock {
    /// Unique, deterministic id (the block's index in edge-key-sorted order — command-order-independent).
    pub block_id: u64,
    /// The shared-edge component contains a CYCLE (a ring shared by lines) ⇒ a capacity-1 global mutex
    /// (the depth-1-forest liveness argument fails on a ring). Else capacity = `passing_places + 1`.
    pub cyclic: bool,
    /// Physically-DOUBLE shared edges bracketing/within the block — the cross-line meet capacity.
    pub passing_places: u32,
    /// Per TRAVERSAL: `(line, path, lo_arclen, hi_arclen)` — the block's arc-length window on that
    /// lane. A lane appears MORE THAN ONCE when an out-and-back train crosses the block both ways
    /// (forward + return are distinct arclen windows), so the runtime mutex tests each.
    pub by_lane: Vec<(u32, u8, i64, i64)>,
}

/// One render-only breadcrumb: a passenger finished their trip at `station` at `t_ms` (citizen id
/// for a stable walk direction/jitter). Drives the walk-out peep; NOT hashed. 16 bytes, `Copy`.
#[derive(Clone, Copy)]
pub struct RecentAlight {
    pub station: u32,
    pub citizen: u32,
    pub t_ms: i64,
}

/// Borrowed canonical view hashed for determinism. Field order = hash order (stable).
/// Vehicle integer state (line/arc-position/dir/dwell/onboard) is included so the
/// determinism test covers movement; render-only floats (x/y/angle) are excluded.
#[derive(Serialize)]
struct Canonical<'a> {
    clock_ms: i64,
    running: bool,
    stations: &'a [Station],
    lines: &'a [Line],
    veh_line: &'a [LineId],
    veh_path: &'a [u8],
    veh_s_mm: &'a [i64],
    veh_dir: &'a [i8],
    veh_dwell_ms: &'a [i64],
    veh_onboard: &'a [u16],
    ridership_total: u64,
    total_journey_ms: u64,
    journey_samples: u64,
    total_wait_ms: u64,
    wait_samples: u64,
    denied_boardings: u64,
    abandoned: u64,
    denied_at: &'a [u64],
    abandoned_at: &'a [u64],
    opex_accrued: i64,
    opex_rem: i64,
    /// Forge-Line buffers (fantasy, S7). Appended LAST so transit (empty slice) re-pins exactly once
    /// and every prior field keeps its byte offset. The µ-unit `forge_accum` remainder is excluded
    /// (derived/transient, like `spawn_accum`); only the integer stock is authoritative state.
    forge_stock: &'a [i64],
    /// Global tribute (fantasy, S7d) — the supply score, 0 for transit.
    tribute: i64,
    /// Legion authoritative state (fantasy, S8) — position/route/strength/target/state. Cartesian
    /// x/y are render-only (derived from `s_mm`), excluded. Empty for transit.
    army_line: &'a [LineId],
    army_path: &'a [u8],
    army_s_mm: &'a [i64],
    army_dir: &'a [i8],
    army_strength: &'a [i64],
    army_target: &'a [u32],
    army_state: &'a [u8],
    // #legion-ride-trains travel sub-state (ON-LINE model: 4 fields, all empty for a legion-free run ⇒
    // appended-bytes re-pin only). The legion always lives on its line's `s_mm` — WALKING advances it slow,
    // RIDING mirrors a boarded vehicle's `s_mm`; no off-rail endpoint fields needed.
    army_wait_line: &'a [i32],
    army_wait_dir: &'a [i8],
    army_riding_veh: &'a [i32],
    army_wait_until_ms: &'a [i64],
    /// Town resistance + conquest count (fantasy, S8b). Empty/0 for transit.
    town_value: &'a [i64],
    towns_captured: i64,
    /// Barracks flags + per-town bounties (fantasy, S8). Empty for transit.
    is_barracks: &'a [bool],
    bounty: &'a [i64],
    /// Decadence pressure (fantasy, S9). 0 for transit.
    decadence: i64,
    /// The spatial decadence tide (fantasy, S10b) — per-cell corruption over the baked board. Appended
    /// LAST so transit (empty slice) re-pins once then byte-identical. Empty for transit / demo arcadia.
    decadence_cells: &'a [i32],
    /// Unlocked-tech bitset (fantasy, S11). Appended LAST so transit + the arcadia golden (both 0) re-pin
    /// exactly once, then stay byte-identical. 0 ⇒ no tech ⇒ every effect takes its shipped-constant path.
    tech_unlocked: u32,
    /// The S11 economy-split channels (fantasy). Appended LAST — 0 for transit + the demo arcadia golden
    /// (no aether/ingot delivered), so the re-pin is appended zero bytes, behaviour byte-identical.
    mana: i64,
    manpower: i64,
    /// The S11 RIVAL — raider 2-D positions + state + spawn cadence/cursor (fantasy). Appended LAST —
    /// empty/0 for transit + demo arcadia (no reservoir ⇒ no raiders), so the re-pin is appended zero
    /// bytes, behaviour byte-identical. Free 2-D position IS the authority (off-rail) ⇒ hashed.
    raider_x_mm: &'a [i64],
    raider_y_mm: &'a [i64],
    raider_state: &'a [u8],
    raider_spawn_accum_ms: i64,
    raider_cursor: u32,
    raider_breach: i64,
    raider_breach_heal_accum: i64,
    /// Cumulative spells cast (fantasy, S11). Appended LAST — 0 for transit + the goldens (no SPELLCRAFT),
    /// so the re-pin is an appended zero, behaviour byte-identical.
    spells_cast: u32,
    /// Per-line rail-attack disable timers (#war). Appended LAST — EMPTY for transit + both goldens (zero
    /// raiders ⇒ no raid ⇒ the lazy Vec never grows), so the re-pin is the appended length-0 byte ONCE then
    /// behaviour-byte-identical forever. Hashed: a raided line freezes its trains (replay-causal).
    line_disabled_until_ms: &'a [i64],
    /// Raider march TARGETS (#war): the point each raider heads for — the capital (a breacher) or a supply
    /// line's seam (a SABOTEUR). Authoritative (the march + no-livelock are measured against it) ⇒ hashed.
    /// Appended LAST — EMPTY for transit + both goldens (no reservoir ⇒ no raiders), so the re-pin is the
    /// appended length-0 bytes, behaviour byte-identical.
    raider_tx_mm: &'a [i64],
    raider_ty_mm: &'a [i64],
    /// TTD L3 C1 — the AUTHORITATIVE, HASHED track-segment slab (the earned geometry-ownership flip). Each
    /// grid line's polyline is abstracted into topology-keyed segments that now OWN the geometry; a BOUND
    /// `Path` omits its geometry from the hash (above) and references segments by id, and the geometry lives
    /// HERE. Appended LAST. EMPTY (length-0 slice) for continuous / non-grid networks (transit + the demo
    /// arcadia fixture is single-line GRID, so arcadia DOES populate it ⇒ its golden moves; transit stays a
    /// pure empty-slice shift). Topology-pure: the projection hashes endpoint CELLS (not the index/seg_id)
    /// in canonical seg-order, so the hash is command-order-independent (same network ⇒ same hash).
    track_segments: CanonSegments<'a>,
    /// TTD L5a — player-placed block signals (the authoritative store). Appended LAST. EMPTY for transit +
    /// the arcadia golden (neither places a signal), so the re-pin is the appended length-0 sequence ONCE
    /// then byte-identical; motion is unaffected in L5a (the position fingerprint proves it). Kept sorted+
    /// deduped at the apply boundary ⇒ command-order-independent, integer-only ⇒ determinism-safe.
    signals: &'a [Signal],
}

/// TTD L3 C1 — the hashed projection of the authoritative segment slab. Hand-written `Serialize` so the
/// hash is a TOPOLOGY-PURE function of the segments (endpoint cells + owned geometry + track), never the
/// allocation/command order: `seg_id` (= the canonical index) and `shared` (a derived render hint) are
/// EXCLUDED. Empty slab ⇒ a length-0 sequence (the transit/continuous clean re-pin).
struct CanonSegments<'a>(&'a crate::track_graph::TrackGraph);

impl serde::Serialize for CanonSegments<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let g = self.0;
        let mut seq = s.serialize_seq(Some(g.segments.len()))?;
        for seg in &g.segments {
            // Topology-pure identity = the two endpoint node CELLS (canonical, a<=b); plus the owned
            // authoritative geometry + track. No seg_id / shared (index/derived). `cell` is `Axial`
            // (serde-serializable), polyline is `Vec<PointMm>`, the rest are integers — no float here.
            let a_cell = g.nodes.get(seg.a as usize).map(|n| n.cell);
            let b_cell = g.nodes.get(seg.b as usize).map(|n| n.cell);
            seq.serialize_element(&(
                a_cell,
                b_cell,
                &seg.cells,
                &seg.polyline,
                &seg.arclen_mm,
                seg.track_type,
                seg.span_mode,
            ))?;
        }
        seq.end()
    }
}

/// Save artifact: a seed plus the ordered command log. Replaying it reconstructs state
/// exactly (the determinism guarantee) and is the future lockstep-multiplayer transport.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveGame {
    pub seed: u64,
    /// The ruleset the save was played under (the fantasy-fork seam). Carried so `replay()` can
    /// refuse to reconstruct a fantasy save on a transit city (S3 guard) — a mismatched ruleset
    /// replays the command log against the wrong `World::apply` and silently diverges. Omitted
    /// older saves deserialize as `"transit"` (the serde default).
    #[serde(default = "crate::city::default_ruleset")]
    pub ruleset: String,
    pub commands: Vec<Command>,
}

impl World {
    pub fn new(seed: u64, city: CityData) -> Self {
        // Build the buildability lookup from the committed grid (div_euclid so negative mm,
        // e.g. west of a Calgary origin, index consistently with the cells' centres).
        let build_cell_mm = if city.buildability.cell_m > 0.0 {
            (city.buildability.cell_m * 1000.0) as i64
        } else {
            120_000
        };
        let mut build_lookup = rustc_hash::FxHashMap::default();
        for cell in &city.buildability.cells {
            build_lookup.insert(
                (
                    cell.x_mm.div_euclid(build_cell_mm) as i32,
                    cell.y_mm.div_euclid(build_cell_mm) as i32,
                ),
                cell.c,
            );
        }
        let max_legs = if city.max_legs == 0 {
            crate::routing::DEFAULT_MAX_LEGS
        } else {
            city.max_legs
        };
        // Growth ceiling: 2× the strongest initial cell (see the field doc). f32 max via fold —
        // index-ordered, deterministic.
        let growth_cap_w = city
            .demand
            .cells
            .iter()
            .fold(0.0f32, |m, c| m.max(c.origin_w).max(c.dest_w))
            * 2.0;
        // Mode toggle (S3): the frozen `ruleset` tag selects the game built here — transit today,
        // `"arcadia"` at S6. One dispatch point; both boxes are unhashed, so this is golden-neutral.
        let (ruleset, demand) = crate::ruleset::select(&city.ruleset);
        // Baked starting corruption (fantasy S4): a more-corrupt continent begins further up the lose
        // meter. 0 for every transit city / the golden fixture / native tests ⇒ byte-identical (zero
        // re-pins). Clamped ≥ 0 (a negative bake can't bank surplus).
        let initial_decadence = city.initial_decadence.max(0);
        // Fantasy economy: the realm's starting gold (seeds `tribute`). 0 for transit + golden fixtures.
        let city_initial_gold = city.initial_gold.max(0);
        // Fantasy S10: derive the decadence CA's static board (hex domain + creep gradient + reservoir)
        // from the baked terrain. Empty unless a baked world supplies buildability + a capital, so this
        // is golden-neutral (un-hashed; transit / the golden fixture build an empty field).
        let decadence_field = crate::decadence_field::DecadenceField::build(&city);
        World {
            decadence_cells: Vec::new(),
            decadence_gain_accum: 0,
            seed,
            clock_ms: 0,
            running: false,
            rng: ChaCha8Rng::seed_from_u64(seed),
            stations: Vec::new(),
            lines: Vec::new(),
            vehicles: VehicleSoA::default(),
            city,
            cmd_log: Vec::new(),
            dispatch_dirty: false,
            captured_origin: Vec::new(),
            captured_dest: Vec::new(),
            dest_by_comm: Vec::new(),
            has_multistage: false,
            station_commodity: Vec::new(),
            station_recipe: Vec::new(),
            spawn_accum: Vec::new(),
            forge_stock: Vec::new(),
            forge_accum: Vec::new(),
            walk_accum: Vec::new(),
            armies: crate::army::ArmySoA::default(),
            raiders: crate::raider::RaiderSoA::default(),
            raider_spawn_accum_ms: 0,
            raider_cursor: 0,
            raider_breach: 0,
            raider_breach_heal_accum: 0,
            line_disabled_until_ms: Vec::new(),
            town_value: Vec::new(),
            towns_captured: 0,
            is_barracks: Vec::new(),
            bounty: Vec::new(),
            decadence: initial_decadence,
            decadence_accum: 0,
            // Fantasy economy: the baked starting war-chest (0 for transit + golden fixtures ⇒ byte-identical).
            tribute: city_initial_gold,
            mana: 0,
            manpower: 0,
            spells_cast: 0,
            spell_flashes: Vec::new(),
            signal_occupancy: Vec::new(),
            signals: Vec::new(),
            autocast: false,
            tech_unlocked: 0,
            waiting: Vec::new(),
            ridership_total: 0,
            boardings: Vec::new(),
            alightings: Vec::new(),
            total_journey_ms: 0,
            journey_samples: 0,
            total_wait_ms: 0,
            wait_samples: 0,
            denied_boardings: 0,
            abandoned: 0,
            denied_at: Vec::new(),
            abandoned_at: Vec::new(),
            serving: Vec::new(),
            junctions: Vec::new(),
            cross_blocks: Vec::new(),
            track_graph: crate::track_graph::TrackGraph::default(),
            footpaths: Vec::new(),
            route_cache: rustc_hash::FxHashMap::default(),
            access_cache: rustc_hash::FxHashMap::default(),
            build_lookup,
            build_cell_mm,
            decadence_field,
            demand_dirty: false,
            last_growth_day: 0,
            last_upkeep_day: 0,
            growth_cap_w,
            economy_enabled: false,
            opex_accrued: 0,
            opex_rem: 0,
            router: Box::new(crate::routing::RaptorRouter),
            ruleset,
            demand,
            max_legs,
            agent_demand: false,
            population: None,
            cell_station_dirty: true,
            recent_alight: std::collections::VecDeque::new(),
        }
    }

    /// Buildability class at a local mm point (Open if outside the grid).
    pub fn classify(&self, x_mm: i64, y_mm: i64) -> u8 {
        let key = (
            x_mm.div_euclid(self.build_cell_mm) as i32,
            y_mm.div_euclid(self.build_cell_mm) as i32,
        );
        self.build_lookup.get(&key).copied().unwrap_or(crate::city::class::OPEN)
    }

    /// Per-segment (disruption, surface-water flag, TRACK capital) for a line's current
    /// geometry + span modes — no trainset cost. A pure read of `self` (the buildability grid)
    /// and the line, shared by `recompute_line_buildability` (the committed line) and
    /// `preview_line_cost` (a hypothetical one) so the cost formula is never duplicated.
    fn line_cost_metrics(&self, l: &Line) -> (i64, bool, i64) {
        use crate::city::class;
        use crate::line::mode;
        use crate::trainset::tmode;
        let tm = l.mode;
        let mut disr = 0i64;
        let mut water = false;
        let mut capital = 0i64;
        // Sum over every service path. Each branch path repeats the shared trunk prefix, so skip
        // that prefix (already costed by the trunk path) to avoid double-counting — only the
        // branch's OWN spans (past the divergence) add cost.
        for (pi, path) in l.paths.iter().enumerate() {
            let skip_to = if pi == 0 {
                0
            } else {
                let d = l.branches.get(pi - 1).map(|b| b.diverge_at as usize).unwrap_or(0);
                path.stop_arclen_mm.get(d).copied().unwrap_or(0)
            };
            for vi in 1..path.polyline.len() {
                if path.arclen_mm[vi] <= skip_to {
                    continue;
                }
                let seg_m = (path.arclen_mm[vi] - path.arclen_mm[vi - 1]) / 1000; // mm -> metres
                if seg_m <= 0 {
                    continue;
                }
                let span = path.span_of(path.arclen_mm[vi]);
                let m = path.span_mode.get(span).copied().unwrap_or(mode::SURFACE);
                let c = self.classify(path.polyline[vi].x_mm, path.polyline[vi].y_mm);
            // Per-mode placement: rail/bus blocked by water + penalised through built land;
            // ferry wants water (penalised over land, water is free); air is exempt.
            let (w, blocks_on_water): (i64, bool) = match tm {
                tmode::BUS => (
                    match c {
                        class::BUILT => 2, // buses run on city streets fine
                        class::WATER => 20,
                        class::PARK => 2,
                        _ => 0,
                    },
                    true,
                ),
                tmode::FERRY => (if c == class::WATER { 0 } else { 14 }, false), // must stay on water
                tmode::AIR => (0, false), // flies over anything
                _ => (
                    match c {
                        class::BUILT => 10,
                        class::WATER => 20,
                        class::PARK => 3,
                        _ => 0,
                    },
                    true,
                ),
            };
            let factor: i64 = match m {
                mode::ELEVATED => 25,
                mode::TUNNEL => 8,
                _ => 100, // Surface pays full
            };
            disr += w * seg_m * factor / 100;
            if blocks_on_water && c == class::WATER && m == mode::SURFACE {
                water = true;
            }
            // Capital per metre by transport mode (rail/heavy also by build-mode).
            let per_km = match tm {
                // Buses ride the existing ROAD network for free; off-road they build a busway.
                tmode::BUS => if c == class::ROAD { 0 } else { 3_000_000 },
                // Ferries cross open WATER for free (just terminals); forced over land they'd dig.
                tmode::FERRY => if c == class::WATER { 0 } else { 5_000_000 },
                // Air builds NO right-of-way — you buy aircraft (capital, added separately) and burn
                // fuel (opex). A per-km track charge would be astronomical at globe distances.
                tmode::AIR => 0,
                tmode::HEAVY => match m {
                    mode::ELEVATED => PER_KM_HSR_ELEVATED,
                    mode::TUNNEL => PER_KM_HSR_TUNNEL,
                    _ => PER_KM_HSR_SURFACE,
                },
                _ => match m {
                    mode::ELEVATED => PER_KM_ELEVATED,
                    mode::TUNNEL => PER_KM_TUNNEL,
                    _ => PER_KM_SURFACE,
                },
            };
            // P2: single track lays one rail pair instead of two — cheaper to build (the trade-off
            // is lower capacity: opposing trains must meet at passing places). Integer percent.
            let track_pct = if path.track_type.get(span).copied().unwrap_or(crate::line::track::DOUBLE)
                == crate::line::track::SINGLE
            {
                SINGLE_TRACK_PCT
            } else {
                100
            };
            // Fantasy TERRAIN multiplier (#terrain): rail through hills/forest/mountains/ley costs more —
            // route around the ridge or pay to cross it. ×100 (no change) for PLAIN + every transit class,
            // so existing cities + the golden fixtures (no biome codes ≥ 6) are byte-identical (golden-neutral).
            let terrain_pct = Self::terrain_capital_pct(c);
            capital += per_km * track_pct / 100 * terrain_pct / 100 * seg_m / 1000;
                // Surface track through built-up land takes land (rail + heavy rail).
                if (tm == tmode::RAIL || tm == tmode::HEAVY) && c == class::BUILT && m == mode::SURFACE {
                    capital += TAKING_PER_KM_BUILT * seg_m / 1000;
                }
            }
        }
        (disr, water, capital)
    }

    /// Recompute a line's disruption + water flag + capital from the buildability grid and its
    /// per-span build modes. Cheap (one pass over the polyline vertices); called on a geometry
    /// or mode change.
    fn recompute_line_buildability(&mut self, line: LineId) {
        let idx = line.index();
        if idx >= self.lines.len() {
            return;
        }
        // Fantasy/design: force SINGLE track on every span (no double-tracking) — one rail reads cleaner and
        // makes opposing trains MEET at passing places, so signalling matters. Done HERE (one chokepoint
        // after every geometry/track change) so the cost below + dispatch later both see SINGLE; the hashed
        // `track_type` becomes SINGLE for a baked-flag world. Off (the default) ⇒ untouched ⇒ byte-identical.
        if self.city.force_single_track {
            for p in self.lines[idx].paths.iter_mut() {
                for t in p.track_type.iter_mut() {
                    *t = crate::line::track::SINGLE;
                }
            }
        }
        // Per-span build modes live on each Path now (sized in Path::rebuild); nothing to size here.
        let (disr, water, mut capital) = self.line_cost_metrics(&self.lines[idx]);
        // Per-MODEL rolling-stock cost (depot rework Stage 1): RAIL reads its roster (Heavy pricier,
        // Express cheaper); every other mode keeps the flat TRAIN_COST. spec 0 ⇒ TRAIN_COST ⇒ byte-identical.
        let mode = self.lines[idx].mode;
        capital += self.lines[idx]
            .trainset
            .map(|t| t.count as i64 * crate::trainset::train_cost(mode, t.spec, TRAIN_COST))
            .unwrap_or(0);
        // TTD L5d: each player-placed block signal on this line adds a small capital cost. 0 signals ⇒
        // 0 added (the goldens place none ⇒ byte-identical). Counted from the authoritative store.
        let sig_count = self.signals.iter().filter(|s| s.line == line).count() as i64;
        capital += SIGNAL_COST * sig_count;
        self.lines[idx].disruption_units = disr;
        self.lines[idx].crosses_water_surface = water;
        self.lines[idx].capital_cost = capital;
    }

    /// Authoritative construction cost (track only, no trains) for a hypothetical line through
    /// the given station ids in `mode` — the cost-preview query for the build HUD, using the
    /// SAME formula as a committed line (no UI-side duplication; AGENTS "logic lives in core").
    /// Spans default to Surface (the draft is surface until grade-separated post-commit).
    pub fn preview_line_cost(&self, station_ids: &[u32], mode: u8, loop_line: bool) -> i64 {
        let pts: Vec<PointMm> = station_ids
            .iter()
            .filter_map(|&id| self.stations.get(id as usize))
            .filter(|s| !s.removed)
            .map(|s| s.pos)
            .collect();
        if pts.len() < 2 {
            return 0;
        }
        let mut l = Line::new(0, DEFAULT_HEADWAY_MS);
        l.mode = mode;
        l.loop_line = loop_line;
        l.rebuild_from_points(&pts, self.city.grid_cell_mm); // empty span_mode ⇒ every span defaults to Surface
        let (_disr, _water, capital) = self.line_cost_metrics(&l);
        capital
    }

    /// Best (shortest) headway among operational lines serving station `s`, if any.
    /// "Operational" = has a trainset, ≥2 stops, and no illegal surface-over-water span (a parked
    /// line serves nobody — same rule as `dispatch`).
    fn best_headway_at(&self, s: usize) -> Option<i64> {
        let mut best: Option<i64> = None;
        for l in &self.lines {
            if !l.removed
                && l.trainset.is_some()
                && l.stops.len() >= 2
                && !l.crosses_water_surface
                && l.paths.iter().any(|p| p.stops.iter().any(|st| st.index() == s))
            {
                best = Some(best.map_or(l.headway_ms, |b| b.min(l.headway_ms)));
            }
        }
        best
    }

    /// 0–100 coverage score: (quality-weighted origin demand served) / (the WHOLE city's origin
    /// demand), lifted by a square root so early networks register progress on a 0–100 dial.
    /// A served station contributes its captured demand weight scaled by the quality of its BEST
    /// (shortest-headway) line, where quality runs from 1.0 at the min headway down to a floor of
    /// 0.5 at the max headway.
    ///
    /// The denominator is the city total — NOT the captured total — so the gauge is a progression
    /// metric: it starts near 0, every newly served station raises it, and an unserved station
    /// leaves it unchanged (it can never read "done" off a single line). The fixed denominator +
    /// the 0.5 quality floor are what keep it MONOTONIC: extending coverage adds a non-negative
    /// term, shortening a headway only raises a station's quality, and sqrt is monotone — none of
    /// them can ever lower the score (PLAN §7). Scale anchors: serving ~30% of the city's demand
    /// at full quality reads ~55; ~64% reads 80; 100 means everything served at min headway.
    /// Run the demand model's `prepare` through the seam (the eager post-edit / on-demand catchment
    /// recompute). Take-out swap so the boxed model can borrow `&mut self` without aliasing the
    /// field; `NoopDemand` is a transient placeholder, never observed. `prepare` is idempotent
    /// (gated on `demand_dirty`) and writes only NON-hashed derived caches, so this is
    /// determinism-neutral — but routing it through the box keeps the seam the single demand path
    /// (a fantasy ruleset recomputes ITS eligibility here).
    pub(crate) fn demand_prepare(&mut self) {
        let mut d = std::mem::replace(&mut self.demand, Box::new(crate::ruleset::NoopDemand));
        d.prepare(self);
        self.demand = d;
    }

    pub(crate) fn coverage_score(&self) -> u8 {
        let total: f32 = self.city.demand.cells.iter().map(|c| c.origin_w).sum();
        if total <= 0.0 {
            return 0;
        }
        let span = (MAX_HEADWAY_MS - MIN_HEADWAY_MS).max(1) as f32;
        let mut served = 0.0f32;
        for (s, &w) in self.captured_origin.iter().enumerate() {
            if w <= 0.0 {
                continue;
            }
            if let Some(h) = self.best_headway_at(s) {
                let frac_h = ((h - MIN_HEADWAY_MS) as f32 / span).clamp(0.0, 1.0);
                let quality = 1.0 - 0.5 * frac_h; // [0.5, 1.0]
                served += w * quality;
            }
        }
        let frac = (served / total).clamp(0.0, 1.0);
        ((frac.sqrt() * 100.0).round() as i64).clamp(0, 100) as u8
    }

    /// Fantasy (arcadia) progress gauge (S11): the realm's standing = SUPPLY REACH (town demand on an
    /// operational line) blended with CONQUEST (towns held), 0–100. The fantasy analog of the transit
    /// coverage gauge — it answers "how much of the realm am I supplying + holding?" rather than the
    /// decadence gauge's "how close is the rot?". MONOTONIC by construction (the build plan's split-gauge
    /// invariant, one channel each): a superset network serves ≥ the same town sinks (supply term
    /// non-decreasing), and `towns_captured` only ever rises (conquest term non-decreasing) — so the
    /// score never falls. A derived READ (f32, never hashed), like `coverage_score`.
    pub(crate) fn arcadia_coverage_score(&self) -> u8 {
        let total_dest: f32 = self.city.demand.cells.iter().map(|c| c.dest_w).sum();
        let mut served = 0.0f32; // town DEST demand on an operational line
        let mut town_sinks = 0i64; // sink (town) stations — the conquest denominator
        for s in 0..self.stations.len() {
            if self.stations[s].removed {
                continue;
            }
            let cd = self.captured_dest.get(s).copied().unwrap_or(0.0);
            let co = self.captured_origin.get(s).copied().unwrap_or(0.0);
            if cd > co && cd > 0.0 {
                town_sinks += 1;
                if self.best_headway_at(s).is_some() {
                    served += cd;
                }
            }
        }
        let supply = if total_dest > 0.0 { (served / total_dest).clamp(0.0, 1.0) } else { 0.0 };
        let conquest = if town_sinks > 0 {
            (self.towns_captured as f32 / town_sinks as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // Supply is the bulk (the loop you run constantly); conquest is the territorial bonus.
        let blend = 0.65 * supply + 0.35 * conquest;
        ((blend.sqrt() * 100.0).round() as i64).clamp(0, 100) as u8
    }

    /// Citizen population for agent demand — scales with the city's residential weight so a bigger
    /// city has more commuters, capped so memory + the one-time route warmup stay bounded. Also
    /// scales with the in-game day length (each citizen commutes twice per day, so trips per
    /// sim-minute ∝ population / day length): the factor keeps felt intensity constant if
    /// `tod::HOUR_MS` is retuned. Tunable.
    pub(crate) fn agent_population_target(&self) -> usize {
        let homes: f64 = self.city.demand.cells.iter().map(|c| c.origin_w as f64).sum();
        let day_scale = crate::tod::HOUR_MS as f64 / 60_000.0; // 1.0 at the original 24-sim-min day
        // Floor kept low so a tiny/sparse city isn't swamped by agents it can't justify.
        ((homes * 14.0 * day_scale) as usize).clamp(1_000, (60_000.0 * day_scale) as usize)
    }

    /// Total one-time construction capital across all lines.
    fn capital_total(&self) -> i64 {
        self.lines.iter().filter(|l| !l.removed).map(|l| l.capital_cost).sum()
    }

    /// Fantasy (arcadia) #infrastructure — the set of stations REACHABLE BY RAIL from a holding. The
    /// realm's network must be ONE connected graph rooted at the capital: rail extends only from a
    /// station already wired to your seat (or to a town conquest has flipped). Two stations are
    /// "connected" iff they share a line — a line welds *all* its stops (trunk + every branch) into
    /// one component. Captured towns (`town_value == 0`) are extra roots, so each conquest opens a
    /// fresh frontier you may rail outward from.
    ///
    /// Returns station indices (`u32`). A pure read — it never mutates hashed state, so the gate stays
    /// golden-neutral. The `Set` is only `insert`/`contains` (never iterated → determinism-safe); the
    /// fixpoint walks `self.lines`/`stops` in index order. Callers gate `influence_hops > 0` realms.
    pub(crate) fn compute_rail_reachable(&self) -> rustc_hash::FxHashSet<u32> {
        let mut reach: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
        // Roots: the capital's co-located station + every captured town. The capital station sits at
        // exactly `(capital_x_mm, capital_y_mm)` (same `to_mm` bake), so a one-cell tolerance catches
        // it without reaching the ≥1.5-cell-distant neighbours (no accidental free-floating anchor).
        let size = self.city.grid_cell_mm.max(1) as i128;
        let tol2 = size * size;
        let (cap_x, cap_y) = (self.city.capital_x_mm, self.city.capital_y_mm);
        for (s, st) in self.stations.iter().enumerate() {
            if st.removed {
                continue;
            }
            let dx = (st.pos.x_mm - cap_x) as i128;
            let dy = (st.pos.y_mm - cap_y) as i128;
            let at_capital = dx * dx + dy * dy <= tol2;
            if at_capital || self.town_value.get(s).copied() == Some(0) {
                reach.insert(s as u32);
            }
        }
        // Fixpoint: a line touching the reachable set welds ALL its stops into it. Bounded by the
        // line count — each pass either grows `reach` or we stop. Index-ordered, Set-only ⇒ no
        // float / no map iteration in the determinism heart.
        loop {
            let mut changed = false;
            for line in &self.lines {
                if line.removed {
                    continue;
                }
                let touches = line.stops.iter().any(|s| reach.contains(&(s.index() as u32)))
                    || line
                        .branches
                        .iter()
                        .any(|b| b.stops.iter().any(|s| reach.contains(&(s.index() as u32))));
                if !touches {
                    continue;
                }
                for s in &line.stops {
                    changed |= reach.insert(s.index() as u32);
                }
                for b in &line.branches {
                    for s in &b.stops {
                        changed |= reach.insert(s.index() as u32);
                    }
                }
            }
            if !changed {
                break;
            }
        }
        reach
    }

    /// Fantasy (arcadia) #infrastructure — may a new `station` be wired onto `line`? Yes iff the realm
    /// is ungated (`influence_hops <= 0` — every transit city, both golden fixtures, native tests), OR
    /// the station is itself a holding/already-on-network, OR the line we're extending already touches
    /// the rail-reachable network. The "a fresh line must start at a holding" rule falls out for free:
    /// an empty line touches nothing, so its first stop must itself be a root (capital / captured town).
    ///
    /// Pure read — recomputes reachability from the command-sourced graph; never touches hashed state.
    pub(crate) fn connected_can_add(&self, line_idx: usize, station_idx: usize) -> bool {
        if self.city.influence_hops <= 0 {
            return true;
        }
        let reach = self.compute_rail_reachable();
        if reach.contains(&(station_idx as u32)) {
            return true;
        }
        let l = &self.lines[line_idx];
        l.stops.iter().any(|s| reach.contains(&(s.index() as u32)))
            || l.branches
                .iter()
                .any(|b| b.stops.iter().any(|s| reach.contains(&(s.index() as u32))))
    }

    /// Rail-attack (#war): is `line` currently RAIDED (its disable timer still ahead of the clock)? The
    /// timer Vec is lazily grown, so an absent index reads 0 ⇒ never frozen (transit + goldens take this
    /// path for every line). A pure read.
    pub fn line_disabled(&self, line: usize) -> bool {
        self.line_disabled_until_ms.get(line).copied().unwrap_or(0) > self.clock_ms
    }

    /// Rail-attack (#war): CUT `line` until `until_ms`, lazily growing the timer Vec so it stays EMPTY
    /// until the first raid (golden-neutral). Returns true iff it cut a fresh/operational line (false for
    /// an out-of-range or already-more-disabled line — a raider that hits an already-raided line is wasted).
    pub(crate) fn disable_line(&mut self, line: usize, until_ms: i64) -> bool {
        if line >= self.lines.len() {
            return false;
        }
        if self.line_disabled_until_ms.len() <= line {
            self.line_disabled_until_ms.resize(line + 1, 0);
        }
        if self.line_disabled_until_ms[line] >= until_ms {
            return false;
        }
        self.line_disabled_until_ms[line] = until_ms;
        true
    }

    /// Current money: start budget + fares − capital − opex. Negative = over budget.
    fn balance(&self) -> i64 {
        START_BUDGET + self.ridership_total as i64 * FARE - self.capital_total() - self.opex_accrued
    }

    /// After a capital-changing mutation + recompute: true iff the economy is on AND the change
    /// raised capital AND drove the balance negative — i.e. the player can't afford it. The
    /// caller must then restore the pre-command state (the afford-gate; clamps live in the core).
    fn overspent(&self, old_capital: i64) -> bool {
        self.economy_enabled && self.capital_total() > old_capital && self.balance() < 0
    }

    /// Fantasy (#terrain): per-segment capital MULTIPLIER (percent) by terrain. Hills/forest/mountains/ley
    /// cost more to lay rail through; PLAIN and EVERY transit cost-class stay ×100 (`(x*100)/100 == x`,
    /// exact integer identity ⇒ existing cities + both golden fixtures byte-identical). Only the fantasy
    /// hex grid carries biome codes ≥ 6, so this is golden-neutral. WATER keeps ×100 — its expense is the
    /// existing water-crossing path (parked unless Elevated/Tunnel), not this multiplier.
    fn terrain_capital_pct(c: u8) -> i64 {
        use crate::city::biome;
        match c {
            biome::MOUNTAIN => 320, // a ridge to blast/tunnel through
            biome::HILL => 190,     // grading + cuttings
            biome::FOREST => 140,   // timber to clear
            biome::LEY => 130,      // unstable arcane ground
            _ => 100,               // PLAIN / OPEN / ROAD / RAIL / BUILT / WATER / PARK — unchanged
        }
    }

    /// The GOLD price of a capital delta under the fantasy build economy: `delta / build_gold_divisor`,
    /// or 0 when off (not arcadia, divisor 0, or capital didn't rise). The shared transit cost formula
    /// ($-scale, now terrain-aware) divided down to the small-integer gold scale.
    fn build_gold_cost(&self, delta_capital: i64) -> i64 {
        if delta_capital <= 0 || self.city.build_gold_divisor <= 0 {
            return 0;
        }
        if crate::ruleset::canon(&self.city.ruleset) != "arcadia" {
            return 0;
        }
        delta_capital / self.city.build_gold_divisor
    }

    /// Unified post-mutation afford-gate for a capital-RAISING command. Returns true ⇒ the caller must
    /// restore the pre-command state (rejected). On the affordable path it CHARGES the cost:
    ///  • Fantasy (arcadia + `build_gold_divisor` > 0): spends `build_gold_cost(delta)` from `tribute`;
    ///    rejects (no spend) if the realm can't afford it.
    ///  • Transit ($ economy on): the classic `overspent` budget check (no running spend — `balance()`
    ///    derives from `capital_total()`).
    ///  • Otherwise (building is free): never rejects.
    fn cannot_afford(&mut self, old_capital: i64) -> bool {
        // Fantasy gold economy takes precedence in arcadia.
        let gold = self.build_gold_cost(self.capital_total() - old_capital);
        if gold > 0 {
            if gold > self.tribute {
                return true; // can't afford — caller restores
            }
            self.tribute -= gold;
            return false;
        }
        // Transit budget gate (unchanged).
        self.overspent(old_capital)
    }

    /// Accrue recurring maintenance (opex) for one running step. Exact integer accrual via a
    /// sub-day remainder; only charged while the economy is enabled. Deterministic.
    fn accrue_opex(&mut self, dt_ms: i64) {
        if !self.economy_enabled || dt_ms <= 0 {
            return;
        }
        let trains: i64 = self.lines.iter().filter(|l| !l.removed).filter_map(|l| l.trainset).map(|t| t.count as i64).sum();
        let km: i64 = self.lines.iter().filter(|l| !l.removed).map(|l| l.length_mm() / 1_000_000).sum();
        let rate_per_day = trains * OPEX_PER_TRAIN_DAY + km * OPEX_PER_KM_DAY;
        self.opex_rem += rate_per_day * dt_ms;
        self.opex_accrued += self.opex_rem / DAY_MS;
        self.opex_rem %= DAY_MS;
    }

    /// Fantasy gold UPKEEP (#economy): once per in-game day, drain the realm treasury by the cost of
    /// keeping the network running (track-km + rolling stock). The opex axis that makes a sprawling
    /// network a tradeoff — you must keep DELIVERING to cover what you've built. Floored at 0 (no gold
    /// debt; unpaid upkeep simply empties the treasury). Gated on arcadia + a baked rate (0 ⇒ free to
    /// run ⇒ transit + goldens byte-identical). `last_upkeep_day` is clock-derived + un-hashed, so this
    /// only ever mutates the already-hashed `tribute` — golden-neutral. The current daily figure is read
    /// back for the HUD via `gold_upkeep_daily()`.
    fn accrue_gold_upkeep(&mut self) {
        let rate = self.city.gold_upkeep_per_day;
        if rate <= 0 || crate::ruleset::canon(&self.city.ruleset) != "arcadia" {
            return;
        }
        let day = self.clock_ms / (24 * crate::tod::HOUR_MS);
        if day <= self.last_upkeep_day {
            return;
        }
        // Charge every day boundary crossed since the last charge (catches a multi-day tick step).
        let days = day - self.last_upkeep_day;
        self.last_upkeep_day = day;
        let owed = self.gold_upkeep_daily().saturating_mul(days);
        self.tribute = (self.tribute - owed).max(0);
    }

    /// The per-in-game-day gold upkeep the current network owes (track-km + rolling stock × the baked
    /// rate). 0 when upkeep is off or not arcadia. A pure read — the HUD shows it, the drain charges it.
    pub fn gold_upkeep_daily(&self) -> i64 {
        let rate = self.city.gold_upkeep_per_day;
        if rate <= 0 {
            return 0;
        }
        let trains: i64 = self.lines.iter().filter(|l| !l.removed).filter_map(|l| l.trainset).map(|t| t.count as i64).sum();
        let km: i64 = self.lines.iter().filter(|l| !l.removed).map(|l| l.length_mm() / 1_000_000).sum();
        (km + trains * GOLD_UPKEEP_TRAIN_KM) * rate / GOLD_UPKEEP_DIVISOR
    }

    /// Charge opex for one running tick (called from the tick phase loop).
    pub(crate) fn tick_economy(&mut self, dt_ms: i64) {
        self.accrue_opex(dt_ms);
        self.accrue_gold_upkeep();
    }

    /// Low-frequency structured readout for the UI (the wasm->ts query port).
    pub fn stats_snapshot(&self) -> StatsSnapshot {
        let waiting_total: u64 = self.waiting.iter().map(|q| q.len() as u64).sum();

        // Average load factor across vehicles (onboard / capacity), plus a per-line mean for the
        // line-inspect strain readout — same single pass, re-binned by line index (no new state).
        let mut load_sum = 0.0f32;
        let mut load_n = 0u32;
        let mut line_load_sum = vec![0.0f32; self.lines.len()];
        let mut line_load_n = vec![0u32; self.lines.len()];
        for i in 0..self.vehicles.len() {
            let li = self.vehicles.line[i].index();
            if let Some(l) = self.lines.get(li) {
                if l.trainset.is_some() {
                    // vehicle_spec, not spec_for_mode: the assigned aircraft (or future roster
                    // entry) sets the capacity this load factor is measured against.
                    let cap = l.vehicle_spec().capacity.max(1) as f32;
                    let lf = self.vehicles.onboard[i] as f32 / cap;
                    load_sum += lf;
                    load_n += 1;
                    line_load_sum[li] += lf;
                    line_load_n[li] += 1;
                }
            }
        }
        let avg_load_factor = if load_n > 0 { load_sum / load_n as f32 } else { 0.0 };

        // #infrastructure: the rail-reachable set (one fixpoint) so each StationStat can flag whether the
        // player may extend rail FROM it — the frontier overlay reads this authoritative set (zero drift
        // from the gate). Empty when the realm is ungated (transit/golden) — a pure, non-hashed read.
        let rail_reach = if self.city.influence_hops > 0 {
            self.compute_rail_reachable()
        } else {
            rustc_hash::FxHashSet::default()
        };

        let per_station = (0..self.stations.len())
            .filter(|&s| !self.stations[s].removed)
            .map(|s| StationStat {
                station_id: s as u32,
                boardings: *self.boardings.get(s).unwrap_or(&0) as f64,
                alightings: *self.alightings.get(s).unwrap_or(&0) as f64,
                waiting: self.waiting.get(s).map(|q| q.len()).unwrap_or(0) as f64,
                demand_origin: *self.captured_origin.get(s).unwrap_or(&0.0) as f64,
                demand_dest: *self.captured_dest.get(s).unwrap_or(&0.0) as f64,
                serving: self.serving.get(s).map(|v| v.len()).unwrap_or(0) as u32,
                denied: *self.denied_at.get(s).unwrap_or(&0) as f64,
                abandoned: *self.abandoned_at.get(s).unwrap_or(&0) as f64,
                town_resistance: *self.town_value.get(s).unwrap_or(&0) as f64,
                // #war legibility: the FULL garrison (for the siege-progress ring) + the barracks flag (for
                // the ⚔ spawn-node badge). garrison_resistance is the depth-scaled full HP; only meaningful
                // for towns (a sink with resistance), 0 elsewhere; both are pure render readouts.
                garrison_max: if self.captured_dest.get(s).copied().unwrap_or(0.0) > self.captured_origin.get(s).copied().unwrap_or(0.0) {
                    crate::army::garrison_resistance(self, s) as f64
                } else {
                    0.0
                },
                is_barracks: self.is_barracks.get(s).copied().unwrap_or(false),
                buffer_fill: {
                    // The fullest of this node's commodity buffers, normalised by BUFFER_CAP (snapshot of
                    // the hashed forge_stock; render-only). 0 for transit (forge_stock empty).
                    let base = s * crate::forge::N_COMMODITIES;
                    let cap = crate::forge::BUFFER_CAP.max(1) as f64;
                    (base..base + crate::forge::N_COMMODITIES)
                        .filter_map(|i| self.forge_stock.get(i))
                        .map(|&v| v as f64 / cap)
                        .fold(0.0, f64::max)
                        .min(1.0)
                },
                // #infrastructure: captured-holding flag, the EXACT mirror of the root test in
                // `compute_rail_reachable` — true only once siege grinds a town's garrison to 0 (a
                // captured town becomes a new rail root; None before the war ticks ⇒ false).
                captured: self.town_value.get(s) == Some(&0),
                // #infrastructure: rail-reachable from the capital ⇒ the player may extend rail from here.
                reachable: rail_reach.contains(&(s as u32)),
            })
            .collect();

        let per_line = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.removed)
            .map(|(i, l)| {
                let ridership: u64 = l
                    .stops
                    .iter()
                    .map(|st| *self.boardings.get(st.index()).unwrap_or(&0))
                    .sum();
                LineStat {
                    line_id: i as u32,
                    name: l.name.clone(),
                    mode: l.mode,
                    color: l.color,
                    ridership: ridership as f64,
                    stops: l.stops.len() as u32,
                    trains: l.trainset.map(|t| t.count as u32).unwrap_or(0),
                    trainset_spec: l.trainset.map(|t| t.spec).unwrap_or(0),
                    headway_ms: l.headway_ms as f64,
                    disruption: l.disruption_units as f64,
                    crosses_water: l.crosses_water_surface,
                    capital_cost: l.capital_cost as f64,
                    load_factor: if line_load_n[i] > 0 { line_load_sum[i] / line_load_n[i] as f32 } else { 0.0 },
                }
            })
            .collect();

        // Economy: balance = start budget + fares − capital − opex (informational when off).
        let capital_spent = self.capital_total();
        let fare_revenue: i64 = self.ridership_total as i64 * FARE;
        let balance = self.balance();

        // Build impact: total disruption per km of track, mapped to 0..100 (lower is better).
        let total_disr: i64 = self.lines.iter().filter(|l| !l.removed).map(|l| l.disruption_units).sum();
        let total_track_m: i64 = self.lines.iter().filter(|l| !l.removed).map(|l| l.length_mm() / 1000).sum();
        let build_difficulty =
            ((total_disr * 5 / total_track_m.max(1)).clamp(0, 100)) as u8;

        StatsSnapshot {
            sim_clock_ms: self.clock_ms as f64,
            running: self.running,
            station_count: self.stations.iter().filter(|s| !s.removed).count() as u32,
            line_count: self.lines.iter().filter(|l| !l.removed).count() as u32,
            vehicle_count: self.vehicles.len() as u32,
            ridership_total: self.ridership_total as f64,
            waiting_total: waiting_total as f64,
            left_behind: self.denied_boardings as f64,
            denied_boardings: self.denied_boardings as f64,
            abandoned: self.abandoned as f64,
            avg_journey_ms: if self.journey_samples > 0 {
                self.total_journey_ms as f64 / self.journey_samples as f64
            } else {
                0.0
            },
            avg_wait_ms: if self.wait_samples > 0 {
                self.total_wait_ms as f64 / self.wait_samples as f64
            } else {
                0.0
            },
            avg_load_factor,
            coverage_score: self.ruleset.coverage_score(self),
            sim_hour: crate::tod::hour_of_day(self.clock_ms),
            period: crate::tod::period_label(crate::tod::hour_of_day(self.clock_ms)).to_string(),
            demand_multiplier: crate::tod::demand_multiplier(crate::tod::hour_of_day(self.clock_ms)) as f64,
            sim_day: (self.clock_ms / (24 * crate::tod::HOUR_MS)).clamp(0, u32::MAX as i64) as u32,
            demand_origin_total: self.city.demand.cells.iter().map(|c| c.origin_w as f64).sum(),
            build_difficulty,
            economy_enabled: self.economy_enabled,
            balance: balance as f64,
            capital_spent: capital_spent as f64,
            fare_revenue: fare_revenue as f64,
            opex_spent: self.opex_accrued as f64,
            per_station,
            per_line,
            ruleset: crate::ruleset::canon(&self.city.ruleset).to_string(),
            tribute: self.tribute as f64,
            mana: self.mana as f64,
            manpower: self.manpower as f64,
            decadence: self.decadence as f64,
            decadence_pct: crate::decadence::pct(self),
            towns_captured: self.towns_captured as f64,
            army_count: self.armies.len() as u32,
            // #war legibility: AFIELD = not DONE (the honest active-force count; army_count inflates with
            // permanent inert garrisons). DONE = crate::army::DONE.
            army_afield: self.armies.state.iter().filter(|&&s| s != crate::army::DONE).count() as u32,
            raider_count: self.raiders.live() as u32,
            // #war legibility: the raider-breach pressure surfaced on its own (distinct from tide creep).
            raider_breach: self.raider_breach as f64,
            raider_breach_pct: (self.raider_breach as f64 / crate::decadence::CAPITAL_THRESHOLD as f64 * 100.0).clamp(0.0, 100.0),
            realm_lost: crate::decadence::is_lost(self),
            tech_unlocked: self.tech_unlocked,
            spells_cast: self.spells_cast,
            autocast: self.autocast,
            build_gold_divisor: self.city.build_gold_divisor.max(0) as f64,
            gold_upkeep_daily: self.gold_upkeep_daily() as f64,
        }
    }

    fn station_pos(&self, id: StationId) -> PointMm {
        self.stations
            .get(id.index())
            .map(|s| s.pos)
            .unwrap_or(PointMm::new(0, 0))
    }

    /// Terrain build/traverse cost for a hex `cell` (TTD #cost-routing): of two same-length one-bend hex
    /// corners, `hexgrid::line_costed` takes the cheaper, swinging AROUND water/mountains toward plains.
    /// Single source for BOTH the grid line router (`rebuild_line_geometry`) AND the free-marching raiders
    /// (`raider.rs`), so a raider routes the same terrain a line does instead of cutting straight through.
    /// Flat (100) with no raster (`build_cell_mm <= 0`) ⇒ the router falls back to the first corner.
    pub(crate) fn terrain_cost(&self, cell: crate::hexgrid::Axial) -> i64 {
        use crate::city::biome;
        let bcm = self.build_cell_mm;
        if bcm <= 0 {
            return 100;
        }
        let pt = crate::hexgrid::center_of(cell, self.city.grid_cell_mm);
        match self.build_lookup.get(&(pt.x_mm.div_euclid(bcm) as i32, pt.y_mm.div_euclid(bcm) as i32)).copied().unwrap_or(0u8) {
            biome::WATER => 800,    // under-water tunnelling — very dear; route around it
            biome::MOUNTAIN => 320, // blast/tunnel a ridge
            biome::HILL => 190,
            biome::FOREST => 140,
            biome::LEY => 130,
            _ => 100, // plain / open
        }
    }

    fn rebuild_line_geometry(&mut self, line: LineId) {
        // Build ONE smoothed Path per service route (trunk + each branch's trunk-prefix→leaf). Buses
        // follow the ROAD raster between stops and ferries follow WATER (auto-routed A*); other modes
        // use the player's hand-placed waypoints (trunk only — branch waypoint editing deferred).
        let idx = line.index();
        if idx >= self.lines.len() {
            return;
        }
        use crate::trainset::tmode;
        let lmode = self.lines[idx].mode;
        let waypoints = self.lines[idx].waypoints.clone();
        let branches = self.lines[idx].branches.clone();
        let specs = self.lines[idx].path_specs();
        let corridor = match lmode {
            tmode::BUS => Some(crate::city::class::ROAD),
            tmode::FERRY => Some(crate::city::class::WATER),
            _ => None,
        };
        // Preserve player-set per-span build modes + track types across the rebuild (by path index).
        let old_span_modes: Vec<Vec<u8>> =
            self.lines[idx].paths.iter().map(|p| p.span_mode.clone()).collect();
        let old_track_types: Vec<Vec<u8>> =
            self.lines[idx].paths.iter().map(|p| p.track_type.clone()).collect();
        // Terrain build-cost per hex cell, for the grid one-bend router (line.rs grid_walk): of the two
        // same-length one-bend corners, it lays track along the cheaper one — swinging around water and
        // mountains toward plains. Deterministic (the buildability grid is hashed CityData); flat (100)
        // when there's no terrain raster, so the router falls back to the first corner (see `terrain_cost`).
        let gcm = self.city.grid_cell_mm;
        let mut new_paths: Vec<crate::line::Path> = Vec::with_capacity(specs.len());
        for (pi, (stops, loop_line)) in specs.into_iter().enumerate() {
            let pts: Vec<PointMm> = stops.iter().map(|&s| self.station_pos(s)).collect();
            let span_points: Vec<Vec<PointMm>> = if let Some(prefer) = corridor {
                (0..pts.len().saturating_sub(1))
                    .map(|i| crate::roadnav::class_route(&self.build_lookup, self.build_cell_mm, prefer, pts[i], pts[i + 1]))
                    .collect()
            } else if pi == 0 {
                waypoints.clone()
            } else if let Some(b) = branches.get(pi - 1) {
                // Branch path = trunk prefix + the branch. Reuse the TRUNK's waypoints for the shared
                // prefix spans (so the spur matches the trunk exactly up to the divergence), then the
                // branch's own per-span waypoints for the spur.
                let d = (b.diverge_at as usize).min(waypoints.len());
                let mut sp: Vec<Vec<PointMm>> = waypoints[..d].to_vec();
                sp.extend(b.waypoints.iter().cloned());
                sp
            } else {
                Vec::new()
            };
            let mut p = crate::line::Path::new(stops, loop_line);
            p.literal = self.lines[idx].literal;
            if let Some(sm) = old_span_modes.get(pi) {
                p.span_mode = sm.clone();
            }
            if let Some(tt) = old_track_types.get(pi) {
                p.track_type = tt.clone();
            }
            // The grid one-bend router's terrain cost (single source — same fn the raiders route with).
            let cost = |cell: crate::hexgrid::Axial| -> i64 { self.terrain_cost(cell) };
            p.rebuild(&pts, &span_points, gcm, &cost);
            new_paths.push(p);
        }
        self.lines[idx].paths = new_paths;
    }

    /// TTD L3 C1 — rebuild the AUTHORITATIVE, HASHED segment slab from the current lines, in the apply
    /// write-path (so `state_hash` reflects it even before a tick). Derives the `track_graph` (its
    /// `segments` ARE the hashed slab), binds each path's ordered `segments`, and — for grid (bound) paths
    /// — re-derives the runtime polyline FROM those segments (so geometry genuinely lives in the slab, not
    /// on `Path`). A pure function of `lines`/`stations` (no rng, no clock, integer + the pre-existing
    /// circumradius float in the geometry build), so replaying the same log rebuilds it bit-for-bit. Empty
    /// for continuous / non-grid networks ⇒ the hashed slab is a length-0 slice (transit's clean re-pin).
    pub(crate) fn rebuild_track_segments(&mut self) {
        self.track_graph = crate::track_graph::derive_track_graph(self);
        crate::dispatch::bind_path_segments(self);
    }

    /// Apply one command. Total + infallible: invalid commands return a `Rejected` event
    /// rather than panicking. Always records the command in the log.
    pub fn apply(&mut self, cmd: &Command) -> Vec<Event> {
        // Mode gate (S3 disjoint-save guard): reject a command not meaningful in this ruleset BEFORE
        // it mutates state or joins the save log, so a cross-mode command never pollutes a save. The
        // transit default accepts every existing Command (golden-neutral — no early return today).
        if let Err(reason) = self.ruleset.validate(cmd) {
            return vec![Event::Rejected { reason }];
        }
        // S11 RAIL-GATE (arcadia): the realm builds RAIL only; bus/ferry/plane are not available, and HEAVY
        // rail (mode 4) unlocks via the HEAVY_RAIL tech. Done here (not in `ruleset.validate`) because it
        // depends on `tech_unlocked`, which `validate` can't see. Transit is untouched (all modes allowed).
        if crate::ruleset::canon(&self.city.ruleset) == "arcadia" {
            if let Command::CreateLine { mode, .. } = cmd {
                let allowed =
                    *mode == 0 || (*mode == 4 && crate::tech::is_unlocked(self.tech_unlocked, crate::tech::HEAVY_RAIL));
                if !allowed {
                    return vec![Event::Rejected {
                        reason: "Arcadia builds rail only — heavy rail needs the Heavy Rail tech".into(),
                    }];
                }
            }
        }
        let events = match cmd {
            Command::PlaceStation { x_mm, y_mm, name } => {
                let id = StationId(self.stations.len() as u32);
                let name = name
                    .clone()
                    .unwrap_or_else(|| format!("Station {}", id.0 + 1));
                self.stations
                    .push(Station::new(PointMm::new(*x_mm, *y_mm), name.clone()));
                self.demand_dirty = true; // catchment capture must recompute
                vec![Event::StationPlaced { id, name }]
            }
            Command::PlaceBarracks { x_mm, y_mm, name } => {
                // A barracks IS a station (reuses the node/route substrate) + a flag. Armies launch
                // only from a barracks on a built route, so this is the player's prerequisite for war.
                let id = StationId(self.stations.len() as u32);
                let name = name.clone().unwrap_or_else(|| format!("Barracks {}", id.0 + 1));
                self.stations.push(Station::new(PointMm::new(*x_mm, *y_mm), name.clone()));
                while self.is_barracks.len() < self.stations.len() {
                    self.is_barracks.push(false);
                }
                self.is_barracks[id.index()] = true;
                self.demand_dirty = true;
                vec![Event::BarracksPlaced { id, name }]
            }
            Command::PostBounty { station, amount } => {
                let s = station.index();
                if s >= self.stations.len() || self.stations[s].removed {
                    vec![Event::Rejected { reason: "PostBounty: unknown station".into() }]
                } else if *amount > 0 && self.tribute < BOUNTY_COST {
                    // V3: posting a bounty costs GOLD (a decree on the treasury); clearing (0) is free.
                    vec![Event::Rejected { reason: "PostBounty: not enough gold".into() }]
                } else {
                    if *amount > 0 {
                        self.tribute -= BOUNTY_COST;
                    }
                    while self.bounty.len() < self.stations.len() {
                        self.bounty.push(0);
                    }
                    self.bounty[s] = (*amount).max(0); // clamp ≥ 0; 0 clears the bounty
                    vec![Event::BountyPosted { station: *station, amount: self.bounty[s] }]
                }
            }
            Command::UnlockTech { tech } => {
                // Buy a tech with MANA (the sole tech resource, S11). Validate id, refuse a repeat (bit set),
                // require the PREREQ (tier gate), then afford-gate against mana — so the spend is exactly once
                // and never drives mana negative. Reject (no mutation) on any failure.
                let id = *tech as usize;
                let ch = crate::tech::TECH_CHANNEL;
                match crate::tech::TECHS.get(id).copied() {
                    None => vec![Event::Rejected { reason: "UnlockTech: unknown tech".into() }],
                    Some(t) if self.tech_unlocked & (1u32 << t.bit) != 0 => {
                        vec![Event::Rejected { reason: "UnlockTech: already unlocked".into() }]
                    }
                    Some(_) if !crate::tech::prereq_met(self.tech_unlocked, id) => {
                        vec![Event::Rejected { reason: "UnlockTech: prerequisite not unlocked".into() }]
                    }
                    Some(t) if ch.balance(self) < t.cost => {
                        vec![Event::Rejected { reason: "UnlockTech: not enough mana".into() }]
                    }
                    Some(t) => {
                        ch.spend(self, t.cost);
                        self.tech_unlocked |= 1u32 << t.bit;
                        vec![Event::TechUnlocked { tech: *tech, balance_left: ch.balance(self) }]
                    }
                }
            }
            Command::CastSpell { kind } => {
                // Player-triggered, engine-targeted spell (S11). Gate on SPELLCRAFT (the spell arm tech);
                // `spell::cast` auto-targets + spends mana, or returns false (no mutation) when mana is short
                // or no valid target exists. The mana spend is the live tradeoff against teching (one pool).
                if !crate::tech::is_unlocked(self.tech_unlocked, crate::tech::SPELLCRAFT) {
                    vec![Event::Rejected { reason: "CastSpell: the spell arm needs the Arcane Awakening tech".into() }]
                } else if crate::spell::cast(self, *kind) {
                    vec![Event::SpellCast { kind: *kind, balance_left: self.mana }]
                } else {
                    vec![Event::Rejected { reason: "CastSpell: not enough mana, or no valid target".into() }]
                }
            }
            Command::SetAutocast { enabled } => {
                self.autocast = *enabled;
                vec![Event::AutocastSet { enabled: *enabled }]
            }
            Command::BuildPlatforms { station, k } => {
                // TTD L2: set the station's berth count (clamped in the CORE, never the UI). K berths ⇒ K
                // parallel dwells. K=1 is the default/no-op (byte-identical to pre-L2). Validate-then-mutate.
                let s = station.index();
                if s >= self.stations.len() || self.stations[s].removed {
                    vec![Event::Rejected { reason: "BuildPlatforms: unknown station".into() }]
                } else {
                    let kk = (*k).clamp(1, crate::station::MAX_PLATFORMS as u16) as u8;
                    self.stations[s].platform_count = kk;
                    vec![Event::PlatformsBuilt { station: *station, k: kk as u16 }]
                }
            }
            Command::PlaceSignal { line, path, span, at_mm } => {
                // TTD L5a — record a player block signal. Validate the (line,path,span) exists and `at_mm`
                // is STRICTLY inside that span's arc-length range, then insert into the canonically
                // sorted+deduped store (so the hash is command-order-independent). Recorded only — L5b
                // makes it re-key occupancy. Idempotent: placing the same signal twice is a no-op.
                match self.signal_span_bounds(*line, *path, *span) {
                    Some((lo, hi)) if *at_mm > lo && *at_mm < hi => {
                        let sig = Signal { line: *line, path: *path, span: *span, at_mm: *at_mm };
                        match self.signals.binary_search_by(|s| signal_key(s).cmp(&signal_key(&sig))) {
                            Ok(_) => {} // already present ⇒ no-op (keeps the store deduped)
                            Err(pos) => self.signals.insert(pos, sig),
                        }
                        // TTD L5d: refresh the line's capital (signals carry a cost). PlaceSignal is
                        // dispatch-exempt (no train reset), so the cost is recomputed here directly — it
                        // touches only disruption/water/capital, never the vehicle SoA.
                        self.recompute_line_buildability(*line);
                        vec![Event::SignalPlaced { line: *line, path: *path, span: *span, at_mm: *at_mm }]
                    }
                    _ => vec![Event::Rejected { reason: "PlaceSignal: signal must lie strictly inside an existing span".into() }],
                }
            }
            Command::RemoveSignal { line, path, span, at_mm } => {
                let sig = Signal { line: *line, path: *path, span: *span, at_mm: *at_mm };
                if let Ok(pos) = self.signals.binary_search_by(|s| signal_key(s).cmp(&signal_key(&sig))) {
                    self.signals.remove(pos);
                    self.recompute_line_buildability(*line); // TTD L5d: refund the signal's capital cost
                }
                vec![Event::SignalRemoved { line: *line, path: *path, span: *span, at_mm: *at_mm }]
            }
            Command::CreateLine { color, name, loop_line, mode, literal } => {
                let id = LineId(self.lines.len() as u32);
                let mut l = Line::new(*color, DEFAULT_HEADWAY_MS);
                l.name = name.clone().unwrap_or_else(|| format!("Line {}", id.0 + 1));
                l.loop_line = *loop_line;
                l.mode = *mode;
                l.literal = *literal;
                self.lines.push(l);
                vec![Event::LineCreated { id }]
            }
            Command::AddStop {
                line,
                station,
                after,
            } => {
                let valid_line = line.index() < self.lines.len();
                let valid_station = station.index() < self.stations.len();
                if valid_line && valid_station && !self.connected_can_add(line.index(), station.index()) {
                    // #infrastructure connected-rail gate (arcadia): the network must be ONE graph rooted
                    // at the capital — you can only extend rail from a station already on it (or from a
                    // captured town). Pure read before any mutation; 0-hops cities (transit/golden) skip it.
                    vec![Event::Rejected {
                        reason: "Connect this to your rail network — extend from your capital or a captured town".into(),
                    }]
                } else if valid_line && valid_station {
                    let old_capital = self.capital_total();
                    let saved_stops = self.lines[line.index()].stops.clone();
                    {
                        let l = &mut self.lines[line.index()];
                        match after {
                            Some(i) if *i <= l.stops.len() => l.stops.insert(*i, *station),
                            _ => l.stops.push(*station),
                        }
                    }
                    self.rebuild_line_geometry(*line);
                    self.recompute_line_buildability(*line);
                    if self.cannot_afford(old_capital) {
                        // Can't afford this extension — restore the line exactly (afford-gate).
                        self.lines[line.index()].stops = saved_stops;
                        self.rebuild_line_geometry(*line);
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected {
                            reason: "Not enough funds for this extension".into(),
                        }]
                    } else {
                        vec![Event::StopAdded {
                            line: *line,
                            station: *station,
                        }]
                    }
                } else {
                    vec![Event::Rejected {
                        reason: "AddStop: unknown line or station".into(),
                    }]
                }
            }
            Command::AddBranchStop { line, branch, diverge_at, station } => {
                let li = line.index();
                let bi = *branch as usize;
                let ok = li < self.lines.len()
                    && station.index() < self.stations.len()
                    && bi <= self.lines[li].branches.len()
                    && (*diverge_at as usize) < self.lines[li].stops.len();
                if ok && !self.connected_can_add(line.index(), station.index()) {
                    // #infrastructure connected-rail gate (arcadia): a spur, too, must grow from rail that
                    // already reaches the capital (the branch's trunk normally does, so this rarely bites).
                    vec![Event::Rejected {
                        reason: "Connect this to your rail network — extend from your capital or a captured town".into(),
                    }]
                } else if ok {
                    let old_capital = self.capital_total();
                    let saved = self.lines[li].branches.clone();
                    {
                        let l = &mut self.lines[li];
                        if bi == l.branches.len() {
                            // New branch leaving the trunk at `diverge_at`, first stop = station.
                            l.branches.push(crate::line::Branch { diverge_at: *diverge_at, stops: vec![*station], waypoints: Vec::new() });
                        } else {
                            l.branches[bi].stops.push(*station); // extend an existing branch
                        }
                    }
                    self.rebuild_line_geometry(*line);
                    self.recompute_line_buildability(*line);
                    if self.cannot_afford(old_capital) {
                        self.lines[li].branches = saved;
                        self.rebuild_line_geometry(*line);
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected { reason: "Not enough funds for this branch".into() }]
                    } else {
                        vec![Event::BranchStopAdded { line: *line, branch: *branch, station: *station }]
                    }
                } else {
                    vec![Event::Rejected {
                        reason: "AddBranchStop: unknown line/station or bad branch/divergence".into(),
                    }]
                }
            }
            Command::SetBranchWaypoints { line, branch, waypoints } => {
                let li = line.index();
                let bi = *branch as usize;
                if li < self.lines.len() && bi < self.lines[li].branches.len() {
                    let wps: Vec<Vec<PointMm>> = waypoints
                        .iter()
                        .map(|span| span.iter().map(|&[x, y]| PointMm::new(x, y)).collect())
                        .collect();
                    self.lines[li].branches[bi].waypoints = wps;
                    self.rebuild_line_geometry(*line);
                    self.recompute_line_buildability(*line);
                    vec![Event::BranchWaypointsSet { line: *line, branch: *branch }]
                } else {
                    vec![Event::Rejected { reason: "SetBranchWaypoints: unknown line or branch".into() }]
                }
            }
            Command::SetBranchTrack { line, branch, mode } => {
                let li = line.index();
                let bi = *branch as usize;
                if li < self.lines.len()
                    && bi < self.lines[li].branches.len()
                    && bi + 1 < self.lines[li].paths.len()
                {
                    let old_capital = self.capital_total();
                    let d = self.lines[li].branches[bi].diverge_at as usize;
                    let saved = self.lines[li].paths[bi + 1].span_mode.clone();
                    let m = (*mode).min(crate::line::mode::TUNNEL);
                    {
                        // The branch's OWN spans (past the divergence) — the shared trunk prefix is
                        // governed by the trunk's Track control.
                        let sm = &mut self.lines[li].paths[bi + 1].span_mode;
                        for k in d..sm.len() {
                            sm[k] = m;
                        }
                    }
                    self.recompute_line_buildability(*line);
                    if self.cannot_afford(old_capital) {
                        self.lines[li].paths[bi + 1].span_mode = saved;
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected { reason: "Not enough funds to grade-separate this branch".into() }]
                    } else {
                        vec![Event::BranchTrackSet { line: *line, branch: *branch, mode: m }]
                    }
                } else {
                    vec![Event::Rejected { reason: "SetBranchTrack: unknown line or branch".into() }]
                }
            }
            Command::RemoveBranch { line, branch } => {
                let li = line.index();
                let bi = *branch as usize;
                if li < self.lines.len() && bi < self.lines[li].branches.len() {
                    self.lines[li].branches.remove(bi);
                    self.rebuild_line_geometry(*line);
                    self.recompute_line_buildability(*line);
                    vec![Event::BranchRemoved { line: *line, branch: *branch }]
                } else {
                    vec![Event::Rejected { reason: "RemoveBranch: unknown line or branch".into() }]
                }
            }
            Command::AssignTrainset { line, spec, count } => {
                if line.index() < self.lines.len() {
                    let count = (*count).clamp(1, MAX_TRAINS_PER_LINE);
                    let old_capital = self.capital_total();
                    let saved = self.lines[line.index()].trainset;
                    self.lines[line.index()].trainset = Some(TrainsetAssignment { spec: *spec, count });
                    self.recompute_line_buildability(*line); // train count affects capital cost
                    if self.cannot_afford(old_capital) {
                        self.lines[line.index()].trainset = saved;
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected {
                            reason: "Not enough funds for these trains".into(),
                        }]
                    } else {
                        vec![Event::TrainsetAssigned { line: *line, count }]
                    }
                } else {
                    vec![Event::Rejected {
                        reason: "AssignTrainset: unknown line".into(),
                    }]
                }
            }
            Command::SetHeadway { line, headway_ms } => {
                if let Some(l) = self.lines.get_mut(line.index()) {
                    let h = (*headway_ms).clamp(MIN_HEADWAY_MS, MAX_HEADWAY_MS);
                    l.headway_ms = h;
                    vec![Event::HeadwaySet {
                        line: *line,
                        headway_ms: h,
                    }]
                } else {
                    vec![Event::Rejected {
                        reason: "SetHeadway: unknown line".into(),
                    }]
                }
            }
            Command::SetSegmentMode { line, span, mode } => {
                let li = line.index();
                if li < self.lines.len() && !self.lines[li].paths.is_empty() {
                    let old_capital = self.capital_total();
                    let saved: Vec<Vec<u8>> =
                        self.lines[li].paths.iter().map(|p| p.span_mode.clone()).collect();
                    let m = (*mode).min(crate::line::mode::TUNNEL);
                    if *span == u32::MAX {
                        // WHOLE LINE = every span of EVERY path (trunk + branches), so legalizing a
                        // loaded water-crossing network tunnels its BRANCHES too (else a branch span
                        // over water keeps the whole line parked — e.g. London's Elizabeth/DLR/Mildmay
                        // lines cross the Thames on a branch).
                        for p in self.lines[li].paths.iter_mut() {
                            for s in p.span_mode.iter_mut() {
                                *s = m;
                            }
                        }
                    } else {
                        // A specific span targets the trunk (per-branch span editing is deferred).
                        let p = &mut self.lines[li].paths[0];
                        if (*span as usize) < p.span_mode.len() {
                            p.span_mode[*span as usize] = m;
                        }
                    }
                    self.recompute_line_buildability(*line);
                    if self.cannot_afford(old_capital) {
                        for (p, sm) in self.lines[li].paths.iter_mut().zip(saved) {
                            p.span_mode = sm;
                        }
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected {
                            reason: "Not enough funds to grade-separate this line".into(),
                        }]
                    } else {
                        vec![Event::SegmentModeSet { line: *line, span: *span, mode: m }]
                    }
                } else {
                    vec![Event::Rejected { reason: "SetSegmentMode: unknown line".into() }]
                }
            }
            Command::SetSegmentTrack { line, seg, track } => {
                let li = line.index();
                if li < self.lines.len() && !self.lines[li].paths.is_empty() {
                    let old_capital = self.capital_total();
                    let saved: Vec<Vec<u8>> =
                        self.lines[li].paths.iter().map(|p| p.track_type.clone()).collect();
                    let t = (*track).min(crate::line::track::SINGLE);
                    // TTD L3 C1: the edit targets a `TrackSegmentId`. The whole-line sentinel
                    // `TrackSegmentId(u32::MAX)` (G6) fans out to every span of every path; otherwise the id's
                    // value is the TRUNK SPAN it covers (per-branch editing deferred). The write lands on the
                    // per-path `track_type` (the edit + persistence store); the segment slab then re-authors
                    // its `track_type` from this span when `rebuild_track_segments` runs at the end of apply.
                    if seg.0 == u32::MAX {
                        // WHOLE LINE = every span of every path (trunk + branches).
                        for p in self.lines[li].paths.iter_mut() {
                            for s in p.track_type.iter_mut() {
                                *s = t;
                            }
                        }
                    } else {
                        let p = &mut self.lines[li].paths[0];
                        if (seg.0 as usize) < p.track_type.len() {
                            p.track_type[seg.0 as usize] = t;
                        }
                    }
                    // Track type changes cost (single is cheaper) + the meet authority; it does NOT
                    // change trainset count, so it must NOT set dispatch_dirty — vehicles keep their
                    // positions and re-derive single-track occupancy next tick.
                    self.recompute_line_buildability(*line);
                    if self.cannot_afford(old_capital) {
                        for (p, tt) in self.lines[li].paths.iter_mut().zip(saved) {
                            p.track_type = tt;
                        }
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected {
                            reason: "Not enough funds to double-track this line".into(),
                        }]
                    } else {
                        vec![Event::SegmentTrackSet { line: *line, seg: *seg, track: t }]
                    }
                } else {
                    vec![Event::Rejected { reason: "SetSegmentTrack: unknown line".into() }]
                }
            }
            Command::SetRunning { running } => {
                self.running = *running;
                vec![Event::RunningSet { running: *running }]
            }
            Command::SetEconomy { enabled } => {
                self.economy_enabled = *enabled;
                vec![Event::EconomySet { enabled: *enabled }]
            }
            Command::RemoveStation { station } => {
                let idx = station.index();
                if idx < self.stations.len() && !self.stations[idx].removed {
                    self.stations[idx].removed = true;
                    // Drop the station from every line that stops there, then rebuild those
                    // lines' geometry + cost (the line simply skips the bulldozed stop).
                    let affected: Vec<usize> = self
                        .lines
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| {
                            !l.removed
                                && (l.stops.iter().any(|s| s.index() == idx)
                                    || l.branches.iter().any(|b| b.stops.iter().any(|s| s.index() == idx)))
                        })
                        .map(|(li, _)| li)
                        .collect();
                    for li in affected {
                        self.lines[li].stops.retain(|s| s.index() != idx);
                        for b in &mut self.lines[li].branches {
                            b.stops.retain(|s| s.index() != idx);
                        }
                        self.lines[li].branches.retain(|b| !b.stops.is_empty());
                        self.rebuild_line_geometry(LineId(li as u32));
                        self.recompute_line_buildability(LineId(li as u32));
                    }
                    if let Some(q) = self.waiting.get_mut(idx) {
                        q.clear(); // riders waiting at a bulldozed station are gone
                    }
                    self.demand_dirty = true; // its catchment frees up for neighbours
                    vec![Event::StationRemoved { station: *station }]
                } else {
                    vec![Event::Rejected {
                        reason: "RemoveStation: unknown or already removed".into(),
                    }]
                }
            }
            Command::RemoveLine { line } => {
                let idx = line.index();
                if idx < self.lines.len() && !self.lines[idx].removed {
                    self.lines[idx].removed = true; // vehicles despawn on the next dispatch rebuild
                    vec![Event::LineRemoved { line: *line }]
                } else {
                    vec![Event::Rejected {
                        reason: "RemoveLine: unknown or already removed".into(),
                    }]
                }
            }
            Command::SetLineWaypoints { line, waypoints } => {
                if line.index() < self.lines.len() && !self.lines[line.index()].removed {
                    let old_capital = self.capital_total();
                    let saved = self.lines[line.index()].waypoints.clone();
                    self.lines[line.index()].waypoints = waypoints
                        .iter()
                        .map(|span| span.iter().map(|&[x, y]| PointMm::new(x, y)).collect())
                        .collect();
                    // Bending the track changes its length → geometry, buildability and cost.
                    self.rebuild_line_geometry(*line);
                    self.recompute_line_buildability(*line);
                    if self.cannot_afford(old_capital) {
                        self.lines[line.index()].waypoints = saved;
                        self.rebuild_line_geometry(*line);
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected { reason: "Not enough funds to reroute this line".into() }]
                    } else {
                        vec![Event::WaypointsSet { line: *line }]
                    }
                } else {
                    vec![Event::Rejected { reason: "SetLineWaypoints: unknown line".into() }]
                }
            }
            Command::SetDemandMode { agents } => {
                self.agent_demand = *agents;
                // Swap the demand box behind the seam so `tick`'s `demand.spawn` dispatches the
                // right model. `agent_demand` (the bool) stays the source of truth for the
                // population top-up inside `demand::grow`; the box is what `spawn` polymorphism
                // keys on. Neither is hashed, so this is determinism-neutral.
                if *agents {
                    self.demand = Box::new(crate::ruleset::AgentDemand);
                    // Generate (or keep) the seed-derived population — sized to the city's homes.
                    if self.population.is_none() {
                        let n = self.agent_population_target();
                        self.population = Some(crate::agents::Population::generate(self, n, self.seed));
                    }
                } else {
                    self.demand = Box::new(crate::ruleset::GravityDemand);
                    self.population = None; // back to gravity; free the table
                }
                vec![Event::DemandModeSet { agents: *agents }]
            }
        };
        // Any change to lines / trainsets / headway / running invalidates dispatch. `PlaceStation` and
        // `BuildPlatforms` are exempt: they don't change line topology / dispatch cadence / SoA sizing
        // (berths are parallel DWELL slots, not extra vehicles), and a needless re-dispatch would reset
        // every train to spawn (a gameplay bug) AND perturb the golden — so building a platform must not
        // invalidate dispatch. TTD L5b: `PlaceSignal`/`RemoveSignal` are EXEMPT for the SAME reason — a
        // signal does NOT change the dispatch cap (it stays `doubles+1`; see dispatch.rs) nor the SoA
        // sizing; its only effect is the per-tick move-phase same-direction following relaxation, which
        // reads `world.signals` afresh every tick in `advance`. So a signal placed on a RUNNING line must
        // take effect WITHOUT re-dispatching (which would teleport every train back to spawn).
        if !matches!(
            cmd,
            Command::PlaceStation { .. }
                | Command::BuildPlatforms { .. }
                | Command::PlaceSignal { .. }
                | Command::RemoveSignal { .. }
        ) {
            self.dispatch_dirty = true;
            // TTD L3 C1: refresh the authoritative HASHED segment slab + each path's segment binding + the
            // segment-derived runtime geometry in the WRITE-PATH, so `state_hash` (which may be taken before
            // any tick) reflects the geometry that genuinely lives in the slab. Gated to the same line/
            // geometry-changing commands that dirty dispatch (PlaceStation/BuildPlatforms don't touch
            // geometry). Idempotent vs the dispatch-time rebuild (a pure fn of the lines).
            self.rebuild_track_segments();
        }
        self.cmd_log.push(cmd.clone());
        // Refresh catchment capture eagerly after the edit so per-station captured demand
        // (homes/jobs) is correct in BUILD mode too — at placement time, when the player is
        // deciding where to build. `prepare` is internally gated on `demand_dirty` (a no-op
        // otherwise) and writes only derived caches (captured_origin/dest, footpaths) that are
        // NOT part of `Canonical`, so the state hash — and replay determinism — are unaffected.
        self.demand_prepare();
        events
    }

    /// Advance the simulation by one fixed step.
    pub fn tick(&mut self, dt_ms: i64) {
        tick::step(self, dt_ms);
    }

    /// TTD L5 — the arc-length bounds `(lo, hi)` of span `span` on `(line, path)`, or `None` if the line/
    /// path/span doesn't exist. A signal must lie strictly inside this range (`lo < at_mm < hi`).
    fn signal_span_bounds(&self, line: LineId, path: u8, span: u32) -> Option<(i64, i64)> {
        let p = self.lines.get(line.index())?.paths.get(path as usize)?;
        let lo = *p.stop_arclen_mm.get(span as usize)?;
        let hi = *p.stop_arclen_mm.get(span as usize + 1)?;
        if hi > lo { Some((lo, hi)) } else { None }
    }

    /// FNV-1a over a canonical, ordered serialization of state. The determinism oracle.
    pub fn state_hash(&self) -> u64 {
        let canon = Canonical {
            clock_ms: self.clock_ms,
            running: self.running,
            stations: &self.stations,
            lines: &self.lines,
            veh_line: &self.vehicles.line,
            veh_path: &self.vehicles.path,
            veh_s_mm: &self.vehicles.s_mm,
            veh_dir: &self.vehicles.dir,
            veh_dwell_ms: &self.vehicles.dwell_until_ms,
            veh_onboard: &self.vehicles.onboard,
            ridership_total: self.ridership_total,
            total_journey_ms: self.total_journey_ms,
            journey_samples: self.journey_samples,
            total_wait_ms: self.total_wait_ms,
            wait_samples: self.wait_samples,
            denied_boardings: self.denied_boardings,
            abandoned: self.abandoned,
            denied_at: &self.denied_at,
            abandoned_at: &self.abandoned_at,
            opex_accrued: self.opex_accrued,
            opex_rem: self.opex_rem,
            forge_stock: &self.forge_stock,
            tribute: self.tribute,
            army_line: &self.armies.line,
            army_path: &self.armies.path,
            army_s_mm: &self.armies.s_mm,
            army_dir: &self.armies.dir,
            army_strength: &self.armies.strength,
            army_target: &self.armies.target,
            army_state: &self.armies.state,
            army_wait_line: &self.armies.wait_line,
            army_wait_dir: &self.armies.wait_dir,
            army_riding_veh: &self.armies.riding_veh,
            army_wait_until_ms: &self.armies.wait_until_ms,
            town_value: &self.town_value,
            towns_captured: self.towns_captured,
            is_barracks: &self.is_barracks,
            bounty: &self.bounty,
            decadence: self.decadence,
            decadence_cells: &self.decadence_cells,
            tech_unlocked: self.tech_unlocked,
            mana: self.mana,
            manpower: self.manpower,
            raider_x_mm: &self.raiders.x_mm,
            raider_y_mm: &self.raiders.y_mm,
            raider_state: &self.raiders.state,
            raider_spawn_accum_ms: self.raider_spawn_accum_ms,
            raider_cursor: self.raider_cursor,
            raider_breach: self.raider_breach,
            raider_breach_heal_accum: self.raider_breach_heal_accum,
            spells_cast: self.spells_cast,
            line_disabled_until_ms: &self.line_disabled_until_ms,
            raider_tx_mm: &self.raiders.tx_mm,
            raider_ty_mm: &self.raiders.ty_mm,
            track_segments: CanonSegments(&self.track_graph),
            signals: &self.signals,
        };
        let bytes = postcard::to_allocvec(&canon).expect("canonical state serializes");
        fnv1a(&bytes)
    }

    /// Authoritative station geometry for rendering (mm as f64; no BigInt at the boundary).
    pub fn stations_view(&self) -> Vec<StationView> {
        self.stations
            .iter()
            .enumerate()
            .map(|(i, s)| StationView {
                id: i as u32,
                x_mm: s.pos.x_mm as f64,
                y_mm: s.pos.y_mm as f64,
                name: s.name.clone(),
                removed: s.removed,
                bounty: self.bounty.get(i).copied().unwrap_or(0) as f64,
                platform_count: s.platform_count,
            })
            .collect()
    }

    /// OD "desire lines" from a selected origin station: the top `top_k` destinations its riders
    /// are drawn toward (gravity attractiveness × accessibility), for the on-selection flow overlay.
    /// Read-only — solves accessibility fresh and mutates nothing. `weight` is normalized 0..1 vs
    /// the strongest link; empty if the origin isn't an operational, served station.
    pub fn station_od(&self, origin: u32, top_k: usize) -> Vec<OdLink> {
        let mut w = crate::demand::od_weights(self, origin as usize);
        if w.is_empty() {
            return Vec::new();
        }
        // Descending by pull; partial_cmp fallback keeps it total-ordered (weights are finite).
        w.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let max = w.first().map(|(_, x)| *x).unwrap_or(1.0).max(1e-9);
        w.into_iter()
            .take(top_k)
            .map(|(d, wt)| {
                let p = self.stations[d as usize].pos;
                OdLink { dest: d, x_mm: p.x_mm as f64, y_mm: p.y_mm as f64, weight: (wt / max) as f32 }
            })
            .collect()
    }

    /// Accessibility isochrone from a selected origin station: every OTHER served station it can
    /// reach by transit, with the travel time (wait + ride + transfers) via `Router::reachable`.
    /// For the opt-in "Reach" overlay. Read-only — solves fresh, mutates nothing. Empty if the
    /// origin isn't an operational, served station; unreachable stations are simply omitted.
    pub fn station_access(&self, origin: u32) -> Vec<AccessLink> {
        let o = origin as usize;
        if o >= self.stations.len() || self.serving.get(o).map(|v| v.is_empty()).unwrap_or(true) {
            return Vec::new();
        }
        let access = self
            .router
            .reachable(&self.lines, &self.serving, &self.footpaths, StationId(origin), self.max_legs);
        if access.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<AccessLink> = Vec::new();
        for d in 0..self.stations.len() {
            if d == o
                || self.stations[d].removed
                || self.serving.get(d).map(|v| v.is_empty()).unwrap_or(true)
            {
                continue;
            }
            match access.get(d).copied() {
                Some(t) if t < i64::MAX => {
                    let p = self.stations[d].pos;
                    out.push(AccessLink { station: d as u32, x_mm: p.x_mm as f64, y_mm: p.y_mm as f64, ms: t as f64 });
                }
                _ => {}
            }
        }
        out
    }

    /// The walk shed of a selected station: the buildability cells reachable from it on foot,
    /// each with its distance-decay `intensity` — for the lopsided catchment overlay. Water severs
    /// it and crossed corridors pinch it (`walkshed::effective_walk_dist`), so the returned cell
    /// set IS the real catchment, not a circle. Empty when the city carries no buildability raster
    /// (the frontend then falls back to drawing the nominal-radius ring). Read-only; mutates
    /// nothing. Cheap — a single station's bounded neighbourhood, recomputed only on selection.
    pub fn station_walkshed(&self, origin: u32) -> Vec<ShedCell> {
        let o = origin as usize;
        if o >= self.stations.len() || self.stations[o].removed || self.build_lookup.is_empty() {
            return Vec::new();
        }
        let sp = self.stations[o].pos;
        let r = crate::demand::CATCHMENT_MM as f64;
        let cell = self.build_cell_mm.max(1);
        let span = crate::demand::CATCHMENT_MM / cell + 1; // cell radius of the search box
        let (cx, cy) = (sp.x_mm.div_euclid(cell), sp.y_mm.div_euclid(cell));
        let mut out: Vec<ShedCell> = Vec::new();
        for gy in (cy - span)..=(cy + span) {
            for gx in (cx - span)..=(cx + span) {
                let px = gx * cell + cell / 2;
                let py = gy * cell + cell / 2;
                if let Some(eff) = crate::walkshed::effective_walk_dist(&self.build_lookup, cell, sp, PointMm::new(px, py), r) {
                    let t = eff / r;
                    let intensity = (-(t * t)).exp() as f32;
                    out.push(ShedCell { x_mm: px as f64, y_mm: py as f64, intensity });
                }
            }
        }
        out
    }

    /// Authoritative line geometry (ordered stops + polyline) for rendering.
    pub fn lines_view(&self) -> Vec<LineView> {
        self.lines
            .iter()
            .enumerate()
            .map(|(i, l)| LineView {
                id: i as u32,
                name: l.name.clone(),
                mode: l.mode,
                loop_line: l.loop_line,
                color: l.color,
                stops: l.stops.iter().map(|s| s.0).collect(),
                // Trunk geometry only for now; branch-track rendering is Stage C (needs a LineView
                // contract change to carry per-branch polylines). #curved-track: GRID lines ship the
                // render-only SMOOTHED polyline (rounded hex corners, stops pinned) so the drawn track
                // curves; transit lines pass through unchanged (identity). The trains ride the SAME
                // smoothing (render_buf), so they stay on the rail.
                polyline_mm: l
                    .paths
                    .first()
                    .map(|p| crate::render_buf::smooth_polyline_mm(p, self.city.grid_cell_mm))
                    .unwrap_or_default(),
                branch_polylines_mm: l
                    .paths
                    .iter()
                    .skip(1)
                    .map(|p| crate::render_buf::smooth_polyline_mm(p, self.city.grid_cell_mm))
                    .collect(),
                branch_modes: l
                    .branches
                    .iter()
                    .enumerate()
                    .map(|(bi, b)| {
                        // Uniform build mode of the branch's OWN spans (past the divergence), else -1.
                        let d = b.diverge_at as usize;
                        match l.paths.get(bi + 1).map(|p| &p.span_mode[d.min(p.span_mode.len())..]) {
                            Some(own) if !own.is_empty() && own.iter().all(|&m| m == own[0]) => own[0] as i32,
                            _ => -1,
                        }
                    })
                    .collect(),
                branch_termini: l
                    .branches
                    .iter()
                    .map(|b| b.stops.last().map(|s| s.0).unwrap_or(0))
                    .collect(),
                min_radius_mm: l.min_radius_mm() as f64,
                span_modes: l.paths.first().map(|p| p.span_mode.clone()).unwrap_or_default(),
                // TTD L5c: the trunk path's SIM-frame stop arc-lengths (mm as f64), so the UI can derive a
                // PlaceSignal `at_mm` from an in-span fraction (stops pin span boundaries across smoothing).
                stop_arclen_mm: l
                    .paths
                    .first()
                    .map(|p| p.stop_arclen_mm.iter().map(|&v| v as f64).collect())
                    .unwrap_or_default(),
                track_types: l.paths.first().map(|p| p.track_type.clone()).unwrap_or_default(),
                crosses_water_surface: l.crosses_water_surface,
                removed: l.removed,
                // #war: ms left on this line's raid (0 = operational) — the lazy timer; absent ⇒ 0.
                raided_remaining_ms: (self.line_disabled_until_ms.get(i).copied().unwrap_or(0) - self.clock_ms).max(0) as f64,
            })
            .collect()
    }

    pub fn save(&self) -> SaveGame {
        SaveGame {
            seed: self.seed,
            ruleset: self.city.ruleset.clone(),
            commands: self.cmd_log.clone(),
        }
    }
}

/// Reconstruct a world by replaying a save (seed + command log) onto a fresh `CityData`.
/// `tick_to` advances the clock by replaying ticks; pass the original tick schedule.
pub fn replay(save: &SaveGame, city: CityData) -> World {
    // Disjoint-save guard (S3): a save and the city it replays onto MUST be the same mode. A
    // fantasy save replayed onto a transit city (or vice-versa) runs the command log against the
    // wrong `World::apply` arms and silently diverges — exactly the class the golden pin can't see
    // (different command vocab, not a hash shift). Compared canonicalised so `""` and `"transit"`
    // are the same mode. A precondition, not user input: a mismatch is a save-loading bug.
    assert_eq!(
        crate::ruleset::canon(&save.ruleset),
        crate::ruleset::canon(&city.ruleset),
        "disjoint-save guard: save ruleset {:?} != city ruleset {:?} — replaying a save onto the \
         wrong game mode would diverge",
        save.ruleset,
        city.ruleset,
    );
    let mut w = World::new(save.seed, city);
    for cmd in &save.commands {
        w.apply(cmd);
    }
    w
}
