# TTD L5 — placeable block signals as the next block-keying graduation (de-risked plan)

**Status:** PLANNING (2026-06-17). Written after L3 (segment-authoritative geometry) and the L4
resolution (graph-routing primitives landed; berth-overtake deferred to a cross-line workstream —
see `ttd-l4-plan.md` RESOLUTION). L5 is the last TTD piece. This is the green-at-every-commit
execution spec; it follows the L3 de-risking discipline exactly.

## What L5 delivers (the gameplay value)

**CORRECTED MODEL (2026-06-17), after pressure-testing the original framing against this sim's
mechanics — read this before implementing L5b.** A player-placed signal subdivides a single span into
SUB-BLOCKS so that **SAME-DIRECTION trains can follow closer** (one consist per sub-block, like TTD
block signals) — raising single-track throughput. This is the real, correct value: today the meet
mutex (vehicle.rs Phase A.1 + Phase B) claims `seg_key(line,path,span)` DIRECTION-AGNOSTICALLY, so a
SINGLE span admits exactly ONE consist at a time regardless of direction ⇒ single-track same-direction
capacity is one-train-per-station-span. Signals lift that to one-train-per-SUB-block.

**A signal does NOT let OPPOSING trains pass.** Opposing passing requires a PASSING LOOP (a double-track
sub-section) — a separate lever that already exists at station/segment granularity via
`SetSegmentTrack` (see `single_track.rs::opposing_trains_meet_at_a_middle_single_span`: double-track
spans flanking a single span ARE the passing places). The original plan's "opposing trains meet at a
signal" was WRONG — admitting opposing consists into adjacent sub-blocks of one single track is a
head-on. **So in L5b the OPPOSING exclusion stays WHOLE-SPAN; only SAME-DIRECTION admission graduates
to per-sub-block.** (A future "placeable passing loop" — a sub-span double section — is the separate,
correct opposing lever; out of scope here.)

The value lives ENTIRELY in the occupancy re-keying (an inert "place a marker that does nothing" is
valueless) — so L5 is a deliberate determinism-core batch. Signals are already DERIVED + rendered as
occupancy markers (`SignalOccupancy`, render.ts `signalLayer`); L5 promotes them to PLACED, block-
defining objects.

## The block-keying graduation (the reusable primitive, one more KEY)

The occ primitive (`occ_claim`/`occ_owner`/`try_claim` + the depth-1-forest no-rest discipline) is
unchanged — only the KEY graduates, exactly as it did per layer
(`(line,path,span)` → `(line,key_station)` → `edge_key` → `TrackSegmentId` → `(station,berth)`):

- **Today:** a single span is one block, keyed `seg_key(line,path,span)`, claimed DIRECTION-AGNOSTICALLY
  ⇒ only ONE consist (any direction) at a time; others rest at the bounding-station gate.
- **L5 (corrected):** N player signals split a span into N+1 SUB-BLOCKS at the signal arc-lengths.
  - **SAME-DIRECTION** following keys by `(line,path,span,sub_block)` ⇒ a follower may enter a FREE
    sub-block behind a leader in an adjacent sub-block (closer following = the throughput gain). A
    signal arc-length is a following-gate where a denied same-direction train rests owning nothing.
  - **OPPOSING** exclusion stays WHOLE-SPAN: an opposing consist may not enter ANY sub-block while the
    span holds a consist of the other direction (no head-on; passing needs a loop, not a signal).
  - The depth-1-forest argument carries: every blocked train rests at a GATE (station OR signal) owning
    nothing; the whole-span opposing exclusion is unchanged in structure (just an additional
    same-direction relaxation within a span), so no new wait-for cycle is introduced.

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

- **L5a — command + inert hashed store (NO behaviour, NO UI). [LANDED 2026-06-17]** Added
  `PlaceSignal`/`RemoveSignal` (+ `SignalPlaced`/`SignalRemoved` events), the `Signal` struct + the
  sorted+deduped `world.signals` store appended LAST to `Canonical`, the `apply` arms (validate the
  signal lies strictly inside an existing span; idempotent dedup), the dispatch-dirty exemption, and
  the TS/`contract.rs` mirror. Signals are RECORDED + replayable but do NOT yet re-key occupancy.
  Per the established `forge_stock`/`track_segments` append-last convention, the empty-slice store
  **re-pins both goldens EXACTLY ONCE** (transit `0xea39…`→`0x9d4e…`, arcadia `0xbc3c…`→`0x94e8…`) —
  a pure serialization shift; the K=1 + K=2 **position fingerprints are byte-identical**, proving zero
  behaviour change. Tests in `placed_signals.rs`: validate/reject, place→remove hash-neutral,
  command-order- + dedup-invariant hashing, signal-bearing log replays bit-for-bit.
- **L5b — same-direction sub-block following (the MOTION change). [LANDED 2026-06-17]** Implemented as
  a RELAXATION layered on the unchanged whole-span meet mutex (mirroring the A.3 berth relaxation): when
  the base mutex would DENY entry, a SAME-DIRECTION follower on a SIGNALLED span is admitted past the
  gate (opposing is NEVER relaxed → no head-on), then P1 governs spacing inside the span. Inert without
  signals ⇒ goldens + K=1/K=2 fingerprints BYTE-IDENTICAL (no re-pin); a new `SIGNALLED_FOLLOWING_
  FINGERPRINT = 0x77d0_16f8_2198_cf36` pins the with-signals motion. **Adversarial review: SAFE** (no
  head-on, no deadlock, no determinism hole). Two findings folded in: (i) the load-bearing effect is the
  ADMISSION (conditions 1+2) — P1 normally provides the tighter clamp, so the sub-block far-gate clamp
  (conditions 3+4) + the B.5 sub-block branch are defence-in-depth (KEEP them; they bind if P1 is ever
  loosened; their dir<0 math is pinned by `vehicle::l5b_subblock_math` unit tests). (ii) `PlaceSignal`
  stays dispatch-dirty-EXEMPT — a signal must not re-dispatch (it would reset running trains), pinned by
  `placed_signals::placing_a_signal_mid_run_does_not_redispatch_or_reset_trains`. RED-first gates landed:
    1. `signals_raise_same_direction_single_track_throughput` — a long single span, several
       same-direction trains on a demand corridor, WITH vs WITHOUT a mid-span signal: with the signal,
       MORE trains occupy the span concurrently (sub-block following) and cumulative ridership is
       strictly HIGHER over the window. RED today (one-train-per-span caps it).
    2. `no_head_on_with_signals` — opposing trains on a signalled single span: NEVER two OPPOSING
       consists inside the span (the whole-span opposing exclusion must survive sub-block keying). A
       signal must NOT become an opposing passing point (that's a loop). STUB proof: relax opposing to
       per-sub-block ⇒ head-on ⇒ RED.
    3. `signalled_single_track_never_freezes` — over-provisioned signalled single line: the
       depth-1-forest no-rest holds across signal gates ⇒ never deadlocks. STUB: rest a denied train
       strictly inside a sub-block (not at a gate) ⇒ a 2-cycle ⇒ RED.
    4. No-signals neutrality: with no signal on a span the sub-block keying degenerates to the whole
       span ⇒ the K=1 + K=2 position fingerprints AND the goldens stay byte-identical (the signal-free
       goldens never place a signal, so L5b adds NO re-pin beyond L5a's append). Assert against the
       pinned constants.
  The with-signals motion is pinned by a NEW position fingerprint on a signalled scenario; the existing
  goldens stay byte-identical (they place no signals) ⇒ NO further golden re-pin at L5b.
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
L5a (LANDED) did the one-time empty-slice append re-pin (the store joins `Canonical`). L5b is the
liveness-critical core re-keying — land its RED-first throughput + no-head-on + never-freeze gates
FIRST (each with its stub proof), then the direction-aware sub-block keying, then pin a NEW with-signals
position fingerprint. **L5b adds NO golden re-pin** (the signal-free goldens never place a signal, so
the sub-block keying degenerates to whole-span and they stay byte-identical) — the no-signals K=1/K=2
fingerprints + goldens are the silent-drift tripwires. L5c (UI place/remove) + L5d (balance) are
frontend/separable. L5b is the determinism heart and warrants dedicated focus + adversarial review.
