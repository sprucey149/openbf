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
const BLOCK_DELAY: u32 = 0x6d_7a_03;
const PROBE: f32 = 25.0;
pub(super) const AIM_TOLERANCE: f32 = std::f32::consts::FRAC_PI_4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeleeBrain {
    pub next_jump: u64,
    pub jump_tick: Option<u64>,
    pub jump_goal: Option<Vec3>,
    /// The combat block cycle ([`MeleeBrain::block`]): held now, and the
    /// tick it flips; 0 out of combat.
    pub block: bool,
    pub block_until: u64,
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
        eat(&[u8::from(self.block)]);
        eat(&self.block_until.to_le_bytes());
    }

    /// The PS2 combat helper's block cycle for a melee weapon in hand. Entry
    /// to combat (B2 `FUN_002ce648`) turns block on for a random 3-6 s; the
    /// update (`FUN_002cf458`) flips it when its time is up, on for 3-7 s,
    /// off for 1.5-4 s, and writes the weapon's AI block bit (+0x198 bit 15),
    /// which holds the combo's block button down; leaving combat
    /// (`FUN_002ceea8`) clears it. `combat`: the bot has a target, seen or
    /// remembered. The caller drops the hold on a tick it wants to swing:
    /// the single-trigger routine clears the bit then (`FUN_002baf70`).
    pub(super) fn block(&mut self, me: UnitHandle, tick: u64, combat: bool) -> bool {
        if !combat {
            self.block = false;
            self.block_until = 0;
            return false;
        }
        let until = |lo: f32, hi: f32| {
            let s = lo + (hi - lo) * unit_unit(me.index, tick, me.gen, BLOCK_DELAY);
            tick + (s * TICK_HZ as f32) as u64
        };
        if self.block_until == 0 {
            self.block = true;
            self.block_until = until(3.0, 6.0);
        } else if tick > self.block_until {
            self.block = !self.block;
            self.block_until = if self.block {
                until(3.0, 7.0)
            } else {
                until(1.5, 4.0)
            };
        }
        self.block
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
    point(sim, me, u, target.soldier.pos, state)
}

pub(super) fn point(
    sim: &Sim,
    me: UnitHandle,
    u: &Unit,
    target_pos: Vec3,
    state: &mut MeleeBrain,
) -> Vec3 {
    if u.soldier.grounded && state.jump_tick.is_some_and(|t| sim.tick > t) {
        state.jump_goal = None;
    }
    if let Some(goal) = state.jump_goal {
        return goal;
    }
    let delta = target_pos - u.soldier.pos;
    let direction = Vec3::new(delta.x, 0.0, delta.z).normalize_or_zero();
    if direction == Vec3::ZERO
        || !u.soldier.grounded
        || u.soldier.look_frozen()
        || !matches!(u.soldier.stance, Stance::Stand | Stance::Crouch)
        || sim.tick <= state.next_jump
    {
        return target_pos;
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
        target_pos
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

    /// The PS2 combat block cycle: on for 3-6 s at combat entry, then on
    /// 3-7 s / off 1.5-4 s, and off and re-armed out of combat.
    #[test]
    fn melee_block_cycle_follows_the_ps2_combat_helper() {
        let hz = u64::from(TICK_HZ);
        for gen in 1..40u32 {
            let me = UnitHandle { index: 3, gen };
            let mut m = MeleeBrain::default();
            assert!(m.block(me, 100, true), "combat entry blocks");
            let first = m.block_until - 100;
            assert!((3 * hz..=6 * hz).contains(&first), "entry {first}");
            let (mut tick, mut on, mut flips) = (100, true, 0);
            while flips < 6 {
                tick += 1;
                let now = m.block(me, tick, true);
                if now != on {
                    let span = m.block_until - tick;
                    let (lo, hi) = if now {
                        (3 * hz, 7 * hz)
                    } else {
                        (3 * hz / 2, 4 * hz)
                    };
                    assert!((lo..=hi).contains(&span), "span {span} on {now}");
                    on = now;
                    flips += 1;
                }
            }
            assert!(!m.block(me, tick + 1, false), "out of combat: no block");
            assert_eq!(m.block_until, 0);
            assert!(m.block(me, tick + 2, true), "a new fight blocks at once");
        }
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
