//! BF2 close mode 7 (PS2 0x2dac80), through ordinary movement and fire input.

use super::draw::{unit_rand, unit_unit};
use crate::fmath::FloatExt;
use crate::{
    sim::Sim,
    soldier::Stance,
    unit::{Unit, UnitHandle},
    TICK_HZ,
};
use glam::{Vec2, Vec3};

const JUMP_DIRECTION: u32 = 0x6d_7a_01;
const JUMP_DELAY: u32 = 0x6d_7a_02;
const PROBE: f32 = 25.0;
pub(super) const AIM_TOLERANCE: f32 = std::f32::consts::FRAC_PI_4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeleeBrain {
    pub next_jump: u64,
    pub jump_tick: Option<u64>,
    pub jump_goal: Option<Vec3>,
}

impl MeleeBrain {
    pub(super) fn begin(unit: UnitHandle, tick: u64) -> Self {
        // PS2 close-mode entry 0x2dac28 delays its first jump by 0..3s.
        Self {
            next_jump: tick
                + (3.0 * TICK_HZ as f32 * unit_unit(unit.index, tick, unit.gen, JUMP_DELAY ^ 1))
                    as u64,
            ..Self::default()
        }
    }
    pub(super) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"MELEEAI");
        eat(&self.next_jump.to_le_bytes());
        eat(&self.jump_tick.unwrap_or(u64::MAX).to_le_bytes());
        match self.jump_goal {
            Some(p) => {
                eat(&[1]);
                for n in p.to_array() {
                    eat(&n.hash_bits().to_le_bytes());
                }
            }
            None => eat(&[0]),
        }
    }
}

/// Choice zero really swaps x/z: verified MIPS 0x2daf6c..0x2daf84.
fn jump_direction(toward: Vec3, near: bool, choice: u32) -> Vec3 {
    if !near {
        return toward;
    }
    match choice % 3 {
        0 => Vec3::new(toward.z, 0.0, toward.x),
        1 => Vec3::new(toward.z, 0.0, -toward.x),
        _ => Vec3::new(-toward.z, 0.0, toward.x),
    }
}

/// [ours] The port's body/floor/barrier checks conservatively implement the
/// PS2 navigation probe. This is a heading test, never a 25m displacement.
fn jump_clear(sim: &Sim, u: &Unit, direction: Vec3) -> bool {
    let from = u.soldier.pos;
    let end = from + direction * PROBE;
    if sim
        .collision
        .wall_sweep(&u.capsule().stepped(), direction, PROBE)
        .is_some()
        || sim.nav.as_ref().is_some_and(|g| {
            g.barrier_stop(
                Vec2::new(from.x, from.z),
                Vec2::new(end.x, end.z),
                crate::nav::SOLDIER,
            ) < 1.0
        })
    {
        return false;
    }
    let mut y = from.y;
    for step in 1..=PROBE as u32 {
        let p = from + direction * step as f32;
        let floor = sim.ground_point_under(Vec3::new(p.x, y + super::PROBE_DIST, p.z), false);
        if floor < y - super::LEDGE_DROP || !floor.is_finite() {
            return false;
        }
        y = floor;
    }
    true
}

/// A perceived, seen target has already passed the normal behaviour chain.
/// The mover follows this point and alone writes the actual jump button.
pub(super) fn goal(
    sim: &Sim,
    me: UnitHandle,
    u: &Unit,
    target: &Unit,
    state: &mut MeleeBrain,
) -> Vec3 {
    if u.soldier.grounded && state.jump_tick.is_some_and(|t| sim.tick > t) {
        state.jump_goal = None;
    }
    if let Some(goal) = state.jump_goal {
        return goal;
    }
    let delta = target.soldier.pos - u.soldier.pos;
    let direction = Vec3::new(delta.x, 0.0, delta.z).normalize_or_zero();
    if direction == Vec3::ZERO
        || !u.soldier.grounded
        || u.soldier.look_frozen()
        || !matches!(u.soldier.stance, Stance::Stand | Stance::Crouch)
        || sim.tick <= state.next_jump
    {
        return target.soldier.pos;
    }
    let direction = jump_direction(
        direction,
        delta.x * delta.x + delta.z * delta.z <= 25.0,
        unit_rand(me.index, sim.tick, me.gen, JUMP_DIRECTION),
    );
    if jump_clear(sim, u, direction) {
        state.jump_tick = Some(sim.tick);
        state.jump_goal = Some(u.soldier.pos + direction * PROBE);
        let delay = 5.0 + 5.0 * unit_unit(me.index, sim.tick, me.gen, JUMP_DELAY);
        state.next_jump = sim.tick + (delay * TICK_HZ as f32) as u64;
        state.jump_goal.unwrap()
    } else {
        state.next_jump = sim.tick + 4 * u64::from(TICK_HZ);
        target.soldier.pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn melee_close_jump_uses_the_literal_ps2_third_direction() {
        let toward = Vec3::new(0.6, 0.0, 0.8);
        assert_eq!(jump_direction(toward, true, 0), Vec3::new(0.8, 0.0, 0.6));
        assert_eq!(jump_direction(toward, true, 1), Vec3::new(0.8, 0.0, -0.6));
        assert_eq!(jump_direction(toward, true, 2), Vec3::new(-0.8, 0.0, 0.6));
        assert_eq!(jump_direction(toward, false, 0), toward);
    }

    #[test]
    fn melee_jump_probe_never_moves_the_soldier_or_grants_energy() {
        let mut sim = Sim::empty(1);
        sim.heightfield = Some(crate::heightfield::Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        });
        sim.tick = 1;
        let me = sim
            .spawn(&crate::unit::UnitSpec::trooper(0, Vec3::ZERO))
            .unwrap();
        let target = sim
            .spawn(&crate::unit::UnitSpec::trooper(1, Vec3::NEG_Z * 10.0))
            .unwrap();
        let u = sim.unit(me).unwrap();
        let before = (u.soldier.pos, u.soldier.energy);
        let mut state = MeleeBrain::default();
        let _ = goal(&sim, me, u, sim.unit(target).unwrap(), &mut state);
        assert_eq!((u.soldier.pos, u.soldier.energy), before);
        assert!(state.next_jump > sim.tick);
        assert_eq!(state.jump_goal, Some(Vec3::NEG_Z * 25.0));
        assert_eq!(state.jump_tick, Some(sim.tick));

        sim.collision = crate::collision::CollisionWorld::new(
            vec![crate::collision::Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 1.0, -5.0)),
                crate::collision::Shape::Box {
                    half: Vec3::new(5.0, 2.0, 0.5),
                },
            )],
            vec![],
        );
        let mut blocked = MeleeBrain::default();
        let u = sim.unit(me).unwrap();
        goal(&sim, me, u, sim.unit(target).unwrap(), &mut blocked);
        assert_eq!(blocked.jump_goal, None, "a wall rejects the heading");
        assert_eq!(blocked.next_jump, sim.tick + 4 * u64::from(TICK_HZ));
        assert_eq!((u.soldier.pos, u.soldier.energy), before);

        sim.collision = Default::default();
        sim.heightfield = Some(crate::heightfield::Heightfield {
            size: 31,
            unit: 1.0,
            offset_x: 15.0,
            offset_z: 25.0,
            heights: (0..31 * 31)
                .map(|i| if i / 31 < 25 { -10.0 } else { 0.0 })
                .collect(),
        });
        let mut ledge = MeleeBrain::default();
        let u = sim.unit(me).unwrap();
        goal(&sim, me, u, sim.unit(target).unwrap(), &mut ledge);
        assert_eq!(ledge.jump_goal, None, "a floor drop rejects the heading");
        assert_eq!(ledge.next_jump, sim.tick + 4 * u64::from(TICK_HZ));
        assert_eq!((u.soldier.pos, u.soldier.energy), before);
    }
}
