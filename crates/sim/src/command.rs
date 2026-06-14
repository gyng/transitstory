//! The command/event vocabulary — the sim's only write port. Every player mutation is
//! one serializable `Command` applied via `World::apply`; the sim emits `Event`s the
//! frontend reads back (assigned ids, auto-names). Save = seed + ordered command log.
//!
//! Wire format: externally-tagged serde (the default), e.g.
//! `{"PlaceStation":{"x_mm":0,"y_mm":0,"name":null}}`. This is the one enum shape that
//! round-trips through BOTH JSON (the live command wire) and postcard (the save artifact);
//! internally-tagged would break postcard, which is not self-describing.
//!
//! Positions are local millimetres (i64) — the sim NEVER sees lng/lat.
use crate::ids::{LineId, StationId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    PlaceStation {
        x_mm: i64,
        y_mm: i64,
        #[serde(default)]
        name: Option<String>,
    },
    CreateLine {
        color: u32,
        #[serde(default)]
        name: Option<String>,
        /// Circular line (last stop connects back to the first) vs out-and-back.
        #[serde(default)]
        loop_line: bool,
        /// Transport mode: 0=rail,1=bus,2=ferry,3=air.
        #[serde(default)]
        mode: u8,
        /// Literal geometry — follow supplied (imported OSM) waypoints directly instead of the
        /// synthesised Catmull-Rom curve. Set for real-world network imports. Default false.
        #[serde(default)]
        literal: bool,
    },
    AddStop {
        line: LineId,
        station: StationId,
        #[serde(default)]
        after: Option<usize>,
    },
    /// Append a stop to a BRANCH of a line (P3, docs/capacity-roadmap.md). `branch` indexes the
    /// line's branches: `branch == branches.len()` creates a new branch leaving the trunk at trunk
    /// stop `diverge_at` (its first stop = `station`); `branch < branches.len()` appends `station`
    /// to that existing branch (`diverge_at` ignored). Branches form a tree off the trunk — multiple
    /// branches may share a `diverge_at` (a 3-way junction, e.g. the Jurong Region Line at Bahar).
    /// The engine derives a root-to-leaf service path per branch; trains run them round-robin.
    AddBranchStop {
        line: LineId,
        branch: u16,
        diverge_at: u16,
        station: StationId,
    },
    /// Set a branch's own per-span shaping points (its real-geometry waypoints) — the spur's OSM
    /// alignment for a literal imported line. `waypoints[i]` shapes the branch span after the i-th
    /// branch vertex (junction→stop0 is span 0). Replaces all of that branch's waypoints in one
    /// command. Local mm `[x,y]`, like `SetLineWaypoints`.
    SetBranchWaypoints {
        line: LineId,
        branch: u16,
        waypoints: Vec<Vec<[i64; 2]>>,
    },
    /// Build mode (0=Surface,1=Elevated,2=Tunnel) for a whole BRANCH — sets every span of the
    /// branch's OWN track (past the divergence; the shared trunk prefix is governed by the trunk).
    /// The per-branch analog of the whole-line Track control.
    SetBranchTrack {
        line: LineId,
        branch: u16,
        mode: u8,
    },
    /// Bulldoze a branch off a line (tombstone-free: it's just dropped; the trunk + other branches
    /// stay). Reversible by replay, like `RemoveLine`.
    RemoveBranch {
        line: LineId,
        branch: u16,
    },
    AssignTrainset {
        line: LineId,
        spec: u8,
        count: u16,
    },
    SetHeadway {
        line: LineId,
        headway_ms: i64,
    },
    /// Build mode for one inter-stop span (0=Surface,1=Elevated,2=Tunnel); span=u32::MAX sets
    /// every span of the line (whole-line toggle).
    SetSegmentMode {
        line: LineId,
        span: u32,
        mode: u8,
    },
    /// Track type for one inter-stop span (0=Double,1=Single; P2); span=u32::MAX sets every span of
    /// the line. Mirrors SetSegmentMode — affects capacity (single-track meets) + capital cost, NOT
    /// the build mode. Single track is cheaper to build but serialises opposing traffic (meets).
    SetSegmentTrack {
        line: LineId,
        span: u32,
        track: u8,
    },
    SetRunning {
        running: bool,
    },
    /// Toggle the (optional) economy on/off.
    SetEconomy {
        enabled: bool,
    },
    /// Bulldoze a station: tombstone it (the id/slot is never reused — determinism) and drop it
    /// from every line that stops there. Its catchment frees up for neighbours.
    RemoveStation {
        station: StationId,
    },
    /// Bulldoze a whole line: tombstone it and despawn its vehicles.
    RemoveLine {
        line: LineId,
    },
    /// Set the freeform control points that bend a line's track between stops. `waypoints[i]`
    /// (local mm `[x, y]`) shapes the span after stop i; replaces ALL of the line's waypoints in
    /// one command (so undo = one step). An empty/shorter list straightens those spans.
    SetLineWaypoints {
        line: LineId,
        waypoints: Vec<Vec<[i64; 2]>>,
    },
    /// Place a BARRACKS (fantasy/arcadia, S8): a node that fields AI legions when tribute funds them.
    /// Creates a station (the barracks) and flags it — armies launch only from a barracks on a built
    /// route, so building one is the player's prerequisite for war (agency). FANTASY-ONLY: the transit
    /// ruleset rejects it (the disjoint-save guard's first real cross-mode teeth).
    PlaceBarracks {
        x_mm: i64,
        y_mm: i64,
        #[serde(default)]
        name: Option<String>,
    },
    /// Post a BOUNTY on a town (fantasy/arcadia, S8 — the Majesty steering lever). The player does NOT
    /// command legions directly; they BAIT them — a bounty pulls AI armies toward that town (the
    /// highest-bounty uncaptured town on a barracks's route becomes its target). `amount = 0` clears it.
    /// FANTASY-ONLY: the transit ruleset rejects it.
    PostBounty {
        station: StationId,
        amount: i64,
    },
    /// Switch the demand model: `agents=true` swaps gravity flow for a seed-derived citizen
    /// population (home/work agents on a schedule); `false` restores gravity. Command-sourced so
    /// it lives in the save and replays deterministically (the population is regenerated from seed).
    SetDemandMode {
        agents: bool,
    },
    /// Buy a tech upgrade (fantasy/arcadia, S11): `tech` is an index into `tech::TECHS`. Spends
    /// `TECHS[tech].cost` TRIBUTE (the same pool that funds legions) and permanently sets the tech's
    /// bit in `tech_unlocked`, which gates a buff to an existing lever. Rejected by transit, by an
    /// unknown id, if already unlocked, or if tribute is short — so the spend is exactly-once.
    UnlockTech {
        tech: u8,
    },
    /// Cast a spell (fantasy/arcadia, S11 — the mana spell arm). `kind` is a `spell::*` id (PURGE_FRONT/
    /// SMITE/WARPATH). AUTO-TARGETED: the engine picks the target (the front nearest the capital, the
    /// breaching raider, the most-stalled siege) — the player chooses WHEN, not WHERE. The "when" IS the
    /// lever: every cast spends MANA from the SAME pool that buys tech, so casting now is teching later.
    /// A no-op (no mana spent — surfaced as `Rejected`, not a panic) if mana is short or no valid target
    /// exists. FANTASY-ONLY + needs the SPELLCRAFT tech (else rejected).
    CastSpell {
        kind: u8,
    },
    /// Toggle AUTOCAST (fantasy/arcadia, S11): on ⇒ the spell arm auto-fires the whole battery each tick at
    /// the biggest threat (the Majesty-style hands-off mode); off (the DEFAULT) ⇒ spells fire only on
    /// `CastSpell`. Command-sourced — NOT a client knob like speed — because it changes whether casts mutate
    /// sim state on a tick, so it must replay deterministically. FANTASY-ONLY.
    SetAutocast {
        enabled: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    StationPlaced { id: StationId, name: String },
    LineCreated { id: LineId },
    StopAdded { line: LineId, station: StationId },
    BranchStopAdded { line: LineId, branch: u16, station: StationId },
    BranchWaypointsSet { line: LineId, branch: u16 },
    BranchTrackSet { line: LineId, branch: u16, mode: u8 },
    BranchRemoved { line: LineId, branch: u16 },
    TrainsetAssigned { line: LineId, count: u16 },
    HeadwaySet { line: LineId, headway_ms: i64 },
    SegmentModeSet { line: LineId, span: u32, mode: u8 },
    SegmentTrackSet { line: LineId, span: u32, track: u8 },
    RunningSet { running: bool },
    EconomySet { enabled: bool },
    StationRemoved { station: StationId },
    LineRemoved { line: LineId },
    WaypointsSet { line: LineId },
    DemandModeSet { agents: bool },
    BarracksPlaced { id: StationId, name: String },
    BountyPosted { station: StationId, amount: i64 },
    TechUnlocked { tech: u8, balance_left: i64 },
    SpellCast { kind: u8, balance_left: i64 },
    AutocastSet { enabled: bool },
    Rejected { reason: String },
}
