//! Emitter shots (card UORDE, `analysis/re-ordnance-kinds.md` §8): one
//! instant cone-and-chain query per weapon shot — no bolt. BF1's arc caster
//! and BF2's arc casters, incinerator and flamethrowers all arrive here; the
//! query law itself lives in [`crate::emitter`].

use super::{soldier_blast_sphere, Event, Sim};
use crate::emitter::{query, Candidate};
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::VehicleHandle;
use crate::weapon::{DamageDesc, ExplosionClass};
use glam::Vec3;

/// A body the query can touch, with the handle that applies its damage.
#[derive(Clone, Copy)]
enum Victim {
    Unit(UnitHandle),
    Vehicle(VehicleHandle),
}

impl Sim {
    /// Fire weapon class `class`'s emitter for unit `i` at `charge`. The
    /// shooter's weapon path calls this instead of spawning a
    /// [`crate::projectile::Bolt`].
    pub(super) fn fire_emitter(&mut self, i: usize, class: usize, charge: f32) {
        let Some(em) = self.weapon_classes[class].emitter.clone() else {
            return;
        };
        let ord = self.weapon_classes[class].ordnance.clone();
        let blend = ord.charged(charge);
        let floor = ord.min_damage.unwrap_or(0.0);
        let unit = self.handle(i);
        let team = self.units[i].team;
        let shooter_is_player = self.units[i].is_player;
        let origin = self.units[i].muzzle();
        let (aim, _, _) = self.units[i].aim_frame();
        // The pull itself is a shot like any other: the fire sound, the
        // muzzle flash and the trigger animation read this, and the arc
        // renderer pairs it with the `Emitter` links below.
        self.events.push(Event::Shot {
            unit,
            weapon_class: class as u32,
            origin,
            dir: aim,
        });

        // The gather: alive soldiers (riders in exposed seats included; BF1's
        // mask has no vehicles) then alive hulls (BF2). The body point is 1 m
        // above an entity's origin, the PS2's "body + 1 m".
        let mut targets = Vec::new();
        let mut victims = Vec::new();
        for (h, u) in self.alive_units() {
            if h.index as usize == i {
                continue;
            }
            let Some(body) = self.hero_body_prims(h.index as usize) else {
                continue;
            };
            targets.push(Candidate {
                pos: body.pos + Vec3::Y,
                team: Some(u.team),
                vehicle: false,
            });
            victims.push(Victim::Unit(h));
        }
        for (h, v) in self.alive_vehicles() {
            let (centre, _) = self.vehicle_blast_sphere(v);
            targets.push(Candidate {
                pos: centre + Vec3::Y,
                team: v.team,
                vehicle: true,
            });
            victims.push(Victim::Vehicle(h));
        }

        let shot = query(
            &em,
            blend.damage,
            floor,
            origin,
            aim,
            team,
            &targets,
            |from, to| crate::perception::los(self, from, to),
        );

        // BF1 forces every health-type and shield scale to 1.0; BF2 uses the
        // class's own.
        let (scales, shield_scale) = if em.bf2 {
            (blend.scales, blend.shield_scale)
        } else {
            ([1.0; 7], 1.0)
        };
        let ff = self.friendly_fire_scale;
        for hit in &shot.hits {
            let body = targets[hit.index];
            let victim = victims[hit.index];
            // A zero-damage link is a pure shove (the Force push family:
            // `Damage` 0 with `Push` / `ConeLength` / `MaxTargets`
            // authored). It takes no health, so it also takes no TakeDamage,
            // no `Event::Hit` and no per-hit blast: the push and the arc
            // below are its whole effect. `analysis/re-ordnance-kinds.md` §8
            // does not state the zero-damage case; the class's own query
            // (ConeLength > 0) is what dispatches, and `Push` is its knob.
            // A spared victim (`bolt_spares`: invincible, or a human a bot of
            // his own team hit) or a raised saber stops the link: no damage,
            // burn, blast or shove, and the arc still draws (as `fire_force`
            // blocks a hero's).
            let stopped = match victim {
                Victim::Unit(h) => {
                    let spared = self
                        .unit(h)
                        .is_some_and(|v| self.shot_spares(team, shooter_is_player, v));
                    let incoming = (body.pos - hit.from).normalize_or_zero();
                    spared || self.saber_deflect(h, incoming, body.pos)
                }
                Victim::Vehicle(_) => false,
            };
            if hit.damage > 0.0 && !stopped {
                let desc = DamageDesc {
                    damage: hit.damage,
                    scales,
                    shield_scale,
                };
                match victim {
                    Victim::Unit(h) => {
                        let Some(friendly) = self.unit(h).map(|u| self.allied(team, u.team)) else {
                            continue;
                        };
                        self.ignite(h, unit, class);
                        let mult = self.buff_scale(Some(unit), h);
                        let Some(v) = self.unit_mut(h) else { continue };
                        let damage = v.lend_droideka_shield(|health| {
                            health.take_damage(&desc, mult, friendly, ff)
                        });
                        if self
                            .units
                            .get(unit.index as usize)
                            .is_some_and(|o| o.gen == unit.gen && o.controller == Controller::Ai)
                        {
                            self.nav_stats.ai_hits += 1;
                        }
                        self.nav_stats.bolt_hits += 1;
                        if friendly && unit != h {
                            self.nav_stats.friend_hits += 1;
                        }
                        self.finish_unit_damage(h, Some(unit), damage, 0.0, aim);
                    }
                    Victim::Vehicle(h) => {
                        self.damage_vehicle_by(h, &desc, 1.0, Some(unit), Some(team), Some(aim));
                    }
                }
                // BF2's `Explosion` goes off at each body hit (the Imperial
                // arc caster's chassis shock). A zero-damage link skips it:
                // the Force push's `Explosion` class (`com_weap_inf_force_
                // push_exp`) authors Damage 0 and Push 0, so in this sim a
                // zero-damage blast would only add 0-damage `Event::Hit`s.
                if em.bf2 {
                    if let Some(e) = &ord.explosion {
                        self.explode(e, body.pos, Some(unit), Some(team));
                        self.events.push(Event::WeaponBlast {
                            pos: body.pos,
                            weapon_class: class as u32,
                        });
                    }
                }
            }
            // The shove (`Push`; 0 = none, and then no LCG draw either).
            if em.push != 0.0 && !stopped {
                if let Victim::Unit(h) = victim {
                    if let Some(u) = self.units.get(h.index as usize) {
                        let (_, w) = soldier_blast_sphere(u.soldier.stance);
                        let e = ExplosionClass {
                            push: em.push,
                            push_outer: f32::INFINITY,
                            ..Default::default()
                        };
                        let pos = body.pos;
                        self.kick_and_push(h, &e, hit.from, pos, w);
                    }
                }
            }
            self.events.push(Event::Emitter {
                unit,
                weapon_class: class as u32,
                from: hit.from,
                to: body.pos,
            });
        }
        if em.lightning.is_none() {
            return;
        }
        let misses = if em.bf2 {
            (em.bolts as usize).saturating_sub(shot.hits.len())
        } else {
            usize::from(shot.hits.is_empty())
        };
        if misses == 0 {
            return;
        }
        let to = origin + aim * crate::perception::reach(self, origin, aim, em.cone_length);
        for _ in 0..misses {
            self.events.push(Event::Emitter {
                unit,
                weapon_class: class as u32,
                from: origin,
                to,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{Body, Shape};
    use crate::sim::tests::{flat_ground, hits};
    use crate::sim::Event;
    use crate::unit::UnitSpec;
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::{EmitterOrdnance, HealthType, OrdnanceClass, ShotSpec, WeaponClass, DEG};
    use glam::Mat4;

    use crate::emitter::fixtures::{bf1, bf2, flame};

    fn sim_with(em: EmitterOrdnance, ord: OrdnanceClass) -> (Sim, usize) {
        let mut s = Sim::empty(7);
        s.heightfield = Some(flat_ground());
        let class = s.add_weapon_class(WeaponClass {
            emitter: Some(em),
            ordnance: ord,
            ..Default::default()
        });
        (s, class)
    }

    fn arc_ord() -> OrdnanceClass {
        OrdnanceClass {
            max_damage: 250.0,
            min_damage: Some(100.0),
            scales: [1.0; 7],
            shield_scale: 1.0,
            ..Default::default()
        }
    }

    /// `com_weap_inf_force_push_ord` (bf2_all.ron): Damage 0 / MaxDamage 0,
    /// ConeLength 20, ConeAngle 120, MaxTargets 4, hit-all, Push 15.
    fn force_push() -> EmitterOrdnance {
        EmitterOrdnance {
            cone_length: 20.0,
            cone_angle: 120.0 * DEG,
            max_targets: 4,
            jump_deduction: 0.0,
            damage_threshold: 0.0,
            max_jump_distance: 0.0,
            first_radius: 0.0,
            push: 15.0,
            lightning: None,
            bolts: 1,
            affect_friends: true,
            affect_enemies: true,
            hit_all: true,
            radius: 0.0,
            bf2: true,
            effect: None,
            burn: Default::default(),
        }
    }

    /// The Force powers' ordnance authors no damage at all (`Damage` 0).
    fn no_damage() -> OrdnanceClass {
        OrdnanceClass::default()
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

    /// A player at `at` aiming +X (yaw -π/2).
    fn shooter(s: &mut Sim, at: Vec3) -> UnitHandle {
        let h = s.spawn(&UnitSpec::player(0, at)).unwrap();
        s.units[h.index as usize].soldier.yaw = -std::f32::consts::FRAC_PI_2;
        h
    }

    fn line(s: &mut Sim, n: usize) -> Vec<UnitHandle> {
        (0..n)
            .map(|k| {
                s.spawn(&UnitSpec::trooper(
                    1,
                    Vec3::new(6.0 + 3.0 * k as f32, 0.0, 0.0),
                ))
                .unwrap()
            })
            .collect()
    }

    fn emitter_links(s: &Sim) -> Vec<(Vec3, Vec3)> {
        s.events
            .iter()
            .filter_map(|e| match e {
                Event::Emitter { from, to, .. } => Some((*from, *to)),
                _ => None,
            })
            .collect()
    }

    fn blasts(s: &Sim) -> Vec<(Vec3, u32)> {
        s.events
            .iter()
            .filter_map(|e| match e {
                Event::WeaponBlast { pos, weapon_class } => Some((*pos, *weapon_class)),
                _ => None,
            })
            .collect()
    }

    /// The BF1 clone pilot's arc caster: 1 target under charge 1/3, 2 under
    /// 5/6, 3 above.
    #[test]
    fn bf1_arc_caster_takes_one_two_or_three_by_charge() {
        for (charge, want) in [(0.3, 1), (0.5, 2), (0.9, 3)] {
            let (mut s, class) = sim_with(bf1(), arc_ord());
            let unit = shooter(&mut s, Vec3::ZERO);
            line(&mut s, 3);
            s.fire_emitter(unit.index as usize, class, charge);
            let n = s
                .events
                .iter()
                .filter(|e| matches!(e, Event::Emitter { .. }))
                .count();
            assert_eq!(n, want, "charge {charge}");
        }
    }

    #[test]
    fn bf1_full_charge_deals_250_175_100_down_a_line() {
        let (mut s, class) = sim_with(bf1(), arc_ord());
        let unit = shooter(&mut s, Vec3::ZERO);
        let at = line(&mut s, 3);
        s.fire_emitter(unit.index as usize, class, 1.0);
        let got = hits(&s);
        let want = [
            (at[0].index, 250.0),
            (at[1].index, 175.0),
            (at[2].index, 100.0),
        ];
        assert_eq!(got, want);
        assert_eq!(s.unit(at[0]).unwrap().health.cur, 50.0);
        assert_eq!(s.unit(at[1]).unwrap().health.cur, 125.0);
        assert_eq!(s.unit(at[2]).unwrap().health.cur, 200.0);
    }

    /// A wall between the first and second bodies skips the second link; the
    /// chain still reaches a third body it can see.
    #[test]
    fn a_wall_skips_one_link_without_ending_the_chain() {
        let (mut s, class) = sim_with(bf1(), arc_ord());
        let unit = shooter(&mut s, Vec3::ZERO);
        let at = line(&mut s, 2);
        let far = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(15.0, 0.0, 3.5)))
            .unwrap();
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(8.7, 1.0, 0.0)),
            Shape::Box {
                half: Vec3::new(0.2, 2.0, 0.6),
            },
        ));
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(
            hits(&s),
            vec![(at[0].index, 250.0), (far.index, 175.0)],
            "the middle body is behind the wall"
        );
        assert_eq!(s.unit(at[1]).unwrap().health.cur, 300.0);
    }

    #[test]
    fn a_miss_draws_one_arc_to_the_cone_length() {
        let (mut s, class) = sim_with(bf1(), arc_ord());
        let unit = shooter(&mut s, Vec3::ZERO);
        s.fire_emitter(unit.index as usize, class, 1.0);
        let links = emitter_links(&s);
        assert_eq!(links.len(), 1);
        let (from, to) = links[0];
        assert_eq!(from, s.units[unit.index as usize].muzzle());
        assert!(to.y.abs() < 0.05, "the arc ends on the ground: {to}");
        assert!(hits(&s).is_empty());

        s.heightfield = None;
        s.events.clear();
        s.fire_emitter(unit.index as usize, class, 1.0);
        let (from, to) = emitter_links(&s)[0];
        assert!(((to - from).length() - 30.0).abs() < 1e-3, "{from} {to}");
    }

    #[test]
    fn a_miss_arc_stops_at_a_wall() {
        let (mut s, class) = sim_with(bf2(), arc_ord());
        let unit = shooter(&mut s, Vec3::ZERO);
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(5.0, 1.0, 0.0)),
            Shape::Box {
                half: Vec3::new(0.2, 4.0, 4.0),
            },
        ));
        s.fire_emitter(unit.index as usize, class, 1.0);
        let links = emitter_links(&s);
        assert_eq!(links.len(), 1);
        assert!((links[0].1.x - 4.8).abs() < 1e-3, "{:?}", links[0]);
    }

    #[test]
    fn bf2_draws_its_unspent_bolts_as_misses() {
        let (mut s, class) = sim_with(EmitterOrdnance { bolts: 3, ..bf2() }, arc_ord());
        let unit = shooter(&mut s, Vec3::ZERO);
        let at = line(&mut s, 1);
        s.fire_emitter(unit.index as usize, class, 1.0);
        let links = emitter_links(&s);
        assert_eq!(hits(&s)[0].0, at[0].index);
        assert_eq!(links.len(), 3);
        assert_eq!(links[1], links[2]);
        assert_eq!(links[1].0, s.units[unit.index as usize].muzzle());
    }

    #[test]
    fn an_emitter_without_lightning_draws_no_miss() {
        let (mut s, class) = sim_with(force_push(), no_damage());
        let unit = shooter(&mut s, Vec3::ZERO);
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert!(emitter_links(&s).is_empty());
    }

    /// The rider in a hull has no body for the cone to find; the hull does.
    #[test]
    fn a_cone_finds_a_rider_only_in_an_exposed_seat() {
        for exposed in [false, true] {
            let (mut s, class) = sim_with(bf1(), arc_ord());
            let unit = shooter(&mut s, Vec3::ZERO);
            let rider = s
                .spawn(&UnitSpec::trooper(1, Vec3::new(6.0, 0.0, 0.0)))
                .unwrap();
            crate::sim::hero_bodies::seat_fixture(&mut s, rider, exposed);
            s.fire_emitter(unit.index as usize, class, 1.0);
            assert_eq!(s.unit(rider).unwrap().health.cur < 300.0, exposed);
        }
    }

    /// BF1 has no team test: an ally in the cone is zapped too.
    #[test]
    fn bf1_zaps_an_ally() {
        let (mut s, class) = sim_with(bf1(), arc_ord());
        let unit = shooter(&mut s, Vec3::ZERO);
        let ally = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(6.0, 0.0, 0.0)))
            .unwrap();
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(hits(&s), vec![(ally.index, 250.0)]);
        assert_eq!(s.unit(ally).unwrap().health.cur, 50.0);
    }

    #[test]
    fn bf2_hit_all_splits_between_two_bodies() {
        let (mut s, class) = sim_with(
            flame(),
            OrdnanceClass {
                max_damage: 15.0,
                min_damage: Some(5.0),
                ..Default::default()
            },
        );
        let unit = shooter(&mut s, Vec3::ZERO);
        let a = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(3.0, 0.0, 0.0)))
            .unwrap();
        let b = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(4.0, 0.0, 0.0)))
            .unwrap();
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(hits(&s), vec![(a.index, 7.5), (b.index, 7.5)]);
        assert_eq!(s.unit(a).unwrap().health.cur, 292.5);
        assert_eq!(s.unit(b).unwrap().health.cur, 292.5);
    }

    #[test]
    fn bf2_pushes_one_blast_per_hit_at_the_body() {
        let (mut s, class) = sim_with(
            flame(),
            OrdnanceClass {
                max_damage: 15.0,
                min_damage: Some(5.0),
                explosion: Some(ExplosionClass {
                    damage: 5.0,
                    damage_outer: 1.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let unit = shooter(&mut s, Vec3::ZERO);
        s.spawn(&UnitSpec::trooper(1, Vec3::new(3.0, 0.0, 0.0)))
            .unwrap();
        s.spawn(&UnitSpec::trooper(1, Vec3::new(4.0, 0.0, 0.0)))
            .unwrap();
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(emitter_links(&s).len(), 2, "both bodies take the flame");
        assert_eq!(
            blasts(&s),
            [
                (Vec3::new(3.0, 1.0, 0.0), class as u32),
                (Vec3::new(4.0, 1.0, 0.0), class as u32),
            ]
        );
    }

    #[test]
    fn a_class_without_an_explosion_pushes_no_blast() {
        let (mut s, class) = sim_with(
            flame(),
            OrdnanceClass {
                max_damage: 15.0,
                min_damage: Some(5.0),
                ..Default::default()
            },
        );
        let unit = shooter(&mut s, Vec3::ZERO);
        s.spawn(&UnitSpec::trooper(1, Vec3::new(3.0, 0.0, 0.0)))
            .unwrap();
        s.spawn(&UnitSpec::trooper(1, Vec3::new(4.0, 0.0, 0.0)))
            .unwrap();
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(hits(&s).len(), 2, "the query still lands");
        assert!(blasts(&s).is_empty());

        let (mut s, class) = sim_with(
            bf1(),
            OrdnanceClass {
                max_damage: 250.0,
                min_damage: Some(100.0),
                explosion: Some(ExplosionClass {
                    damage: 5.0,
                    damage_outer: 1.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let unit = shooter(&mut s, Vec3::ZERO);
        line(&mut s, 2);
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(hits(&s).len(), 2);
        assert!(blasts(&s).is_empty(), "BF1 has no per-hit explosion");
    }

    #[test]
    fn bf2_affect_enemies_false_leaves_the_enemy_alone() {
        let mut em = bf2();
        em.affect_enemies = false;
        let (mut s, class) = sim_with(
            em,
            OrdnanceClass {
                max_damage: 300.0,
                min_damage: Some(300.0),
                ..Default::default()
            },
        );
        let unit = shooter(&mut s, Vec3::ZERO);
        let enemy = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(6.0, 0.0, 0.0)))
            .unwrap();
        let ally = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(9.0, 0.0, 0.0)))
            .unwrap();
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(hits(&s), vec![(ally.index, 300.0)]);
        assert_eq!(s.unit(enemy).unwrap().health.cur, 300.0);
    }

    #[test]
    fn bf2_hits_a_vehicle_and_bf1_does_not() {
        let v = |pos| VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos,
            yaw: 0.0,
            max_health: 500.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        };
        let (mut s, class) = sim_with(bf1(), arc_ord());
        let unit = shooter(&mut s, Vec3::ZERO);
        let h = s.spawn_vehicle(&v(Vec3::new(6.0, 0.0, 0.0)));
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(s.vehicle(h).unwrap().health.cur, 500.0);

        let (mut s, class) = sim_with(
            bf2(),
            OrdnanceClass {
                max_damage: 300.0,
                min_damage: Some(300.0),
                ..Default::default()
            },
        );
        let unit = shooter(&mut s, Vec3::ZERO);
        let h = s.spawn_vehicle(&v(Vec3::new(6.0, 0.0, 0.0)));
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(s.vehicle(h).unwrap().health.cur, 200.0);
    }

    /// A zero-damage cone (the Force push) is still an emitter: the class
    /// dispatches on its cone, the query shoves the body and spawns no bolt.
    #[test]
    fn a_zero_damage_cone_pushes_and_spawns_no_bolt() {
        let (mut s, class) = sim_with(force_push(), no_damage());
        let unit = shooter(&mut s, Vec3::ZERO);
        s.units[unit.index as usize].weapon_class = class;
        let target = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(6.0, 0.0, 0.0)))
            .unwrap();
        s.soldier_shots(unit.index as usize, None, &[shot()]);
        assert!(s.bolts.is_empty(), "an emitter spawns no bolt");
        assert!(hits(&s).is_empty(), "a zero-damage link deals no damage");
        assert_eq!(s.unit(target).unwrap().health.cur, 300.0);
        let vel = s.units[target.index as usize].soldier.vel;
        assert!(vel.x > 1.0, "pushed along the aim: {vel}");
        assert_eq!(s.nav_stats.knocks, 1);
        assert_eq!(emitter_links(&s).len(), 1);
    }

    /// BF1 has no team test: a zero-damage BF1 cone shoves an ally too.
    #[test]
    fn bf1_zero_damage_cone_pushes_an_ally() {
        let mut em = bf1();
        em.push = 15.0;
        let (mut s, class) = sim_with(em, no_damage());
        let unit = shooter(&mut s, Vec3::ZERO);
        let ally = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(6.0, 0.0, 0.0)))
            .unwrap();
        s.fire_emitter(unit.index as usize, class, 1.0);
        let vel = s.units[ally.index as usize].soldier.vel;
        assert!(vel.x > 1.0, "the ally is pushed: {vel}");
        assert_eq!(s.unit(ally).unwrap().health.cur, 300.0);
        assert_eq!(emitter_links(&s).len(), 1);
    }

    /// BF2's `AffectFriends = 0` keeps the ally out of the gather: no push,
    /// no link, while the enemy behind him is shoved.
    #[test]
    fn bf2_affect_friends_false_does_not_push_the_ally() {
        let mut em = force_push();
        em.affect_friends = false;
        let (mut s, class) = sim_with(em, no_damage());
        let unit = shooter(&mut s, Vec3::ZERO);
        let ally = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(6.0, 0.0, 0.0)))
            .unwrap();
        let enemy = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(9.0, 0.0, 0.0)))
            .unwrap();
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(s.units[ally.index as usize].soldier.vel, Vec3::ZERO);
        let vel = s.units[enemy.index as usize].soldier.vel;
        assert!(vel.x > 1.0, "the enemy is pushed: {vel}");
        assert_eq!(
            emitter_links(&s),
            vec![(s.unit(unit).unwrap().muzzle(), Vec3::new(9.0, 1.0, 0.0))]
        );
    }

    /// The line of sight gates a push as it gates a damaging link.
    #[test]
    fn a_wall_stops_a_push() {
        let (mut s, class) = sim_with(force_push(), no_damage());
        let unit = shooter(&mut s, Vec3::ZERO);
        let target = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(6.0, 0.0, 0.0)))
            .unwrap();
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(3.0, 1.0, 0.0)),
            Shape::Box {
                half: Vec3::new(0.2, 2.0, 0.6),
            },
        ));
        s.fire_emitter(unit.index as usize, class, 1.0);
        assert_eq!(s.units[target.index as usize].soldier.vel, Vec3::ZERO);
        assert_eq!(s.unit(target).unwrap().health.cur, 300.0);
        assert!(emitter_links(&s).is_empty());
    }

    /// A push link emits its arc record like a damaging one: one record per
    /// body in the cone, from the muzzle to the body point.
    #[test]
    fn a_push_emits_one_arc_per_link() {
        let (mut s, class) = sim_with(force_push(), no_damage());
        let unit = shooter(&mut s, Vec3::ZERO);
        s.spawn(&UnitSpec::trooper(1, Vec3::new(6.0, 0.0, 0.0)))
            .unwrap();
        s.spawn(&UnitSpec::trooper(1, Vec3::new(9.0, 0.0, 0.0)))
            .unwrap();
        s.fire_emitter(unit.index as usize, class, 1.0);
        let muzzle = s.units[unit.index as usize].muzzle();
        assert_eq!(
            emitter_links(&s),
            vec![
                (muzzle, Vec3::new(6.0, 1.0, 0.0)),
                (muzzle, Vec3::new(9.0, 1.0, 0.0)),
            ]
        );
    }

    /// The weapon path routes an emitter class here, not to a bolt.
    #[test]
    fn an_emitter_shot_goes_through_soldier_shots_without_a_bolt() {
        let (mut s, class) = sim_with(bf1(), arc_ord());
        let unit = shooter(&mut s, Vec3::ZERO);
        s.units[unit.index as usize].weapon_class = class;
        let target = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(6.0, 0.0, 0.0)))
            .unwrap();
        s.soldier_shots(
            unit.index as usize,
            None,
            &[ShotSpec {
                tilt: (0.0, 0.0),
                charge: 1.0,
                stage: 0,
                pitch_spread: 0.0,
                yaw_spread: 0.0,
            }],
        );
        assert!(s.bolts.is_empty(), "an emitter spawns no bolt");
        assert_eq!(s.unit(target).unwrap().health.cur, 50.0);
        assert!(s.events.iter().any(|e| matches!(e, Event::Emitter { .. })));
        assert_eq!(
            s.events
                .iter()
                .filter(|e| matches!(e, Event::Shot { .. }))
                .count(),
            1,
            "the pull is one shot event (the fire sound and the animation)"
        );
    }
}
