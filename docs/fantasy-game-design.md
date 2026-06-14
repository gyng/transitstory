# The fantasy game — design ("Against the Dark")

> **Status:** design, not yet built. The culmination of a multi-pass design arc (progression →
> supply-chain depth → war machine), each run as understand/design → adversarial review → synthesis.
> This is the GAME design (what it is); [fantasy-fork.md](fantasy-fork.md) is the ARCHITECTURE (how
> it's built — the ruleset-at-construction seam). Companion: [shared-rail.md](shared-rail.md),
> [p5-shared-track-roadmap.md](p5-shared-track-roadmap.md). Obeys every [AGENTS.md](../AGENTS.md)
> non-negotiable; departures from the cosy/monotonic base are deliberate and confined to the fantasy
> ruleset.

## 0. The pitch

*You are an isekai'd transit engineer — the logistics backbone of the empire the world calls **evil**.
The propaganda is wrong. You are the order that holds back the decadence; the **"good" towns** are its
carriers — decadent and corrupt behind a virtuous banner. You never touch a soldier; you command a
rail network, and the network IS the war.* You supply towns; well-fed towns mint tribute (manpower,
gold, mana); a barracks launches AI-controlled legions that ride your rails to the front and besiege a
"good" town, grinding down its resistance; a conquered town is **purged and made to prosper** under
your supply — a new tribute source *and* a liability, because the moment you stop feeding it the **decadence
creeps back** (the old "good" order reasserting itself), spreading town-to-town and marching on your
capital. The whole game is the pull between **greed** (one more conquest) and **consolidation** (hold
the decadence back in what you already took). You point the war by pointing logistics.

> **Tone — the moral inversion (near-zero engine cost; pure writing/contracts).** You are *evil on
> paper* — the dark aesthetic, the menacing legions, the world's fear — but actually the just,
> order-bringing force; the "good"/free towns are the real villains (the decadence). This reconciles all
> three framings (benevolent unifier · the evil empire · evil-on-paper) into one: your conquest
> genuinely *improves* the towns it takes, and the "rebellion" is the corrupt old order clawing back
> when you neglect them. The dissonance — menacing on the outside, benevolent in substance — is the
> charm and the villain-isekai hook; it lives entirely in story beats + contract framing.

## 1. The one loop (everything composes onto one network)

```
supply a town ─► town mints TRIBUTE (manpower/gold) ─► rides your rail ─► BARRACKS accumulates
   ▲                                                                              │
   │                                                              launch_cost reached
   │                                                                              ▼
 captured town = new supply target + tribute source ◄─ town FLIPS ◄─ supply-gated SIEGE ◄─ AI ARMY
   │                                                                                    rides rail + walks
   └───────────── over-extend ─► rear towns unfed ─► DECADENCE rises + spreads ─► you LOSE ground ─┘
```

The war machine is the **apex stage of the supply chain**, not a parallel system. There is exactly
**one new player verb** (place a barracks); everything else is the rail game that already exists:
place nodes, draw rail, assign vehicles + headway, Play, read, tweak.

## 2. Supply chain — "The Forge Line" (the depth)

The middle band between trivial and Factorio. The compulsion is the **moving bottleneck**.

| Dial | Setting |
|---|---|
| Depth | **3 stages** (raw → intermediate → finished → sink) — min depth where a bottleneck can *move* |
| Commodities | **8 fixed**, baked, ordered (id in `citizen_id`). Raw: ORE · GRAIN · AETHER (scarce ley) · FUEL. Intermediate: INGOT (←ORE) · FLOUR (←GRAIN). Finished: **ARMS** (←INGOT+AETHER, the war good) · **BREAD** (←FLOUR+**FUEL**, the staple) + SANCTUM (AETHER+ORE→mana) |
| Node types | **≤8** behind **one** place-node tool + popover (node type *is* the recipe): mine · granary · smelter · mill · sanctum · armory · bakery · **barracks** |
| **Recipe arity** | **2-input** (the key dial) — output gated by the *scarcest* input (Liebig's law) |
| Editable recipes | **No** — fixed-by-type. The hard "stay below Factorio" line |
| Conversion | **rate-based, small per-input-slot i64 buffer** (the one determinism cost) |
| Town demand | **2–3 goods, milestone-gated** |

The Armory's AETHER buffer sits empty (long line from the rare ley-ridge) while INGOT pegs full → the
empty slot **throbs** → shorten the aether line → ARMS flow → but now the Bakery's *FUEL* buffer is
empty (the bottleneck **jumped** to a different chain) → fix that → output back-pressures → add a
cart. Every fix exposes the next link. Played entirely with **Capacity + Headway + place-a-node** —
no recipe editor, no belts, no inventory screen. **The one balance knob that's yours: the buffer
cap** — too small = perpetual failure, *too large silently deletes the puzzle*. Narrow window;
playtest before it ships.

**Disjoint chains (the anti-dominant-commodity fix):** feeding towns (GRAIN+FUEL → BREAD) and arming
legions (ORE+AETHER → ARMS) run through **completely separate upstream chains** — so they compete for
*different* terrain (an ore-rich map = strong army / starving towns, and vice versa), the bottleneck
genuinely *moves* with the map, and no single hex wins everything.

## 3. The war machine — barracks, AI armies, siege, decadence

- **Barracks** = a converter node that sinks **manpower** into an `i64` reservoir; at `launch_cost`
  it `saturating_sub`s and emits **one army**. The reservoir bar *is* the throttle — feed it faster
  (more rail / shorter headway / more upstream towns) and the front advances; starve it and it
  stalls. No timer, no build-queue UI: launch is a pure function of accumulated supply.
- **An army** = a fixed-shape record in a bounded `Vec<Army>` (`MAX_ARMIES` cap): `strength:i32`,
  `state` (RIDING|WALKING|BESIEGING), `target`, position. **RIDING** = a `kind=ARMY` slot injected
  into `VehicleSoA`, gliding up your colored line with the same two-clock alpha-interp as a train.
  At the nearest railhead it **WALKS** the last stretch via the existing `roadnav` grid A*. Visible
  martial spectacle; **zero unit micro**.
- **Rail = power projection (the fusion).** The deterministic AI general targets the highest-value
  town its *score* can reach, where score subtracts `rail_distance` (line arc-length + the A* walk
  cost). A town you haven't railed toward scores terribly → the AI won't march into the wilderness.
  **So "send the army north" is never an order — it's a spur you build.** The off-rail walk is
  **hard-capped in cells** (a far town is *unreachable*, not slow) so "build rail closer" is always
  the lever.
- **Capture = a supply-gated siege.** A besieged town has `resolve:i32`; each tick
  `resolve -= (strength * supply_factor) >> SHIFT`, where `supply_factor` reads whether manpower is
  *still* reaching that railhead this period. An army that outran its supply does ~0 damage and
  visibly stalls. `resolve ≤ 0` → the town flips to your color, becomes a new supply target + tribute
  source, the army disbands. **This makes the supply line the *continuous* verb** — distance has an
  ongoing cost, the network keeps mattering after every capture.
- **The decadence = the loss-aversion (the dread).** A per-town `decadence:i32` (the "good" towns' true corrupt
  nature, held back only by your occupation) rises each day on any town whose fed-this-period flag is
  false, and **diffuses to grid-adjacent towns**; cross the threshold and the town reverts to the old
  corrupt "good" order — you lose its tribute and the reach beyond it. The cure is pure logistics:
  restore supply, tribute purges the decadence back down. The decadence front always creeps **toward your
  capital** (a loud, telegraphed proximity channel); reaching it loses the run. *This is why the decadence,
  not a rival kingdom — it supplies real teeth as one double-buffered integer field, and the engine
  has no town-decay today.* **Optional escalation:** add a global term so the more "good" towns you
  conquer, the faster the *remaining* free world's decadence rises (you're the big bad now) — a natural
  late-game difficulty curve on the same field, no opponent AI.

## 3.5 AI — the general you steer, the dark that pushes back

The player never controls a unit; the AI is the general, steered by **where you lay rail + bounties** (Majesty-style).

- **Friendly legions** run in a new `war_step` sub-phase (locked order: accrue → launch → retarget → move-walk →
  grind → flip). A barracks launches only when its best reachable **targeting score** (`town_value + bounty − rail_cost
  − garrison_resolve`, index-ordered fold, lowest-id tiebreak) clears `ACCEPT_FLOOR` — else it **banks manpower** (the
  launch *vacuum gate*; without it, a rich-reservoir/no-target state is a launch/disband bonfire). A legion that clears
  the floor rides rails → walks the capped last stretch → supply-gated siege → flip → disband. Idle legions
  **garrison-park** at a railhead (a defense bonus that slows the decadence — "defend here" = "rail here"), re-fold each
  tick under **hysteresis** (`SWITCH_MARGIN`; a *besieging* legion never retargets — commitment). Supply cut → stall →
  retreat + a **consolidate** nudge.
- **Enemy = Tier-1-LITE** (the minimum that's a *real* threat): the decadence CA field + static town garrisons + **decadence
  raiders** that *walk* `roadnav` (never `VehicleSoA` → sidestep the army-as-vehicle deadlock) and **sever
  `supply_factor`** — the exact scalar gating your siege grind and decadence purge. So a *well-run* front gets interdicted,
  not just a neglected one; fought purely with logistics (re-supply / re-rail / bounty the cell / legion-cleanse).
  Tier 0 alone (field + static garrison) is a passive nuisance. Rival kingdom deferred — but write `war_step(world,
  owner)` day-one so it's data, not a rewrite.
- **Bounties = the only direct steering verb** (+ live rail): post gold on a town/front → it enters the targeting
  score; the AI may **decline** (influence, not control). A gold sink. No order panel, no unit selection.
- **Legibility (the Majesty fix — ships with the FIRST legion):** a per-Army RATIONALE "why" channel shows the
  integer score breakdown + state; a decline reads as a legible instruction (*"scores 12 < 20 — rail closer / raise a
  bounty"*). **Two hard locks:** (1) surface the *commitment countdown* + held-vs-candidate gap, or hysteresis latency
  reads as "the AI ignored my gold"; (2) tune `ACCEPT_FLOOR`/`SWITCH_MARGIN` **responsive, not stubborn** (a stubborn
  AI passes every test but feels broken — only playtest catches it). Live-build means **rail is also a live lever**,
  which softens the bounty-only-agency narrowness.
- **Determinism + the SIX gate-blind defects (kill RED-first — each passes `run==run` while unplayable):** (1) armies
  OWN their position + are skipped by dispatch PASS-1 (a re-dispatch must not teleport the fleet); (2) **PURGE must
  strictly dominate DIFFUSE** or a fed frontier pins at nonzero decadence forever; (3) the launch vacuum gate; (4) raider
  **frontier steady-state** (no cut/resupply/respawn sawtooth — bounded per-raider ≠ bounded system-wide); (5) bounty
  paid **exactly once** across the grind→flip boundary; (6) `town_value`'s f32 demand-weight is a **cross-build**
  divergence channel blind to both `run==run` and a same-build golden hash → quantize to i64 + a candidate-order
  permutation test. Property tests assert **reaches-zero / bounded-flips / exactly-once**, never merely
  "non-increasing/capped." All else: index-ordered, i64 saturating, keyed RNG (`seed ^ const`), bounded `Vec<Army>`,
  no HashMap iteration, locked sub-phase order.

## 4. Progression

- **Money: soft** — fares/tribute fund player-*chosen* unlocks, never block a build (no un-undoable
  wall). Gold can be a second barracks input for tuning; captured towns expand the soft-money base.
- **Gauge: SPLIT (load-bearing).** A monotonic fixed-denominator **progress** anchor (served / peak
  demand — never punishes a strictly-better empire) **+ a separate volatile front-health channel**
  (territory held, blight-front distance-to-capital) that *can* fall. A single monotonic gauge
  *fights* the dread; two gauges, two jobs.
- **Unlocks: milestone**, spent from earnings — node types, bigger `MAX_ARMIES`, faster carts,
  longer walk-cap, cheaper `launch_cost`. All knobs on existing levers, no new verb.
- **Goals: thin authored isekai prologue → endless procedural contracts** ("liberate/hold this
  region," "sustain a siege at X for N days," "push the blight front back below the river") — each a
  logistics objective, no new verb. **Endless + prestige** (a cleared continent rebakes a harder one
  with longer baseline supply lines = more over-extension pressure).

## 4.5 Build interaction & mode (Factorio-informed; FFF-377/378)

- **Rail planner — drag-to-plan an auto-routed ghost.** Not click-each-stop: drag from a point toward
  a destination, the planner auto-routes the grid track (octilinear + curves) as a live ghost, smart-
  snapping to existing rails ("finds connections you wouldn't expect"), commit on release. Extends the
  existing dashed-blueprint-follows-cursor primitive. Frontend, lattice-agnostic.
- **Grade separation is a build VERB.** Surface/Elevated/Tunnel isn't just a per-span cost flag — when
  a junction jams, route an elevated bypass *over* it (the planner toggles level + routes across): the
  spatial payoff of the capacity puzzle, tied to the P4 mutex / "grade-separate = the fix." Prefer a
  *visible elevated bypass* over a hidden tunnel for legibility (Factorio's own reasoning).
- **LIVE-BUILD with optional pause — the hard Build/Run wall is RETIRED for this ruleset (decided
  2026-06-14).** You build while the sim runs; pause (the existing `SetRunning(false)`) any time to
  plan. This kills the **#1 agency dead spot** (the forced passive-watch) and is the Factorio
  compulsion engine. Consequences:
  - **NOT a determinism problem** — command-sourcing already supports a build Command applied at any
    tick (replays bit-for-bit). The wall was a UX/clarity choice, not a determinism requirement.
  - **Save/replay shape evolves:** the command log gains **TICK-STAMPS** (`seed + [(tick, command)…]`)
    so replay reproduces the live interleaving — the lockstep model, and the multiplayer-aligned
    direction AGENTS already pointed at. (`Canonical` unchanged; `SaveGame` grows, back-compat: an
    unstamped command = pre-Run/build-order.)
  - **Undo shifts** from splice-the-log-and-replay (rewrite history) toward **forward-removal commands**
    (bulldoze exists); true history-rewrite undo becomes a paused/recent affair. Forward-only log is
    cleaner.
  - **"Build mode does not tick" retires** for this ruleset; the two-clocks *render/stats* throttle
    (60fps interp, 1–4 Hz stats) is unaffected.
  - **The one careful case — live-editing a RUNNING line's geometry** while vehicles are on it: the
    edit sets `dispatch_dirty` → the line re-dispatches (deterministic; possibly a small visible
    reset), or gate geometry edits on a segment that currently has vehicles. Design this explicitly.
  - Mode/ruleset-scoped: the cosy transit game can keep its hard wall; the save-contract evolution is
    engine-level but back-compatible.

## 4.6 Economy & tech ladder (the flywheel)

**One number, three hats:** a town's **satisfaction tier** sets its **tribute**, fuels the **war** (food surplus =
recruitable manpower), and buys the **future** (gates tech). The flywheel: bread feeds the town → town pays gold →
gold buys rail → rail carries the legion → legion takes the ridge → ridge gives aether → aether arms the next legion.

- **Three i64 resources** (reuse the `opex_accrued`/`_rem` integer-accrual pattern): **Gold** (the existing balance
  lineage), **Mana** (arcane; gates tech + bounty potency; only from AETHER/SANCTUM towns), **Manpower** (war fuel).
- **Income:** **tribute** (per-town, scaled by satisfaction tier — a sullen town pays a floor, a fed town up to
  ~250%; anchored so one fed town ≈ funds one train of opex) + a one-shot **conquest plunder** that **vests only
  while you hold and feed the town** (no raze-and-run; decays on re-capture). Fares = a minor trickle.
- **Soft money:** capital *debits* gold but **never blocks a build** (the reject-wall is behind an opt-in
  `hard_money` flag). Sinks: empire-scaled opex, bounties (one-way gold→influence), tech-spend (one-way gold+mana).
- **The brake — five integer governors** (must press *both* expander and turtle, and survive a bulldoze): (1)
  **empire-scaled opex** (per town/legion/km — the expander brake); (2) **tribute diminishing-returns by
  supply-distance** (hops from railhead via the Router BFS — fix = lay more rail); (3) a **global decadence-tide advancing
  into all ground over time** (the turtle-killer — standing still loses ground); (4) a **separate monotonic hashed
  `debt_accrued`** (NOT interest on the recompute-able balance, which a bulldoze would launder away) — accrued debt
  survives, throttles tribute while in the red; (5) **tribute saturation cap** so a turtle can't bank the capstone
  techs (which gate on territory/tribute-total) → expansion is forced. No hard-stops; the curve bends so the snowball
  needs continuous steering.
- **Tech ladder:** one new Command **`UnlockTech{id}`** flips a bit in a **hashed unlock bitset** that gates which
  Commands the Toolbar *offers* (UI-only gate; sim stays permissive). Two-phase: a milestone **reveals** a tech
  (free, automatic, from already-tracked stats); you **spend** gold/mana to **unlock** it. Four competing trees:
  **Logistics** (Cart Roads → *Iron Rails* [the isekai "aha"] → Block Signals → Elevated [decadence-immune] → Tunnels),
  **Military** (Conscription → Drill Yards → Troop Trains → Standing Legions), **Industry** (Smelter&Mill →
  Granary/Bakery/Armory → **Two-Input Recipes** [turns the moving-bottleneck on] → Roundhouses [opex relief] → Bulk
  Freight), **Arcane** (Mana Tribute → Ley-Line Signals [a *placed node* biasing junction tiebreaks — the deferred
  player-signal as a node, NOT an editor] → Teleport Relays). **Every entry is a knob** (an integer-rational delta —
  ÷2, ×3/2, ×7/10 — read off a hashed bit; zero float) **or a new type in an existing enum slot — never a new verb or
  panel.** SoA pre-sized for post-unlock maxima; dispatch stays the live clamp.
- **Balance spec = four RED property tests** (the adversary's flaws, now the gate): net-income-per-turn stays in-band
  as `town_count` grows · tribute is monotonic-in-tier but **saturating** (not increasing) · cumulative
  conquest-gold ≤ cumulative bounty-spend on those conquests · node output ≤ scarcest input. Plus: economy-off ⇒ all
  new accrual early-returns ⇒ transit byte-identical.

## 5. The locked decisions

| | Decision | Pick |
|---|---|---|
| Army mobility | rails vs free terrain | **Ride your rails + hard-capped last-stretch walk** |
| Defense / opponent | PvE vs the decadence vs rival | **The decadence + ENEMY AGENTS** — asymmetric: enemy has NO isekai engineer, so their economy IS the decadence (decadence-fed spawning) and transport is primitive (roadnav marching + corrupted-corridor "rail-lite" shortcuts). You starve them by purging the decadence via supply. Rival kingdom = deferred seam |
| Lattice | square vs hex | **HEX** (owner taste, 2026-06-14) — area-control decadence creep + contested borders are organic *natively*; roadnav A* simpler (6-nbr). Cost: port the just-built square grid geometry (grid_walk→hex line-draw, node_of→axial, grid A*) + re-pin grid/cross-line tests. **Hazards (correctness): mm→axial float-round (use f64-then-.round()-to-i64 + replay pin); hex line-draw MUST be canonical-symmetric (sort the pair first) or the cross-line mutex silently disengages.** Cross-line mutex *logic* + arc-length authority layer + VehicleSoA + render are shape-agnostic (untouched). Transit/OSM game stays continuous. |
| Area-control | render-effect vs dense field | **Dense contested-cell field (core)** — the visible center you optimize toward, acted on ONLY through rail+supply+barracks+bounties (output, never a commandable input). Hex makes the creep organic for free |
| Friendly idle legions | passive vs auto-defend | **Auto-defend** — an idle legion gravitates (defense-score mirror) to garrison the most-threatened owned frontier town / rail corridor, repelling enemy marchers + resisting decadence |
| Steering the AI | orders vs bounties | **Majesty-style bounties** — post a gold reward on a town/front; the AI weighs it into targeting (may decline). A gold SINK. The only strategic-agency verb; no unit orders, no order panel |
| Capture | walk-in vs siege | **Supply-gated resolve siege** (walk-in = playtest fallback) |
| Capital | aggregator vs threatenable | **Aggregator + corruption lose-condition** (telegraphed) |
| Gauge | one monotonic vs split | **Split** (monotonic progress + volatile front-health) |
| Front/priority flag | ship vs defer | **Defer** (pure rail+supply steering; a flag invites the order-panel RTS trap) |
| Scale / framing | continent vs regions | **One contiguous continent**; the **"evil" empire that is actually the just order** (evil on paper; the "good" towns are the decadence) |
| Build mode | hard Build/Run wall vs live-build | **Live-build + optional pause** (kills the passive-watch dead spot; save gains tick-stamps; undo → forward-removal) |
| Build interaction | click-stops vs planner | **Drag-to-plan auto-routed ghost** + grade-separation (elevated bypass) as a build verb |

## 6. Determinism & thin-loop rails (where it lives or dies)

This is the **biggest new sim subsystem** in the project. The hard constraints:

- **Golden-hash first (the sharp edge):** *no golden-hash constant exists anywhere today* —
  `determinism.rs` is pure `run()==run()`, provably **blind to a uniform hash shift**. Land a fantasy
  golden-hash `u64` literal + a committed ticked save artifact **RED, before any conquest field**;
  diff transit's hash to zero through the carve.
- **The army-as-vehicle bypass (the #1 verified risk):** block deadlock-freedom is guaranteed
  *upstream* by the dispatch single-track capacity cap (`vehicle.rs:127`). An army injected into
  `VehicleSoA` that **bypasses** that cap makes a deterministic deadlock *reachable* (passes
  `run==run`, unplayable). So thread the `kind=ARMY` byte through **every** `VehicleSoA` consumer
  (board_alight skips boarding, dispatch skips headway-counting, P1–P4 meet/junction/cross-block
  mutexes skip-or-account army slots) **and** admit armies to single-track blocks through the *same*
  occ-cap accounting — never reinvent occupancy.
- **Blight diffusion is NOT a `grow()` clone:** `demand::grow` is per-cell *independent* (no neighbor
  coupling), so naive in-place diffusion is iteration-order-dependent. **Double-buffer** it (read
  prev-day, write next-day); explicit index-ordered grid-neighbor lookup; RED test: identical blight
  field after K days twice in one process.
- **Integer + index-ordered + keyed-RNG + bounded:** all combat/siege/blight/accrual math is i64/i32
  saturating; AI targeting is an index-ordered fold with a lowest-`TownId` tiebreak (never HashMap
  iteration); any jitter uses a keyed `seed ^ WAR_CONST` sub-stream, never `world.rng`; the army Vec
  is fixed-cap (launches clamp, never grow).
- **Lock the `war_step` sub-phase order** (launch / move-walk / grind / flip) as strictly as
  `tick.rs`'s six phases — "free-then-launch" vs "launch-then-free" silently shifts the hash.
- **Liveness is gate-blind** → RED property tests: never-livelock (a supplied army's distance-to-
  target is monotone non-increasing); captured-count monotone under a favorable log; supply-severed
  siege damage → ~0; over-extension invariant (a longer-but-equally-supplied frontier doesn't score
  higher); deterministic blight field; tied-score AI picks the same target across builds.
- **Thin loop held:** the only new build surface is *place a barracks* (one node). **No** unit-
  command UI, **no** order-giving, **no** battle micro, **no** front/priority panel — those turn a
  logistics builder into an RTS. The off-rail walk hard-cap is what keeps "build rail" the steering
  lever instead of an order.
- **New hashed `Canonical` fields** (fantasy-ruleset-owned, fixed audited order): a `kind` byte per
  vehicle slot; the ArmySoA; per-barracks `manpower_stock`; per-town `owner`/`resolve`/`blight`/
  `fed_this_period`; `capital_stockpile`. Inert unless `grid_cell_mm > 0` ⇒ **transit byte-identical,
  zero transit re-pins**. New Command: `PlaceBarracks` (+ `contract.rs` partitioned tag set +
  `types.ts`/`codec.ts` mirror, validate-gated before `cmd_log.push`, in one commit).

## 7. Build sequence (honest — this is CORE-heavy)

The "near-zero new code" of the early progression slices was the *transit-reuse* path. This game is
the fantasy ruleset, so the core carve is on the critical path.

0. **Prerequisite — the Ruleset/Demand trait carve is UNBUILT** (only `Box<dyn Router>` exists;
   `tick.rs` phases are hardcoded with no dispatch hook). Do `fantasy-fork.md` Steps 0–4: golden-pin
   → trait scaffolding (defined, not called) → default `TransitRuleset` byte-identical → the Step-2
   carve (move `coverage_score`/`line_cost`/demand behind traits, hash hex diff zero).
1. **Forge Line slice** — the 3-stage multi-input supply chain + the per-input buffer Canonical
   change + `PlaceNode` + the re-authored `SupplyChainDemand` spawn (route commodity C → nearest
   C-consuming node). Manpower is one of its commodities.
2. **War machine first slice** (§8).
3. **Slice two** — full multi-candidate AI scan + active blight pushback/recapture + the capital
   lose-condition.
4. **Slice three** — multi-commodity barracks recipes; the rival kingdom as a later owner-faction
   seam (same `war_step` over `owner != PLAYER` rows).

## 8. First slice — capital + south town → first capture

Bake `arcadia_world.json` (`ruleset:"fantasy"`, `grid_cell_mm > 0`, synthetic origin, lattice-
snapped): a **capital** (hosting the first **barracks**) at center, one player-owned **south town**
already fed (manpower flows from turn one), one neutral **north town** a *short* (hard-cap-respecting)
walk past a railhead, a faint blight seed. The session: feed the south town → it mints manpower → draw
rail south-town → barracks → reservoir fills → at `launch_cost` a column spawns (flash + chime) and
**glides up your line**, peels off, walks the last cells → the supply line holds so the resolve ring
drains fast (~2 in-game days) → **the town flips**, recolors, the front-health gauge jumps, a new
target lights up north → two tribute sources now feed the barracks → the next launch comes sooner.

**Scope for slice one (defer the rest):** single-commodity manpower (no multi-input yet), ONE target
town (no multi-candidate scan), supply-gated siege, ONE barracks, small `MAX_ARMIES`, blight present-
but-slow (channel + signal, no active recapture yet), no front flag, capital aggregator-only (lose-
condition wired but unreachable on this board). Proves end-to-end: the tag selects fantasy + replays
to the fantasy golden hash; tribute rides the Pax/Router/VehicleSoA/board_alight spine unchanged;
barracks accumulate-and-launch as a pure supply function; army rides then walks (with the `kind` guard
+ occ-cap admission); supply-gated siege flips ownership; the flip lights the next target.

## 9. Open decisions still genuinely yours
- **Rival kingdom** — ship only if you commit to it as a pillar (it's the strongest single hook — a
  race with a face — but the biggest gate-blind risk: a livelocking/oscillating rival passes
  `run==run`, and a visible enemy army invites the defend-order RTS trap). Default: corruption first,
  rival deferred.
- **Siege pacing** — supply-gated grind vs walk-in fallback; resolved by playtest (mitigate dead-air
  with a fast-draining ring + a stalled-army retreat timer that frees the slot and prompts
  "consolidate").
- **The buffer cap** (§2) — the one non-derivable balance knob; budget playtest iteration.
