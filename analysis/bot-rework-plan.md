# Bot AI rework assessment and staged plan

Sections 1–3 retain the November 10 pre-carve assessment. The November 11
[stage 1 design and checkpoint](bot-carve-design.md) records the implemented
ground pipeline, the declared hash move and the equivalence checks. [Issue
#108](https://github.com/sprucey149/openbf/issues/108) tracks physical recovery
and difficulty acceptance. Section 3's proposed values are OURS and PROVISIONAL.
Measure: both proxy sides in the prescribed [four-map and BF1 cohorts](faction-balance-2026-11.md).


Sources I read: `analysis/bot-design.md` (BOTD, with its amendments through D25), `analysis/difficulty-proposal.md`, `analysis/bot-quality.md`, `re-soldier-ai.md` §0/1.4/1.5/13, `re-ai-think-rate.md` §0, the headings of `re-ai-difficulty.md`, `re-ai-sniper.md` and `re-ai-class-selection.md`, the dated November 5 playtest and pre-difficulty sprint order, the current code under `engine/crates/core/src/ai/` plus `perception.rs`, `skill.rs` and `commander.rs`, and the five fix branches (diffs below). I built nothing and ran no cargo.

---

## 1. Honest assessment

### 1.1 What the October design got built

Most of the leaf layers and measurement tools from BOTD's 16-card list now exist:

| BOTD layer / card | State on main | Where |
|---|---|---|
| SPL11 seams: one MoveRequest, one AimRequest | built | `ai/mover.rs` (1,442), `ai/gunner_foot.rs` (1,336) |
| HUBP, MOVR leg aim and arrived latch, RETW, ARCM, UNSN, WHY | built | `route/`, `mover.rs`, `arcm.rs`, `unseen.rs`, `why.rs` |
| TPOS position table and Tactician | built, for fights only | `tactic.rs` (1,435). Its header says Capture, Guard and Overwatch at positions are "Not yet" |
| TEAM unit grid, Cover and Flank roles, told contacts | built | `team.rs` (612), `perception::tell` |
| TURR, SNPR, GRAI, FIX | built as separate modules | `man.rs`, `snipe.rs`, `arsenal.rs`, `fix.rs` |
| HYST (wake plus position hysteresis) | built | `wake.rs`, `tactic::choose` |
| Difficulty: TOKN, DFOCUS, DTEAM, DPRX proxy, REACT, DRETIRE | built | `tokn.rs`, `dfocus.rs`, `skill::team_skill`, `proxy.rs` |

### 1.2 What did not get built, and it is the part that mattered

**The Behaviour layer was never built.** BOTD §3-5 put it at the centre: one behaviour runs, with an entry, an exit, a minimum time, and interrupt-and-resume. On main:

- `AiState::behaviour` is documented as "DIAGNOSTIC ONLY - never hashed, never read by a rule" (`ai/state.rs`, Behaviour enum at :774). It is a label worked out at the end of the tick from whichever link happened to walk the bot (`brain.rs` ~896-930).
  - Two rules read it anyway: `droideka::takes_fight` (:31) and `droideka::steer` (:47) read last tick's label. So unhashed state now steers the sim, through a one-tick feedback loop.
- There is no minimum time, no resume stack and no "one change per think". Commitment exists only in pieces: `HoldState::commit_until`, the Tactician's 15 % margin, and the target's 1.5 s hysteresis.
- `behave/brain.rs` is 1,012 lines. `chain()` runs from line 56 to about 952, so it is one function of roughly 900 lines with about 140 branch points. 33 commits have touched it since 2026-10-01.
- The October diagnosis ("one function, many writers") is still true, and on the route it is worse:

| state | Oct (M7) | now |
|---|---|---|
| `st.route =` assignments | 24 sites | 34 sites in 9 files (40 counting `.route =`, 11 files) |
| `st.hold =` | n/a | 20 sites in 5 files |
| `st.fight.mode =` | n/a | 15 sites in 3 files |
| `AiState` | 976-line file | 51 public fields. `HoldState`'s 11 fields serve six meanings: fight spot, Tactician spot, guard post, sniper perch, react step, hub hold |
| tuning constants in `ai.rs` | n/a | 115 |

- **The goal is still an implicit priority chain.** `brain.rs` 440-575 takes an `Option` from each source (return-to-level, flee, pickup, hold, board, mend, ring, snipe spot, fix, dfocus push, command, route leg, post, memory) and merges them with chained `match`/`.or()`. A second chain (575-660) picks the aim, and a third (896-930) labels the result.
- **"Sole writer" holds only in name.** The mover writes the move sticks (17 pad writes), but then:
  - `fix::serve` replaces the whole pad after the chain (`brain.rs` :38).
  - `droideka::steer` zeroes `lx` and clears five buttons after the mover (:41-46).
  - `jet::run` writes the pad from inside the mover.
- **The October mover (4.7) was not built.** That means barrier corners tested at every step, velocity-space avoidance of soldiers and moving vehicles, and a probe only on ground not already checked. The `avoid.rs` header (1,247 lines) says so ("not built ... card OBST2"). Instead it is a sweep-based wall follower that tries many headings. PERF.md shows EPA still takes 73-78 % of the slow ticks on Yavin and Mygeeto, from `avoid::pick`. Issue #108 (stuck bots) is still item 3 of open work.
- **Information leak.** `hunt` is the nearest enemy over every live unit, omniscient by design for the hunt goal. It also feeds the holder's look: `h.look = p + 1.4 Y`, so a holding bot with nobody in sight faces the nearest enemy it has never perceived. C4 only stopped it pitching through floors. It still turns toward enemies behind walls.
- **Tier behaviour is borrowed from the measuring tool.** `Tier::{picks, mend, hunts}` are the proxy's player-like rules ("[ours] PROVISIONAL: also serve the tier columns", `proxy.rs`). Elite therefore "attacks the nearest unheld post, never defends" because the stand-in for a human does. The difficulty levers spread across about 25 read sites (perception 7, proxy 7, position 3, tactic 2, brain 2 and others).

### 1.3 What this week's fixes say about the structure

Each fix is reasonable on its own. Together they show the same three gaps.

| fix | what it had to do | what that says |
|---|---|---|
| C1 far-hit reaction | Adds a flinch field (`flinch_until`) and moves a contact's `first_seen` into the future to model "find the shooter". Gated by a new `hit_reaction_applies()` that excludes seats, proxies and space by its own test | A reaction is a behaviour (Investigate, or a hit reaction with a duration). It was faked by editing perception timestamps, because there is nowhere to put a short behaviour with an exit |
| C3 walk backwards | 33 lines of filters (`walking_on`, `faced`, `faced_memory`, `walks_past`, `fight_reach`) put in front of the aim `match` | Whether the bot faces its target depends on what it is doing. With no Behaviour owner, the AimRequest is rebuilt from scratch at every caller |
| C4 sniper floor | `memory_blocked` (one ray per think, hashed), lane look for snipers, a vertical-axis slew, `can_aim`. Gated by `ground_round()` | The same problem in a third place, plus the hunt leak above. "What can I see or engage" is worked out ad hoc in four places |
| C5 sniper lead | A `swept` flag passed through `seen_aim`/`droid_aim`, gated by `!space_round()` | Fine as a gunner fix. It still needed its own space test |
| B1/B2 bot damage | Bots never get the head or critical-prim multiplier on ground rounds. Defines another `space_round()` | Correct and aligned with D2. It also lowers Hard and Elite strength, which moves every difficulty band (see 1.4) |

**Four spellings of "is this a space round" landed in one week:**

1. C1: `space_assault.is_none() && landing_regions.is_empty()`.
2. C3 and C4: `Sim::ground_round()`, which checks the space-carry CTF flag. The two branches add the same text.
3. C5: `Sim::space_round()` in `sim/state.rs`, true for any CTF or any landing regions.
4. B1/B2: a second `Sim::space_round()` in `sim/combat.rs`, true only for the space-carry CTF flag.

Problems this causes:

- **C5 and B1/B2 define the same inherent method in two `impl Sim` blocks.** They do not conflict as text, but merged together they will fail to compile (duplicate definition). They also disagree on meaning.
- C5 places the new function between `add_hazard`'s doc comment and `add_hazard`, so the grenade text now documents `space_round`.

Since LightVelox's space work requires identical space behaviour, this needs one round-rules value, resolved once per round.

**There is no pinned space hash.** `game/tests/suite/space_assault.rs::a_space_round_replays_to_the_same_hash` only compares two runs with each other, and it returns early (passes) when assets are missing. Nothing currently proves that "space unchanged" holds.

### 1.4 Where difficulty actually stands

- Issue-86 retune (Nov 3), Good proxy, three check maps, both sides, 96 rounds a tier: **Easy 91.7 %, Normal 82.3 %, Hard 56.2 %, Elite 26.0 %**. Elite's 50/192 has a Wilson 95 % interval of 20.3-32.7 %, so it sits on the band's floor.
- Those bands were hit with levers the maintainer now rejects or that changed this week:
  - pressure ×1.3 / ×1.6 (D1);
  - Elite's view ×1.5 (which feeds D1's "far human sniper");
  - the bot head ×3 and critical ×4 multipliers from Hard (removed by B1/B2).
- Hard and Elite share base skill 20 (D3), so in a fight Elite only differs from Hard by its extras. The October arena already measured Elite>Hard as a coin flip.
- The proxy is an AI unit driven by the same brain (`proxy.rs`). If the brain gets smarter, the stand-in human gets smarter too, which can hide a difficulty change.

---

## 2. Recommended architecture

**In short:** keep the PS2's shape that BOTD chose (team layer → one behaviour → navigator, mover and gunner). Finally build the missing centre: a per-bot **utility selector with explicit commitment**, over a fixed set of behaviour types. Each behaviour owns its own state and emits one intent per tick.

### 2.1 Layers and data flow (one owner per output)

```
TEAM (per team, 1 Hz, index order)      Commander: post draw (kept, PS2)
                                        Board: roles with capacities + engage slots per human (TOKN)
                                        Danger: decaying deaths per hub (flank/route cost)
        | Assignment {objective, role, role_until}
BOT, on think (+wake):  Situation::build  (perception contacts + ONE derived snapshot)
                        Selector          (utility + commitment -> ActiveBehaviour)
BOT, every tick:        ActiveBehaviour::tick -> Intent {
                            move: MoveRequest (goal | nav handle, pace, FACING MODE, stance),
                            aim:  AimRequest  (unit | point | heading, FIRE PERMISSION),
                            acts: grenade | roll | jump | ability | board }
                        Navigator (sole owner of Route; Following / Arrived / Failed(why))
                        Tactician (positions, reservations; asked by behaviours only)
                        Mover   (sole writer of move sticks, stance, roll, jump, jet; droideka/fixer = mover modes)
                        Gunner  (sole writer of look sticks, fire, weapon keys; TOKN, flinch, reaction gates here)
```

**`Situation`** is a plain `Copy` struct, built once per think from `&Sim`. Every behaviour reads it and none recomputes it. Fields:

- best target, with `engageable` (seen, within fight reach, a weapon that hurts it, sight ray clear) and `faceable`;
- threat count, recent hits and near misses, who is aiming at me;
- own health and ammo shares;
- cover state of the current spot (table lookup);
- distance to the objective, friends near, hazard.

This removes the C3 and C4 class of bug structurally: the facing rule becomes "a behaviour that is not engaging uses `Facing::Path`". The omniscient `hunt` is used only for the Hunt goal's hub, never for looks.

**`RoundRules`** is resolved once at round start: `{game: Bf1|Bf2, kind: Ground|SpaceAssault|SpaceCtf, ...}`. It replaces all four predicates. The proxy becomes a `Profile` that rules read through the same struct, so we stop scattering `proxy_human.is_none()` checks.

**`AiState`** is split by owner: `{know, assign, active: Behaviour (enum with per-variant data), resumes: Option<Suspended>, nav, mover, gun, books}`. `HoldState` disappears into the variants that need a spot.

### 2.2 Decision model: utility with commitment, not BT or GOAP

**Behaviour set (about 14, fixed):** Travel, Capture, Guard, Engage, TakeCover/Reposition, Flank, Investigate, Evade, FallBack/Heal, Overwatch, Man, Board/Ride, Serve/Repair, Follow/Hold (orders), Recover.

**Interrupt classes:**

1. **Reflex** (Evade from a grenade, Recover, off-level return). Bypasses scoring.
2. **Orders** (squad command, explicit boarding). Outranks scoring.
3. **Tactical**, everything else. Chosen by score.

A tactical behaviour has:

- `gate(&Situation) -> bool`: yes/no tests;
- at most 4 non-negative considerations, each clamped 0-1 and passed through piecewise-linear curves (no `pow`/`exp`, so cheap and deterministic);
- one weight vector from the tier profile;
- `min_ticks`;
- `exit(&Situation) -> Option<Outcome>`.

**Commitment:**

- The running behaviour keeps its score plus an inertia bonus.
- A challenger must beat it by both a relative margin (15 %, the HYST figure that measured well) and an absolute one.
- No switch happens inside `min_ticks` except by a reflex, an order, or loss of subject.
- At most one change per think.
- Interrupted Capture, Guard, Travel and Overwatch resume. Engage replaces itself, as on the PS2.

**Why this model for this game:**

- **The tier requirement is a weighting problem.** "Harder = uses cover, flanks, grenades, heals more and better" is about how often and how well. A behaviour tree or a priority list can only express that with gate chances bolted onto nodes, which is what `Tier::react`, `grenade`, `cover` and `scope_sight` already are. Utility makes each tier lever one weight or gate in one table.
- **Dithering is the known risk of utility, and it is answered.** Halo 2 rejected "score everything, bonus the last winner". Our answer is the hybrid: hard interrupt classes keep Halo 2's "only a higher class interrupts" for the safety cases, and scoring runs only at think rate (3 Hz) with a margin and a minimum time. The repo has measured evidence that this works: HYST's margin cut flips per fight-minute from 2.02 to 1.71 and unexplained standing from 9,164 to 8,389.
- **GOAP or HTN is not worth it here.** Each goal has one to three ways to achieve it, fights last 2-3 s on the PS2's own spells, and a planner would re-plan constantly. F.E.A.R.'s useful part is the goal list that falls through, which the interrupt classes already provide. A planner also makes the F9 "why" harder to read.
- **Determinism and cost are easy to keep:**
  - pure function of `&Sim` with index-order write-back (already true in `sim/step.rs` :288-305);
  - fixed behaviour order, ties broken by enum index;
  - personality from the existing per-unit hash (`draw.rs`) as ±10 % weight jitter;
  - no RNG and no hash-map iteration.
- **Cost estimate:** about 14 gates plus at most 4 considerations from a prebuilt struct is roughly 1-2 µs per bot per think. Rays stay where they are: perception, one `can_aim`, the Tactician's 1-3 confirms.
- **Readable in playtests.** The selector writes the top three scores and the switch reason into the WHY log and the F9 row. The maintainer can then see "Elite chose Flank 0.71 over Engage 0.58" rather than a link name.

---

## 3. Tiers as behaviour (replacing D1-D4)

Rules that apply at every tier:

- no damage multiplier into humans (DRETIRE and B1/B2 hold);
- no sight through walls;
- map bias never corrected;
- enemy bots stronger than the human's allies only through the DTEAM surplus (cap 3, applied at spawn);
- Elite TOKN 7.

Every number below is OURS and PROVISIONAL, with `measure:` the proxy gate.

| lever (owner) | Easy | Normal | Hard | Elite | replaces |
|---|---|---|---|---|---|
| think interval (selector) | 0.67 s | 0.5 s | 0.33 s | 0.33 s | flat 0.33 s (also saves CPU at Easy) |
| first shot after sight (gunner) | 0.4 s | 0.3 s | 0.25 s | 0.2 s | 9/9/6/5 ticks |
| far-hit search scale (C1) | ×1.5 | ×1.0 | ×0.8 | ×0.6 | C1's flat search |
| **human target weight** (situation scoring) | 1.0 | 1.0 | 1.0 | 1.0 | **D1**: pressure 0.8/1.0/1.3/1.6 deleted. A human is chosen for his threat (shoots me, aims at me, nearest engageable), never across a nearer engageable bot |
| target considerations | nearest seen | + who shoots me | + who aims at me, capturer on my post | + wounded, vehicle crew, gunner | `picks` |
| engage/hit slots per human (TOKN) | 2 | 3 | 5 | 7 | unchanged (a cap, not a magnet) |
| focus on a human (DFOCUS) | no | no | only when he is the top local threat | same, 30° spread | today's "token holder makes him its target" (part of D1) |
| view distance | ×1.0 | ×1.0 | ×1.1 | ×1.2 | ×1.0/1.0/1.25/1.5 (part of D1) |
| TakeCover weight / hide-fire cycle | 0.3, fires exposed | 0.5 (PS2 draw) | 0.8, peeks | 1.0, peeks | `Tier::cover` + Tactician |
| leave a compromised spot | never | 8 s | 3 s | at once | REACT "stay" share |
| hit reaction cooldown | 2.0 s | 1.5 s | 1.0 s | 0.7 s | flat 1.5 s (about 40 % of hits ignored) |
| evasive step when aimed at ≥0.3 s | 0 | 0.2 | 0.45 | 0.6 | not built |
| grenade use | clock window only | + cluster | + covered target, capturer | + flush a camper (same spot >8 s) | `Tier::grenade` |
| Flank weight / share | 0 | 10-20 % | 20-30 % | 25-35 %, + flank a camping human | FLNK flat share |
| FallBack/Heal under 35 % | no | no | yes | yes, + a friend covers | `mend` borrowed from the proxy |
| objective play: defend threatened post, counter-attack within 30 s, cover the capturer | no | 1 responder | 2 | 3, + two-post spread when ahead | `hunts` borrowed from the proxy; DOBJ unbuilt |
| Investigate (noise or lost target) | no | 3 s memory, no walk | 5 s, walks | 8 s, walks | FGHT Investigate, measured and not taken |
| aim (D4) | wider error floor, smoother error motion (direction eased over a think, not re-drawn); skill 14/16/18/20 kept as the last band-fitting knob only | | | | D4; D3 "Hard = Elite skill" |
| ally offset (DTEAM) | 0 | -1/surplus, floor 14 | -2, floor 11 | -1, floor 11 | kept. It lowers the **behaviour row** as well as aim |

**D3** (Coruscant CIS easy, BF1 Hoth and Docks brutal) is measured after B1/B2, per side and per game. Per-map side bias stays visible and uncorrected. The bands are judged on the two-side average.

---

## 4. Staged build plan

Each stage lands alone, deletes what it replaces, and uses only filtered tests (per the repo's rule).

### Stage 0: ground truth (no behaviour change)
- Merge the five fixes. Replace the four space and ground predicates with one `RoundRules` (this also resolves the duplicate `Sim::space_round` between C5 and B1/B2, and restores `add_hazard`'s doc).
  - **Done (7ae6e222):** the fixes are merged and every space-or-ground rule reads one test, `Sim::space_round()`; `add_hazard`'s doc is back. No `RoundRules` struct was needed for stage 0; later stages that mention it mean this test until one is built.
- Pin golden hashes for bf2_spa1 Space Assault and one space CTF map (fail when assets are missing, do not skip). Pin the three ground check maps.
  - **Pinned (2026-11-10, at 39c6f1eb):** `scripts/round_goldens.py` holds 14 goldens, each a seeded headless round whose `Sim::hash` is pinned at fixed ticks.
    - Space: bf2_spa1 Space Assault and bf2_spa1 one-flag CTF, through the game's battle builder (`game --sim-trace --mode ass|1flag --mission ... --no-human --state-hashes`), bots only, Normal, seed 42, 9000 ticks, pinned at ticks 1800, 3600, 5400 and 9000. They need a PS2-only space bake and `gen_missions.py --space --ps2-only` missions in a folder of their own: the shared bake has no space map, and adding one would add rows to the sweep.
    - Ground: bf1_yav2, tat1 and bf1_nab2, GCW, one full round per tier, seed 1, 16 a side on mission tickets, a Good proxy on team 0 (`round_to_outcome 90000 1 <map>`), pinned every 3000 ticks and at the round's end. These are the first pins with a human present at every tier.
    - Each pin also stores the inputs' `world_fingerprint.py` hash. Missing inputs, a changed fingerprint, a failed run or two runs that disagree FAIL; nothing skips. A differing hash that repeats is MOVED, naming the first tick that moved.
    - Check: `scripts/round_goldens.py` (about 25 s at 4 rounds at once). Re-bless: `scripts/round_goldens.py --bless` prints the pins file and never writes it; replacing the pins needs a card that declares the move and the maintainer's approval, and a space pin also needs the space owner's agreement. After an input change (a rebake makes every ground golden FAIL "inputs changed"), bless with a binary built from main, then check the branch against those pins; a branch binary would fold its own behaviour change into the new pins.
    - The `rounds` group in `scripts/gates.py` runs these pins. Its resume path rechecks the live inputs and source.
- Freeze the proxy's decisions on a fixed profile, so a smarter brain does not make the stand-in human smarter.
- New census lines:
  - D1 "far-human preference": the share of picks where a human is chosen while an engageable bot is at least 2× nearer;
  - per-tier behaviour-lever counts;
  - think cost per bot, p50 and p99 (`worst_think` exists).
- Re-baseline the proxy gate after B1/B2: 3 maps × 16 seeds × 2 sides per tier, with Wilson intervals.
  - **Done (2026-11-10, bot/stage0):** the proxy is frozen on literal copies of today's tier rows and thinks through `ai::legacy_think`, with the asset-free `an_all_proxy_skirmish_keeps_its_hash` as its tripwire. The census lines (`BOT_CENSUS=1`: `d1`, `levers`, `think`) are stderr only and change no hash. The re-baseline (seeds 1-32, 768 rounds) and the 34-map census are in `analysis/bot-baseline-2026-11.md`, aggregated by `scripts/bot_census.py`.
- **Accept:** golden hashes pinned; baseline table written; space hash unchanged by the merge (or the change explained to LightVelox).

### Stage 1: the skeleton (carve)

Source checkpoint verified November 11. The [design and test results](bot-carve-design.md)
document the declared droideka hash move, matching census and space pins, and
timing within budget.

- New `ai/brain/{situation, selector, behaviours/*}`. The existing chain order becomes fixed step scores, so it behaves the same.
- Navigator API becomes the only `Route` writer (34 sites go to 1 module).
- Droideka and fixer become mover modes. The pad is written only by the mover and the gunner.
- Ground bot droidekas read the behaviour label and include it in their unit hash. The ground walk owner is diagnostic.
- Space and difficulty proxies use the legacy chain. The shadow feature compares
  the ground pipeline with the legacy chain on identical ground bot inputs.
- **Accept:**
  - ground hash identical, or a declared move with every census line inside seed noise on 34 maps × 6 seeds (recycles, relocations, stand20, Recover %, B12, kills a minute);
  - space golden unchanged;
  - think time within +5 %.
- **Ground replacements:** fixed walk priorities and the post-walk label table. The mover and foot gunner apply cutter and droideka plans. Legacy and diagnostic readers retain the existing movement codes.

### Stage 2: state by owner
- `AiState` split.
- `HoldState` goes into variants.
- Reservations replace the `spot_free` O(n) loop.
- **Delete:** `strafe_until`/`strafe_side`/`dodge_until` (routeless dodge), `Route::sidestep`, `wander`, the bookkeeping `squad` field, `hub_reached` skip.
- **Accept:** as stage 1. The hash order moves once, as a lead decision under the `--sim-hash 100` invariant.

### Stage 3: utility for the combat cluster
- Engage, TakeCover, Reposition, Evade, Investigate and FallBack get their considerations.
- Targeting comes from `Situation`. Pressure is deleted (D1).
- Facing is set by the behaviour, which absorbs C3 and C4's filters and the holder's hunt look.
- **Accept:**
  - dithering metric (A→B→A inside 2 s per bot-minute) at most a fixed small value;
  - flips per fight-minute ≤ 1.71;
  - pinball (B1) and staying path (B2) no worse;
  - B12 cover share no worse;
  - far-human preference roughly equal to the same rate measured for bot targets;
  - proxy bands reported only.

### Stage 4: tier profiles
- A `TierProfile` data table (section 3).
- `picks`, `mend` and `hunts` become behaviours available to both bots and the proxy profile.
- D4 error shape.
- **Accept:**
  - proxy gate in band on both sides, with Wilson intervals: Easy ≥90 %, Normal 77-90 %, Hard 45-60 %, Elite 25-40 %;
  - arena neighbours ordered, including **elite>hard at paired t ≥ 2**;
  - lever census ordered by tier;
  - a test that no tier multiplies damage into a human.

### Stage 5: team play
- Objective play (DOBJ), the danger map feeding Flank, and the human-flank role.
- **Accept:** proxy margin moves the right way at Hard and Elite; contact, deaths a minute and the hotspot share (B5-B7) hold.

### Stage 6: mover (OBST2), can run in parallel after stage 1
- Barrier corners tested at every step, velocity sampling for soldiers and moving vehicles, the probe only on unchecked ground.
- **Accept:**
  - #108's "0 recycles and relocations in the 102-round sweep plus Elite witnesses";
  - steering under 25 % of think samples;
  - AI wall per tick at 32v32: mean ≤1 ms, p99 ≤3 ms;
  - the per-map 60 fps population table finally measured.
- **Delete:** `avoid::pick`'s heading sweep.

Ground recovery cleared the #108 sweep on November 12, 2026. All 129
Hard and 102 standard Elite rounds ended without recycles or fallback
relocation. The 32-per-side AI wall-time budget passed on all 34 ground
maps. Steering remained 29–40% of ground think instructions on four
sampled maps. Both adapters met the headless 720p budget at that
population. Live windowed capacity and the mover redesign remain open.

### Stage 7: one brain
- Remove the legacy chain for space when LightVelox signs off, or when the space goldens match.

---

## 5. Risks and open questions for the maintainer

1. **Hash changes.** Stage 1 uses the approved declared-move rule and retains
   its measured equivalence. Later stages need their own acceptance checks.
2. **Space.** Stage 1 keeps legacy space foot dispatch. Coordinate any later
   space work and new baselines with LightVelox.
3. **Proxy validity.** Stage 0 froze the proxy. A live Normal playtest comparison
   still needs evidence. The [November gate study](faction-balance-2026-11.md)
   records the current awards-off cohorts and misses.
4. **Think cadence.** Stage 1 keeps one think cadence across all tiers.
5. **Tuning burden.** Utility can grow knobs. Proposed limits: at most 4 considerations per behaviour, weights shared across tiers, and tiers change gates plus one weight each. Is that tight enough?
6. **Elite TOKN 7 plus focus** could read as ganging up (the D1 feel). Should focus at Elite need the human to be the top local threat, as proposed, or stay as it is today?
7. **Investigate** was measured and not taken in October: Polis Massa bots milled under an unreachable sniper. With exits and commitment it may now work. Retry at Hard and Elite only?
8. **Map bias** (Kamino 0/8, Mygeeto 8/8 symmetric) dominates per-map results. Use both sides and retain the four-map and BF1 cohort results. Per-map tuning remains open.
9. **Scope.** Vehicle drivers (`ride`, `flyer_drive`, `walker_drive`, `hover_drive`) stay as they are and are entered through Board/Ride. Aircraft AI (#131) is not part of this rework.
10. **Sequencing.** Every stage edits `core/src/ai/`. Under the 2-heavy-job cap, stages 1-5 are serial. Stage 6 (mover) is the only parallel track.

No files were written. Branches read: fix/c1-far-hit-reaction, fix/c3-walk-backwards, fix/c4-sniper-floor, fix/c5-sniper-lead-assist, fix/b1b2-bot-damage. Key paths:
- engine/crates/core/src/ai/behave/brain.rs
- engine/crates/core/src/ai/state.rs
- engine/crates/core/src/ai/avoid.rs
- engine/crates/core/src/ai/droideka.rs
- engine/crates/core/src/skill.rs
- engine/crates/core/src/ai/proxy.rs
- engine/crates/game/tests/suite/space_assault.rs
