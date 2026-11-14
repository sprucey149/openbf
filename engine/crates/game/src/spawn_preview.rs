//! The spawn screens' 3-D soldiers (card SSELB slice 3,
//! `analysis/re-spawn-flow.md` §2): on the side select the HIGHLIGHTED
//! team's first class stands in the camera shot (none on Auto Assign;
//! `B2:393254-393270`, the soldier `DAT_00825ea0` built by `FUN_00395a68`).
//!
//! The PS2 draws him as a UI model at a screen place (`FUN_005a2e20(-100,
//! 150)` in the 512 x 448 screen, one viewport; B2:391941) with the
//! colour word 0xffa0a0a0 (`B2:391928`). The port stands him in the world
//! before the camera, placed so that he covers the PS2's pixels [M]
//! (`lead_tmp/sselb/tools/soldier_box.py`: the Hoth frame
//! `lead_tmp/ssel/t1_hot1/s0_team1.png` against `s2_auto.png`), shrunk
//! about the eye by [`SHRINK`] (a uniform scale about the eye leaves the
//! picture as it is and brings him within a metre and a half, nearer than
//! any map piece of a camera shot). Every screen soldier is lit as the
//! PS2 lights a UI model (`analysis/re-spawn-flow.md` §4.2): by the
//! interface light set, not the map's ([`render::LightPath::Iface`]), times
//! his colour word ([`colour_unit`]). A class the PS2 refuses stands black
//! there (the colour word 0, B2:394375-394380; `SpawnPage::Class`'s
//! `refused`, from `Sim::class_refusal`, re-spawn-flow §4.3).
//!
//! He is DIMMED with the backdrop [M]: the PS2's frames hold their soldiers
//! at about half the light they have in play (the Snowtrooper's armour
//! 246 on the field in `s5_spawned`, at most 112 in front of the class
//! panel in `s4_class`, as the dimmed snow is 0.51 of the field's), so the
//! HUD pass's 0.51 veil lies over him too and he is drawn in the 3-D pass.
//!
//! The class select's LINE-UP (slice 4): every class of the team stands on
//! a ring of six places, the picked one in front, and a pick slides the
//! ring by one place in a third of a second (`FUN_0039a980`, the places
//! [`SLOTS`], [`SLIDE_RATE`]). The PS2 sets the places in its UI scene; the
//! port maps that scene into the camera's frame by a perspective fitted to
//! the PS2's Hoth frame ([`lineup_at`]).
//!
//! `[ours]` The PS2's line-up runs behind the class panel at its own 4:3
//! (`s4_class`: the soldiers at places 3 and 4 show dimly through the box,
//! the front one's elbow under its edge). The port slides the whole group
//! clear of the panel and, when even that is too narrow, scales it
//! ([`lineup_fit`]). The panel is centre-anchored and the view is Hor+, so
//! the overlap was the same at every aspect: a wide window only left the
//! room on the left unused.

use crate::animator::Animator;
use crate::soldier_vis::{Outline, SoldierVisuals};
use core::fmath::FloatExt;
use glam::{Mat4, Vec3};
use render::{Camera, DrawCmd, SkinnedDrawCmd};
use ui::spawn_draw::SpawnPage;

/// The side preview's feet in the camera's frame at full size [M]: metres
/// right, up and ahead of the eye (fitted to the PS2's Hoth frame at its
/// tan_v 0.34131: his box 331 px tall, the top at y 97 of 480, the legs'
/// centre near x 190 of 640 px - the PS2's -100 UI units are x 195).
pub const SIDE_AT: Vec3 = Vec3::new(-0.80, -1.054, 4.26);

/// Every screen soldier's turn from facing the camera (rad): the side
/// preview and the line-up alike. Both are built by `FUN_00395898`, which
/// gives a model its place (+0x30), its scale and its draw priority 0xbe
/// (`FUN_005a3368` writes +0x48 and re-sorts the draw list; B2:391745) and
/// no turn, and the side preview's `FUN_005a2e20` (B2:391941) only moves
/// its place [D]. Fitted [M]: the line-up against the PS2's Hoth class
/// frame (slice 4), the side preview against `s0_team1` (silhouette IoU
/// 0.46 at 0.0 against 0.33 at 0.7 and 0.39 / 0.41 at +-0.3;
/// `lead_tmp/sselb/tools/side_iou.py`).
pub const SOLDIER_YAW: f32 = 0.0;

/// The side preview's colour word 0xffa0a0a0 (`B2:391928`) as a factor on
/// his light.
pub const SIDE_TINT: u8 = 0xa0;

/// A colour word's byte as the factor [`render::LightPath::Iface`] puts on
/// the light: byte / 255 [M] (on the Hoth frames byte / 128 left the side
/// Snowtrooper clipped white - p50 121 against the PS2's 61 - under every
/// sign of the light; `lead_tmp/sselb/tools/light_score.py`, `s7/`).
pub fn colour_unit(byte: u8) -> f32 {
    f32::from(byte) / 255.0
}

/// The scale about the eye the screens' soldiers are drawn at (the picture
/// is the same; module doc).
pub const SHRINK: f32 = 0.25;

// He stands within a metre and a half of the eye.
const _: () = assert!(SIDE_AT.z * SHRINK < 1.5);

/// The soldier the side select shows: (team, class), or `None`.
pub fn shown(page: &SpawnPage) -> Option<(u8, u8)> {
    match page {
        SpawnPage::Side { selected, .. } if *selected < 2 => Some((*selected as u8, 0)),
        _ => None,
    }
}

/// Whether `page` stands any 3-D soldier (the side preview or the line-up).
pub fn stands(page: &SpawnPage) -> bool {
    shown(page).is_some() || matches!(page, SpawnPage::Class { classes, .. } if !classes.is_empty())
}

/// The line-up's six places on a unit ring in the PS2's UI scene
/// (`DAT_006a5080`, x and z; B2:394865-394882): place 2 is the front one,
/// the picked class's; class `c + k` stands at place `2 + k`, and place 5,
/// at the back, is where a soldier comes and goes (its colour is 0, 0, 0,
/// 1: unseen).
// 0.7071 is the ELF's word 0x3f350481, not 1 / sqrt 2.
#[allow(clippy::approx_constant)]
pub const SLOTS: [[f32; 2]; 6] = [
    [-0.6659, -0.9588],
    [-0.7071, 0.7071],
    [0.5588, 3.0],
    [0.9659, 0.2588],
    [0.5071, -0.7071],
    [-0.2588, -0.9659],
];

/// Each place's colour word (`DAT_006a50e0`, B2:394881-394917): 0xb4 grey,
/// opaque; place 5 black at alpha 1.
pub const SLOT_COLOURS: [[u8; 4]; 6] = [
    [0xb4, 0xb4, 0xb4, 0xff],
    [0xb4, 0xb4, 0xb4, 0xff],
    [0xb4, 0xb4, 0xb4, 0xff],
    [0xb4, 0xb4, 0xb4, 0xff],
    [0xb4, 0xb4, 0xb4, 0xff],
    [0, 0, 0, 1],
];

/// Below this alpha a soldier is unseen at the back: the port draws its
/// soldiers opaque [H].
const SEEN_ALPHA: f32 = 0.5;

/// A slide's progress per second: `SD+0x3c += dt * 3`, done at 1
/// (B2:394131-394135), so the ring turns one place in a third of a second.
pub const SLIDE_RATE: f32 = 3.0;

/// The UI scene's place of the ring for one viewport: the places times 150
/// (`DAT_005ff828`) plus (100, 350, -452) (`DAT_006a5050`, B2:394886-394893)
/// in the line-up's frame, which stands at (-w / 2, 18 - h / 2)
/// (B2:391575-391581) - read with the 512 x 448 screen [H] - so the ring
/// sits about the screen's centre at x -156, y 144 (down), z -452 (away).
const RING_SCALE: f32 = 150.0;
const RING_AT: Vec3 = Vec3::new(100.0 - 256.0, 350.0 + 18.0 - 224.0, -452.0);

/// The UI scene's perspective as fitted to the PS2's Hoth class frame
/// (`lead_tmp/ssel/t1_hot1/s4_class.png`, the dark trooper at place 1 and
/// the officer at place 0 behind the Snowtrooper) [M]: the eye stands 500
/// units before the screen plane, a unit is 1.25 px across and 1.507 px
/// down of the 640 x 480 frame, and the far vanishing point is at y 201.
/// PROVISIONAL.
const UI_EYE: f32 = 500.0;
const UI_PX: [f32; 2] = [1.25, 1.507];
const UI_VANISH_Y: f32 = 201.0;

/// The PS2 shot's tan_v (re-spawn-flow §1.1): 240 px of the 480 at depth 1.
const TAN_V: f32 = 0.34131;

/// A UI-scene point (screen-centred, y down, z toward the eye) as feet in
/// the camera's frame at full size (right, up, ahead), at the depth the
/// side preview stands at for the screen plane (both soldiers are built at
/// the same scale by `FUN_00395898`) and moved by the side fit's offset
/// from this mapping (the model's origin against its pixels).
fn ui_to_camera(p: Vec3) -> Vec3 {
    let raw = |p: Vec3| {
        let r = UI_EYE / (UI_EYE - p.z);
        let z = SIDE_AT.z / r;
        let f = 240.0 / TAN_V;
        let px = UI_PX[0] * r * p.x;
        let py = UI_VANISH_Y + UI_PX[1] * r * p.y;
        Vec3::new(px * z / f, (240.0 - py) * z / f, z)
    };
    // The side soldier's UI place (-100, 150) (B2:391941).
    raw(p) + (SIDE_AT - raw(Vec3::new(-100.0, 150.0, 0.0)))
}

/// The feet (camera frame, full size) and colour of a soldier at ring
/// place `ring` (0..6, fractions between places as `FUN_0039a5f8` blends
/// them: place `i` toward place `i + 1`).
pub fn lineup_at(ring: f32) -> (Vec3, [f32; 4]) {
    let ring = ring.rem_euclid(6.0);
    let i = (ring.floor() as usize).min(5);
    let t = ring - i as f32;
    let j = (i + 1) % 6;
    let lerp = |a: f32, b: f32| a * (1.0 - t) + b * t;
    let x = lerp(SLOTS[i][0], SLOTS[j][0]);
    let z = lerp(SLOTS[i][1], SLOTS[j][1]);
    let colour = std::array::from_fn(|k| {
        let (a, b) = (SLOT_COLOURS[i][k], SLOT_COLOURS[j][k]);
        if k == 3 {
            lerp(f32::from(a), f32::from(b)) / 255.0
        } else {
            lerp(colour_unit(a), colour_unit(b))
        }
    });
    let ui = RING_AT + Vec3::new(x, 0.0, z) * RING_SCALE;
    (ui_to_camera(ui), colour)
}

/// The ring place of class `j` when `pick` of `n` is in front: `2 + k`
/// for its offset `k` from the pick, taken within the `n` offsets the PS2
/// shows (`FUN_0039a980`'s switch: -2..=2 for five or more classes, else
/// from -((n - 1) / 2)); the rest wait unseen at place 5.
pub fn ring_place(j: usize, pick: usize, n: usize) -> usize {
    let n = n.max(1) as i64;
    let lo = if n >= 6 { -3 } else { -((n - 1) / 2) };
    let k = (j as i64 - pick as i64 - lo).rem_euclid(n) + lo;
    if (-2..=2).contains(&k) {
        (2 + k) as usize
    } else {
        5
    }
}

/// Where class `j` stands `t` (0..1) into a slide from `from` to `pick`:
/// a pick one after the old (Left, class + 1) turns the ring back, one
/// before (Right) turns it on, each soldier going the way round the ring
/// that direction takes (the one that wraps sweeps the back places,
/// `FUN_0039a980`'s n < 6 arm); any other change cuts.
pub fn ring_at(j: usize, from: usize, pick: usize, n: usize, t: f32) -> f32 {
    let new = ring_place(j, pick, n) as f32;
    let old = ring_place(j, from, n) as f32;
    let n = n.max(1);
    if pick == (from + 1) % n && pick != from {
        old - (old - new).rem_euclid(6.0) * t
    } else if from == (pick + 1) % n && pick != from {
        old + (new - old).rem_euclid(6.0) * t
    } else {
        new
    }
}

/// The line-up's scale about the eye: its farthest soldier, at place 0
/// (9.4 m at full size), stands within a metre and a quarter.
pub const LINEUP_SHRINK: f32 = 0.12;

/// `[ours]` How far a droideka seems to reach from its feet, in metres at full
/// size, left and right on screen: its own model has no
/// [`SoldierVisuals::still_outline`]. The class screen's real pixels on every
/// baked map: 1.49 left, 0.80 right.
const DROIDEKA_REACH: [f32; 2] = [1.5, 0.85];

/// `[ours]` The gap kept to the class panel and to the window's left edge, in
/// the shell's screen units.
const FIT_MARGIN: f32 = 6.0;

/// A soldier's cloth and hair past his outline.
const OUTLINE_GROWTH: f32 = 1.05;

/// Places along the ring the group's bounds are sampled at: a slide passes
/// between the ring's stops.
const RING_SAMPLES: usize = 60;

/// How the line-up is moved on screen for a window: see [`lineup_fit`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineupFit {
    /// The move across the screen, in tangents (x / depth).
    shift: f32,
    /// The scale on screen, 1 unless the room is narrower than the group.
    pub scale: f32,
    /// The height the scale keeps: the front soldier's feet.
    feet: f32,
}

/// The class panel's left edge in screen tangents, for a camera of `tan_v`:
/// the panel is centre-anchored in the 4:3 box of the window's height, so
/// this does not depend on the aspect.
fn panel_left(tan_v: f32) -> f32 {
    (ui::spawn_draw::CLASS_BOX[0] / 256.0 - 1.0) * (4.0 / 3.0) * tan_v
}

/// `[ours]` The line-up's move for a window of `aspect` under a camera of
/// half-height tangent `tan_v` (the shot's, or the play camera's on the pause
/// class change): its right edge a margin from the panel, scaled down only as
/// much as its left edge needs to stay on the window. `outline` is the team's
/// widest ([`Outline`]) with every class at every place: every class passes
/// every seen place, so it holds for any pick and through a slide, and the group
/// does not jump between picks.
pub fn lineup_fit(aspect: f32, tan_v: f32, outline: &Outline) -> LineupFit {
    let places: Vec<Vec3> = (0..RING_SAMPLES)
        .map(|k| lineup_at(6.0 * k as f32 / RING_SAMPLES as f32))
        .filter(|(_, colour)| colour[3] >= SEEN_ALPHA)
        .map(|(at, _)| at)
        .collect();
    let points: Vec<[f32; 2]> = outline.points().collect();
    let (left, right) = (
        -aspect * tan_v + FIT_MARGIN / 256.0 * (4.0 / 3.0) * tan_v,
        panel_left(tan_v) - FIT_MARGIN / 256.0 * (4.0 / 3.0) * tan_v,
    );
    let front = lineup_at(2.0).0;
    let fit_of = |shift, scale| LineupFit {
        shift,
        scale,
        feet: front.y / front.z,
    };
    let span = |fit: &LineupFit| {
        let mut out = (f32::MAX, f32::MIN);
        for &at in &places {
            let feet = fit.feet(at);
            for [x, z] in &points {
                let depth = (feet.z - z * fit.scale).fmax(feet.z * 0.25);
                let seen = (feet.x + x * fit.scale) / depth;
                out = (out.0.fmin(seen), out.1.fmax(seen));
            }
        }
        out
    };
    // The move that puts the group's right edge on the panel's margin; its edge
    // only moves right as the group does.
    let against_panel = |scale| {
        let (mut low, mut high) = (-8.0, 8.0);
        for _ in 0..20 {
            let mid: f32 = (low + high) * 0.5;
            if span(&fit_of(mid, scale)).1 < right {
                low = mid;
            } else {
                high = mid;
            }
        }
        fit_of(low, scale)
    };
    let whole = against_panel(1.0);
    if span(&whole).0 >= left {
        return whole;
    }
    let (mut fits, mut wide) = (0.1, 1.0);
    for _ in 0..12 {
        let mid: f32 = (fits + wide) * 0.5;
        if span(&against_panel(mid)).0 >= left {
            fits = mid;
        } else {
            wide = mid;
        }
    }
    against_panel(fits)
}

impl LineupFit {
    /// The feet of a soldier standing at `at`, moved: the same depth, so the
    /// back of the group stays as near the eye as [`LINEUP_SHRINK`] put it.
    pub fn feet(&self, at: Vec3) -> Vec3 {
        let x = self.shift + self.scale * at.x / at.z;
        let y = self.feet + self.scale * (at.y / at.z - self.feet);
        Vec3::new(x * at.z, y * at.z, at.z)
    }

    /// The model matrix of a line-up soldier whose place is `at`.
    pub fn place(&self, cam: &Camera, at: Vec3, yaw: f32) -> Mat4 {
        place_at(cam, self.feet(at), yaw, LINEUP_SHRINK) * Mat4::from_scale(Vec3::splat(self.scale))
    }
}

/// The widest of `classes`, grown a little for what is not in it.
fn team_outline(vis: &SoldierVisuals, team: u8, classes: &[u8]) -> Outline {
    let mut wide = Outline::default();
    for &class in classes {
        match vis.still_outline(team, class) {
            Some(one) => wide.merge(&one),
            None => {
                wide.add(Vec3::new(-DROIDEKA_REACH[0], 0.0, 0.0));
                wide.add(Vec3::new(DROIDEKA_REACH[1], 0.0, 0.0));
            }
        }
    }
    let mut grown = Outline::default();
    for [x, z] in wide.points() {
        grown.add(Vec3::new(x * OUTLINE_GROWTH, 0.0, z * OUTLINE_GROWTH));
    }
    grown
}

/// The model matrix of a soldier whose feet stand at `at` in `cam`'s frame
/// (right, up, ahead; full size) turned `yaw` from facing the camera, drawn
/// at [`SHRINK`] about the eye. The soldier models face +Z.
pub fn place(cam: &Camera, at: Vec3, yaw: f32) -> Mat4 {
    place_at(cam, at, yaw, SHRINK)
}

/// [`place`] at the scale `shrink` about the eye.
pub fn place_at(cam: &Camera, at: Vec3, yaw: f32, shrink: f32) -> Mat4 {
    let f = cam.forward().normalize_or_zero();
    let r = cam.right().normalize_or_zero();
    let u = r.cross(f);
    let feet = cam.position + (r * at.x + u * at.y + f * at.z) * shrink;
    Mat4::from_cols(
        r.extend(0.0),
        u.extend(0.0),
        (-f).extend(0.0),
        feet.extend(1.0),
    ) * Mat4::from_rotation_y(yaw)
        * Mat4::from_scale(Vec3::splat(shrink))
}

/// The screens' soldiers between frames: an animator per soldier, kept
/// while the same team's soldiers are shown, and the line-up's slide.
#[derive(Default)]
pub struct Preview {
    animators: Vec<Animator>,
    cloths: Vec<PreviewCloth>,
    cloth_meshes: std::collections::HashMap<(usize, String, usize), render::MeshHandle>,
    shown: Option<(u8, u8)>,
    /// The line-up's team and front class, and the slide: (from, t).
    lineup: Option<(u8, usize)>,
    slide: Option<(usize, f32)>,
    /// The window's aspect, as [`Self::draws_live`]'s renderer says it; 4:3 for
    /// the entries with no renderer.
    aspect: Option<f32>,
    /// The widest of the classes shown.
    outline: Option<(Vec<u8>, Outline)>,
    /// The line-up's move under the window's aspect and the camera's tangent
    /// ([`lineup_fit`]).
    fit: Option<(f32, f32, LineupFit)>,
}

#[derive(Default)]
struct PreviewCloth {
    body: String,
    states: Vec<crate::cloth::State>,
}

impl Preview {
    /// The draws of the soldiers `page` stands under `cam`, their clips
    /// stepped `dt`: (bodies, weapons); empty when it stands none or they
    /// have no model.
    pub fn draws(
        &mut self,
        vis: &SoldierVisuals,
        page: Option<&SpawnPage>,
        cam: &Camera,
        dt: f32,
    ) -> (Vec<SkinnedDrawCmd>, Vec<DrawCmd>) {
        self.draws_with(vis, page, cam, dt, |_, _, _, _| None)
    }

    /// Specialized class models replace the infantry body and weapon together.
    /// `Some(empty)` means the class has no available model, so it must not borrow
    /// another class's infantry geometry. No companion cloth: that needs the
    /// renderer, see [`Self::draws_live`].
    pub fn draws_with(
        &mut self,
        vis: &SoldierVisuals,
        page: Option<&SpawnPage>,
        cam: &Camera,
        dt: f32,
        mut specialized: impl FnMut(u8, u8, Mat4, f32) -> Option<Vec<DrawCmd>>,
    ) -> (Vec<SkinnedDrawCmd>, Vec<DrawCmd>) {
        self.compose(None, vis, page, cam, dt, |_, team, class, model, dt| {
            specialized(team, class, model, dt)
        })
    }

    /// A live preview: specialized class models as in [`Self::draws_with`], and
    /// infantry bodies upload their companion cloth. The CPU-only/golden entry
    /// keeps its historical fallback and needs no renderer.
    pub fn draws_live(
        &mut self,
        r: &mut render::Renderer,
        vis: &SoldierVisuals,
        page: Option<&SpawnPage>,
        cam: &Camera,
        dt: f32,
        mut specialized: impl FnMut(&mut render::Renderer, u8, u8, Mat4, f32) -> Option<Vec<DrawCmd>>,
    ) -> (Vec<SkinnedDrawCmd>, Vec<DrawCmd>) {
        let (w, h) = r.size();
        self.aspect = Some(w as f32 / h.max(1) as f32);
        self.compose(Some(r), vis, page, cam, dt, |r, team, class, model, dt| {
            r.and_then(|r| specialized(r, team, class, model, dt))
        })
    }

    fn compose(
        &mut self,
        mut r: Option<&mut render::Renderer>,
        vis: &SoldierVisuals,
        page: Option<&SpawnPage>,
        cam: &Camera,
        dt: f32,
        mut specialized: impl FnMut(
            Option<&mut render::Renderer>,
            u8,
            u8,
            Mat4,
            f32,
        ) -> Option<Vec<DrawCmd>>,
    ) -> (Vec<SkinnedDrawCmd>, Vec<DrawCmd>) {
        // Interface draws use the camera's orientation and a local origin.
        // A distant backdrop must not round away the preview's skin and depth.
        let cam = &Camera {
            position: Vec3::ZERO,
            ..*cam
        };
        let who = page.and_then(shown);
        let line = match page {
            Some(SpawnPage::Class {
                team,
                class,
                classes,
                ..
            }) if !classes.is_empty() => Some((*team, *class, classes.as_slice())),
            _ => None,
        };
        let refused = match page {
            Some(SpawnPage::Class { refused, .. }) => *refused,
            _ => 0,
        };
        let same_team = matches!((self.lineup, line), (Some((a, _)), Some((b, _, _))) if a == b);
        if who != self.shown
            || (line.is_some() && !same_team)
            || (line.is_none() && self.lineup.is_some())
        {
            let cloth_meshes = std::mem::take(&mut self.cloth_meshes);
            *self = Self {
                cloth_meshes,
                shown: who,
                lineup: line.map(|(team, class, _)| (team, class)),
                aspect: self.aspect,
                ..Default::default()
            };
        }
        let mut out = (Vec::new(), Vec::new());
        if let Some((team, class)) = who {
            self.animators.resize_with(1, Animator::default);
            self.cloths.resize_with(1, PreviewCloth::default);
            let model = place(cam, SIDE_AT, SOLDIER_YAW);
            self.stand_nth(
                0,
                vis,
                team,
                class,
                model,
                [colour_unit(SIDE_TINT); 3],
                dt,
                &mut specialized,
                &mut out,
                r.as_deref_mut(),
            );
        }
        if let Some((team, pick, classes)) = line {
            let n = classes.len();
            let from = self.lineup.map_or(pick, |(_, c)| c);
            if from != pick {
                self.slide = Some((from, 0.0));
                self.lineup = Some((team, pick));
            }
            let (from, t) = match &mut self.slide {
                Some((from, t)) => {
                    *t = (*t + dt * SLIDE_RATE).fmin(1.0);
                    (*from, *t)
                }
                None => (pick, 1.0),
            };
            if t >= 1.0 {
                self.slide = None;
            }
            self.animators.resize_with(n, Animator::default);
            self.cloths.resize_with(n, PreviewCloth::default);
            let (sin, cos) = core::fmath::sin_cos(cam.fov_y * 0.5);
            let (aspect, tan_v) = (self.aspect.unwrap_or(4.0 / 3.0), sin / cos);
            if self.outline.as_ref().is_none_or(|(of, _)| of != classes) {
                self.outline = Some((classes.to_vec(), team_outline(vis, team, classes)));
                self.fit = None;
            }
            let fit = match self.fit {
                Some((a, t, fit)) if a == aspect && t == tan_v => fit,
                _ => {
                    let outline = self.outline.as_ref().map(|(_, o)| *o).unwrap_or_default();
                    let fit = lineup_fit(aspect, tan_v, &outline);
                    self.fit = Some((aspect, tan_v, fit));
                    fit
                }
            };
            for (j, &class) in classes.iter().enumerate() {
                let (at, colour) = lineup_at(ring_at(j, from, pick, n, t));
                if colour[3] < SEEN_ALPHA {
                    continue;
                }
                let model = fit.place(cam, at, SOLDIER_YAW);
                // A refused class's colour word is (0, 0, 0, 0xff): black
                // (`FUN_0039a980`, B2:394368-394383).
                let tint = if refused >> j & 1 != 0 {
                    [0.0; 3]
                } else {
                    [colour[0], colour[1], colour[2]]
                };
                self.stand_nth(
                    j,
                    vis,
                    team,
                    class,
                    model,
                    tint,
                    dt,
                    &mut specialized,
                    &mut out,
                    r.as_deref_mut(),
                );
            }
        }
        out
    }

    /// Soldier `k`'s draws (his own animator), tinted by `tint`.
    #[allow(clippy::too_many_arguments)]
    fn stand_nth(
        &mut self,
        k: usize,
        vis: &SoldierVisuals,
        team: u8,
        class: u8,
        model: Mat4,
        tint: [f32; 3],
        dt: f32,
        specialized: &mut impl FnMut(
            Option<&mut render::Renderer>,
            u8,
            u8,
            Mat4,
            f32,
        ) -> Option<Vec<DrawCmd>>,
        out: &mut (Vec<SkinnedDrawCmd>, Vec<DrawCmd>),
        mut r: Option<&mut render::Renderer>,
    ) {
        if let Some(mut draws) = specialized(r.as_deref_mut(), team, class, model, dt) {
            for d in &mut draws {
                d.color[..3].iter_mut().zip(tint).for_each(|(v, t)| *v *= t);
                d.light = render::LightPath::Iface;
            }
            out.1.extend(draws);
            return;
        }
        let Some((mut body, mut gun)) =
            vis.still_draws(&mut self.animators[k], team, class, model, dt)
        else {
            return;
        };
        if let Some(r) = r {
            self.cloth_draws(r, vis, k, team, class, model, dt, &mut body, &mut gun);
        }
        let tint = |c: &mut [f32; 4]| c[..3].iter_mut().zip(tint).for_each(|(v, t)| *v *= t);
        body.iter_mut().for_each(|d| {
            tint(&mut d.color);
            d.light = render::LightPath::Iface;
        });
        gun.iter_mut().for_each(|d| {
            tint(&mut d.color);
            d.light = render::LightPath::Iface;
        });
        out.0.extend(body);
        out.1.extend(gun);
    }

    #[allow(clippy::too_many_arguments)]
    fn cloth_draws(
        &mut self,
        r: &mut render::Renderer,
        vis: &SoldierVisuals,
        k: usize,
        team: u8,
        class: u8,
        model: Mat4,
        dt: f32,
        body: &mut Vec<SkinnedDrawCmd>,
        out: &mut Vec<DrawCmd>,
    ) {
        let Some(sa) = vis
            .per_class
            .get(team as usize)
            .and_then(|t| t.get(class as usize))
            .and_then(|c| c.body)
            .and_then(|b| vis.assets.get(b))
        else {
            return;
        };
        let preview = &mut self.cloths[k];
        if preview.body != sa.asset.name {
            preview.body = sa.asset.name.clone();
            preview.states.clear();
        }
        let palette = sa
            .asset
            .skeleton
            .palette(&self.animators[k].pose(&sa.asset.skeleton).locals());
        let globals: Vec<_> = palette
            .iter()
            .zip(&sa.asset.skeleton.global_bind)
            .map(|(p, b)| *p * *b)
            .collect();
        if preview.states.len() != sa.cloths.len() {
            preview.states = sa
                .cloths
                .iter()
                .enumerate()
                .map(|(i, c)| crate::cloth::State::new(c, model, &globals, i as u32))
                .collect();
        }
        for (i, (state, def)) in preview.states.iter_mut().zip(&sa.cloths).enumerate() {
            state.step(def, model, &globals, dt);
            let Some(source) = sa.meshes.get(def.raw.part) else {
                continue;
            };
            let Some(material) = body.iter().find(|d| d.mesh == source.0) else {
                continue;
            };
            let (texture, layer) = (material.texture, material.layer);
            let vertices = state.vertices(def, &globals);
            let indices: Vec<_> = def.raw.triangles.iter().flatten().copied().collect();
            let key = (k, sa.asset.name.clone(), i);
            let mesh = match crate::cloth::upload(
                r,
                self.cloth_meshes.get(&key).copied(),
                &vertices,
                &indices,
            ) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("preview cloth: {e}");
                    continue;
                }
            };
            self.cloth_meshes.insert(key, mesh);
            let mut cmd = match texture {
                Some(t) => DrawCmd::textured(mesh, t, model),
                None => DrawCmd::flat(mesh, [1.; 4], model),
            };
            cmd.layer = layer;
            out.push(cmd);
            body.retain(|d| d.mesh != source.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(selected: usize) -> SpawnPage {
        SpawnPage::Side {
            title: String::new(),
            rows: vec!["Empire".into(), "Rebels".into(), "Auto Assign".into()],
            selected,
            select: String::new(),
        }
    }

    /// The highlighted team's first class; none on Auto Assign or another
    /// page.
    #[test]
    fn the_side_select_shows_the_highlighted_teams_first_class() {
        assert_eq!(shown(&side(0)), Some((0, 0)));
        assert_eq!(shown(&side(1)), Some((1, 0)));
        assert_eq!(shown(&side(2)), None, "Auto Assign: no soldier");
        let map = SpawnPage::Map {
            objective: String::new(),
            legend: Default::default(),
        };
        assert_eq!(shown(&map), None);
    }

    /// Shrunk about the eye, he projects where he would at full size.
    #[test]
    fn the_shrink_keeps_the_picture() {
        let cam = Camera {
            position: Vec3::new(10.0, 2.0, -4.0),
            yaw: 0.7,
            pitch: -0.1,
            fov_y: 2.0 * 0.34131f32.atan(),
            ..Default::default()
        };
        let vp = cam.view_proj(4.0 / 3.0);
        let m = place(&cam, SIDE_AT, SOLDIER_YAW);
        let f = cam.forward().normalize();
        let r = cam.right().normalize();
        let full = cam.position + r * SIDE_AT.x + r.cross(f) * SIDE_AT.y + f * SIDE_AT.z;
        let head = |p: Vec3| vp.project_point3(p).truncate();
        for local in [
            Vec3::ZERO,
            Vec3::new(0.0, 1.8, 0.0),
            Vec3::new(0.3, 1.0, 0.2),
        ] {
            let near = m.transform_point3(local);
            let far = full
                + (Mat4::from_cols(
                    r.extend(0.0),
                    r.cross(f).extend(0.0),
                    (-f).extend(0.0),
                    glam::Vec4::W,
                ) * Mat4::from_rotation_y(SOLDIER_YAW))
                .transform_vector3(local);
            assert!((head(near) - head(far)).length() < 1e-4, "{local}");
        }
        // His feet's origin where the fit put it: x 188 of 640 px, y 414.
        let p = head(m.transform_point3(Vec3::ZERO));
        let px = ((p.x + 1.0) * 320.0, (1.0 - p.y) * 240.0);
        assert!(
            (px.0 - 188.0).abs() < 2.0 && (px.1 - 414.0).abs() < 2.0,
            "{px:?}"
        );
    }

    /// The still line-up as `FUN_0039a980`'s switch places it (the pick
    /// `c` in front): five or more classes c-2..=c+2 at places 0..=4,
    /// four c-1..=c+2 at 1..=4, three c-1..=c+1, two c and c+1, one c; the
    /// rest unseen at place 5.
    #[test]
    fn the_still_line_up_follows_the_switch() {
        let places = |pick: usize, n: usize| -> Vec<usize> {
            (0..n).map(|j| ring_place(j, pick, n)).collect()
        };
        assert_eq!(places(0, 1), [2]);
        assert_eq!(places(0, 2), [2, 3]);
        assert_eq!(places(1, 2), [3, 2]);
        assert_eq!(places(0, 3), [2, 3, 1]);
        assert_eq!(places(0, 4), [2, 3, 4, 1]);
        assert_eq!(places(0, 5), [2, 3, 4, 0, 1]);
        // Hoth's Empire: the dark trooper (5) at place 1, the officer (4)
        // at 0, the sniper (3) unseen (`t1_hot1/s4_class`).
        assert_eq!(places(0, 6), [2, 3, 4, 5, 0, 1]);
        assert_eq!(places(3, 8), [5, 0, 1, 2, 3, 4, 5, 5]);
    }

    /// Left (class + 1) turns the ring back one place in a third of a
    /// second; Right turns it on; with five classes the one that wraps
    /// sweeps the back (0 -> 5 -> 4), with six one comes in from place 5
    /// and one goes out to it.
    #[test]
    fn a_pick_slides_the_ring() {
        assert_eq!(1.0 / SLIDE_RATE, 1.0 / 3.0);
        // Five: 0 -> 1 (Left). Class 3 was at place 0, ends at 4.
        assert_eq!(ring_at(1, 0, 1, 5, 0.0), 3.0);
        assert_eq!(ring_at(1, 0, 1, 5, 1.0), 2.0);
        assert_eq!(ring_at(3, 0, 1, 5, 0.5), -1.0, "half way: place 5");
        assert_eq!(ring_at(3, 0, 1, 5, 1.0).rem_euclid(6.0), 4.0);
        // Six: class 3 comes in from 5 to 4, class 4 goes out from 0 to 5.
        assert_eq!(ring_at(3, 0, 1, 6, 0.0), 5.0);
        assert_eq!(ring_at(3, 0, 1, 6, 1.0), 4.0);
        assert_eq!(ring_at(4, 0, 1, 6, 1.0).rem_euclid(6.0), 5.0);
        // Right (class - 1) the other way; a jump cuts.
        assert_eq!(ring_at(0, 1, 0, 6, 0.5), 1.5);
        assert_eq!(ring_at(0, 3, 0, 6, 0.5), 2.0);
        // Half way to place 5 a soldier is half seen; at 5 unseen.
        assert!(lineup_at(4.5).1[3] > 0.5 && lineup_at(5.0).1[3] < 0.01);
    }

    fn outline(points: &[[f32; 2]]) -> Outline {
        let mut out = Outline::default();
        for [x, z] in points {
            out.add(Vec3::new(*x, 0.0, *z));
        }
        out
    }

    /// Every outline at every ring place, any pick, through a slide: no point
    /// crosses the panel or the window's left edge, the front soldier stays the
    /// nearest, a muzzle toward the viewer counts, and a wide window does not
    /// shrink an infantry team. A droideka's reach costs its team some size even
    /// at 16:9, and most at 4:3.
    #[test]
    fn the_group_clears_the_panel_at_any_aspect() {
        let launcher = outline(&[[-0.3, -0.3], [-0.5, 1.1], [0.4, 0.0]]);
        let droideka = outline(&[[-DROIDEKA_REACH[0], 0.0], [DROIDEKA_REACH[1], 0.0]]);
        let small = outline(&[[-0.4, 0.0], [0.4, 0.0]]);
        for tan_v in [TAN_V, 0.7] {
            for aspect in [4.0 / 3.0, 16.0 / 9.0, 3149.0 / 1610.0, 21.0 / 9.0, 1.0] {
                let panel = panel_left(tan_v);
                let front = lineup_at(2.0).0.z;
                for shape in [&launcher, &droideka, &small] {
                    let fit = lineup_fit(aspect, tan_v, shape);
                    for k in 0..600 {
                        let (at, colour) = lineup_at(k as f32 / 100.0);
                        if colour[3] < SEEN_ALPHA {
                            continue;
                        }
                        let feet = fit.feet(at);
                        assert_eq!(feet.z, at.z, "the depth stays");
                        for [x, z] in shape.points() {
                            let seen = (feet.x + x * fit.scale) / (feet.z - z * fit.scale);
                            assert!(seen < panel, "{aspect} {tan_v} {k}: {seen} {panel}");
                            assert!(seen > -aspect * tan_v, "{aspect} {tan_v} {k}: {seen}");
                        }
                        assert!(at.z >= front - 1e-3, "place 2 is the nearest");
                    }
                }
                if aspect >= 16.0 / 9.0 && tan_v == TAN_V {
                    assert_eq!(lineup_fit(aspect, tan_v, &launcher).scale, 1.0, "{aspect}");
                }
            }
        }
        assert!(lineup_fit(4.0 / 3.0, TAN_V, &launcher).scale > 0.7);
        let droideka = outline(&[[-DROIDEKA_REACH[0], 0.0], [DROIDEKA_REACH[1], 0.0]]);
        let (narrow, wide, wider) = (
            lineup_fit(4.0 / 3.0, TAN_V, &droideka).scale,
            lineup_fit(16.0 / 9.0, TAN_V, &droideka).scale,
            lineup_fit(1.96, TAN_V, &droideka).scale,
        );
        assert!((0.6..0.75).contains(&narrow), "{narrow}");
        assert!((0.85..1.0).contains(&wide), "{wide}");
        assert_eq!(wider, 1.0);
    }

    /// The fit only moves a group left of the panel it would otherwise go behind.
    #[test]
    fn the_ps2_places_cross_the_panel_unmoved() {
        let shape = outline(&[[-0.4, 0.0], [0.4, 0.0]]);
        let (at, _) = lineup_at(4.0);
        let unmoved = (at.x + 0.4) / at.z;
        assert!(unmoved > panel_left(TAN_V), "{unmoved}");
        let moved = lineup_fit(16.0 / 9.0, TAN_V, &shape).feet(at);
        assert!((moved.x + 0.4) / moved.z < panel_left(TAN_V));
    }

    /// The line-up stands where the PS2's Hoth frame has it: the dark
    /// trooper (place 1) 105 px left of the Snowtrooper's feet and 88 up,
    /// the officer (place 0) 55 left and 116 up (`t1_hot1/s4_class.png`,
    /// 640 x 480), within 8 px; the back stands within 1.25 m of the eye.
    #[test]
    fn the_line_up_stands_on_the_ps2_places() {
        let cam = Camera {
            position: Vec3::new(3.0, 1.0, 2.0),
            yaw: -0.4,
            pitch: 0.05,
            fov_y: 2.0 * TAN_V.atan(),
            ..Default::default()
        };
        let vp = cam.view_proj(4.0 / 3.0);
        let px = |ring: f32| {
            let m = place_at(&cam, lineup_at(ring).0, SOLDIER_YAW, LINEUP_SHRINK);
            let p = vp.project_point3(m.transform_point3(Vec3::ZERO));
            glam::Vec2::new((p.x + 1.0) * 320.0, (1.0 - p.y) * 240.0)
        };
        let front = px(2.0);
        for (ring, want) in [(1.0, (-105.0, -88.0)), (0.0, (-55.0, -116.0))] {
            let d = px(ring) - front;
            assert!(
                (d.x - want.0).abs() < 8.0 && (d.y - want.1).abs() < 8.0,
                "place {ring}: {d}"
            );
        }
        for k in 0..6 {
            assert!(lineup_at(k as f32).0.z * LINEUP_SHRINK < 1.25, "place {k}");
        }
        // The side preview's own place maps onto its fit.
        assert!((ui_to_camera(Vec3::new(-100.0, 150.0, 0.0)) - SIDE_AT).length() < 1e-5);
    }
}
