// Command builders + JSON encoder. The wire format is JSON (postcard is Rust-side save
// only). Builders produce the exact externally-tagged shape crates/sim deserializes.
import type { Command } from "../types";

export const cmd = {
  placeStation: (x_mm: number, y_mm: number, name: string | null = null): Command => ({
    PlaceStation: { x_mm, y_mm, name },
  }),
  createLine: (color: number, name: string | null = null, loop_line = false, mode = 0, literal = false): Command => ({
    CreateLine: { color, name, loop_line, mode, literal },
  }),
  addStop: (line: number, station: number, after: number | null = null): Command => ({
    AddStop: { line, station, after },
  }),
  /** Append a stop to a line's branch tree (P3). branch == current branch count creates a new
   *  branch leaving the trunk at stop `diverge_at`; branch < count extends that branch. */
  addBranchStop: (line: number, branch: number, diverge_at: number, station: number): Command => ({
    AddBranchStop: { line, branch, diverge_at, station },
  }),
  /** Set a branch's own per-span real-geometry waypoints (mm `[x,y]`) — the spur's OSM alignment. */
  setBranchWaypoints: (line: number, branch: number, waypoints: [number, number][][]): Command => ({
    SetBranchWaypoints: { line, branch, waypoints },
  }),
  /** Build mode (0=Surface,1=Elevated,2=Tunnel) for a whole branch's own track. */
  setBranchTrack: (line: number, branch: number, mode: number): Command => ({
    SetBranchTrack: { line, branch, mode },
  }),
  /** Bulldoze a branch off a line (trunk + other branches stay). */
  removeBranch: (line: number, branch: number): Command => ({ RemoveBranch: { line, branch } }),
  assignTrainset: (line: number, spec: number, count: number): Command => ({
    AssignTrainset: { line, spec, count },
  }),
  setHeadway: (line: number, headway_ms: number): Command => ({
    SetHeadway: { line, headway_ms },
  }),
  /** span = WHOLE_LINE (0xffffffff) sets every span; mode 0=Surface,1=Elevated,2=Tunnel. */
  setSegmentMode: (line: number, span: number, mode: number): Command => ({
    SetSegmentMode: { line, span, mode },
  }),
  /** Track type (P2): span = WHOLE_LINE sets every span; track 0=Double,1=Single. */
  setSegmentTrack: (line: number, span: number, track: number): Command => ({
    SetSegmentTrack: { line, span, track },
  }),
  setRunning: (running: boolean): Command => ({ SetRunning: { running } }),
  setEconomy: (enabled: boolean): Command => ({ SetEconomy: { enabled } }),
  /** Bulldoze a station (tombstone; dropped from any line through it). */
  removeStation: (station: number): Command => ({ RemoveStation: { station } }),
  /** Bulldoze a whole line (tombstone; its vehicles despawn). */
  removeLine: (line: number): Command => ({ RemoveLine: { line } }),
  /** Set the per-span control points (mm `[x,y]`) that bend a line's track between its stops. */
  setLineWaypoints: (line: number, waypoints: [number, number][][]): Command => ({ SetLineWaypoints: { line, waypoints } }),
  /** Switch the demand model: agents=true → seed-derived citizen commuters; false → gravity flow. */
  setDemandMode: (agents: boolean): Command => ({ SetDemandMode: { agents } }),
  /** Fantasy/arcadia (S8): place a barracks (fields AI legions). Rejected by the transit ruleset. */
  placeBarracks: (x_mm: number, y_mm: number, name: string | null = null): Command => ({
    PlaceBarracks: { x_mm, y_mm, name },
  }),
  /** Fantasy/arcadia (S8): post a bounty on a town (Majesty steering). amount=0 clears it. */
  postBounty: (station: number, amount: number): Command => ({ PostBounty: { station, amount } }),
  /** Fantasy/arcadia (S11): buy a tech upgrade (index into the tech table) with tribute. Rejected in transit. */
  unlockTech: (tech: number): Command => ({ UnlockTech: { tech } }),
};

/** An economy channel (S11 split) — MUST mirror crates/sim/tech.rs `Channel`. */
export type Channel = "gold" | "mana" | "manpower";
/** Channel display metadata (glyph + the Stats field it reads). */
export const CHANNELS: Record<Channel, { glyph: string; statKey: "tribute" | "mana" | "manpower" }> = {
  gold: { glyph: "⚜", statKey: "tribute" },
  mana: { glyph: "✦", statKey: "mana" },
  manpower: { glyph: "⚔", statKey: "manpower" },
};
/** The tech table — MUST mirror crates/sim/tech.rs `TECHS` (id = index, mana cost, prereq). All tech is
 *  bought with MANA (the sole tech resource). `tier` + `prereq` drive the panel's tree layout + gating.
 *  The HUD reads `Stats.techUnlocked` (bit per id) + `Stats.mana` to render locked/affordable/owned. */
export interface TechDef { id: number; name: string; cost: number; tier: number; prereq: number; blurb: string }
export const TECHS: TechDef[] = [
  { id: 0, name: "Forge Mastery", cost: 30, tier: 1, prereq: -1, blurb: "Sources produce twice as fast" },
  { id: 1, name: "Conscription", cost: 35, tier: 1, prereq: -1, blurb: "Legions cost half the manpower" },
  { id: 2, name: "Sappers", cost: 30, tier: 1, prereq: -1, blurb: "The decadence tide creeps half as fast" },
  { id: 3, name: "Production Surge", cost: 60, tier: 2, prereq: 0, blurb: "Sources produce three times as fast" },
  { id: 4, name: "Bounty Mastery", cost: 45, tier: 2, prereq: 1, blurb: "Legions besieging a bountied town grind +50%" },
  { id: 5, name: "Heavy Rail", cost: 70, tier: 2, prereq: 0, blurb: "Unlocks heavy rail — high-capacity arterial track" },
  { id: 6, name: "Siege Doctrine", cost: 65, tier: 2, prereq: 1, blurb: "All besieging legions grind +50%" },
  { id: 7, name: "Standing Garrison", cost: 55, tier: 2, prereq: 1, blurb: "Captured towns cut down raiders" },
  { id: 8, name: "Forced March", cost: 55, tier: 2, prereq: 1, blurb: "Legions march +50% faster" },
  { id: 9, name: "Ley Tap", cost: 45, tier: 2, prereq: 2, blurb: "Aether mints +50% mana" },
  { id: 10, name: "Ward Lines", cost: 55, tier: 2, prereq: 2, blurb: "Your rails cut raiders down from +50% range" },
];
/** True iff tech `id` is unlocked in the bitset (mirrors tech::is_unlocked; bit == id for the shipped set). */
export const techUnlocked = (bits: number, id: number): boolean => (bits & (1 << id)) !== 0;

export const WHOLE_LINE = 0xffffffff;
export const BUILD_MODE = { SURFACE: 0, ELEVATED: 1, TUNNEL: 2 } as const;
export const TRACK_TYPE = { DOUBLE: 0, SINGLE: 1 } as const;
/** Transport mode (Line.mode in the sim). Matches crates/sim trainset::tmode. */
export const TRANSPORT = { RAIL: 0, BUS: 1, FERRY: 2, AIR: 3, HEAVY: 4 } as const;
export type TransportMode = (typeof TRANSPORT)[keyof typeof TRANSPORT];

export function encodeCommand(c: Command): string {
  return JSON.stringify(c);
}
