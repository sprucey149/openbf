//! `[ours]` Players' names over their heads (LAN §5b, issue #168): in a
//! multiplayer round (and later splitscreen, one [`NameTags`] per view)
//! every OTHER human player's session name floats above his head. Bots and
//! the viewer himself get none.
//!
//! Rules:
//! - A name shows only while the viewer's camera has a direct line of
//!   sight to the head: the AI's own sight test (`core::perception::los_seat`:
//!   the terrain, then the world's ORDNANCE bodies and vehicle hulls, past
//!   the viewer's and the target's own vehicles and turret buildings), never
//!   through walls or terrain.
//! - The sight ray is cast at most every [`CHECK_EVERY`] seconds per player
//!   (staggered by slot), and never past [`GONE_AT`]; between checks the last
//!   answer holds.
//! - It fades with distance: full to [`FULL_TO`] m, gone by [`GONE_AT`] m.
//! - Presentation only: the sim is read, never written, and nothing here
//!   enters `Sim::hash`.

use core::fmath::FloatExt;
use core::sim::Sim;
use core::unit::{Controller, UnitHandle};
use glam::{Mat4, Vec3};
use std::collections::BTreeMap;

/// Seconds between one player's sight checks.
pub const CHECK_EVERY: f32 = 0.25;
/// Full opacity up to this distance (m).
pub const FULL_TO: f32 = 40.0;
/// Gone from this distance (m).
pub const GONE_AT: f32 = 80.0;
/// The name's anchor above the top of the soldier's capsule (m).
pub const ABOVE_HEAD: f32 = 0.45;

/// One name to draw this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct NameTag {
    pub slot: u8,
    pub name: String,
    /// The anchor point in the world, above the head.
    pub at: Vec3,
    /// 0..1 from the distance fade.
    pub alpha: f32,
    /// On the viewer's side (the HUD's friend colour; else enemy).
    pub friend: bool,
}

/// The distance fade: 1 to [`FULL_TO`], 0 from [`GONE_AT`], linear between.
pub fn fade(distance: f32) -> f32 {
    ((GONE_AT - distance) / (GONE_AT - FULL_TO)).clamp(0.0, 1.0)
}

/// The human players `viewer` may see a name of: alive, driven by a
/// player slot, not the viewer's own unit or slot. Bots never.
pub fn candidates(sim: &Sim, viewer: UnitHandle) -> Vec<(u8, UnitHandle)> {
    let own_slot = sim
        .unit(viewer)
        .and_then(|u| u.spec.player_slot)
        .or_else(|| sim.units.get(viewer.index as usize)?.spec.player_slot);
    sim.alive_units()
        .filter(|(h, _)| h.index != viewer.index)
        .filter_map(|(h, u)| match u.controller {
            Controller::Player { slot } if Some(slot) != own_slot => Some((slot, h)),
            _ => None,
        })
        .collect()
}

/// The point a name hangs over: above the top of `h`'s capsule.
pub fn head(sim: &Sim, h: UnitHandle) -> Option<Vec3> {
    let c = sim.unit(h)?.capsule();
    Some(c.base + Vec3::Y * (c.height + ABOVE_HEAD))
}

/// Whether the camera at `eye` (the viewer `viewer`'s) sees `target`'s head
/// `at`: the AI's sight test (module rules).
pub fn sees(sim: &Sim, eye: Vec3, viewer: UnitHandle, target: UnitHandle, at: Vec3) -> bool {
    use core::perception::{los_seat, past_buildings, past_mounts};
    los_seat(
        sim,
        eye,
        at,
        past_mounts(sim, viewer, target),
        past_buildings(sim, viewer, target),
    )
}

/// One view's names: the last sight answer per slot and when to ask again.
#[derive(Clone, Debug, Default)]
pub struct NameTags {
    /// Slot → (seconds to the next check, last answer).
    checks: BTreeMap<u8, (f32, bool)>,
}

impl NameTags {
    /// This frame's names for the camera at `eye` of `viewer`, `dt` seconds
    /// after the last call; `name` gives a slot's session name (`None`
    /// draws none).
    pub fn update(
        &mut self,
        sim: &Sim,
        viewer: UnitHandle,
        eye: Vec3,
        dt: f32,
        name: &dyn Fn(u8) -> Option<String>,
    ) -> Vec<NameTag> {
        let viewer_team = sim.unit(viewer).map(|u| u.team);
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for (slot, h) in candidates(sim, viewer) {
            seen.push(slot);
            let Some(at) = head(sim, h) else { continue };
            let alpha = fade(at.distance(eye));
            let entry = self
                .checks
                .entry(slot)
                // Staggered: each slot's first check its own share of the period.
                .or_insert((CHECK_EVERY * f32::from(slot % 8) / 8.0, false));
            if alpha <= 0.0 {
                *entry = (0.0, false);
                continue;
            }
            entry.0 -= dt;
            if entry.0 <= 0.0 {
                entry.1 = sees(sim, eye, viewer, h, at);
                entry.0 += CHECK_EVERY;
                entry.0 = entry.0.fmax(0.0);
            }
            if !entry.1 {
                continue;
            }
            let Some(name) = name(slot) else { continue };
            let team = sim.units[h.index as usize].team;
            out.push(NameTag {
                slot,
                name,
                at,
                alpha,
                friend: viewer_team.is_some_and(|t| sim.allied(t, team)),
            });
        }
        self.checks.retain(|s, _| seen.contains(s));
        out
    }

    /// Forget every answer (a new round).
    pub fn clear(&mut self) {
        self.checks.clear();
    }
}

/// `at` on a screen of `size` (any unit, y down) through `view_proj`, or
/// `None` behind the camera or off the screen.
pub fn to_screen(view_proj: Mat4, at: Vec3, size: [f32; 2]) -> Option<[f32; 2]> {
    let c = view_proj * at.extend(1.0);
    if c.w <= 1e-3 {
        return None;
    }
    let ndc = c.truncate() / c.w;
    if ndc.x.abs() > 1.05 || ndc.y.abs() > 1.05 {
        return None;
    }
    Some([(ndc.x + 1.0) * 0.5 * size[0], (1.0 - ndc.y) * 0.5 * size[1]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walk::WalkSim;
    use core::unit::UnitSpec;

    fn walk_with_humans() -> (WalkSim, UnitHandle, UnitHandle) {
        let mut w = WalkSim::new(7, Vec3::ZERO, None);
        let me = w.player;
        let at = w.sim.unit(me).unwrap().soldier.pos;
        let friend = w
            .sim
            .spawn(&UnitSpec {
                controller: Controller::Player { slot: 1 },
                player_slot: Some(1),
                ..UnitSpec::trooper(w.sim.unit(me).unwrap().team, at + Vec3::new(20.0, 0.0, 0.0))
            })
            .expect("a second human");
        w.sim
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(1, at + Vec3::new(0.0, 0.0, 20.0))
            })
            .expect("a bot");
        (w, me, friend)
    }

    fn eye(w: &WalkSim) -> Vec3 {
        let me = w.sim.unit(w.player).unwrap();
        me.soldier.pos + Vec3::Y * 1.7
    }

    /// The distance fade's ends and middle.
    #[test]
    fn name_tags_fade_with_distance() {
        assert_eq!(fade(10.0), 1.0);
        assert_eq!(fade(FULL_TO), 1.0);
        assert!((fade(60.0) - 0.5).abs() < 1e-6);
        assert_eq!(fade(GONE_AT), 0.0);
        assert_eq!(fade(200.0), 0.0);
    }

    /// Only the other human: never a bot, never the viewer.
    #[test]
    fn name_tags_skip_bots_and_the_viewer() {
        let (w, me, friend) = walk_with_humans();
        let c = candidates(&w.sim, me);
        assert_eq!(c, vec![(1, friend)]);
        let mut tags = NameTags::default();
        let name = |s: u8| Some(format!("playtest_{s}"));
        let mut shown = Vec::new();
        for _ in 0..30 {
            shown = tags.update(&w.sim, me, eye(&w), 1.0 / 60.0, &name);
        }
        assert_eq!(shown.len(), 1);
        assert_eq!((shown[0].slot, shown[0].name.as_str()), (1, "playtest_1"));
        assert!(shown[0].friend && shown[0].alpha == 1.0);
    }

    /// A wall between the camera and the head hides the name at the next
    /// check; clear again, it comes back.
    #[test]
    fn name_tags_need_a_line_of_sight() {
        let (mut w, me, friend) = walk_with_humans();
        let e = eye(&w);
        let at = head(&w.sim, friend).unwrap();
        assert!(sees(&w.sim, e, me, friend, at));
        let mut tags = NameTags::default();
        let name = |s: u8| Some(format!("P{s}"));
        let step = |tags: &mut NameTags, w: &WalkSim| {
            (0..20).fold(Vec::new(), |_, _| {
                tags.update(&w.sim, me, e, 1.0 / 60.0, &name)
            })
        };
        assert_eq!(step(&mut tags, &w).len(), 1);
        // A wall halfway, across the sight line.
        let mid = (e + at) * 0.5;
        w.sim
            .collision
            .push_body(core::collision::Body::with_layers(
                Mat4::from_translation(mid),
                core::collision::Shape::Box {
                    half: Vec3::new(0.5, 10.0, 10.0),
                },
                core::collision::Layers::ALL,
            ));
        assert!(!sees(&w.sim, e, me, friend, at));
        assert!(step(&mut tags, &w).is_empty(), "hidden behind the wall");
    }

    /// The projection: ahead lands on the screen, behind does not.
    #[test]
    fn name_tags_project_to_the_screen() {
        let cam = render::Camera {
            position: Vec3::ZERO,
            ..render::Camera::default()
        };
        let vp = cam.view_proj(16.0 / 9.0);
        let p = to_screen(vp, Vec3::new(0.0, 0.0, -10.0), [1600.0, 900.0]).unwrap();
        assert!((p[0] - 800.0).abs() < 1.0 && (p[1] - 450.0).abs() < 1.0);
        assert!(to_screen(vp, Vec3::new(0.0, 0.0, 10.0), [1600.0, 900.0]).is_none());
    }
}
