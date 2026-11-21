//! Issue #182 B2, the AXIS CLAMP: a soldier's tick of movement may not carry
//! a point of his axis through a SOLDIER-layer face.
//!
//! The wall pass (`resolve_soldier_walls`) resolves the capsule against the
//! walls in sub-steps, and B1 holds a soldier low where he has no headroom.
//! The leak census still finds runs whose axis passes a wall between two
//! ticks: a thin wall a roll or a sprint jump carries him through (`push`,
//! `tunnel`). After the wall pass, the LEVEL part of the tick's move is cast
//! from where it started, at the census's axis heights ([`AXIS_HEIGHTS`],
//! those both stances' capsules reach). When one meets a face, the level
//! move is undone: he keeps the tick's height change but stays at its start
//! in x and z, and loses the velocity into the face. The floor contact then
//! resolves there.
//!
//! Any face counts, not only walls: a SLOPED face the level cast meets is a
//! floor at least 0.8 m over his feet, or a ceiling at his head, and the wall
//! pass (which only resolves faces steeper than the walkable limit) lets the
//! capsule walk into it. BF2 Kashyyyk's `kas2_prop_rock_m16` has a face at
//! n.y 0.62 whose lip stands over the terrain higher than a step; a soldier
//! walking along the rock went under the lip and into the closed rock
//! (census `floor`). A level ray cannot meet a flat floor or ceiling, and
//! a soldier walking up a walkable slope keeps his axis over it (at the
//! steepest walkable slope, 60 deg, the 0.8 m point meets it 0.46 m ahead;
//! a tick's run is under 0.35 m).
//!
//! A FALL is never clamped: a first version undid the whole move on any
//! face and hung 300-odd census runs in the air (a fall whose every tick met
//! a ledge's face was undone every tick). A soldier who ends up inside a
//! deck or under a floor is the rescue's (`sim/rescue.rs`).
//!
//! A RISE is ([`Sim::keep_head_under`]): after the floor contact, a tick
//! that carried an axis point UP through any face (a deck's underside, the
//! next turn of a spiral stair, the side of a box a step-up lifted a roll
//! into) is undone whole, back to the tick's start, and he loses his rise
//! and his run. The floor contact's
//! lift and a steep stair's launch both do that on BF2 Felucia's spiral
//! stair: the head went 0.06 m into the turn above in one tick, where the
//! ceiling sweep (which starts clear) then no longer saw it, and he came
//! out standing on the turn above (the census's `ceiling` class). Undone,
//! he stands at the foot of the low turn as at a wall. Only while he
//! started the tick with his axis outside the bodies, as the level clamp.
//!
//! - Only a soldier whose axis started the tick outside the bodies (his
//!   stepped capsule in none deeper than [`AXIS_CLEAR`] of its radius,
//!   [`Sim::axis_clear`]): one already in a wall must still be free to
//!   leave it, and the faces' winding does not tell their side (PS2
//!   collision faces have none), so the clamp cannot tell entering from
//!   leaving by the face.
//! - Static bodies only; vehicle hulls move and have their own pass.
//! - Not on a flyer's deck (the deck's carry is in the move) and not a
//!   corpse.
//! - The floor snap after the wall pass (a step up, B3's lift out from
//!   under the terrain) is not part of the cast move.
//!
//! OURS: the PS2's soldier collision keeps its own contacts and does not
//! show this rule; ours exists because our capsule pass leaks where the
//! census shows it. PROVISIONAL; measure: the census's crossing classes and
//! the per-tick cost (`scripts/perf_matrix.py`).

use super::Sim;
use crate::collision::{Capsule, HullFilter, Layers, WALKABLE_MIN_NY};
use crate::fmath::FloatExt;
use crate::unit::capsule_height;
use glam::Vec3;

/// The axis heights over the feet the clamp casts at (m), the leak
/// census's own; each only where the tick's end capsule reaches it with
/// 0.15 m to spare (the rise clamp: where both do).
pub const AXIS_HEIGHTS: [f32; 3] = [0.8, 1.25, 1.7];

/// PROVISIONAL (ours): how deep (a share of the radius) a body may hold the
/// tick's start capsule for the clamps still to hold his axis out of it.
/// Deeper, his axis may already be past the face (a sheet he straddles, a
/// solid he is in), and he must be free to leave.
pub const AXIS_CLEAR: f32 = 0.95;

impl Sim {
    /// The module header's clamp for unit `i`, whose move started in `from`.
    pub(super) fn keep_axis_side(&mut self, i: usize, from: &Capsule) {
        let u = &self.units[i];
        if !u.alive || u.deck.is_some() || u.mount.is_some() || u.droideka.is_some() {
            return;
        }
        let to = u.soldier.pos;
        // The level part of the move, cast at the start's height.
        let d = (to - from.base).with_y(0.0);
        let len = d.length();
        if len < 1e-5 {
            return;
        }
        let dir = d / len;
        // The heights the tick's END capsule reaches: a roll or a prone
        // that stands up this tick is cast at his new chest and head too
        // (BF2 Felucia's census: 1,600 of its 1,650 `push` runs were a
        // sprint roll ending in a stand whose wall pass pushed the new,
        // taller capsule through a bush's face, at heights the prone
        // capsule of the tick's start never reached).
        let h = capsule_height(u.soldier.stance);
        let hit = clamp_heights(h).find_map(|a| {
            self.collision.ray_with(
                from.base + Vec3::Y * a,
                dir,
                len,
                Layers::SOLDIER,
                HullFilter::Static,
            )
        });
        // A DESCENT: the whole move, cast, meeting a floor or a slope (no
        // wall) on the way down - a jump or a fall landing on a steep slope
        // whose face his axis would pass under before the floor contact
        // finds it, or a fall past a deck's rim that would carry his body
        // under the deck. The level part is undone: he keeps falling,
        // straight down at the tick's start (BF2 Felucia's census: 100-odd
        // jumps and jets that ended under a rock's sloped face; all maps:
        // 900 falls under a flat deck). A landing so fast that his axis
        // reaches the floor he lands on loses that tick's run.
        let hit = hit.or_else(|| {
            let full = to - from.base;
            if full.y >= 0.0 {
                return None;
            }
            let flen = full.length();
            clamp_heights(h)
                .find_map(|a| {
                    self.collision.ray_with(
                        from.base + Vec3::Y * a,
                        full / flen,
                        flen,
                        Layers::SOLDIER,
                        HullFilter::Static,
                    )
                })
                .filter(|hit| hit.normal.y.abs() >= WALKABLE_MIN_NY)
        });
        let Some(hit) = hit else {
            return;
        };
        if !self.axis_clear(from) {
            return;
        }
        // The face's normal, level and turned against the move: the side
        // he came from (none for a face too flat to have a level part).
        let n = hit.normal.with_y(0.0).normalize_or_zero();
        let n = if n.dot(dir) > 0.0 { -n } else { n };
        let u = &mut self.units[i];
        u.soldier.pos.x = from.base.x;
        u.soldier.pos.z = from.base.z;
        u.soldier.vel -= n * u.soldier.vel.dot(n).fmin(0.0);
    }
}

/// The heights a clamp casts at for a capsule `h` m tall: its middle (the
/// one a prone or a crouch has: a prone soldier the floor contact lifted
/// 0.56 m into a bush's box on BF2 Felucia passed every axis height, none of
/// which a 0.55 m capsule reaches), then [`AXIS_HEIGHTS`] it reaches with
/// 0.15 m to spare.
fn clamp_heights(h: f32) -> impl Iterator<Item = f32> {
    std::iter::once(h * 0.5).chain(AXIS_HEIGHTS.into_iter().filter(move |a| *a < h - 0.15))
}

impl Sim {
    /// Is the axis of the capsule `from` outside every body, so a face its
    /// tick's move meets is one he would go INTO? No body may hold the
    /// stepped capsule deeper than [`AXIS_CLEAR`] of its radius: shallower,
    /// the deepest contact is his skin in a face whose far side his axis
    /// has not reached (BF2 Felucia's census: 3,580 of 3,700 crossings
    /// started 0.08-0.33 m deep in the face they then crossed, where the
    /// wall pass's slow push out of a lip or a slanted ceiling had left
    /// him; the old gate, clear of every body, let all of them through).
    fn axis_clear(&self, from: &Capsule) -> bool {
        self.collision
            .capsule_overlap(&from.stepped())
            .is_none_or(|(d, _)| d <= from.radius * AXIS_CLEAR)
    }

    /// The module header's rise clamp for unit `i`, whose move started in
    /// `from`: after the floor contact.
    pub(super) fn keep_head_under(&mut self, i: usize, from: &Capsule) {
        let u = &self.units[i];
        if !u.alive || u.deck.is_some() || u.mount.is_some() || u.droideka.is_some() {
            return;
        }
        let to = u.soldier.pos;
        let d = to - from.base;
        if d.y <= 1e-4 {
            return;
        }
        let len = d.length();
        let dir = d / len;
        let h = from.height.fmin(capsule_height(u.soldier.stance));
        let hit = clamp_heights(h).find_map(|a| {
            self.collision.ray_with(
                from.base + Vec3::Y * a,
                dir,
                len,
                Layers::SOLDIER,
                HullFilter::Static,
            )
        });
        if hit.is_none() || !self.axis_clear(from) {
            return;
        }
        let u = &mut self.units[i];
        u.soldier.pos = from.base;
        u.soldier.vel = Vec3::new(0.0, u.soldier.vel.y.fmin(0.0), 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::heightfield::Heightfield;
    use crate::unit::UnitSpec;
    use glam::Mat4;

    fn sim_with(bodies: Vec<Body>) -> Sim {
        let mut s = Sim::empty(1);
        s.collision = CollisionWorld::new(bodies, Vec::new());
        s.heightfield = Some(Heightfield {
            size: 65,
            unit: 2.0,
            offset_x: 64.0,
            offset_z: 64.0,
            heights: vec![0.0; 65 * 65],
        });
        s
    }

    /// A thin wall slab 4 m tall, 4 cm thick, across x = 1.
    fn sheet() -> Body {
        Body::with_layers(
            Mat4::from_translation(Vec3::new(1.0, 2.0, 0.0)),
            Shape::Box {
                half: Vec3::new(0.02, 2.0, 4.0),
            },
            Layers::SOLDIER,
        )
    }

    /// A move whose axis would pass the sheet is undone, and its velocity
    /// into the sheet is gone; the along-wall part stays.
    #[test]
    fn a_move_through_a_thin_wall_is_undone() {
        let mut s = sim_with(vec![sheet()]);
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
        let i = h.index as usize;
        let from = s.units[i].capsule();
        s.units[i].soldier.pos = Vec3::new(1.6, 0.0, 0.3);
        s.units[i].soldier.vel = Vec3::new(9.0, 0.0, 1.0);
        s.keep_axis_side(i, &from);
        assert_eq!(s.units[i].soldier.pos, Vec3::ZERO);
        let v = s.units[i].soldier.vel;
        assert!(v.x.abs() < 1e-4 && (v.z - 1.0).abs() < 1e-4, "{v}");
    }

    /// A falling soldier whose move crosses a floor face keeps falling (the
    /// clamp is level-only and walls-only): no hanging in the air.
    #[test]
    fn a_fall_past_a_ledge_is_not_held() {
        let ledge = Body::with_layers(
            Mat4::from_translation(Vec3::new(0.0, 4.9, 0.0)),
            Shape::Box {
                half: Vec3::new(2.0, 0.1, 2.0),
            },
            Layers::SOLDIER,
        );
        let mut s = sim_with(vec![ledge]);
        let h = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(2.3, 5.0, 0.0)))
            .expect("spawned");
        let i = h.index as usize;
        let mut from = s.units[i].capsule();
        from.base = Vec3::new(2.3, 5.0, 0.0);
        s.units[i].soldier.pos = Vec3::new(2.2, 4.4, 0.0);
        s.keep_axis_side(i, &from);
        assert_eq!(s.units[i].soldier.pos, Vec3::new(2.2, 4.4, 0.0));
    }

    /// A level move into a sloped face (walkable, n.y 0.62) whose lip
    /// stands over his feet higher than a step is undone like a wall's:
    /// the BF2 Kashyyyk rock squeeze.
    #[test]
    fn a_move_under_a_sloped_lip_is_undone() {
        // A box tilted about z so its +x top face leans at n.y ~0.62,
        // its lower edge 1 m over the floor at x = 1.
        let tilt = glam::Quat::from_rotation_z(0.9);
        let lip = Body::with_layers(
            Mat4::from_rotation_translation(tilt, Vec3::new(2.2, 1.9, 0.0)),
            Shape::Box {
                half: Vec3::new(1.0, 1.0, 3.0),
            },
            Layers::SOLDIER,
        );
        let mut s = sim_with(vec![lip]);
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
        let i = h.index as usize;
        let from = s.units[i].capsule();
        assert!(s.clear_of_bodies(&from));
        s.units[i].soldier.pos = Vec3::new(1.5, 0.0, 0.0);
        s.units[i].soldier.vel = Vec3::new(6.0, 0.0, 0.0);
        s.keep_axis_side(i, &from);
        assert_eq!(s.units[i].soldier.pos, Vec3::ZERO);
        assert!(
            s.units[i].soldier.vel.x.abs() < 1e-3,
            "{}",
            s.units[i].soldier.vel
        );
    }

    /// A rise that carries his head up through a deck's underside is
    /// undone whole; the same rise with room over him, and a fall past the
    /// deck's edge, are kept.
    #[test]
    fn a_rise_through_a_deck_is_undone() {
        let deck = || {
            Body::with_layers(
                Mat4::from_translation(Vec3::new(0.0, 2.05, 0.0)),
                Shape::Box {
                    half: Vec3::new(2.0, 0.05, 2.0),
                },
                Layers::SOLDIER,
            )
        };
        let mut s = sim_with(vec![deck()]);
        let h = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.2, 0.0)))
            .expect("spawned");
        let i = h.index as usize;
        let mut from = s.units[i].capsule();
        from.base = Vec3::new(0.0, 0.0, 0.0);
        s.units[i].soldier.pos = Vec3::new(0.1, 0.4, 0.0);
        s.units[i].soldier.vel = Vec3::new(3.0, 6.0, 0.0);
        s.keep_head_under(i, &from);
        assert_eq!(s.units[i].soldier.pos, Vec3::ZERO);
        assert_eq!(s.units[i].soldier.vel, Vec3::ZERO);
        // Out from under the deck: kept.
        from.base = Vec3::new(3.0, 0.0, 0.0);
        s.units[i].soldier.pos = Vec3::new(3.1, 0.4, 0.0);
        s.keep_head_under(i, &from);
        assert_eq!(s.units[i].soldier.pos, Vec3::new(3.1, 0.4, 0.0));
    }

    /// A move that stays on its side, or one that starts inside the wall
    /// (he must be free to leave it), is left alone.
    #[test]
    fn a_clear_move_or_one_out_of_a_wall_is_kept() {
        let mut s = sim_with(vec![sheet()]);
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
        let i = h.index as usize;
        let from = s.units[i].capsule();
        s.units[i].soldier.pos = Vec3::new(0.4, 0.0, 0.5);
        s.keep_axis_side(i, &from);
        assert_eq!(s.units[i].soldier.pos, Vec3::new(0.4, 0.0, 0.5));
        let mut from_in = from;
        from_in.base = Vec3::new(1.0, 0.0, 0.0);
        s.units[i].soldier.pos = Vec3::new(1.6, 0.0, 0.0);
        s.keep_axis_side(i, &from_in);
        assert_eq!(s.units[i].soldier.pos, Vec3::new(1.6, 0.0, 0.0));
    }
}
