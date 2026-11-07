//! Infantry AI on the authored planning graphs, driving the same soldier
//! and weapon code as the player through a synthesized pad. Design and
//! named deviations: `analysis/bot-design.md`; PS2 evidence:
//! `analysis/re-soldier-ai.md`. OURS / PROVISIONAL labels belong to the
//! individual rules and constants; implemented rules are not all fidelity.
//!
//! Ownership (`analysis/bot-design.md` 3):
//! - `behave::think` runs one priority chain and hands one aim request to
//!   `gunner_foot` and one move request to `mover`; it writes no sticks.
//! - `gunner_foot` alone writes foot look sticks and FIRE; `mover` alone
//!   writes move sticks, ROLL_STANCE and JUMP and keeps progress books.
//! - `route` owns objectives, graph legs, same-level waypoints and barrier
//!   detours; `steer` probes walls, floors, hulls and level edges; `avoid`
//!   commits to a side around obstacles. `stall` owns last-resort recycling.
//! - `tactic` chooses and reserves fighting positions, falling back to the
//!   stand/circle styles in `behave`; `arsenal` chooses weapons and grenades;
//!   `crate::perception` owns contact memory and target acquisition.
//! - `command` applies squad orders; `team` coordinates roles; `ride`,
//!   `gunner`, `hover_drive`, `walker_drive` and `man` own seated behavior.
//!   GBOARD/WGORD deliver BF1 call targets through `ride` at a scheduled think;
//!   WGPAS carries explicit unarmed-secondary admission through entry.
//! - `state` owns AI state and its hash; `draw` owns per-unit hash draws.
//!   This facade retains re-exports, shared tuning and test fixtures.
//!
//! Deterministic: sim state only, with per-unit draws keyed by unit index,
//! tick and generation; `think` reads `&Sim`. Headings are held between
//! `HEADING_PERIOD` picks. The think/heading phase is
//! the unit index, not its spawn tick. The AI spread floor (zero, no free
//! shot buffer) is in `crate::weapon`; sniper distance tuning is
//! `weapon::ai_sniper_yaw`, gated by `WeaponClass::ai_distance_table`.
//!
//! Movement contracts (`analysis/bot-design.md` 4.7, 5, 6.3, 9 / D18):
//! - Authored `BARR` corners detour a leg without changing routing tables;
//!   jetless soldiers exclude `crate::nav::Arc::JET_JUMP` arcs. Hubs and
//!   waypoints must be on the unit's level, and unwalkable arcs are rejected.
//! - The wall sweep is OURS: the PS2 uses barriers, contact push and stuck
//!   timers (`analysis/re-soldier-ai.md` 2). The pad uses the chosen WORLD
//!   direction, allowing strafing while yaw catches up. `Unit::wall_push`
//!   remains hashed/tested but does not drive blocked/side-step decisions:
//!   the A.2 experiments rejected both uses (PLAN Log "PHASE A.2 DESIGN").
//! - Every candidate needs floor within `LEDGE_DROP` of both the feet and
//!   aim point; no side-step over a drop. With no open way the bot stands
//!   for re-planning. Edge barriers stop it; mission DisableBarriers calls
//!   are replayed (`sim/bounds.rs`, card BNDS).
//! - An arrived bot releases the stick until the destination moves beyond
//!   `DEST_DEADBAND` or another request walks it; `ARRIVED_SLACK` tolerates
//!   shoves and the last step is scaled. Each walk answers `Walk`; after
//!   `GIVE_UP_AFTER` failed windows `behave::recover` gives up its goal.
//! - `route::leg_point` leads the projection on the current leg (from the
//!   last detour corner) by speed plus `LEG_LEAD` (MOVR2 / MOUTH). The PS2
//!   stuck clock, arrival radii and third-window give-up were measured and
//!   rejected; current tuning remains OURS and PROVISIONAL (D18).
//!
//! Recovery (`LostState`, `StallState`, cards BNDS / VMESH / RCYC / RLOOP):
//! off-level bots return to a reachable hub on their level, never farther
//! down, with low-wall jumps (AIRC), or recycle after `LOST_RECYCLE`.
//! A foot bot shut inside a watertight prop (`Sim::shut_in_solid`, never a
//! room/cave/tunnel) recycles after `SHUT_RECYCLE`. A bot on the level
//! wanting to walk but roaming less than `RECYCLE_ROAM` for `RECYCLE_AFTER`
//! ticks recycles with its ticket; endgame progress requires new ground.
//! Repeated failed windows at one place (`PIN_FAILS`, `PIN_ROAM`) first
//! try RETW's floor walk to the leg waypoint, else recycle. Recycling is a
//! last resort: the sweep records every use and repeated spots are defects.
//! Card UNSN (`unseen`, D23): a bot on the level that would be recycled is
//! moved to a hub past its failing link instead while no human can see it
//! or the spot; every move is counted as the same defect.
//! Card ARCM (`arcm`, D20): an arc whose walk failed twice running, or on
//! which the bot went back and forth, is closed to it for 60 s and counted
//! against its team; `route::plan_leg` draws round closed and shunned arcs
//! while another way exists. Card WHY (`why`): every failed walk and every
//! behaviour change carries a reason code (diagnostic only).
//!
//! Fighting (`behave`, `tactic`, `skill::Tier`): the Tactician's position
//! takes priority; fallback fighters stand clear of others or circle within
//! their own range. An engaged bot outside a position keeps walking its
//! route while aiming and firing. Hit reactions (REACT) are a named
//! deviation: tier chances choose staying, strafing, rolling or relocating;
//! the PS2's attacker ignores hits. `HoldState::step_to` is the goal writer,
//! with `HOLD_COMMIT`, `HOLD_WALK_DIST`, `HOLD_BLOCKED` and `HOLD_SPACING`
//! controlling steps; roll input follows `ROLL_LEAD` of travel. Tactician
//! holders instead report hits to `tactic::hit`. Ground grenades warn through
//! `Sim::hazards` (PS2 7.5 m), with tier delay/chance to flee; grenade choice
//! and throwing are implemented by `arsenal` / `gunner_foot`. In the
//! endgame (`HOLD_ENDGAME`) no Tactician position or sniper spot is taken.

mod abilities;
pub mod arcm;
mod arsenal;
mod avoid;
mod behave;
mod command;
pub mod dfocus;
mod draw;
pub mod droideka;
pub mod fix;
pub mod flyer_drive;
pub(crate) mod flyer_weapons;
pub use flyer_weapons::armed_shot;
mod gunner;
pub use gunner::flyer_on_target;
pub(crate) mod gunner_foot;
pub mod hover_drive;
mod items;
pub mod jet;
pub mod man;
mod melee;
mod mover;
pub mod proxy;
mod ride;
mod route;
pub mod snipe;
mod stall;
mod state;
mod steer;
pub mod tactic;
pub mod team;
pub(crate) mod think_memo;
pub mod tokn;
pub mod unseen;
pub mod wake;
pub mod walker_drive;
pub mod why;

pub(crate) use abilities::input as ability_input;
pub use arsenal::{droid_suited, item_suited, vehicle_bonus};
pub use avoid::ROUND_MAX;
pub use behave::think;
pub use command::CmdState;
pub use draw::{
    route_rand, stance_hold, unit_dir, unit_rand, unit_unit, SALT_BURST, SALT_CHARGE, SALT_DODGE,
    SALT_DROID, SALT_ERR_AXIS, SALT_ERR_DIR, SALT_FOLLOW, SALT_FORMATION, SALT_GRENADE, SALT_HOLD,
    SALT_NEED_GUNNER, SALT_REACT, SALT_STANCE, SALT_TAP,
};
pub use gunner_foot::yaw_toward;
pub(crate) use gunner_foot::{burst_len, heat_fits, heat_spent, pause_ticks};
pub(crate) use gunner_foot::{friend_near_blast, item_line_blocked, release_blasts_own_side};
pub use mover::SPOT_FLOOR_TOL;
pub(crate) use route::unit_goal_hub;
pub use route::{first_leg, floor_walked};
pub use state::{
    AiState, Behaviour, FightState, FlankState, HoldState, LostState, MoverState, NavTick,
    RideState, RoleState, Round, Route, RouteSource, SnipeBook, SnipeState, StallState, SteerPick,
    TposState, Walk, WalkAsk,
};
pub use steer::{rotate_xz, way_ahead, Stop, StopKind};

pub(crate) use ride::{called_claim_pass, claimed_seat, in_reach, vehicle_bail_allowed};
pub(crate) use route::{hub_on_level, mover, plan_leg, start_hub};

// ---------------------------------------------------------------------------
// Card MOVR (`analysis/bot-design.md` 4.7, 5, 6.3): the foot mover's rules.
// All four are OURS and PROVISIONAL, chosen by the sweep (the card's report
// has every variant's numbers, the PS2's own stuck clock and arrival radii
// among them; card MOVR2 measured those again on card OBST's steering, 24
// seeds x 32 maps, and 3 against 6 failed windows: `analysis/bot-design.md`
// 9, D18).
// ---------------------------------------------------------------------------
/// A request whose destination is within this of the one the mover is
/// committed to is the same request (m): the design's deadband.
pub const DEST_DEADBAND: f32 = 0.5;
/// A bot that has arrived stands until it is this far outside its arrive
/// radius (m): a push by a friend does not walk it back.
pub const ARRIVED_SLACK: f32 = 1.5;
/// The mover picks its heading (the steering's probes) on every this-many-th
/// tick, the phase by unit index, and holds it between (the PS2 steers
/// every frame near the human and every 3rd or 5th away, re-soldier-ai 1.3).
pub const HEADING_PERIOD: u64 = 3;
/// A walk that has failed this many progress windows in a row gives its
/// goal up (`behave::recover`): the post, for the commander to draw another,
/// or the boarding claim.
pub const GIVE_UP_AFTER: u8 = 6;

/// Stop advancing when an engaged enemy is closer than this. PROVISIONAL.
pub const HOLD_RANGE: f32 = 25.0;
/// Fire when yaw/pitch errors are within this (radians). PROVISIONAL.
pub const AIM_TOLERANCE: f32 = 0.035;
/// Card FFLN: a bot does not fire while a friend's capsule axis is within
/// this of its line of fire (m): the capsule's radius and half as much
/// again. PROVISIONAL (ours).
pub const LINE_CLEAR: f32 = crate::unit::SOLDIER_RADIUS * 1.5;
/// ...widening per metre along the line by the weapon's spread and this:
/// [`AIM_TOLERANCE`], the angle the aim may be off its target by when the
/// trigger goes down. PROVISIONAL (ours).
pub const LINE_SPREAD: f32 = AIM_TOLERANCE;
/// ...and the line is watched this far past the target (m), where a miss
/// flies on into the friends fighting it. PROVISIONAL (ours).
pub const LINE_PAST: f32 = 5.0;
/// Re-evaluate targets every N ticks (spreads the O(n²) scans). PROVISIONAL.
pub const THINK_PERIOD: u64 = 10;
// ---------------------------------------------------------------------------
// Phase C commit 3: the aim model (brief D9/D10). The P4 placeholder — a
// bounded random angle re-rolled every 15 ticks — is GONE; what replaces it is
// the PS2's own shape: a second-order tracker that steers an aim DIRECTION
// toward the wanted one, aiming at a tracked POSITION that carries a skill-
// dependent error radius.
// ---------------------------------------------------------------------------

/// The tracker slews (instead of accelerating) when the wanted direction is
/// more than 30° away: `cos 30°` and `sin 30°` (RE §4.2 step 3).
pub const SLEW_COS: f32 = 0.866_025_4;
/// `sin 30°`, the slew step's other half.
pub const SLEW_SIN: f32 = 0.5;
/// The tracked-position error can never exceed this many metres, whatever the
/// skill or the target's speed (RE §4.3, `FUN_002ae7b8`'s `min.s` against
/// 2.0; `analysis/re-ai-sniper.md` §1 confirms the constant).
pub const ERR_MAX: f32 = 2.0;
/// With NO target the PS2 holds the error at a constant 1.5 m floor
/// (`FUN_002ae7b8`, B2:233299 — LEAD ADDENDUM 4a): `err` persists across
/// targets and is never reset on acquisition, so a unit that spent a while
/// target-less starts its next target at >= 1.5 m.
pub const ERR_NO_TARGET: f32 = 1.5;
/// A unit's error grows by this times the CHANGE of its velocity between two
/// thinks of its block, over its own MaxSpeed (card CLSE2,
/// `analysis/re-soldier-ai.md` 18.1: a start or a stop 0.455, a reversal at
/// full speed 0.91, a straight run nothing). A BF1 round's shooter error
/// grows by it per metre its target moved, over the target's top speed (D10).
pub const ERR_GROWTH: f32 = 0.455;
/// How much the target's CURRENT speed widens the error on top of the
/// tracked growth (RE §4.3's per-tick term).
pub const ERR_SPEED_TERM: f32 = 0.6;

// ---------------------------------------------------------------------------
// Phase A navigation constants. ALL FOUR ARE PROVISIONAL (ours) and live here,
// together, so the sweep can move them in one place — PLAN Phase A design
// decision 4 ("measured by the sweep, not tuned by taste").
// ---------------------------------------------------------------------------

/// A hub counts as reached at its own radius, floored at this (m): the discs
/// author radii of 1–6 m and eleven hubs of radius 0.
pub const ARRIVE_MIN_RADIUS: f32 = 1.5;
/// Ticks in one progress window.
pub const STUCK_WINDOW: u16 = 60;
/// Metres of XZ progress toward the waypoint a unit must make in one window.
pub const STUCK_PROGRESS: f32 = 0.3;
/// Ticks of side-stepping after a window with no progress.
pub const SIDESTEP_TICKS: i8 = 30;

// ---------------------------------------------------------------------------
// Phase A.1 steering constants. ALL PROVISIONAL (ours), all in one place for
// the same reason as the four above — the sweep moves them, taste does not.
// ---------------------------------------------------------------------------

/// How far ahead the wall probe looks (m).
pub const PROBE_DIST: f32 = 1.0;
/// A detour corner counts as reached at this XZ distance (m). The PS2's mover
/// arrives at 1.0 m on the last node of a path and 4.0 m between (RE_LOG,
/// "Arrival and locate"); a corner is an intermediate node, so 1.0 m is the
/// tighter of the two and keeps a unit on the outside of the barrier.
pub const CORNER_ARRIVE: f32 = 1.0;
/// The PS2's leg rule (BF2 `FUN_002a5fa8`, `analysis/re-soldier-ai.md` 2): a
/// soldier on a leg aims at its projection on the leg's line advanced by its
/// speed (m/s read as m) plus this (m) (`route::leg_point`, card MOVR2).
pub const LEG_LEAD: f32 = 0.2;
/// PROVISIONAL (ours): a stuck unit's re-plan beside its arc's line goes to
/// the hub's centre first when it is further than this off the line (m)
/// (`route::replan`).
pub const ARC_LINE_TOL: f32 = 1.0;
/// The margin the corner walk gives a soldier: its capsule radius plus 0.5 m.
/// OURS, PROVISIONAL: the PS2 tests a leg with the unit radius alone
/// (`FUN_0026c0d0` -> `FUN_00268f68` -> `FUN_00267d18`, RE_LOG "The corner
/// search, read again") and puts its corners 0.5 m further out; the port's
/// test is 0.5 m wider than that. Measured by card OBST2 on the PS2's search
/// (8 seeds x 32 maps): with the capsule radius alone 558 of 52,079 crossing
/// legs find no way for 3,462 of 68,987 and recycles are 47 for 62, but
/// The maintainer's Kamino scene has 13 of 396 trails over 6 reversals for 3 of 376
/// (the corners then lie 1.0 m off a pod's wall) - not taken.
pub const DETOUR_RADIUS: f32 = crate::unit::SOLDIER_RADIUS + 0.5;
/// ...and a soldier's corner walk falls back to the PS2's own margin (the
/// capsule radius) when the wide one finds no way, ignores a barrier the
/// PS2's keeps (an end point within its margin), or finds one longer than the
/// PS2's by more than this (m). OURS, PROVISIONAL (issue #34): BF2
/// Coruscant's stairs to the War Room are 2.5 m between their rail barriers;
/// at 1.0 m the east flight is shut or its rail dropped, and the west one is
/// walked round the outside of its rail, onto the floor 3 m under the
/// flight's top.
pub const DETOUR_WIDE_SLACK: f32 = 2.0;
/// How far to the side an armed side-step offsets the move target (m). The
/// PS2's contact side-step is 5 m along the path perpendicular
/// (`FUN_0026aba8`); ours is triggered by the stuck window rather than by a
/// contact normal, which is the PROVISIONAL part.
pub const SIDESTEP_OFFSET: f32 = 5.0;
/// Card STFAL: a candidate step is refused when the floor one [`PROBE_DIST`]
/// ahead is more than this below both the unit's feet and the point it is
/// walking at (m). Over the 3.3 m ledge a BF2 TAT1 route walks down
/// (MEASURED: 2 m held units there for thousands of ticks), under the
/// 6.6 m MUS1 drop that strands a unit on the 59.4 level and the 10 m from
/// KAM1's decks to its rims.
pub const LEDGE_DROP: f32 = 4.0;
/// Card STFAL: the floor probe also asks this far either side of the probe
/// point (m). Half a soldier's radius: the full radius held units beside
/// holes in a map's collision (BF2 GEO1, TAT1) for thousands of ticks.
pub const LEDGE_MARGIN: f32 = 0.25;
/// Card STFAL: the wall pass's push at the probe's end counts as a wall when
/// it pushes back against the step by more than this (m).
pub const HULL_PUSH_MIN: f32 = 0.05;
/// Card OFFG: the steering's ramp test (`steer::ramp_clear`) samples the floor
/// ahead this often (m), takes a rise of more than [`RAMP_SAMPLE_RISE`]
/// between two samples for a step and not a ramp, and leaves a rise under
/// [`RAMP_MIN_RISE`] over the whole probe to the level sweep.
pub const RAMP_SAMPLE: f32 = 0.25;
pub const RAMP_SAMPLE_RISE: f32 = 0.3;
pub const RAMP_MIN_RISE: f32 = 0.1;
/// One step of the blocked-forward fan (radians): the design's 30 deg.
pub const FAN_STEP: f32 = 30.0 * crate::weapon::DEG;
/// Detour corners carried on one leg. A longer corner path is cut to this
/// many, and at the last of them the rest is searched for
/// (`route::advance_route`, card OBST2: about 2 % of the discs' crossing
/// legs need more - 1,251 of 61,849 in 64 rounds - a walk round the rails
/// of a pit or a platform).
pub const DETOUR_MAX: usize = 6;

// ---------------------------------------------------------------------------
// Card BNDS (the maintainer, 2026-10-01): the AI's bounds and the way back. The BOUNDS
// are the disc's own `BARR` barriers — the PS2's walk step never crosses one
// (RE_LOG "Barriers are the PS2's Obstacles": `FUN_00250fc8` re-aims at a
// corner per step) — taken by the steering as walls. The way BACK is ours:
// every constant here is PROVISIONAL.
// ---------------------------------------------------------------------------
/// A hub is on a unit's level, and is ARRIVED at, when its resolved floor is
/// within this of the unit's feet (m): over a hub disc on a ramp, under the
/// 8.6 m between BF2 Kamino's stacked walkways.
pub const HUB_LEVEL_TOL: f32 = 3.0;
/// A soldier whose own hub is on another storey asks this many of the nearest
/// hubs for one on its own (`route::level_hub`, card STLVL). PROVISIONAL
/// (ours): [`crate::sim::LEVEL_HUBS`], the level test's count.
pub const LEVEL_HUB_TRIES: usize = 6;
/// ...a hub whose floor is within this of the feet (m) is on their storey
/// without asking the walk. PROVISIONAL (ours): a step and a kerb.
pub const LEVEL_HUB_NEAR: f32 = 1.0;
/// ...and the floor walked from the feet reaches a hub when it arrives within
/// this of the hub's floor (m). PROVISIONAL (ours): under a soldier's height.
pub const LEVEL_HUB_REACH: f32 = 1.5;
/// The margin the steering keeps from a barrier (m): the PS2 tests its legs
/// against the rectangle grown by the unit's radius.
pub const BARRIER_MARGIN: f32 = crate::unit::SOLDIER_RADIUS;
/// A barrier is an edge of the level when the ground this far past the unit
/// along its step (m) is off the level or missing (`steer::barrier_ahead`):
/// past a rail and the first metres of a hull, onto what is clearly below.
pub const EDGE_LOOK: f32 = 6.0;
/// A lost bot refuses a step whose floor is more than this under its feet
/// (m): it never walks further down a hull.
pub const LOST_DOWN: f32 = 0.4;
/// A lost bot jumps a wall ahead when the sweep is clear this far over its
/// feet (m): under the soldier's 1.78 m jump apex.
pub const LOST_JUMP_CLEAR: f32 = 1.3;
/// ...and tries again this many ticks after a jump (1.5 s: the jump's 0.87 s,
/// the landing's soft lock and a few steps of the slide along the wall).
pub const LOST_JUMP_AGAIN: u64 = 45;
/// Card RETW: a lost bot walking its floor path back (`Sim::return_path`)
/// has reached a point of it this close in XZ (m)...
pub const RETURN_ARRIVE: f32 = 1.0;
/// ...and this close in height (m). PROVISIONAL (ours).
pub const RETURN_ARRIVE_DY: f32 = 1.0;
/// ...and looks for a walk at most this many times a spell. PROVISIONAL.
pub const RETURN_PLANS: u8 = 3;
/// A bot off the level this long with no way back is given up (ticks, 45 s):
/// it dies as lost ([`crate::sim::DeathCause::Lost`]) and respawns.
pub const LOST_RECYCLE: u64 = 45 * crate::TICK_HZ as u64;
/// Card VMESH: a bot on foot shut inside a map body
/// (`CollisionWorld::enclosing` 1 m over its feet) this long is given up as
/// lost, whatever it is doing - capturing included (ticks, 5 s). No walk
/// leaves a closed body, nobody outside sees him and he contests the post
/// from inside (BF1 Endor's cp2 boulder: 806 s on one bot). PROVISIONAL
/// (ours): the PS2 puts nobody inside a body.
pub const SHUT_RECYCLE: u64 = 5 * crate::TICK_HZ as u64;
/// The height over a soldier's feet the shut test asks at (m): the chest,
/// over the floor a closed prop has inside it.
pub const SHUT_PROBE_UP: f32 = 1.0;

// ---------------------------------------------------------------------------
// Card RCYC (the maintainer, 2026-10-01): the last-resort recycle of a bot that is on
// the level and going nowhere. The PS2 sets a stuck flag (2 m in 3 s,
// re-round-stalls.md §3); what its agents do with it is unread. This rule is
// ours and PROVISIONAL.
// ---------------------------------------------------------------------------
/// A bot that wants to walk and has made no net progress for this many such
/// ticks (120 s) is given up: it dies as stalled
/// ([`crate::sim::DeathCause::Stalled`]) and respawns.
pub const RECYCLE_AFTER: u32 = 120 * crate::TICK_HZ;
/// Net progress mid-round (m): this far from where the count started —
/// coming this much nearer a waypoint is such a move, so the one test covers
/// both. Card OBSR: 15 m, not card RCYC's 5 — a bot LAPPING a few metres of
/// ground moves 5 m every second and was never given up (AUDIT OBST: BF1
/// Tatooine's rock gap held bots for 480 - 600 s, and the count of recycles
/// could not see them). A bot that walks for two minutes and is never 15 m
/// from where it began is going nowhere. MEASURED, 32 seeds x 32 maps (main
/// e54841a + the rim rule): 206 recycles -> 284, the 78 more at the places
/// bots lapped unseen (BF1 Tatooine's gap 23 -> 44, BF1 Bespin (0, -160)
/// 0 -> 24, BF1 Geonosis (108, -12) 0 -> 7); rounds finished 1,021 of 1,024
/// both ways. At 30 m: 350 (BF2 Geonosis 16 -> 42), not taken.
pub const RECYCLE_ROAM: f32 = 15.0;
/// New ground (m): once the bot's own team is down to [`HOLD_ENDGAME`]
/// reinforcements, progress is this far beyond the ground covered since the
/// watch opened ([`StallState`]'s box). The opposing team still uses its own
/// ticket count, so mobile attackers can reach the last defenders.
/// OURS. trace: compare the original Endor finish and stuck handling with
/// one team at its last reinforcement and the other still advancing.
/// Measured, 8 seeds x 32 maps x 120,000 ticks (lead_tmp/rcyc/README.md):
/// against a point alone a bot PACING a wall is never given up — BF2 UTA1
/// seed 42 stood 1 v 9 for 30,000 ticks, its last bot walking a 14 x 9 m
/// box at 4 m/s (20 m still missed BF2 KAM1 seed 4's 32 x 19 m one); new
/// ground alone, all round, made the recycle 31 % of BF2 Kamino's deaths
/// (630 of them) — a rule of play there, not a last resort.
pub const RECYCLE_NET: f32 = 5.0;
/// Card RLOOP (ours, PROVISIONAL): a walk's failed progress windows are
/// counted at ONE place — while the bot has not been this far (m) from where
/// the first of them happened.
pub const PIN_ROAM: f32 = 2.0;
/// ...and a bot with this many is given up ([`stall::watch`]): 10 windows of
/// [`STUCK_WINDOW`] = 20 s of failing, about 22 - 26 s of standing. The
/// Recover ladder's own count starts again whenever a new plan's waypoint is
/// nearer than the last one's, so a bot handed two plans in turn against one
/// wall (BF2 Endor (98, -1, -50): `-1 -> 12` / `12 -> 11`; BF2 Kashyyyk
/// (46, 28, -109)) never reached the post's give-up and stood for the 120 s
/// of [`RECYCLE_AFTER`]. MEASURED, 8 seeds x 32 maps on main 4ca302f
/// (`lead_tmp/rloop/sw`): spells of 20 s+ inside a 2 m box in recover /
/// travel 59 (39 over 30 s, 2,835 s) -> 32 (0 over 30 s, 749 s) at 10; 5
/// over 30 s at 12 and 33 at 15. No bound is free: 27 bots in those 256
/// rounds walked away after 10 or more failures at one place (the most 27),
/// and at 10 they are given up instead.
pub const PIN_FAILS: u8 = 10;
/// ...and sent on a floor walk to its leg's waypoint ([`stall::send_off`]);
/// pinned again within this of where that walk began (m), it is recycled.
/// PROVISIONAL (ours): the walk itself may be up to 60 m long; a
/// bot that got off its pin and was dragged back near it is the same trap.
pub const PIN_WALKED: f32 = 4.0;

// ---------------------------------------------------------------------------
// Card HOLD (the maintainer, 2026-10-01): bots hold ground and fight. On the PS2 a bot
// is stationary 55 % of the time (BF1 KAS2, 5 minutes of live RAM against the
// port's 20 % — lead_tmp/kas2ai/README.md); the port marched post to post and
// stood only inside `HOLD_RANGE` of an enemy it saw. The rule is ours and
// every constant PROVISIONAL; the per-tier liveliness is `skill::Tier`'s.
// ---------------------------------------------------------------------------
/// No Tactician's position (nor a sniper's spot) is taken once either team
/// is down to this many reinforcements: the endgame is for closing in.
pub const HOLD_ENDGAME: u32 = 10;
/// A CLOSE hold — an enemy seen inside [`HOLD_RANGE`] — lasts this long past
/// the last tick he was (ticks, 2 s).
pub const HOLD_CLOSE: u64 = 2 * crate::TICK_HZ as u64;
/// A hold from which no enemy has been perceived for this long is given up
/// (ticks, 6 s): the bot holds to FIGHT, and with nobody to fight it advances.
pub const HOLD_QUIET: u64 = 6 * crate::TICK_HZ as u64;
/// With no hint, a point within this of the hub's centre (m; the hub's own
/// radius when that is smaller).
pub const HOLD_SPOT_RADIUS: f32 = 5.0;
/// A shuffle steps this far from the spot at most (m), a strafe this far
/// sideways (m), and a holder counts as standing on its point within
/// [`HOLD_ARRIVE`].
pub const HOLD_SHUFFLE: f32 = 2.5;
pub const HOLD_STRAFE: f32 = 2.5;
pub const HOLD_ARRIVE: f32 = 0.6;
/// Ticks between shuffle opportunities (3 s).
pub const HOLD_MOVE_PERIOD: u64 = 3 * crate::TICK_HZ as u64;
// Card JITR (the maintainer's snapshot, 2026-10-02: a holder ran 46 m in 10 s with 26
// reversals inside a 1.5 m box). All five are OURS and PROVISIONAL until card
// SAIR says what the PS2's soldier does on a position.
/// A holder's goal is kept this long (2 s): no shuffle and no strafe takes
/// another one before it. They draw from this one budget.
pub const HOLD_COMMIT: u64 = 2 * crate::TICK_HZ as u64;
/// A holder within this of its goal (m) WALKS there — the stick at
/// [`HOLD_WALK_STICK`] — and does not run: every shuffle and strafe, and the
/// last metres of the walk to the position.
pub const HOLD_WALK_DIST: f32 = 3.0;
pub const HOLD_WALK_STICK: f32 = 0.5;
/// A holder whose steering has been turned off the straight line to its goal
/// for this many ticks (1 s, summed over the goal's life) stands where it is:
/// a point behind a crate is not circled.
pub const HOLD_BLOCKED: u8 = crate::TICK_HZ as u8;
/// No position, shuffle or strafe within this of another holder's point or
/// of a player (m).
pub const HOLD_SPACING: f32 = 2.0;
/// A roll carries a soldier about this far (m): RollSpeedFactor 1.7 x
/// MaxSpeed 7 over 0.85 s. A hit bot only rolls with this much clear floor.
pub const ROLL_REACH: f32 = 10.0;

// ---------------------------------------------------------------------------
// Card FGHT: the PS2's FIGHT (`analysis/re-soldier-ai.md` 4.2, 4.4, 4.5; BF1's
// twin in section 5) — a seen target inside the weapon's range is fought from
// a position of the bot's own fixed style, until card TPOS's Tactician.
// ---------------------------------------------------------------------------
/// A fight is on while its seen target is inside the weapon's `MaxRange`
/// plus this (m; the combat helper's attack gives up beyond it).
pub const FIGHT_BEYOND: f32 = 10.0;
/// The circle's pause on a ring point (1.9 s; attack mode 3).
pub const CIRCLE_PAUSE: u64 = 57;
/// A circling bot on its way stops for a pause at this RATE (a second): the
/// PS2's attack mode 3 draws 1 in 12 a think (`FUN_002a3ff0`: a bound of 6
/// times `DAT_006980a0` = 2; `analysis/re-soldier-ai.md` 1.1, 4.5) and its
/// fighter 7-28 m in front of the player thinks every 0.4 s — 0.21 a second
/// (farther off, a think every 0.8-1.6 s, 0.05-0.10; inside 7 m 0.83).
pub const CIRCLE_STOP_RATE: f32 = 1.0 / (12.0 * 0.4);
/// ...as the chance per think of the port, which thinks every
/// [`THINK_PERIOD`] ticks (0.33 s): about 1 in 14 (card FGHTA; it was 1 in 6
/// a think, 0.5 a second). `FEEL_NOTES.md`.
pub const CIRCLE_STOP: f32 = CIRCLE_STOP_RATE * THINK_PERIOD as f32 / crate::TICK_HZ as f32;
/// The walk to a ring point ends this near it (m). OURS, PROVISIONAL: the
/// PS2's path ends at its last node.
pub const RING_ARRIVE: f32 = 1.5;
/// A ring point is taken in the hub nearest the drawn one, when one is within
/// this of it (m): the walk stays on the authored graph. OURS, PROVISIONAL
/// (the PS2 hands its point to the path finder): ring points drawn anywhere
/// walked bots off Bespin's platforms (42 more deaths under the map in 8
/// rounds) and into Geonosis's death regions (22 more).
pub const RING_SNAP: f32 = 10.0;
/// ...and never one more than this farther from the target than the bot
/// stands (m): a circle does not back away.
pub const RING_BACK: f32 = 2.0;
/// The share of the bots whose fixed style is "stand and shoot" (the rest
/// circle), every tier: the PS2's weights for codes 2 and 3 with under 5 m
/// of open ground round the bot, where zig-zag and flank are not drawn
/// (74-93 % of its combat rows; re-soldier-ai 4.4) — the PS2's Normal
/// and Elite 1 : 1. (The PS2's Easy 3 : 1 went with the port's old skill-8 Easy, 2026-10-28.)
pub const STAND_SHARE: f32 = 0.5;
/// A third of the standers crouch (attack mode 2: `n % 3 == 1`).
pub const STAND_CROUCH: f32 = 1.0 / 3.0;
/// OURS (the maintainer, 2026-10-05: "everyone stops in their tracks which puts them
/// all right next to each other ... spread out a little bit"): a bot that
/// stands to shoot within this of another fighter's position or of a player
/// (m) takes a free point within [`FIGHT_STEP`] instead of where it stands.
/// PROVISIONAL; `FEEL_NOTES.md`.
pub const FIGHT_SPACING: f32 = 4.0;
/// ...that far off at most (m).
pub const FIGHT_STEP: f32 = 7.0;

// ---------------------------------------------------------------------------
// Phase C D11 — fire discipline. The burst's bounds and pause are the weapon
// class's `BarrageMin` / `BarrageMax` / `BarrageDelay` (card CLSE4,
// `WeaponClass::barrage`; `analysis/re-soldier-ai.md` 20.3).
// ---------------------------------------------------------------------------

/// Ticks between two dodges of a bot walking its leg (2 s, RE §4.1).
pub const DODGE_COOLDOWN: u64 = 60;
/// Card REACT: ticks between two reactions to a hit of a bot on a position
/// (1.5 s: a burst is one draw). OURS, PROVISIONAL (the maintainer: "about 1-2 s").
pub const REACT_COOLDOWN: u64 = 45;
/// Card REACT: a hit bot's roll is pressed this many ticks after it sets
/// off sideways. A soldier standing still rolls on the spot (`soldier.rs`:
/// the roll's impulse scales a speed over 0.1 m/s) — as card HOLD's roll
/// did, pressed on the tick of the hit. OURS, PROVISIONAL.
pub const ROLL_LEAD: u64 = 6;
/// Card REACT: a roll needs floor this far past [`ROLL_REACH`] and this far
/// to each side of its line (m). OURS, PROVISIONAL.
pub const ROLL_PAST: f32 = 3.0;
pub const ROLL_LANE: f32 = 1.0;
/// Card REACT: with no ring point to take, a hit bot's new position is a
/// spot within this of where it stands (m; half of it at least). OURS,
/// PROVISIONAL.
pub const REACT_STEP: f32 = 7.0;
/// The floor of the charge-release draw `U(0.15, charge_hi)` (RE §4.4).
pub const CHARGE_MIN: f32 = 0.15;

// ---------------------------------------------------------------------------
// Card VAI: vehicles (`ride`). Which bot boards what, and when it gets out,
// is openbf's own AI (PLAN §AI); the PS2's AI boarding (B1 FUN_00244848 →
// FUN_0023c418) was located, not transcribed (re-vehicles §5). Every value
// here is PROVISIONAL and ours; the arrival radius is the PS2's (RE_LOG
// "Arrival and locate": an EntityHover's is ForwardSpeed × 0.5).
// ---------------------------------------------------------------------------
/// A bot on foot goes for a free vehicle no farther than this (XZ, m).
/// Card WAI2: the PS2's bots come 8-71 m for a walker, once 146 m
/// (re-walker-ai §13); 40 m left BF1 Geonosis's AT-TEs unclaimed.
pub const BOARD_SEEK: f32 = 75.0;
pub const VEHICLE_NOTIFY_RADIUS: f32 = 50.0;
/// ...and only when its objective is farther than this (XZ, m).
pub const BOARD_TRIP: f32 = 80.0;
/// A claim not boarded within this many ticks is given up.
/// (Card WAI2: 30 s, for 75 m walked over the graph.)
pub const BOARD_GIVEUP: u64 = 30 * crate::TICK_HZ as u64;
/// Card WAI2: a bot boards from this much past the player's reach (m) — a
/// claimer walking at an AT-AT's origin slid on its hull 8.1-9.7 m from it,
/// 0.1-1.7 m short, until its claim ran out.
pub const BOARD_SLACK: f32 = 2.0;
/// Card TBRD: the PS2's board agent asks to enter when the bot's body sphere
/// and the vehicle's are this close (m, 3-D): BF2 1.5 (`FUN_002c42d8`, `lui
/// 0x3fc0` at 0x2c4564), BF1 0.5 (`FUN_00244848`, `lui 0x3f00` at 0x244a4c;
/// analysis/re-soldier-ai.md 7.2, 7.5). The port applies it to building
/// turrets; a vehicle keeps the player's reach and [`BOARD_SLACK`].
pub const BOARD_PAD_BF2: f32 = 1.5;
pub const BOARD_PAD_BF1: f32 = 0.5;
/// Card TBRD, OURS: a bot whose body's centre is this far (m) above or below
/// a building turret's origin, inside its reach in XZ, is on another floor
/// and gives its claim up (`ride::on_another_floor`): over a soldier's
/// height, under the 2.4 m that BF1 Bespin's balcony gun turrets stand
/// over the walk below them.
pub const OTHER_FLOOR: f32 = 2.0;
/// After a dismount or a failed claim, no new claim for this many ticks.
pub const BOARD_BLOCK: u64 = 30 * crate::TICK_HZ as u64;
/// A driver whose route stuck run reaches this many ticks gets out and walks.
pub const DRIVE_STUCK_BAIL: u16 = 3 * STUCK_WINDOW;
/// The ARRIVAL radius the hover's ForwardSpeed × 0.5 gives (RE_LOG): the
/// factor.
pub const HOVER_ARRIVE_FACTOR: f32 = 0.5;
/// Card WAI: a walker whose type cannot reach a post's hub is claimed for
/// the hub nearest the post only when the ride ends this much nearer the
/// post than the vehicle stands (XZ, m).
pub const RIDE_GAIN: f32 = 40.0;
/// Card VDROP: a hover whose type cannot reach a post's hub rides to the
/// hub nearest the post, as a walker does (`route::post_hub`).
pub const HOVER_NEAREST: bool = false;
/// VBAIL Part 1, independent of the nearest-hub rule. Disable for paired
/// null controls (also skips its entropy draw and ride hash extension).
pub const VEHICLE_BAIL: bool = true;
/// CWAIT: ground boarding maintenance expires at three seconds
/// (`analysis/re-driver-wait.md` §§3,7); the original give-up clock is separate.
pub const BOARD_FRESH: u64 = 3 * crate::TICK_HZ as u64;
/// Legacy port turret-claim abandonment only; ground claims use maintenance.
pub const GUNNER_WAIT: u64 = 8 * crate::TICK_HZ as u64;

// ---------------------------------------------------------------------------
// Card TURR (`analysis/bot-design.md` 6.1, D11; the maintainer's annoyance 1): a bot
// mans a building turret only with a PURPOSE and gets out by five rules
// (`man`). The PS2's own values are marked; the rest is OURS, PROVISIONAL.
// ---------------------------------------------------------------------------
/// The offer pass runs once in this many ticks (1 s).
pub const MAN_OFFER_PERIOD: u64 = crate::TICK_HZ as u64;
/// A free turret is offered to the nearest free bot within this (m): the
/// PS2's offer scan, `DAT_00698454` = 50 m (re-soldier-ai 7.1).
pub const MAN_SEEK: f32 = 50.0;
/// The defence clause: a turret this near (XZ, m) to a post the team owns.
pub const MAN_POST_NEAR: f32 = 40.0;
/// ...with an enemy the team knows of this near the post (XZ, m).
pub const MAN_SEEN_NEAR: f32 = 60.0;
/// At most this many turrets within [`MAN_POST_NEAR`] of one post are manned
/// or claimed by a team.
pub const MAN_POST_CAP: usize = 2;
/// Exit 1: this long (ticks, 10 s - the PS2's clock, re-soldier-ai 7.3)
/// with no target the turret can hit.
pub const MAN_NO_TARGET: u64 = 10 * crate::TICK_HZ as u64;
/// Exit 2: this long seated (45 s)...
pub const MAN_LONG: u64 = 45 * crate::TICK_HZ as u64;
/// ...with no shot fired in the last this-many ticks (20 s).
pub const MAN_NO_SHOT: u64 = 20 * crate::TICK_HZ as u64;
/// A turret under this share of its health is not boarded (no longer an
/// exit: the low-health bail-out was dropped, the maintainer 2026-10-28).
pub const MAN_LOW_HEALTH: f32 = 0.3;
/// Exit 4: the endgame - a team down to [`HOLD_ENDGAME`] tickets, or the
/// gunner's own team down to this many living units.
pub const MAN_ENDGAME_ALIVE: usize = 3;
/// Exit 5: the purpose test has failed for this long (ticks, 10 s).
pub const MAN_PURPOSE_LOST: u64 = 10 * crate::TICK_HZ as u64;
/// After leaving, the turret is closed to that bot this long (20 s: the
/// PS2's unboard cooldown, re-soldier-ai 7.3).
pub const MAN_LEFT_CLOSED: u64 = 20 * crate::TICK_HZ as u64;
/// A turret the bot's walk did not reach is closed to it this long (60 s).
pub const MAN_UNREACHED_CLOSED: u64 = 60 * crate::TICK_HZ as u64;

#[cfg(test)]
mod tests {
    //! The fixtures the parts' test modules share; each part's own tests
    //! live beside its code, in that part's `mod tests`.
    use crate::nav::{self, Arc, Hub, NavGraph, RouteTable};
    use crate::sim::Sim;
    use crate::unit::UnitHandle;
    use glam::Vec3;

    /// An L of three hubs: the post sits at hub 2, which is NOT in the
    /// straight line from the unit's start — so a unit that reaches it walked
    /// the corridor.
    pub(super) fn l_graph() -> NavGraph {
        let hub = |x: f32, z: f32| Hub {
            pos: Vec3::new(x, 0.0, z),
            radius: 2.0,
            arcs: Vec::new(),
        };
        let mut hubs = vec![hub(0.0, 0.0), hub(0.0, -40.0), hub(40.0, -40.0)];
        hubs[0].arcs = vec![0];
        hubs[1].arcs = vec![0, 1];
        hubs[2].arcs = vec![1];
        let arcs = vec![
            Arc {
                a: 0,
                b: 1,
                type_mask: 1,
                flags: 0,
            },
            Arc {
                a: 1,
                b: 2,
                type_mask: 1,
                flags: 0,
            },
        ];
        // Authored table: from hub 0 to hub 2 take slot 0 (arc 0); from hub 1
        // to hub 2 take slot 1 (arc 1).
        let n = 3usize;
        let mut rows = vec![Vec::new(); crate::nav::TYPE_COUNT * n];
        let mut r0 = vec![0u8; n];
        r0[2] = 31 << 3; // slot 0
        rows[nav::SOLDIER * n] = r0;
        let mut r1 = vec![0u8; n];
        r1[2] = (31 << 3) | 1; // slot 1
        rows[nav::SOLDIER * n + 1] = r1;
        NavGraph::new(
            hubs,
            arcs,
            RouteTable::from_rows(n, crate::nav::TYPE_COUNT, rows),
            Vec::new(),
            Vec::new(),
        )
        .expect("graph")
    }

    /// Card BNDS: a deck (top y 10, x and z within ±10) with a hull falling
    /// west from its rim to y 4 at x -22, a `BARR` barrier along that rim, two
    /// hubs on the deck and a post at the first. No terrain: off the deck and
    /// the hull there is nothing. The AI unit starts at the post.
    pub(super) fn rim_sim(barrier: bool) -> (Sim, UnitHandle) {
        rim_sim_with(barrier, Vec::new())
    }

    /// [`rim_sim`] with more collision bodies (a rail on the rim).
    pub(super) fn rim_sim_with(
        barrier: bool,
        extra: Vec<crate::collision::Body>,
    ) -> (Sim, UnitHandle) {
        use crate::collision::{Body, CollisionWorld, Shape};
        use crate::cp::CpDef;
        use crate::region::Region;
        use crate::unit::{Controller, UnitSpec};
        use glam::Mat4;
        let deck = Body::new(
            Mat4::from_translation(Vec3::new(0.0, 9.75, 0.0)),
            Shape::Box {
                half: Vec3::new(10.0, 0.25, 10.0),
            },
        );
        let fall = (6.0f32 / 12.0).atan();
        let hull = Body::new(
            Mat4::from_translation(Vec3::new(-16.0, 6.75, 0.0)) * Mat4::from_rotation_z(fall),
            Shape::Box {
                half: Vec3::new(6.8, 0.25, 10.0),
            },
        );
        let mut sim = Sim::empty(7);
        let mut bodies = vec![deck, hull];
        bodies.extend(extra);
        sim.collision = CollisionWorld::new(bodies, Vec::new());
        let hub = |x: f32, arcs: Vec<u16>| Hub {
            pos: Vec3::new(x, 12.0, 0.0),
            radius: 4.0,
            arcs,
        };
        let mut g = NavGraph::new(
            vec![hub(0.0, vec![0]), hub(6.0, vec![0])],
            vec![Arc {
                a: 0,
                b: 1,
                type_mask: 1,
                flags: 0,
            }],
            RouteTable::empty(2),
            Vec::new(),
            Vec::new(),
        )
        .expect("graph");
        if barrier {
            g.barriers.push(nav::Barrier {
                centre: glam::Vec2::new(-10.0, 0.0),
                axes: [glam::Vec2::X, glam::Vec2::Y],
                half: glam::Vec2::new(0.5, 12.0),
                mask: 1,
                enabled: true,
            });
        }
        sim.nav = Some(g);
        sim.add_cp(
            CpDef {
                name: "cp1".into(),
                pos: Vec3::new(0.0, 10.0, 0.0),
                capture: Region::sphere(Vec3::new(0.0, 10.0, 0.0), 6.0),
                control: None,
                value_bleed: 100,
                value_atk: [100, 100],
                value_def: [100, 100],
                spawn_nodes: vec![],
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            Some(1),
        );
        let h = sim
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(0.0, 10.0, 0.0))
            })
            .expect("spawned");
        (sim, h)
    }

    thread_local! {
        /// A scene's bots all STAND to shoot (`Some((true, true))`: none
        /// crouched, at any range; `Some((true, false))`: by the weapon's
        /// band, as a drawn stander) or all circle (`Some((false, _))`),
        /// whatever their own draw (`None`: the draw, and the weapon's
        /// band): the tests of the gunner, the mover and the held position
        /// script a shooter that keeps its place ([`all_stand`]).
        pub(super) static STYLE: std::cell::Cell<Option<(bool, bool)>> =
            const { std::cell::Cell::new(None) };
    }

    /// Resets [`STYLE`] when the scene ends.
    pub(super) struct Style;

    impl Drop for Style {
        fn drop(&mut self) {
            STYLE.with(|s| s.set(None));
        }
    }

    /// Every bot of this thread's scenes stands to shoot (card FGHT).
    pub(super) fn all_stand() -> Style {
        STYLE.with(|s| s.set(Some((true, true))));
        Style
    }

    /// Every bot of this thread's scenes is a stander by style only: it
    /// stands where the weapon's band says, as one drawn a stander does.
    pub(super) fn all_standers() -> Style {
        STYLE.with(|s| s.set(Some((true, false))));
        Style
    }

    /// Every bot of this thread's scenes circles.
    pub(super) fn all_circle() -> Style {
        STYLE.with(|s| s.set(Some((false, false))));
        Style
    }

    /// A sim with the L graph, flat ground and one post in a capture sphere at
    /// hub 2. The AI unit starts at hub 0.
    pub(super) fn l_sim() -> (Sim, UnitHandle) {
        use crate::cp::CpDef;
        use crate::heightfield::Heightfield;
        use crate::region::Region;
        use crate::unit::{Controller, UnitSpec};
        let mut sim = Sim::empty(7);
        sim.heightfield = Some(Heightfield {
            size: 2,
            unit: 400.0,
            offset_x: 200.0,
            offset_z: 200.0,
            heights: vec![0.0; 4],
        });
        sim.nav = Some(l_graph());
        sim.add_cp(
            CpDef {
                name: "cp1".into(),
                pos: Vec3::new(40.0, 0.0, -40.0),
                capture: Region::sphere(Vec3::new(40.0, 0.0, -40.0), 6.0),
                control: None,
                value_bleed: 100,
                value_atk: [100, 100],
                value_def: [100, 100],
                spawn_nodes: vec![],
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            Some(1),
        );
        let h = sim
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .expect("spawned");
        (sim, h)
    }
}
