use super::state::Sim;
use crate::collision::HullFilter;
use crate::projectile::{Bolt, Hit, Shooter, Target};
use glam::Vec3;

/// How many objects one beam ray may pass, its owner counted: BF2's beam
/// keeps a list of 128 (`FUN_00311d00`, B2:303281) and stops on the hit
/// that finds it full.
const BEAM_PASS_LIMIT: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beam {
    pub shooter: Shooter,
    pub hulls: HullFilter,
    pub charge: f32,
    pub origin: Vec3,
    pub dir: Vec3,
    pub life: f32,
    pub len: f32,
    pub follow: bool,
    /// The seat gun that fired it, whose fire point it rides.
    pub seat: Option<SeatBeam>,
}

/// A seat-fired beam's fire point (B2 `FUN_00311b50`, B2:303059: the firing
/// weapon's fire point nearest the spawn, held at +0x114; the update
/// `FUN_00311d00` re-reads origin and direction from it every frame while
/// the weapon stands, B2:303166-303185).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeatBeam {
    pub mount: crate::vehicle::Mount,
    /// The firing weapon's index in its seat.
    pub weapon: u8,
    /// Which of the weapon's `fire_points` it left; `None` = the seat's
    /// muzzle along its aim.
    pub point: Option<u8>,
}

impl Beam {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&[0xbe, u8::from(self.follow)]);
        for v in [
            self.origin.x,
            self.origin.y,
            self.origin.z,
            self.dir.x,
            self.dir.y,
            self.dir.z,
            self.life,
            self.len,
            self.charge,
        ] {
            eat(&v.to_bits().to_le_bytes());
        }
        eat(&self.shooter.owner.index.to_le_bytes());
        eat(&self.shooter.owner.gen.to_le_bytes());
        eat(&(self.shooter.weapon_class as u64).to_le_bytes());
        if let Some(s) = self.seat {
            eat(&s.mount.vehicle.index.to_le_bytes());
            eat(&s.mount.vehicle.gen.to_le_bytes());
            eat(&[s.mount.seat, s.weapon, s.point.map_or(u8::MAX, |p| p)]);
        }
    }
}

impl Sim {
    pub(super) fn launch_round(&mut self, round: Bolt, follow: bool) {
        self.launch_round_from(round, follow, None);
    }

    /// [`Self::launch_round`] of a seat's gun: a beam rides the seat's fire
    /// point.
    pub(super) fn launch_round_from(&mut self, round: Bolt, follow: bool, seat: Option<SeatBeam>) {
        let Some(class) = &self.weapon_classes[round.weapon_class].ordnance.beam else {
            self.bolts.push(round);
            return;
        };
        self.beams.push(Beam {
            shooter: round.shooter(),
            hulls: round.hulls,
            charge: round.charge,
            origin: round.pos,
            dir: round.vel.normalize_or_zero(),
            life: class.life,
            len: 0.0,
            follow,
            seat,
        });
    }

    pub(super) fn fly_bolts(&mut self, dt: f32) {
        let first = self.cast_beams(dt);
        let count = self.beams.len();
        self.sweep_bolts(dt);
        let mut done = Vec::new();
        for (i, b) in self.bolts.iter_mut().enumerate() {
            if b.advance(dt) {
                done.push(i);
            }
            if let Some(shot) = b.visual_shot.filter(|shot| shot.tick == self.tick) {
                self.shot_flights
                    .push(crate::projectile::ShotFlight { shot, end: b.end });
            }
        }
        // Where each beam's ray ended this tick: its round's end, or the
        // last hit of a piercing beam's chain.
        let mut ends: Vec<Vec3> = self.bolts[first..first + count]
            .iter()
            .map(|b| b.end)
            .collect();
        for &i in &done {
            let b = self.bolts[i].clone();
            match i.checked_sub(first).filter(|&k| k < count) {
                Some(k) if self.beam_pierces(k) => ends[k] = self.pierce_beam(k, b, dt),
                _ => self.resolve_bolt(&b),
            }
        }
        self.settle_beams(&ends, dt);
        self.bolts.retain(|b| b.remaining > 0.0);
    }

    fn cast_beams(&mut self, dt: f32) -> usize {
        let first = self.bolts.len();
        for k in 0..self.beams.len() {
            let beam = self.beams[k];
            let (origin, dir) = match beam.seat {
                Some(s) => self
                    .seat_beam_frame(s, beam.shooter.owner)
                    .unwrap_or((beam.origin, beam.dir)),
                None if beam.follow => self.beam_frame(beam),
                None => (beam.origin, beam.dir),
            };
            let ord = &self.weapon_classes[beam.shooter.weapon_class].ordnance;
            let range = ord.beam.as_ref().map_or(0.0, |c| c.range);
            let mut bolt = Bolt::fire(
                origin,
                dir,
                range / dt,
                dt,
                beam.shooter,
                self.heightfield.as_ref(),
                self.water_plane(),
                &self.collision,
                beam.hulls,
            );
            bolt.charge = beam.charge;
            self.beams[k].origin = origin;
            self.beams[k].dir = dir;
            self.bolts.push(bolt);
        }
        first
    }

    /// Does beam `k` go on through what it strikes? Its class's
    /// PassThrough, on a ground round only: space keeps the first hit
    /// stopping every beam (`Sim::space_round`).
    fn beam_pierces(&self, k: usize) -> bool {
        !self.space_round()
            && self.weapon_classes[self.beams[k].shooter.weapon_class]
                .ordnance
                .beam
                .as_ref()
                .is_some_and(|c| c.pierce)
    }

    /// A PassThrough beam's ray this tick (BF2 `FUN_00311d00`,
    /// B2:303242-303353; `re-ordnance-kinds.md` §9b): every soldier,
    /// vehicle, droid, turret, mine or rocket it crosses takes its share,
    /// once, and the ray goes on from that hit with the range left. It
    /// ends at terrain, water, a building or other map body, a saber that
    /// turns it, or the end of its range, and after its 128th object (the
    /// original's list of 128 passed objects, the owner first, is full).
    /// `first` is beam `k`'s round, already swept and flown; the result is
    /// where the ray ended.
    fn pierce_beam(&mut self, k: usize, first: Bolt, dt: f32) -> Vec3 {
        let range = self.weapon_classes[first.weapon_class]
            .ordnance
            .beam
            .as_ref()
            .map_or(0.0, |c| c.range);
        let (origin, dir, charge) = (self.beams[k].origin, self.beams[k].dir, first.charge);
        let speed = range / dt;
        let mut passed = Vec::new();
        let mut bodies = None;
        let mut b = first;
        loop {
            if self.resolve_bolt_turned(&b) {
                return b.end;
            }
            let Some(target) = self.beam_passes(b.hit) else {
                return b.end;
            };
            if passed.len() + 1 >= BEAM_PASS_LIMIT {
                return b.end;
            }
            passed.push(target);
            let left = range - (b.end - origin).length();
            if left <= 0.0 {
                return b.end;
            }
            let mut next = Bolt::fire(
                b.end,
                dir,
                speed,
                left / speed,
                b.shooter(),
                self.heightfield.as_ref(),
                self.water_plane(),
                &self.collision,
                b.hulls,
            );
            next.charge = charge;
            let bodies = bodies.get_or_insert_with(|| self.sweep_bodies());
            self.sweep_bolt(bodies, &mut next, dt, &passed);
            next.advance(dt);
            b = next;
        }
    }

    /// What a piercing beam goes on through: anything but the ground and
    /// the map. A building turret's body is its building, and stops it.
    fn beam_passes(&self, hit: Hit) -> Option<Target> {
        let Hit::Target(t) = hit else {
            return None;
        };
        match t {
            Target::Unit(_)
            | Target::Shield(_)
            | Target::Mine(_)
            | Target::Droid(_)
            | Target::Turret(_)
            | Target::Guided(_) => Some(t),
            Target::Vehicle(v) => self.collision.turret_instance(v).is_none().then_some(t),
            Target::Station(_) | Target::Building(_) => None,
        }
    }

    fn settle_beams(&mut self, ends: &[Vec3], dt: f32) {
        for (beam, end) in self.beams.iter_mut().zip(ends) {
            beam.len = (*end - beam.origin).length();
            beam.life -= dt;
        }
        self.beams.retain(|b| b.life > 0.0);
    }

    /// A seat beam's fire point now: the weapon's fire point, or the
    /// seat's muzzle, on the vehicle as it stands, along the gun's aim.
    /// A pilot's barrel keeps converging on the camera's target, including
    /// a one-point ring below the seat's other weapon.
    /// `None` once the vehicle or its gun is gone: the beam keeps its ray.
    fn seat_beam_frame(&self, s: SeatBeam, owner: crate::unit::UnitHandle) -> Option<(Vec3, Vec3)> {
        let v = self.vehicle(s.mount.vehicle).filter(|v| v.alive)?;
        let seat = usize::from(s.mount.seat);
        let g = self
            .seat_setup
            .get(v.class as usize)?
            .as_ref()?
            .guns
            .get(seat)?
            .as_ref()?;
        let st = v.guns.get(seat)?.as_ref()?;
        let fixed = g.pilot && v.kind == crate::vehicle::VehicleKind::Flyer;
        let local = g.shot_point(
            usize::from(s.weapon),
            s.point.map(usize::from),
            st.aim,
            fixed,
        );
        let dir = (v.rot * crate::vehicle::gun::dir(st.aim.0, st.aim.1)).normalize_or_zero();
        let origin = v.pos + v.rot * local;
        let weapon = g.weapons.get(usize::from(s.weapon))?;
        let dir = if g.turn.is_none() && !fixed && !weapon.fire_points.is_empty() {
            let view = v.rot * crate::vehicle::gun::dir(st.look.0, st.look.1);
            let eye = v.pos + v.rot * g.ray_eye(st.look, st.zoom.is_some());
            let eye = self.chase_ray_origin(
                s.mount,
                g,
                self.unit(owner).map(|_| owner.index as usize),
                st.look,
                st.zoom.is_some(),
                eye,
                view,
            );
            let riders: Vec<_> = v.seats.iter().filter_map(|s| s.occupant).collect();
            let target = self.aim_point(eye, view, &riders, s.mount.vehicle);
            super::guns::ring_aim(
                g,
                weapon.fire_limits.get(usize::from(s.point.unwrap_or(0))),
                v.rot,
                origin,
                target,
                dir,
            )
            .0
        } else {
            dir
        };
        Some((origin, dir))
    }

    fn beam_frame(&self, beam: Beam) -> (Vec3, Vec3) {
        let h = beam.shooter.owner;
        if self.unit(h).is_none_or(|u| u.mount.is_some()) {
            return (beam.origin, beam.dir);
        }
        let (origin, (fwd, _, _)) = self.shot_frame(h.index as usize);
        (origin, fwd)
    }
}

#[cfg(test)]
mod tests;
