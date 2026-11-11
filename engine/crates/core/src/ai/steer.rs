//! Local steering probes under the route: wall sweeps ([`wall_ahead`],
//! [`way_ahead`]), edge `BARR` barriers ([`barrier_ahead`], BNDS), low
//! vehicle hulls (STFAL), steep floors ([`steep_ahead`], STHOP), safe floor
//! ([`floor_ahead`], STFAL) and [`rotate_xz`]. `avoid::pick` chooses the
//! heading from these probes; the sweep mechanism is OURS (see `ai.rs`).
//! Floor/hull evidence: `analysis/re-round-stalls.md` C1, C8.
//!
//! Opening doors (MOUTH2 / DOORS, `analysis/re-bf2-doors.md` 11): a leaf
//! whose door is opening ([`Sim::door_opening`]) is [`StopKind::Door`]
//! when nothing else blocks the way. Avoidance drops its obstacle commitment
//! and walks toward its point; the wall pass holds it until the leaf clears.
//! At the clip's end a leaf still in the way is a wall again. This is OURS
//! in mechanism (the PS2 has no sweep), reproducing its bots' wait at leaves.
//! A steep-floor ray can hit a rolled leaf: a steep face in an opening
//! leaf's box ([`Sim::opening_leaf_over`]) is excluded too, so it cannot
//! outrank the door answer. This exemption applies only while opening.
//!
//! Ramps are tested along their floor ([`ramp_clear`], OFFG). A short steep
//! band is passable when its own slope reaches a floor the stick can hold
//! within `SHORT_BAND_RISE`, with standing clearance throughout
//! ([`short_band`], SKIRT). This matches `Soldier::resolve_ground` carrying
//! a run up Kamino's pod skirt (`analysis/re-soldier-ground.md` 7).
//! TAT1's ramp and BF2 Geonosis's buttress cannot be stood on and stay walls.
//!
//! Measurements and remaining door/jamb cases are in the existing reports
//! under `engine/artifacts/loop/reports/`: `20261012_1729_card_MOUTH2.md`,
//! `20261012_2027_card_DOORS.md`, `20261015_1233_card_SKIRT.md`.
//! The local walking tests also cover the stuck-window side-step and the
//! soldier frame used to build the pad.

use super::{
    BARRIER_MARGIN, EDGE_LOOK, HULL_PUSH_MIN, LEDGE_MARGIN, PROBE_DIST, RAMP_MIN_RISE, RAMP_SAMPLE,
    RAMP_SAMPLE_RISE,
};
use crate::collision::Capsule;
use crate::fmath;
use crate::sim::Sim;
use glam::Vec3;

/// Rotate an XZ direction about +Y by `a` radians. Public because a
/// follower's standing point is drawn in its LEADER's frame
/// ([`crate::squad::stand_offset`]) and this is that rotation.
pub fn rotate_xz(v: Vec3, a: f32) -> Vec3 {
    let (sa, ca) = fmath::sin_cos(a);
    Vec3::new(v.x * ca + v.z * sa, 0.0, -v.x * sa + v.z * ca)
}

/// Card STFAL (`analysis/re-round-stalls.md` C1): is there a floor ONE
/// PROBE AHEAD along `dir` no lower than `low`? The ground query
/// ([`Sim::ground_under`]) at `u`'s feet moved [`PROBE_DIST`] along `dir`
/// and up by as much, and at the two points [`LEDGE_MARGIN`] either side of
/// it: the soldier stands on ONE ray under its feet, so a step that keeps
/// the feet a few cm inside a walkway's edge is one tick of drift from the
/// drop (KAM1 seed 1, unit 29: a slide along a wall 0.1 m from the edge, a
/// 4 cm drift, the sea).
///
/// OURS, like the sweep. On BF2 KAM1 the steering's own moves — the slide,
/// the fan, the 5 m side-step, off-line corner points and straight legs
/// across open air — carried units off 5–8 m walkways: 98.6 % of deaths
/// over 8 seeds were the sea under the decks, where the PS2's bots fall
/// 1.6 % of theirs (live RAM, 420 s). A step the steering commits has to
/// keep a floor under it.
pub(super) fn floor_ahead(sim: &Sim, u: &crate::unit::Unit, dir: Vec3, low: f32) -> bool {
    let d = Vec3::new(dir.x, 0.0, dir.z);
    // Asked from a probe's length ABOVE the feet: the ground query starts
    // its ray at feet + `STEP_HEIGHT`, so from the feet a ramp rising more
    // than that in a metre (BF2 DAG1's 35 deg hut ramps, GEO1's) has its
    // floor over the ray's start and reads as no floor at all.
    let at = u.soldier.pos + d * PROBE_DIST + Vec3::Y * PROBE_DIST;
    let side = Vec3::new(-d.z, 0.0, d.x) * LEDGE_MARGIN;
    [at, at + side, at - side]
        .into_iter()
        .all(|p| sim.ground_point_under(p, false) >= low)
}

/// Card STFAL (C8): is the way ahead along `dir` shut, and by what (the
/// wall's normal)? The SWEEP of the stepped capsule first
/// ([`crate::collision::CollisionWorld::wall_sweep`]); when it is clear, the
/// wall pass's own answer against the vehicles' hulls at the probe's end
/// ([`crate::collision::CollisionWorld::push_out_soldier_hulls`], the FULL
/// capsule with the ledge rule) — a push back against `dir` of more than
/// [`HULL_PUSH_MIN`] is a wall too. Hulls only: MEASURED, the same test
/// against the map's own bodies cost BF2 GEO1 and TAN1 finishes (8 → 5 and
/// 8 → 6 of 8 seeds).
///
/// The sweep lifts its capsule by the step, so a low solid whose top is over
/// the step but under the lifted capsule's round bottom — the feet of BF1
/// KAS2's three parked AT-STs, 2 m ahead of the Empire spawn — reads clear,
/// while the pass pushes the unit back off it every tick (20 episodes, a
/// 20,520-tick stuck run at seed 5).
pub(super) fn wall_ahead(
    sim: &Sim,
    u: &crate::unit::Unit,
    cap: &Capsule,
    dir: Vec3,
) -> Option<Vec3> {
    stop_ahead(sim, u, cap, dir)
        .filter(|s| s.kind != StopKind::Door)
        .map(|s| s.normal)
}

/// Which of the steering's tests shut the way ahead ([`Stop`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopKind {
    /// The sweep of the stepped capsule met a body (and no ramp carries the
    /// unit over it).
    Wall,
    /// A `BARR` barrier at an edge of the level (card BNDS).
    Barrier,
    /// A vehicle's hull at the probe's end (card STFAL).
    Hull,
    /// A floor too steep to climb (card STHOP) — never a face of a leaf of
    /// a door that is opening (card DOORS, [`Sim::opening_leaf_over`]).
    Steep,
    /// No floor one probe ahead (card STFAL) — [`way_ahead`] only.
    Drop,
    /// The sweep met a leaf of a door that is opening (card MOUTH2) and
    /// nothing else shuts the way: no wall ([`wall_ahead`] says open) — the
    /// avoidance walks on at the point and drops what it was going round.
    Door,
}

/// What shut the way ahead: [`wall_ahead`]'s answer with its reason, the
/// sweep's distance and the body it met. The steering reads the normal; the
/// rest is for a diagnostic ([`way_ahead`]).
#[derive(Clone, Copy)]
pub struct Stop<'a> {
    pub kind: StopKind,
    pub normal: Vec3,
    /// Metres to the body along the sweep ([`StopKind::Wall`]); the probe's
    /// length otherwise.
    pub dist: f32,
    /// The map body the sweep met ([`StopKind::Wall`] only).
    pub body: Option<&'a crate::collision::Body>,
}

pub(super) fn stop_ahead<'a>(
    sim: &'a Sim,
    u: &crate::unit::Unit,
    cap: &Capsule,
    dir: Vec3,
) -> Option<Stop<'a>> {
    let stop = |kind, normal| Stop {
        kind,
        normal,
        dist: PROBE_DIST,
        body: None,
    };
    // Card MOUTH2: a leaf of a door that is opening is no wall — the bot
    // walks on at its point and is through when the leaf has slid clear
    // ([`Sim::door_opening`]; the header has the measurement). It is the
    // answer only when none of the other tests shuts the way.
    let mut door = None;
    if let Some((t, n, body)) = sim.collision.wall_sweep_body(cap, dir, PROBE_DIST) {
        let met = |kind| Stop {
            kind,
            normal: n,
            dist: t,
            body: Some(body),
        };
        if sim.door_opening(body) {
            door = Some(met(StopKind::Door));
        // Card OFFG: the sweep is level, the floor may not be. Up a ramp the
        // capsule is asked again along the floor that will carry it.
        } else if !ramp_clear(sim, u, cap, dir) {
            return Some(met(StopKind::Wall));
        }
    }
    if let Some(n) = barrier_ahead(sim, u, dir) {
        return Some(stop(StopKind::Barrier, n));
    }
    let mut end = u.capsule();
    end.base += dir * PROBE_DIST;
    let push = sim.collision.push_out_soldier_hulls(&end);
    if push.dot(dir) < -HULL_PUSH_MIN {
        return Some(stop(StopKind::Hull, push.normalize()));
    }
    // Card DOORS: the steep-floor probe's ray comes down a probe's length
    // ahead, and at a door that is opening it can come down on the leaf
    // (the diagonal edge of BF2 Death Star's blast door is a face at n.y
    // 0.69 over the feet). A face found in the box of an opening leaf is
    // the leaf's, and the leaf is leaving: no steep floor.
    steep_ahead(sim, u, dir)
        .filter(|_| !sim.opening_leaf_over(u.soldier.pos + dir * PROBE_DIST))
        .map(|n| stop(StopKind::Steep, n))
        .or(door)
}

/// A diagnostic, never called by the sim (the playtest black box, card
/// BLKB): what the steering's tests say about a step along `dir` for `u` —
/// the first of [`stop_ahead`]'s that shuts it, else a missing floor no lower
/// than `low` ([`floor_ahead`]), else `None`: the way is open.
pub fn way_ahead<'a>(sim: &'a Sim, u: &crate::unit::Unit, dir: Vec3, low: f32) -> Option<Stop<'a>> {
    let cap = u.capsule().stepped();
    stop_ahead(sim, u, &cap, dir).or_else(|| {
        (!floor_ahead(sim, u, dir, low)).then_some(Stop {
            kind: StopKind::Drop,
            normal: -dir,
            dist: PROBE_DIST,
            body: None,
        })
    })
}

/// Card OFFG: is the probe ahead along `dir` a RAMP the soldier walks up,
/// with nothing standing on it? The floor is sampled every [`RAMP_SAMPLE`]:
/// no sample more than [`RAMP_SAMPLE_RISE`] over the one before it (a 45
/// degree floor rises 0.25 m in 0.25 m; a crate's side is a step, not a
/// ramp) and [`RAMP_MIN_RISE`] or more in all. Then the stepped capsule is
/// swept over each sample's stretch from the height of the floor at its end,
/// so a rail or a wall standing on the slope is still met.
///
/// The wall sweep moves the stepped capsule LEVEL, [`STEP_HEIGHT`] over the
/// feet. A ramp steeper than about 23 degrees rises past that inside the
/// probe and its reach, so from the ramp's foot the sweep meets the slope
/// itself, or whatever stands at its top at kerb height, and calls a wall
/// what the soldier walks up. MEASURED on BF2 Kamino (main 8c34344, 8 seeds),
/// whose bridges end in 32 degree ramps (1.08 m up in 2 m): the sweep
/// answered a wall facing straight back 0.8 m ahead of a bot at the foot,
/// 752 of 985 walks at the bridge from hub 65 to hub 60 turned back there,
/// 12.7 % of all unit time went on legs given up before their hub, the 14
/// worst of them bridges; rounds took 23 minutes at 8.0 kills a minute
/// (PS2 17.3).
///
/// [`STEP_HEIGHT`]: crate::collision::STEP_HEIGHT
fn ramp_clear(sim: &Sim, u: &crate::unit::Unit, cap: &Capsule, dir: Vec3) -> bool {
    const STEPS: usize = (PROBE_DIST / RAMP_SAMPLE) as usize;
    let d = Vec3::new(dir.x, 0.0, dir.z);
    let feet = u.soldier.pos;
    let mut rise = [0.0f32; STEPS];
    let mut y = feet.y;
    for (k, r) in rise.iter_mut().enumerate() {
        let p = feet + d * (RAMP_SAMPLE * (k + 1) as f32);
        let f = sim.ground_point_under(Vec3::new(p.x, y, p.z), false);
        if f <= crate::collision::NO_GROUND || f - y > RAMP_SAMPLE_RISE {
            return false;
        }
        y = f;
        *r = f - feet.y;
    }
    if y - feet.y < RAMP_MIN_RISE {
        return false;
    }
    rise.iter().enumerate().all(|(k, r)| {
        let mut c = *cap;
        c.base += d * (RAMP_SAMPLE * k as f32) + Vec3::Y * *r;
        sim.collision.wall_sweep(&c, dir, RAMP_SAMPLE).is_none()
    })
}

/// Card BNDS: does one probe ahead along `dir` take the unit into a `BARR`
/// barrier that is an EDGE of the playable level — the step goes in at the
/// barrier (grown by [`BARRIER_MARGIN`]), and the ground [`EDGE_LOOK`] on
/// along `dir` is off the level (`Sim::level_at`) or is not there at all?
/// Then the barrier is a wall, with the normal of the side the unit is on.
///
/// The disc bounds the AI's walkable areas with barriers (BF2 Kamino rings
/// every platform's rim rail and lines every bridge with them) and the PS2's
/// walk step never crosses one: `FUN_00250fc8` tests every step and re-aims
/// at the barrier's corner (RE_LOG "Barriers are the PS2's Obstacles"). The
/// port tested only the LEG when it was planned, and its slide, fan and 5 m
/// side-step then carried units over Kamino's rails (26 % of unit-seconds
/// off the graph, 48 lives of 20 s or more off it in four 10-minute rounds).
///
/// OURS (PROVISIONAL), the edge test: most barriers stand where there is a
/// wall anyway, or across a doorway a leg walks through, and taking EVERY
/// barrier as a wall cost BF2 Hoth and both Tatooines most of their rounds
/// (8 seeds: 8/8 → 0/8, 8/8 → 3/8, 8/8 → 2/8) — the port's corner walk is
/// planned once per leg, not re-aimed per step like the PS2's. A barrier
/// with the level's floor on its far side is left to the wall sweep; one
/// with a hull, a drop or the sea behind it is a rim. A unit coming BACK
/// over a rim has the deck ahead of it, so nothing stops it.
fn barrier_ahead(sim: &Sim, u: &crate::unit::Unit, dir: Vec3) -> Option<Vec3> {
    let g = sim.nav.as_ref()?;
    if g.barriers.is_empty() {
        return None;
    }
    let ty = super::route::mover(sim, u).ty;
    let a = glam::Vec2::new(u.soldier.pos.x, u.soldier.pos.z);
    let step = glam::Vec2::new(dir.x, dir.z);
    let b = a + step * PROBE_DIST;
    let reach = PROBE_DIST + BARRIER_MARGIN;
    let mut edge: Option<bool> = None;
    for bar in &g.barriers {
        if !bar.enabled || !bar.stops(ty) {
            continue;
        }
        let d = a - bar.centre;
        // Coarse reject: the PS2 keeps the same bound at +0x8c.
        let far = bar.bound_radius() + reach;
        if d.length_squared() > far * far {
            continue;
        }
        if !bar.crosses_r(a, b, BARRIER_MARGIN) {
            continue;
        }
        // The side of the rectangle the unit stands on; a step along the
        // barrier or out of it is not a step in.
        let (x, y) = (d.dot(bar.axes[0]), d.dot(bar.axes[1]));
        let n = if bar.half.x - x.abs() < bar.half.y - y.abs() {
            bar.axes[0] * x.signum()
        } else {
            bar.axes[1] * y.signum()
        };
        if step.dot(n) > -1e-3 {
            continue;
        }
        // What lies past it, asked once per direction.
        let off = *edge.get_or_insert_with(|| {
            // Asked from as far over the feet as it is ahead of them: ground
            // that RISES past the barrier is still ground (BF2 Geonosis's
            // dunes read as no floor from a metre up, and every barrier on a
            // slope became a wall — 7/8 rounds → 3/8).
            let q = u.soldier.pos + Vec3::new(dir.x, 0.0, dir.z) * EDGE_LOOK;
            let f = sim.ground_point_under(q + Vec3::Y * EDGE_LOOK, false);
            f <= crate::collision::NO_GROUND || sim.level_at(Vec3::new(q.x, f, q.z)).is_off()
        });
        if off {
            return Some(Vec3::new(n.x, 0.0, n.y));
        }
        return None;
    }
    None
}

/// Card STHOP: is the floor one probe ahead along `dir` a face the soldier's
/// ground rules will not let him climb — above his feet, rising along `dir`
/// and steeper than the game's control limit
/// ([`crate::soldier::GroundRules::climb_min_ny`])? Then it is a wall for the
/// steering, with the face's horizontal normal.
///
/// The PS2 takes a contact with n.y <= 0.6 as a wall and gives the stick no
/// hold on a floor under n.y 0.7 (BF1 0.75) — analysis/re-soldier-ground.md
/// §1.1, §1.3. The port's wall pass and its sweep call a face a wall only
/// under n.y 0.5 (`WALKABLE_MIN_NY`), and before STHOP the ground clamp
/// walked a soldier up anything flatter. MEASURED with the PS2's rules and no
/// probe: bots pressed on such faces for the rest of a round (TAT1's 55 deg
/// ramp at (-137, 95), 3 of 8 seeds without an outcome; GEO1's buttress at
/// (-72, 268), 2 of 8).
fn steep_ahead(sim: &Sim, u: &crate::unit::Unit, dir: Vec3) -> Option<Vec3> {
    let d = Vec3::new(dir.x, 0.0, dir.z);
    let at = u.soldier.pos + d * PROBE_DIST + Vec3::Y * PROBE_DIST;
    let steep = |(y, n): (f32, Vec3)| {
        let h = Vec3::new(n.x, 0.0, n.z);
        (y > u.soldier.pos.y && n.y < sim.ground_rules.climb_min_ny() && h.dot(d) < 0.0)
            .then(|| h.normalize())
    };
    // ONE ray first (card OBST2): the point is a probe's length over the
    // feet, so the foot's ring of eight more rays ran on every pick over a
    // level floor — 15 % of the whole simulation. The ring is asked only of
    // a face the point itself calls steep: a kerb's bevel has a top the ring
    // finds (card RAIL), and that is no wall.
    steep(sim.ground_point_under_n(at, at.y))?;
    steep(sim.ground_under_n(at, false, at.y)).filter(|&h| !short_band(sim, u, dir, h))
}

/// How high a band [`short_band`] lets the steering walk at: the soldier's
/// body takes 1.5 m of a 48 degree face from a run-up, the PS2's 1.49-1.50
/// (re-soldier-ground.md §7.1), and the test's 0.2 m samples read a band's
/// top up to 0.1 m high. PROVISIONAL (ours, sized on BF2 Kamino's pod skirt).
const SHORT_BAND_RISE: f32 = 1.6;
/// [`short_band`]'s march up the face: samples this far apart, this many
/// (2.4 m: a 1.6 m band at the steepest floor, 53 degrees, is 1.2 m deep).
const SHORT_BAND_STEP: f32 = 0.2;
const SHORT_BAND_SAMPLES: u32 = 12;
/// A floor this far over the feet is the band's top, not the floor before
/// it; a sample this far under the last one is a drop (a ridge).
const SHORT_BAND_SLACK: f32 = 0.3;

/// The band a soldier's run carries him over (card SKIRT; re-soldier-ground.md
/// §7): the steep face [`steep_ahead`] met, of horizontal normal `h`, is no
/// wall when, marching up the face's OWN slope from the probe's point, every
/// floor is one the body stands on (over [`crate::soldier::FLOOR_MIN_NY`]: the soldier keeps
/// his contact and climbs on his momentum) and a floor the stick holds on
/// begins within [`SHORT_BAND_RISE`] over the feet. Up the face's slope, not
/// along `dir`: a bot walking at a point on BF2 Kamino's pod skirt meets the
/// band at a slant, and along its own way the band never ends. A face the
/// soldier cannot stand on (TAT1's ramp at (-137, 95), n.y 0.57; BF2
/// Geonosis's buttress, 0.53) or a ridge that drops away behind its top
/// stays a wall.
fn short_band(sim: &Sim, u: &crate::unit::Unit, dir: Vec3, h: Vec3) -> bool {
    let feet = u.soldier.pos;
    let up = -Vec3::new(h.x, 0.0, h.z).normalize_or_zero();
    let start = feet + Vec3::new(dir.x, 0.0, dir.z) * PROBE_DIST;
    let from_y = feet.y + SHORT_BAND_RISE + 0.5;
    let mut last = f32::NEG_INFINITY;
    for k in 0..=SHORT_BAND_SAMPLES {
        let q = start + up * (k as f32 * SHORT_BAND_STEP);
        let (y, n) = sim.ground_point_under_n(Vec3::new(q.x, from_y, q.z), from_y);
        if y < last - SHORT_BAND_SLACK {
            return false; // a drop: a ridge, not a band with a floor on top
        }
        if n.y >= sim.ground_rules.climb_min_ny() {
            if y > feet.y + SHORT_BAND_SLACK {
                // the band's top, halfway between its last sample and the floor's first
                let top = if last.is_finite() {
                    0.5 * (last + y)
                } else {
                    y
                };
                return top <= feet.y + SHORT_BAND_RISE;
            }
        } else if n.y <= crate::soldier::FLOOR_MIN_NY || y > feet.y + SHORT_BAND_RISE {
            return false;
        }
        last = y;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::floor_ahead;
    use super::*;
    use crate::ai::tests::l_sim;
    use crate::ai::{think, AiState, NavTick, SteerPick, STUCK_WINDOW};
    use crate::sim::PadState;

    /// One pick of the avoidance for a unit that is going round nothing yet
    /// (`slide`, when not 0, is a side only asked for), at a point 30 m along
    /// `want`.
    fn steer_dir(
        sim: &Sim,
        u: &crate::unit::Unit,
        want: Vec3,
        low: f32,
        slide: &mut i8,
        side: i8,
        nt: &mut NavTick,
    ) -> (Vec3, SteerPick) {
        let mut rd = crate::ai::Round::default();
        let aim = u.soldier.pos + want * 30.0;
        crate::ai::avoid::pick(sim, u, want, low, aim, slide, side, &mut rd, nt)
    }

    /// Card MOUTH2's scene: a sliding door across `l_sim`'s first leg (the
    /// origin to hub 1 at z -40), 7.5 m wide between two walls, its two
    /// leaves sliding 3.75 m apart in 41 frames — BF2 Kamino's pod room door
    /// — and its centre 2.4 m to the bot's left, so the bot comes at it on an
    /// OUTER lane, where the leaf is still in the way when he gets there.
    /// `trigger` is the radius of the door's trigger about its centre.
    fn door_scene(trigger: f32) -> (Sim, crate::unit::UnitHandle) {
        door_scene_thick(trigger, 0.2)
    }

    /// [`door_scene`] with leaves `2 * half_z` thick (the walls beside them
    /// too): Kamino's are 0.4 m, BF2 Death Star's blast doors over a metre —
    /// thicker than the steering's probe is long (card DOORS).
    fn door_scene_thick(trigger: f32, half_z: f32) -> (Sim, crate::unit::UnitHandle) {
        use crate::collision::{Aabb, Body, CollisionWorld, Shape};
        use crate::door::{Door, DoorBody, Trigger};
        use glam::{Mat4, Quat};
        const CX: f32 = -2.4;
        const Z: f32 = -12.0;
        let (mut sim, h) = l_sim();
        let slab = |x: f32, half_x: f32| {
            Body::new(
                Mat4::from_translation(Vec3::new(x, 1.5, Z)),
                Shape::Box {
                    half: Vec3::new(half_x, 1.5, half_z),
                },
            )
        };
        let bodies = vec![
            slab(CX - 3.75 - 10.0, 10.0),
            slab(CX + 3.75 + 10.0, 10.0),
            slab(CX - 1.875, 1.875),
            slab(CX + 1.875, 1.875),
        ];
        let mut env = Aabb::empty();
        env.add(Vec3::new(CX - 7.5, 0.0, Z - half_z));
        env.add(Vec3::new(CX + 7.5, 3.0, Z + half_z));
        sim.collision = CollisionWorld::with_movers(bodies, Vec::new(), vec![(2, env), (3, env)]);
        let leaf = |body: usize, side: f32| DoorBody {
            body,
            frames: (0..41)
                .map(|f| {
                    let slid = 3.75 * f as f32 / 40.0;
                    (
                        Quat::IDENTITY,
                        Vec3::new(CX + side * (1.875 + slid), 1.5, Z),
                    )
                })
                .collect(),
        };
        sim.doors = vec![Door {
            triggers: vec![Trigger {
                center: Vec3::new(CX, 0.0, Z),
                radius: trigger,
            }],
            frames: 41,
            bodies: vec![leaf(2, -1.0), leaf(3, 1.0)],
            ..Door::default()
        }];
        (sim, h)
    }

    /// Card MOUTH2's acceptance: a bot sent through a sliding door on an
    /// outer lane is through on the first try. It meets the leaf before the
    /// leaf has slid clear (the door takes 1.33 s, the bot is there 0.5 s
    /// after it enters the trigger), walks on at its point — no pick of a
    /// side, no slide, no step back — and has lost no more than the clip's
    /// length against a walk with no door at all. The PS2's bots stand half
    /// a second at that leaf and go through (re-bf2-doors.md 11); the port's
    /// went round it: 121 stalls in 270 passes of BF2 Kamino's pod room door.
    ///
    /// The control is the same scene with a trigger nobody ever stands in:
    /// the leaf is then a wall like any other and the avoidance goes round
    /// it, so it is the door's opening that the rule answers to.
    #[test]
    fn a_bot_walks_on_at_a_door_that_is_opening_and_is_through_first_try() {
        // (ticks to pass z -25, picks that were not straight on, the side
        // committed to at any tick, ticks stood at the leaf, the furthest
        // the door opened)
        let walk = |sim: &mut Sim, h| {
            let (mut past, mut blocked, mut side, mut held) = (None, 0u32, false, 0u32);
            let mut opened = 0.0f32;
            for t in 0..600u32 {
                sim.step(&[PadState::default()]);
                let u = sim.unit(h).expect("alive");
                blocked += u32::from(!matches!(
                    u.ai.steer,
                    SteerPick::Fwd | SteerPick::Round | SteerPick::Idle
                ));
                side |= u.ai.slide != 0;
                let v = u.soldier.vel;
                held += u32::from(u.soldier.pos.z > -12.0 && v.x.hypot(v.z) < 1.0 && t > 30);
                opened = opened.max(sim.doors.first().map_or(0.0, |d| d.open));
                if u.soldier.pos.z < -25.0 {
                    past = Some(t);
                    break;
                }
            }
            (past, blocked, side, held, opened)
        };
        let (mut open_field, h) = l_sim();
        let free = walk(&mut open_field, h).0.expect("an open leg is walked");

        let (mut sim, h) = door_scene(4.0);
        let (past, blocked, side, held, opened) = walk(&mut sim, h);
        let past = past.expect("the bot is through the door");
        assert!(held > 0, "the scene: the leaf was still in his lane");
        assert_eq!(blocked, 0, "no pick but straight on at his point");
        assert!(!side, "no side committed to");
        assert!(
            past <= free + 40,
            "through within the clip's length of a free walk: {past} against {free}"
        );
        assert!(opened > 0.9, "and it was his door that opened: {opened}");

        // The control: a door that does not open for him is a wall.
        let (mut shut, h) = door_scene(0.5);
        let (_, blocked, side, _, opened) = walk(&mut shut, h);
        assert!(
            blocked > 0 && side,
            "a leaf that is not opening is gone round: {blocked} picks"
        );
        assert_eq!(opened, 0.0, "it never opened");
    }

    /// Card MOUTH2: what the probes say of a leaf. While the door is opening
    /// the sweep's answer is [`StopKind::Door`] with the leaf as its body, and
    /// [`wall_ahead`] — what the avoidance asks of every other heading —
    /// says the way is open; once the clip is at its end a leaf still in the
    /// way is a wall again.
    #[test]
    fn an_opening_leaf_is_a_door_to_the_sweep_and_open_to_wall_ahead() {
        let (mut sim, h) = door_scene(4.0);
        let south = Vec3::new(0.0, 0.0, -1.0);
        let met = |sim: &Sim| {
            let u = sim.unit(h).expect("alive");
            let cap = u.capsule().stepped();
            (
                stop_ahead(sim, u, &cap, south).map(|s| (s.kind, s.body.is_some())),
                wall_ahead(sim, u, &cap, south).is_some(),
            )
        };
        let mut seen = false;
        for _ in 0..200 {
            sim.step(&[PadState::default()]);
            if let (Some((kind, body)), wall) = met(&sim) {
                assert_eq!(kind, StopKind::Door, "the leaf of a door that is opening");
                assert!(body, "with the leaf as its body");
                assert!(!wall, "and no wall to the other headings");
                seen = true;
                break;
            }
        }
        assert!(seen, "the bot's sweep met the leaf");
        // The same leaf, the clip at its end: a wall.
        sim.doors[0].open = 1.0;
        assert_eq!(met(&sim), (Some((StopKind::Wall, true)), true));
    }

    /// Card DOORS's scene: BF2 Death Star's blast door (`dea1_prop_door_blast`)
    /// across `l_sim`'s first leg — its six boxes as the bake poses them (the
    /// shut pose, relative to the door's origin on the floor; each slides
    /// 5 m along x in 151 frames, a 16 m trigger a metre up). The two leaves
    /// part on a DIAGONAL: the boxes of a leaf are rolled 43.9 degrees, so
    /// the edge that leaves the doorway last is a face at n.y 0.69 rising
    /// from the sill. The door's centre is 0.84 m to the bot's left: the
    /// lane unit 31 of seed 62 came down at the Empire's spawn (main 4dbb460).
    fn blast_door_scene() -> (Sim, crate::unit::UnitHandle) {
        use crate::collision::{Aabb, Body, CollisionWorld, Shape};
        use crate::door::{Door, DoorBody, Trigger};
        use glam::{Mat4, Quat};
        const CX: f32 = -0.84;
        const Z: f32 = -14.0;
        // (roll's quaternion x, x and y of the shut pose, the slide's sign,
        // the box's half extents)
        const LEAVES: [(f32, f32, f32, f32, [f32; 3]); 6] = [
            (0.37345, 0.871, 1.712, 1.0, [1.26, 3.50, 0.74]),
            (-0.37345, 0.945, 3.362, 1.0, [1.32, 3.60, 0.20]),
            (0.0, 2.626, 2.499, 1.0, [1.71, 2.57, 0.74]),
            (0.0, -2.626, 2.499, -1.0, [1.71, 2.57, 0.74]),
            (0.37345, -0.790, 3.410, -1.0, [1.26, 3.50, 0.74]),
            (-0.37345, -0.995, 1.712, -1.0, [1.32, 3.60, 0.20]),
        ];
        let (mut sim, h) = l_sim();
        let quat = |qx: f32| Quat::from_xyzw(qx, (1.0 - qx * qx).sqrt(), 0.0, 0.0);
        let wall = |x: f32| {
            Body::new(
                Mat4::from_translation(Vec3::new(x, 3.0, Z)),
                Shape::Box {
                    half: Vec3::new(10.0, 3.0, 0.74),
                },
            )
        };
        let mut bodies = vec![wall(CX - 15.0), wall(CX + 15.0)];
        let mut env = Aabb::empty();
        env.add(Vec3::new(CX - 12.0, -2.0, Z - 0.8));
        env.add(Vec3::new(CX + 12.0, 8.0, Z + 0.8));
        let mut movers = Vec::new();
        let mut leaves = Vec::new();
        for (qx, x, y, side, half) in LEAVES {
            let body = bodies.len();
            bodies.push(Body::new(
                Mat4::from_rotation_translation(quat(qx), Vec3::new(CX + x, y, Z)),
                Shape::Box {
                    half: Vec3::from(half),
                },
            ));
            movers.push((body, env));
            leaves.push(DoorBody {
                body,
                frames: (0..151)
                    .map(|f| {
                        let slid = 5.0 * f as f32 / 150.0;
                        (quat(qx), Vec3::new(CX + x + side * slid, y, Z))
                    })
                    .collect(),
            });
        }
        sim.collision = CollisionWorld::with_movers(bodies, Vec::new(), movers);
        sim.doors = vec![Door {
            triggers: vec![Trigger {
                center: Vec3::new(CX, 1.0, Z),
                radius: 16.0,
            }],
            frames: 151,
            bodies: leaves,
            ..Door::default()
        }];
        (sim, h)
    }

    /// Card DOORS's acceptance: a bot that comes to the Death Star's blast
    /// door while it opens (5 s; he is there after 1.5) waits at the leaf and
    /// is through on the first try — no side committed, no pick but straight
    /// on. Before, the steep-floor probe turned him away: its ray comes down
    /// a metre ahead onto the leaf's diagonal edge, a face over his feet
    /// that no soldier climbs, and that answer outranked the door's (seed 62
    /// unit 31: a side at the third pick, 22 m along the hangar wall).
    ///
    /// The probes are asked along the way to his leg's own point, as the
    /// avoidance asks them: the diagonal's level normal is square to the
    /// doorway, so it is the crumb of a walk that is not dead square that
    /// makes the face "rise along the way" (in the round as here).
    #[test]
    fn a_bot_waits_at_a_blast_door_whose_diagonal_leaf_reads_as_a_steep_floor() {
        let (mut sim, h) = blast_door_scene();
        let (mut past, mut blocked, mut side, mut held) = (None, 0u32, false, 0u32);
        // (ticks the leaf was the sweep's answer; ticks the probe's own ray
        // called the floor ahead steep, and of them with the sweep clear)
        let (mut met, mut steep, mut toe) = (0u32, 0u32, 0u32);
        for t in 0..600u32 {
            sim.step(&[PadState::default()]);
            let u = sim.unit(h).expect("alive");
            blocked += u32::from(!matches!(
                u.ai.steer,
                SteerPick::Fwd | SteerPick::Round | SteerPick::Idle
            ));
            side |= u.ai.slide != 0;
            let v = u.soldier.vel;
            held += u32::from(u.soldier.pos.z > -14.0 && v.x.hypot(v.z) < 1.0 && t > 30);
            let wp = u.ai.route().as_ref().expect("on its leg").waypoint;
            let want = Vec3::new(wp.x - u.soldier.pos.x, 0.0, wp.z - u.soldier.pos.z).normalize();
            let cap = u.capsule().stepped();
            let stop = stop_ahead(&sim, u, &cap, want);
            if let Some(s) = stop {
                assert_eq!(s.kind, StopKind::Door, "tick {t}: a leaf of his own door");
                assert!(wall_ahead(&sim, u, &cap, want).is_none());
                met += 1;
            }
            if steep_ahead(&sim, u, want).is_some() {
                steep += 1;
                toe += u32::from(stop.is_none());
            }
            if u.soldier.pos.z < -25.0 {
                past = Some(t);
                break;
            }
        }
        let past = past.expect("the bot is through the blast door");
        assert!(held > 0 && met > 0, "the scene: the leaf was in his lane");
        assert!(
            steep > 30 && toe > 0,
            "the scene: the probe's ray did call the leaf's edge steep ({steep} ticks), \
             and its toe after the sweep was clear ({toe})"
        );
        assert_eq!(blocked, 0, "no pick but straight on at his point");
        assert!(!side, "no side committed to");
        assert!(past <= 150, "through before the clip is over: tick {past}");

        // The control: the same face is a steep floor when the door is not
        // opening (the clip stopped where he stood at it, nobody's door).
        let (mut sim, h) = blast_door_scene();
        let mut seen = false;
        for _ in 0..200 {
            sim.step(&[PadState::default()]);
            let u = sim.unit(h).expect("alive");
            let wp = u.ai.route().as_ref().expect("on its leg").waypoint;
            let want = Vec3::new(wp.x - u.soldier.pos.x, 0.0, wp.z - u.soldier.pos.z).normalize();
            if steep_ahead(&sim, u, want).is_some() {
                let cap = u.capsule().stepped();
                sim.doors[0].triggers.clear();
                let u = sim.unit(h).expect("alive");
                let kind = stop_ahead(&sim, u, &cap, want).map(|s| s.kind);
                assert!(
                    matches!(kind, Some(StopKind::Wall | StopKind::Steep)),
                    "a leaf nobody opens shuts the way: {kind:?}"
                );
                seen = true;
                break;
            }
        }
        assert!(seen, "the control reached the leaf");
    }

    /// The fixtures stand on flat ground at y 0: any floor at or above this
    /// is one to walk on.
    const FLAT_LOW: f32 = -1.0;

    /// Card OBST2: the steep-floor probe asks ONE ray, and the foot's ring
    /// only of a face that ray calls steep. A kerb — a 53 deg bevel half a
    /// metre up to a flat top, BF2 Kamino's rim rail (card RAIL) — is under
    /// the point as its bevel and under the foot as its top: no wall. The
    /// same bevel going on up with no top is one.
    #[test]
    fn a_kerbs_bevel_under_the_probe_is_no_wall_when_the_foot_finds_its_top() {
        use crate::collision::{Body, CollisionWorld, Shape};
        let a = (0.5f32 / 0.375).atan();
        // The face rises toward -Z from the floor at z -0.6625: under the
        // probe's point (z -1) it is 0.45 m up.
        let face = |len: f32| {
            Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -0.6625))
                    * glam::Mat4::from_rotation_x(a)
                    * glam::Mat4::from_translation(Vec3::new(0.0, -0.5, -0.5 * len)),
                Shape::Box {
                    half: Vec3::new(5.0, 0.5, 0.5 * len),
                },
            )
        };
        let top = Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -3.5188)),
            Shape::Box {
                half: Vec3::new(5.0, 0.5, 2.4812),
            },
        );
        let scene = |bodies: Vec<Body>| {
            let (mut sim, _) = l_sim();
            sim.collision = CollisionWorld::new(bodies, Vec::new());
            sim.ground_rules = crate::soldier::GroundRules::Bf2;
            let u = sim.units.iter_mut().find(|u| u.alive).expect("a unit");
            u.soldier.pos = Vec3::ZERO;
            sim
        };
        let at = Vec3::new(0.0, 1.0, -1.0);
        for (name, bodies, wall) in [
            ("a kerb", vec![face(0.625), top], false),
            ("a steep face", vec![face(10.0)], true),
        ] {
            let sim = scene(bodies);
            let u = sim.units.iter().find(|u| u.alive).expect("a unit");
            // The one ray stands on the bevel either way.
            let (y, n) = sim.ground_point_under_n(at, at.y);
            assert!(
                (y - 0.45).abs() < 0.01 && (n.y - 0.6).abs() < 0.01,
                "{name}: {y} {n}"
            );
            let got = steep_ahead(&sim, u, Vec3::NEG_Z);
            assert_eq!(got.is_some(), wall, "{name}: {got:?}");
            if wall {
                assert!(got.is_some_and(|n| n.z > 0.99), "{name}: it faces back");
            }
        }
    }

    /// Card STHOP: a 55 deg face (n.y 0.57) is a floor to the ground ray and
    /// no wall to the sweep (`WALKABLE_MIN_NY` 0.5), and the soldier's ground
    /// rules will not climb it — so the steering has to call it a wall. A
    /// 30 deg ramp is not one, nor is the same steep face going DOWN.
    #[test]
    fn a_floor_too_steep_to_climb_is_a_wall_for_the_steering() {
        use crate::collision::{Body, CollisionWorld, Shape};
        let ramp = |deg: f32| {
            let (mut sim, h) = l_sim();
            // A slab tilted about X whose top face rises toward -Z, its low
            // edge about half a metre ahead of a unit at the origin.
            let a = deg.to_radians();
            let m = glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -0.5))
                * glam::Mat4::from_rotation_x(a)
                * glam::Mat4::from_translation(Vec3::new(0.0, -0.5, -5.0));
            sim.collision = CollisionWorld::new(
                vec![Body::new(
                    m,
                    Shape::Box {
                        half: Vec3::new(5.0, 0.5, 5.0),
                    },
                )],
                Vec::new(),
            );
            let u = sim.units.iter_mut().find(|u| u.alive).expect("a unit");
            u.soldier.pos = Vec3::ZERO;
            let _ = h;
            sim
        };
        for (deg, rules, blocked) in [
            (55.0, crate::soldier::GroundRules::Bf2, true),
            (55.0, crate::soldier::GroundRules::Bf1, true),
            (43.0, crate::soldier::GroundRules::Bf2, false), // n.y 0.731
            (43.0, crate::soldier::GroundRules::Bf1, true),  // BF1's limit is 0.75
            (30.0, crate::soldier::GroundRules::Bf1, false),
        ] {
            let mut sim = ramp(deg);
            sim.ground_rules = rules;
            let u = sim.units.iter().find(|u| u.alive).expect("a unit");
            let (y, n) = sim.ground_under_n(Vec3::new(0.0, 1.0, -1.0), false, 1.0);
            assert!(
                y > 0.0 && (n.y - deg.to_radians().cos()).abs() < 1e-3,
                "{deg}: {y} {n}"
            );
            let up = steep_ahead(&sim, u, Vec3::NEG_Z);
            assert_eq!(up.is_some(), blocked, "{deg} deg, {rules:?}");
            if let Some(w) = up {
                assert!(w.z > 0.99 && w.y == 0.0, "the wall faces back at him: {w}");
            }
            // Away from the face there is nothing to climb.
            assert_eq!(steep_ahead(&sim, u, Vec3::Z), None);
        }
    }

    /// Card SKIRT: BF2 Kamino's pod skirt - a band of 48 degrees (n.y 0.669,
    /// under both games' climb limits, over the 0.6 a soldier stands on) 1.2
    /// m high with a floor on top - is no wall: the soldier's run carries him
    /// over it (re-soldier-ground.md §7). Head on, and at a slant (a bot
    /// walking at a point on the skirt meets it 70 degrees off its slope;
    /// along its own way the band never ends). A band 2 m high and one of 55
    /// degrees (n.y 0.57: TAT1's ramp) stay walls.
    #[test]
    fn a_short_band_with_a_floor_on_top_is_no_wall_for_the_steering() {
        use crate::collision::{Body, CollisionWorld, Shape};
        use crate::soldier::GroundRules;
        let band = |deg: f32, rise: f32, rules: GroundRules, from_z: f32| {
            let (mut sim, h) = l_sim();
            sim.ground_rules = rules;
            // A slab tilted about X whose top face rises toward -Z from a low
            // edge half a metre ahead of a unit at the origin, `rise` high.
            let a = deg.to_radians();
            let len = rise / a.sin();
            let edge = -0.5 - len * a.cos();
            let mut bodies = vec![Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -0.5))
                    * glam::Mat4::from_rotation_x(a)
                    * glam::Mat4::from_translation(Vec3::new(0.0, -0.5, -len * 0.5)),
                Shape::Box {
                    half: Vec3::new(5.0, 0.5, len * 0.5),
                },
            )];
            // The floor on top: level, from the band's top edge on.
            bodies.push(Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, rise * 0.5, edge - 2.5)),
                Shape::Box {
                    half: Vec3::new(5.0, rise * 0.5, 2.5),
                },
            ));
            sim.collision = CollisionWorld::new(bodies, Vec::new());
            sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(0.0, 0.0, from_z);
            (sim, h)
        };
        let slant = Vec3::new(70f32.to_radians().sin(), 0.0, -70f32.to_radians().cos());
        // At the slant the unit stands 0.2 m off the band's foot, so the probe
        // comes down on the band; along its way the band rises 0.38 m a metre.
        for (deg, rise, rules, dir, open) in [
            (48.0, 1.2, GroundRules::Bf2, Vec3::NEG_Z, true),
            (48.0, 1.2, GroundRules::Bf2, slant, true),
            (48.0, 1.2, GroundRules::Bf1, slant, true),
            (48.0, 1.45, GroundRules::Bf2, Vec3::NEG_Z, true),
            (48.0, 1.2, GroundRules::Bf1, Vec3::NEG_Z, true),
            (48.0, 2.0, GroundRules::Bf2, Vec3::NEG_Z, false),
            (55.0, 1.2, GroundRules::Bf2, Vec3::NEG_Z, false),
        ] {
            let from_z = if dir == slant { -0.3 } else { 0.0 };
            let (sim, h) = band(deg, rise, rules, from_z);
            let u = sim.unit(h).expect("alive");
            let (y, n) = sim.ground_point_under_n(Vec3::new(0.0, 3.0, -1.0), 3.0);
            assert!(
                y > 0.3 && (n.y - deg.to_radians().cos()).abs() < 1e-3,
                "the probe's band: {y} {n}"
            );
            let what = format!("{deg} deg {rise} m {rules:?} {dir}");
            assert_eq!(steep_ahead(&sim, u, dir).is_none(), open, "{what}");
        }
    }

    /// Card OFFG: the level sweep meets a 32 degree ramp (BF2 Kamino's
    /// bridge ends) 0.6 m over the feet and called it a wall; the ramp test
    /// asks again from where the floor carries the capsule. A crate over the
    /// step is still a wall, and so is a wall standing on the ramp.
    #[test]
    fn a_ramp_is_no_wall_for_the_steering_and_a_crate_or_a_wall_on_it_still_is() {
        use crate::collision::{Body, CollisionWorld, Shape};
        // A slab whose top face rises toward -Z from a low edge at the feet
        // of a unit at the origin: the foot of the ramp, where the level
        // sweep's capsule ends its metre inside the slope.
        let ramp = |deg: f32| {
            Body::new(
                glam::Mat4::from_rotation_x(deg.to_radians())
                    * glam::Mat4::from_translation(Vec3::new(0.0, -0.5, -5.0)),
                Shape::Box {
                    half: Vec3::new(5.0, 0.5, 5.0),
                },
            )
        };
        let block = |centre: Vec3, half: Vec3| {
            Body::new(glam::Mat4::from_translation(centre), Shape::Box { half })
        };
        let pick = |bodies: Vec<Body>| {
            let (mut sim, h) = l_sim();
            sim.collision = CollisionWorld::new(bodies, Vec::new());
            sim.unit_mut(h).expect("alive").soldier.pos = Vec3::ZERO;
            let u = sim.unit(h).expect("alive");
            let (mut slide, mut nt) = (0, NavTick::default());
            steer_dir(&sim, u, Vec3::NEG_Z, FLAT_LOW, &mut slide, 1, &mut nt).1
        };
        assert_eq!(pick(vec![ramp(32.0)]), SteerPick::Fwd, "up the ramp");
        assert_eq!(pick(vec![ramp(20.0)]), SteerPick::Fwd, "a shallow one");
        // A 0.75 m crate, its face 0.9 m ahead: a step, not a ramp.
        let crate_ = block(Vec3::new(0.0, 0.375, -1.9), Vec3::new(1.0, 0.375, 1.0));
        assert_ne!(pick(vec![crate_]), SteerPick::Fwd, "a crate over the step");
        // A wall on the ramp, its face 1.2 m ahead.
        let wall = block(Vec3::new(0.0, 2.0, -1.45), Vec3::new(5.0, 2.0, 0.25));
        assert_ne!(
            pick(vec![ramp(32.0), wall]),
            SteerPick::Fwd,
            "a wall on the ramp"
        );
        // A 0.9 m rail 0.2 m thick on the ramp, 0.55 m ahead: thinner than
        // the floor samples are apart.
        let rail = block(Vec3::new(0.0, 0.5, -1.15), Vec3::new(5.0, 0.85, 0.1));
        assert_ne!(
            pick(vec![ramp(27.0), rail]),
            SteerPick::Fwd,
            "a rail on the ramp"
        );
        // A face too steep to climb stays one (card STHOP).
        assert_ne!(pick(vec![ramp(55.0)]), SteerPick::Fwd, "55 degrees");
    }

    /// Card BNDS: a barrier with a hull falling away behind it is a wall for
    /// the steering — the unit does not walk out over the rim, it goes along
    /// it — and the same rim without the barrier is walked over. From the
    /// hull, the way back in is open either way.
    #[test]
    fn a_rim_barrier_is_a_wall_and_the_way_back_is_open() {
        use crate::ai::tests::rim_sim;
        let west = Vec3::NEG_X;
        for (barrier, blocked) in [(true, true), (false, false)] {
            let (mut sim, h) = rim_sim(barrier);
            sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(-8.6, 10.0, 0.0);
            let u = sim.unit(h).expect("alive");
            let mut slide = 0;
            let mut nt = NavTick::default();
            // Any floor down to the hull's foot is one to walk on here.
            let (m, pick) = steer_dir(&sim, u, west, 0.0, &mut slide, 1, &mut nt);
            assert_eq!(
                pick != SteerPick::Fwd,
                blocked,
                "barrier {barrier}: {pick:?}"
            );
            if blocked {
                assert!(m.x > -0.1, "it does not go on out: {m}");
            }
        }
        // On the hull, 6 m past the rim and 3 m under the deck: back east.
        let (mut sim, h) = rim_sim(true);
        let y = sim.ground_under(Vec3::new(-16.0, 12.0, 0.0), false);
        sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(-16.0, y, 0.0);
        let u = sim.unit(h).expect("alive");
        let (mut slide, mut nt) = (0, NavTick::default());
        let (_, pick) = steer_dir(&sim, u, Vec3::X, y - 0.4, &mut slide, 1, &mut nt);
        assert_eq!(pick, SteerPick::Fwd, "the barrier does not hold it out");
    }

    /// Card BOWL: the rail is a wall for a walk that meets it obliquely, and
    /// the slide along it stays on the deck. At 45 degrees the edge look
    /// lands 3 m out on the hull, 1.5 m under the deck — ON the level before
    /// the card, so the rail was no wall and the slide went over it.
    ///
    /// NOT covered (measured, `artifacts/lead_tmp/bowl/README.md`): a walk
    /// shallower than about 30 degrees looks at the hull's first metre, which
    /// is a kerb's drop, and steps over; the level test then calls the bot
    /// lost and it walks back (the recovery test). Looking across the barrier as
    /// well closed that and cost BF2 GEO1, MUS1 and DEA1 rounds and kills.
    #[test]
    fn a_rim_rail_holds_an_oblique_walk_and_the_slide_along_it() {
        use crate::ai::tests::rim_sim;
        let (mut sim, h) = rim_sim(true);
        // 45 degrees off the rail's own line, heading out over it.
        let (sin, cos) = 45f32.to_radians().sin_cos();
        let want = Vec3::new(-sin, 0.0, cos);
        sim.unit_mut(h).expect("alive").soldier.pos = Vec3::new(-8.0, 10.0, -8.0);
        let (mut slide, mut nt, mut held) = (0, NavTick::default(), false);
        // 10 m of walking where the steering says, at 6 m/s.
        for _ in 0..50 {
            let u = sim.unit(h).expect("alive");
            let (m, pick) = steer_dir(&sim, u, want, 0.0, &mut slide, 1, &mut nt);
            held |= pick != SteerPick::Fwd;
            let p = &mut sim.unit_mut(h).expect("alive").soldier.pos;
            *p += m * (6.0 / 30.0);
            let p = *p;
            assert!(p.x > -9.5, "over the rail at {p}");
            assert_eq!(sim.rim_drop(p), None, "beyond the rim at {p}");
        }
        assert!(held, "the rail turned it");
        let p = sim.unit(h).expect("alive").soldier.pos;
        assert!(p.z > 0.0, "and it went on along the rail: {p}");
    }

    /// A jammed unit re-routes and side-steps after its progress window.
    /// Phase A.4: the stuck window hands its side-step side to the
    /// WALL-FOLLOW too. `steer_dir` reads the side-step's `side` only to break
    /// a tie on a unit with no remembered slide, so before this the PS2's
    /// "side-step to the side we did not use last time" could never change
    /// the side of a unit that had been sliding for a whole window.
    #[test]
    fn no_progress_arms_the_sidestep_and_resets_the_slide_side() {
        let (mut sim, h) = l_sim();
        // A jam: four walls round the unit, the way round them a circle.
        sim.collision = closed_room();
        // (Card OBST: a bot going along the slab is not stuck while it moves
        // on, for `avoid::ROUND_MAX` ticks; then the window fires.)
        let mut armed = false;
        for _ in 0..700 {
            sim.step(&[PadState::default()]);
            if sim
                .unit(h)
                .and_then(|u| u.ai.route())
                .is_some_and(|r| r.sidestep != 0)
            {
                armed = true;
                break;
            }
        }
        assert!(armed, "the stuck window never fired");
        let u = sim.unit(h).expect("alive");
        let r = u.ai.route().expect("route");
        assert_ne!(u.ai.slide, 0, "a side is set for the next pick");
        assert_eq!(
            u.ai.slide,
            r.sidestep.signum(),
            "the side-step's side won outright ({:?})",
            u.ai.steer
        );
        assert!(r.stuck_run >= STUCK_WINDOW, "stuck run {}", r.stuck_run);
        assert!(sim.nav_stats.stuck_max >= STUCK_WINDOW);
    }

    /// Four 12 m walls round the origin, 5 m out: no way out.
    fn closed_room() -> crate::collision::CollisionWorld {
        use crate::collision::{Body, CollisionWorld, Shape};
        let slab =
            |c: Vec3, half: Vec3| Body::new(glam::Mat4::from_translation(c), Shape::Box { half });
        CollisionWorld::new(
            vec![
                slab(Vec3::new(0.0, 0.0, -5.5), Vec3::new(6.0, 6.0, 0.5)),
                slab(Vec3::new(0.0, 0.0, 5.5), Vec3::new(6.0, 6.0, 0.5)),
                slab(Vec3::new(-5.5, 0.0, 0.0), Vec3::new(0.5, 6.0, 6.0)),
                slab(Vec3::new(5.5, 0.0, 0.0), Vec3::new(0.5, 6.0, 6.0)),
            ],
            Vec::new(),
        )
    }

    /// The soldier frame the pad is built in, spelled out — the AI now turns a
    /// WORLD direction into `(ly, lx)` and getting the sign wrong would send
    /// every unit the wrong way round every wall. `Soldier::step` is the
    /// authority: `fwd_axis = (-sin yaw, -cos yaw)`, `right_axis = (cos yaw,
    /// -sin yaw)`, so facing -Z (yaw 0) `ly = +1` walks -Z and `lx = +1`
    /// walks +X.
    #[test]
    fn the_strafe_axis_is_plus_x_for_a_unit_facing_minus_z() {
        use crate::soldier::{Soldier, SoldierConstants};
        let c = SoldierConstants::default();
        let run = |lx: i16, ly: i16| {
            let mut s = Soldier::new(Vec3::ZERO);
            assert_eq!(s.yaw, 0.0, "Soldier::new faces -Z");
            let pad = PadState {
                lx,
                ly,
                ..PadState::default()
            };
            for _ in 0..60 {
                let rules = crate::soldier::GroundRules::Bf2;
                s.step(crate::TICK_DT, &pad, &c, rules);
                s.resolve_ground(crate::TICK_DT, 0.0, Vec3::Y, 0.0, rules);
            }
            s.pos
        };
        let right = run(32767, 0);
        assert!(
            right.x > 1.0 && right.z.abs() < 1e-3,
            "lx +1 is +X: {right}"
        );
        let fwd = run(0, 32767);
        assert!(fwd.z < -1.0 && fwd.x.abs() < 1e-3, "ly +1 is -Z: {fwd}");

        // ... and that is exactly the frame `think` builds its pad in.
        let (sy, cy) = fmath::sin_cos(0.0f32);
        let fwd_axis = Vec3::new(-sy, 0.0, -cy);
        let right_axis = Vec3::new(cy, 0.0, -sy);
        assert_eq!(fwd_axis, Vec3::NEG_Z);
        assert_eq!(right_axis, Vec3::X);
    }

    /// A unit whose waypoint is 90 degrees off its facing WALKS AT IT on the
    /// first tick instead of turning in place: the pad is the world direction
    /// resolved in the soldier frame, so it strafes while the yaw catches up.
    /// Before Phase A.1 the `ADVANCE_CONE` gate held `ly` at 0 until the turn
    /// finished.
    #[test]
    fn a_unit_walks_sideways_while_its_yaw_catches_up() {
        let (mut sim, h) = l_sim();
        // Face +X (yaw -90 deg): the route's first waypoint is at -Z.
        sim.unit_mut(h).unwrap().soldier.yaw = -std::f32::consts::FRAC_PI_2;
        let mut st = AiState::default();
        let (pad, _) = think(&sim, h, &mut st);
        assert_eq!(st.steer, SteerPick::Fwd, "nothing in the way");
        // Facing +X, walking -Z is a LEFT strafe: lx = -1, ly = 0.
        assert!(pad.lx < -30000, "strafes left: {pad:?}");
        assert!(pad.ly.abs() < 2000, "and barely forward: {pad:?}");
        // It also actually moves that way.
        let before = sim.unit(h).unwrap().soldier.pos;
        for _ in 0..10 {
            sim.step(&[PadState::default()]);
        }
        let after = sim.unit(h).unwrap().soldier.pos;
        assert!(after.z < before.z - 0.5, "moved -Z: {before} -> {after}");
    }

    /// The wall in front picks the slide, and the slide side is REMEMBERED
    /// while the way ahead stays blocked — recomputing it every tick flips it
    /// near head-on and the unit runs on the spot.
    #[test]
    fn a_blocked_probe_slides_and_keeps_its_side() {
        use crate::collision::{Body, CollisionWorld, Shape};
        let (mut sim, h) = l_sim();
        // (Head-on: the leg's point is the next hub's centre, dead ahead —
        // a hub of radius 0 has no other, card HUBP.)
        if let Some(g) = sim.nav.as_mut() {
            g.hubs[1].radius = 0.0;
        }
        // A slab across the first corridor, 3 m ahead of the unit.
        sim.collision = CollisionWorld::new(
            vec![Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -3.0)),
                Shape::Box {
                    half: Vec3::new(30.0, 6.0, 0.5),
                },
            )],
            Vec::new(),
        );
        let mut st = AiState::default();
        // 3 m out the probe is still clear.
        let (_, _) = think(&sim, h, &mut st);
        assert_eq!(st.steer, SteerPick::Fwd);
        assert_eq!(st.slide, 0, "nothing to go round yet");
        // Walk up to it; once inside PROBE_DIST the pick is the slide.
        // Count every change of the remembered side along the way (a first
        // pick out of 0 is not a change, and a CLEAR probe's reset to 0 is
        // not either — see the bound below).
        let mut picks = Vec::new();
        let mut flips = 0;
        let mut prev = 0i8;
        let count_flip = |s: i8, flips: &mut i32, prev: &mut i8| {
            if s != 0 && s != *prev {
                if *prev != 0 {
                    *flips += 1;
                }
                *prev = s;
            }
        };
        for _ in 0..120 {
            sim.step(&[PadState::default()]);
            let u = sim.unit(h).unwrap();
            picks.push(u.ai.steer);
            count_flip(u.ai.slide, &mut flips, &mut prev);
        }
        assert!(picks.contains(&SteerPick::Fwd), "walked in: {picks:?}");
        let u = sim.unit(h).unwrap();
        assert_ne!(u.ai.steer, SteerPick::Fwd, "the slab is in the way");
        let side = u.ai.slide;
        assert_ne!(side, 0, "a side was chosen");
        // Card OBST: the side is a commitment. Sliding along a 60 m slab
        // never closes on the waypoint, and the progress window does not
        // hand over the other side while the bot moves on.
        assert!(
            u.soldier.pos.x.abs() > 2.0,
            "slid along the wall: {}",
            u.soldier.pos.x
        );
        for _ in 0..60 {
            sim.step(&[PadState::default()]);
            let u = sim.unit(h).unwrap();
            count_flip(u.ai.slide, &mut flips, &mut prev);
        }
        assert_eq!(flips, 0, "the side changed: alternating, not going round");
    }

    /// A pocket for the tangent tests: a wall across the front 1.0 m from the
    /// unit's capsule, and optionally one closing each side at the same
    /// distance. Everything is 12 m tall, so the probe capsule (which skips
    /// the bottom `STEP_HEIGHT`) sees a wall and not a kerb.
    fn pocket(minus_x: bool, plus_x: bool) -> crate::collision::CollisionWorld {
        use crate::collision::{Body, CollisionWorld, Shape};
        let slab =
            |c: Vec3, half: Vec3| Body::new(glam::Mat4::from_translation(c), Shape::Box { half });
        let mut bodies = vec![slab(Vec3::new(0.0, 0.0, -1.5), Vec3::new(30.0, 6.0, 0.5))];
        if minus_x {
            bodies.push(slab(Vec3::new(-1.5, 0.0, 0.0), Vec3::new(0.5, 6.0, 1.0)));
        }
        if plus_x {
            bodies.push(slab(Vec3::new(1.5, 0.0, 0.0), Vec3::new(0.5, 6.0, 1.0)));
        }
        CollisionWorld::new(bodies, Vec::new())
    }

    /// Card OBST (rule 2; Phase A.4 flipped here): when the way along the
    /// wall is shut by ANOTHER wall, the steering turns along that one on
    /// the same hand — the side is kept, with no memory yet and from a
    /// remembered side. (A side whose way is a drop IS a dead end and flips:
    /// `a_step_off_a_platform_is_refused`.) Before A.4 a closed side fell
    /// straight through to the fan and the next tick re-picked the same dead
    /// end, the 0.8 m oscillation TAT2 unit 18 and BF2 MUS1 unit 1 were in
    /// for 50,000+ ticks.
    #[test]
    fn a_shut_slide_turns_along_the_wall_that_shuts_it() {
        let (mut sim, h) = l_sim();
        let mut nt = NavTick::default();

        // 1. The front wall alone: which side does the steering choose?
        sim.collision = pocket(false, false);
        let mut st = AiState::default();
        let (m0, pick0) = {
            let u = sim.unit(h).expect("alive");
            steer_dir(&sim, u, Vec3::NEG_Z, FLAT_LOW, &mut st.slide, 1, &mut nt)
        };
        assert_eq!(pick0, SteerPick::Slide, "blocked ahead, open along");
        let s0 = st.slide;
        assert_ne!(s0, 0, "a side was chosen");
        assert!(
            m0.x.abs() > 0.9 && m0.z.abs() < 0.1,
            "the tangent is along the wall: {m0}"
        );

        // 2. Close THAT side at 1 m: along the wall that closes it, away
        //    from the front wall, on the same side.
        sim.collision = pocket(m0.x < 0.0, m0.x > 0.0);
        for slide in [0, s0] {
            let mut st = AiState {
                slide,
                ..AiState::default()
            };
            let (m1, pick1) = {
                let u = sim.unit(h).expect("alive");
                steer_dir(&sim, u, Vec3::NEG_Z, FLAT_LOW, &mut st.slide, s0, &mut nt)
            };
            assert_eq!(pick1, SteerPick::Slide);
            assert_eq!(st.slide, s0, "the side is kept");
            assert!(m1.z > 0.9, "along the side wall: {m0} -> {m1}");
        }
    }

    /// Card STFAL: a unit on a 5 m platform does not step off it. The floor
    /// probe refuses the step over the edge, the slide that would carry it
    /// along the wall and over the edge takes the other side, and with every
    /// way off the platform shut but the drop it stands.
    #[test]
    fn a_step_off_a_platform_is_refused() {
        use crate::collision::{Body, CollisionWorld, Shape};
        let (mut sim, h) = l_sim();
        // A platform, top at y 5, x in (-10, 0.8), z in (-10, 10); the
        // ground is the fixture's flat terrain at y 0.
        let platform = Body::new(
            glam::Mat4::from_translation(Vec3::new(-4.6, 2.5, 0.0)),
            Shape::Box {
                half: Vec3::new(5.4, 2.5, 10.0),
            },
        );
        sim.collision = CollisionWorld::new(vec![platform], Vec::new());
        sim.unit_mut(h).unwrap().soldier.pos = Vec3::new(0.0, 5.0, 0.0);
        let low = 5.0 - super::super::LEDGE_DROP;
        let u = sim.unit(h).expect("alive");
        assert!(!floor_ahead(&sim, u, Vec3::X, low), "+X is over the edge");
        assert!(floor_ahead(&sim, u, Vec3::NEG_X, low), "-X is the platform");
        assert!(floor_ahead(&sim, u, Vec3::NEG_Z, low), "-Z runs along it");

        // Straight at the edge: the steering goes round it along the edge.
        let mut nt = NavTick::default();
        let mut slide = 0;
        let (m, pick) = steer_dir(&sim, u, Vec3::X, low, &mut slide, 1, &mut nt);
        assert_ne!(pick, SteerPick::Fwd, "the drop is refused");
        assert!(m.x <= 1e-3, "and nothing it picks leaves the platform: {m}");

        // A wall across -Z whose remembered tangent (+X) runs off the edge:
        // the slide flips to -X.
        let wall = Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 6.0, -1.5)),
            Shape::Box {
                half: Vec3::new(30.0, 6.0, 0.5),
            },
        );
        sim.collision.push_body(wall);
        let u = sim.unit(h).expect("alive");
        let mut slide = 1;
        let (m, pick) = steer_dir(&sim, u, Vec3::NEG_Z, low, &mut slide, 1, &mut nt);
        assert_eq!(pick, SteerPick::Slide);
        assert!(m.x < -0.9, "slides the platform's way: {m}");
    }

    /// Card OBST: in a U-shaped pocket — the front and both sides shut — the
    /// way along the front wall meets a side wall, and the bot turns along
    /// THAT wall on the same hand: back out of the pocket.
    #[test]
    fn a_pocket_is_left_along_its_side_wall() {
        let (mut sim, h) = l_sim();
        sim.collision = pocket(true, true);
        let mut nt = NavTick::default();
        let mut st = AiState::default();
        let u = sim.unit(h).expect("alive");
        let (m, pick) = steer_dir(&sim, u, Vec3::NEG_Z, FLAT_LOW, &mut st.slide, 1, &mut nt);
        assert_eq!(pick, SteerPick::Slide);
        assert!(m.z > 0.9, "out of the pocket, along its side: {m}");
    }
}
