//! Blaster bolts (BF1 FUN_002762a0 spawn / FUN_002764c0 move+collide /
//! FUN_00272e98 lifetime; BF2 FUN_00313600). A fired bolt (`Bolt::fire`)
//! asks ONE predictive ray at the press of what does not move - buildings
//! and terrain (mask 0x48 / BF2 0x90) - and is swept against soldiers and
//! vehicles each frame from its previous to its current place (B1
//! FUN_00273128, BF2 FUN_0030fad0; `re-ordnance-kinds.md` §2.4 [V][M],
//! `Sim::sweep_bolts`): a unit that steps out of the line is missed and
//! one that steps in is hit. `Bolt::spawn` (every candidate on the one ray)
//! is the AI's shot PREDICTION. Complete bit-exact bolt replay serialization
//! is owned here by `Bolt::hash_into` (card HASH).

use crate::collision::{ray_heightfield, Capsule, CollisionWorld, HullFilter, Layers};
use crate::heightfield::Heightfield;
use crate::unit::UnitHandle;
use glam::Vec3;

/// Terrain march step for the spawn ray (meters).
pub const TERRAIN_RAY_STEP: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    None,
    /// The heightfield sheet (`collision::ray_heightfield`).
    Terrain,
    /// A `coll` mesh or `prim` primitive of the baked world
    /// (`collision::CollisionWorld::ray`) — a wall, a crate, a building.
    /// Like [`Hit::Terrain`] it carries no payload: the impact point is
    /// [`Bolt::end`], and nothing consumes the surface normal yet.
    World,
    Water,
    /// Something that takes damage — see [`Target`].
    Target(Target),
}

/// What a bolt can damage: a soldier, or a vehicle's ONE hull pool (no
/// region — the PS2 has no damage regions, analysis/re-vehicles.md §6).
/// `Sim::resolve_bolt` deals a vehicle hit to the pool (VDMG); the spawn
/// ray picks a hull when its ordnance-subset bodies are the nearest hit
/// (card VHULL).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Unit(UnitHandle),
    Vehicle(crate::vehicle::VehicleHandle),
    Station(u16),
    Mine(u64),
    Droid(u64),
    Turret(u64),
    Guided(u64),
    /// A fallen building turret (`Sim::building_turrets` index): only the
    /// fusion cutter's target (card TBLD); no round meets one.
    Building(u16),
    Shield(UnitHandle),
}

/// Presentation identity of one emitted Shot event. The tick is the step's
/// input tick; Sim.tick advances before the caller sees that step's events.
/// Physics and deterministic hashes never read this annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VisualShot {
    pub tick: u64,
    pub event: usize,
}

/// A new round's own predicted or actual endpoint after its first flight step.
/// Retained for presentation even when the round expires inside that step.
#[derive(Clone, Copy, Debug)]
pub struct ShotFlight {
    pub shot: VisualShot,
    pub end: Vec3,
}

#[derive(Clone, Debug)]
pub struct Bolt {
    /// Presentation provenance, excluded from physics and the sim hash.
    pub visual_shot: Option<VisualShot>,
    pub pos: Vec3,
    pub vel: Vec3,
    /// Seconds of flight left (+0x3c): LifeSpan, scaled by the hit fraction.
    pub remaining: f32,
    /// Seconds since spawn (+0x40) — BF2 damage falloff runs on this.
    pub age: f32,
    pub owner: UnitHandle,
    pub team: u8,
    pub weapon_class: usize,
    pub hit: Hit,
    /// Where the spawn ray said the bolt ends (impact or expiry point).
    pub end: Vec3,
    /// Was the shooter a human? Carried so `resolve_bolt` can tell a bot's
    /// bolt into a human from a human's own (D12's damage multiplier); the
    /// owner may be dead and recycled by the time the bolt lands.
    pub shooter_is_player: bool,
    /// The hulls this bolt may strike (`Bolt::fire`'s filter): a rider's bolt
    /// passes its own vehicle. The sim tests them, and the soldiers, along
    /// each tick's flight (`Sim::sweep_bolts`, issue #41).
    pub hulls: HullFilter,
    /// The struck prim's HitLocation factor, set by the sweep (issue #41):
    /// a soldier's head x3 (`Unit::bolt_hit`), a vehicle prim its class's
    /// (`Body::hit_mult`: the x4 critical prims, BF1's x0 two), else 1.0.
    pub hit_mult: f32,
    /// A shell's fall, m/s² ([`Bolt::shaped`]); 0 flies straight.
    pub gravity: f32,
    /// The charge it was fired at: its damage's blend
    /// (`OrdnanceClass::damage_desc_charged`); 1.0 for an uncharged weapon.
    pub charge: f32,
    /// A `missile` round's steering state (card UORD,
    /// [`crate::missile::Missile`]): `None` on every other ordnance.
    pub missile: Option<crate::missile::Missile>,
    pub fat: Option<f32>,
}

/// Who fired (carried by the bolt for damage attribution + friendly fire).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shooter {
    pub owner: UnitHandle,
    pub team: u8,
    pub weapon_class: usize,
    /// The firing unit's `is_player` at the press (D12).
    pub is_player: bool,
}

impl Bolt {
    pub fn shooter(&self) -> Shooter {
        Shooter {
            owner: self.owner,
            team: self.team,
            weapon_class: self.weapon_class,
            is_player: self.shooter_is_player,
        }
    }

    /// Bit-exact replay state (card HASH). Hit variants have distinct tags;
    /// historical handles retain their generations after slot reuse.
    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for v in [
            self.pos.x,
            self.pos.y,
            self.pos.z,
            self.vel.x,
            self.vel.y,
            self.vel.z,
            self.remaining,
            self.age,
            self.end.x,
            self.end.y,
            self.end.z,
        ] {
            eat(&v.to_bits().to_le_bytes());
        }
        eat(&self.owner.index.to_le_bytes());
        eat(&self.owner.gen.to_le_bytes());
        eat(&[self.team]);
        eat(&(self.weapon_class as u64).to_le_bytes());
        eat(&[u8::from(self.shooter_is_player)]);
        match self.hit {
            Hit::None => eat(&[0]),
            Hit::Terrain => eat(&[1]),
            Hit::World => eat(&[2]),
            Hit::Target(Target::Unit(h)) => {
                eat(&[3]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
            Hit::Target(Target::Vehicle(h)) => {
                eat(&[4]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
            Hit::Target(Target::Station(s)) => {
                eat(&[5]);
                eat(&s.to_le_bytes());
            }
            Hit::Water => eat(&[6]),
            Hit::Target(Target::Mine(id)) => {
                eat(&[7]);
                eat(&id.to_le_bytes());
            }
            Hit::Target(Target::Guided(id)) => {
                eat(&[10]);
                eat(&id.to_le_bytes());
            }
            Hit::Target(Target::Droid(id)) => {
                eat(&[8]);
                eat(&id.to_le_bytes());
            }
            Hit::Target(Target::Turret(id)) => {
                eat(&[9]);
                eat(&id.to_le_bytes());
            }
            Hit::Target(Target::Building(i)) => {
                eat(&[11]);
                eat(&i.to_le_bytes());
            }
            Hit::Target(Target::Shield(h)) => {
                eat(&[12]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
        }
        let skip = match self.hulls {
            HullFilter::All | HullFilter::Hulls => [None; 2],
            HullFilter::Static => {
                eat(&[7]);
                [None; 2]
            }
            HullFilter::Except(s) | HullFilter::HullsExcept(s) => s,
            HullFilter::Only(h) => {
                eat(&[9]);
                [Some(h), None]
            }
        };
        for v in skip.iter().flatten() {
            eat(&v.index.to_le_bytes());
            eat(&v.gen.to_le_bytes());
        }
        if self.hit_mult != 1.0 {
            eat(b"HITMULT");
            eat(&self.hit_mult.to_bits().to_le_bytes());
        }
        if let Some(m) = self.missile {
            eat(b"MSL");
            for v in [
                m.dir.x,
                m.dir.y,
                m.dir.z,
                m.launch.x,
                m.launch.y,
                m.launch.z,
                m.ramp,
                m.speed,
                m.phases[0],
                m.phases[1],
            ] {
                eat(&v.to_bits().to_le_bytes());
            }
            match m.target {
                None => eat(&[0]),
                Some(crate::missile::MissileTarget::Unit(h)) => {
                    eat(&[1]);
                    eat(&h.index.to_le_bytes());
                    eat(&h.gen.to_le_bytes());
                }
                Some(crate::missile::MissileTarget::Vehicle(h)) => {
                    eat(&[2]);
                    eat(&h.index.to_le_bytes());
                    eat(&h.gen.to_le_bytes());
                }
                Some(crate::missile::MissileTarget::Turret(id)) => {
                    eat(&[3]);
                    eat(&id.to_le_bytes());
                }
            }
        }
        if let Some(damage) = self.fat {
            eat(b"FAT");
            eat(&damage.to_bits().to_le_bytes());
        }
    }

    /// A shot's bolt (issue #41; the module header): at the press only what
    /// does not move is asked - the terrain, the water and the map's bodies -
    /// for where the flight ends; the soldiers and the hulls `hulls` admits
    /// are tested along each tick's flight by `Sim::sweep_bolts`, so a bolt
    /// strikes what it passes through then.
    #[allow(clippy::too_many_arguments)]
    pub fn fire(
        origin: Vec3,
        dir: Vec3,
        speed: f32,
        life_span: f32,
        shooter: Shooter,
        hf: Option<&Heightfield>,
        water: Option<crate::water::Plane<'_>>,
        world: &CollisionWorld,
        hulls: HullFilter,
    ) -> Bolt {
        // A rider's round passes his own building turret's bodies (card
        // TCOL, `analysis/re-turret-collision.md` 6.2: the PS2's ray skips
        // the owner entity, as the seat's camera ray does): its gun's prims
        // stand round the muzzle.
        let own = match hulls {
            HullFilter::Except(s) | HullFilter::HullsExcept(s) => {
                s.map(|v| v.and_then(|v| world.turret_instance(v)))
            }
            _ => [None; 2],
        };
        let mut b = Bolt::spawn_past(
            origin,
            dir,
            speed,
            life_span,
            shooter,
            hf,
            water,
            world,
            HullFilter::Static,
            own,
            std::iter::empty(),
        );
        b.hulls = hulls;
        b.pass_own_building();
        b
    }

    /// A rider's round leaves his own building turret: the press ray met
    /// his turret's building (a body [`building_hit`] names as the turret)
    /// where the hull filter skips his vehicle, so it is a wall the round
    /// stops on, as before the building took damage, not a hit on his own
    /// turret.
    pub fn pass_own_building(&mut self) {
        let skip = match self.hulls {
            HullFilter::Except(s) | HullFilter::HullsExcept(s) => s,
            _ => return,
        };
        if let Hit::Target(Target::Vehicle(v)) = self.hit {
            if skip.contains(&Some(v)) {
                self.hit = Hit::World;
            }
        }
    }

    /// Spawn with the predictive raycast: candidates = terrain + the given
    /// unit capsules (owner excluded by the caller). `speed` = ordnance
    /// Velocity, `life_span` = ordnance LifeSpan.
    ///
    /// Four candidates, nearest wins: the terrain CROSSING, the `water`
    /// plane crossed downward over a wet cell, the `world` models — the map's and the
    /// vehicles' hulls that `hulls` admits (a rider's bolt skips its own
    /// vehicle); a hull hit is [`Target::Vehicle`] — and the unit capsules.
    ///
    /// `world` carries both the map's bodies and its terrain cutters. The
    /// terrain ray is a crossing test that ignores a crossing inside a cutter
    /// (`core::collision`), so a bolt fired inside a cave or a cut building is
    /// not killed at t = 0 by the heightfield overhead; the model ray
    /// (`CollisionWorld::ray`) is what stops it at the cave's actual wall.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        origin: Vec3,
        dir: Vec3,
        speed: f32,
        life_span: f32,
        shooter: Shooter,
        hf: Option<&Heightfield>,
        water: Option<crate::water::Plane<'_>>,
        world: &CollisionWorld,
        hulls: HullFilter,
        targets: impl Iterator<Item = (UnitHandle, Capsule)>,
    ) -> Bolt {
        Bolt::spawn_past(
            origin, dir, speed, life_span, shooter, hf, water, world, hulls, [None; 2], targets,
        )
    }

    /// [`Bolt::spawn`] whose model ray passes the bodies of the map
    /// instances in `skip` (`CollisionWorld::ray_past`).
    #[allow(clippy::too_many_arguments)]
    fn spawn_past(
        origin: Vec3,
        dir: Vec3,
        speed: f32,
        life_span: f32,
        shooter: Shooter,
        hf: Option<&Heightfield>,
        water: Option<crate::water::Plane<'_>>,
        world: &CollisionWorld,
        hulls: HullFilter,
        skip: [Option<u32>; 2],
        targets: impl Iterator<Item = (UnitHandle, Capsule)>,
    ) -> Bolt {
        let dir = dir.normalize();
        let total = speed * life_span;
        let mut best_t = total;
        let mut hit = Hit::None;
        if let Some(hf) = hf {
            if let Some(t) =
                ray_heightfield(hf, &world.cutters, origin, dir, total, TERRAIN_RAY_STEP)
            {
                if t < best_t {
                    best_t = t;
                    hit = Hit::Terrain;
                }
            }
        }
        if let Some(t) = water.and_then(|w| w.crossing(origin, dir)) {
            if t < best_t {
                best_t = t;
                hit = Hit::Water;
            }
        }
        // The PS2's bolt ray passes the trailing flag, so it tests only the
        // target model's ORDNANCE subset (analysis/re-collision-layers.md §4,
        // `FUN_002764C0`).
        if let Some(h) = world.ray_past(origin, dir, best_t, Layers::ORDNANCE, hulls, skip) {
            if h.t < best_t {
                best_t = h.t;
                hit = match h.owner {
                    Some(v) => Hit::Target(Target::Vehicle(v)),
                    None => building_hit(world, h.instance),
                };
            }
        }
        for (h, cap) in targets {
            if let Some(t) = cap.ray_hit(origin, dir, best_t) {
                if t < best_t {
                    best_t = t;
                    hit = Hit::Target(Target::Unit(h));
                }
            }
        }
        let remaining = if hit == Hit::None {
            life_span
        } else {
            life_span * (best_t / total)
        };
        Bolt {
            visual_shot: None,
            pos: origin,
            vel: dir * speed,
            remaining,
            age: 0.0,
            owner: shooter.owner,
            team: shooter.team,
            weapon_class: shooter.weapon_class,
            hit,
            end: origin + dir * best_t,
            shooter_is_player: shooter.is_player,
            hulls,
            hit_mult: 1.0,
            gravity: 0.0,
            charge: 1.0,
            missile: None,
            fat: None,
        }
    }

    /// A `shell` round as its ordnance flies it (`analysis/re-ordnance-kinds.md`
    /// §6, issue #29): it falls at Gravity x [`crate::weapon::SHELL_G`], has no
    /// predictive ray - the world is met tick by tick on its arc
    /// (`Sim::fly_shells`) - and lives its whole LifeSpan, then bursts with
    /// its ExplosionExpire. Any other ordnance's bolt is returned as it is.
    pub fn shaped(mut self, o: &crate::weapon::OrdnanceClass) -> Self {
        if o.kind != crate::weapon::OrdKind::Shell || o.gravity == 0.0 {
            return self;
        }
        self.gravity = o.gravity * crate::weapon::SHELL_G;
        self.hit = Hit::None;
        self.remaining = o.life_span;
        self.end = self.pos + self.vel * o.life_span;
        self
    }

    /// One tick: `pos += vel × min(dt, remaining)`; returns true when the
    /// flight is over (impact or expiry) this tick. The age counts only the
    /// updates the bolt came through, so a bolt that lands in its first one
    /// strikes at age 0 with its full damage (PCSX2 BF2 shotgun at 3 m:
    /// 65 a pellet, never the falloff's 48.3; `re-ordnance-kinds.md` §7).
    pub fn advance(&mut self, dt: f32) -> bool {
        let step = dt.min(self.remaining.max(0.0));
        self.pos += self.vel * step;
        self.remaining -= dt;
        let over = self.remaining <= 0.0;
        if !over {
            self.age += dt;
        }
        over
    }

    /// A `missile` round as its class flies it (card UORD,
    /// `re-ordnance-kinds.md` §4): no predictive ray - the world is met on
    /// its own flight each tick (`Sim::fly_missiles`, the shells' path) - no
    /// gravity, and the steering state of [`crate::missile::Missile`].
    pub fn as_missile(
        &mut self,
        o: &crate::weapon::OrdnanceClass,
        c: &crate::missile::MissileClass,
        target: Option<crate::missile::MissileTarget>,
        lcg: &mut crate::lcg::GameLcg,
    ) {
        self.hit = Hit::None;
        self.gravity = 0.0;
        self.remaining = o.life_span;
        self.end = self.pos + self.vel * o.life_span;
        self.missile = Some(crate::missile::Missile::launch(
            c, self.vel, o.velocity, target, lcg,
        ));
    }

    /// Steer a missile one update (`re-ordnance-kinds.md` §4.3) and write its
    /// velocity. `aim` is the target's point this tick, `None` when the round
    /// has no target. Returns whether it still has one.
    pub fn guide(
        &mut self,
        c: &crate::missile::MissileClass,
        aim: Option<Vec3>,
        dt: f32,
        lcg: &mut crate::lcg::GameLcg,
    ) -> bool {
        let Some(m) = self.missile.as_mut() else {
            return false;
        };
        if m.target.is_some() && aim.is_none() {
            m.target = None;
        }
        self.vel = m.step(c, aim, self.pos, dt, lcg);
        m.target.is_some()
    }
}

/// What a press ray that met a map body (no hull owner) struck: a station's
/// body is the station, a building turret's body is the turret
/// (`CollisionWorld::bind_turret`; the PS2 damages the armed building,
/// `analysis/re-turret-collision.md` 3.1), anything else the world.
fn building_hit(world: &CollisionWorld, instance: Option<u32>) -> Hit {
    let Some(i) = instance else {
        return Hit::World;
    };
    if let Some(s) = world.station(i) {
        return Hit::Target(Target::Station(s));
    }
    world
        .turret(i)
        .map_or(Hit::World, |v| Hit::Target(Target::Vehicle(v)))
}
