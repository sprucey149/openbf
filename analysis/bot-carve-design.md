# Bot rework stage 1: the carve design (2026-11-10) - [O] UNAUDITED design, nothing built

Written at main 39c6f1eb for stage 1 of `bot-rework-plan.md`. Line numbers drift; verify before building.

# Bot rework stage 1: the carve (design, 2026-11-10)

[O] UNAUDITED design. Nothing is built.

| | |
|---|---|
| Base | private main 39c6f1eb plus whatever stage 0 lands |
| Plan | `analysis/bot-rework-plan.md` §4, stage 1 |
| Issue | #108 |
| Defaults taken | Q1: a declared hash move is allowed if every census metric stays inside seed noise. Q2: space foot bots stay on the legacy chain. Q3: the proxy is frozen. Q4: one think rate. Q5-Q10 as the plan proposes. |

Stage 1 is a refactor. Every step keeps each round hash and every per-tick trajectory identical. The one exception is step S1.8: it hashes the behaviour label, but only for droidekas, the only units whose rules read it. That changes the hash format and nothing else, and only on maps with droidekas.

Space foot bots and proxies run a frozen copy of today's chain from the second step on. Ground bots run the new skeleton. A shadow check proves, on every think, that the skeleton gives the same state, pad and diagnostics as the frozen chain.

---

## 1. The tick as it is (verified at 39c6f1eb)

`ai::think` is `behave::brain::think` (brain.rs:34). It is a pure function of `(&Sim, &mut AiState)`. It returns `(PadState, NavTick)`, and `Sim::step` writes the results back in unit-index order (sim/step.rs:296-374). No RNG state is touched: every draw is the `unit_unit(index, tick, gen, salt)` hash. So identity means equal `AiState` bytes, an equal pad and an equal `NavTick` for every AI unit on every tick.

| phase | lines | what it does |
|---|---|---|
| prologue | 57-81 | ride::settle; the tier, mend station and skill; wake; `thinking` |
| THINK (on a think only) | 82-325 | perceive and pick a target; hunt; snipe and fix upkeep; the route destination (one of six arms); boarding calls and claims; the level test; the hold's upkeep; the guard post |
| mounted exit | 328-334 | ride::drive (label Ride, or Man) |
| reads | 341-376 | the target and contacts; the dispersion table; arsenal on a think |
| navigator and reactions | 378-442 | advance_route and hub_reached; hazard_flee; flinch; the hit reaction (which may re-route); the roll |
| candidates | 444-600 | the goal is built by later assignments overriding earlier ones; walk_kind |
| arrive and final goal | 601-626 | ride::maintain; arrive; goal = ret, then flee, item, holding, then the goal |
| facing and aim | 628-745 | the C3 filters; the 15-arm aim match; the IDLE EXIT at 737 |
| gunner | 748-753 | gunner_foot::run (EXIT at 750 when it returns None) |
| after the gunner | 762-883 | the FFLN step; the holding refresh; fight, free, melee, the Tactician on memory, fight_position or release, keep, excused, the holder re-read |
| mover | 884-928 | the MoveRequest (with the melee override); mover::run |
| outcome | 931-986 | the Tactician's arrival or cut-short, the ring pause, snipe arrive, fix walk_failed, items detour, the recover ladder |
| label | 987-1052 | behaviour and Why; trooper; dwell |
| after chain() | think 38-51 | fix::serve swaps the pad; droideka::steer; the unseen relocate; why::log |

## 2. Target layout for stage 1

```
ai/brain.rs              think(): picks Brain::{Skeleton, Legacy} per unit; Brain, UnitRules
ai/brain/legacy.rs       today's think()+chain()+fight_reach/guard_post/recover/hub_reached, frozen
ai/brain/shadow.rs       feature "brain-shadow": legacy vs skeleton on every think, asserts equality
ai/brain/situation.rs    Sense (think part), Frame (tick part), Cands (selection inputs)
ai/brain/selector.rs     Dest requests (think), StepKind + SCORES + select(), the late Melee rule
ai/brain/intent.rs       Intent{aim, mv, acts}, Acts, MoveKind, compose_goal(), compose_aim()
ai/brain/doing.rs        settle(): the label table; Active (per-variant view)
ai/brain/behaviours/     fight.rs (fight_position, react, FFLN step, Tactician glue: from behave/position.rs),
                         recover.rs (return walk, ladder), board.rs, overwatch.rs, serve.rs, pickup.rs,
                         travel.rs, order.rs, melee.rs
ai/route.rs (+route/)    the NAVIGATOR: NavState (private route), its write API, stuck_window moved here
ai/mover.rs              + MoverMode, finish()
ai/gunner_foot.rs        + GunMode, finish()
sim/round_rules.rs       RoundRules, Sim::round_rules(); space_round() becomes a wrapper
```

`behave.rs` and `behave/brain.rs` are deleted at the end (S1.9). `behave/position.rs` moves into `brain/behaviours/fight.rs`. `engine/CRATES.md` gets the new map.

## 3. RoundRules and UnitRules

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space { Assault, OneFlag, Landing }   // Landing: a fixture with landing regions only
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Conquest, Deathmatch, Ctf, SpaceAssault, Other }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoundRules { pub game: GroundRules, pub space: Option<Space>, pub mode: Mode }
impl RoundRules {
    pub fn ground(self) -> bool { self.space.is_none() }
    pub fn space_assault(self) -> bool { self.space == Some(Space::Assault) }
}
impl Sim { pub fn round_rules(&self) -> RoundRules /* derived from space_assault, ctf.is_space,
           landing_regions, deathmatch, ground_rules */ }
```

- **Derived on each call, never stored.** It adds no hash bytes. Fixtures that set `sim.space_assault` or `landing_regions` by hand stay in step with it. "Resolved once" means one definition. `Sim::space_round()` becomes `self.round_rules().space.is_some()` and keeps its eight non-AI callers.
- **Per unit, per tick, in the `Tick` context:**
  ```rust
  pub struct UnitRules { round: RoundRules, proxy: Option<Profile>, seated: bool, droideka: bool, brain: Brain }
  ```
  - `hit_reaction()` = `!seated && proxy.is_none() && round.ground()`. This replaces perception.rs:851, which stays as a wrapper for perception.rs:589.
  - `faces_walk()` = the rules part of walking_on (brain.rs:640-643).
  - `swept()` (gunner_foot.rs:196, 272) and `lane_look()` (snipe.rs:265, brain.rs:283) read `round.ground()`.
  - retarget.rs:41 and ride.rs:231, 528 read `round.space_assault()`.
  - The proxy reads (brain.rs:641, position.rs:217, gunner_foot.rs:194, perception.rs:358) read `rules.proxy`.
- **Out of scope:** flyer_drive's `ctf` and `landing_regions` tests (Q9: aircraft AI). Note them only.

## 4. Situation

The plan wants one `Copy` snapshot per think. That breaks identity: several inputs are per tick, and a few are deliberately read again after a mutation. Stage 1 therefore builds the Situation in three parts, each at the point where its values are read today. Stage 3 merges them, which is a behaviour change.

**Sense** (think ticks only; `None` otherwise):

| field | today |
|---|---|
| thinking, woken | brain.rs:79-81 |
| contacts held / heard / told / acquired | 96-110 |
| memory_blocked (stored, hashed) | 117-123 |
| heard_turn | 129 |
| hunt (nearest enemy, omniscient) | 137-146 |
| seen_any | 152 |
| fixing_dest | 156 |

**Frame** (every tick, before the reactions):

| field | today |
|---|---|
| me, u, eye, wc, tier, skill (the `Tick`) | 64-75, 355-362 |
| rules: UnitRules | 68, 641-643, 706 |
| mend station | 71 |
| held, seen, enemy, memory, droid, turret, seen_at | 341-367 |
| pos | 354 |
| leg_hub before advancing | 378 |
| hit_now (last_hit = tick-1) | 392-396, 410-411 |

**Cands** (after the reactions; the selector's inputs, built in today's order, mutating calls included):

| field | today |
|---|---|
| flee | 385 |
| route_leg, post, corner | 444-451 |
| push | 454 |
| base goal | 455-479 |
| boarding (mutates ride and nt.claim) | 485 |
| item (mutates items; flee filter after the call) | 486 |
| lost advance, then ret | 497-532 |
| urgent | 534 |
| ring | 537 |
| spot | 545 |
| fixing | 555 |
| mend_approach | 564-586 |
| holding | 588-590 |

**Post** (after the gunner):

| field | today |
|---|---|
| gun: GunReport | 748 |
| holding (refreshed) | 776 |
| hold | 779 |
| fight | 788-792 |
| defending | 793-794 |
| free | 800-801 |
| manning | 849-853 |
| excused | 870-874 |
| hit_recent (1 s) | 990-992 |

Reads made again after a mutation, kept on purpose and marked in code:

- `ring_walk` inside `in_range` (898), after fight_position.
- `guard_post` in the label (1005), which reads the end-of-tick `fight.mode`.
- the holder's goal (876-883).

## 5. The selector: the chain as fixed scores

The chain is not one priority list. It is four orders:

- the goal composition, where later assignments override earlier ones, gated by "route on its last leg";
- the walk owner (walk_kind plus the arrive sub-kind);
- the aim match;
- the label.

Stage 1 makes the **walk owner** a scored choice. It keeps `compose_goal` (455-626) and `compose_aim` (636-745) verbatim as functions of `Cands`. It turns the label into one table in `doing.rs`.

**5.1 Think-time destination requests.** These are applied in order and the last writer wins. They are not collapsed: `plan_leg` bumps `NavTick` counters that feed the census, and a second `retarget_board` keeps a route the first one planned.

| score | request | gate | today |
|---|---|---|---|
| 600 | Order | u.command | 159 squad::steer |
| 500 | Mend | mend station | 160-161 |
| 400 | Fix | fix::walk | 164-165 |
| 300 | Ring | ring_walk | 170 |
| 200 | Spot | snipe::walk | 172 |
| 100 | Objective | otherwise | 173 retarget(hunt) |
| then | Ordered or player board | ride.ordered_call or player_board | 183-194 (clear on the claim tick) |
| then | Claim | foot, no command, fresh claim | 197-207 |

Exactly one of the scored rows runs. The two "then" rows run after it.

**5.2 Per-tick step (the walk owner).** Pick the highest score whose gate passes. `last` = `route.is_none_or(is_last)`. Gates are verbatim, including the `urgent`, `cmd` and `mend` filters on fixing, ring and spot.

| score | StepKind | gate | MoveKind code | arrive |
|---|---|---|---|---|
| 1000 | Return | ret | RETURN 1 | none |
| 900 | Flee | flee | FLEE 2 | none |
| 800 | Pickup | item | PICKUP 6 | (item, 0.2) |
| 750 | Melee (LATE: decided after the gunner, 802-815) | free && saber && !arm.spare && pick ok && enemy on foot | LEG 5 | none; also post none, in_range false, hold none, throwing false |
| 700 | Hold | holding | HOLD 3 | (h.goal, HOLD_ARRIVE), read again at 876-883 |
| 600 | Board | boarding && last | BOARD 4 | none |
| 500 | Mend | mend_approach | MEND 7 | none (goal none once it reaches) |
| 450 | Fix | fixing | LEG 5 | last ? (fixing, HOLD_ARRIVE) : none |
| 400 | RingTake | ring && fight.mode == TAKE | LEG 5 | last ? (ring, HOLD_ARRIVE) : none |
| 390 | Ring | ring | LEG 5 | last ? (ring, RING_ARRIVE) : none |
| 300 | Spot | spot | LEG 5 | last ? (spot, RING_ARRIVE) : none |
| 200 | Push | push && compose_goal == push | LEG 5 | (push, CORNER_ARRIVE) |
| 100 | Leg | otherwise | LEG 5 | cmd: none; route: last ? (waypoint, CORNER_ARRIVE) : none; no route: (goal, CORNER_ARRIVE) |

Why the scores are total and safe:

- Ring, spot, fixing and holding all need `!urgent`, so they never meet Return, Flee, Pickup or Board.
- Spot needs ring to be none.
- Holding needs ring, fixing and mend to be none.
- Mend needs fixing and push to be none.
- When fixing and ring are both set, fixing wins both the goal and the arrive, as at 556 and 612.

The only out-of-order rule is Melee. It needs the gunner's `arm.pick` (gunner_foot.rs:443 can clear it). Like today it is checked after the gunner, and it can only replace the pre-gunner step.

The aim is built from the pre-melee step, as it is today. A melee bot's aim is the seen enemy either way, because walking_on is false with a saber.

**5.3 The two exits stay.** These are kept in compose_aim and in the skeleton's tick:

- the IDLE EXIT (737): no aim point, so steer = Idle, behaviour = None, jet.cancel;
- the gunner's None (750).

**5.4 The label (doing.rs::settle), first row that matches:**

| # | test | Behaviour | Why |
|---|---|---|---|
| 1 | ret | Recover | OffLevel |
| 2 | flee | Evade | Grenade |
| 3 | item | Pickup | Pickup |
| 4 | snipe::holds(st, holding) | Overwatch | SnipeSpot |
| 5 | holding.spot == guard_post(st at end of tick) | Guard | GuardSpot |
| 6 | holding or ring | Fight | Hit, else Seen, else Memory |
| 7 | step == Board or boarding | Board | Claim |
| 8 | cmd | Follow | Order |
| 9 | Arrived && post && defending | Guard | Arrived |
| 10 | Arrived && post | Capture | Arrived |
| 11 | hold or (fight && free) | Fight | fought |
| 12 | Walk::Idle | None | Idle |
| 13 | mv.recover > 0 | Recover | None |
| 14 | otherwise | Travel | from the previous label |

Rows 4-6 use the holding refreshed at 776. That value can be stale after the melee clear at 818. Keep it.

## 6. The Behaviour enum and per-variant data

`Behaviour` (state.rs:791) stays the 13-tag unit enum. It is serialised for the black box, the census, the probe and the sweep, so its tags and serde names do not change.

Stage 1 adds two things:

- `AiState::step: StepKind`, the walk owner. Diagnostic only, not hashed.
- `doing::Active`, the per-variant view. It is built by `settle`, logged in WHY and F9, and not stored.

```rust
pub enum Active {
    None,
    Travel(Via),              // Via = StepKind of a LEG walk: Leg | Push | Fix | Spot | Mend | Melee
    Fight(FightHow),          // Position (row 6) | InReach (row 11)
    Evade,
    Capture { cp: u32 },      // view of target_cp
    Guard(GuardHow),          // Role (row 5) | Post (row 9)
    Board { vehicle: VehicleHandle },     // view of ride.board
    Ride, Man,
    Follow,                   // view of cmd
    Recover(RecoverHow),      // OffLevel (row 1) | Ladder (row 13)
    Overwatch,
    Pickup,
}
impl Active { pub fn tag(&self) -> Behaviour }
```

Stage 2's target, where the variant owns its data and the AiState fields are deleted:

| variant | stage 2 owns |
|---|---|
| Fight | HoldState (12 fields; shuffle_at is kept only for the hash), FightState, TposState |
| Guard | RoleState's spot, look, until |
| Overwatch | SnipeState plus its hold |
| Recover | LostState and the ladder count |
| Pickup | ItemSeek |
| Board | the claim subset of RideState |
| Follow | CmdState |
| Serve (new tag) | FixClaim |

These are a declared hash move under the plan's stage 2. Adding Serve changes the census tags, so it waits for stage 3.

## 7. Intent

```rust
pub(super) struct Intent<'a> { pub aim: AimRequest<'a>, pub mv: MoveRequest<'a>, pub acts: Acts }
pub(super) struct Acts {
    pub roll: i8,            // MoveRequest.roll today (387, 418, 439)
    pub throw_stand: bool,   // MoveRequest.throwing (910-915)
    pub melee_jump: bool,    // mover.rs:439 reads st.melee.jump_tick; set at 884, after melee::goal
    pub support: Option<u32>,// gunner_foot.rs:159 ability_input (pure; same tick)
}
#[repr(u8)] pub enum MoveKind { Return = 1, Flee = 2, Hold = 3, Board = 4, Leg = 5, Pickup = 6, Mend = 7 }
```

- **Aim is built before the move.** The gunner runs between the two halves (H7 in section 12). `Intent.aim` is fixed before the gunner; `mv` and `acts` are fixed after the rules that run after the gunner.
- **The `WalkAsk` codes must survive.** `MoverState.kind` stores the code and is hashed (state.rs, tag 6). The black box also reads `st.walk.kind` (game/src/unit_probe.rs:393, blackbox_read.rs:165). So `MoveKind` keeps today's values. Only the chain that computes them goes.
- **`MoveRequest` gains `mode: MoverMode`.** `AimRequest` is unchanged; the facing mode waits for stage 3.

## 8. The navigator: sole writer of the route

```rust
// ai/route.rs
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct NavState { route: Option<Route> }            // private to ai::route
impl NavState {
    pub fn route(&self) -> Option<Route>;
    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])); // [0] | [1]+Route, same bytes, same place
}
pub(crate) fn clear(st: &mut AiState);
pub(crate) fn plan_to(sim, st, me, cur, dest, nt);                // st.route = plan_leg(..)
pub(crate) fn keep_or_plan(sim, st, me, cur, dest, nt);           // replan unless dest_hub == dest
pub(crate) fn set_waypoint(st, p: Vec3, refresh: Refresh);        // Refresh::{None, RefDist(pos)}
pub(crate) fn set_last_waypoint(st, p: Vec3, refresh: Refresh);   // only when r.is_last()
pub(crate) fn grant_flank(st, r: Route);
pub(crate) fn dodge(st, side: i8) -> bool;                        // sidestep = side*(SIDESTEP_TICKS+1)
pub(crate) fn sidestep_tick(st);                                  // countdown while holding or fleeing
pub(crate) fn progress(sim, me, st, pos, walking, osc, nt) -> bool;// stuck_window, moved from mover
pub(crate) fn mark_abandoned(st, floor: u16);                     // stuck_run = max(stuck_run, floor)
// unchanged, already inside: retarget*, retarget_board, aim_last_leg, replan, advance_route
// (advance_route returns whether the leg's hub was passed, for hub_reached)
```

`AiState.route` becomes `AiState.nav: NavState`. Its hash is eaten at the same place in `AiState::hash_into`, so the bytes are identical. The compiler then enforces the single writer.

| site | becomes |
|---|---|
| brain.rs:189 | clear |
| brain.rs:425 | dodge, with the strafe fallback kept |
| brain.rs:820 | clear |
| mover.rs:475 | sidestep_tick |
| mover.rs:593-680 (667, 678) | progress (stuck_window moves into route) |
| walker_drive.rs:310 | set_waypoint(cp.def.pos, RefDist(pos)) |
| walker_drive.rs:407 | mark_abandoned(DRIVE_STUCK_BAIL) |
| unseen.rs:157 | clear |
| team.rs:519 | grant_flank |
| command.rs:41 | clear |
| squad.rs:494, 532 | clear (obey, sim side) |
| squad.rs:790 | keep_or_plan. It keeps its own `reachable(.., m.ty)` gate, which differs from `reachable_by`. Do not merge the two. |
| squad.rs:793, 800, 805 | clear |
| squad.rs:796 | set_last_waypoint(c.goal, None) |
| seats.rs:405 | clear |
| route.rs and retarget.rs (22 sites) | already inside; they switch to NavState internals |

`plan_leg` still reads `&AiState` (flank, arcm, wander), so the navigator keeps read access to the whole state.

## 9. Mover and gunner modes; only these two modules write the pad

```rust
pub(super) enum MoverMode { Walk, Jet /* decided inside run(), as today */, Serve, Pilot, Droideka { fight: bool } }
pub(super) enum GunMode   { Aim, Serve(ServePlan), Droideka { shield: bool } }
pub(super) struct ServePlan { rx: i16, ry: i16, button: u32 }   // fix::serve's pad, as data
```

**Serve is evaluated once.** `fix::serve` runs three times per tick today: jet.rs:150, mover.rs:466 and think:38. It reads only `st.fix.cur`, `st.contacts` and `&Sim`.

- Contacts and the fix claim change only in the THINK block (perception scan, fix::upkeep) and in `fix::walk_failed` (brain.rs:972).
- `walk_failed` needs `Walk::Failed`. The stuck window cannot fail while `serve` is `Some`, because `piloting` skips the window.
- So, computed once right after the THINK block (before the mount check), the value equals all three of today's calls on every path: on foot, mounted and the early exits.

This also saves up to two `los_terrain` rays per fixer per tick.

**Finish pass.** Stage 1 keeps the edits after the label, because the droideka reads this tick's label (H10) and the mover's progress books read the stick values from before the edits. The finish runs at the end of `think()` on every path, in today's order, then the unseen relocate, then why::log.

- `mover::finish(mode, &mut pad)`:
  - Serve: lx = ly = 0; clear JUMP and ROLL_STANCE.
  - Droideka: lx = 0; clear ROLL_STANCE, JUMP, SPRINT and ACTION (the last two are never set; kept for equality); if `fight`, ly = 0.
- `gunner_foot::finish(mode, &mut pad)`:
  - Serve: rx and ry from the plan; clear FIRE, SECONDARY_FIRE, SWITCH_WEAPON and SWITCH_SECONDARY; set `plan.button`.
  - Droideka: clear SECONDARY_FIRE; if `shield`, set SECONDARY_FIRE and clear FIRE.

Here `fight` = behaviour == Fight, the form is Walk and there is a target. `shield` = `fight`, not transforming, the shield is off and its points exceed `SHIELD_RAISE` × max.

These two passes equal serve's whole-pad swap, because no AI foot writer sets any other bit and `mx`, `my`, `fire_lag` and `pick` stay at their defaults. Serve and droideka never meet, since a droideka carries no cutter; add a debug assertion.

**The rule.**

| pad | writers |
|---|---|
| foot | the mover (lx, ly, JUMP, ROLL_STANCE; jet is a mover mode) and the foot gunner (rx, ry, FIRE, SECONDARY_FIRE, SWITCH_*) |
| seat | the seat gunner (gunner.rs) and the seat drivers; unchanged (Q9) |

Delete `fix::serve`'s pad and `droideka::steer`. `droideka::takes_fight` stays, as a fight gate.

A latent bug is kept on purpose: a bot that boards with a live fix claim can have its seat pad swapped by serve for up to one think. Fixing it is a behaviour change, for stage 3.

## 10. The behaviour as hashed state

The label is read by rules only for droidekas:

- takes_fight reads the previous tick's label;
- steer and form_order (sim/droidekas.rs:208, 240) read this tick's.

So the hash is incomplete for droidekas today. Stage 1 hashes what is read, where it is read: in `Unit::hash_into`'s droideka block (unit.rs:748), `eat(&[b'B', behaviour as u8])`.

- Every round with no droideka keeps its hash: both space goldens, and most ground maps.
- Rounds with droidekas move by format only. The trajectory is identical; check it with motion.csv, the shadow check and the census, the same signature `scripts/sim_gates.sh` describes for a format change.

Hashing the label for every bot would move the space goldens too. That waits for stage 3, when the selector itself reads the running behaviour for commitment and the hash moves anyway.

`step` and `Active` stay unhashed in stage 1, because no rule reads them.

## 11. Space keeps the legacy chain; the proxy stays frozen

`ai/brain/legacy.rs` is a verbatim copy of today's `think` and `chain`, plus `fight_reach`, `guard_post`, `recover` and `hub_reached`. Its only edits are API renames: `nav.route()`, RoundRules wrappers, finish calls in place of serve and steer.

```rust
pub enum Brain { Skeleton, Legacy }
fn brain_for(r: &UnitRules) -> Brain {
    if r.proxy.is_some() { return Brain::Legacy; }       // Q3. OURS, replace: a recalibrated proxy
    if r.round.space.is_some() && !SPACE_ON_SKELETON { return Brain::Legacy; }
    Brain::Skeleton
}
/// INTERIM. replace: the space owner's sign-off, or the space goldens matching
/// under the skeleton (plan stage 7).
const SPACE_ON_SKELETON: bool = false;
```

- **Dispatch comes early (S1.2).** From then on, a slip in any later step can only move ground hashes.
- **Evidence for stage 7.** The shadow check runs both brains for every unit, so its record shows continuously whether space and proxy units would match under the skeleton.
- **Helpers are shared.** The legacy copy calls the same navigator, mover, gunner, Tactician, perception, arsenal and proxy functions. Rule for stage 3 and later: a change to a helper that legacy calls must be gated on `Brain::Skeleton`, or be declared against the space goldens and the all-proxy golden. Those goldens are the tripwires.
- **The all-proxy golden.** A round where every unit on both teams is a proxy runs only the frozen brain. If stage 0 did not pin it, S1.1 adds it.
- **Plan conflict.** Stage 4's "picks, mend, hunts become behaviours available to the proxy profile" conflicts with Q3. The proxy keeps `proxy::pick`, `mend` and `front` through legacy.

## 12. Hash identity: what makes it hard, and how each case is kept

The rule is that **phases keep today's call order, and every mutating call happens once, at its present place, even when its result is overridden.**

| # | order-dependent mutation | how identity is kept |
|---|---|---|
| H1 | react's retarget_board (position.rs:407) rewrites the route before route_leg, corner and goal are read (444-466) | the reaction phase stays before Cands |
| H2 | advance_route and hub_reached (378-383) write the route and hold before the goal | the navigator's advance runs first and reports whether a hub was reached |
| H3 | board_goal (485) mutates the ride and nt.claim every tick, even when ret overrides it | it is called once in Cands, in place |
| H4 | items::goal is called even while fleeing, and mutates items; the filter comes after | the call and then the filter, as today |
| H5 | items::goal reads lost.confirmed before the lost-path advance and the pinned clear (497-513) | Cands order: boarding, items, lost advance |
| H6 | ride::maintain (601) only on a think, with boarding, no ret, no flee | unchanged place |
| H7 | the gunner runs before melee, fight_position and FFLN; the MoveRequest uses the holding chosen before the fight (refreshed only at 776); in_range reads ring_walk after fight_position (898) | Intent is built in two halves; the read after the mutation is kept and marked |
| H8 | melee clears hold, fight, route and stance after holding is refreshed; the label and excused see the stale holding | settle reads `Post.holding`, not st.hold |
| H9 | guard_post in the label reads the end-of-tick fight.mode | settle runs after the outcome dispatch |
| H10 | takes_fight reads last tick's label; steer and form_order read this tick's | the label is settled before the finish pass |
| H11 | fix::serve is evaluated three times | evaluated once, with the proof in section 9 |
| H12 | recover can call ride::give_up and set mv.give_up inside the outcome dispatch; then mv.recover is reset on Arrived (984) | the outcome handlers run in today's match order |
| H13 | tactic::grant clears hold and fight in the write-back (step.rs:355) | unchanged, outside think |
| H14 | the IDLE and gunner-None exits skip the mover | both exits are kept, with steer, label and jet.cancel |
| H15 | the mounted path runs the whole THINK block first | the THINK block is shared; the mount check comes after it |
| H16 | after chain(): finish, then unseen relocate and reset (route None), then why::log | the same order in both brains |
| H17 | the order of nt.failed pushes (fails[4]) feeds the census | handlers keep their order |
| H18 | the think-time destination calls, where later calls see the route earlier calls planned | requests applied in sequence (5.1) |

**Instruments, in the order each step uses them:**

1. **Shadow (S1.1).** For every AI unit on every tick, run legacy and skeleton on copies of `u.ai`, inside `Sim::step` where the real think runs (after the commander, team board, tokn and unit grid). Assert equal:
   - the `AiState::hash_into` bytes;
   - the unhashed fields: behaviour, walk, why, next_think, last_think, wake;
   - the PadState;
   - the NavTick.

   It runs behind a core feature `brain-shadow`, which is off in normal builds, so it changes no hash. Scenes:
   - the `Sim::new(42)` fixture, 600 ticks;
   - the three ground check maps and bf2_spa1 plus the space CTF map, 2,000 ticks each, both teams, BF1 and BF2 rules;
   - one map with droidekas, one with fixers and turrets, one with jet troopers.
2. **Exhaustive selector table test (S1.6).** Every combination of the candidate booleans: select() and compose against a test-local copy of brain.rs:592-625 and 626.
3. **Pinned values.**
   - `sim::state::tests::a_squad_command_is_part_of_the_hash_and_no_command_costs_nothing` (the `--sim-hash 100` literal);
   - `scripts/sim_gates.sh` (walk_demo, aim_demo, `--sim-hash 600`, motion.csv with BASE, TRACE=1);
   - stage 0's goldens: three ground maps, spa1, space CTF, all-proxy.
4. **Census on 34 maps × 6 seeds.** Only if a step has to declare a behaviour move (Q1). The metrics are recycles, relocations, stand20, Recover %, B12 and kills a minute.

**Declared-move protocol.** If a step cannot be made identical:

- split out the identical part;
- land the rest as its own commit, declaring either "format" (motion.csv identical) or "behaviour" (census inside seed noise);
- re-pin the moved goldens in that commit, giving the reason.

Space and proxy cannot move, because they run legacy.

## 13. Stage acceptance

| check | requirement |
|---|---|
| ground hashes | identical, except the droideka-map format move from S1.8, declared |
| space and all-proxy goldens | unchanged |
| shadow | green on all scenes, including space and proxy units under the skeleton |
| think time | p50 and p99 per bot within +5 % of stage 0's census (expected lower: serve dedupe) |
| writers | the compiler shows one route writer module; grep shows pad writes only in mover.rs, jet.rs (a mover mode) and gunner_foot.rs on the foot path |
| issue #108 | updated |

## 14. Open for the maintainer

1. Accept the droideka-only format move in S1.8, or defer hashing the label to stage 3?
2. Proxies on legacy for good (Q3), which means stage 4 builds new tier behaviours for bots only?
3. Keep the frozen legacy copy (about 1,100 lines) until stage 7, as the space and proxy freeze?

## Carve steps

1. S1.0 Ground truth check (no code). Confirm on the base commit:
- stage 0's goldens exist and pass: the three ground check maps, bf2_spa1 Space Assault and one space CTF map, each failing (not skipping) when assets are missing;
- the proxy freeze and the think-cost census lines (p50 and p99 per bot) exist.
Record the base hashes, motion.csv (sim_gates with BASE) and the census table in the issue #108 card. Accept: everything green on the base. Tests: stage 0's golden tests by name; `scripts/sim_gates.sh` with TRACE=1.
2. S1.1 Freeze the legacy chain and add the shadow check (test only).
- Add ai/brain/legacy.rs: a verbatim copy of behave/brain.rs think, chain, fight_reach, guard_post, recover and hub_reached.
- Add ai/brain/shadow.rs behind a new core feature `brain-shadow`, with a cfg(feature) hook in sim/step.rs right after `ai::think` (step.rs:300).
- Add the all-proxy golden if stage 0 did not.
Accept: normal builds unchanged and every hash identical; shadow green (trivially, at this point). Tests:
- cargo test -p core --features brain-shadow brain::shadow (the fixture scene)
- the game suite shadow scenes, asset-gated: the three ground maps, spa1, space CTF, a droideka map, a fixer map, a jet map
- sim::state::tests::a_squad_command_is_part_of_the_hash_and_no_command_costs_nothing
3. S1.2 RoundRules, UnitRules and the per-unit brain dispatch.
- Add sim/round_rules.rs: derived Sim::round_rules(), with space_round() as a wrapper.
- Add UnitRules to the Tick. Switch the AI reads to it: brain.rs:68, 641-643, 706; gunner_foot.rs:194, 196, 272; snipe.rs:265; perception.rs:358, 851; position.rs:217; retarget.rs:41; ride.rs:231, 528.
- Make ai::think a dispatch: proxies and space foot bots go to Legacy, ground bots to Skeleton (which is still today's code).
Accept: all goldens identical; shadow green; the shadow log shows space and proxy units matching under the skeleton. Tests:
- perception::tests::a_space_round_keeps_the_memory_aim_through_geometry, perception::tests::a_space_one_flag_round_is_a_space_round
- ai::behave::brain::tests::a_space_rounds_walking_bot_still_looks_at_the_vehicle
- ai::gunner_foot filtered by lead and track; ai::snipe; ai::proxy; ai::route filtered by deathmatch and space
- game suite space_assault (filter a_space_round)
4. S1.3a Navigator API inside the module.
- Add NavState (route field, same hash bytes at the same place) and rename AiState.route to AiState.nav with a read accessor.
- Add clear, plan_to, keep_or_plan, set_waypoint, set_last_waypoint, grant_flank, dodge, sidestep_tick, progress, mark_abandoned.
- Convert the 22 sites in route.rs and route/retarget.rs. Move mover::stuck_window into route::progress.
Accept: every hash identical; shadow green. Tests:
- ai::state::tests::the_route_is_part_of_the_ai_hash
- ai::route::tests, ai::mover::tests, ai::arcm::tests, ai::stall
- the pinned --sim-hash 100 test; sim_gates.
5. S1.3b Navigator API outside the module, then make it the sole writer.
- Convert the 19 outside sites: brain.rs:189, 425, 820; mover.rs:475; walker_drive.rs:310, 407; unseen.rs:157; team.rs:519; command.rs:41; squad.rs:494, 532, 790, 793, 796, 800, 805; sim/seats.rs:405. Do the same in legacy.rs, renames only.
- Make NavState's field private, so the compiler proves one writer.
Accept: identical; shadow green; nothing outside ai::route writes .route. Tests:
- squad::tests, sim::seats::tests, ai::team::tests, ai::unseen::tests, ai::walker_drive::tests, ai::hover_drive::tests, ai::avoid (the advance_route test)
- game suite squad_orders_ps2 and squad_vehicle_maps (filtered); sim_gates.
6. S1.4 Pad finish modes.
- Compute ServePlan once, right after the THINK block. Pass it to jet::run's block, the mover's `piloting`, and the finish.
- Add MoverMode, GunMode, mover::finish and gunner_foot::finish, run at the end of think() on every path (on foot, mounted, both early exits), in both brains.
- Delete fix::serve's pad (brain.rs:38-40) and droideka::steer (brain.rs:41, droideka.rs:34-55).
Accept: identical; shadow green, pad equality included; a debug assertion that serve and droideka never meet. Tests:
- ai::fix::tests, ai::droideka::tests (the steer tests rewritten against finish), ai::gunner_foot::tests::droideka, ai::jet::tests
- sim droideka tests
- game suite droideka_preview, jet_bots, remote_droids, turret_damage (filter repair/fix)
7. S1.5 Tick context and Situation (moving data only).
- Add Sense, Frame and Post, built at today's points.
- Replace the locals in the skeleton (not in legacy) with their fields. No change of call order.
Accept: identical; shadow green; the brain tests compile against the new types. Tests:
- ai::behave::brain::tests (the C3 walk tests), ai::behave::position::tests
- perception::tests filtered by can_aim and memory
8. S1.6 Candidates, the selector and Intent.
- Add Cands, built in today's order with every mutating source called once in place: board_goal, items::goal, the lost advance.
- Add selector.rs with StepKind, the score table, the late Melee rule and the think-time Dest requests.
- Add intent.rs with Intent, Acts, MoveKind (same codes 1-7), and compose_goal and compose_aim copied verbatim.
- Replace walk_kind, the arrive match and the aim match in the skeleton. Store AiState.step as a diagnostic.
Accept: identical; shadow green; a new exhaustive table test, brain::selector::tests::scores_match_the_chain, comparing select() and compose against a test copy of brain.rs:592-626 over every candidate combination. Tests:
- that table test; ai::items::tests; the ride tests (claim_wait, pile_in, order_wait); ai::mover::tests
9. S1.7 Behaviour owners.
- Move behave/position.rs into brain/behaviours/fight.rs: fight_position, react, ring_point, stand_spot, hazard_flee, the FFLN step and the Tactician glue.
- Add behaviours recover (return walk, ladder), board, overwatch (snipe glue), serve (fix walk outcome), pickup, travel, order and melee.
- Make the outcome dispatch (brain.rs:931-982) call the owner's on_walk(), keeping the match order.
- The rules after the gunner run as a fixed pipeline in today's order, whatever the owner.
Accept: identical; shadow green. Tests:
- ai::tactic::tests, the moved position tests, ai::snipe::tests, ai::fix::tests, ai::why::tests
- the ride get_out and post_setout tests
10. S1.8 Settle the label; hash it where it is read.
- Add doing.rs with settle() (the 14-row label table), Active and tag(). Replace brain.rs:987-1027 in the skeleton.
- Eat the behaviour tag in Unit::hash_into's droideka block (unit.rs:748), in a commit of its own, declared as a FORMAT change.
Accept:
- trajectories identical (shadow green; sim_gates motion.csv byte-identical; census identical);
- goldens unchanged on rounds with no droideka: both space goldens and the --sim-hash 100 fixture if it has no droideka;
- rounds with droidekas re-pinned in that commit with the reason.
Tests:
- a new ai::state test, the_label_is_hashed_only_for_a_droideka
- ai::droideka::tests (takes_fight, form_order); the mover tests that assert labels; ai::why::tests; game suite droideka_preview
11. S1.9 Delete and tidy.
- Delete behave.rs and behave/brain.rs: the skeleton in ai/brain is the ground path, and legacy.rs stays frozen for space and proxies.
- Remove the dead WalkAsk constants (the struct stays as the black box's diagnostic; the codes live in MoveKind).
- Update the module headers, engine/CRATES.md and analysis/INDEX.md (link this design).
Accept: identical; shadow green; no dead code (clippy -D warnings on core). Tests: a compile of the core tests; the shadow scenes; sim_gates; stage 0's goldens.
12. S1.10 Stage acceptance and record.
- Run every golden. Measure think cost p50 and p99 per bot on the three check maps against stage 0's census (must be within +5 %).
- Run the 34 × 6 census only if any behaviour move was declared.
- Write the evidence to the issue #108 card.
Accept: section 13 of the design. Tests: stage 0's goldens; the shadow scenes; the census line for think cost. No full suites.

## Risks

1. HASH IDENTITY MAY FAIL ON AN ORDER-DEPENDENT MUTATION. The 18 hazards are listed in the design's section 12 (H1-H18). The shadow check finds the first unit and tick that differ. If one cannot be fixed, the declared-move protocol applies (Q1). Space and proxy units are safe in any case, because they run the frozen legacy chain from S1.2 on.

2. LEGACY DRIFT THROUGH SHARED HELPERS. legacy.rs calls the same navigator, mover, gunner, Tactician, perception, arsenal and proxy code. Any later-stage change to those helpers changes space and proxy behaviour. The tripwires are the space goldens and the all-proxy golden. The rule (gate the change on Brain::Skeleton, or declare it) has to be written into each later stage's brief.

3. THE SHADOW CHECK NEEDS A CORE FEATURE AND A HOOK IN Sim::step. It is test only, but it is code in core. Running it doubles to triples the AI time in the shadow scenes. Map scenes need local assets, so on machines without them only the fixture runs.

4. THE BEHAVIOUR ENUM IS SERIALISED. It is read by the black box (unit_probe), play_census and round_to_outcome. Adding data to its variants would break those formats and the census tags. The design therefore keeps Behaviour as tags and adds Active and StepKind beside it.

5. MoverState.kind stores the walk code and is hashed. Any renumbering of MoveKind moves every hash.

6. The S1.8 format move re-pins the goldens of maps with droidekas. If the maintainer prefers a pure stage 1, defer it to stage 3. The incompleteness this leaves already exists today.

7. Q2 and Q3 keep a frozen copy of about 1,100 lines until stage 7, and for proxies possibly for good. That is a second brain to keep compiling through API renames. Stage 4 of the plan, as written, conflicts with a frozen proxy.

8. THINK TIME. Building Situation and Cands copies a few hundred bytes per bot per tick, and the dispatch adds a branch. The serve dedupe removes up to two terrain rays per fixer per tick. Expected net: neutral or better. It is still measured against stage 0's p50 and p99 census.

9. DEPENDENCE ON STAGE 0, which is running in parallel.
- If stage 0 declares its own hash move, or pins the goldens under different names, S1.0 rebases on its values.
- If stage 0 freezes the proxy some other way (for example a pinned profile only), S1.2's legacy dispatch must agree with it.

10. Some sim-side code writes AI state outside think:
- the route: squad.rs obey, seats.rs ai_use_pass, team.rs flank_pass;
- hold and fight: tactic::grant in the write-back.
The navigator API must work on &mut AiState outside think, and grant stays where it is.

11. A preserved latent bug: a bot that boards with a live fix claim can have its seat pad swapped by the serve finish for up to one think. It cannot be fixed in stage 1 without moving the hash.

12. SCOPE AND SEQUENCE. About 18 files change over 11 steps. Under the 2-heavy-job cap the steps are serial. Each step needs:
- one core test compile with focused filters;
- one release build for sim_gates;
- the asset-gated golden and shadow runs.
Stage 6 (mover OBST2) would conflict with S1.3 and S1.4 in mover.rs, so it should start after S1.4 lands.

13. The 'fixed scores' make only the walk owner a scored choice. compose_goal and compose_aim stay verbatim, so the C3 and C4 facing filters and the hunt look survive into stage 3, as the plan intends.
