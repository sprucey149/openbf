//! GBOARD pending call targets end on seating, receiver death and target removal.
//! WPOKE mounted pending orders end on unseating, including same-tick reboarding.
//! Seating, unseating, death/despawn and target removal end claim maintenance (CWAIT).
//! The sim's vehicle slots (card VENT, `crate::vehicle`): spawn with slot
//! reuse, the handle queries, and the two occupancy writers that keep
//! `Seat::occupant` and `Unit::mount` in step — and the live team with them
//! (re-vehicles §7.1, card VTEAM) — and the hull pool's damage and death
//! (§6, card VDMG) — and the spawn pads' tick (§7, card VPAD,
//! `crate::vehicle_pad`) — and the hovers' tick (card HOVR,
//! `crate::vehicle::hover`) and the walkers' (card WALK,
//! `crate::vehicle::walker`), stage 3a — and the hulls (card VHULL): each
//! live vehicle's class bodies ([`Sim::hull_parts`]) placed in
//! [`Sim::collision`] at its pose, so soldiers stop at it, stand on it and
//! bolts and eyes meet it; kept in step by [`Sim::sync_hulls`] (the step's
//! start and after 3a), a spawn and a removal. Nothing here runs unless a
//! caller adds a pad or spawns a vehicle. Successful seating starts WAIT4
//! for an AI ground pilot; unseating clears it, including seat switches.
//! STCHG clears the previous weapon's AI charge deadline on both boundaries.

use super::awards::endurance_rate;
use super::state::{DeathCause, Event, Sim};
use crate::collision::NO_GROUND;
use crate::collision::{CollisionWorld, HullFilter, Layers, Shape};
use crate::fmath::FloatExt;
use crate::heightfield::Heightfield;
use crate::unit::SOLDIER_RADIUS;
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::flyer::FlightPhase;
use crate::vehicle::hover::{self, Contact, ContactSource, HullBody, HullShape};
use crate::vehicle::walker::{self, WalkerState, WalkerWorld};
use crate::vehicle::{
    collision_desc, impact_damage, Mount, SeatKind, Vehicle, VehicleHandle, VehicleKind,
    VehicleSpec,
};
use crate::vehicle_pad::{PadDef, VehiclePad, IDLE_CLASS_CHANGED, IDLE_OUTSIDE, PLAYER_MARGIN};
use crate::weapon::DamageDesc;
use glam::{Mat3, Mat4, Vec3};

/// The pad's spawn ray starts this far over the pad and ends as far under it
/// (re-vehicle-rest §1: `*(0x66ef00)` = (0, 1, 0, 0) in RAM, both games).
const PAD_RAY: f32 = 1.0;
/// How far over or under a pad a soldier counts as on it (m): a deck above or
/// below a pad is not its footprint. PROVISIONAL (ours).
const PAD_HOLD_RISE: f32 = 4.0;

/// What a hover's bodies touch (re-hover §13): the terrain by the PS2's
/// sphere rule (`hover::terrain_sphere`, §13.5: one height sample under the
/// sphere, met only while the terrain is within r of the centre's height — a
/// sphere buried deeper, a hangar floor under an uncut sheet on POL1, does
/// not touch it — and none where a cutter cuts the SURFACE point under the
/// centre, as FUN_0055f258 tests it), then every SOLDIER-subset body of the
/// world (PROVISIONAL: the PS2 pairs a hover with each building's VEHICLE
/// subset, §13.3; the port has none yet, card VSUB).
struct WorldContacts<'a> {
    heightfield: Option<&'a Heightfield>,
    collision: &'a CollisionWorld,
    water: Option<crate::water::Plane<'a>>,
}

impl WorldContacts<'_> {
    fn terrain(&self, c: Vec3, r: f32, out: &mut Vec<Contact>) {
        let Some(hf) = self.heightfield else { return };
        let h = hf.height_at(c.x, c.z);
        let n = Vec3::from(hf.normal_at(c.x, c.z));
        if let Some(contact) = hover::terrain_sphere(c.y, r, h, n) {
            if !self.collision.is_cut(Vec3::new(c.x, h, c.z)) {
                out.push(contact);
            }
        }
    }

    fn buildings(&self, shape: Shape, to_world: Mat4, out: &mut Vec<Contact>) {
        if self.collision.bodies().is_empty() {
            return;
        }
        let mut hits = Vec::new();
        match shape {
            Shape::Sphere { radius } => self.collision.sphere_contacts(
                to_world.w_axis.truncate(),
                radius,
                Layers::SOLDIER,
                &mut hits,
            ),
            _ => self
                .collision
                .shape_contacts(&shape, to_world, Layers::SOLDIER, &mut hits),
        }
        out.extend(
            hits.into_iter()
                .map(|(depth, normal)| Contact { depth, normal }),
        );
    }
}

impl ContactSource for WorldContacts<'_> {
    fn sphere_contacts(&self, c: Vec3, r: f32, out: &mut Vec<Contact>) {
        self.terrain(c, r, out);
        self.buildings(Shape::Sphere { radius: r }, Mat4::from_translation(c), out);
    }

    /// A hull body: the terrain for a sphere in the terrain subset (a box's
    /// or cylinder's terrain rule is not built, `hover` module header), the
    /// buildings by the body's own shape (parry's contact, PROVISIONAL).
    fn hull_contacts(&self, b: &HullBody, pos: Vec3, rot: Mat3, out: &mut Vec<Contact>) {
        let shape = match b.shape {
            HullShape::Sphere { radius } => {
                if b.terrain {
                    self.terrain(pos, radius, out);
                }
                Shape::Sphere { radius }
            }
            HullShape::Box { half } => Shape::Box { half },
            HullShape::Cylinder {
                radius,
                half_height,
            } => Shape::Cylinder {
                radius,
                half_height,
            },
        };
        if b.building {
            let to_world = Mat4::from_cols(
                rot.x_axis.extend(0.0),
                rot.y_axis.extend(0.0),
                rot.z_axis.extend(0.0),
                pos.extend(1.0),
            );
            self.buildings(shape, to_world, out);
        }
    }

    fn water_under(&self, at: Vec3) -> Option<f32> {
        self.water.as_ref().and_then(|w| w.under(at))
    }
}

/// The walker's terrain ray step (m): FUN_0055fb38's own march is untraced
/// (PROVISIONAL), the probes are 3-6 m long.
const WALKER_TERRAIN_STEP: f32 = 0.25;

/// What a walker's rays and spheres meet (PROVISIONAL, module header of
/// `crate::vehicle::walker`): the heightfield sheet where no cutter punches
/// it, and the SOLDIER-subset bodies as the buildings.
impl WalkerWorld for WorldContacts<'_> {
    fn has_terrain(&self) -> bool {
        self.heightfield.is_some()
    }

    fn ground_below(&self, origin: Vec3, len: f32) -> Option<f32> {
        let mut best = None::<f32>;
        if let Some(hf) = self.heightfield {
            let dist = origin.y - hf.height_at(origin.x, origin.z);
            let ground = Vec3::new(origin.x, origin.y - dist, origin.z);
            if (0.0..=len).contains(&dist) && !self.collision.is_cut(ground) {
                best = Some(dist);
            }
        }
        if !self.collision.bodies().is_empty() {
            if let Some(h) = self.collision.ray_with(
                origin,
                Vec3::NEG_Y,
                len,
                Layers::SOLDIER,
                HullFilter::Static,
            ) {
                let t = h.t;
                best = Some(best.map_or(t, |b| b.fmin(t)));
            }
        }
        best
    }

    fn terrain_segment(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let hf = self.heightfield?;
        let d = to - from;
        let len = d.length();
        if len <= 0.0 {
            return None;
        }
        crate::collision::ray_heightfield(
            hf,
            &self.collision.cutters,
            from,
            d / len,
            len,
            WALKER_TERRAIN_STEP,
        )
        .map(|t| t / len)
    }

    fn building_contacts(&self, c: Vec3, r: f32, out: &mut Vec<Contact>) {
        self.buildings(Shape::Sphere { radius: r }, Mat4::from_translation(c), out);
    }

    fn terrain_contacts(&self, c: Vec3, r: f32, out: &mut Vec<Contact>) {
        self.terrain(c, r, out);
    }
}

impl Sim {
    /// Build `spec` into the first dead slot (generation bumped) or a new one.
    pub fn spawn_vehicle(&mut self, spec: &VehicleSpec) -> VehicleHandle {
        let slot = self.vehicles.iter().position(|v| !v.alive);
        let gen = slot.map_or(1, |i| self.vehicles[i].gen + 1);
        let mut v = spec.build(gen);
        let index = slot.unwrap_or(self.vehicles.len());
        // PROVISIONAL: the spring jitter's LCG per vehicle, seeded by slot.
        v.hover.jitter = (index as u32).wrapping_mul(0x9E37_79B9) ^ gen;
        if let Some(p) = self
            .walker_params
            .get(spec.class as usize)
            .and_then(|p| p.as_ref())
        {
            v.walker = WalkerState::new(p);
        }
        let index = match slot {
            Some(i) => {
                self.vehicles[i] = v;
                i
            }
            None => {
                self.vehicles.push(v);
                self.vehicles.len() - 1
            }
        };
        self.sync_hull(index);
        VehicleHandle {
            index: index as u32,
            gen,
        }
    }

    /// Put slot `i`'s hull where its vehicle is (card VHULL): its class's
    /// [`Sim::hull_parts`] at `pos` and the drawn heading (a walker's legs,
    /// `walker::legs_rot`, as `game::viewer::vehicles` draws it; else `rot`),
    /// rigid — PROVISIONAL: a walker's animated prims stay at the bind pose.
    /// A dead slot, or a class with no parts, has none; nor has a dying
    /// walker posed by the tow frames (`Sim::sync_dying_walkers`).
    pub(crate) fn sync_hull(&mut self, i: usize) {
        let Some(v) = self.vehicles.get(i) else {
            return;
        };
        let h = VehicleHandle {
            index: i as u32,
            gen: v.gen,
        };
        let parts = self
            .hull_parts
            .get(v.class as usize)
            .and_then(|p| p.clone())
            .filter(|_| v.alive && self.dying_walker_bodies(v).is_none());
        match parts {
            Some(parts) => {
                let rot = if v.kind == crate::vehicle::VehicleKind::Walker {
                    walker::legs_rot(v)
                } else {
                    v.rot
                };
                let pose = glam::Mat4::from_rotation_translation(rot, v.pos);
                self.collision.set_hull(h, &parts, pose);
            }
            None => self.collision.remove_hull(h),
        }
    }

    /// [`Sim::sync_hull`] for every slot: at the step's start (a caller may
    /// have moved a vehicle, or installed a new world) and after stage 3a.
    pub fn sync_hulls(&mut self) {
        if self.hull_parts.is_empty() {
            return;
        }
        for i in 0..self.vehicles.len() {
            self.sync_hull(i);
        }
    }

    pub fn vehicle(&self, h: VehicleHandle) -> Option<&Vehicle> {
        self.vehicles
            .get(h.index as usize)
            .filter(|v| v.gen == h.gen && v.alive)
    }

    pub fn vehicle_mut(&mut self, h: VehicleHandle) -> Option<&mut Vehicle> {
        self.vehicles
            .get_mut(h.index as usize)
            .filter(|v| v.gen == h.gen && v.alive)
    }

    /// Alive vehicles with their handles, in index order.
    pub fn alive_vehicles(&self) -> impl Iterator<Item = (VehicleHandle, &Vehicle)> {
        self.vehicles
            .iter()
            .enumerate()
            .filter(|(_, v)| v.alive)
            .map(|(i, v)| {
                (
                    VehicleHandle {
                        index: i as u32,
                        gen: v.gen,
                    },
                    v,
                )
            })
    }

    /// The vehicle's live team (re-vehicles §7.1): the last entrant's, `None`
    /// = neutral. The field CanEnter, the team relation and targeting read.
    pub fn vehicle_team(&self, h: VehicleHandle) -> Option<u8> {
        self.vehicle(h)?.team
    }

    /// Put a live, unmounted unit into an empty seat of a live vehicle, both
    /// sides at once, and give the vehicle the entrant's team — ANY seat does
    /// (B1 FUN_0023c488 B1:154825-154829, B2 FUN_002f5478 B2:283845-283846). `false` (and nothing written) otherwise. Who MAY enter
    /// which seat (§5's cone, team and PilotType rules) is `super::seats`'
    /// (VSEAT); this only keeps the bookkeeping whole.
    pub fn seat_unit(&mut self, u: UnitHandle, vh: VehicleHandle, seat: u8) -> bool {
        if self.unit(u).is_none_or(|x| x.mount.is_some()) {
            return false;
        }
        let team = self.units[u.index as usize].team;
        let Some(v) = self.vehicle_mut(vh) else {
            return false;
        };
        match v.seats.get_mut(usize::from(seat)) {
            Some(s) if s.occupant.is_none() => s.occupant = Some(u),
            _ => return false,
        }
        v.team = Some(team);
        let kind = v.kind;
        let hover_ai = v.hover.ai;
        let pilot = v.seats[usize::from(seat)].kind == SeatKind::Pilot;
        let ground = matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker);
        self.units[u.index as usize].mount = Some(Mount { vehicle: vh, seat });
        // A rider leaves his foot stance: the PS2 seats him in its rider
        // state (BF1 state 12, re-turret-collision 3.2) and a drawn gunner
        // is drawn in the seat's pose. A seated unit never steps, so a
        // crouch kept here would keep his bolt prims crouched under his
        // drawn head for the whole ride (playtest 2026-10-29).
        self.units[u.index as usize].soldier.stance = crate::soldier::Stance::Stand;
        self.units[u.index as usize].ai.pending_board = None;
        if let Some(r) = self.units[u.index as usize].ai.ride.as_mut() {
            r.board = None;
            r.called_board = None;
            r.player_board = None;
            r.ordered_call = false;
            r.maintenance = None;
        }
        // STCHG: successful seating begins a new AI charge tenure.
        self.units[u.index as usize].ai.charge_until = 0;
        // The flyer's pilot shares the pickup wait (re-driver-wait.md §2),
        // behind the switch that keeps it off ground maps. It draws no VBAIL.
        let waits = ground || (kind == VehicleKind::Flyer && self.flyer_bots);
        if waits && pilot && self.units[u.index as usize].controller == Controller::Ai {
            let r = self.units[u.index as usize].ai.ride.get_or_insert_default();
            r.begin_pickup(vh, self.tick, kind, hover_ai);
        }
        // VBAIL: use the sim entropy, never a tick/slot hash. Failed entries
        // and player/flyer/turret entries consume no draw. The card's per-
        // boarding lifetime is PROVISIONAL against the PS2 controller draw.
        if crate::ai::VEHICLE_BAIL
            && ground
            && self.units[u.index as usize].controller == Controller::Ai
        {
            let draw = (self.rng.next_rand() % 101) as u8;
            let r = self.units[u.index as usize].ai.ride.get_or_insert_default();
            r.bail_draw = Some(draw);
            r.bail_seen = false;
        }
        true
    }

    /// Take a unit out of its seat, both sides; a no-op for an unmounted unit.
    /// Works on a dead unit's slot too (a unit that dies seated, or is shed or
    /// vacated, leaves the seat) and on a dead vehicle (its riders are
    /// released). The last rider out leaves an ordinary class neutral (§7.1).
    pub fn unseat_unit(&mut self, u: UnitHandle) {
        let endgame_command = self.unit(u).is_some_and(|unit| {
            unit.controller == Controller::Ai
                && unit
                    .mount
                    .and_then(|m| self.vehicle(m.vehicle))
                    .and_then(|v| self.walker_params.get(v.class as usize))
                    .and_then(|p| p.as_ref())
                    .is_some_and(|p| !p.spawn_points.is_empty())
                && crate::ai::walker_drive::command_endgame(self, unit.team)
        });
        let Some(unit) = self
            .units
            .get_mut(u.index as usize)
            .filter(|x| x.gen == u.gen)
        else {
            return;
        };
        // Death/despawn also comes here for a unit on foot. Preserve the
        // existing claim's release policy, but end its heartbeat immediately.
        if !unit.alive {
            unit.ai.pending_board = None;
            if let Some(r) = unit.ai.ride.as_mut() {
                if r.ordered_call || r.called_board.is_some() {
                    r.board = None;
                    r.called_board = None;
                    r.player_board = None;
                    r.ordered_call = false;
                    r.use_req = false;
                }
                r.maintenance = None;
            }
        }
        let human = matches!(unit.controller, Controller::Player { .. });
        let Some(m) = unit.mount.take() else {
            return;
        };
        if unit
            .order
            .is_some_and(|p| p.pickup.is_some() || p.order == crate::squad::Order::GetOut)
        {
            unit.order = None;
        }
        unit.ai.charge_until = 0;
        if endgame_command {
            unit.ai.ride.get_or_insert_default().command_dismounted = true;
        }
        if let Some(r) = unit.ai.ride.as_mut() {
            r.pickup = None;
            r.board = None;
            r.called_board = None;
            r.player_board = None;
            r.ordered_call = false;
            r.maintenance = None;
            if r.flyer.is_some_and(|f| f.ai.cursor.is_some()) {
                r.flyer = None;
            }
            r.bail_draw = None;
            r.bail_seen = false;
        }
        // An EVERYONE OUT press belongs to the issuer's current pilot tenure.
        // End it even if that same human reenters before the next think.
        if human {
            for receiver in &mut self.units {
                if receiver
                    .order
                    .is_some_and(|p| p.order == crate::squad::Order::GetOut && p.leader == u)
                {
                    receiver.order = None;
                }
            }
        }
        let Some(v) = self
            .vehicles
            .get_mut(m.vehicle.index as usize)
            .filter(|v| v.gen == m.vehicle.gen)
        else {
            return;
        };
        if let Some(s) = v.seats.get_mut(usize::from(m.seat)) {
            if s.occupant == Some(u) {
                s.occupant = None;
            }
        }
        // Detaching the follower releases route occupancy immediately, even
        // when another pilot enters before clear_driverless runs.
        if m.seat == 0 && v.kind == VehicleKind::Flyer && v.flyer.ai.cursor.is_some() {
            v.flyer.ai = crate::vehicle::flyer::FlyerAi::default();
        }
        // A pilot change ends a turret's zoom (card VTURZ, re-turret-draw 4).
        if let Some(Some(g)) = v.guns.get_mut(usize::from(m.seat)) {
            g.zoom = None;
        }
        if v.neutral_when_empty && v.is_empty() {
            v.team = None;
        }
    }

    /// Deal `desc` to a live vehicle's ONE hull pool (re-vehicles §6) through
    /// the same TakeDamage as a soldier's (`Health::take_damage`), so a bolt
    /// takes the ordnance's `VehicleScale` — or its HealthType's scale, e.g.
    /// BF1's tauntaun `AnimalScale`. Friendly = the attacker's team is the
    /// vehicle's live team; a neutral (empty) vehicle has no friends. At 0
    /// health it is destroyed in the same call ([`Sim::destroy_vehicle`]).
    /// Returns the health removed; 0 for a dead or stale handle.
    pub fn damage_vehicle(
        &mut self,
        h: VehicleHandle,
        desc: &DamageDesc,
        multiplier: f32,
        attacker: Option<(UnitHandle, u8)>,
    ) -> f32 {
        let (shooter, team) = (attacker.map(|(u, _)| u), attacker.map(|(_, t)| t));
        self.damage_vehicle_by(h, desc, multiplier, shooter, team, None)
    }

    /// [`Sim::damage_vehicle`] with the attacker's unit and team apart: a
    /// blast's team is its source's even when no unit is credited (an empty
    /// vehicle's death blast, [`Sim::explode`]), and the way the hit
    /// travelled (`Event::VehicleHit`'s `dir`).
    pub(super) fn damage_vehicle_by(
        &mut self,
        h: VehicleHandle,
        desc: &DamageDesc,
        multiplier: f32,
        shooter: Option<UnitHandle>,
        team: Option<u8>,
        dir: Option<Vec3>,
    ) -> f32 {
        let ff = self.friendly_fire_scale;
        let tick = self.tick;
        let crashes = self.vehicle(h).is_some_and(|v| {
            v.kind == VehicleKind::Flyer
                && self
                    .flyer_params
                    .get(v.class as usize)
                    .and_then(Option::as_ref)
                    .is_some_and(|p| p.bf2)
        });
        let Some(v) = self.vehicle_mut(h) else {
            return 0.0;
        };
        // A spent pool is a vehicle inside its own `destroy_vehicle` (its
        // riders' deaths come after the blast, which can chain back to it):
        // no second death.
        if !v.health.alive() {
            return 0.0;
        }
        let friendly = team.is_some() && v.team == team;
        let pool = (v.health.cur, v.health.shield);
        let damage = v.health.take_damage(desc, multiplier, friendly, ff);
        let landed = pool != (v.health.cur, v.health.shield);
        let victim_team = v.team;
        // Card TURR: a turret's gunner reads who hit it (`ai::man`).
        if let (Some(u), VehicleKind::Turret) = (shooter, v.kind) {
            v.last_hit = Some((u, tick));
        }
        if crashes {
            v.flyer.damager = shooter;
        }
        let killed = !v.health.alive();
        let in_flight = crashes
            && matches!(
                v.flyer.phase,
                FlightPhase::Takeoff | FlightPhase::Flying | FlightPhase::Landing
            );
        self.events.push(Event::VehicleHit {
            shooter,
            vehicle: h,
            damage,
            dir,
        });
        if let Some(shooter) = shooter.filter(|_| landed) {
            self.events.push(Event::HitLanded {
                shooter,
                team: victim_team,
            });
        }
        if killed && in_flight {
            self.flyer_starts_crash(h);
        } else if killed {
            self.destroy_vehicle(h, shooter);
        }
        damage
    }

    fn flyer_starts_crash(&mut self, h: VehicleHandle) {
        let has_chunks = self.vehicle(h).is_some_and(|v| {
            self.flyer_params
                .get(v.class as usize)
                .and_then(Option::as_ref)
                .is_some_and(|p| p.has_chunks)
        });
        let timer = if has_chunks {
            self.lcg.unit() * 2.0 + 0.2
        } else {
            600.0
        };
        if let Some(v) = self.vehicle_mut(h) {
            v.flyer.phase = FlightPhase::Falling;
            v.flyer.crash_timer = timer;
        }
    }

    /// A hull contact (re-vehicles §3.2 item 10): the class's threshold and
    /// scale rows through [`impact_damage`], times the other body's factor,
    /// dealt raw ([`collision_desc`]) with no attacker. The integrator that
    /// finds the contact is HOVR's.
    #[allow(clippy::too_many_arguments)]
    pub fn impact_vehicle(
        &mut self,
        h: VehicleHandle,
        normal_speed: f32,
        n_up: f32,
        n_fwd: f32,
        threshold: [f32; 3],
        scale: [f32; 3],
        other_factor: f32,
    ) -> f32 {
        let d = impact_damage(normal_speed, n_up, n_fwd, threshold, scale) * other_factor;
        if d <= 0.0 {
            return 0.0;
        }
        self.damage_vehicle(h, &collision_desc(d), 1.0, None)
    }

    /// The hull pool ran out (re-vehicles §6): in the same tick the PS2
    /// detonates the class's `ExplosionName`, throws the chunks and removes
    /// the vehicle — no wreck. The sim emits [`Event::VehicleDestroyed`],
    /// detonates the class's [`crate::vehicle::BlastSetup::explosion`] at its
    /// position ([`Sim::explode`], card VDTH), kills the riders and removes
    /// it; the chunks are the game layer's.
    ///
    /// The blast carries the PILOT seat's occupant as its attacker (the kill
    /// credit) and the vehicle's live team (re-vehicle-death §4 [O]: B1
    /// FUN_001ea9f8 B1:97302-97306 passes {entity+0x380, entity team}; that
    /// +0x380 is the driver is [D], and what it holds on an empty vehicle
    /// [H] — here: no attacker).
    ///
    /// The riders (re-vehicle-death §10 [O], card RIDK): after the blast, B1
    /// FUN_001ea190 / B2 FUN_0021a840 KILL the occupant of every turret
    /// seat, then every passenger seat, then the driver's (B1:96783-96806,
    /// B2:131271-131313), each with the vehicle's last-damage record — the
    /// killing hit's attacker, `killer` here — and past the invulnerability
    /// switch (third argument 0). So `Some(k)`: a kill of `k` per rider
    /// ([`Event::Kill`], a team kill if friendly, a suicide for a rider who
    /// destroyed his own ride); `None` (a crash into the world): a death with
    /// no killer ([`DeathCause::VehicleDestroyed`]). A ticket each, as any
    /// death. Each rider first takes this tick's carried pose, so he dies
    /// where he sits: the origin, or a building turret gunner's seat. The
    /// blast comes first with the riders still seated, so it spares them
    /// (§5); it does not gather the dying vehicle itself, nor
    /// the riders of a dying building turret (the PS2's turret path is not
    /// read; they die in the loop below either way).
    ///
    /// A walker does not go at once: it dies in place, plays its death clip
    /// and only then throws its chunks ([`walker::WalkerDeath`],
    /// [`Sim::start_walker_death`]).
    pub fn destroy_vehicle(&mut self, h: VehicleHandle, killer: Option<UnitHandle>) {
        let Some(v) = self.vehicle(h) else {
            return;
        };
        if v.kind == VehicleKind::Walker
            && self
                .walker_params
                .get(v.class as usize)
                .is_some_and(Option::is_some)
        {
            self.start_walker_death(h, killer);
            return;
        }
        let (class, pos, vel, team) = (v.class, v.pos, v.vel, v.team);
        let (riders, pilot) = self.riders_in_death_order(v);
        self.space_flyer_destroyed(h, killer);
        if self.vehicles[h.index as usize].kind == VehicleKind::Flyer {
            self.vehicles[h.index as usize].flyer.phase = FlightPhase::Crashed;
        }
        self.events.push(Event::VehicleDestroyed {
            vehicle: h,
            class,
            pos,
            vel,
            killer,
        });
        let exp = self
            .blast_setup
            .get(class as usize)
            .and_then(|b| b.as_ref())
            .and_then(|b| b.explosion.clone());
        self.events.push(Event::VehicleExplosion {
            vehicle: h,
            class,
            pos,
            last: false,
        });
        if let Some(e) = exp {
            self.explode_from(
                &e,
                pos,
                pilot,
                team,
                Some(super::blast::BlastSource::Vehicle(h)),
            );
        }
        self.kill_riders(riders, killer);
        self.remove_vehicle(h);
        // A building turret falls to its wreck or pad (card TBLD).
        self.fell_building_turret(h);
    }

    /// Every rider of `v` with where he sits (`rider_pose`), so he dies
    /// there, in the death function's order: turret seats, passenger seats,
    /// then the driver's (re-vehicle-death §10); and the driver alone.
    fn riders_in_death_order(&self, v: &Vehicle) -> (Vec<(UnitHandle, Vec3)>, Option<UnitHandle>) {
        let seated = |kind: fn(SeatKind) -> bool| -> Vec<(UnitHandle, Vec3)> {
            v.seats
                .iter()
                .enumerate()
                .filter(|(_, s)| kind(s.kind))
                .filter_map(|(i, s)| Some((s.occupant?, self.rider_pose(v, i as u8).0)))
                .collect()
        };
        let mut riders = seated(|k| matches!(k, SeatKind::Gunner(_)));
        riders.extend(seated(|k| matches!(k, SeatKind::Passenger(_))));
        let pilot_seat = seated(|k| k == SeatKind::Pilot).first().copied();
        riders.extend(pilot_seat);
        (riders, pilot_seat.map(|(u, _)| u))
    }

    /// Kill each rider where he sits, with the vehicle's last-damage record
    /// (re-vehicle-death §10): a kill of `killer`, or a death with no killer.
    fn kill_riders(&mut self, riders: Vec<(UnitHandle, Vec3)>, killer: Option<UnitHandle>) {
        for (u, at) in riders {
            if let Some(r) = self.unit_mut(u) {
                r.soldier.pos = at;
            }
            match killer {
                Some(k) => {
                    let Some(r) = self.unit_mut(u) else {
                        continue;
                    };
                    r.health.cur = 0.0;
                    self.unit_killed(u, Some(k));
                }
                None => self.kill_environment(u, DeathCause::VehicleDestroyed),
            }
        }
    }

    /// A walker's hull pool ran out ([`walker::WalkerDeath`]): its death
    /// function kills the riders (the health slot +0xc ahead of FUN_0025ce68,
    /// B2:179015-179018; B1:130333-130336 ahead of FUN_00218ac8), marks the
    /// walker dead and detonates `ExplosionName` at `ExplosionOffset`. The
    /// walker stays in the world, empty and solid, finishes the clip in hand
    /// and plays death01; [`Sim::finish_walker_death`] ends it.
    fn start_walker_death(&mut self, h: VehicleHandle, killer: Option<UnitHandle>) {
        let Some(v) = self.vehicle(h) else {
            return;
        };
        if v.walker.dead {
            return;
        }
        let (riders, _) = self.riders_in_death_order(v);
        if let Some(v) = self.vehicle_mut(h) {
            v.walker.dead = true;
            v.walker.killer = killer;
            v.health.cur = v.health.cur.min(0.0);
        }
        self.space_flyer_destroyed(h, killer);
        self.kill_riders(riders, killer);
        self.walker_blast(h, false);
    }

    /// FUN_0025f4a0's dead branch at entry 4 (B2:179231-179307; B1:130521-
    /// 130588): the vehicle-kill award and the chunks
    /// ([`Event::VehicleDestroyed`]), the final blast, then the walker is
    /// gone.
    pub(super) fn finish_walker_death(&mut self, h: VehicleHandle) {
        let Some(v) = self.vehicle(h) else {
            return;
        };
        let (class, pos, vel, killer) = (v.class, v.pos, v.vel, v.walker.killer);
        self.events.push(Event::VehicleDestroyed {
            vehicle: h,
            class,
            pos,
            vel,
            killer,
        });
        self.walker_blast(h, true);
        self.remove_vehicle(h);
    }

    /// The explosion class `class` detonates at its death
    /// ([`Event::VehicleExplosion`]): `ExplosionName`, or for a walker's
    /// `last` blast its `FinalExplosion` when it names one
    /// ([`walker::WalkerDeath`]).
    pub fn vehicle_explosion(
        &self,
        class: u32,
        last: bool,
    ) -> Option<&crate::weapon::ExplosionClass> {
        let name = self
            .blast_setup
            .get(class as usize)
            .and_then(Option::as_ref)
            .and_then(|b| b.explosion.as_ref());
        let fin = self
            .walker_params
            .get(class as usize)
            .and_then(Option::as_ref)
            .and_then(|p| p.death.final_explosion.as_ref())
            .filter(|_| last);
        fin.or(name)
    }

    /// One of a dying walker's two blasts ([`walker::WalkerDeath`]): the
    /// first is `ExplosionName` at `ExplosionOffset`, the `last` one
    /// `FinalExplosion` (else `ExplosionName`) at `FinalExplosionOffset`,
    /// each through the walker's frame. Both carry the destroyer as their
    /// attacker, with his team: their desc is the vehicle's record.
    fn walker_blast(&mut self, h: VehicleHandle, last: bool) {
        let Some(v) = self.vehicle(h) else {
            return;
        };
        let (class, killer) = (v.class, v.walker.killer);
        let Some(death) = self
            .walker_params
            .get(class as usize)
            .and_then(Option::as_ref)
            .map(|p| &p.death)
        else {
            return;
        };
        let offset = if last {
            death.final_offset
        } else {
            death.offset
        };
        let exp = self.vehicle_explosion(class, last).cloned();
        let at = v.pos + v.rot * offset;
        self.events.push(Event::VehicleExplosion {
            vehicle: h,
            class,
            pos: at,
            last,
        });
        let team = killer.and_then(|k| {
            self.units
                .get(k.index as usize)
                .filter(|u| u.gen == k.gen)
                .map(|u| u.team)
        });
        if let Some(e) = exp {
            self.explode_from(
                &e,
                at,
                killer,
                team,
                Some(super::blast::BlastSource::Vehicle(h)),
            );
        }
    }

    /// Remove a vehicle (the PS2 leaves no wreck, §6), releasing every rider
    /// still seated. A destroyed vehicle goes through [`Sim::destroy_vehicle`].
    pub fn remove_vehicle(&mut self, h: VehicleHandle) {
        let Some(v) = self.vehicle(h) else {
            return;
        };
        let riders: Vec<UnitHandle> = v.seats.iter().filter_map(|s| s.occupant).collect();
        for u in riders {
            self.unseat_unit(u);
        }
        self.vehicles[h.index as usize].alive = false;
        for u in &mut self.units {
            if u.ai.pending_board.is_some_and(|(_, vh)| vh == h) {
                u.ai.pending_board = None;
            }
            if let Some(r) = u.ai.ride.as_mut().filter(|r| r.board == Some(h)) {
                if r.player_board.is_some() {
                    r.board = None;
                    r.use_req = false;
                }
                r.called_board = None;
                r.player_board = None;
                r.maintenance = None;
            }
        }
        self.collision.remove_hull(h);
        self.collision.unbind_turret(h);
    }

    /// Make placed instance `instance`'s bodies building turret `h`'s
    /// building (playtest 2026-10-28, `analysis/re-turret-collision.md` 1.2, 3.1, 4):
    /// a round that meets one damages the turret's pool through its
    /// HealthType (building) scale, a blast measures it past its own bodies,
    /// and at 0 the turret is destroyed with its gunner. The rider's own
    /// rounds still stop on it (`Bolt::pass_own_building`).
    pub fn bind_building(&mut self, h: VehicleHandle, instance: u32) {
        let Some(v) = self.vehicle_mut(h) else {
            return;
        };
        v.instance = Some(instance);
        self.collision.bind_turret(instance, h);
    }

    /// Step 3a: every live vehicle whose class has [`hover::HoverParams`], in slot
    /// order — `hover::step` against the heightfield and the world, then the
    /// impacts its bottomed springs took (`Sim::impact_vehicle`, factor 1).
    pub(crate) fn tick_hovers(&mut self, dt: f32) {
        for i in 0..self.vehicles.len() {
            let v = &self.vehicles[i];
            if !v.alive {
                continue;
            }
            let endurance = self.driver_medal(i, super::awards::ENDURANCE_MEDAL);
            let v = &self.vehicles[i];
            let Some(p) = self
                .hover_params
                .get(v.class as usize)
                .and_then(|p| p.as_ref())
            else {
                continue;
            };
            let boosted;
            let p = if endurance {
                boosted = hover::HoverParams {
                    energy_auto_restore: endurance_rate(p.energy_auto_restore),
                    ..p.clone()
                };
                &boosted
            } else {
                p
            };
            let world = WorldContacts {
                heightfield: self.heightfield.as_ref(),
                collision: &self.collision,
                water: self.water_level.map(|level| crate::water::Plane {
                    level: level
                        + if self.water_oscillates {
                            crate::water::bob(self.tick as f32 * crate::TICK_DT)
                        } else {
                            0.0
                        },
                    mask: self.water_mask.as_ref(),
                }),
            };
            let from = v.pos;
            let held_up = v.rot * Vec3::Y;
            hover::drive(&mut self.vehicles[i], p, dt);
            let mut impacts = hover::contacts(&mut self.vehicles[i], p, &world);
            super::hover_pairs::contacts(
                &mut self.vehicles,
                i,
                p,
                &self.hover_params,
                &self.hull_parts,
                &self.collision,
                &mut impacts,
            );
            hover::springs(&mut self.vehicles[i], p, |at| world.water_under(at), dt);
            let (threshold, scale) = (p.collision_threshold, p.collision_scale);
            self.hold_pilot_aim(i, held_up);
            self.vehicle_regen(i, dt);
            self.keep_out(i, from);
            self.ground_traffic(i, from);
            if impacts.is_empty() {
                continue;
            }
            let h = VehicleHandle {
                index: i as u32,
                gen: self.vehicles[i].gen,
            };
            for im in impacts {
                self.impact_vehicle(h, im.normal_speed, im.n_up, im.n_fwd, threshold, scale, 1.0);
            }
        }
    }

    /// re-hover §5.1 I10-I11: the drive's own pitch this tick comes off the
    /// pilot's aim pitch (`aimPitch -= oldUp . E'.fwd`, B2:131896-131935), so
    /// the aim holds its world direction while the body tips, then the aim is
    /// clamped to `PitchLimits` again. PS2 RAM (KAS2, the IFT-X parked on a
    /// slope 11 deg nose up): every hover's aim direction E+0x350 is level.
    fn hold_pilot_aim(&mut self, i: usize, held_up: Vec3) {
        let v = &self.vehicles[i];
        let Some(a) = self
            .seat_setup
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .and_then(|s| s.guns.first())
            .and_then(Option::as_ref)
            .filter(|g| g.pilot && g.turn.is_none())
            .map(|g| g.aimer)
        else {
            return;
        };
        let fwd = v.rot * Vec3::Z;
        if let Some(Some(st)) = self.vehicles[i].guns.first_mut() {
            st.look.1 = (st.look.1 - held_up.dot(fwd)).clamp(a.pitch[0], a.pitch[1]);
        }
    }

    /// Step 3a, after the hovers: every live vehicle whose class has
    /// [`walker::WalkerParams`], in slot order — `walker::step` against the
    /// heightfield and the world.
    pub(crate) fn tick_walkers(&mut self, dt: f32) {
        for i in 0..self.vehicles.len() {
            if self.vehicles[i].yield_until != 0 && self.tick >= self.vehicles[i].yield_until {
                self.vehicles[i].yield_until = 0;
            }
            let v = &self.vehicles[i];
            if !v.alive {
                continue;
            }
            let endurance = self.driver_medal(i, super::awards::ENDURANCE_MEDAL);
            let v = &self.vehicles[i];
            let Some(p) = self
                .walker_params
                .get(v.class as usize)
                .and_then(|p| p.as_ref())
            else {
                continue;
            };
            let boosted;
            let p = if endurance {
                boosted = walker::WalkerParams {
                    energy_auto_restore: endurance_rate(p.energy_auto_restore),
                    ..p.clone()
                };
                &boosted
            } else {
                p
            };
            let world = WorldContacts {
                heightfield: self.heightfield.as_ref(),
                collision: &self.collision,
                water: self.water_level.map(|level| crate::water::Plane {
                    level: level
                        + if self.water_oscillates {
                            crate::water::bob(self.tick as f32 * crate::TICK_DT)
                        } else {
                            0.0
                        },
                    mask: self.water_mask.as_ref(),
                }),
            };
            if walker::death_finished(v) {
                let h = VehicleHandle {
                    index: i as u32,
                    gen: v.gen,
                };
                self.finish_walker_death(h);
                continue;
            }
            let from = v.pos;
            walker::step(&mut self.vehicles[i], p, &world, dt);
            self.vehicle_regen(i, dt);
            self.keep_out(i, from);
            self.ground_traffic(i, from);
            // Card WAI's sweep counter (OUTPUT only).
            let v = &self.vehicles[i];
            let bot = v
                .seats
                .first()
                .filter(|s| s.kind == SeatKind::Pilot)
                .and_then(|s| s.occupant)
                .and_then(|o| self.unit(o))
                .is_some_and(|u| u.controller == Controller::Ai);
            if bot {
                self.nav_stats.ai_walker_cm +=
                    (crate::nav::xz_dist(from, v.pos) * 100.0).round() as u64;
            }
        }
    }

    /// Card VMESH (ours): a vehicle whose step took its centre from `from`,
    /// outside every map body, into a watertight one ([`Sim::shut_in_solid`]) is put
    /// back at `from` with its horizontal velocity gone. The contact pass is
    /// discrete, and against a body that is one shell of faces a speeder
    /// bike's hull crossed BF1 Endor's cp2 boulder at 23 m/s and a walker
    /// walked its centre into the boulder's low edge; inside, the contacts
    /// push it further in. What the PS2's own bike does at that wall is
    /// unmeasured.
    fn keep_out(&mut self, i: usize, from: Vec3) {
        let to = self.vehicles[i].pos;
        if to == from || self.shut_in_solid(to).is_none() || self.shut_in_solid(from).is_some() {
            return;
        }
        let v = &mut self.vehicles[i];
        v.pos = from;
        v.vel.x = 0.0;
        v.vel.z = 0.0;
    }

    /// Register a spawn pad (card VPAD); returns its index (the vehicles'
    /// `spawn_pad`).
    pub fn add_vehicle_pad(&mut self, def: PadDef) -> usize {
        self.vehicle_pads.push(VehiclePad::new(def));
        self.vehicle_pads.len() - 1
    }

    /// Step 7: every pad, in index order (B1 FUN_002b0780; BF2 FUN_00373138).
    /// It follows its post's owner, drops its dead vehicles, decays each live
    /// one, then counts down to a spawn while it has fewer than `SpawnCount`
    /// and a class for its team.
    pub(crate) fn tick_vehicle_pads(&mut self, dt: f32) {
        for pi in 0..self.vehicle_pads.len() {
            if let Some(ci) = self.vehicle_pads[pi].def.cp {
                let owner = self.cps.get(ci).and_then(|c| c.owner);
                self.vehicle_pads[pi].follow(owner);
            }
            let mut k = 0;
            while k < self.vehicle_pads[pi].vehicles.len() {
                let h = self.vehicle_pads[pi].vehicles[k].handle;
                if self.vehicle(h).is_none() {
                    self.vehicle_pads[pi].vehicles.remove(k);
                    continue;
                }
                self.decay_pad_vehicle(pi, k, dt);
                k += 1;
            }
            if self.vehicle_pads[pi].wants_spawn() && self.pad_box_taken(pi) {
                continue;
            }
            let pad = &mut self.vehicle_pads[pi];
            if !pad.wants_spawn() || !pad.count_down(dt) {
                continue;
            }
            let class = pad.def.class_for(pad.team).expect("wants_spawn");
            // OURS (PROVISIONAL; the PS2's own test is not read): the craft
            // waits for a soldier on foot to leave its footprint. Made on top
            // of him, its hull would shut him in for good (census `roam`, BF1
            // Bespin's Y-wing pads). The countdown has run; the first tick he
            // is clear it comes.
            let (at, reach) = (pad.def.pos, class.radius + SOLDIER_RADIUS);
            if self.units.iter().any(|u| {
                u.alive
                    && u.mount.is_none()
                    && (u.soldier.pos.y - at.y).abs() < PAD_HOLD_RISE
                    && (u.soldier.pos - at).with_y(0.0).length_squared() < reach * reach
            }) {
                continue;
            }
            let team = self.vehicle_pads[pi].team;
            self.spawn_from_pad(pi, team);
        }
    }

    /// Spawn a vehicle from pad `pi` for `team` immediately, registering it
    /// on the pad. Used by `tick_vehicle_pads` and `--vehicle` CLI startup.
    pub fn spawn_from_pad(&mut self, pi: usize, team: Option<u8>) -> Option<VehicleHandle> {
        let pad = self.vehicle_pads.get(pi)?;
        let class = pad.def.class_for(team)?;
        let live = pad
            .vehicles
            .iter()
            .filter(|v| self.vehicle(v.handle).is_some())
            .count();
        if live >= pad.def.spawn_count.max(1) as usize || self.pad_box_taken_for(pi, team) {
            return None;
        }
        let mut spec = class.spec.clone();
        spec.home_team = team;
        spec.spawn_pad = Some(pi as u32);
        spec.yaw = pad.def.yaw;
        // B1:240463-240510 rays down through the pad from 1 m over it to
        // 1 m under it and puts the vehicle on the hit, else at the pad's
        // own point (re-vehicle-rest §1). A flyer's constructor then adds
        // its class's LandedHeight, and nothing moves it while it is
        // landed (§2, §3; card VREST). PROVISIONAL: the ray is blind to
        // the hulls (the PS2's meets a landed flyer's body), and a hover
        // or walker keeps the soldier's ground probe to any depth — the
        // PS2 creates it in the air over 14 of 215 pads and it falls to
        // the same place (§1).
        let p = pad.def.pos;
        let landed_height = class.landed_height;
        spec.pos = match landed_height {
            Some(lh) => {
                let g = self.floor_point_under_static(p, p.y + PAD_RAY);
                let y = if (p.y - PAD_RAY..=p.y + PAD_RAY).contains(&g) {
                    g
                } else {
                    p.y
                };
                Vec3::new(p.x, y + crate::vehicle::flyer::rest_height(lh), p.z)
            }
            None => {
                let g = self.ground_under_with(p, false, HullFilter::Static);
                if g > NO_GROUND {
                    Vec3::new(p.x, g, p.z)
                } else {
                    p
                }
            }
        };
        let h = self.spawn_vehicle(&spec);
        self.vehicle_pads[pi].team = team;
        self.vehicle_pads[pi].spawned(h);
        Some(h)
    }

    fn pad_box_taken(&self, pi: usize) -> bool {
        self.pad_box_taken_for(pi, self.vehicle_pads[pi].team)
    }

    fn pad_box_taken_for(&self, pi: usize, team: Option<u8>) -> bool {
        let pad = &self.vehicle_pads[pi];
        let Some((lo, hi)) = pad.def.class_for(team).and_then(|class| class.vis_box) else {
            return false;
        };
        let (centre, half) = ((lo + hi) * 0.5, (hi - lo) * 0.5);
        let inverse = crate::vehicle::hover::quat_y(-pad.def.yaw).inverse();
        pad.vehicles.iter().any(|k| {
            self.vehicle(k.handle).is_some_and(|v| {
                let (at, radius) = self.vehicle_blast_sphere(v);
                let local = inverse * (at - pad.def.pos) - centre;
                let out = (local.abs() - half).max(Vec3::ZERO);
                out.length_squared() < radius * radius
            })
        })
    }

    /// The idle decay of pad `pi`'s `k`-th vehicle (B1 FUN_002afa90, [O]):
    /// - no post, or the vehicle has a live team (occupied, or a command
    ///   class) → the idle count resets and nothing happens;
    /// - any seat taken → the same;
    /// - it idles toward 20 s when outside its post's ControlRegion (or the
    ///   post has none), toward 10 s when the pad's class for its current
    ///   team is not the one it spawned, and not at all otherwise;
    /// - once a second, unless a live player is within r_player plus
    ///   r_vehicle plus 10 m, the count grows by 1; past the threshold the hull takes
    ///   MaxHealth / DecayTime, raw (all seven HealthType scales 1,
    ///   B1:240330-240338).
    ///
    /// [H]: the PS2 also asks each player's entity a vtable question about
    /// the vehicle (+0x48) before measuring; the port measures every live
    /// player.
    fn decay_pad_vehicle(&mut self, pi: usize, k: usize, dt: f32) {
        let pad = &self.vehicle_pads[pi];
        let pv = pad.vehicles[k];
        let v = self.vehicle(pv.handle).expect("pruned");
        let (pos, max, occupied, team) = (v.pos, v.health.max, !v.is_empty(), v.team);
        let cp = pad.def.cp.and_then(|ci| self.cps.get(ci));
        let Some(cp) = cp.filter(|_| team.is_none() && !occupied) else {
            self.vehicle_pads[pi].vehicles[k].idle = 0.0;
            return;
        };
        let threshold = if !cp.def.control.as_ref().is_some_and(|r| r.contains(pos)) {
            IDLE_OUTSIDE
        } else if pad.def.class_id(pv.spawned_team) != pad.def.class_id(pad.team) {
            IDLE_CLASS_CHANGED
        } else {
            return;
        };
        let r_vehicle = pad.def.class_for(pv.spawned_team).map_or(0.0, |c| c.radius);
        let decay_time = pad.def.decay_time;
        let pv = &mut self.vehicle_pads[pi].vehicles[k];
        pv.tick -= dt;
        if pv.tick >= 0.0 {
            return;
        }
        pv.tick += 1.0;
        let reach = SOLDIER_RADIUS + r_vehicle + PLAYER_MARGIN;
        let near = self.units.iter().any(|u| {
            u.alive && u.is_player && u.soldier.pos.distance_squared(pos) <= reach * reach
        });
        if near {
            return;
        }
        let pv = &mut self.vehicle_pads[pi].vehicles[k];
        pv.idle += 1.0;
        if pv.idle <= threshold {
            return;
        }
        let h = pv.handle;
        self.damage_vehicle(h, &collision_desc(max / decay_time), 1.0, None);
    }

    /// The seat a unit rides in, if any.
    pub fn seat_of(&self, u: UnitHandle) -> Option<(VehicleHandle, SeatKind)> {
        let m = self.unit(u)?.mount?;
        let s = self.vehicle(m.vehicle)?.seats.get(usize::from(m.seat))?;
        Some((m.vehicle, s.kind))
    }
}

#[cfg(test)]
mod tests;
