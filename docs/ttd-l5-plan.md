# TTD L5 — placeable block signals as the next block-keying graduation (de-risked plan)

**Status:** PLANNING (2026-06-17). Written after L3 (segment-authoritative geometry) and the L4
resolution (graph-routing primitives landed; berth-overtake deferred to a cross-line workstream —
see `ttd-l4-plan.md` RESOLUTION). L5 is the last TTD piece. This is the green-at-every-commit
execution spec; it follows the L3 de-risking discipline exactly.

## What L5 delivers (the gameplay value)

**A player-placed signal subdivides a single-track span into sub-blocks, creating a mid-span passing
point** — so two opposing trains can MEET at a signal instead of only at the bounding stations. That
raises single-track line capacity (more frequent service without paying for double track everywhere)
— OpenTTD's core single-track lever. Today a "block" on a SINGLE span is the WHOLE span between two
stations (`seg_key(line, path, span)`, vehicle.rs Phase A.1 / Phase B meet mutex); the only passing
places are stations. L5 makes the player able to add passing points anywhere on a span.

The value lives ENTIRELY in the occupancy re-keying (an inert "place a marker that does nothing" is
valueless and misleading) — so L5 is a deliberate determinism-core batch, not a cosmetic add. Signals
are already DERIVED + rendered as occupancy markers (`SignalOccupancy`, render.ts `signalLayer`); L5
promotes them to PLACED, block-defining objects.

## The block-keying graduation (the reusable primitive, one more KEY)

The occ primitive (`occ_claim`/`occ_owner`/`try_claim` + the depth-1-forest no-rest discipline) is
unchanged — only the KEY graduates, exactly as it did per layer
(`(line,path,span)` → `(line,key_station)` → `edge_key` → `TrackSegmentId` → `(station,berth)`):

- **Today:** a single span is one meet block, keyed `seg_key(line,path,span)`. Opposing trains may not
  both be inside it; the loser rests at the bounding-station gate.
- **L5:** N player signals on a span split it into N+1 SUB-BLOCKS at the signal arc-lengths. The meet
  mutex keys by `(line, path, span, sub_block)`; a signal arc-length is a PASSING-PLACE GATE (like a
  station gate) where the denied opposing train rests owning nothing. The depth-1-forest argument
  carries: every blocked train still rests at a GATE (station OR signal) owning nothing.

## State + command surface

- **`Command::PlaceSignal { line, path, span, at_mm }` + `RemoveSignal { line, path, span, at_mm }`**
  (+ the `SignalPlaced`/`SignalRemoved` event mirror, like every other command). Validation at the
  `apply()` boundary: the span must exist and be SINGLE (signals on double track are a no-op / rejected
  in this slice — double track already passes), `at_mm` strictly inside the span. Mirror into
  `types.ts` + `codec.ts` + `contract.rs` in the SAME commit (the hand-mirrored-contract rule).
- **`world.signals: Vec<Signal>`** where `Signal { line: LineId, path: u8, span: u32, at_mm: i64 }`,
  index-ordered, integer-only. Joins `Canonical` **APPENDED LAST** (the world.rs:412 discipline:
  empty slice ⇒ re-pins exactly ONCE, every prior field keeps its byte offset). **Default empty ⇒ the
  transit/arcadia goldens + the K=1 position fingerprint are byte-identical** until a player places a
  signal — the same inertness as `BuildPlatforms{k:1}` and `forge_stock`.

## Sub-steps (smallest independently-green commits)

- **L5a — command + inert hashed store (NO behaviour, NO UI).** Add `PlaceSignal`/`RemoveSignal`, the
  `signals` store + Canonical append, the `apply` arms + validation, the TS/contract mirror. Signals
  are RECORDED + replayable but do NOT yet re-key occupancy. **Golden-neutral while empty** (assert: a
  log with no PlaceSignal is byte-identical; a PlaceSignal log replays bit-for-bit; placing then
  removing returns to the empty hash). Re-pin is deferred to L5b (when signals first affect motion).
  RED-first: a `signal_store_replays_and_empty_is_neutral` test.
- **L5b — the meet mutex keys by signal sub-block (the MOTION change + the liveness-critical core).**
  Phase A.1 occupancy + Phase B meet gate + Phase B.5 no-rest re-keyed from whole-span to
  signal-subdivided sub-block; a signal arc-length becomes a passing-place gate. **RED-first liveness
  gates FIRST** (a gate-blind deadlock replays green):
    1. `mid_span_signal_lets_opposing_trains_pass` — a long single span, a mid-span signal, opposing
       trains on a demand corridor: NO head-on (never two opposing inside one SUB-BLOCK) AND cumulative
       ridership strictly rises (the meet resolves AT the signal, not just at stations). RED today
       (no mid-span passing ⇒ lower throughput / the test's tighter headway starves).
    2. `signalled_single_track_never_freezes` — over-provisioned single line WITH signals: the
       depth-1-forest no-rest property holds across signal gates ⇒ never deadlocks. STUB proof: make a
       denied train rest STRICTLY INSIDE a sub-block (not at the signal gate) ⇒ a 2-cycle ⇒ RED.
    3. K=0-signals neutrality: with no signals the sub-block keying degenerates to the whole span ⇒
       the K=1 position fingerprint + goldens stay byte-identical (assert against the pinned constants).
  **The ONE earned golden re-pin** lands here (signals first enter a hashed scenario's motion), as a
  deliberate documented commit + a NEW position fingerprint on a with-signals scenario.
- **L5c — UI: place/remove a signal on a single-track span** (the diegetic toolbar gesture; snap to a
  span, click to drop a signal; render placed signals distinctly from the derived occupancy markers).
  Frontend-only, golden-neutral. Cause→effect: place a signal ⇒ opposing trains visibly pass there.
- **L5d — single-track capacity/cost balance** (a placed signal raises the line's effective single-track
  throughput; small capital cost per signal). Tunable, behind the existing economy.

## Gates + discipline (carried from L3/L4)
- Two orthogonal gates: the **state-hash goldens** move ONLY at L5b's earned re-pin (a new Canonical
  field made non-empty in a hashed scenario); the **position fingerprint** (K=1 / no-signals) MUST stay
  byte-identical through L5a AND L5b (no-signals path unchanged) — drift = silent regression, STOP.
- Liveness NEVER inferred from replay: every L5b liveness gate is RED-first on a DEMAND corridor
  asserting ridership rises, and proven non-vacuous by a stub that makes it go RED.
- `tick()` never panics: signal lookups are bounds-checked; validation lives at `apply()`.
- Determinism: `Signal` is integer-only (i64 mm), index-ordered; no float / HashMap-iteration / wall
  clock enters the hash. Signals sort canonically (by line,path,span,at_mm) for the sub-block split.

## Sequencing
L5a is golden-neutral plumbing (safe to land first). L5b is the liveness-critical core re-keying + the
ONE earned re-pin — land its RED-first never-freeze + head-on gates FIRST, then the keying, then re-pin
the goldens deliberately with a NEW with-signals fingerprint. L5c/L5d are frontend/balance, separable.
The no-signals position fingerprint is the silent-drift tripwire throughout.
