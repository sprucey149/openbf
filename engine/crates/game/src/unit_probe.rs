//! One unit's state as a diagnostic row: where it is, what it wants and what
//! its steering says. Read-only on the sim; nothing here is hashed.
//!
//! Two readers share it, so the field list is written once: the sweep's
//! `UNIT_SAMPLES` CSV (`game/examples/round_to_outcome.rs`, one row per alive
//! unit per second) and the playtest black box ([`crate::blackbox`], card
//! BLKB), which keeps the rows in its ring and writes them as JSON. The row
//! is `Copy` and holds no string — a class, a post, a hub and a body are
//! indices — so a ring of rows allocates nothing per sample.

use core::ai::{SteerPick, StopKind};
use core::perception::Foe;
use core::sim::Sim;
use core::unit::{Unit, UnitHandle};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Where the planning graph says `p` is: `H`ub / `A`rc corridor / `N`earest
/// (off the graph) with the hub, and the feet against the highest floor on
/// that hub's vertical line. `('-', -1, NaN)` without a graph.
pub fn locus_of(sim: &Sim, p: Vec3) -> (char, i32, f32) {
    let Some(g) = sim.nav.as_ref() else {
        return ('-', -1, f32::NAN);
    };
    let (tag, hub) = match g.locate_unit_on(p, core::nav::Mover::SOLDIER, &|_| true) {
        Some(core::nav::Locus::Hub(i)) => ('H', i as i32),
        Some(core::nav::Locus::Arc(a, t)) => {
            let arc = g.arcs[a as usize];
            ('A', if t < 0.5 { arc.a } else { arc.b } as i32)
        }
        // (The nearer end of the nearest arc.)
        Some(core::nav::Locus::Nearest(a, t)) => {
            let arc = g.arcs[a as usize];
            ('N', if t < 0.5 { arc.a } else { arc.b } as i32)
        }
        None => ('-', -1),
    };
    let dy = g.hub(hub.max(0) as u16).map_or(f32::NAN, |hb| {
        p.y - sim.ground_under(Vec3::new(hb.pos.x, 1000.0, hb.pos.z), false)
    });
    (tag, hub, dy)
}

/// The commander's objective for a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Goal {
    #[default]
    None,
    Attack,
    Defend,
    Hunt,
}

/// The steering's pick ([`SteerPick`]; a fan's signed step is `Probe::fan`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Steer {
    #[default]
    Idle,
    Fwd,
    Slide,
    Fan,
    Back,
    Ledge,
    /// Walking by a small body seen ahead (card OBST): not a blocked pick.
    Round,
}

/// What the bot is doing ([`core::ai::Behaviour`]: the link of `think`'s
/// chain that ran).
pub use core::ai::Behaviour as Doing;

/// What shuts the way the steering was asked to walk ([`StopKind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stop {
    /// The way is open, or the unit did not walk this tick.
    #[default]
    Open,
    Wall,
    Barrier,
    Hull,
    Steep,
    Drop,
}

/// The row. Short keys: the black box writes thousands of them; its file's
/// header carries the legend ([`LEGEND`]). `-1` is "none" in every index.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Probe {
    /// The unit's slot (`UnitHandle::index`) and generation.
    pub id: u32,
    pub gen: u32,
    pub team: u8,
    /// Index into the team's class table.
    pub class: u8,
    /// Driven by a pad, not the AI.
    pub human: bool,
    pub pos: [f32; 3],
    pub yaw: f32,
    /// Horizontal speed (m/s).
    pub speed: f32,
    pub grounded: bool,
    /// `core::soldier::Stance` as 0 stand, 1 crouch, 2 prone, 3 sprint,
    /// 4 jump, 5 roll, 6 fall, 7 thrown, 8 tumble, 9 get-up, 10 dead, 11 jet
    /// (the jump-jet), 12 hover.
    pub stance: u8,
    pub hp: f32,
    /// The vehicle slot and seat it rides, or -1.
    pub veh: i32,
    pub seat: u8,
    /// `H`ub / `A`rc / `N`earest / `-` ([`locus_of`]), its hub, and the feet
    /// against that hub's highest floor.
    pub locus: char,
    pub hub: i32,
    pub floor_dy: f32,
    /// The playable level: 0 on it, a drop in metres below it, -1 unknown.
    pub level: f32,
    /// Metres to the first soldier-layer face straight up, -1 none in 60 m.
    pub overhead: f32,
    /// The commander's objective and its post.
    pub goal: Goal,
    pub goal_cp: i32,
    /// The post the brain is walking at (`AiState::target_cp`).
    pub cp: i32,
    /// Under a squad order.
    pub ordered: bool,
    /// The leg: has one, the hub it came from, walks to and ends at, its arc,
    /// `t`able / `b`fs / `s`traight, the aim point and metres to it (XZ).
    pub leg: bool,
    pub from: i32,
    pub next: i32,
    pub dest: i32,
    pub arc: i32,
    pub src: char,
    pub wp: [f32; 3],
    pub wp_d: f32,
    /// The steering: its pick, a fan's signed step, the remembered slide
    /// side, the side-step state.
    pub steer: Steer,
    pub fan: i8,
    pub slide: i8,
    pub sidestep: i8,
    /// Card OBST2 (schema 4): the behaviour that ran, and the walk's failed
    /// progress windows since it last made progress (`MoverState::recover`).
    #[serde(default)]
    pub doing: Doing,
    /// The owner of the ground bot's walk request, when one was selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk_owner: Option<core::ai::StepKind>,
    #[serde(default)]
    pub recover: u8,
    /// The going-round commitment (`core::ai::Round`): live (a side chosen at
    /// a contact — the side is `slide`), the nearest the walk has been to its
    /// aim point since (XZ m), ticks of the progress window's allowance
    /// spent (of `core::ai::ROUND_MAX`), and whether what shut the way last
    /// was a rim barrier (no allowance).
    #[serde(default)]
    pub round: bool,
    #[serde(default)]
    pub round_near: f32,
    #[serde(default)]
    pub round_t: u16,
    #[serde(default)]
    pub round_rim: bool,
    /// What the steering walked at this tick (`core::ai::WalkAsk`, card
    /// BLKF): 0 it did not walk, 1 a lost bot's return point, 2 out of a
    /// hazard, 3 a held position's goal, 4 the vehicle it boards, 5 the leg
    /// (walk point, corner, an order's point, the post), 6 a pickup detour
    /// or its return, 7 a health droid. The direction (XZ) includes the
    /// steering's side-step offset.
    #[serde(default)]
    pub walk: u8,
    #[serde(default)]
    pub walk_at: [f32; 3],
    #[serde(default)]
    pub walk_dir: [f32; 2],
    /// What shuts the way along `walk_dir` now (asked by the probe with the
    /// steering's own direction and floor, not carried by the brain), metres
    /// to it, its normal (XZ) and the map body the sweep met (an index into
    /// `CollisionWorld::bodies`). Open with no body when `walk` is 0. (A
    /// schema 1 file asked toward `wp` instead, walking or not.)
    pub stop: Stop,
    pub stop_d: f32,
    pub stop_n: [f32; 2],
    pub body: i32,
    /// 2 holding a position, 1 owing hubs after one, 0 neither.
    pub hold: u8,
    /// A hold's goal (where it stands or steps to now), its spot, seconds of
    /// dwell left, the side its last strafe went, seconds its goal is still
    /// committed, ticks the steering was turned off the line to the goal.
    #[serde(default)]
    pub hold_goal: [f32; 3],
    #[serde(default)]
    pub hold_spot: [f32; 3],
    #[serde(default)]
    pub hold_left: f32,
    #[serde(default)]
    pub hold_side: i8,
    #[serde(default)]
    pub hold_commit: f32,
    #[serde(default)]
    pub hold_blocked: u8,
    /// Card TPOS: the fight's mode (`FightState`: 0 none, 1 stand, 2 walk,
    /// 3 pause, 4 move, 5 take = walking to a Tactician's position, 6 cover
    /// = at it), the position held (a table index) and its point, bits (1
    /// covered from the target when chosen, 2 firing, 4 lean -1, 8 lean +1),
    /// hits taken at it and seconds to its phase's end (at it: the hide /
    /// fire swap; walking there: the walk's deadline).
    #[serde(default)]
    pub fmode: u8,
    #[serde(default)]
    pub tpos: Option<u32>,
    #[serde(default)]
    pub tpos_at: [f32; 3],
    #[serde(default)]
    pub tpos_bits: u8,
    #[serde(default)]
    pub tpos_hits: u8,
    #[serde(default)]
    pub tpos_phase: f32,
    /// The enemy it has (a unit slot) and the range to it.
    pub target: i32,
    pub target_d: f32,
    /// The remote droid it has as its enemy instead (its id).
    #[serde(default)]
    pub target_droid: Option<u64>,
    /// Ticks into the progress window, ticks in windows without progress,
    /// ticks the no-net-progress watch has run (card RCYC).
    pub stuck_t: u16,
    pub stuck: u16,
    pub stall: u32,
    /// Off the level: 0 no, 1 suspected, 2 confirmed; and its jump tries.
    pub lost: u8,
    pub lost_tries: u8,
    /// The vehicle slot it is walking to board, or -1.
    pub board: i32,
    /// That vehicle's position.
    #[serde(default)]
    pub board_at: [f32; 3],
    pub skill: u8,
    pub kills: u32,
    pub deaths: u32,
}

/// What each key of a [`Probe`] means, for the black box's header.
pub const LEGEND: &[(&str, &str)] = &[
    ("id/gen", "unit slot and generation"),
    ("class", "index into names.classes[team]"),
    ("stance", "0 stand 1 crouch 2 prone 3 sprint 4 jump 5 roll 6 fall 7 thrown 8 tumble 9 getup 10 dead 11 jet 12 hover"),
    ("veh/seat", "vehicle slot ridden (-1 on foot) and seat"),
    ("locus/hub/floor_dy", "H hub, A arc, N nearest (off the graph), - none; that hub; feet minus the hub's top floor"),
    ("level", "0 on the playable level, >0 metres below it, -1 unknown"),
    ("overhead", "metres to a face straight up, -1 none"),
    ("goal/goal_cp", "the commander's objective and post index"),
    ("cp", "the post the brain walks at"),
    ("from/next/dest/arc/src", "the route leg: hubs, arc (-1 last leg or straight), t table b bfs s straight f flank r round a shunned arc (card ARCM)"),
    ("wp/wp_d", "the leg's aim point and XZ metres to it"),
    ("steer/fan/slide/sidestep", "the steering's pick, fan step x30 deg, slide side, side-step state"),
    ("walk_owner", "ground bot walk owner selected for the tick"),
    ("doing/recover", "the behaviour that ran this tick (none travel fight evade capture guard board ride follow recover) and the walk's failed progress windows since it last made progress (schema 4)"),
    ("round/round_near/round_t/round_rim", "going round an obstacle on one committed side (the side is slide): live, the nearest the walk has been to its aim point since the obstacle was met (XZ m), ticks of the progress window's allowance spent (300 = spent: the walk's windows fail again), and the last contact was a rim barrier (no allowance) (schema 4)"),
    ("walk/walk_at/walk_dir", "what the steering walked at this tick: 0 it did not walk, 1 a lost bot's return point, 2 out of a hazard, 3 the hold goal, 4 the vehicle it boards, 5 the leg (walk point / corner / order point / post), 6 a pickup detour or its return, 7 a health droid; the point; the XZ direction the steering was asked (side-step offset included)"),
    ("stop/stop_d/stop_n/body", "what shuts the way along walk_dir (the steering's own direction and floor): wall (a body, see names.bodies), barrier, hull, steep, drop; open = nothing, or the bot did not walk (walk 0). Schema 1 asked toward wp, walking or not"),
    ("hold", "2 holding a position, 1 owing hubs, 0 no"),
    ("hold_goal/hold_spot/hold_left/hold_side/hold_commit/hold_blocked", "a hold (hold 2): where it stands or steps to now, the position taken, seconds of dwell left, the side the last strafe went (0 none yet), seconds the goal is still committed, ticks the steering was turned off the line to the goal"),
    ("fmode/tpos/tpos_at/tpos_bits/tpos_hits/tpos_phase", "card TPOS: the fight's mode (0 none 1 stand 2 walk 3 pause 4 move 5 take = walking to a Tactician's position 6 cover = at it); the position held (table index, null none) and its point; bits 1 covered from the target, 2 firing, 4 lean -1, 8 lean +1; hits taken at it; seconds to the phase's end (the hide / fire swap, or the walk's deadline)"),
    ("target/target_d/target_droid", "the enemy unit slot (or remote droid id) and range"),
    ("stuck_t/stuck/stall", "ticks (30/s): progress window, no-progress run, no-net-progress watch"),
    ("lost/lost_tries", "off the level: 1 suspected 2 confirmed; jump tries"),
    ("board/board_at", "vehicle slot it walks to board and where that vehicle is"),
];

/// A stance as the rows keep it ([`Probe::stance`]).
pub fn stance_code(s: core::soldier::Stance) -> u8 {
    use core::soldier::Stance;
    match s {
        Stance::Stand => 0,
        Stance::Crouch => 1,
        Stance::Prone => 2,
        Stance::Sprint => 3,
        Stance::Jump => 4,
        Stance::Roll => 5,
        Stance::Fall => 6,
        Stance::Thrown => 7,
        Stance::Tumble => 8,
        Stance::GetUp => 9,
        Stance::Dead => 10,
        Stance::Jet => 11,
        Stance::Hover => 12,
    }
}

/// A behaviour as the rows keep it ([`Probe::doing`]).
pub fn doing_of(b: core::ai::Behaviour) -> Doing {
    b
}

impl Probe {
    /// The row for unit `h` at `tick`. `deep` adds what costs a query per
    /// unit beyond the sample's own: the steering's stop test (the black box
    /// asks; the sweep's CSV does not carry it).
    pub fn of(sim: &Sim, h: UnitHandle, u: &Unit, tick: u64, deep: bool) -> Probe {
        let p = u.soldier.pos;
        let v = u.soldier.vel;
        let (locus, hub, floor_dy) = locus_of(sim, p);
        let (goal, goal_cp) = match sim.commander[usize::from(u.team).min(1)].of(h.index) {
            Some(core::commander::Objective::Attack(c)) => (Goal::Attack, c as i32),
            Some(core::commander::Objective::Defend(c)) => (Goal::Defend, c as i32),
            Some(core::commander::Objective::Hunt) => (Goal::Hunt, -1),
            None => (Goal::None, -1),
        };
        // What is over the head: the first soldier-layer face straight up.
        let overhead = sim
            .collision
            .ray(
                p + Vec3::Y * 1.9,
                Vec3::Y,
                60.0,
                core::collision::Layers::SOLDIER,
            )
            .map_or(-1.0, |(t, _)| t + 1.9);
        // The playable level (`core::sim::Level`): 0 on it, a drop in
        // metres below it, -1 unknown.
        let level = match sim.level_at(p) {
            core::sim::Level::On => 0.0,
            core::sim::Level::Off(d) => d.max(0.01),
            core::sim::Level::Unknown => -1.0,
        };
        let r = u.ai.route();
        let (steer, fan) = match u.ai.steer {
            SteerPick::Idle => (Steer::Idle, 0),
            SteerPick::Fwd => (Steer::Fwd, 0),
            SteerPick::Slide => (Steer::Slide, 0),
            SteerPick::Round => (Steer::Round, 0),
            SteerPick::Fan(k) => (Steer::Fan, k),
            SteerPick::Back => (Steer::Back, 0),
            SteerPick::Ledge => (Steer::Ledge, 0),
        };
        let live_hold = u.ai.hold.filter(|h| tick < h.until);
        let secs_to = |t: u64| t.saturating_sub(tick) as f32 / core::TICK_HZ as f32;
        let mut row = Probe {
            id: h.index,
            gen: h.gen,
            team: u.team,
            class: u.spec.class,
            human: u.is_player,
            pos: p.to_array(),
            yaw: u.soldier.yaw,
            speed: glam::Vec2::new(v.x, v.z).length(),
            grounded: u.soldier.grounded,
            stance: stance_code(u.soldier.stance),
            hp: u.health.cur,
            veh: u.mount.map_or(-1, |m| m.vehicle.index as i32),
            seat: u.mount.map_or(0, |m| m.seat),
            locus,
            hub,
            floor_dy,
            level,
            overhead,
            goal,
            goal_cp,
            cp: u.ai.target_cp.map_or(-1, |c| c as i32),
            ordered: u.command.is_some(),
            leg: r.is_some(),
            // The leg (card OFFG): its arc (-1 the last leg or a straight
            // line), the hub it walks to, the goal's hub, metres to go.
            arc: r.and_then(|r| r.via_arc).map_or(-1, i32::from),
            from: r
                .and_then(|r| {
                    let arc = sim.nav.as_ref()?.arcs.get(usize::from(r.via_arc?))?;
                    Some(i32::from(arc.other(r.next_hub)))
                })
                .unwrap_or(-1),
            next: r.map_or(-1, |r| i32::from(r.next_hub)),
            dest: r.map_or(-1, |r| i32::from(r.dest_hub)),
            src: r.map_or('-', |r| match r.source {
                core::ai::RouteSource::Table => 't',
                core::ai::RouteSource::Bfs => 'b',
                core::ai::RouteSource::Straight => 's',
                core::ai::RouteSource::Flank => 'f',
                core::ai::RouteSource::Shun => 'r',
            }),
            wp: r.map_or([0.0; 3], |r| r.waypoint.to_array()),
            wp_d: r.map_or(-1.0, |r| core::nav::xz_dist(p, r.waypoint)),
            steer,
            fan,
            slide: u.ai.slide,
            sidestep: r.map_or(0, |r| r.sidestep),
            doing: doing_of(u.ai.behaviour),
            walk_owner: (!sim.space_round()
                && u.controller == core::unit::Controller::Ai
                && u.proxy_human.is_none()
                && u.ai.step != core::ai::StepKind::None)
                .then_some(u.ai.step),
            recover: u.ai.mv.recover,
            round: u.ai.mv.round.live(),
            round_near: u.ai.mv.round.meet,
            round_t: u.ai.mv.round.ticks,
            round_rim: u.ai.mv.round.rim,
            walk: u.ai.walk.kind,
            walk_at: u.ai.walk.goal.to_array(),
            walk_dir: [u.ai.walk.dir.x, u.ai.walk.dir.z],
            stop: Stop::Open,
            stop_d: 0.0,
            stop_n: [0.0; 2],
            body: -1,
            // 2 holding a position, 1 owing hubs after one, 0 neither.
            hold: u.ai.hold.map_or(0, |h| if tick < h.until { 2 } else { 1 }),
            hold_goal: live_hold.map_or([0.0; 3], |h| h.goal.to_array()),
            hold_spot: live_hold.map_or([0.0; 3], |h| h.spot.to_array()),
            hold_left: live_hold.map_or(0.0, |h| secs_to(h.until)),
            hold_side: live_hold.map_or(0, |h| h.strafe_side),
            hold_commit: live_hold.map_or(0.0, |h| secs_to(h.commit_until)),
            hold_blocked: live_hold.map_or(0, |h| h.blocked),
            fmode: u.ai.fight.mode,
            tpos: u.ai.tpos.held,
            tpos_at: u
                .ai
                .tpos
                .held
                .and_then(|i| sim.positions.positions.get(i as usize))
                .map_or([0.0; 3], |q| q.pos.to_array()),
            tpos_bits: u8::from(u.ai.tpos.covered)
                | (u8::from(u.ai.tpos.firing) << 1)
                | match u.ai.tpos.lean {
                    -1 => 4,
                    1 => 8,
                    _ => 0,
                },
            tpos_hits: u.ai.tpos.hits,
            tpos_phase: u
                .ai
                .tpos
                .held
                .map_or(0.0, |_| secs_to(u.ai.tpos.phase_until)),
            target: u
                .ai
                .target
                .and_then(|t| t.unit())
                .map_or(-1, |t| t.index as i32),
            target_d: u
                .ai
                .target
                .and_then(|t| match t {
                    Foe::Unit(h) => sim.unit(h).map(|o| o.soldier.pos),
                    Foe::Droid(id) => sim.live_droid(id).map(|d| d.pos),
                    Foe::Turret(id) => sim.live_turret(id).map(|t| t.centre()),
                })
                .map_or(-1.0, |t| t.distance(p)),
            target_droid: u.ai.target.and_then(|t| t.droid()),
            stuck_t: r.map_or(0, |r| r.stuck_ticks),
            stuck: r.map_or(0, |r| r.stuck_run),
            stall: u.ai.stall.map_or(0, |s| s.ticks),
            lost: u.ai.lost.map_or(0, |l| if l.confirmed { 2 } else { 1 }),
            lost_tries: u.ai.lost.map_or(0, |l| l.tries),
            board: u
                .ai
                .ride
                .and_then(|r| r.board)
                .map_or(-1, |v| v.index as i32),
            board_at: u
                .ai
                .ride
                .and_then(|r| r.board)
                .and_then(|v| sim.vehicles.get(v.index as usize))
                .map_or([0.0; 3], |v| v.pos.to_array()),
            skill: u.skill,
            kills: u.kills,
            deaths: u.deaths,
        };
        if deep && !u.is_player && u.mount.is_none() {
            row.ask_stop(sim, u);
        }
        row
    }

    /// The steering's own tests (`core::ai::way_ahead`) along the direction
    /// and over the floor it was asked this tick (`AiState::walk`): which
    /// one says "shut", how far, and the body. Nothing for a bot that did not
    /// walk.
    fn ask_stop(&mut self, sim: &Sim, u: &Unit) {
        let w = u.ai.walk;
        if w.kind == 0 || w.dir == Vec3::ZERO {
            return;
        }
        let Some(s) = core::ai::way_ahead(sim, u, w.dir, w.low) else {
            return;
        };
        self.stop = match s.kind {
            // An opening door's leaf (card MOUTH2): the body's name says so.
            StopKind::Wall | StopKind::Door => Stop::Wall,
            StopKind::Barrier => Stop::Barrier,
            StopKind::Hull => Stop::Hull,
            StopKind::Steep => Stop::Steep,
            StopKind::Drop => Stop::Drop,
        };
        self.stop_d = s.dist;
        self.stop_n = [s.normal.x, s.normal.z];
        self.body = s
            .body
            .and_then(|b| {
                sim.collision
                    .bodies()
                    .iter()
                    .position(|x| std::ptr::eq(x, b))
            })
            .map_or(-1, |i| i as i32);
    }

    /// The steering pick as the sweep prints it (`SteerPick::label`).
    pub fn steer_label(&self) -> String {
        match self.steer {
            Steer::Idle => "idle".to_owned(),
            Steer::Fwd => "fwd".to_owned(),
            Steer::Slide => "slide".to_owned(),
            Steer::Back => "back".to_owned(),
            Steer::Ledge => "ledge".to_owned(),
            Steer::Round => "round".to_owned(),
            Steer::Fan => format!(
                "fan {}{}",
                if self.fan < 0 { "-" } else { "+" },
                i32::from(self.fan).abs() * 30
            ),
        }
    }

    /// The objective as the sweep prints it: `attack3`, `defend0`, `hunt`.
    pub fn goal_label(&self) -> String {
        match self.goal {
            Goal::Attack => format!("attack{}", self.goal_cp),
            Goal::Defend => format!("defend{}", self.goal_cp),
            Goal::Hunt => "hunt".into(),
            Goal::None => "none".into(),
        }
    }
}
