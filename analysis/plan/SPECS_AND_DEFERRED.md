<!-- Moved verbatim from PLAN.md (original lines shown in the pointer table) on 2026-10-08. PLAN.md lines 333-602: Deferred design work + DESIGN SPECS. -->

## Deferred design work
1. Infantry AI base — **RESOLVED 2026-09-01 (the maintainer): openbf's own AI, neither
   original's; Log "AI decision".**
2. Space Assault rework design.
3. CTF fun pass.
4. Weapon rebalance process (spreadsheet + playtest loop) — during P4–P5.
5. New-mode selection & design — post-1.0.
6. Campaign extension / new storyline — stretch, may drop.
7. GC credits economy tuning (informed by R1).
8. MP Galactic Conquest autosave ownership (host-only vs replicated-to-all vs hybrid).

## Historical design specifications (moved here 2026-09-08, cleanse C2)

These designs and their inline corrections are preserved as decision history. For current ownership, start at `engine/CRATES.md`: perception and tiers in `core/src/ai/perception.rs` and `core/src/skill.rs`, squad commands in `core/src/squad.rs`, vehicles in `core/src/vehicle.rs`; consult the owning cards for implementation and acceptance status. The alpha MASK classifier refinement is an individual design item below, not evidence that all four designs remain unbuilt.

**Historical preface (2026-09-08):** the following paragraph describes the original storage/build situation. Its “not history,” untracked-artifact and “only committed copy” claims are superseded in this migrated checkout; the detailed design bodies below remain verbatim.

These four designs were DECIDED and written down in the Log, which makes them
history the eye slides past. They are not history: they are the specification
the next build session works from. They live here, verbatim, for one measured
reason — `git ls-files engine/artifacts | wc -l` is **0**, so
`engine/artifacts/briefs/brief_phase_c.md` and `brief_phase_d.md`, which
HANDOFF §What next points at, are UNTRACKED. Until they are built, PLAN.md is
the only committed copy. Their Log entries are now one-line pointers back here.

- 2026-09-01 (Fable session 8): **PHASE C DESIGN — perception, the skill
  vector and the four tiers.** Anchored on `analysis/re-ai-difficulty.md`
  (lead-verified numbers). **Decided:**
  1. **Skill scale**: keep the originals' integer skill 1..20 per unit
     (`t = (s − 1)/19`) as the anchor scale, so "old Elite" is a number:
     enemy 17. Our tiers set EVERY bot's base skill: Easy 8, Normal 17,
     Hard 20, Elite 20 + the Elite-only extras below (PROVISIONAL; the
     arena tunes them). `Skill::from_level(s)` yields the vector: spread
     multiplier lerp(3, 1) (Normal ×1.32), AI distance-dispersion
     multiplier lerp(2, 0) (Normal ×0.32; the BF1 sniper distance table is
     already in `core::weapon`), aim-tracker accel/damping lerp(0.8, 14) /
     lerp(0.8, 2.4) with the 30° slew and the non-cannon halving, the
     tracked-position error radius (decay lerp(0.1, 1.1) m/s, growth
     0.455·Δpos/maxspeed, floor 1 m below skill 12 else 0, cap 2 m / 1 m,
     ×(3 − 2·visibility)) which REPLACES the placeholder `aim_wobble`,
     bursts min + U(0, max − min)·t, pause (1.5 − 0.8t) × the weapon's
     constant, charge U(0.15, t)·C, grenade chance t per 3 s window,
     dodge-on-hit lerp(0, 0.2) with a 2 s cooldown. Elite extras: error
     cap 0.5 m and a 0.15 s reaction floor (ours — the originals have no
     reaction timer). Every draw comes from the sim RNG in unit index
     order or a per-unit hash; nothing reads wall time.
  2. **Perception (new — the originals cast no LOS ray)**: per unit a
     bounded memory of ≤ 6 contacts `(handle, last_seen_tick, last_pos,
     heard)`. A target is SEEN when within view distance (the PS2 shape
     (20·radius + 30)·mult·vis, so ≈ 40–50 m for a soldier at mult 1.0;
     our tiers scale `mult` 0.8 / 1.0 / 1.25 / 1.5, PROVISIONAL) AND inside
     the cone (10 m omni, else the [D] 77.5° half-angle) AND a clear LOS
     ray against terrain and model collision (the `CollisionWorld::ray`
     from the bolts-vs-collision item — it lands before Phase C); a first
     sighting becomes a target only after `reaction_s` (0.5 / 0.3 / 0.2 /
     0.15 s by tier, PROVISIONAL); memory keeps `last_pos` for 3 s after
     LOS is lost and the unit may move toward it; HEARING: a shot within
     `hear_radius` (30 m) or being hit records the shooter's position as
     heard (no LOS). The rule that makes the top tiers fair: **a bot may
     aim or fire only at a contact its perception holds**, asserted in
     the sim (every AI `Event::Shot` must have a perception record for its
     target) and tested.
  3. **Target choice**: nearest-first over perceived contacts with the
     originals' +10 hysteresis and ≥ 1.5 s minimum retarget interval,
     plus the PRESSURE weight for human targets (layer 2 of the difficulty
     model): ×1.0 / ×1.0 / ×1.3 / ×1.6 by tier (PROVISIONAL).
  4. **Difficulty plumbing**: `Difficulty::{Easy, Normal, Hard, Elite}` on
     the session (host authority, PLAN §Saves) → `Sim.difficulty`; every
     bot receives the tier's skill at spawn (symmetric competence, all
     teams); humans take a damage multiplier 1.0 / 1.0 / 1.15 / 1.3
     (PROVISIONAL — the PS2's own level-3 lever is a > 1 damage multiplier
     for AI attackers, identity [H]); the team handicap (bleed ×1.25 and a
     reinforcement bias against the humans' team at Elite, PROVISIONAL)
     applies only when the opposing team has no human and is a host option
     defaulting ON in single-player. BF2's ±4 skill rubber band is NOT
     ported (DDA stays out of competitive play).
  5. **Hints in combat**: Defend and idle units take the nearest Cover /
     Snipe hint within 25 m and its authored `PrimaryStance` (1–3 → stand
     / crouch / prone, [H] mapping to verify at the PS2 stance machine);
     Snipe hints are used only by units whose weapon has a scope.
     **CORRECTED 2026-09-08 evening (lead-verified RE, RE_LOG 2026-09-08
     evening; `lead_tmp/s15/re/primary_stance.md`): `PrimaryStance` is a
     BITMASK — bit0 stand, bit1 crouch, bit2 prone — and the PS2 draws one
     stance uniformly among the set bits when the unit arrives; BF2 masks it
     `& 3` (its AI never prones), BF1 keeps the byte. Value 3 is
     stand-or-crouch, not prone. Phase D commit 0b BUILT the mask (landed 2026-09-08 late
     night, `6275a6d`; `commander::stance_mask`) — the 1/2/3 mapping Phase C
     commit 4 shipped is gone.**
  6. **The arena**: `game --example arena` — a seeded N-v-N bot battle on a
     flat field and on one real map per tier, printing kills/min, hit
     rate, time-to-first-kill, shots-per-kill, per tier; the same seed is
     byte-identical run to run. It is the tuning instrument and the
     acceptance referee.
  **Acceptance:** arena kill-rate and hit-rate monotonic Easy < Normal <
  Hard < Elite; Normal's numbers within a band of the PS2-anchored
  multipliers (REPORTED, and the band is set from the first measurement,
  not guessed); the perception assertion holds over every sweep round;
  the sweep count unchanged with two runs identical; a Tier-4 note for
  The maintainer with a `--difficulty` flag on the game binary and the arena table,
  so "Normal feels like old Elite" is his call to make with a pad.

  **CORRECTION 2026-09-08 (lead-verified, `analysis/re-ai-sniper.md`; Phase C
  addendum 4 in the brief carries the build detail):** point 1's tracked-position
  error is a FLOAT, not an integer — `FUN_002a2df8` has no integer conversion,
  so the floor is `max(0, lerp(1.6, 0.4))` (1.6 m at skill 1, 0.97 at 11, 0.59
  at 17, 0.4 at 20) and the cap `max(0, lerp(2.2, 1.2))`, consumer-clamped to
  [floor, 2.0], no-target floor 1.5 m. "Floor 1 m below skill 12 else 0, cap
  2 m / 1 m" above is WRONG and `re-ai-difficulty.md` §4.1 was corrected with
  it. The AI sniper distance table (a step table on SQUARED distance, added in
  the `YawSpread` slot when `YawSpread == 0 && AI && TriggerSingle &&
  ZoomMax >= 6.0`) is decoded on both games and is D9's spec.

- 2026-09-01 (Fable session 8): **PHASE D DESIGN — squad commands.** the maintainer's
  rules (decided): follow me / hold position / disperse / clear all; a
  command reaches the unassigned AI teammates within 20 m of the player
  in any direction, at most 6; one command per bot, the newest replaces
  the older; the player's death clears every command they gave, a bot's
  death clears its own. The originals' engine shape (RE_LOG "Barriers are
  the PS2's Obstacles…" §squad): BF1 broadcasts to team units within
  45 m / 30 m and builds a `UnitFollowAgent` (10 m steps toward the leader
  ±30°, released after ~20 updates beyond 20 m) or a `UnitHoldAgent`
  (wander within 10 m), with `SetAIFormationRadius` giving each follower
  a random offset in the leader's frame every 0.25 s; BF2 targets the
  reticle with a follower-slot cap and raises `CharacterIssueAICommand`.
  **Decided:**
  1. `core::squad`: `Unit.command: Option<Command { kind: Follow(leader)
     | Hold(anchor: Vec3) | Disperse(until_tick), issued_tick }>`, hashed;
     `Sim::issue_command(slot, kind)` selects the ≤ 6 nearest unassigned AI
     teammates within 20 m (XZ, any direction; index order breaks ties)
     and sets each; `clear_all(slot)` clears every bot whose command was
     issued by that slot; death clears as the maintainer specified (both in the
     kill path). Commanded bots leave the commander's pool (Phase B) until
     cleared.
  2. Behaviour: Follow routes to the leader's hub each plan period and,
     within 10 m, takes a formation offset in the leader's frame (radius
     4 m, re-rolled every 0.25 s from the sim RNG, PROVISIONAL), still
     engaging enemies it perceives and holding within HOLD_RANGE; a
     follower more than 40 m from its leader for 20 s is released. Hold
     wanders within 10 m of its anchor, takes the nearest Cover/Snipe
     hint inside that radius, engages what it perceives — and a HOLDER shoved
     more than 40 m from its anchor for 20 s is released too, on Follow's own
     rule (queue item HR, `6c5cd4a`, 2026-09-09 night; PROVISIONAL and
     unreachable in every automated referee we have — a Tier-4 hand-made
     capture would show it). Disperse hands
     the bots back to the commander with a 30 s bias: each is assigned a
     DIFFERENT objective and none the player's current one (so "disperse"
     means spread out, not merely "stop following"). Clear all restores
     normal behaviour immediately.
  3. Input: the d-pad — up Follow me, down Hold position, left Disperse,
     right Clear all (remappable, PLAN §Platform); the BF1 squad icon
     strings (`squad_cmd_cross` / `squad_cmd_highlight`) name the HUD
     marker; VO = the team's `*_com_chatter_tac_*` chatter lines already in
     the banks (follow / hold / move-out triples exist per faction).
  4. Multiplayer: commands are per slot, carried in the pad log as button
     events (the P9 flat log format takes them as pad bits), so replay and
     lockstep stay exact; a bot can be commanded by only one player at a
     time (newest wins, same rule).
  5. Tests: selection (radius, cap, unassigned-only, index-order ties),
     replacement, both death rules, the follower's release, disperse's
     distinct-objective assignment; a headless capture for the maintainer of six
     bots following the idle player and then holding.
  **Acceptance:** the sweep unchanged (commands are player-issued; the
  sweep's player is idle); `--sim-hash` with a scripted command log
  byte-identical run to run.

  **RE-POINTED 2026-09-07/09-08 (`analysis/re-sound-global.md`, the maintainer's decision
  (b)):** D12's VO must NOT look in the per-map banks. BF1's tac chatter is in
  SOUND/CW.LVL + GCW.LVL streams; BF2's command VO is `<side>_command_*` on the
  `<side>_unit_vo_quick` streams. Both need the GLOBAL/CW/GCW bake (queue item
  15), so D12's audio is silent until that lands; the rest of Phase D is not
  blocked.

- **P7 DESIGN — entity shape (Fable, decided 2026-09-01; no P7 code this
  session).** The question was whether a vehicle is a variant of `Unit` or a
  second entity kind. **Decided: a second entity kind.** `Unit` is a soldier
  by construction — `Unit.soldier` (stance machine, energy, capsule),
  `capsule()` (radius 0.5, per-stance height), `muzzle()` (75 % of the
  capsule), `Bolt::spawn` targets `(UnitHandle, Capsule)`, `ai::think`
  synthesises a soldier pad, `separate_units` pushes capsules apart,
  `update_cps` counts units as points, `Unit::hash_into` hashes soldier
  fields, and a `Unit` slot IS a team member: it carries the respawn timer,
  the spawn-wave slot and the alive/ticket bookkeeping of re-tickets-spawn
  §1.5/§2.2. A vehicle is none of those things: it is not a member (the alive
  counter counts entities "whose class is not ticket-exempt" — a vehicle is
  not in the member pool at all), it does not respawn through waves, it has
  no stance, and it can be EMPTY. Folding it into `Unit` would hash and step
  soldier state that has no meaning and would put vehicle lifetime under the
  member rules. So:
  - `Sim.vehicles: Vec<Vehicle>` with generation-checked `VehicleHandle
    { index, gen }` (same slot-reuse scheme as units, dense, index-ordered).
    `Vehicle { gen, alive, team: Option<u8> (the spawn pad's owner; overridden
    by the pilot's team while occupied), class: VehicleKind::{Hover, Walker,
    Flyer}, pos, vel, orientation (yaw/pitch/roll as three f32 — no
    quaternion, so the hash and the P9 `fmath` audit stay simple), seats:
    Vec<Seat { kind: Pilot | Gunner(turret) | Passenger, occupant:
    Option<UnitHandle> }>, turrets: Vec<Turret { yaw, pitch, weapon:
    WeaponState }>, health: Vec<RegionHealth> (one per damage region; the
    hull is region 0), respawn: Option<u64>, spawn_pad: usize }`.
  - Occupancy lives on BOTH sides: `Unit.mount: Option<Mount { vehicle:
    VehicleHandle, seat: u8 }>` and `Seat.occupant`. A mounted unit's soldier
    is frozen (pos = the seat hardpoint every tick, stance Stand, no gravity,
    no separation, not a capture point — `capture_posts` is already per unit)
    and its pad is ROUTED: the pilot's pad drives the vehicle integrator, a
    gunner's pad drives its turret, a passenger's pad does nothing. Vehicles
    have no controller of their own; an empty vehicle integrates with a zero
    pad. This is why P9(a) — N pads indexed by player slot — must land BEFORE
    P7: seat routing is slot-based from the first line.
  - Targets: `projectile::Hit::Unit(h)` becomes `Hit::Target(Target)` with
    `enum Target { Unit(UnitHandle), Vehicle(VehicleHandle, region: u8) }`;
    `Bolt::spawn` takes `(Target, Shape)` candidates where a soldier's shape is
    its capsule and a vehicle's is its OWN `coll`/`prim` hull through the same
    `core::collision::Shape` the walkable-collision work introduced (the discs
    ship them: BF1 BES1 `bes_fly_cloudcar` has a `coll` mesh plus `p_sphere_*`
    primitives). Damage → `Vehicle::take_damage(region, desc)` through the
    existing `Health`/`HealthType` table (weapon.rs; the vehicle/armour rows
    are already parsed from re-blaster-damage — verify at the RE pass).
  - AI: `ai::think(sim, me)` dispatches on `Unit.mount` — a mounted AI drives
    (placeholder: toward its target post) and never fires the soldier weapon.
    Boarding/unboarding decisions belong to the AI base decision (Deferred
    design #1), not to P7.
  - Hash: `Sim::hash` appends every vehicle after the units (index order,
    every field above bit-exact), then the bolts as today. Replay: nothing
    changes in the log format; pads reach vehicles only through units.
  - Collision: vehicles use `CollisionWorld` (hover: ray-down + a hover height
    spring; walker: a sphere/capsule on the ground query, legs are visual;
    flyer: free flight + `ray` for terrain/building impact). Soldiers are
    pushed out of vehicle hulls by the same capsule-vs-shape pass as walls.
  - Render: `game::vehicle_vis` draws the vehicle `modl` at its transform with
    turret bones from its `skel` (hardpoints `hp_*`); no skinning.
  What the RE pass (HOLD #1) must answer, in order, before P7 implementation:
  (1) the `hover`/`walker`/`flyer` class props the integrators read
  (the bake already captures every prop hash+value — `assets_baked/classes/
  bf1_imp.ron` `imp_walk_atst` shows the BODY/medium/`common.vehiclepositions.
  pilot` shape) and the three integrators' PS2 addresses (BF2GameExt's
  `FlyerAI.md`/`EntityCarrierSystem.md` are PC-keyed pointers to locate the
  PS2 analogues by string/constant matching, per analysis/community-projects.
  md); (2) seats: enter/exit (BF2 `EnterVehicle`/`ExitVehicle` bindings
  exist, BF1 has none — A3 §vehicle — so the engine-side interaction range,
  seat order and exit placement must be traced in both); (3) damage regions
  and the chunk/explosion classes (`*_chunk1..4`, `*_exp`); (4) vehicle spawn
  pads (`wrld` instances of the vehicle-spawn class: which props bind a pad
  to a post and a team, respawn timing); (5) turrets as seats on buildings
  (scope check: same seat machinery or not); (6) BF1 aim assist against
  vehicles (re-aim-assist.md covers soldiers only); (7) BF1 vs BF2 handling
  differences, since PLAN §Vehicles locks BF2 handling for ground vehicles
  and flyers faithful per map. Order of work after this session: P9(a) N
  pads → P9(b) `fmath` → shared `manifest` crate → P7 RE pass → hover first
  (BF1 TAT1 `all_hover_skiff` is on a map that already finishes a round), then
  walker (AT-ST), then flyer. **Decided:** second entity kind; occupancy on
  both sides; `Target` enum; hulls from `coll`/`prim`; P9 before P7.
  **Waits on the maintainer:** nothing blocks the RE pass; the AI base decision (item 6
  brief) decides vehicle AI, and "BF2 handling for BF1 maps' vehicles" is
  already locked in §Vehicles (re-open only if the RE shows BF1/BF2 diverge
  more than expected, as that section anticipates).

- **The alpha MASK classifier (Fable rule, 2026-09-01; Opus-safe, unbuilt).**
  Lifted from the Log entry "Two artifacts in the collision captures,
  root-caused" (`analysis/PLAN_LOG_2026.md` §2026-09-01), which keeps the
  evidence (BF1 Kamino's `kam_prop_crate`: the
  crate's 256² `kam_crate` carries a grunge/specular mask in ALPHA, 47,373 of
  65,536 texels below the 0.5 test, so `bake::detail::ALPHA_IS_MEANINGFUL`
  marks it meaningful and the alpha test deletes most of the box):
  **Fix (Opus-safe after A.1, Fable rule):** a bake-time
  classifier — alpha is a MASK only when its histogram is bimodal (a small
  fraction of texels between ~32 and ~224; bamboo, grates and fronds
  qualify; grime/specular noise like the crate does not), else the
  texture bakes opaque; report the verdict per flagged texture over the 32
  maps, view the changed goldens (KAM1 crates become solid) before
  regenerating them. PROVISIONAL until HOLD 10(g) says what the PS2 does
  with that alpha.
  (ALPHA RULE v2 landed since and routes OPAQUE / MASK / BLEND per draw; this
  classifier is the remaining refinement for flagged textures whose alpha is
  noise, and it is still unbuilt.)


## Milestones, the full text of 2026-09-25 to 2026-09-30
Moved from PLAN.md on 2026-11-02; PLAN.md has the short current form, and its scope line wins.
DONE: P0 audits, P1 bake, P2 viewer, P3 soldier, P4 combat, P5 vertical slice, P6 shell, P6.5 infantry AI A–D, P9's two
spine changes (N-pad `Sim::step`, libm-backed `core::fmath`). The loop measures every milestone below on
`engine/artifacts/loop/STATUS.md`'s scoreboard; cards (`engine/artifacts/loop/cards/`) carry a `milestone:`.

**First — M-ALPHA, the public alpha (the maintainer, 2026-09-29; defined by the planner 2026-09-30).** the maintainer shows the game in
public (a GitHub repo and a video) once it reads as a real game, not once every lighting detail matches. *Done when* a
full conquest round with drivable vehicles plays on six showcase maps and looks and sounds like a game in a 5-minute
video. The checklist: real effects (muzzle flash, bolts, impacts, explosions), the real BF2 HUD, gameplay audio,
grenades and pickups, class select on spawn, no soldier or AI out of bounds, no missing walls or sky, no droid chrome.
The showcase maps are BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1 and BF2 HOT1. They were
picked for recognisable settings, ground vehicles, a finished sweep and few open cards naming them. Hoth is the
exception on the last count, kept for the video.
- Cards carry `alpha: yes`.
- What the alpha does not need carries `defer: post-alpha` and waits, without being deleted: heroes, the real shell,
  LAN, campaign, Galactic Conquest, splitscreen, and pure lighting-parity work off the showcase maps.
- *Measured by* STATUS's M-ALPHA scoreboard and the PARITY score on the showcase maps.
- M1–M3 below continue through their alpha cards first.

**Active, interleaved by value (the maintainer, 2026-09-25):**
- **M1 Looks right** — every map retains its original character (colour, water, sky, fog, lighting, foliage and
  objects) under the modern-port presentation rules. *Done when* the original-game capture library covers every map,
  and each high-confidence finding has a fixed and rechecked comparison or a recorded explanation.
  *Measured by* open M1 cards and open findings.
- **M2 Vehicles** (old P7 + P6.5 E) — ground vehicles, then flyers, faithful per map, with vehicle AI. *Done when* every
  vehicle class each map ships spawns, is drivable/flyable by the player and by AI with its weapons, turrets and enter/exit,
  and a PCSX2 spot check per class reads right. *Measured by* vehicle classes spawned / drivable / AI-driven per map.

**Next, in order:**
- **M3 Plays right** — full Conquest rounds on all 34 ground maps end by tickets with an empty stuck census.
  *Measured by* the `finished 34/34` sweep footer, census and original-game gameplay spot checks.
- **M4 Heroes, space, awards** (old P8): Space Assault faithful first; heroes killable (BF2 conquest hero health, time/damage drain and healing
  on kills); awards/medals; class meta-unlock.
- **M5 Campaign, Galactic Conquest, splitscreen, polish, Win/Linux/macOS builds** (old P10) — the shell and menus
  (card BSHL) ship a **Multiplayer** entry in BF2's main-menu slot, greyed out and unselectable, behind an `enabled` flag in
  data (the maintainer, 2026-09-30). **Splitscreen is here, before PACK, and is not held** (the maintainer, 2026-09-30 evening; card SSCR):
  local play on one machine — one sim, N cameras, N input devices, no netcode. It starts only after the alpha cards and
  M1–M3's open cards are done; the loop works it out without asking the maintainer. How the shell reaches it is SSCR's call (the
  greyed-out Multiplayer entry is LAN's).
- **M6 PACK, only on the maintainer's go-ahead when the core game is done (card PACK):** one self-contained game file (every baked
  asset inside, no DLLs or helper files) and one Python script that takes the two ISOs, bakes, compiles and emits that
  file. Dev builds keep the assets separate. PACK does not wait for multiplayer, and its design leaves room for it: adding
  netcode later must not force a redesign of the single-file build or the bake script (the maintainer, 2026-09-30).
- **M7 Multiplayer — LAN only, the LAST milestone, HELD until the maintainer says it is ready (the maintainer, 2026-09-30; splitscreen moved
  to M5 the same evening).** 16-player LAN lockstep on the spine already built. Until then every LAN / network /
  lockstep / lobby card carries `needs: maintainer — multiplayer hold` and the `net` crate stays the stub it is. The
  determinism spine stays healthy meanwhile (replay hashes, `fmath`, the sweep's byte-compare) — no netcode is written.
- Post-1.0 (old P11): new modes, space/CTF reworks, campaign extension, GC economy tuning.

**Always on — M0 Health:** ci green; referees that test what they say; files ≤ 1,500 lines (`scripts/size_report.sh`);
disk hygiene; the auditor's error rate low.
