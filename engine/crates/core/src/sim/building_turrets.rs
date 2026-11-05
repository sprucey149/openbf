//! A map's building turrets (cards VTURB, TBLD): how each is spawned on its
//! building, and its fall and rise. The PS2's armed building has a build
//! state (`analysis/re-build-zones.md` 1 [O][V]): 0 built, 1 destroyed, 2
//! unbuilt. At 0 health a built one falls - to its `DestroyedGeometryName`
//! wreck (state 1) when the class has one, else to its unbuilt model
//! (`UnbuiltGeometryName`, the "little square" `com_prop_buildzone`, state 2)
//! when it has that, else it is hidden (state 1) - and its model AND its
//! collision become the fallen look's (B2 `FUN_001e3638`, B2:93584-93660).
//! Only when its health reaches MaxHealth is it built again (the health
//! handler `FUN_001e3458`, B2:93483-93492): state 0, the class's model,
//! collision and full health. The fusion cutter adds to that health shot by
//! shot ([`Sim::repair_building`]): `BuildingRebuild` on a wreck,
//! `BuildingBuild` on a pad.
//!
//! **No regeneration** (`[ours]`, the maintainer 2026-10-28: "when a turret is destroyed
//! it can only be fixed by a fusion cutter for all maps. It shouldn't
//! magically heal itself after some set time"). The PS2 sets a fallen
//! building's regeneration to MaxHealth / `RespawnTime` (B2:93789; BF1's gun
//! turret 2.5 HP a second measured, BF2's TUR turrets 999999.9 s, Geonosis's
//! cannon 30 s); the port reads no `RespawnTime` and a fallen turret stays
//! down until a cutter builds it (bots: `crate::ai::fix`).
//!
//! The port: a fallen turret is no vehicle (its seats and pool are gone, as
//! the PS2's alive bit is clear); its [`BuildingTurret`] keeps the health it
//! regains, and its instance's bodies are the fallen look's until it stands
//! again as a fresh, neutral, empty turret at full health.

use super::state::Sim;
use crate::collision::{Aabb, HullPart};
use crate::vehicle::gun::Rig;
use crate::vehicle::{VehicleHandle, VehicleSpec};
use crate::weapon::HealthType;
use glam::{Mat4, Quat, Vec3};

/// What a fallen building turret looks like (and collides as).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurretFall {
    /// Its `DestroyedGeometryName` (state 1).
    Wreck,
    /// Its unbuilt model, the build pad (state 2).
    Pad,
    /// Nothing at all (state 1, a class with neither model).
    Hidden,
}

/// A map's building turret: its spawn, its building, its fall and rise.
#[derive(Clone)]
pub struct BuildingTurret {
    pub spec: VehicleSpec,
    pub rot: Quat,
    pub aim_sphere: Option<(Vec3, f32)>,
    /// The placed instance whose bodies are its building.
    pub instance: u32,
    /// The turret standing now (dead while down).
    pub handle: VehicleHandle,
    /// What it falls to.
    pub fall: TurretFall,
    /// The fallen look's bodies in the instance's space (none for
    /// [`TurretFall::Hidden`] or a model with no collision).
    pub fallen_parts: Vec<HullPart>,
    /// The standing building's bodies in the instance's space, taken from
    /// the world when it is added.
    pub standing_parts: Vec<HullPart>,
    /// Its health while down; `None` while it stands.
    pub down: Option<f32>,
    /// Its prims on the nodes its gun moves (card TCOL), `None` when it has
    /// none.
    pub posed: Option<PosedParts>,
    pub self_pilots: Vec<super::self_turrets::SelfPilot>,
}

/// A building turret's collision prims that hang on a node its gunner seat
/// moves (the turret, aimer or a barrel node). The PS2 places every body of
/// an armed building through the same node table its turret writes the
/// live gun into (`analysis/re-turret-collision.md` 0.1, 1.1-1.6), so a prim
/// on the aimer node turns and pitches with the gun: Geonosis's cannon has 3
/// of its 4 prims on `aimer_x`, its whole gun from 1.6 to 3.2 m.
/// PROVISIONAL (ours): placed from the seat's gun state of the tick before
/// (the turret's task runs before the aimer's, re-turret-draw 2.3), barrels
/// at rest (the recoil slide does not move a barrel's prim).
#[derive(Clone, Debug)]
pub struct PosedParts {
    pub rig: Rig,
    /// The gunner seat whose `GunState` poses the rig.
    pub seat: usize,
    pub bodies: Vec<PosedBody>,
    /// The (look, aim) bits the bodies stand at now.
    pub at: Option<[u32; 4]>,
}

/// One posed body: its index in the collision world, its node, and its
/// matrix in the instance's space with the rig at its bind pose.
#[derive(Clone, Copy, Debug)]
pub struct PosedBody {
    pub body: usize,
    pub node: usize,
    pub rest: Mat4,
}

impl BuildingTurret {
    /// The instance's matrix.
    pub fn pose(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rot, self.spec.pos)
    }

    /// Is it broken: down, or standing below MaxHealth (`health`: the
    /// standing turret's)? What a cutter carrier goes to fix.
    pub fn broken(&self, health: Option<f32>) -> bool {
        self.down.is_some() || health.is_some_and(|h| h < self.spec.max_health)
    }

    /// The cutter's amount index while down (B2:93593-93620, written when
    /// the class's HealthType is building): 6 `BuildingBuild` on a pad, 5
    /// `BuildingRebuild` otherwise; standing, its HealthType.
    pub fn repair_kind(&self) -> HealthType {
        match (self.down, self.spec.health_type, self.fall) {
            (Some(_), HealthType::Building, TurretFall::Pad) => HealthType::Building3,
            (Some(_), HealthType::Building, _) => HealthType::Building2,
            _ => self.spec.health_type,
        }
    }

    /// The aim sphere's centre and radius in the world.
    pub fn sphere(&self) -> (Vec3, f32) {
        let (c, r) = self.aim_sphere.unwrap_or((Vec3::ZERO, 0.0));
        (self.spec.pos + self.rot * c, r)
    }
}

impl Sim {
    /// Spawn a map's building turret ([`BuildingTurret`]: its spec, pose,
    /// aim sphere, building, and what it falls to with that look's bodies)
    /// and keep it, so that once destroyed it falls and a cutter builds it
    /// again ([`Sim::repair_building`]).
    pub fn add_building_turret(
        &mut self,
        spec: VehicleSpec,
        rot: Quat,
        aim_sphere: Option<(Vec3, f32)>,
        instance: u32,
        fall: (TurretFall, Vec<HullPart>),
    ) -> VehicleHandle {
        let pose = Mat4::from_rotation_translation(rot, spec.pos);
        let standing_parts = self.collision.instance_parts(instance, pose);
        let mut t = BuildingTurret {
            spec,
            rot,
            aim_sphere,
            instance,
            handle: VehicleHandle { index: 0, gen: 0 },
            fall: fall.0,
            fallen_parts: fall.1,
            standing_parts,
            down: None,
            posed: None,
            self_pilots: Vec::new(),
        };
        t.handle = self.stand_turret(&t);
        self.building_turrets.push(t);
        self.building_turrets[self.building_turrets.len() - 1].handle
    }

    /// Initial BF2 build pad: retain the standing spec/rig but remove its
    /// empty vehicle without a death, explosion or regeneration. The old
    /// handle is invalidated by remove_vehicle and cannot alias a reused slot.
    pub fn start_building_unbuilt(&mut self, instance: u32) {
        let Some(t) = self
            .building_turrets
            .iter()
            .find(|t| t.instance == instance)
        else {
            return;
        };
        let h = t.handle;
        self.remove_vehicle(h);
        self.fell_building_turret(h);
    }

    fn stand_turret(&mut self, t: &BuildingTurret) -> VehicleHandle {
        let h = self.spawn_vehicle(&t.spec);
        if let Some(v) = self.vehicle_mut(h) {
            v.rot = t.rot;
            v.aim_sphere = t.aim_sphere;
        }
        self.bind_building(h, t.instance);
        h
    }

    /// The building turret standing as `h` has just been removed at 0
    /// health: it falls, at health 0, to its fallen look and bodies.
    pub(super) fn fell_building_turret(&mut self, h: VehicleHandle) {
        let Some(i) = self.building_turrets.iter().position(|t| t.handle == h) else {
            return;
        };
        let t = &mut self.building_turrets[i];
        t.down = Some(0.0);
        let (instance, pose) = (t.instance, t.pose());
        let parts = std::mem::take(&mut t.fallen_parts);
        self.collision.replace_instance(instance, &parts, pose);
        self.building_turrets[i].fallen_parts = parts;
    }

    /// Is the building turret on `instance` down (fallen, not yet built
    /// again)? The viewer draws its fallen look while it is.
    pub fn building_down(&self, instance: u32) -> bool {
        self.building_turrets
            .iter()
            .any(|t| t.instance == instance && t.down.is_some())
            || self
                .stations
                .iter()
                .any(|s| s.instance == Some(instance) && s.destroyed)
    }

    pub fn building_hidden(&self, instance: u32) -> bool {
        self.building_turrets
            .iter()
            .any(|t| t.instance == instance && t.down.is_some() && t.fall == TurretFall::Hidden)
    }

    /// Add `amount` to fallen building turret `i`'s health; at MaxHealth it
    /// is built again. Returns the health gained (0 on a standing one).
    pub fn repair_building(&mut self, i: u16, amount: f32) -> f32 {
        let Some(t) = self.building_turrets.get_mut(usize::from(i)) else {
            return 0.0;
        };
        let (Some(before), true) = (t.down, amount > 0.0 && amount.is_finite()) else {
            return 0.0;
        };
        let after = (before + amount).min(t.spec.max_health);
        t.down = Some(after);
        if after >= t.spec.max_health {
            self.raise_building_turret(usize::from(i));
        }
        after - before
    }

    /// Built again: its standing bodies back, a neutral, empty turret at
    /// full health on its building.
    fn raise_building_turret(&mut self, i: usize) {
        let t = self.building_turrets[i].clone();
        self.collision
            .replace_instance(t.instance, &t.standing_parts, t.pose());
        let h = self.stand_turret(&t);
        let t = &mut self.building_turrets[i];
        t.handle = h;
        t.down = None;
        for p in &mut t.self_pilots {
            *p = super::self_turrets::SelfPilot::new(p.seat);
        }
        self.file_posed(i);
    }

    /// Give the building turret on `instance` its posed prims ([`PosedParts`]):
    /// they become movers of the collision world, each filed under the
    /// box a turn of its node about the building can reach, and are placed
    /// from the gun at the next tick.
    pub fn set_turret_posed(&mut self, instance: u32, posed: PosedParts) {
        let Some(i) = self
            .building_turrets
            .iter()
            .position(|t| t.instance == instance)
        else {
            return;
        };
        self.building_turrets[i].posed = Some(posed);
        self.file_posed(i);
    }

    fn file_posed(&mut self, i: usize) {
        let t = &mut self.building_turrets[i];
        let origin = t.spec.pos;
        let Some(p) = t.posed.as_mut() else {
            return;
        };
        p.at = None;
        let bodies = self.collision.bodies();
        let movers: Vec<(usize, Aabb)> = p
            .bodies
            .iter()
            .filter_map(|b| {
                let a = bodies.get(b.body)?.aabb;
                // Turned about its node (at most `pivot` from the origin),
                // a point stays inside `|p| + 2 |pivot|` of it.
                let pivot = p.rig.binds.get(b.node)?.w_axis.truncate().length();
                let r = (a.min - origin).abs().max((a.max - origin).abs()).length()
                    + 2.0 * pivot
                    + 0.01;
                Some((
                    b.body,
                    Aabb {
                        min: origin - Vec3::splat(r),
                        max: origin + Vec3::splat(r),
                    },
                ))
            })
            .collect();
        self.collision.add_movers(&movers);
    }

    /// Step 2b': every standing building turret's posed prims to its gun's
    /// pose (card TCOL, [`PosedParts`]).
    pub(super) fn pose_building_turrets(&mut self) {
        for i in 0..self.building_turrets.len() {
            let t = &self.building_turrets[i];
            let Some(p) = t.posed.as_ref().filter(|_| t.down.is_none()) else {
                continue;
            };
            let Some(g) = self
                .vehicle(t.handle)
                .and_then(|v| v.guns.get(p.seat))
                .and_then(Option::as_ref)
            else {
                continue;
            };
            let (look, aim) = (g.look, g.aim);
            let key = [look.0, look.1, aim.0, aim.1].map(f32::to_bits);
            if p.at == Some(key) {
                continue;
            }
            let pose = t.pose();
            let now = p.rig.pose(look, aim, &[]);
            let moves: Vec<(usize, Mat4)> = p
                .bodies
                .iter()
                .map(|b| {
                    let m = now[b.node] * p.rig.binds[b.node].inverse();
                    (b.body, pose * m * b.rest)
                })
                .collect();
            for (body, to) in moves {
                self.collision.move_body(body, to);
            }
            if let Some(p) = self.building_turrets[i].posed.as_mut() {
                p.at = Some(key);
            }
        }
    }
}

#[cfg(test)]
mod tests;
