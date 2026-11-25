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

/// The projection a view's tags are placed with (issue #194 B8): that view's camera at its cell's
/// aspect, the one its world is drawn at (`render::Renderer::render_view`).
pub fn cell_view_proj(camera: &render::Camera, cell: crate::split_layout::Cell) -> Mat4 {
    camera.view_proj(cell[2] as f32 / cell[3].max(1) as f32)
}

/// A cell in window pixels as an egui rectangle in points.
pub fn cell_rect(ctx: &ui::egui::Context, cell: crate::split_layout::Cell) -> ui::egui::Rect {
    let ppp = ctx.pixels_per_point().max(0.1);
    let [x, y, w, h] = cell.map(|v| v as f32 / ppp);
    ui::egui::Rect::from_min_size(ui::egui::pos2(x, y), ui::egui::vec2(w, h))
}

/// Where and how big a split view's egui text (tags, round timer, banner) draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellText {
    /// The cell in points.
    pub rect: ui::egui::Rect,
    /// The text's scale: the cell's HUD box over the window's ([`cell_text_scale`]).
    pub scale: f32,
    /// The round timer's top centre, in points from the cell's top centre ([`cell_timer_at`]).
    pub timer_at: [f32; 2],
}

/// View `cell`'s [`CellText`] in `ctx`'s window, with the HUD size setting `hud_k`.
pub fn cell_text(ctx: &ui::egui::Context, cell: crate::split_layout::Cell, hud_k: f32) -> CellText {
    let ppp = ctx.pixels_per_point().max(0.1);
    let window_h = ctx.content_rect().height() * ppp;
    CellText {
        rect: cell_rect(ctx, cell),
        scale: cell_text_scale(cell, window_h, hud_k),
        timer_at: cell_timer_at(cell, hud_k).map(|v| v / ppp),
    }
}

/// How much smaller a split cell draws its egui text: the cell's HUD box height
/// ([`crate::split_ui::cell_frame`], the HUD size setting `hud_k` included) over the single
/// view's (the window's height, `window_h` pixels), so the text shrinks with the HUD drawn
/// beside it. 1 at most.
pub fn cell_text_scale(cell: crate::split_layout::Cell, window_h: f32, hud_k: f32) -> f32 {
    let box_h = crate::split_ui::cell_frame(cell, hud_k).box_h;
    (box_h / window_h.max(1.0)).clamp(0.1, 1.0)
}

/// The round timer's top in a split cell, in pixels below the cell's top: in that cell's
/// reinforcement counts row (`SPLITSCREEN.md` 18.1 item 6), [`ui::hud::SPLIT_TIMER_TOP`] of the
/// cell's HUD box, whose top-anchored roots hang from the cell's top edge.
pub fn cell_timer_top(cell: crate::split_layout::Cell, hud_k: f32) -> f32 {
    crate::split_ui::cell_frame(cell, hud_k).box_h * ui::hud::SPLIT_TIMER_TOP
}

/// The round timer's top centre in a split cell, in pixels from the cell's top centre: down
/// [`cell_timer_top`], and left of the counts ([`ui::hud::SPLIT_COUNTS_HALF`],
/// [`ui::hud::SPLIT_TIMER_GAP`]), clear of the target bar between them, the reticle under them
/// and the minimap in the top-right corner, in every cell shape.
pub fn cell_timer_at(cell: crate::split_layout::Cell, hud_k: f32) -> [f32; 2] {
    let box_h = crate::split_ui::cell_frame(cell, hud_k).box_h;
    let half_w = 0.5 * ui::hud::SPLIT_TIMER_SIZE[0] * box_h;
    let right = (ui::hud::SPLIT_COUNTS_HALF + ui::hud::SPLIT_TIMER_GAP) * box_h;
    [-(right + half_w), cell_timer_top(cell, hud_k)]
}

/// The round timer's box at most in a split cell, in pixels from the cell's top-left: left, top,
/// right, bottom ([`cell_timer_at`], [`ui::hud::SPLIT_TIMER_SIZE`]).
pub fn cell_timer_box(cell: crate::split_layout::Cell, hud_k: f32) -> [f32; 4] {
    let box_h = crate::split_ui::cell_frame(cell, hud_k).box_h;
    let [w, h] = ui::hud::SPLIT_TIMER_SIZE.map(|s| s * box_h);
    let [dx, top] = cell_timer_at(cell, hud_k);
    let x = 0.5 * cell[2] as f32 + dx;
    [x - 0.5 * w, top, x + 0.5 * w, top + h]
}

/// A view's tags as its HUD pass draws them (issue #194 B22, [`ui::hud::NameTagDraw`]): each
/// anchor placed through `view_proj` (that view's camera at its screen's aspect) on a screen of
/// `size` pixels (a split cell's own, from its top-left; the window's for a single view). A tag
/// behind the camera or off the screen is left out.
pub fn placed(tags: &[NameTag], view_proj: Mat4, size: [f32; 2]) -> Vec<ui::hud::NameTagDraw> {
    tags.iter()
        .filter_map(|t| {
            Some(ui::hud::NameTagDraw {
                at: to_screen(view_proj, t.at, size)?,
                name: t.name.clone(),
                friend: t.friend,
                alpha: t.alpha,
            })
        })
        .collect()
}

/// One split view's tags this frame, as the window hands them to that view's HUD model (issue
/// #194 B8, B20): the names `viewer` sees from `camera` ([`NameTags::update`], `tags` that view's
/// own answers), placed in its `cell` ([`cell_view_proj`], [`placed`]). The same in an offline
/// split round and a LAN one: every other human of the round, this machine's further players
/// and the remote ones alike, named by `name` (the round's one name table).
pub fn cell_tags(
    tags: &mut NameTags,
    sim: &Sim,
    viewer: UnitHandle,
    camera: &render::Camera,
    cell: crate::split_layout::Cell,
    dt: f32,
    name: &dyn Fn(u8) -> Option<String>,
) -> Vec<ui::hud::NameTagDraw> {
    let seen = tags.update(sim, viewer, camera.position, dt, name);
    placed(
        &seen,
        cell_view_proj(camera, cell),
        [cell[2] as f32, cell[3] as f32],
    )
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
        let name = |s: u8| Some(format!("Rex_{s}"));
        let mut shown = Vec::new();
        for _ in 0..30 {
            shown = tags.update(&w.sim, me, eye(&w), 1.0 / 60.0, &name);
        }
        assert_eq!(shown.len(), 1);
        assert_eq!((shown[0].slot, shown[0].name.as_str()), (1, "Rex_1"));
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

    /// The cell's projection: a point ahead lands in the cell's middle at the cell's aspect, and
    /// a point at the edge of a 4:3-wide view of a tall cell stays on it (`fit_fov_y` widened the
    /// camera, `cell_view_proj` uses the same aspect the world is drawn at).
    #[test]
    fn to_screen_with_the_cell_aspect() {
        let cell = [642, 0, 638, 720];
        let cam = render::Camera {
            position: Vec3::ZERO,
            fov_y: crate::split_layout::fit_fov_y(render::Camera::default().fov_y, 638.0 / 720.0),
            ..render::Camera::default()
        };
        let vp = cell_view_proj(&cam, cell);
        let size = [cell[2] as f32, cell[3] as f32];
        let p = to_screen(vp, Vec3::new(0.0, 0.0, -10.0), size).unwrap();
        assert!(
            (p[0] - 319.0).abs() < 0.5 && (p[1] - 360.0).abs() < 0.5,
            "{p:?}"
        );
        // The cell's aspect, not the window's: the same point placed with a 16:9 projection
        // would land elsewhere in the cell.
        let side = Vec3::new(3.0, 0.0, -10.0);
        let at_cell = to_screen(vp, side, size).unwrap();
        let at_wide = to_screen(cam.view_proj(16.0 / 9.0), side, size).unwrap();
        assert!(
            (at_cell[0] - at_wide[0]).abs() > 20.0,
            "{at_cell:?} {at_wide:?}"
        );
        // On the unwidened camera, half a 4:3 width at 10 m is on the cell's edge.
        let half_w = 10.0 * (render::Camera::default().fov_y * 0.5).tan() * 4.0 / 3.0;
        let edge = to_screen(vp, Vec3::new(half_w, 0.0, -10.0), size).unwrap();
        assert!((edge[0] - size[0]).abs() < 1.0, "{edge:?}");
    }

    fn tag(name: &str, at: Vec3, friend: bool) -> NameTag {
        NameTag {
            slot: 1,
            name: name.into(),
            at,
            alpha: 0.75,
            friend,
        }
    }

    /// A cell's text follows its HUD box (`Frame::cell`, the HUD size setting included), and its
    /// timer hangs the same share of that box down.
    #[test]
    fn cell_text_follows_the_cells_hud_box() {
        let side = [960, 0, 960, 1080];
        assert!((cell_text_scale(side, 1080.0, 1.0) - 720.0 / 1080.0).abs() < 1e-4);
        assert!((cell_text_scale(side, 1080.0, 0.7) - 0.7 * 720.0 / 1080.0).abs() < 1e-4);
        assert_eq!(cell_text_scale([0, 0, 1920, 1080], 1080.0, 1.0), 1.0);
        let quad = [0, 540, 960, 540];
        assert!((cell_text_scale(quad, 1080.0, 1.0) - 0.5).abs() < 1e-4);
        assert!((cell_timer_top(quad, 1.0) - 540.0 * ui::hud::SPLIT_TIMER_TOP).abs() < 1e-3);
        assert!(cell_timer_top(quad, 0.7) < cell_timer_top(quad, 1.0));
        // Every cell puts the timer left of the counts, in their row, and inside the cell.
        for cell in [[0, 0, 1920, 540], side, quad] {
            let b = cell_timer_box(cell, 1.0);
            let centre = 0.5 * cell[2] as f32;
            let box_h = crate::split_ui::cell_frame(cell, 1.0).box_h;
            assert!(b[2] <= centre - ui::hud::SPLIT_COUNTS_HALF * box_h, "{b:?}");
            assert!(b[0] >= 0.0 && b[3] < 0.2 * box_h, "{b:?}");
        }
    }

    /// The single view's tags land where the window's camera puts them; one behind it is left
    /// out (issue #194 B22: `placed` replaces the egui draw).
    #[test]
    fn placed_puts_each_tag_on_its_screen() {
        let cam = render::Camera::default();
        for size in [[1280.0, 720.0], [640.0, 480.0], [1280.0, 1024.0]] {
            let vp = cam.view_proj(size[0] / size[1]);
            let tags = [
                tag("Rex", Vec3::new(1.0, 0.5, -12.0), true),
                tag("Echo", Vec3::new(-3.0, 0.0, -30.0), false),
                tag("Behind", Vec3::new(0.0, 0.0, 10.0), true),
            ];
            let drawn = placed(&tags, vp, size);
            assert_eq!(drawn.len(), 2, "{size:?}");
            for (d, t) in drawn.iter().zip(&tags) {
                assert_eq!(Some(d.at), to_screen(vp, t.at, size));
                assert_eq!(
                    (d.name.as_str(), d.friend, d.alpha),
                    (t.name.as_str(), t.friend, t.alpha)
                );
            }
        }
    }

    /// Issue #194 B8 (the split check): with two local players side by side, P2 looking at P1,
    /// every view's tags are planned from that view's own camera (`view_camera`) and drawn into
    /// its own cell: P1's name lands inside P2's cell and nowhere in P1's.
    #[test]
    fn p1s_tag_lands_in_p2s_cell_and_nowhere_in_p1s() {
        use crate::split_layout::{cells, TwoPlayer};
        let mut w = WalkSim::new(7, Vec3::ZERO, None);
        let p1 = w.player;
        let team = w.sim.unit(p1).unwrap().team;
        let p2 = w.add_local_player(1, Some(team)).expect("slot 1 is free");
        let base = w.sim.unit(p1).unwrap().soldier.pos;
        w.sim.units[p2.index as usize].alive = true;
        w.sim.spawn_stage[1] = core::sim::SpawnStage::Alive;
        w.sim.units[p2.index as usize].soldier.pos = base + Vec3::new(30.0, 0.0, 0.0);
        let cells = cells(2, 1280, 720, TwoPlayer::SideBySide);
        let look = core::sim::PadState::default();
        // P1 stands 15 m in front of P2's camera.
        let cam1 = crate::render_blend::view_camera(&mut w, 1, cells[1], &look, None);
        let ahead = cam1.forward() * Vec3::new(1.0, 0.0, 1.0);
        let at = cam1.position + ahead.normalize() * 15.0;
        w.sim.units[p1.index as usize].soldier.pos = Vec3::new(at.x, base.y, at.z);
        let cams: Vec<render::Camera> = (0..2)
            .map(|i| crate::render_blend::view_camera(&mut w, i, cells[i], &look, None))
            .collect();
        assert_eq!(
            format!("{:?}", cams[1]),
            format!(
                "{:?}",
                crate::render_blend::view_camera(&mut w, 1, cells[1], &look, None)
            ),
            "a view camera leaves the walk as it found it"
        );
        let names = |s: u8| Some(["Luke", "Han"][usize::from(s)].to_owned());
        let mut tags = [NameTags::default(), NameTags::default()];
        let mut planned = Vec::new();
        for _ in 0..30 {
            planned = (0..2)
                .map(|i| {
                    let shown = tags[i].update(
                        &w.sim,
                        w.local_unit(i),
                        cams[i].position,
                        1.0 / 60.0,
                        &names,
                    );
                    (cells[i], shown, cell_view_proj(&cams[i], cells[i]))
                })
                .collect::<Vec<_>>();
        }
        assert!(
            planned[1].1.iter().any(|t| t.name == "Luke"),
            "P2's view tags P1: {:?}",
            planned[1].1
        );
        assert!(
            planned[0].1.iter().all(|t| t.name != "Luke"),
            "nobody tags himself"
        );
        // Each view's tags in its own cell's pixels: P1's lands inside P2's cell.
        for (i, (cell, shown, vp)) in planned.iter().enumerate() {
            let drawn = placed(shown, *vp, [cell[2] as f32, cell[3] as f32]);
            let luke: Vec<_> = drawn.iter().filter(|t| t.name == "Luke").collect();
            assert_eq!(luke.len(), usize::from(i == 1), "view {i}: {drawn:?}");
            for t in luke {
                assert!(t.at[0] >= 0.0 && t.at[0] <= cell[2] as f32, "{t:?}");
                assert!(t.at[1] >= 0.0 && t.at[1] <= cell[3] as f32, "{t:?}");
            }
        }
    }
}
