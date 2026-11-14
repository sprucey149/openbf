//! An explosion's area damage (card VDTH, `analysis/re-vehicle-death.md`
//! [O]): the ONE function every blast goes through — destroyed vehicles
//! ([`Sim::destroy_vehicle`]), exploding bolts and grenades. B1 FUN_002701a0 / B2 FUN_0030c8f8 (§2): gather at most 20
//! objects in reach, keep those a ray from the blast reaches past buildings
//! and terrain, and hand each the class's damage times the
//! falloff on its sphere (§3) through the bolt's TakeDamage (§4). A soldier
//! riding a vehicle takes none (§5). Then card KNOCK's death kick and push
//! (`analysis/re-knockback.md` [O]): a corpse's body is gathered too, a dead
//! body is kicked, and the soldiers the class pushes are thrown
//! ([`Sim::kick_and_push`]).

use super::state::Sim;
use crate::fmath::FloatExt;
use crate::projectile::Target;
use crate::soldier::Stance;
use crate::unit::UnitHandle;
use crate::vehicle::{VehicleHandle, VehicleKind};
use crate::weapon::{blast_falloff, ExplosionClass};
use glam::Vec3;

/// A blast gathers at most this many objects (the `20` passed to B1
/// FUN_00161c00 B1:194072 / B2 FUN_00161a98 B2:300525).
pub const BLAST_GATHER_MAX: usize = 20;

/// A soldier's collision sphere as a blast measures it — `(height of the
/// centre above the feet, radius w)` — per stance, measured in BF1 RAM over
/// 2,652 soldiers (§3 [V-RAM]; the code that writes them was not found [H]).
/// PROVISIONAL: sprint, jump, fall, roll and the throw take the standing
/// sphere; the tumble and get-up are posture 1 (the crouch's) and a corpse
/// posture 2 (the prone's: §6 measured +0.35, w 1.6221 on corpses).
pub fn soldier_blast_sphere(stance: Stance) -> (f32, f32) {
    match stance {
        Stance::Crouch | Stance::Tumble | Stance::GetUp => (0.75, 1.102_27),
        Stance::Prone | Stance::Dead => (0.35, 1.622_066),
        _ => (1.0, 1.462_635),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Victim {
    /// The blast's own attacker under HurtOwner 0: it takes a gather slot
    /// and nothing else.
    Owner,
    Unit(UnitHandle),
    Vehicle(VehicleHandle),
    Station(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BlastSource {
    Vehicle(VehicleHandle),
    Station(u16),
}

impl Sim {
    /// The sphere a blast measures vehicle `v` by, in the world: its class's
    /// ([`crate::vehicle::BlastSetup`]). A building turret whose model has
    /// no baked sphere (the BF1 common bake's gun turret) takes its
    /// building's bounds, `Vehicle::aim_sphere` - PROVISIONAL, ours: the
    /// PS2's is the model's sphere grown by its primitives, as for a
    /// vehicle; at the bare origin the floor under the building blocks the
    /// blast's line.
    pub fn vehicle_blast_sphere(&self, v: &crate::vehicle::Vehicle) -> (Vec3, f32) {
        match (self.blast_setup.get(v.class as usize), v.aim_sphere) {
            (Some(Some(b)), _) if b.sphere_radius > 0.0 => {
                (v.pos + v.rot * b.sphere_centre, b.sphere_radius)
            }
            (_, Some((c, r))) => (v.pos + v.rot * c, r),
            (Some(Some(b)), None) => (v.pos + v.rot * b.sphere_centre, b.sphere_radius),
            _ => (v.pos, 0.0),
        }
    }

    /// Detonate `e` at `pos` (§2). `attacker` is the unit credited with the
    /// damage and kills (none for an empty vehicle's blast), `team` the
    /// blast's team for the friendly-fire rule — a victim of that team takes
    /// `friendly_fire_scale`, and a blast whose attacker is not a human
    /// player cannot hurt a human player of that team at all (§4, B1
    /// FUN_001bc190).
    ///
    /// The gather (§2 step 3) takes units and corpses, then live vehicles and
    /// stations, in slot order. Ground uses BF2's inclusive sphere/box
    /// overlap and horizontal radius before filling the [`BLAST_GATHER_MAX`]
    /// slots. Space and the separate item pass use axis overlap. Eligible
    /// riders can fill slots, and only building turret riders take the splash.
    /// LOS (§2 step 4): the ray to the sphere centre — the PS2's target point
    /// for most classes; the soldier's own FUN_0020c680 [H] — must clear the
    /// terrain and the world's ORDNANCE subset (PROVISIONAL: the port's
    /// stand-in for the building layer; units and vehicles never block — the
    /// hulls are skipped, [`crate::collision::HullFilter::Static`]).
    pub fn explode(
        &mut self,
        e: &ExplosionClass,
        pos: Vec3,
        attacker: Option<UnitHandle>,
        team: Option<u8>,
    ) {
        self.explode_from(e, pos, attacker, team, None);
    }

    /// [`Sim::explode`] from a dying vehicle (`source`, card RIDK): the
    /// gather leaves out the vehicle itself and its riders, who die after
    /// the blast ([`Sim::destroy_vehicle`]).
    pub(super) fn explode_from(
        &mut self,
        e: &ExplosionClass,
        pos: Vec3,
        attacker: Option<UnitHandle>,
        team: Option<u8>,
        source: Option<BlastSource>,
    ) {
        let source_vehicle = match source {
            Some(BlastSource::Vehicle(h)) => Some(h),
            _ => None,
        };
        let source_instance = match source {
            Some(BlastSource::Station(i)) => self.stations[usize::from(i)].instance,
            _ => None,
        };
        let r = e.gather_radius();
        if r <= 0.0 {
            return;
        }
        let ground = !self.space_round();
        let box_min = pos - Vec3::splat(r);
        let box_max = pos + Vec3::splat(r);
        let overlaps = |c: Vec3, w: f32, ground_gather: bool| {
            let delta = c - pos;
            let reach = r + w;
            if ground_gather {
                let outside = c - c.max(box_min).min(box_max);
                delta.is_finite()
                    && delta.x * delta.x + delta.z * delta.z <= reach * reach
                    && outside.length_squared() <= w * w
            } else {
                delta.abs().cmple(Vec3::splat(reach)).all()
            }
        };
        let mut gathered: Vec<(Victim, Vec3, f32)> = Vec::with_capacity(BLAST_GATHER_MAX);
        for (i, u) in self.units.iter().enumerate() {
            // A corpse's body is in the world too (card KNOCK, §3).
            if !(u.alive || u.body)
                || source_vehicle.is_some_and(|v| u.mount.is_some_and(|m| m.vehicle == v))
                || (u.alive && u.mount.is_some() && !self.hero_body_active(i))
            {
                continue;
            }
            // HurtOwner 0: the owner is gathered (he fills the cap) and
            // then skipped, damage and push alike (B2:300612-300633).
            let owner =
                !e.hurt_owner && attacker.is_some_and(|a| a.index as usize == i && a.gen == u.gen);
            let (lift, w) = soldier_blast_sphere(u.soldier.stance);
            let c = u.soldier.pos + Vec3::Y * lift;
            if overlaps(c, w, ground) {
                let v = if owner {
                    Victim::Owner
                } else {
                    Victim::Unit(self.handle(i))
                };
                gathered.push((v, c, w));
                if gathered.len() == BLAST_GATHER_MAX {
                    break;
                }
            }
        }
        for (h, v) in self.alive_vehicles() {
            if gathered.len() == BLAST_GATHER_MAX {
                break;
            }
            if source_vehicle == Some(h) {
                continue;
            }
            let (c, w) = self.vehicle_blast_sphere(v);
            if overlaps(c, w, ground) {
                gathered.push((Victim::Vehicle(h), c, w));
            }
        }
        for (i, s) in self.stations.iter().enumerate() {
            if gathered.len() == BLAST_GATHER_MAX {
                break;
            }
            if !s.destroyed && overlaps(s.sphere_centre, s.sphere_radius, ground) {
                gathered.push((Victim::Station(i as u16), s.sphere_centre, s.sphere_radius));
            }
        }
        let items: Vec<_> = self
            .mines
            .iter()
            .map(|m| (Target::Mine(m.id), m.flight.pos, crate::mine::RADIUS))
            .chain(
                self.droids
                    .iter()
                    .map(|d| (Target::Droid(d.id), d.pos, crate::remote::RADIUS)),
            )
            .chain(
                self.turrets
                    .iter()
                    .map(|t| (Target::Turret(t.id), t.centre(), crate::autoturret::RADIUS)),
            )
            .chain(self.guided.iter().filter(|r| r.live()).map(|r| {
                (
                    Target::Guided(r.id),
                    r.pos,
                    crate::guided::COLLISION_RADIUS_PROVISIONAL,
                )
            }))
            // The cheap reach test first, as for every other victim: only an
            // item inside the outer radius is worth a line-of-sight ray.
            .filter(|&(_, c, w)| overlaps(c, w, false))
            .filter(|&(_, c, _)| {
                crate::perception::los_seat(
                    self,
                    pos,
                    c,
                    crate::collision::HullFilter::Static,
                    [source_instance, None],
                )
            })
            .map(|(t, c, w)| {
                (
                    t,
                    blast_falloff(c.distance(pos), w, e.damage_inner, e.damage_outer),
                )
            })
            .collect();
        for (t, f) in items {
            self.damage_item(t, e.damage * f, &e.scales, team, attacker);
        }
        let attacker_is_player = attacker
            .and_then(|a| self.units.get(a.index as usize).filter(|u| u.gen == a.gen))
            .is_some_and(|u| u.is_player);
        let desc = e.desc();
        for &(victim, c, w) in gathered.iter().rev() {
            let clear = match victim {
                Victim::Owner => continue,
                Victim::Station(i) => {
                    let instance = self.stations[usize::from(i)].instance;
                    crate::perception::los_seat(
                        self,
                        pos,
                        c,
                        crate::collision::HullFilter::Static,
                        [instance, source_instance],
                    )
                }
                // A building turret is measured past its own bodies (the
                // PS2's blast LOS ignores the victim itself,
                // re-turret-collision 3.4); its gunner is not: he is
                // shielded by them.
                Victim::Vehicle(h) => {
                    let own = self.vehicle(h).and_then(|v| v.instance);
                    crate::perception::los_seat(
                        self,
                        pos,
                        c,
                        crate::collision::HullFilter::Static,
                        [own, source_instance],
                    )
                }
                Victim::Unit(_) => crate::perception::los_seat(
                    self,
                    pos,
                    c,
                    crate::collision::HullFilter::Static,
                    [source_instance, None],
                ),
            };
            if !clear {
                continue;
            }
            let f = if e.damage_outer > 0.0 {
                blast_falloff((c - pos).length(), w, e.damage_inner, e.damage_outer)
            } else {
                0.0
            };
            match victim {
                Victim::Owner => {}
                Victim::Unit(h) => {
                    if f > 0.0 {
                        let dir = (c - pos).normalize_or_zero();
                        self.blast_unit(h, &desc, f, attacker, attacker_is_player, team, dir);
                    }
                    self.kick_and_push(h, e, pos, c, w);
                }
                Victim::Vehicle(h) if f > 0.0 => {
                    let dir = (c - pos).normalize_or_zero();
                    self.damage_vehicle_by(h, &desc, f, attacker, team, Some(dir));
                }
                Victim::Vehicle(_) => {}
                Victim::Station(i) if f > 0.0 => {
                    self.damage_station(i, &desc, f, attacker, team);
                }
                Victim::Station(_) => {}
            }
        }
    }

    /// After a soldier's splash (card KNOCK, `analysis/re-knockback.md`):
    /// - **the death kick** (B1 FUN_00200e68 -> FUN_00200f90, §3): a body
    ///   whose unit is dead - this blast's kill or an older corpse - gets
    ///   [`crate::soldier::DEATH_KICK`] along normalize(feet - blast);
    /// - **the push** (`vt+0x84`, B1 FUN_002009a0, §1): unless it is
    ///   mounted, thrown, tumbling or getting up:
    ///   [`crate::soldier::Soldier::knock`] with normalize(c - blast) + (0,
    ///   0.6, 0) and Push x [`crate::soldier::KNOCK_PUSH_SCALE`] x
    ///   [`knock_falloff`], under [`crate::soldier::KNOCK_RULES`] on every
    ///   map. Deviations (the maintainer, 2026-10-27): the living are pushed on BF2
    ///   maps too (BF2's `PushDeadOnly` is not read), and the push fades with
    ///   the distance (the disc authors PushRadiusInner = Outer, all or
    ///   nothing). A droid class rolls its `ChunkFrequency` first
    ///   ([`Sim::chunk_push_roll`], card DROIDDEATH): won, it is marked and
    ///   not pushed.
    pub(super) fn kick_and_push(
        &mut self,
        h: UnitHandle,
        e: &ExplosionClass,
        pos: Vec3,
        c: Vec3,
        w: f32,
    ) {
        let Some(u) = self
            .units
            .get(h.index as usize)
            .filter(|u| u.gen == h.gen && (u.alive || u.body))
        else {
            return;
        };
        if u.droideka.is_some() {
            return;
        }
        let mounted = u.mount.is_some();
        let i = h.index as usize;
        if !u.alive {
            let feet = self.units[i].soldier.pos;
            self.units[i].soldier.kick(feet - pos);
        }
        if e.push <= 0.0 || mounted || (e.spares_living && self.units[i].alive) {
            return;
        }
        if self.units[i].soldier.stance.knocked() || self.chunk_push_roll(i) {
            return;
        }
        let d = c - pos;
        let len = d.length();
        if len <= 1e-4 {
            return;
        }
        let f = knock_falloff(len, w, e.push_outer);
        if f <= 0.0 {
            return;
        }
        let draw = self.lcg.unit();
        let dir = d / len + Vec3::Y * crate::soldier::KNOCK_LIFT;
        let scale = e.push * crate::soldier::KNOCK_PUSH_SCALE * f;
        self.units[i]
            .soldier
            .knock(dir, scale, crate::soldier::KNOCK_RULES, draw);
        self.nav_stats.knocks += 1;
    }
}

/// The push's share at `d` from the blast for a body of width `w`: 1 at the
/// blast, falling linearly to 0 at `outer` (PushRadius) from the body's near
/// side. Ours (the maintainer, 2026-10-27): the PS2 pushes all or nothing inside it.
pub fn knock_falloff(d: f32, w: f32, outer: f32) -> f32 {
    if outer <= 0.0 {
        return 0.0;
    }
    (1.0 - (d - w * 0.5).fmax(0.0) / outer).fmax(0.0)
}

impl Sim {
    /// One soldier's splash (B1 FUN_00200e68 → FUN_00221600, §3-§5): riders
    /// of anything but a building turret skip; TakeDamage with the falloff
    /// as the multiplier; the kill through [`Sim::unit_killed`]; `dir` is the hit's way for the
    /// event (card HATK).
    #[allow(clippy::too_many_arguments)]
    fn blast_unit(
        &mut self,
        h: UnitHandle,
        desc: &crate::weapon::DamageDesc,
        f: f32,
        attacker: Option<UnitHandle>,
        attacker_is_player: bool,
        team: Option<u8>,
        dir: Vec3,
    ) {
        // A rider died in an earlier chained blast, or was killed above.
        let Some(v) = self.unit(h) else {
            return;
        };
        if let Some(m) = v.mount {
            if self
                .vehicle(m.vehicle)
                .is_some_and(|x| x.kind != VehicleKind::Turret)
            {
                return;
            }
        }
        // Card DPRX: a proxy is a human victim; the cheat is a player's only.
        let (victim_team, victim_is_player, victim_cheats) = (v.team, v.is_human(), v.is_player);
        let friendly = team.is_some_and(|team| self.allied(team, victim_team));
        if (victim_cheats && self.cheats_of(v).invincible)
            || (victim_is_player && friendly && !attacker_is_player)
        {
            return;
        }
        // Card TOKN: a blast of a bot without one of his tokens is real
        // too; counted.
        if let Some(a) = attacker.filter(|&a| victim_is_player && crate::ai::tokn::bars(self, a, h))
        {
            self.nav_stats.tokn_stray += 1;
            if self.unit(a).is_some_and(|o| o.ai.target != Some(h.into())) {
                self.nav_stats.tokn_cross += 1;
            }
        }
        // No tier multiplies a bot's blast into a human (card DRETIRE).
        let mult = self.buff_scale(attacker, h);
        let ff = self.friendly_fire_scale;
        let damage = self
            .unit_mut(h)
            .unwrap()
            .lend_droideka_shield(|health| health.take_damage(desc, f * mult, friendly, ff));
        self.finish_unit_damage(h, attacker, damage, super::state::BLAST_AGE, dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::tests::flat_ground;
    use crate::sim::{DeathCause, Event};
    use crate::unit::UnitSpec;
    use crate::vehicle::{BlastSetup, SeatKind, VehicleSpec};
    use crate::weapon::HealthType;

    /// `imp_walk_atst_exp` (§7): Damage 50, DamageRadius 20, Push 13 / 7.
    fn atst_exp() -> ExplosionClass {
        ExplosionClass {
            damage: 50.0,
            damage_outer: 20.0,
            push: 13.0,
            push_outer: 7.0,
            ..Default::default()
        }
    }

    /// The AAT / fighter-tank class (§7): 100 / 4, VehicleScale 0.
    fn tank_exp() -> ExplosionClass {
        let mut e = ExplosionClass {
            damage: 100.0,
            damage_outer: 4.0,
            ..Default::default()
        };
        e.scales[HealthType::Vehicle as usize] = 0.0;
        e
    }

    fn bike(pos: Vec3) -> VehicleSpec {
        VehicleSpec {
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
        }
    }

    fn sim() -> Sim {
        let mut s = Sim::empty(3);
        s.heightfield = Some(flat_ground());
        s
    }

    /// §3's falloff on d' = d − w/2 with the disc's usual Inner 0: full
    /// damage within w/2 of the centre, linear to 0 at Outer + w/2.
    #[test]
    fn falloff_is_linear_on_the_sphere_surface_distance() {
        assert_eq!(blast_falloff(0.5, 1.0, 0.0, 20.0), 1.0 - 0.0 / 20.0);
        assert_eq!(blast_falloff(0.4, 1.0, 0.0, 20.0), 1.0);
        assert_eq!(blast_falloff(10.5, 1.0, 0.0, 20.0), 0.5);
        assert_eq!(blast_falloff(20.5, 1.0, 0.0, 20.0), 0.0);
        // The rep fighter tank's 4.5–6.5: flat to 4.5, then 2 m of ramp.
        assert_eq!(blast_falloff(4.0, 0.0, 4.5, 6.5), 1.0);
        assert_eq!(blast_falloff(5.5, 0.0, 4.5, 6.5), 0.5);
        assert_eq!(blast_falloff(6.5, 0.0, 4.5, 6.5), 0.0);
    }

    /// A standing trooper 10 m from an AT-ST's blast: centre 1 m up, w
    /// 1.4626 → d' = √101 − 0.7313, f = (20 − d')/20, damage 50·f.
    #[test]
    fn a_soldier_takes_the_falloff_of_its_stance_sphere() {
        let mut s = sim();
        let t = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(10.0, 0.0, 0.0)))
            .unwrap();
        let hp = s.unit(t).unwrap().health.cur;
        s.explode(&atst_exp(), Vec3::ZERO, None, Some(0));
        let d = 101f32.sqrt() - 1.462_635 * 0.5;
        let want = 50.0 * (20.0 - d) / 20.0;
        let got = hp - s.unit(t).unwrap().health.cur;
        assert!((got - want).abs() < 1e-3, "{got} vs {want}");
        // Prone, the same spot: centre 0.35 up, w 1.6221 — closer, more.
        s.units[t.index as usize].health.cur = hp;
        s.units[t.index as usize].soldier.stance = Stance::Prone;
        s.explode(&atst_exp(), Vec3::ZERO, None, Some(0));
        let d = (100.0f32 + 0.35 * 0.35).sqrt() - 1.622_066 * 0.5;
        let want = 50.0 * (20.0 - d) / 20.0;
        let got = hp - s.unit(t).unwrap().health.cur;
        assert!((got - want).abs() < 1e-3, "{got} vs {want}");
    }

    /// `rep_mace_groundcrack_exp` (HurtOwner 0, Damage 400, DamageRadius 8,
    /// Push 8 / PushRadius 7.5): B2 FUN_0030c8f8 leaves out the blast's
    /// owner when HurtOwner is clear (B2:300612-300633), for the damage and
    /// the push. Everyone else in reach, his own side included, is handled
    /// as before. Control: the same blast with HurtOwner 1 hits and throws
    /// him (the playtest's Mace killed by his own jump attack, 2026-10-30).
    #[test]
    fn a_hurt_owner_0_blast_leaves_out_its_owner_only() {
        for hurt_owner in [false, true] {
            let mut s = sim();
            let owner = s.spawn(&UnitSpec::trooper(1, Vec3::ZERO)).unwrap();
            let enemy = s
                .spawn(&UnitSpec::trooper(0, Vec3::new(2.0, 0.0, 0.0)))
                .unwrap();
            let ally = s
                .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 2.0)))
                .unwrap();
            for h in [owner, enemy, ally] {
                s.units[h.index as usize].health.max = 2000.0;
                s.units[h.index as usize].health.cur = 2000.0;
            }
            let e = ExplosionClass {
                damage: 400.0,
                damage_inner: 0.0,
                damage_outer: 8.0,
                push: 8.0,
                push_outer: 7.5,
                hurt_owner,
                ..Default::default()
            };
            s.explode(&e, Vec3::Y * 0.2, Some(owner), Some(1));
            let o = s.unit(owner).unwrap();
            if hurt_owner {
                assert!(o.health.cur < 2000.0, "control: HurtOwner 1 hurts him");
                assert!(o.soldier.stance.knocked(), "control: and throws him");
            } else {
                assert_eq!(o.health.cur, 2000.0, "no damage to the owner");
                assert!(!o.soldier.stance.knocked(), "no push on the owner");
                assert_eq!(o.soldier.vel, Vec3::ZERO, "no push on the owner");
            }
            assert!(
                s.unit(enemy).unwrap().health.cur < 2000.0,
                "the enemy is hit"
            );
            assert!(s.unit(enemy).unwrap().soldier.stance.knocked());
            assert!(
                s.unit(ally).unwrap().soldier.stance.knocked(),
                "an ally is still pushed"
            );
        }
    }

    /// Terrain blocks a blast: a trooper behind a ridge takes nothing.
    #[test]
    fn terrain_blocks_the_blast() {
        let mut s = Sim::empty(3);
        // A 30 m wall of terrain between x = 2 and x = 6 (4 m cells).
        let mut hf = flat_ground();
        hf.size = 1001;
        hf.unit = 4.0;
        hf.heights = vec![0.0; 1001 * 1001];
        for z in 0..1001 {
            hf.heights[z * 1001 + 501] = 30.0;
        }
        s.heightfield = Some(hf);
        let far = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(10.0, 0.0, 0.0)))
            .unwrap();
        let near = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(-3.0, 0.0, 0.0)))
            .unwrap();
        let hp = s.unit(far).unwrap().health.cur;
        s.explode(&atst_exp(), Vec3::new(-1.0, 0.5, 0.0), None, Some(0));
        assert_eq!(s.unit(far).unwrap().health.cur, hp, "behind the ridge");
        assert!(s.unit(near).unwrap().health.cur < hp, "in the open");
    }

    /// Riders of a vehicle take no splash (§5); the vehicle's hull does,
    /// through its HealthType's scale — 0 for the tank class (§7).
    #[test]
    fn riders_skip_and_the_hull_takes_its_scale() {
        let mut s = sim();
        s.blast_setup = vec![Some(BlastSetup {
            explosion: None,
            sphere_centre: Vec3::new(0.0, 1.0, 0.0),
            sphere_radius: 3.1467,
        })];
        let v = s.spawn_vehicle(&bike(Vec3::new(3.0, 0.0, 0.0)));
        let rider = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(3.0, 0.0, 0.0)))
            .unwrap();
        assert!(s.seat_unit(rider, v, 0));
        let hp = s.unit(rider).unwrap().health.cur;
        s.explode(&atst_exp(), Vec3::ZERO, None, Some(0));
        assert_eq!(s.unit(rider).unwrap().health.cur, hp, "a rider is spared");
        // d = √10, w 3.1467: d' = 1.589, f = 0.9206; 500 − 46.03.
        let d = 10f32.sqrt() - 3.1467 * 0.5;
        let want = 500.0 - 50.0 * (20.0 - d) / 20.0;
        let got = s.vehicle(v).unwrap().health.cur;
        assert!((got - want).abs() < 1e-3, "{got} vs {want}");
        s.explode(&tank_exp(), Vec3::ZERO, None, Some(0));
        assert_eq!(s.vehicle(v).unwrap().health.cur, got, "VehicleScale 0");
    }

    /// At most 20 are gathered, handled from the end of the list: of 25
    /// troopers in slot order, the last five are out of the cap.
    #[test]
    fn the_gather_stops_at_twenty() {
        let mut s = sim();
        let hs: Vec<_> = (0..25)
            .map(|i| {
                let a = i as f32 * 0.25;
                s.spawn(&UnitSpec::trooper(
                    1,
                    Vec3::new(a.cos() * 2.0, 0.0, a.sin() * 2.0),
                ))
                .unwrap()
            })
            .collect();
        let hp = s.unit(hs[0]).unwrap().health.cur;
        s.explode(&atst_exp(), Vec3::ZERO, None, Some(0));
        let hit = hs
            .iter()
            .filter(|&&h| s.unit(h).unwrap().health.cur < hp)
            .count();
        assert_eq!(hit, BLAST_GATHER_MAX);
        assert!(s.unit(hs[24]).unwrap().health.cur == hp);
    }

    #[test]
    fn ground_blast_gather_filters_before_filling_slots() {
        use crate::soldier::GroundRules;

        let e = ExplosionClass {
            damage: 50.0,
            damage_outer: 6.5,
            ..ExplosionClass::default()
        };
        let (lift, radius) = soldier_blast_sphere(Stance::Stand);
        let reach = e.gather_radius() + radius;
        let diagonal = reach * 0.8;
        let corner = Vec3::new(diagonal, 0.0, diagonal);
        let mixed_corner = e.gather_radius() + radius * 0.9;
        let vertical = Vec3::new(diagonal, diagonal, 0.0);
        let mixed_corner = Vec3::new(mixed_corner, mixed_corner, 0.0);
        for (rules, space, lift_to_blast, offset, hit) in [
            (GroundRules::Bf1, false, lift, corner, true),
            (GroundRules::Bf2, false, lift, corner, true),
            (GroundRules::Bf2, false, lift, vertical, false),
            (GroundRules::Bf2, false, lift, mixed_corner, true),
            (GroundRules::Bf2, false, lift, Vec3::X * reach, false),
            (GroundRules::Bf2, false, lift, Vec3::Y * reach, false),
            (
                GroundRules::Bf2,
                false,
                28.705_603,
                Vec3::Y * 7.962_637,
                false,
            ),
            (GroundRules::Bf2, true, lift, corner, false),
            (GroundRules::Bf2, true, lift, mixed_corner, false),
        ] {
            let mut s = sim();
            s.ground_rules = rules;
            if space {
                s.space_assault = Some(crate::sim::SpaceAssault::new(10));
            }
            let blast = Vec3::Y * lift_to_blast;
            let feet = blast - Vec3::Y * lift;
            for _ in 0..BLAST_GATHER_MAX {
                let at = feet + offset;
                let decoy = s.spawn(&UnitSpec::trooper(1, at)).unwrap();
                // Preserve vertical separation after the spawn grounds the unit.
                s.unit_mut(decoy).unwrap().soldier.pos.y = at.y;
            }
            let at = feet + Vec3::X * 2.0;
            let target = s.spawn(&UnitSpec::trooper(1, at)).unwrap();
            s.unit_mut(target).unwrap().soldier.pos.y = at.y;
            let hp = s.unit(target).unwrap().health.cur;
            s.explode(&e, blast, None, Some(0));
            assert_eq!(
                s.unit(target).unwrap().health.cur < hp,
                hit,
                "{rules:?} space={space} blast={blast} crowd offset={offset}"
            );
        }
    }

    /// §4's team rules: a blast of team 0 whose attacker is a bot spares a
    /// human of team 0 entirely, scales a bot of team 0 by the friendly-fire
    /// scale, and hits team 1 in full; a kill credits the attacker.
    #[test]
    fn team_rules_and_the_kill_credit() {
        let mut s = sim();
        s.friendly_fire_scale = 0.5;
        let bot = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 30.0)))
            .unwrap();
        let human = s
            .spawn(&UnitSpec::player(0, Vec3::new(1.0, 0.0, 0.0)))
            .unwrap();
        let mate = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(-1.0, 0.0, 0.0)))
            .unwrap();
        let foe = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 1.0)))
            .unwrap();
        let hp = s.unit(mate).unwrap().health.cur;
        s.explode(&atst_exp(), Vec3::ZERO, Some(bot), Some(0));
        assert_eq!(
            s.unit(human).unwrap().health.cur,
            hp,
            "AI blast, human mate"
        );
        // d = √2, w/2 = 0.7313 → f = (20 − 0.6829)/20.
        let f = (20.0 - (2f32.sqrt() - 1.462_635 * 0.5)) / 20.0;
        let mate_hit = hp - s.unit(mate).unwrap().health.cur;
        assert!((mate_hit - 25.0 * f).abs() < 1e-3, "{mate_hit}");
        let foe_hit = hp - s.unit(foe).unwrap().health.cur;
        assert!((foe_hit - 50.0 * f).abs() < 1e-3, "{foe_hit}");
        // Enough blasts kill the foe: the kill is the bot's.
        while s.unit(foe).is_some() {
            s.explode(&atst_exp(), Vec3::ZERO, Some(bot), Some(0));
        }
        assert!(s.events.contains(&Event::Kill {
            shooter: bot,
            victim: foe
        }));
        assert!(s.unit(bot).unwrap().kills >= 1);
    }

    /// A blast's hit says so: its age is [`BLAST_AGE`], never the 0 of a
    /// bolt that struck in its first update.
    #[test]
    fn a_blast_hit_carries_the_blast_age() {
        let mut s = sim();
        let bot = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 30.0)))
            .unwrap();
        let foe = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 1.0)))
            .unwrap();
        s.explode(&atst_exp(), Vec3::ZERO, Some(bot), Some(0));
        let ages: Vec<f32> = s
            .events
            .iter()
            .filter_map(|e| match *e {
                Event::Hit { victim, age, .. } if victim == foe => Some(age),
                _ => None,
            })
            .collect();
        assert_eq!(ages, [crate::sim::BLAST_AGE]);
    }

    /// The whole chain: a destroyed vehicle's class explosion hits a trooper
    /// beside it — its pilot dies with it (and is credited), the blast is
    /// the pilot's on the vehicle's team; an empty vehicle's blast kills
    /// with no attacker ([`DeathCause::Blast`]).
    #[test]
    fn a_destroyed_vehicle_detonates_its_class() {
        let mut s = sim();
        s.blast_setup = vec![Some(BlastSetup {
            explosion: Some(ExplosionClass {
                damage: 1000.0,
                damage_outer: 10.0,
                ..Default::default()
            }),
            sphere_centre: Vec3::ZERO,
            sphere_radius: 3.0,
        })];
        let v = s.spawn_vehicle(&bike(Vec3::ZERO));
        let pilot = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
        assert!(s.seat_unit(pilot, v, 0));
        let foe = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(4.0, 0.0, 0.0)))
            .unwrap();
        s.destroy_vehicle(v, None);
        assert!(s.unit(foe).is_none(), "the blast killed the bystander");
        assert!(s.events.contains(&Event::Kill {
            shooter: pilot,
            victim: foe
        }));

        let v = s.spawn_vehicle(&bike(Vec3::ZERO));
        let other = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(4.0, 0.0, 0.0)))
            .unwrap();
        s.destroy_vehicle(v, None);
        assert!(s.events.contains(&Event::EnvDeath {
            unit: other,
            cause: DeathCause::Blast
        }));
    }

    // ── Card KNOCK (`analysis/re-knockback.md`; PS2 rows in
    //    `lead_tmp/knock/out/`) ─────────────────────────────────────────────

    /// BF1's thermal detonator (`push_table.txt`): Damage 320 over 4.5-6.5,
    /// Push 10 over PushRadius 4.5-4.5.
    fn thermal() -> ExplosionClass {
        ExplosionClass {
            damage: 320.0,
            damage_inner: 4.5,
            damage_outer: 6.5,
            push: 10.0,
            push_outer: 4.5,
            ..Default::default()
        }
    }

    fn sim_for(rules: crate::soldier::GroundRules) -> Sim {
        let mut s = sim();
        s.ground_rules = rules;
        s
    }

    fn vel(s: &Sim, h: UnitHandle) -> Vec3 {
        s.units[h.index as usize].soldier.vel
    }

    /// The push a standing soldier at `feet` gets from `e` at `blast` under
    /// the port's rule: the PS2's (normalize + lift) x Push, x
    /// [`crate::soldier::KNOCK_PUSH_SCALE`] x [`knock_falloff`].
    fn port_push(e: &ExplosionClass, feet: Vec3, stance: Stance, blast: Vec3) -> Vec3 {
        let (lift, w) = soldier_blast_sphere(stance);
        let d = feet + Vec3::Y * lift - blast;
        let f = knock_falloff(d.length(), w, e.push_outer);
        (d.normalize() + Vec3::Y * crate::soldier::KNOCK_LIFT)
            * e.push
            * crate::soldier::KNOCK_PUSH_SCALE
            * f
    }

    /// The falloff: whole at the blast, half halfway, none at PushRadius
    /// (measured from the body's near side).
    #[test]
    fn knock_falloff_fades_to_the_push_radius() {
        assert_eq!(knock_falloff(0.5, 1.0, 4.0), 1.0);
        assert!((knock_falloff(2.5, 1.0, 4.0) - 0.5).abs() < 1e-6);
        assert_eq!(knock_falloff(4.5, 1.0, 4.0), 0.0);
        assert_eq!(knock_falloff(9.0, 1.0, 4.0), 0.0);
        assert_eq!(knock_falloff(1.0, 1.0, 0.0), 0.0);
    }

    /// One grenade, three soldiers in a line: the nearest flies hardest,
    /// the next less, the one past PushRadius not at all (the maintainer 2026-10-27,
    /// on BF1 and BF2 maps alike).
    #[test]
    fn knock_the_farther_the_softer_on_both_games() {
        for rules in [
            crate::soldier::GroundRules::Bf1,
            crate::soldier::GroundRules::Bf2,
        ] {
            let mut s = sim_for(rules);
            let at = [1.5, 3.0, 6.5].map(|x| {
                s.spawn(&UnitSpec::trooper(1, Vec3::new(x, 0.0, 0.0)))
                    .unwrap()
            });
            let e = ExplosionClass {
                damage: 0.0,
                damage_outer: 0.0,
                ..thermal()
            };
            s.explode(&e, Vec3::ZERO, None, Some(0));
            let [near, mid, far] = at.map(|h| vel(&s, h).length());
            assert!(near > mid && mid > 0.0, "{rules:?}: {near} {mid}");
            assert_eq!(far, 0.0, "{rules:?}: outside the push radius");
        }
    }

    /// `bf1_hot1_knock.json` trial 2, P (§6): feet 2.01 m from the blast,
    /// which is 0.12 m above them: the PS2's dv (9.13, 10.01, 0.76) and
    /// thrown. The port pushes the same way, [`port_push`] as strong, and
    /// still throws him. The same soldier 7.5 m off, one tumbling and a
    /// rider: nothing.
    /// The Bulldog's rule (the maintainer 2026-10-27, `ExplosionClass::spares_living`):
    /// the blast still hurts a living soldier but does not throw him; the same
    /// blast without the flag does.
    #[test]
    fn a_push_that_spares_the_living_hurts_but_does_not_throw() {
        for spares in [false, true] {
            let mut s = sim_for(crate::soldier::GroundRules::Bf2);
            // 50 damage: he lives through it (a corpse is pushed either way).
            let e = ExplosionClass {
                spares_living: spares,
                damage: 50.0,
                ..thermal()
            };
            let p = s
                .spawn(&UnitSpec::trooper(1, Vec3::new(2.0, 0.0, 0.0)))
                .unwrap();
            let hp = s.units[p.index as usize].health.cur;
            s.explode(&e, Vec3::new(0.0, 0.12, 0.0), None, Some(0));
            let u = &s.units[p.index as usize];
            assert!(u.health.cur < hp, "spares {spares}: the blast hurts");
            if spares {
                assert_eq!(u.soldier.stance, Stance::Stand);
                assert_eq!(vel(&s, p), Vec3::ZERO, "not thrown");
            } else {
                assert_eq!(u.soldier.stance, Stance::Thrown);
            }
        }
    }

    #[test]
    fn knock_bf1_pushes_the_living_and_skips_the_knocked() {
        let mut s = sim_for(crate::soldier::GroundRules::Bf1);
        let e = ExplosionClass {
            damage: 0.0,
            damage_outer: 0.0,
            ..thermal()
        };
        let p = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(2.0015, 0.0, 0.167)))
            .unwrap();
        let far = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(-7.5, 0.0, 0.0)))
            .unwrap();
        let tum = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -2.0)))
            .unwrap();
        s.units[tum.index as usize].soldier.stance = Stance::Tumble;
        let blast = Vec3::new(0.0, 0.12, 0.0);
        let feet = s.units[p.index as usize].soldier.pos;
        s.explode(&e, blast, None, Some(0));
        let ps2 = Vec3::new(9.13, 10.01, 0.76);
        let want = port_push(&e, feet, Stance::Stand, blast);
        assert!(want.normalize().dot(ps2.normalize()) > 0.9999);
        assert!(want.length() < ps2.length());
        assert!(
            (vel(&s, p) - want).abs().max_element() < 1e-4,
            "{:?}",
            vel(&s, p)
        );
        assert_eq!(s.units[p.index as usize].soldier.stance, Stance::Thrown);
        assert_eq!(vel(&s, far), Vec3::ZERO, "outside PushRadius + w/2");
        assert_eq!(vel(&s, tum), Vec3::ZERO, "a tumbling soldier is skipped");
        assert_eq!(s.nav_stats.knocks, 1);
    }

    /// A soldier the blast kills (§3, B1 of trial 2): pushed and kicked while
    /// still standing, so thrown; his death (the dead state, the item drop)
    /// waits for the landing and the get-up - 2.5-3.6 s on the PS2. The kill
    /// itself is at the blast.
    #[test]
    fn knock_bf1_the_blast_killed_fly_and_die_at_the_landing() {
        let mut s = sim_for(crate::soldier::GroundRules::Bf1);
        let b = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(1.5, 0.0, 0.0)))
            .unwrap();
        s.units[b.index as usize].health.cur = 1.0;
        s.explode(&thermal(), Vec3::new(0.0, 0.12, 0.0), None, Some(0));
        let u = &s.units[b.index as usize];
        assert!(!u.alive && u.body && u.drop_due);
        assert_eq!(u.soldier.stance, Stance::Thrown);
        // The port's push + kick 5 x normalize(feet - blast).
        let blast = Vec3::new(0.0, 0.12, 0.0);
        let k = Vec3::new(1.5, -0.12, 0.0).normalize() * 5.0;
        let want = port_push(&thermal(), Vec3::new(1.5, 0.0, 0.0), Stance::Stand, blast) + k;
        assert!((u.soldier.vel - want).abs().max_element() < 1e-4);
        let mut ticks = 0;
        while s.units[b.index as usize].soldier.stance != Stance::Dead {
            let st = s.units[b.index as usize].soldier.stance;
            s.step(&[]);
            ticks += 1;
            if s.units[b.index as usize].soldier.stance == Stance::Dead {
                assert_eq!(st, Stance::Stand, "dies standing, after the get-up");
            }
            assert!(ticks < 200);
        }
        let secs = ticks as f32 / 30.0;
        // The PS2 lands him 2.5-3.6 s later; the port's softer push sooner.
        assert!((1.5..2.5).contains(&secs), "{secs} s");
        assert!(!s.units[b.index as usize].drop_due, "the drop came with it");
    }

    /// `bf1_hot1_knock.json` trial 2, C1 (§3 [M]): a corpse 0.9 s dead 2.35 m
    /// from the blast takes no damage here and gets push + kick (PS2 (-14.91,
    /// 7.22, 0.00); the port's push is [`port_push`]); its body slides and
    /// leaves the world 4.0 s into the dead state.
    #[test]
    fn knock_bf1_a_corpse_is_pushed_kicked_and_gone_at_four_seconds() {
        let mut s = sim_for(crate::soldier::GroundRules::Bf1);
        let c = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(-2.3535, 0.0, 0.0)))
            .unwrap();
        s.unit_killed(c, None);
        let u = &mut s.units[c.index as usize];
        u.soldier.stance = Stance::Dead;
        u.soldier.time_in_stance = 0.9;
        let hp = u.health.cur;
        let blast = Vec3::new(0.0, 0.04, 0.0);
        let feet = s.units[c.index as usize].soldier.pos;
        s.explode(&thermal(), blast, None, Some(0));
        let k = (feet - blast).normalize() * crate::soldier::DEATH_KICK;
        let want = port_push(&thermal(), feet, Stance::Dead, blast) + k;
        assert!(
            (vel(&s, c) - want).abs().max_element() < 1e-4,
            "{:?}",
            vel(&s, c)
        );
        assert_eq!(s.units[c.index as usize].health.cur, hp);
        assert_eq!(s.units[c.index as usize].soldier.stance, Stance::Dead);
        let mut ticks = 0;
        while s.units[c.index as usize].body {
            s.step(&[]);
            ticks += 1;
            assert!(ticks < 200);
        }
        let dead_for = 0.9 + ticks as f32 / 30.0;
        assert!((dead_for - 4.0).abs() <= 1.5 / 30.0, "{dead_for}");
        let x = s.units[c.index as usize].soldier.pos.x;
        // PS2 past 10 m; the port's softer push about 7 m.
        assert!(x < -5.0, "it slid away: {x}");
        // Out of the world: the next blast neither gathers nor moves it.
        let v = vel(&s, c);
        s.explode(&thermal(), Vec3::new(x + 1.0, 0.0, 0.0), None, Some(0));
        assert_eq!(vel(&s, c), v);
    }

    /// BF2 (§4): on the PS2 a thermal detonator (PushDeadOnly by default)
    /// hurts the living and pushes none, and a class that authors
    /// PushDeadOnly 0 throws him - `bf2_hot1flag_knock.json`, P: (-4.80,
    /// 8.71, -8.34). The port throws the living on every map: that way,
    /// [`port_push`] as strong.
    #[test]
    fn knock_bf2_pushes_the_living_too() {
        let feet = Vec3::new(3.031, -0.0199, -7.4046) - Vec3::new(4.5896, 0.1, -4.6986);
        let blast = Vec3::new(0.0, -feet.y, 0.0);
        let at = Vec3::new(feet.x, 0.0, feet.z);
        let bf2 = ExplosionClass {
            damage: 450.0,
            damage_inner: 4.5,
            damage_outer: 6.5,
            push: 10.0,
            push_outer: 6.5,
            ..Default::default()
        };
        let mut s = sim_for(crate::soldier::GroundRules::Bf2);
        let p = s.spawn(&UnitSpec::trooper(1, at)).unwrap();
        s.units[p.index as usize].health.cur = 30000.0;
        s.explode(&bf2, blast, None, Some(0));
        assert!(s.units[p.index as usize].health.cur < 30000.0, "damaged");
        let ps2 = Vec3::new(-4.80, 8.71, -8.34);
        let want = port_push(&bf2, at, Stance::Stand, blast);
        assert!(want.normalize().dot(ps2.normalize()) > 0.9999);
        assert!(
            (vel(&s, p) - want).abs().max_element() < 1e-4,
            "{:?}",
            vel(&s, p)
        );
        assert_eq!(s.units[p.index as usize].soldier.stance, Stance::Thrown);
    }
}
