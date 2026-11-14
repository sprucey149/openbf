//! Authored damage regions: Mustafar's lava and steam, Felucia's and Polis
//! Massa's pools (BF2 only; no BF1 map authors one). A region named `damage...`
//! carries `key=value` pairs (BF2 `FUN_0035c8e0`), and every update each living
//! object inside takes `damagerate x dt` scaled by its health type, with no
//! attacker and no scale on a shield (`FUN_0035c578`, `analysis/re-jetpack.md` §16).
//! An object is inside when its bounding sphere touches the volume: measured live,
//! a falling soldier is first hit with his feet 0.43-0.67 m above a box's top.
use super::{soldier_blast_sphere, DeathCause, Sim};
use crate::region::Region;
use crate::vehicle::VehicleHandle;
use crate::weapon::DamageDesc;
use glam::Vec3;

/// One region's volume and its damage record. A scale it does not name is 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageRegion {
    pub region: Region,
    pub rate: f32,
    /// By [`crate::weapon::HealthType`]: person, animal, droid, vehicle and the
    /// three building states.
    pub scales: [f32; 7],
}

impl DamageRegion {
    /// `None` unless the name starts with `damage`. Keys match by their shortest
    /// unambiguous prefix, as the PS2's table lookup does; other tokens are skipped.
    pub fn parse(name: &str, region: Region) -> Option<Self> {
        const KEYS: [&str; 8] = [
            "damagerate",
            "personscale",
            "animalscale",
            "droidscale",
            "vehiclescale",
            "buildingscale",
            "buildingdeadscale",
            "buildingunbuiltscale",
        ];
        let name = name.to_ascii_lowercase();
        if !name.starts_with("damage") {
            return None;
        }
        let mut values = [0.0f32; 8];
        for token in name.split_whitespace().skip(1) {
            let Some((key, value)) = token.split_once('=') else {
                continue;
            };
            let mut hits = KEYS.iter().enumerate().filter(|(_, k)| k.starts_with(key));
            if let (Some((i, _)), None, Ok(v)) = (hits.next(), hits.next(), value.parse()) {
                values[i] = v;
            }
        }
        let mut scales = [0.0; 7];
        scales.copy_from_slice(&values[1..]);
        Some(Self {
            region,
            rate: values[0],
            scales,
        })
    }

    fn desc(&self) -> DamageDesc {
        DamageDesc {
            damage: self.rate * crate::TICK_DT,
            scales: self.scales,
            shield_scale: 0.0,
        }
    }
}

impl Sim {
    pub(super) fn tick_damage_regions(&mut self) {
        for r in 0..self.damage_regions.len() {
            let dr = self.damage_regions[r];
            let desc = dr.desc();
            for i in 0..self.units.len() {
                let u = &self.units[i];
                if !u.alive || u.mount.is_some() {
                    continue;
                }
                let (lift, radius) = soldier_blast_sphere(u.soldier.stance);
                let centre = u.soldier.pos + Vec3::Y * lift;
                if !dr.region.touches_sphere(centre, radius) {
                    continue;
                }
                if u.is_player && self.cheats_of(u).invincible {
                    continue;
                }
                let h = self.handle(i);
                let u = &mut self.units[i];
                if u.health.take_damage(&desc, 1.0, false, 1.0) > 0.0 {
                    u.cloak.strength = 0.0;
                }
                if !u.health.alive() {
                    self.unit_killed_with_cause(h, None, DeathCause::DamageRegion);
                }
            }
            for i in 0..self.vehicles.len() {
                let v = &self.vehicles[i];
                let (centre, radius) = self.vehicle_blast_sphere(v);
                // A scale of 0 is no hit at all: no hit event every update.
                if v.alive
                    && dr.scales[v.health.health_type as usize] != 0.0
                    && dr.region.touches_sphere(centre, radius)
                {
                    let h = VehicleHandle {
                        index: i as u32,
                        gen: v.gen,
                    };
                    self.damage_vehicle(h, &desc, 1.0, None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heightfield::Heightfield;
    use crate::sim::{Event, PadState};
    use crate::unit::UnitSpec;

    fn zone() -> Region {
        Region::sphere(Vec3::ZERO, 5.0)
    }

    #[test]
    fn a_region_name_gives_its_rate_and_scales() {
        let d = DamageRegion::parse(
            "damageregion damagerate=900 personscale=100 animalscale=2 droidscale=3 vehiclescale=4",
            zone(),
        )
        .unwrap();
        assert_eq!(d.rate, 900.0);
        assert_eq!(d.scales, [100.0, 2.0, 3.0, 4.0, 0.0, 0.0, 0.0]);
        // The shortest prefix that names one key is enough; `building` alone is
        // three keys, and is not taken.
        let d = DamageRegion::parse("Damage da=20 p=1 building=7 junk", zone()).unwrap();
        assert_eq!(d.rate, 20.0);
        assert_eq!(d.scales, [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!(DamageRegion::parse("deathregion damagerate=9", zone()).is_none());
    }

    fn ground(s: &mut Sim) {
        s.heightfield = Some(Heightfield {
            size: 2,
            unit: 400.0,
            offset_x: 200.0,
            offset_z: 200.0,
            heights: vec![0.0; 4],
        });
    }

    #[test]
    fn inside_a_region_a_soldier_loses_rate_by_scale_per_second_and_dies_unattributed() {
        let mut s = Sim::empty(1);
        ground(&mut s);
        s.damage_regions =
            vec![DamageRegion::parse("damage damagerate=1 personscale=50", zone()).unwrap()];
        let spec = |x: f32| UnitSpec::trooper(0, Vec3::new(x, 0.0, 0.0));
        let inside = s.spawn(&spec(0.0)).unwrap();
        let outside = s.spawn(&spec(50.0)).unwrap();
        let start = s.unit(inside).unwrap().health.cur;
        for _ in 0..crate::TICK_HZ {
            s.step(&[PadState::default(), PadState::default()]);
        }
        let lost = start - s.unit(inside).unwrap().health.cur;
        assert!((lost - 50.0).abs() < 0.01, "50 per second, took {lost}");
        assert_eq!(s.unit(outside).unwrap().health.cur, start);
        s.damage_regions[0].rate = 100_000.0;
        s.step(&[PadState::default(), PadState::default()]);
        assert!(s.unit(inside).is_none(), "dead");
        assert!(s.events.iter().any(|e| matches!(
            e,
            Event::EnvDeath {
                cause: DeathCause::DamageRegion,
                ..
            }
        )));
        assert_eq!(s.unit(outside).unwrap().health.cur, start);
    }

    #[test]
    fn the_invincible_cheat_and_an_unnamed_scale_spare_a_soldier() {
        let mut s = Sim::empty(1);
        ground(&mut s);
        s.damage_regions =
            vec![DamageRegion::parse("damage damagerate=100 droidscale=1", zone()).unwrap()];
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
        let start = s.unit(h).unwrap().health.cur;
        s.step(&[PadState::default()]);
        assert_eq!(s.unit(h).unwrap().health.cur, start, "no person scale");

        let mut s = Sim::empty(1);
        ground(&mut s);
        s.damage_regions =
            vec![DamageRegion::parse("damage damagerate=100 personscale=1", zone()).unwrap()];
        s.cheats.invincible = true;
        let h = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        s.step(&[PadState::default()]);
        assert_eq!(s.unit(h).unwrap().health.cur, s.unit(h).unwrap().health.max);
    }

    #[test]
    fn a_soldiers_sphere_touches_a_box_below_his_feet_as_on_the_ps2() {
        // BF2 Mustafar's steam box, top y 75.51; live, the first hit came with the
        // feet 0.43-0.67 m over it.
        let boxy = Region::from_rows(
            "box",
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [0.0, 71.067, 0.0],
            [2.646, 4.446, 2.733],
        )
        .unwrap();
        let (lift, radius) = soldier_blast_sphere(crate::soldier::Stance::Stand);
        let at = |feet: f32| boxy.touches_sphere(Vec3::new(0.0, feet + lift, 0.0), radius);
        assert!(at(75.513 + 0.43));
        assert!(!at(75.513 + 0.47));
        assert!(boxy.touches_sphere(Vec3::new(2.646 + 1.0, 71.0, 0.0), 1.1));
        assert!(!boxy.touches_sphere(Vec3::new(2.646 + 1.0, 71.0, 0.0), 0.9));
    }

    #[test]
    fn a_vehicle_takes_its_scale_and_a_zero_scale_is_no_hit() {
        let mut s = Sim::empty(1);
        ground(&mut s);
        let h = s.spawn_vehicle(&crate::vehicle::VehicleSpec {
            kind: crate::vehicle::VehicleKind::Hover,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: 100.0,
            health_type: crate::weapon::HealthType::Vehicle,
            seats: vec![crate::vehicle::SeatKind::Pilot],
        });
        let start = s.vehicle(h).unwrap().health.cur;
        s.damage_regions =
            vec![DamageRegion::parse("damage damagerate=30 personscale=1", zone()).unwrap()];
        s.step(&[]);
        assert_eq!(s.vehicle(h).unwrap().health.cur, start);
        assert!(!s
            .events
            .iter()
            .any(|e| matches!(e, Event::VehicleHit { .. })));
        s.damage_regions =
            vec![DamageRegion::parse("damage damagerate=30 vehiclescale=2", zone()).unwrap()];
        s.step(&[]);
        assert!((start - s.vehicle(h).unwrap().health.cur - 2.0).abs() < 0.01);
    }
}
