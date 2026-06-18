# Symmetric enemy AI — design & scope (#13)

**Status:** scoping (no code yet). **Owner ask:** "too aggro from enemy — the enemy should also follow the
same rules as the player (build tracks etc)", plus the hard requirement: **the AI's intent must be
telegraphed.** This doc lays out the honest scope, the design forks, a phased plan, and the determinism /
winnability / legibility constraints, so we pick a direction before writing code (AGENTS: *seams, not
half-built*).

---

## 1. Design pillars

1. **Symmetric — the enemy plays by the player's rules.** It owns a network, earns + spends resources,
   builds/extends track, fields legions, conquers towns — through the *same* `Command`/`apply` machinery
   the player uses, not a bespoke "rot" track.
2. **Telegraphed intent (load-bearing).** The opponent makes **no hidden or instant moves.** Every plan it
   commits to — where it's laying track next, which town it's expanding toward, where a legion is marching
   to strike — surfaces on the map **before it resolves**, through the channel the player already reads the
   legion/raider intent arcs (plus a beat for the big moves). The opponent is a *readable adversary you can
   pre-empt*, not surprise pressure. (This is also the antidote to "too aggro": asymmetric rot feels unfair
   precisely because it isn't legible; a telegraphed rules-player feels *fair*.)
3. **Deterministic.** Every AI decision is integer, index-ordered, keyed-RNG (`ChaCha8Rng` off `seed ^
   AI_CONST`), bit-for-bit on replay — the replay-equality gate stays green.
4. **Winnable + tunable.** A difficulty knob; a `tests/balance.rs` gate that certifies the campaign winnable
   against the AI.

---

## 2. Current state (what we're extending)

The enemy today is the **environment**, on a **single-faction `World`**:

- **Decadence** (`decadence.rs`): a scalar doom-clock (grows ~50/s, pushed back by captured towns, lose at
  20 000) + a spatial-CA **tide** (`decadence_field.rs`, `world.decadence_cells` hashed) creeping from the
  far edge toward the capital.
- **Raiders** (`raider.rs`): up to 64 spawned marauders from the tide reservoir; march at fixed objectives
  (capital / supply-line seams / captured towns); cut down by the player's station cordon. Already a
  *deterministic, telegraphed* mini-AND (raider-intent arcs exist).
- **The player's legions** (`army.rs`): the only thing that *builds toward winning* — fielded from barracks,
  march/ride the player's rails, siege towns.
- **`war_step`** (`ruleset/arcadia.rs:43–71`, called from `tick.rs:52`): the per-tick orchestration —
  `maybe_launch → army_travel_step → army_board → siege → raider::step → decadence`. **This is the seam an
  AI tick hooks into.**

**The crux:** `World` has *one* `tribute`/`manpower`/`mana`, *one* `lines`/`stations`/`vehicles`, and *no
owner field per node*. The player builds all of it; the rival has no build mechanism. A symmetric rival is
therefore **a second faction** — the central rework.

---

## 3. Design forks

| Fork | What the rival is | Symmetric? | Telegraph | Scope |
|---|---|---|---|---|
| **A. Raid+ (stateless)** | smarter marauders from author-placed bases, march at objectives | ✗ (no build/economy) | easy (extend raider arcs) | ~1–2 days, ~500 LOC |
| **B. Hybrid (baked rival realm)** | a pre-authored rival **network it owns**, commands its own legions dynamically (spends, doesn't build) | ½ (owns + deploys, doesn't expand) | medium | ~1–2 wk |
| **C. Command-driven (true 2nd player)** | a faction that **builds + extends track + fields legions** via `Command`/`apply` | ✓ full | full (build/expand/attack all telegraphed) | ~2–4 wk, ~4–6 KLOC |

Fork **C** is what the ask literally describes ("builds tracks, same rules"). It's a multi-week program.
Fork **A** is fast but fake symmetry (still environmental). Fork **B** is the pragmatic middle.

---

## 4. Recommended: phase toward C (each phase ships + is telegraphed)

Rather than a 4-week monolith, build C in shippable phases — each one a coherent, determinism-clean,
*telegraphed* increment. We can stop after any phase.

### Phase 1 — the faction seam + a visible rival realm (~1 wk)
- Add a `u8 faction` (0 = player, 1 = rival) to `Station`/`Line`/the army SoA, hashed. Default 0 everywhere
  ⇒ transit + the current goldens are byte-identical until a rival exists (then a deliberate re-pin).
- Per-faction resource pools (`tribute`/`manpower`/`mana` → a small `Faction` struct or parallel arrays).
- A **baked rival realm** (a `CityData` rival network: its own capital + a few stations/lines, pre-authored
  — *not* dynamically built yet). It earns its own resources and **fields legions toward the player's
  towns** (reusing `army.rs` with the rival faction).
- **Telegraph:** the rival's legion-intent arcs (a distinct enemy hue) + a "the rival marches on ⟨town⟩"
  beat. The player can *see* the threat forming and counter it.
- *Delivers:* a visible, rules-playing opponent that owns a network + attacks legibly — the "fair, readable
  enemy" feel — without the dynamic builder. This is essentially Fork B, but on the faction seam C needs.

### Phase 2 — the AI builder (~1–2 wk)
- A `trait AI { fn think(&World, dt, &mut rng) -> Vec<Command> }` seam (mirrors `Router`/`Demand`), selected
  by `CityData.ai_type` (`"none"` = `NoopAI`, golden-neutral). Ticked as a new sub-phase in `war_step`; its
  Commands flow through `apply` (faction-gated) exactly like the player's.
- A deterministic greedy builder: extend track toward a contested/uncaptured town, place a station/barracks,
  field a legion when manpower clears a threshold — all integer-scored, index-ordered tie-breaks, keyed RNG.
- **Telegraph (the pillar, concretely):** the AI **announces** a build before it commits — a *ghost/intent*
  overlay (a dashed rival-coloured spur to where it will lay track next, an expansion arc to its target
  town) shown for a beat *before* the `Command` applies, so the player reads "it's pushing east — cut it
  off" and acts. Same dashed-blueprint language the player's own draft uses, in the enemy hue.
- *Delivers:* full symmetry — the rival expands its network and contests territory, legibly.

### Phase 3 — win conditions + balance (~3–5 days)
- Territory/standing split + a capture-the-capital / hold-the-realm victory and a stalemate rule.
- A `RivalDifficulty` knob (spend thresholds / build cadence) on `CityData`; a `balance.rs` gate
  (`winnable_against_<difficulty>`). Tune so a competent player beats Normal in the window.

---

## 5. Determinism plan

- AI RNG: `ChaCha8Rng::seed_from_u64(world.seed ^ AI_CONST)` threaded through `think` (never wall-clock,
  never a fresh entropy seed).
- `think` is total + infallible (saturating/clamped, no panics), a pure fn of `(world, dt, rng)`, returning
  a deterministic-length, index-ordered `Vec<Command>`.
- AI Commands mutate via `apply` ⇒ they fold into `Canonical` for free. New faction fields appended-last in
  the hash. Goldens re-pin **once** when a rival first exists (documented, like every prior intentional
  re-pin); `ai_type:"none"` keeps transit + demo byte-identical.

## 6. Winnability / balance plan
- `balance.rs` gains a `winnable_against_normal` gate (player network beats the AI within the decadence
  window). The existing decadence/raider knobs stay; the AI adds a difficulty knob. **Risk:** an AI that
  builds + conquers can deadlock or overwhelm — Phase 3 is where we tune, and the gate is the guardrail.

## 7. Seams (AGENTS-compliant)
- `trait AI` in `crates/sim/src/ai.rs` (new), behind `CityData.ai_type` — mirrors `Router`/`Demand`.
- `Command` gains faction context in `apply` (gate each build/spend on the acting faction); no new wire
  syntax if the AI calls `apply` internally.
- `CityData` gains `ai_type`, `ai_difficulty`, and the baked rival-realm fields.
- Render: a `rivalIntentLayer` (mirrors `armyIntentLayer`) + the dashed expansion-ghost overlay — the
  telegraph.

## 8. Open forks for the owner
1. **How far?** Stop at **Phase 1** (a visible, attacking, legible rival on a baked network — ~1 wk, gets
   the "fair readable enemy" feel), or go to **Phase 2** (the rival actually builds — the literal ask, +1–2
   wk), or **Phase 3** (full contest + tuning)?
2. **One rival or many?** A single rival realm (simplest) vs multiple AI factions (the faction seam supports
   it; more balance surface).
3. **Does the rival contest the SAME towns/resources** (territory war) or run a parallel network it defends?
   (Affects win conditions + how aggressive it feels.)
4. **Difficulty exposure:** a player-facing difficulty setting, or a fixed baked level per scenario?

**Recommendation:** commit to **Phase 1** as the next concrete deliverable (it establishes the faction seam
+ a telegraphed, rules-playing opponent and immediately addresses the "too aggro / unfair" feel), then
decide on Phase 2 once it's in hand. The aggro tune already shipped buys the runway to build this calmly.
