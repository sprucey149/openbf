//! Human zoom state and aim rays (`Sim::aim_hit`, `Sim::human_shot`, `Sim::tick_zoom`).

use super::super::input::{buttons, PadState};
use super::super::state::Sim;
use crate::fmath::FloatExt;
use crate::projectile::TERRAIN_RAY_STEP;
use crate::soldier::Stance;
use crate::unit::Controller;
use glam::Vec3;

/// The aim ray starts this far past the camera's distance behind the eye
/// (`re-aim-assist.md` §10.2 step 5: `lui 0x4000`), and is this long (`lui
/// 0x4270`): what is nearer than the start is not seen at all (the PS2's
/// blind spot, FEEL_NOTES AIMA).
pub const AIM_RAY_START: f32 = 2.0;
pub const AIM_RAY_LENGTH: f32 = 60.0;
/// In the scope view the aim's far point is this far along it, with no ray.
pub const SCOPE_AIM_LENGTH: f32 = 200.0;
/// OURS (playtest 2026-11-05, B6): when the aim ray meets nothing within
/// its reach and the reticle picks nothing, a second ray goes on from its
/// far end this much further through the terrain and the ordnance layer,
/// so a shot at the ground or a wall out to 300 m converges on the
/// reticle's point instead of the ray's far end. Past a launcher's
/// MaxRange (150 m), close to a torpedo's 320 m flight. Ground maps only:
/// a foot shot in a space hangar keeps the PS2's far end. replace: a
/// maintainer playtest value if 300 m reads as too far or too near.
pub const FAR_AIM_LENGTH: f32 = 240.0;

impl Sim {
    /// The views drawn: one per player slot (split screen tilts the camera
    /// less, `unit::SPLIT_TILT`).
    pub fn views(&self) -> usize {
        self.players.len().max(1)
    }

    /// Nearest terrain or ordnance-layer hit along `origin + d * t` up to
    /// `limit`, returning `Some(t)` when hit (a ray starting underground
    /// skips the terrain probe so it does not hit on the way back out).
    fn world_ray_hit(&self, i: usize, origin: Vec3, d: Vec3, limit: f32) -> Option<f32> {
        let mut best = limit;
        let mut hit = false;
        let under = self.heightfield.as_ref().is_some_and(|h| {
            origin.y < h.height_at(origin.x, origin.z)
                && !crate::collision::is_cut_by(&self.collision.cutters, origin)
        });
        if !under {
            if let Some(t) = self.terrain_ray(origin, d, best, TERRAIN_RAY_STEP) {
                best = best.fmin(t);
                hit = true;
            }
        }
        let own = crate::collision::HullFilter::except(self.units[i].mount.map(|m| m.vehicle));
        let layer = crate::collision::Layers::ORDNANCE;
        if let Some(h) = self.collision.ray_with(origin, d, best, layer, own) {
            best = best.fmin(h.t);
            hit = true;
        }
        hit.then_some(best)
    }

    /// H of §10.2 step 5 for unit `i`: the first ordnance-solid thing on its
    /// aim ray between [`AIM_RAY_START`] past the track offset and
    /// [`AIM_RAY_LENGTH`] further - another unit's capsule, a vehicle's hull,
    /// the map's ordnance layer, the terrain - else the ray's far end.
    pub fn aim_hit(&self, i: usize) -> Vec3 {
        let u = &self.units[i];
        let (a, d) = u.aim_ray(self.ground_rules, self.views());
        // The scope view casts no ray (§10.2 step 7): the point 200 m along
        // the aim.
        if u.scope_path() {
            return a + d * SCOPE_AIM_LENGTH;
        }
        let sec = u.section();
        let start = a + d * (sec.offset.z + AIM_RAY_START);
        let mut best = self
            .world_ray_hit(i, start, d, AIM_RAY_LENGTH)
            .unwrap_or(AIM_RAY_LENGTH);
        let me = self.handle(i);
        // A human sees the others `HUMAN_SHOT_LAG` behind (OURS, as his
        // bolt's sweep meets them).
        let lag = if matches!(u.controller, Controller::Player { .. }) {
            crate::unit::HUMAN_SHOT_LAG
        } else {
            0.0
        };
        for (h, o) in self.alive_units() {
            if h != me {
                let mut cap = o.capsule();
                cap.base -= o.soldier.vel * lag;
                if let Some(t) = cap.ray_hit(start, d, best) {
                    best = best.fmin(t);
                }
            }
        }
        // An auto-turret or remote droid is a body on the PS2's aim ray as on
        // a bolt's flight (its `p_-so_sphere`, mask soldier + ordnance): the
        // shot flies to the one the reticle is on. Its owner's own bolts pass
        // it, so his reticle's ray does too.
        let items = self
            .turrets
            .iter()
            .filter(|t| t.owner.index != me.index)
            .map(|t| (t.centre(), crate::autoturret::RADIUS))
            .chain(
                self.droids
                    .iter()
                    .filter(|o| o.owner.index != me.index)
                    .map(|o| (o.pos, crate::remote::RADIUS)),
            );
        for (at, radius) in items {
            if let Some(t) = crate::unit::ray_sphere(start, d, at, radius).filter(|&t| t < best) {
                best = t;
            }
        }
        if best >= AIM_RAY_LENGTH {
            if let Some(t) = self.beyond_reach(i, start, d) {
                return start + d * t;
            }
        }
        start + d * best
    }

    /// OURS (playtest 2026-10-28, BF1 Yavin 4, "far away shooting"): when
    /// the reticle's ray meets nothing within its reach and what the
    /// reticle picks (`Sim::reticle_pick`: the cone's pick, a unit or a
    /// vehicle) is past that reach, the shot is aimed at the point of the
    /// ray abreast of the pick, not at the ray's far end. The PS2 aims at
    /// the far end 65.5 m out (§10.5 item 2): the fire point is 0.51 m
    /// (BF1) / 0.77 m (BF2) under the ray, so the bolt crosses the ray there
    /// and climbs over it - 0.35 m at 100 m, 0.65 m at 150 m: a shot at the
    /// chest of a soldier past 110 m flies over his head. Two lines of 480
    /// on the PS2's screen; on today's it is most of a soldier. Within the
    /// reach the PS2's rule stands.
    ///
    /// Without such a pick (playtest 2026-11-05, B6: a rocket at the beach
    /// from Kashyyyk's docks), the ray goes on [`FAR_AIM_LENGTH`] further
    /// through the terrain and the ordnance layer, and the shot converges
    /// on the ground, wall or vehicle hull it meets. Soldiers, auto-turrets
    /// and droids past the reach that the reticle does not pick stay unseen
    /// (the PS2's rule). `None` when the far ray meets nothing either, and
    /// always on a space map (a foot shot in a hangar is the PS2's): the
    /// far end.
    fn beyond_reach(&self, i: usize, start: Vec3, d: Vec3) -> Option<f32> {
        let me = self.handle(i);
        let slot = self.players.iter().position(|p| *p == Some(me))?;
        let pick = self.reticle_pick.get(slot).copied().flatten();
        if let Some(at) = pick.and_then(|(p, _)| self.aim_candidate(p)).map(|c| c.ctr) {
            let t = (at - start).dot(d);
            if t > AIM_RAY_LENGTH {
                return Some(t);
            }
        }
        // Space Assault and space CTF: the far ray is a ground-map rule.
        if self.space_round() {
            return None;
        }
        let far = start + d * AIM_RAY_LENGTH;
        self.world_ray_hit(i, far, d, FAR_AIM_LENGTH)
            .map(|t| AIM_RAY_LENGTH + t)
    }

    /// The soldier's zoom routine (B1 `FUN_00204578`, B2 `FUN_0023ead8`,
    /// `analysis/re-zoom.md` §4), run before unit `i` moves, so it sees the
    /// stance and the weapon as the last tick left them. Only a human's
    /// soldier zooms. The zoom ends in a roll or a sprint (BF2's rule; BF1
    /// has no sprint and the port's takes it, FEEL_NOTES), when a blast
    /// throws him (the PS2's thrown state; the port also holds it off
    /// through the tumble, the get-up and its lock, FEEL_NOTES), on a weapon that
    /// cannot zoom - the secondary's grenade once it is the weapon in hand -
    /// and while the primary reloads; otherwise a press of ZOOM steps it
    /// (`WeaponClass::zoom_press`). Death, a seat and the primary's switch
    /// end it where they happen. Not ported: the continuous zoom of a class
    /// with `ZoomRate` (no hand weapon), BF2's melee test (no disc lightsaber
    /// has a `ZoomMax` over 1).
    pub(super) fn tick_zoom(&mut self, i: usize, pad: &PadState) {
        let u = &self.units[i];
        let human = matches!(u.controller, Controller::Player { .. });
        let press = pad.pressed(buttons::ZOOM) && u.soldier.prev_buttons() & buttons::ZOOM == 0;
        let cancel =
            matches!(u.soldier.stance, Stance::Roll | Stance::Sprint) || u.soldier.no_fire();
        let (class, reloading) = match u.cur1.filter(|_| u.active_channel == 1) {
            Some(k) => (u.held[usize::from(k)].class, false),
            None => (u.weapon_class, u.weapon.reloading()),
        };
        let wc = &self.weapon_classes[class];
        let step = u.zoom.map(|z| z.step);
        let step = if !human || cancel || !wc.zooms() || reloading {
            None
        } else if press {
            wc.zoom_press(step)
        } else {
            step
        };
        let zoom = step.map(|step| {
            let (value, divisor) = wc.zoom_at(step);
            crate::unit::Zoom {
                step,
                value,
                divisor,
                scope: wc.zoom_first_person,
            }
        });
        self.units[i].zoom = zoom;
    }

    pub(super) fn put_aim(&mut self, i: usize, (yaw, pitch): (f32, f32)) {
        let s = &mut self.units[i].soldier;
        (s.yaw, s.pitch) = (yaw, pitch);
    }

    /// A human's shot (§10.2 step 6): from the fire point F along
    /// normalize(H - F), with the spread's frame built on that line.
    pub(in crate::sim) fn human_shot(&self, i: usize) -> (Vec3, (Vec3, Vec3, Vec3)) {
        let u = &self.units[i];
        // The scope view: from the scope point along the aim itself.
        if u.scope_path() {
            return (u.scope_point(self.ground_rules), u.aim_frame());
        }
        let f = u.droideka_muzzle().unwrap_or_else(|| u.fire_point());
        let (aim, aim_right, _) = u.aim_frame();
        let fwd = (self.aim_hit(i) - f).normalize_or(aim);
        let right = fwd.cross(Vec3::Y).normalize_or(aim_right);
        (f, (fwd, right, right.cross(fwd).normalize()))
    }
}
