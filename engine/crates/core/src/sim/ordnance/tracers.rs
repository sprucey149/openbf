//! BF2's secondary ordnance (`analysis/re-ordnance-kinds.md` §7): a weapon's
//! `SecondaryOrdnanceName` / `SecondaryOrdnancePeriod` (B2:324171-324181) is a
//! round spawned beside the primary round every `period`-th round - the
//! shotgun's one per pellet, the chaingun's one every 5th shot. The round is a
//! real [`Bolt`] of its own class (its own speed, LifeSpan and
//! `LaserTexture`), so the drawn streak and the flight both come from the one
//! object; it flies the primary's ray (the PS2 spawns both from one packet, so
//! the port draws no extra LCG step).

use crate::collision::HullFilter;
use crate::projectile::{Bolt, Hit, Shooter};
use crate::weapon::Secondary;
use glam::Vec3;

use crate::sim::state::Sim;

impl Sim {
    /// Spawn the secondary round for round `round` (1-based) of the class at
    /// `stage`, on the primary's `origin` / `dir`. `false` when the class
    /// carries none or when the round is not its multiple.
    ///
    /// The bolt is the secondary class's own: its Velocity and LifeSpan, its
    /// damage and its laser look. A tracer's press ray stop is dropped - the
    /// round is a drawn line, so it flies its whole reach even through a wall
    /// the primary stopped on - and [`Sim::sweep_bolts`] skips its visual-only collision. Explosive
    /// secondaries still meet bodies even when their direct damage is zero.
    pub(in crate::sim) fn fire_secondary(
        &mut self,
        who: Shooter,
        stage: usize,
        round: u32,
        origin: Vec3,
        dir: Vec3,
        charge: f32,
    ) -> bool {
        let Some(sec) = self.weapon_classes[stage].secondary.as_ref() else {
            return false;
        };
        if !spawns_secondary(sec, round) {
            return false;
        }
        let ord = &sec.ordnance;
        let mut bolt = Bolt::fire(
            origin,
            dir,
            ord.velocity,
            ord.life_span,
            Shooter {
                weapon_class: sec.index,
                ..who
            },
            self.heightfield.as_ref(),
            self.water_plane(),
            &self.collision,
            HullFilter::Static,
        );
        bolt.charge = charge;
        if ord.is_tracer() && ord.explosion.is_none() {
            bolt.hit = Hit::None;
            bolt.remaining = ord.life_span;
            bolt.end = origin + dir * (ord.velocity * ord.life_span);
        }
        self.launch_round(bolt, false);
        true
    }
}

/// Is `sec` registered and is `round` its multiple? The whole spawn
/// rule in one place.
fn spawns_secondary(sec: &Secondary, round: u32) -> bool {
    sec.index != usize::MAX && sec.on_round(round)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::state::Event;
    use crate::sim::PadState;
    use crate::unit::{UnitHandle, UnitSpec};
    use crate::weapon::{OrdnanceClass, Secondary, ShotSpec, WeaponClass};

    /// A test gun: the BF1 rifle's bolt as the primary, a 0-damage 200 m/s
    /// 0.05 s tracer as its secondary (the disc tracers' shape).
    fn gun(period: u32, damage: f32) -> WeaponClass {
        let mut w = WeaponClass::bf1_rifle();
        w.name = format!("test_gun_{period}_{damage}");
        w.secondary = Some(Secondary {
            name: format!("test_tracer_{period}_{damage}"),
            ordnance: OrdnanceClass {
                velocity: 200.0,
                life_span: 0.05,
                max_damage: damage,
                ..OrdnanceClass::default()
            },
            period,
            index: usize::MAX,
        });
        w
    }

    fn scene(period: u32, damage: f32) -> (Sim, UnitHandle, usize, usize) {
        let mut s = Sim::empty(9);
        let gun = s.add_weapon_class(gun(period, damage));
        let spec = UnitSpec {
            weapon_class: gun,
            ..UnitSpec::player(0, Vec3::ZERO)
        };
        let h = s.spawn(&spec).unwrap();
        let tracer = s.weapon_classes[gun].secondary.as_ref().unwrap().index;
        (s, h, gun, tracer)
    }

    fn shot() -> ShotSpec {
        ShotSpec {
            tilt: (0.0, 0.0),
            charge: 1.0,
            stage: 0,
            pitch_spread: 0.0,
            yaw_spread: 0.0,
        }
    }

    #[test]
    fn period_one_rides_a_tracer_on_every_round() {
        let (mut s, h, gun, tracer) = scene(1, 0.0);
        // Registration: the tracer owns a class entry with its own data, and
        // the weapon's `Secondary` carries its index.
        assert_eq!(s.weapon_classes[tracer].name, "test_tracer_1_0");
        assert!(s.weapon_classes[tracer].ordnance.is_tracer());
        assert_eq!(
            s.weapon_classes[gun].secondary.as_ref().unwrap().index,
            tracer
        );
        s.soldier_shots(h.index as usize, None, &[shot()]);
        assert_eq!(
            s.bolts.iter().map(|b| b.weapon_class).collect::<Vec<_>>(),
            vec![gun, tracer]
        );
        s.soldier_shots(h.index as usize, None, &[shot()]);
        assert_eq!(
            s.bolts.iter().map(|b| b.weapon_class).collect::<Vec<_>>(),
            vec![gun, tracer, gun, tracer]
        );
        assert_eq!(s.unit(h).unwrap().weapon.secondary_rounds, 2);
    }

    #[test]
    fn period_five_rides_the_first_and_the_sixth_round() {
        let (mut s, h, gun, tracer) = scene(5, 0.0);
        s.soldier_shots(h.index as usize, None, &[shot(); 5]);
        assert_eq!(
            s.bolts.iter().map(|b| b.weapon_class).collect::<Vec<_>>(),
            vec![gun, tracer, gun, gun, gun, gun]
        );
        assert_eq!(s.unit(h).unwrap().weapon.secondary_rounds, 5);
        s.soldier_shots(h.index as usize, None, &[shot()]);
        assert_eq!(s.bolts.last().map(|b| b.weapon_class), Some(tracer));
        s.soldier_shots(h.index as usize, None, &[shot()]);
        assert_eq!(s.bolts.last().map(|b| b.weapon_class), Some(gun));
    }

    #[test]
    fn the_tracer_does_no_damage_and_dies_at_its_life_end() {
        let (mut s, h, _, tracer) = scene(1, 0.0);
        s.soldier_shots(h.index as usize, None, &[shot()]);
        assert_eq!(s.bolts[1].weapon_class, tracer);
        assert_eq!(
            s.weapon_classes[tracer]
                .ordnance
                .damage_desc(0.0, false)
                .damage,
            0.0
        );
        let mut expired = false;
        // Its LifeSpan is 0.05 s: two ticks at 30 Hz (the primary rifle's is
        // 1.5 s and still in the air).
        for _ in 0..4 {
            s.step(&[PadState::default()]);
            expired |= s
                .events
                .iter()
                .any(|e| matches!(e, Event::BoltExpired { .. }));
        }
        assert!(expired, "the tracer must end its flight in the air");
        assert!(s.bolts.iter().all(|b| b.weapon_class != tracer));
        assert!(s.bolts.iter().any(|b| b.weapon_class != tracer));
    }

    #[test]
    fn no_secondary_class_spawns_nothing_extra() {
        let mut s = Sim::new(11);
        s.soldier_shots(0, None, &[shot()]);
        assert_eq!(s.bolts.len(), 1);
        assert_eq!(s.bolts[0].weapon_class, 0);
        assert_eq!(s.units[0].weapon.secondary_rounds, 0);
        assert_eq!(s.weapon_classes.len(), 1);
    }

    #[test]
    fn a_fat_ray_second_round_rides_the_primary_with_its_charge() {
        let mut s = Sim::empty(9);
        let mut w = gun(1, 100.0);
        if let Some(sec) = w.secondary.as_mut() {
            sec.ordnance.fat_ray = Some(crate::weapon::FatRayClass::default());
        }
        let gun = s.add_weapon_class(w);
        let fat = s.weapon_classes[gun].secondary.as_ref().unwrap().index;
        assert_ne!(fat, usize::MAX);
        let h = s
            .spawn(&UnitSpec {
                weapon_class: gun,
                ..UnitSpec::player(0, Vec3::ZERO)
            })
            .unwrap();
        s.soldier_shots(
            h.index as usize,
            None,
            &[ShotSpec {
                charge: 0.5,
                ..shot()
            }],
        );
        assert_eq!(
            s.bolts.iter().map(|b| b.weapon_class).collect::<Vec<_>>(),
            vec![gun, fat]
        );
        assert_eq!(s.bolts[1].charge, 0.5);
    }

    #[test]
    fn a_real_second_round_flies_as_its_own_bolt() {
        let (mut s, h, gun, round) = scene(1, 100.0);
        assert_ne!(round, usize::MAX);
        assert!(!s.weapon_classes[round].ordnance.is_tracer());
        s.soldier_shots(h.index as usize, None, &[shot()]);
        assert_eq!(
            s.bolts.iter().map(|b| b.weapon_class).collect::<Vec<_>>(),
            vec![gun, round]
        );
        assert_eq!(s.bolts[1].remaining, s.bolts[0].remaining.min(0.05));
    }
    #[test]
    fn visual_tracer_passes_a_body_without_reporting_a_hit() {
        let (mut s, h, _, tracer) = scene(1, 0.0);
        let victim = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -3.0)))
            .unwrap();
        let origin = s.unit(h).unwrap().muzzle();
        let dir = (s.unit(victim).unwrap().aim_point() - origin).normalize();
        let round = Bolt::fire(
            origin,
            dir,
            200.0,
            0.05,
            Shooter {
                owner: h,
                team: 0,
                weapon_class: tracer,
                is_player: true,
            },
            None,
            None,
            &s.collision,
            HullFilter::All,
        );
        s.bolts.push(round);
        s.fly_bolts(crate::TICK_DT);
        assert_eq!(s.nav_stats.bolt_hits, 0);
        assert!(s.unit(victim).unwrap().last_hit.is_none());
        assert_eq!(s.bolts[0].hit, Hit::None);
    }

    #[test]
    fn zero_direct_damage_explosive_still_collides() {
        let (mut s, h, gun, tracer) = scene(1, 0.0);
        let blast = crate::weapon::ExplosionClass {
            damage: 100.0,
            damage_inner: 1.0,
            damage_outer: 4.0,
            ..Default::default()
        };
        s.weapon_classes[tracer].ordnance.explosion = Some(blast.clone());
        s.weapon_classes[gun]
            .secondary
            .as_mut()
            .unwrap()
            .ordnance
            .explosion = Some(blast);
        let victim = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -3.0)))
            .unwrap();
        let origin = s.unit(h).unwrap().muzzle();
        let dir = (s.unit(victim).unwrap().aim_point() - origin).normalize();
        s.fire_secondary(
            Shooter {
                owner: h,
                team: 0,
                weapon_class: gun,
                is_player: true,
            },
            gun,
            1,
            origin,
            dir,
            1.0,
        );
        s.fly_bolts(crate::TICK_DT);
        assert!(s.bolts.is_empty());
        assert!(s.unit(victim).unwrap().health.cur < s.unit(victim).unwrap().health.max);
    }
}
