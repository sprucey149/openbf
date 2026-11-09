//! What a bolt does when it lands (`resolve_bolt`, step 5: damage through the
//! id guard, the kill and its ticket, then its ordnance's impact explosion -
//! card RKEX, the rocket launchers) and the one environmental death path,
//! [`Sim::kill_environment`] (a mission death region, step 3c, or a lethal
//! landing, step 3). The bolt, cheat, damage and environmental-death tests
//! live here; the bolts themselves are fired inline in `step` (step 4).

use super::state::{DeathCause, Event, Sim};
use crate::collision::{HullFilter, Layers};
use crate::droideka::{self, Sphere};
use crate::impact::Surface;
use crate::projectile::{Bolt, Hit, Target};
use crate::soldier::Stance;
use crate::unit::{Controller, Unit, UnitHandle};
use crate::vehicle::VehicleHandle;
use crate::weapon::DamageDesc;
use crate::TICK_HZ;
use glam::Vec3;

// Only for `kill_environment`'s docs, which link `LETHAL_LANDING_SPEED`.
#[cfg(doc)]
use super::state::LETHAL_LANDING_SPEED;

/// The head prim's HitLocation factor (see `Sim::hit_scale`): BF2's hard-coded
/// default {head prim, x3.0}, `FUN_002517e8` (`analysis/re-ai-difficulty.md` §9.2).
pub const HEADSHOT_SCALE: f32 = 3.0;

const LARGE_SHIELD_RADIUS_SQ: f32 = 256.0;

impl Sim {
    /// The PS2's per-frame bolt sweep (B1 FUN_00273128, BF2 FUN_0030fad0;
    /// `re-ordnance-kinds.md` §2.4 [V][M]; issue #41: the port had kept the
    /// spawn ray's target): what each live bolt passes through on this
    /// tick's flight - the soldiers but its shooter (their
    /// bolt prims, `Unit::bolt_hit`) and the hulls its `Bolt::hulls` admits. The nearest such
    /// hit short of where the flight ends becomes the bolt's hit, so it lands
    /// this tick. The map and the terrain were asked at the press: they do
    /// not move. A rider of a vehicle whose hull the bolt passes is passed
    /// too (a gun's own crew), and so is any rider with no body (an enclosed
    /// seat, `SeatSetup::body_active`). Cost: one prim list a tick, a box test per bolt and
    /// soldier, and a hull-only ray per bolt. A human's bolt meets each soldier
    /// [`crate::unit::HUMAN_SHOT_LAG`] back along his velocity, a fast
    /// round its age further (OURS: where the window drew him,
    /// [`crate::unit::human_meet_lag`]).
    pub(super) fn sweep_bolts(&mut self, dt: f32) {
        if self.bolts.is_empty() {
            return;
        }
        self.sweep_fat_rays(dt);
        type Body = (
            UnitHandle,
            Option<VehicleHandle>,
            crate::unit::BoltPrims,
            Vec3,
            Vec3,
            Vec3,
        );
        type Deka = (UnitHandle, Vec<Sphere>, Option<Sphere>, Vec3);
        let dekas: Vec<Deka> = self
            .alive_units()
            .filter(|(_, u)| u.droideka.is_some())
            .map(|(h, u)| {
                let back = -u.soldier.vel * crate::unit::HUMAN_SHOT_LAG;
                (h, u.droideka_spheres(), u.droideka_shield(), back)
            })
            .collect();
        let units: Vec<Body> = self
            .alive_units()
            .filter(|(_, u)| u.droideka.is_none())
            .filter_map(|(h, u)| {
                // A rider is met only in an exposed seat, where he sits and
                // moving as his vehicle does; an enclosed one has no body.
                let p = self.hero_body_prims(h.index as usize)?;
                let (min, max) = p.bounds();
                let rides = u.mount.map(|m| m.vehicle);
                let vel = rides
                    .and_then(|v| self.vehicle(v))
                    .map_or(u.soldier.vel, |v| v.vel);
                Some((h, rides, p, min, max, vel))
            })
            .collect();
        let (world, bolts, classes) = (&self.collision, &mut self.bolts, &self.weapon_classes);
        for b in bolts.iter_mut() {
            let ord = &classes[b.weapon_class].ordnance;
            if ord.fat_ray.is_some() || (ord.is_tracer() && ord.explosion.is_none()) {
                continue;
            }
            let step = dt.min(b.remaining.max(0.0));
            let speed = b.vel.length();
            if step <= 0.0 || speed <= 0.0 {
                continue;
            }
            let dir = b.vel / speed;
            let mut reach = speed * step;
            let (from, to) = (b.pos, b.pos + dir * reach);
            let (lo, hi) = (from.min(to), from.max(to));
            let mut hit = None;
            let crew = match b.hulls {
                HullFilter::Except(s) | HullFilter::HullsExcept(s) => s,
                _ => [None; 2],
            };
            let mut mult = 1.0;
            let lag = crate::unit::human_meet_lag(speed, b.age);
            for (h, rides, prims, min, max, vel) in &units {
                let by = if b.shooter_is_player {
                    -*vel * lag
                } else {
                    Vec3::ZERO
                };
                if *h == b.owner
                    || (rides.is_some() && crew.contains(rides))
                    || (*max + by).cmplt(lo).any()
                    || (*min + by).cmpgt(hi).any()
                {
                    continue;
                }
                if let Some((t, zone)) = prims.moved(by).hit(from, dir, reach) {
                    reach = t;
                    hit = Some(Target::Unit(*h));
                    mult = if zone == crate::unit::HitZone::Head {
                        HEADSHOT_SCALE
                    } else {
                        1.0
                    };
                }
            }
            for (h, spheres, shield, back) in &dekas {
                if *h == b.owner {
                    continue;
                }
                let by = if b.shooter_is_player {
                    *back
                } else {
                    Vec3::ZERO
                };
                let moved = |s: &Sphere| Sphere {
                    center: s.center + by,
                    radius: s.radius,
                };
                if let Some(t) = shield
                    .as_ref()
                    .and_then(|s| droideka::ray_sphere(from, dir, reach, moved(s)))
                {
                    reach = t;
                    hit = Some(Target::Shield(*h));
                    mult = 1.0;
                }
                for s in spheres {
                    if let Some(t) = droideka::ray_sphere(from, dir, reach, moved(s)) {
                        reach = t;
                        hit = Some(Target::Unit(*h));
                        mult = 1.0;
                    }
                }
            }
            let mines = self.mines.iter().filter(|m| m.health > 0.0);
            let droids = self
                .droids
                .iter()
                .filter(|d| d.owner.index != b.owner.index);
            let turrets = self
                .turrets
                .iter()
                .filter(|t| t.owner.index != b.owner.index);
            let items = mines
                .map(|m| (Target::Mine(m.id), m.flight.pos, crate::mine::RADIUS))
                .chain(droids.map(|d| (Target::Droid(d.id), d.pos, crate::remote::RADIUS)))
                .chain(
                    turrets.map(|t| (Target::Turret(t.id), t.centre(), crate::autoturret::RADIUS)),
                );
            for (target, at, radius) in items {
                if let Some(t) =
                    crate::unit::ray_sphere(from, dir, at, radius).filter(|&t| t <= reach)
                {
                    reach = t;
                    hit = Some(target);
                    mult = 1.0;
                }
            }
            for r in self
                .guided
                .iter()
                .filter(|r| r.owner.index != b.owner.index)
            {
                if let Some(t) = crate::unit::ray_sphere(
                    from,
                    dir,
                    r.pos,
                    crate::guided::COLLISION_RADIUS_PROVISIONAL,
                )
                .filter(|&t| t <= reach)
                {
                    reach = t;
                    hit = Some(Target::Guided(r.id));
                    mult = 1.0;
                }
            }
            if let Some(filter) = b.hulls.hulls_only() {
                if let Some(r) = world.ray_with(from, dir, reach, Layers::ORDNANCE, filter) {
                    if let Some(v) = r.owner.filter(|_| r.t < reach) {
                        reach = r.t;
                        hit = Some(Target::Vehicle(v));
                        mult = r.hit_mult;
                    }
                }
            }
            if let Some(target) = hit {
                b.hit = Hit::Target(target);
                b.hit_mult = mult;
                b.end = from + dir * reach;
                b.remaining = reach / speed;
            }
        }
    }

    /// The struck prim's damage factor (issue #41): a soldier's head prim
    /// takes the PS2's default HitLocation x3 (BF2 FUN_002517e8
    /// B2:170444-170450, BF1 FUN_0020e2e0 B1:122725; analysis/re-hitzones.md),
    /// a vehicle prim its class's HitLocation (the x4 critical prims; BF1's
    /// two x0). A factor over 1 is a bot's only on the PS2's top level (BF2
    /// FUN_001e0580 B2:91567-91580: dropped for an AI attacker under level
    /// 3; the port's Hard and Elite - our Normal has the PS2 Elite's base
    /// skill but not its level-3 rules, so it goes without
    /// this). BF1 has no such gate; one rule on both games' maps.
    pub(super) fn bolt_spares(&self, b: &Bolt, victim: &Unit) -> bool {
        self.shot_spares(b.team, b.shooter_is_player, victim)
    }

    /// The rule of [`Self::bolt_spares`] for a shot with no bolt (an emitter).
    pub(super) fn shot_spares(&self, team: u8, shooter_is_player: bool, victim: &Unit) -> bool {
        (victim.is_player && self.cheats.invincible)
            || (victim.is_human() && self.allied(team, victim.team) && !shooter_is_player)
    }

    pub(super) fn bolt_desc(&self, b: &Bolt) -> DamageDesc {
        let ord = &self.weapon_classes[b.weapon_class].ordnance;
        let mut desc = ord.damage_desc_charged(b.age, false, b.charge);
        if let Some(beam) = &ord.beam {
            desc.damage *= beam.share(crate::TICK_DT);
        }
        desc
    }

    /// Card DPRX: a proxy is a human victim (no cheats: below).
    /// A bolt of a unit that is not a human player does nothing to
    /// a human player of its own team (B1 FUN_001bc190, B2
    /// FUN_001dfb38: TakeDamage returns before the scale;
    /// re-blaster-damage.md section 6.3) - as a blast, `blast_unit`.
    pub(super) fn bolt_strikes_unit(
        &mut self,
        b: &Bolt,
        h: UnitHandle,
        desc: &DamageDesc,
        scale: f32,
    ) -> Option<f32> {
        let victim = self.unit(h)?;
        let spared = self.bolt_spares(b, victim);
        let friendly = self.allied(b.team, victim.team);
        let ff = self.friendly_fire_scale;
        let mult = self.buff_scale(Some(b.owner), h) * scale;
        // Native fat-ray TakeDamage uses flags -1 (shield and health);
        // a bolt that hits the body from inside the bubble keeps flags 2.
        let area = self.weapon_classes[b.weapon_class]
            .ordnance
            .fat_ray
            .is_some();
        let v = self.unit_mut(h)?;
        let damage = if spared {
            0.0
        } else if area {
            v.lend_droideka_shield(|health| health.take_damage(desc, mult, friendly, ff))
        } else {
            v.health.take_damage(desc, mult, friendly, ff)
        };
        if self
            .units
            .get(b.owner.index as usize)
            .filter(|s| s.gen == b.owner.gen)
            .is_some_and(|s| s.controller == Controller::Ai)
        {
            self.nav_stats.ai_hits += 1;
        }
        self.nav_stats.bolt_hits += 1;
        if friendly && b.owner != h {
            self.nav_stats.friend_hits += 1;
        }
        Some(damage)
    }

    fn hit_scale(&self, b: &Bolt) -> f32 {
        // [ours] A proxy with `headshots` counts its head hits as a player
        // does (its slot keeps the flag through deaths).
        let proxy_head = self
            .units
            .get(b.owner.index as usize)
            .and_then(|u| u.proxy_human)
            .is_some_and(|p| p.headshots);
        let bot_gate = !b.shooter_is_player
            && !proxy_head
            && self.difficulty_of(b.team) < crate::skill::Difficulty::Hard;
        if b.hit_mult > 1.0 && bot_gate {
            1.0
        } else {
            b.hit_mult
        }
    }

    /// Impact handler (BF1 FUN_00276810 → TakeDamage FUN_001bc190; BF2
    /// FUN_00313c40 → FUN_001dfb38): damage only if the remembered target is
    /// still the same live entity (id guard).
    pub(super) fn resolve_bolt(&mut self, b: &Bolt) {
        match b.hit {
            Hit::Target(Target::Unit(h)) => {
                if self.deflect_bolt(h, b) {
                    return;
                }
                if self.unit(h).is_none() {
                    return; // died in flight → bolt dies, no impact
                }
                // Card TOKN: a bot without one of this human's tokens aimed
                // beside him; a bolt that meets him anyway is real (counted).
                if crate::ai::tokn::bars(self, b.owner, h) {
                    self.nav_stats.tokn_stray += 1;
                    if self
                        .unit(b.owner)
                        .is_some_and(|o| o.ai.target != Some(h.into()))
                    {
                        self.nav_stats.tokn_cross += 1;
                    }
                }
                let desc = self.bolt_desc(b);
                // No tier multiplies a bot's bolt into a human (card DRETIRE,
                // The maintainer 2026-10-28: D12's ×1.15 / ×1.3 retired).
                let scale = self.hit_scale(b);
                let damage = self.bolt_strikes_unit(b, h, &desc, scale).unwrap_or(0.0);
                if scale > 1.0 && damage > 0.0 {
                    self.events.push(Event::CriticalHit {
                        shooter: b.owner,
                        victim: Target::Unit(h),
                        weapon_class: b.weapon_class,
                    });
                }
                self.push_impact(b, Surface::Soldier, None);
                if self.finish_direct_damage(
                    h,
                    Some(b.owner),
                    damage,
                    b.age,
                    b.vel.normalize_or_zero(),
                ) {
                    // The direct hit's death kick along the shot (B1
                    // FUN_00200c30 -> FUN_00200f90, re-knockback §3).
                    let u = &mut self.units[h.index as usize];
                    if u.body {
                        u.soldier.kick(b.vel);
                    }
                }
                self.bolt_explodes(b);
            }
            Hit::Target(Target::Shield(h)) => {
                let Some(victim) = self.unit(h) else {
                    return;
                };
                let spared = self.bolt_spares(b, victim);
                let ff = if self.allied(b.team, victim.team) {
                    self.friendly_fire_scale
                } else {
                    1.0
                };
                let desc = self.bolt_desc(b);
                let dmg = desc.damage * self.buff_scale(Some(b.owner), h) * ff;
                let tick = self.tick;
                if let Some(u) = self.unit_mut(h) {
                    if let Some(d) = u.droideka.as_mut().filter(|_| !spared) {
                        d.shield.absorb(dmg, desc.shield_scale);
                    }
                    u.last_hit = Some((b.owner, tick));
                }
                self.nav_stats.bolt_hits += 1;
                self.events.push(Event::BoltExpired {
                    pos: b.end,
                    struck: true,
                });
                self.push_impact(b, Surface::Shield, None);
                self.bolt_explodes(b);
            }
            // The hull pool (re-vehicles §6): the ordnance's damage at this
            // age, scaled by the hull's HealthType (`VehicleScale`). The id
            // guard as for a soldier: a vehicle gone in flight kills the bolt
            // with no impact. The hit strikes (the puff), and it lands before
            // the damage so a killing bolt's puff precedes the blast. The
            // spawn ray picks a hull from the ORDNANCE layer
            // (`Bolt::spawn`: `world.ray_with`).
            // A building turret's bodies are its building's
            // (`Sim::bind_building`): the round strikes a building, as it
            // did when they were plain map bodies, and the turret's pool
            // takes the damage.
            Hit::Target(Target::Vehicle(vh)) => {
                let Some(v) = self.vehicle(vh) else {
                    return;
                };
                let surface =
                    if v.kind == crate::vehicle::VehicleKind::Turret && v.instance.is_some() {
                        Surface::Building
                    } else {
                        Surface::Vehicle
                    };
                let desc = self.bolt_desc(b);
                self.events.push(Event::BoltExpired {
                    pos: b.end,
                    struck: true,
                });
                self.push_impact(b, surface, Some(vh));
                let scale = self.hit_scale(b);
                let damage = self.damage_vehicle_by(
                    vh,
                    &desc,
                    scale,
                    Some(b.owner),
                    Some(b.team),
                    Some(b.vel.normalize_or_zero()),
                );
                // A critical prim's x4 (the weak spot) that took health:
                // the critical hit, as a head's (hitmark).
                if scale > 1.0 && damage > 0.0 {
                    self.events.push(Event::CriticalHit {
                        shooter: b.owner,
                        victim: Target::Vehicle(vh),
                        weapon_class: b.weapon_class,
                    });
                }
                self.bolt_explodes(b);
            }
            Hit::Target(
                t @ (Target::Droid(_) | Target::Mine(_) | Target::Turret(_) | Target::Guided(_)),
            ) => {
                let desc = self.bolt_desc(b);
                self.damage_item(t, desc.damage, &desc.scales, Some(b.team), Some(b.owner));
                self.events.push(Event::BoltExpired {
                    pos: b.end,
                    struck: true,
                });
                self.push_impact(b, Surface::Vehicle, None);
                self.bolt_explodes(b);
            }
            // A model wall ends the flight exactly as the terrain sheet does:
            // the same event, so the same puff and the same impact sound.
            Hit::Target(Target::Station(s)) => {
                let Some(st) = self.stations.get(usize::from(s)) else {
                    return;
                };
                if st.destroyed {
                    return;
                }
                let desc = self.bolt_desc(b);
                self.events.push(Event::BoltExpired {
                    pos: b.end,
                    struck: true,
                });
                let surface = if st.health.shield > 0.0 {
                    let large = st
                        .instance
                        .and_then(|i| self.collision.instance_radius_sq_at(i, b.end))
                        .is_some_and(|r| r >= LARGE_SHIELD_RADIUS_SQ);
                    if large {
                        Surface::ShieldLarge
                    } else {
                        Surface::Shield
                    }
                } else {
                    Surface::Building
                };
                self.push_impact(b, surface, None);
                self.damage_station(s, &desc, 1.0, Some(b.owner), Some(b.team));
                self.bolt_explodes(b);
            }
            // A fallen building turret's bodies are plain world bodies: no
            // round is ever a hit on one; were it, it is a wall.
            Hit::Terrain | Hit::World | Hit::Water | Hit::Target(Target::Building(_)) => {
                self.events.push(Event::BoltExpired {
                    pos: b.end,
                    struck: true,
                });
                let surface = match b.hit {
                    Hit::Terrain => Surface::Terrain,
                    Hit::Water => Surface::Water,
                    _ => Surface::Building,
                };
                self.push_impact(b, surface, None);
                self.bolt_explodes(b);
            }
            Hit::None => {
                // Base ordnance expiry applies to every flight family
                // (B1 FUN_00272e98, B1:195363–195367; re-ordnance-kinds §3).
                if let Some(e) = self.weapon_classes[b.weapon_class].ordnance.expire.clone() {
                    self.explode(&e, b.pos, Some(b.owner), Some(b.team));
                    self.events.push(Event::OrdnanceExpired {
                        pos: b.pos,
                        weapon_class: b.weapon_class as u32,
                    });
                }
                self.events.push(Event::BoltExpired {
                    pos: b.end,
                    struck: false,
                })
            }
        }
    }

    /// A struck bolt's impact explosion (card RKEX): the ordnance base's
    /// collision (B1 FUN_00272d70, B2 FUN_0030f550) detonates the class's
    /// `Explosion` / `ExplosionImpact` / `ExplosionName` class (B1 class
    /// +0x38, B2 +0x4c) 0.2 m off the hit with the shooter's {owner, team}
    /// whenever the class names one - on every surface (re-blaster-damage
    /// 7.3; the rocket launchers: MaxDamage 500 on the one it strikes, then
    /// Damage 400 over DamageRadius 4). After the direct hit [D]: the bolt
    /// handler (B1 FUN_00276810, B2 FUN_00313c40) builds its damage at
    /// B1:197920 and calls the base last (B1:198103), so a victim the direct
    /// hit killed takes no more damage; its corpse still consumes a gather
    /// slot and can receive the blast's kick/push. A bolt whose target died in flight
    /// dies with no impact and no blast, as before. The drawn explosion and
    /// its sound ride [`Event::Impact`].
    fn bolt_explodes(&mut self, b: &Bolt) {
        let Some(e) = self.weapon_classes[b.weapon_class]
            .ordnance
            .explosion
            .clone()
        else {
            return;
        };
        let at = b.end - b.vel.normalize_or_zero() * crate::weapon::EXPLOSION_OFFSET;
        self.explode(&e, at, Some(b.owner), Some(b.team));
    }

    /// BF2's buffs on one damage (card PWUP, `re-powerups.md` §5.5): the
    /// attacker's offense multiplier x the victim's defense multiplier, 1.0
    /// while neither timer runs.
    pub(super) fn buff_scale(&self, attacker: Option<UnitHandle>, victim: UnitHandle) -> f32 {
        let dealt = attacker
            .and_then(|a| self.units.get(a.index as usize).filter(|u| u.gen == a.gen))
            .map_or(1.0, |u| u.buffs.dealt());
        let taken = self
            .units
            .get(victim.index as usize)
            .map_or(1.0, |u| u.buffs.taken());
        dealt * taken
    }

    /// [`Event::Impact`] for `b` striking `surface` at its end (card FXP3).
    fn push_impact(
        &mut self,
        b: &Bolt,
        surface: Surface,
        vehicle: Option<crate::vehicle::VehicleHandle>,
    ) {
        self.events.push(Event::Impact {
            pos: b.end,
            dir: b.vel.normalize_or_zero(),
            weapon_class: b.weapon_class as u32,
            surface,
            vehicle,
        });
    }

    /// Mark a direct contact (see `Event::Hit::direct`) for the visual hit
    /// pose without changing damage bookkeeping.
    pub(super) fn finish_direct_damage(
        &mut self,
        victim: UnitHandle,
        attacker: Option<UnitHandle>,
        damage: f32,
        age: f32,
        dir: Vec3,
    ) -> bool {
        let at = self.events.len();
        let killed = self.finish_unit_damage(victim, attacker, damage, age, dir);
        if let Some(Event::Hit { direct, .. }) = self.events.get_mut(at) {
            *direct = true;
        }
        killed
    }

    pub(super) fn finish_unit_damage(
        &mut self,
        victim: UnitHandle,
        attacker: Option<UnitHandle>,
        damage: f32,
        age: f32,
        dir: Vec3,
    ) -> bool {
        let u = &mut self.units[victim.index as usize];
        if damage > 0.0 {
            u.cloak.strength = 0.0;
        }
        let killed = !u.health.alive();
        if let Some(shooter) = attacker {
            u.last_hit = Some((shooter, self.tick));
            self.events.push(Event::Hit {
                direct: false,
                shooter,
                victim,
                damage,
                age,
                health_left: u.health.cur,
                dir,
            });
        }
        if killed {
            self.unit_killed(victim, attacker);
        }
        killed
    }

    /// The bookkeeping of a unit that damage killed (its health is already
    /// ≤ 0): respawn timer, wave slot, the battle's alive count and ticket,
    /// the squad rules, the seat, and the kill credit — [`Event::Kill`] for
    /// `killer` (scored on its life even when it is dead), or a killer-less blast's
    /// [`Event::EnvDeath`] ([`DeathCause::Blast`]).
    pub(super) fn unit_killed(&mut self, h: UnitHandle, killer: Option<UnitHandle>) {
        self.unit_killed_with_cause(h, killer, DeathCause::Blast);
    }

    /// A direct health setter can end a life without a new damage source.
    /// Keep the same corpse, drop, seat, ticket and respawn bookkeeping.
    pub(super) fn unit_killed_with_cause(
        &mut self,
        h: UnitHandle,
        killer: Option<UnitHandle>,
        cause: DeathCause,
    ) {
        self.space_unit_killed(h, killer);
        // A placed self-piloted gun has no soldier to credit. Its distinct
        // shot identity is retained on Hit/Shot, but never a player Kill:
        // HUD and round statistics require an actual unit for that event.
        let killer = killer.filter(|k| k.index != u32::MAX);
        // An exposed rider dies where he sits and leaves a corpse there (B1
        // FUN_001ffec0, re-vehicle-death 10.3); one inside the hull leaves none.
        let seated_body = self.units[h.index as usize]
            .mount
            .and_then(|_| self.hero_body_prims(h.index as usize));
        let v = &self.units[h.index as usize];
        let (victim_team, victim_is_player) = (v.team, v.is_player);
        let respawn_at = self.tick + (self.respawn_delay * TICK_HZ as f32) as u64;
        let slot = self.assign_unit_wave(victim_team);
        let v = &mut self.units[h.index as usize];
        v.alive = false;
        v.cloak = crate::cloak::CloakState::default();
        v.deaths += 1;
        v.respawn_at = Some(respawn_at);
        v.spawn_slot = slot;
        // Card KNOCK (re-knockback §3): a soldier killed on foot keeps his
        // body; its item drop waits for the dead state (`Sim::death_block`).
        // An exposed rider's body stays at his seat; an enclosed one's drop is
        // made now (the seat's x 9 throw is not built).
        v.body = v.mount.is_none() || seated_body.is_some();
        if let Some(body) = seated_body {
            v.soldier.pos = body.pos;
            v.soldier.yaw = body.yaw;
        }
        v.drop_due = v.body;
        if !v.body {
            self.drop_item(h, None);
        }
        if victim_team < 2 {
            self.battle.on_death(victim_team as usize, victim_is_player);
        }
        self.class_change_life_ended(h);
        // Phase D: the squad rules for a death, both of them.
        crate::squad::on_unit_died(self, h);
        self.unseat_unit(h);
        match killer {
            Some(k) => {
                let killer_team = self
                    .units
                    .get(k.index as usize)
                    .filter(|s| s.gen == k.gen)
                    .map(|s| s.team);
                self.nav_stats.unit_kills += 1;
                if k == h {
                    self.nav_stats.self_kills += 1;
                } else if killer_team.is_some_and(|team| self.allied(team, victim_team)) {
                    self.nav_stats.team_kills += 1;
                }
                if k != h && killer_team.is_some_and(|team| self.hostile(team, victim_team)) {
                    self.heal_hero_kill(k);
                }
                self.deathmatch_kill(k, victim_team);
                // The PS2 credits the member, alive or dead (B1 FUN_00297440:
                // a destroyed vehicle's riders die after its destroyer may
                // have, re-vehicle-death 10.5): this life's slot by generation.
                if let Some(s) = self
                    .units
                    .get_mut(k.index as usize)
                    .filter(|s| s.gen == k.gen)
                {
                    s.kills += 1;
                }
                self.events.push(Event::Kill {
                    shooter: k,
                    victim: h,
                });
            }
            None => self.events.push(Event::EnvDeath { unit: h, cause }),
        }
    }

    /// The death block of a dead unit's body (card KNOCK, re-knockback §3: B1
    /// B1:116889-116968, B2 B2:159605-159669): skipped while it is thrown,
    /// tumbling or getting up - a soldier a blast kills dies when he has
    /// landed - else the body enters [`Stance::Dead`] and the kill's item
    /// drop comes now. A droid may burst instead ([`Sim::chunk_death`], card
    /// DROIDDEATH): no corpse, the same drop.
    pub(super) fn death_block(&mut self, i: usize) {
        let u = &self.units[i];
        if u.alive || !u.body || u.soldier.stance.knocked() || u.soldier.stance == Stance::Dead {
            return;
        }
        let drop = u.drop_due;
        let burst = if u.droideka.is_some() {
            self.droideka_burst(i);
            true
        } else {
            self.chunk_death(i)
        };
        if !burst {
            let u = &mut self.units[i];
            u.soldier.stance = Stance::Dead;
            u.soldier.time_in_stance = 0.0;
        }
        let u = &mut self.units[i];
        u.drop_due = false;
        if drop {
            let h = self.handle(i);
            self.drop_item(h, None);
        }
    }

    /// The ONE environmental death path: a mission death region, or a lethal
    /// landing. It does to the unit, the battle and the spawn waves exactly
    /// what a bolt kill does — respawn timer, wave slot, alive count, one
    /// ticket — and emits [`Event::EnvDeath`] so the HUD and audio can tell
    /// the two apart. A dead or stale handle is a no-op. The `invincible`
    /// cheat spares the player from the causes that are ordinary damage on
    /// the PS2 ([`DeathCause::cheat_spares`]: a landing, a blast) and from
    /// nothing else: a death region, the kill floor, a landing under the map
    /// and a destroyed vehicle kill him too.
    ///
    /// `AddDeathRegion`'s consumer (`analysis/re-kill-floor.md`, 2026-10-01): a
    /// per-frame test of every soldier and vehicle against the 32 region
    /// slots, an instant kill with no killer and no cheat check (BF1
    /// `FUN_002af1c0` / `FUN_002af0d0`, BF2 `FUN_00371148` / `FUN_00370fd8`).
    /// BF1 tests the feet, as here; BF2 tests the collision sphere (not
    /// built). The lethal-landing rule is [`LETHAL_LANDING_SPEED`]
    /// (PROVISIONAL, card SLAND).
    pub fn kill_environment(&mut self, h: UnitHandle, cause: DeathCause) {
        let Some(u) = self.unit(h) else {
            return;
        };
        if u.is_player && self.cheats.invincible && cause.cheat_spares() {
            return;
        }
        self.space_unit_killed(h, None);
        let Some(u) = self.unit(h) else {
            return;
        };
        let (team, is_player) = (u.team, u.is_player);
        // A crash with no destroyer leaves an exposed rider's corpse as a
        // credited kill does (re-vehicle-death 8).
        let seated_body = (cause == DeathCause::VehicleDestroyed)
            .then(|| self.hero_body_prims(h.index as usize))
            .flatten();
        let respawn_at = self.tick + (self.respawn_delay * TICK_HZ as f32) as u64;
        let slot = self.assign_unit_wave(team);
        let v = &mut self.units[h.index as usize];
        v.alive = false;
        v.cloak = crate::cloak::CloakState::default();
        v.deaths += 1;
        v.health.cur = 0.0;
        v.respawn_at = Some(respawn_at);
        v.spawn_slot = slot;
        if let Some(body) = seated_body {
            v.soldier.pos = body.pos;
            v.soldier.yaw = body.yaw;
            v.body = true;
            v.drop_due = true;
        } else {
            self.drop_item(h, Some(cause));
        }
        if team < 2 {
            self.battle.on_death(team as usize, is_player);
        }
        self.class_change_life_ended(h);
        crate::squad::on_unit_died(self, h);
        self.unseat_unit(h);
        if matches!(cause, DeathCause::Lost | DeathCause::Stalled) {
            // Recycles remove the actor instead of leaving a dead owner.
            self.retire_saber_flights(h.index as usize);
        }
        self.events.push(Event::EnvDeath { unit: h, cause });
    }
}

#[cfg(test)]
mod tests;
