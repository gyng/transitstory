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
  /** Track type (P2): TTD L3 C1 targets a TrackSegmentId (`seg`); seg = WHOLE_LINE is the whole-line
   * sentinel. track 0=Double,1=Single. (The `seg` arg keeps the prior `span` call sites working —
   * the frontend only ever passes WHOLE_LINE.) */
  setSegmentTrack: (line: number, seg: number, track: number): Command => ({
    SetSegmentTrack: { line, seg, track },
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
  /** Fantasy/arcadia (S11): buy a tech upgrade (index into the tech table) with mana. Rejected in transit. */
  unlockTech: (tech: number): Command => ({ UnlockTech: { tech } }),
  /** Fantasy/arcadia (S11): cast a spell (auto-targeted, spends mana). kind = a `SPELLS` id. Rejected in transit. */
  castSpell: (kind: number): Command => ({ CastSpell: { kind } }),
  /** Fantasy/arcadia (S11): toggle autocast (on = the AI auto-fires spells each tick). Rejected in transit. */
  setAutocast: (enabled: boolean): Command => ({ SetAutocast: { enabled } }),
  /** Fantasy/arcadia (#13): seed the rival realm's seat (faction-1 far-edge capital). Idempotent; fired once
   *  at boot after the player network. Rejected in transit. */
  seedRival: (): Command => ({ SeedRival: {} }),
  // TTD L2: set a station's platform berth count (k clamped to [1, MAX_PLATFORMS] in the core).
  buildPlatforms: (station: number, k: number): Command => ({ BuildPlatforms: { station, k } }),
  // TTD L5: place/remove a player block signal strictly inside span `span` of (line, path).
  placeSignal: (line: number, path: number, span: number, atMm: number): Command => ({
    PlaceSignal: { line, path, span, at_mm: atMm },
  }),
  removeSignal: (line: number, path: number, span: number, atMm: number): Command => ({
    RemoveSignal: { line, path, span, at_mm: atMm },
  }),
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
  { id: 0, name: "Forge Mastery", cost: 18, tier: 1, prereq: -1, blurb: "Sources produce twice as fast" },
  { id: 1, name: "Conscription", cost: 18, tier: 1, prereq: -1, blurb: "Legions cost half the manpower" },
  { id: 2, name: "Sappers", cost: 16, tier: 1, prereq: -1, blurb: "The decadence tide creeps half as fast" },
  { id: 3, name: "Production Surge", cost: 30, tier: 2, prereq: 0, blurb: "Sources produce three times as fast" },
  { id: 4, name: "Bounty Mastery", cost: 28, tier: 2, prereq: 1, blurb: "Legions besieging a bountied town grind +50%" },
  { id: 5, name: "Heavy Rail", cost: 36, tier: 2, prereq: 0, blurb: "Unlocks heavy rail — high-capacity arterial track" },
  { id: 6, name: "Siege Doctrine", cost: 32, tier: 2, prereq: 1, blurb: "All besieging legions grind +50%" },
  { id: 7, name: "Standing Garrison", cost: 30, tier: 2, prereq: 1, blurb: "Captured towns cut down raiders" },
  { id: 8, name: "Forced March", cost: 30, tier: 2, prereq: 1, blurb: "Legions march +50% faster" },
  { id: 9, name: "Ley Tap", cost: 24, tier: 2, prereq: 2, blurb: "Aether mints +50% mana" },
  { id: 10, name: "Ward Lines", cost: 28, tier: 2, prereq: 2, blurb: "Your rails cut raiders down from +50% range" },
  { id: 11, name: "Arcane Awakening", cost: 40, tier: 2, prereq: 2, blurb: "Awakens the spell arm — cast Purge, Smite & Warpath (or toggle autocast)" },
];
/** True iff tech `id` is unlocked in the bitset (mirrors tech::is_unlocked; bit == id for the shipped set). */
export const techUnlocked = (bits: number, id: number): boolean => (bits & (1 << id)) !== 0;

/** The spell table — MUST mirror crates/sim/spell.rs (kind id, mana cost). Auto-targeted; the player picks
 *  WHEN to cast (every cast spends mana from the same pool that buys tech). Gated by Arcane Awakening (id 11).
 *  The HUD reads `Stats.mana` to enable/disable each button. `glyph` is the cast-flash hue's sibling. */
export interface SpellDef { kind: number; name: string; cost: number; glyph: string; blurb: string }
export const SPELLS: SpellDef[] = [
  { kind: 0, name: "Purge", cost: 14, glyph: "✷", blurb: "Retreat the decadence tide — clear the corruption nearest the capital" },
  { kind: 1, name: "Smite", cost: 10, glyph: "⚡", blurb: "Strike down a raider that has breached the rail cordon" },
  { kind: 2, name: "Warpath", cost: 18, glyph: "⚔", blurb: "Empower the most-stalled siege to crack a deep garrison" },
];

export const WHOLE_LINE = 0xffffffff;
export const BUILD_MODE = { SURFACE: 0, ELEVATED: 1, TUNNEL: 2 } as const;
export const TRACK_TYPE = { DOUBLE: 0, SINGLE: 1 } as const;
/** Transport mode (Line.mode in the sim). Matches crates/sim trainset::tmode. */
export const TRANSPORT = { RAIL: 0, BUS: 1, FERRY: 2, AIR: 3, HEAVY: 4 } as const;
export type TransportMode = (typeof TRANSPORT)[keyof typeof TRANSPORT];

export function encodeCommand(c: Command): string {
  return JSON.stringify(c);
}
