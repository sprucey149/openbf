//! The command post's hologram (card CPHOL, `analysis/re-point-lights.md` §3,
//! §10 and §15 [O]; BF2 `EntityHologram` update `FUN_002172a8`, draw
//! `FUN_002186c8`; BF1's twins `FUN_001e6a68`, `FUN_001e7d88`): over every
//! post a translucent additive beam in the VIEWER's relation colour and the
//! owner's team icon turning at its top.
//!
//! **What is drawn** (the class `com_holo_controlzone`, [`CONTROLZONE`]):
//! - the beam ([`render::Beam`]): from the post building's `hp_hologram`
//!   point 3.5 m up, radius 0.5 → 1.0 m, colour = the relation colour,
//!   alpha = I × `HoloBeamIntensity`;
//! - the icon: the owner's `com_icon_<side>` model at `HoloHeight`, scaled
//!   by `HoloSize`, turning at `HoloRotateRate` (one turn in 5 s), tinted in
//!   the relation colour, alpha = I × `HoloImageIntensity` 0.5, source-over,
//!   no depth write. A neutral post has none.
//!
//! **I, the intensity** ([`Holo`]): the fade × a random one-update dip to
//! U[0.4, 0.8]. The fade runs 2 s (`HoloFadeInTime`) either way; the post
//! switches its hologram OFF while its neutralize meter (an owned post) or
//! its capture meter (a neutral one) is within `HoloTurnOnTime` 2.2 s of
//! its end, and on again otherwise; a change of owner sets the new team and
//! icon and switches it on. A switch in mid-fade turns the fade round where
//! it is. The "pop" is the picture's own glitch: at random the icon is
//! squashed or stretched (x, z × p, y ÷ p, p from U[0.5, 2), easing back to
//! 1), sometimes tilted and its turn thrown to a random angle.
//!
//! The state steps at the PS2's update rate ([`STEP`]) on the SIM's clock,
//! with its own random stream: nothing here is simulation state. A render
//! with no battle ([`still_draws`]) shows every post steady at its starting
//! owner, seen by team 1.
//!
//! The axial flare and authored scan-line scroll follow §16.
//! PROVISIONAL / not built: the direction of the icon's turn ([H]), END1's and HOT1's holograms on a building that is not the
//! post's own (they stand at the post).

use std::collections::HashMap;

use glam::{Mat4, Vec3};
use render::{Beam, Camera, ParticleDraw, ParticleShape, Renderer, TextureHandle};

use crate::assets::Scene;
use crate::point_light::Relation;
use crate::walk::WalkSim;

/// One PS2 update, seconds: the hologram's clocks and random draws step at
/// this rate (BF2's update step; BF1's taken as the same, PROVISIONAL).
pub const STEP: f32 = 1.0 / 30.0;

/// A hologram class's keys (`com_holo_controlzone`, both games' discs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoloClass {
    /// `HoloHeight`: the beam's length and the icon's height.
    pub height: f32,
    /// `HoloSize`: the icon's scale; the beam's top is 0.8 × it across.
    pub size: f32,
    /// The beam's base diameter (key 0x1de206a6; 0.5 m radius in RAM).
    pub base_width: f32,
    pub beam_intensity: f32,
    pub image_intensity: f32,
    /// `HoloFlareIntensity`, class +0x84 → beam +0x194 (BF2).
    pub flare_intensity: f32,
    /// `HoloRotateRate` × 2π, rad/s.
    pub rotate: f32,
    pub pop_rate: f32,
    pub flicker_rate: f32,
    pub flicker_alpha: (f32, f32),
    pub fade_time: f32,
    pub visible: f32,
    /// The post class's `HoloTurnOnTime`.
    pub turn_on_time: f32,
}

pub const CONTROLZONE: HoloClass = HoloClass {
    height: 3.5,
    size: 2.5,
    base_width: 1.0,
    beam_intensity: 1.0,
    image_intensity: 0.5,
    flare_intensity: 1.0,
    rotate: 0.2 * std::f32::consts::TAU,
    pop_rate: 0.01,
    flicker_rate: 0.01,
    flicker_alpha: (0.4, 0.8),
    fade_time: 2.0,
    visible: 200.0,
    turn_on_time: 2.2,
};

/// `hp_hologram` over the post's origin: `com_bldg_controlzone` and its
/// `_large` model (`lead_tmp/b1dk/re/hp_census.py`, RAM).
pub const HEIGHT: f32 = 0.320;
pub const HEIGHT_LARGE: f32 = 1.363;
pub const HEIGHT_BUNKER_PANEL: f32 = 0.670;

/// The relation colours of a game's team table (§3; BF1 re-kamino-decks §6).
pub fn relation_rgb(rel: Relation, bf1: bool) -> [u8; 3] {
    match (rel, bf1) {
        (Relation::Friend, false) => [1, 86, 213],
        (Relation::Friend, true) => [100, 200, 100],
        (Relation::Enemy, _) => [200, 100, 100],
        (Relation::Neutral, _) => [192, 192, 192],
    }
}

/// The PS2's random stream (two LCG steps a draw), private to the holograms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lcg(pub u32);

impl Lcg {
    /// U[0, 1).
    pub fn unit(&mut self) -> f32 {
        let a = self.0.wrapping_mul(0x0019_660d).wrapping_add(0x3c6e_f35f);
        self.0 = a.wrapping_mul(0x0019_660d).wrapping_add(0x3c6e_f35f);
        f32::from_bits(((a >> 16) | (self.0 & 0xffff_0000)) >> 9 | 0x3f80_0000) - 1.0
    }
}

/// One hologram's state (the PS2 object's fields by offset, BF2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Holo {
    /// +0xb4: the turn's clock (angle = spin × `rotate`).
    pub spin: f32,
    /// Continuous visual time, unaffected by the icon's random turn jumps.
    pub clock: f32,
    /// +0xb8, +0xc0: seconds since the last pop / dip.
    pub pop_clock: f32,
    pub flicker_clock: f32,
    /// +0xbc: the squash, easing to 1.
    pub pop: f32,
    /// +0xc4: the fade's clock.
    pub fade_clock: f32,
    /// +0xcc: the icon's tilt, radians.
    pub tilt: f32,
    /// +0xd4, +0xd0: the fade and the intensity I.
    pub fade: f32,
    pub intensity: f32,
    /// +0xd9: switched on.
    pub on: bool,
    /// +0xdc: the team it shows.
    pub team: Option<u8>,
}

impl Holo {
    /// Steady and on, showing `team`.
    pub fn steady(team: Option<u8>) -> Self {
        Holo {
            spin: 0.0,
            clock: 0.0,
            pop_clock: 0.0,
            flicker_clock: 0.0,
            pop: 1.0,
            fade_clock: CONTROLZONE.fade_time,
            tilt: 0.0,
            fade: 1.0,
            intensity: 1.0,
            on: true,
            team,
        }
    }

    /// Switch on or off (`FUN_00218550` / `FUN_00218658`): a fade under way
    /// turns round where it is.
    pub fn switch(&mut self, on: bool, c: &HoloClass) {
        if self.on != on {
            self.fade_clock = if self.fade_clock < c.fade_time {
                c.fade_time - self.fade_clock
            } else {
                0.0
            };
            self.on = on;
        }
    }

    /// One update of `dt` (`FUN_002172a8`).
    pub fn update(&mut self, dt: f32, c: &HoloClass, rng: &mut Lcg) {
        self.pop_clock += dt;
        self.spin += dt;
        self.clock += dt;
        self.flicker_clock += dt;
        self.fade_clock += dt;
        self.pop += (1.0 - self.pop) * (dt + dt).min(0.8);
        if rng.unit() < self.pop_clock * c.pop_rate * 0.5 {
            let r = rng.unit();
            if r < 0.8 {
                let q = rng.unit();
                self.pop = if q < 0.5 { 0.5 + q } else { q + q };
            }
            if r > 0.7 {
                let t = rng.unit();
                self.tilt = t * t * 0.523_598_85;
                self.spin = rng.unit() * std::f32::consts::TAU / c.rotate;
            }
            self.pop_clock = 0.0;
        }
        if rng.unit() < self.flicker_clock * c.flicker_rate * 0.5 {
            self.flicker_clock = 0.0;
        }
        self.fade = if self.fade_clock < c.fade_time {
            let t = self.fade_clock / c.fade_time;
            if self.on {
                t
            } else {
                1.0 - t
            }
        } else if self.on {
            1.0
        } else {
            0.0
        };
        self.intensity = self.fade;
        if self.flicker_clock <= 0.0 {
            let (lo, hi) = c.flicker_alpha;
            self.intensity *= lo + rng.unit() * (hi - lo);
        }
    }
}

/// Is a post's hologram switched on (the post's update, BF2 `FUN_00357440`
/// 352850): off while the meter that will change it is within
/// `HoloTurnOnTime` of its end.
pub fn switched_on(cp: &core::cp::CommandPost, c: &HoloClass) -> bool {
    let (meter, time) = match cp.owner {
        Some(_) => (cp.neutral, cp.def.neutralize_time),
        None => (cp.capture, cp.def.capture_time),
    };
    meter < time - c.turn_on_time
}

/// A model's triangles per sheet, in model space.
type Parts = Vec<IconPart>;

struct IconPart {
    texture: Option<TextureHandle>,
    tris: Vec<(Vec3, [f32; 2])>,
    scroll: [f32; 2],
}

/// What the holograms draw with: the four sides' icons and each post's
/// `hp_hologram` height by post name.
#[derive(Default)]
pub struct HoloArt {
    pub bf1: bool,
    /// By faction key (all | imp | rep | cis).
    icons: HashMap<&'static str, Parts>,
    pub heights: HashMap<String, f32>,
}

/// Faction key → the icon model's side name.
const SIDES: [(&str, &str); 4] = [
    ("all", "alliance"),
    ("imp", "imperial"),
    ("rep", "republic"),
    ("cis", "cis"),
];

/// The `hp_hologram` height of a post of class `class` whose geometry is
/// `geometry` where the map's own class table has it: the `_large` model
/// carries it at 1.363 m, and BF1's `com_bldg_major_controlzone` is that
/// model (BF2's is the ordinary one; both `common.ron`), Endor's bunker
/// panel at 0.670 m (its 2.5 m to the side is not built).
pub fn bone_height(class: &str, geometry: Option<&str>, bf1: bool) -> f32 {
    let class = class.to_ascii_lowercase();
    let large = geometry.is_some_and(|g| g.to_ascii_lowercase().contains("controlzone_large"))
        || (bf1 && geometry.is_none() && class == "com_bldg_major_controlzone");
    if large {
        HEIGHT_LARGE
    } else if class == "end_bldg_bunker_controlpanel" {
        HEIGHT_BUNKER_PANEL
    } else {
        HEIGHT
    }
}

impl HoloArt {
    /// Where the hologram of the post `name` at `pos` stands.
    pub fn origin(&self, name: &str, pos: Vec3) -> Vec3 {
        pos + Vec3::Y * self.heights.get(name).copied().unwrap_or(HEIGHT)
    }

    /// The icons of `common_<game>` and the scene's posts' heights. A
    /// missing icon is printed and not drawn.
    pub fn load(r: &mut Renderer, scene: &Scene) -> Self {
        let game = scene.manifest.source_game.to_ascii_lowercase();
        let mut art = HoloArt {
            bf1: game != "bf2",
            ..HoloArt::default()
        };
        for cp in &crate::layout::MapLayout::from_manifest(&scene.manifest).cps {
            // The class's geometry, drawn or not (the port draws no BF1 post).
            let geometry = scene
                .resolve_class(&cp.class)
                .and_then(|c| c.geometry.as_deref());
            art.heights
                .insert(cp.name.clone(), bone_height(&cp.class, geometry, art.bf1));
        }
        let Some(common) = scene.dir.parent().map(|d| d.join(format!("common_{game}"))) else {
            return art;
        };
        for (faction, side) in SIDES {
            let glb = common.join("models").join(format!("com_icon_{side}.glb"));
            let cpu = match crate::assets::load_glb(&glb) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("hologram icon com_icon_{side}: {e:#} (not drawn)");
                    continue;
                }
            };
            let mut parts = Parts::new();
            for (i, (start, count, material, _, _)) in cpu.submeshes.iter().enumerate() {
                let tex = crate::viewer::upload_texture_file(
                    r,
                    &common.join("textures"),
                    &material.to_ascii_lowercase(),
                )
                .unwrap_or_else(|e| {
                    eprintln!("hologram icon com_icon_{side}: {e:#}");
                    None
                });
                let tris = cpu.indices[*start as usize..(*start + *count) as usize]
                    .iter()
                    .map(|&i| {
                        let i = i as usize;
                        (
                            Vec3::from(cpu.positions[i]),
                            cpu.uvs.get(i).copied().unwrap_or([0.0; 2]),
                        )
                    })
                    .collect();
                parts.push(IconPart {
                    texture: tex,
                    tris,
                    scroll: cpu.uv_scroll(i),
                });
            }
            art.icons.insert(faction, parts);
        }
        art
    }
}

/// The icon's matrix over `origin` (the hologram's own point): scale
/// (s·p, s/p, s·p) with s = `HoloSize` × the fade, the tilt, the turn, then
/// `HoloHeight` up (`FUN_002186c8` 129980–129995).
pub fn icon_matrix(h: &Holo, c: &HoloClass, origin: Vec3) -> Mat4 {
    let s = c.size * h.fade;
    Mat4::from_translation(origin + Vec3::Y * c.height)
        * Mat4::from_rotation_y(-h.spin * c.rotate)
        * Mat4::from_rotation_x(-h.tilt)
        * Mat4::from_scale(Vec3::new(s * h.pop, s / h.pop, s * h.pop))
}

/// The icon's alpha byte for a view: I × `HoloImageIntensity`, faded over
/// the last 19 % of the visible distance squared and as the icon comes
/// within its own size of the near plane; `None` where it is not drawn.
pub fn icon_alpha(h: &Holo, c: &HoloClass, origin: Vec3, camera: &Camera) -> Option<u8> {
    let k = h.intensity * c.image_intensity;
    let d2 = camera.position.distance_squared(origin);
    let vis2 = c.visible * c.visible;
    if k <= 0.0 || d2 >= vis2 {
        return None;
    }
    let mut fade = if d2 > 0.81 * vis2 {
        (1.0 - d2 / vis2) * 5.263_156_4
    } else {
        1.0
    };
    let s = c.size * h.fade;
    let depth = (origin + Vec3::Y * c.height - camera.position).dot(camera.forward()) - camera.near;
    if depth <= 0.0 {
        return None;
    }
    if depth < s {
        fade *= depth / s;
    }
    Some((255.0 * k * fade) as u8)
}

/// Keep UVs bounded without changing their wrap/repeat phase.
fn scroll_uv(uv: [f32; 2], rate: [f32; 2], clock: f32) -> [f32; 2] {
    [
        uv[0] + (rate[0] * clock).fract(),
        uv[1] + (rate[1] * clock).fract(),
    ]
}

/// The axial flare's radius and alpha before the untextured GS colour scale.
/// BF2 `FUN_00469628`: g < 0, A g^8 HoloFlareIntensity; radius advances
/// halfway from the mean start radius to the mean end radius at g = -1.
fn flare_parameters(h: &Holo, c: &HoloClass, origin: Vec3, eye: Vec3) -> Option<(f32, u8)> {
    let v = origin - eye;
    let d2 = v.length_squared();
    if v.y >= 0.0 || d2 >= c.visible * c.visible || h.intensity <= 0.0 {
        return None;
    }
    let r0 = c.base_width * 0.5;
    let r1 = c.size * 0.4 * h.fade * h.pop;
    let g = v.y / d2.sqrt().max(r0);
    let g2 = g * g;
    let g4 = g2 * g2;
    let g8 = g4 * g4;
    let alpha = (255.0 * h.intensity * c.beam_intensity * g8 * c.flare_intensity).round() as u8;
    (alpha > 0).then_some((r0 + g8 * (r1 - r0) * 0.5, alpha))
}

/// Procedural, depth-tested flare: the native two three-point fans and
/// seven-sector soft ring (`FUN_00468238`, `FUN_00468a28`). The PS2's
/// readback occlusion is replaced by ordinary per-fragment depth testing.
fn flare_draw(
    h: &Holo,
    c: &HoloClass,
    origin: Vec3,
    rgb: [u8; 3],
    camera: &Camera,
) -> Option<ParticleDraw> {
    let (radius, alpha) = flare_parameters(h, c, origin, camera.position)?;
    let delta = origin - camera.position;
    let depth = delta.dot(camera.forward());
    if depth <= camera.near {
        return None;
    }
    let right = camera.right();
    let up = right.cross(camera.forward());
    let angle = radius * std::f32::consts::FRAC_PI_3
        + (delta.dot(right) + 2.0 * delta.dot(up)) * std::f32::consts::PI / depth;
    // LensFlare is untextured: GS RGB bytes are half-strength, while its
    // alpha byte is normalized on the GS's 0..128 scale by the particle path.
    let color = |a| {
        [
            rgb[0].div_ceil(2),
            rgb[1].div_ceil(2),
            rgb[2].div_ceil(2),
            a,
        ]
    };
    let at = |x, y, a| (origin + right * x + up * y, [0.0; 2], color(a));
    let on_ring = |t: f32, x: f32, y: f32, a| {
        let (sin, cos) = core::fmath::sin_cos(t);
        at(cos * x, sin * y, a)
    };
    let mut tris = Vec::with_capacity(81);
    for (rx, ry, phase) in [
        (2.0, 4.0, angle),
        (7.0, 2.0, std::f32::consts::FRAC_PI_3 - angle),
    ] {
        for i in 0..3 {
            let a = phase + i as f32 * std::f32::consts::TAU / 3.0;
            let b = a + std::f32::consts::TAU / 3.0;
            tris.extend([
                at(0.0, 0.0, alpha),
                on_ring(a, rx * radius, ry * radius, 0),
                on_ring(b, rx * radius, ry * radius, 0),
            ]);
        }
    }
    for i in 0..7 {
        let a = angle + i as f32 * std::f32::consts::TAU / 7.0;
        let b = a + std::f32::consts::TAU / 7.0;
        let inner = |t: f32| on_ring(t, radius, radius, alpha / 2);
        let outer = |t: f32| on_ring(t, radius * 4.0, radius * 4.0, 0);
        tris.extend([
            at(0.0, 0.0, alpha),
            inner(a),
            inner(b),
            inner(a),
            outer(a),
            outer(b),
            inner(a),
            outer(b),
            inner(b),
        ]);
    }
    Some(ParticleDraw {
        shape: ParticleShape::UntexturedShaded { tris },
        key: origin,
        color: color(alpha),
        texture: None,
        additive: true,
    })
}

/// One hologram's draws for a viewer on team `viewer`: beam, axial flare,
/// and the owner's icon.
#[allow(clippy::too_many_arguments)]
pub fn holo_draws(
    art: &HoloArt,
    h: &Holo,
    origin: Vec3,
    faction: Option<&str>,
    viewer: u8,
    camera: &Camera,
    out: &mut Vec<ParticleDraw>,
) {
    let c = &CONTROLZONE;
    if h.intensity <= 0.0 {
        return;
    }
    let rgb = relation_rgb(Relation::of(h.team, viewer), art.bf1);
    let rgba = |a: u8| [rgb[0], rgb[1], rgb[2], a];
    out.push(ParticleDraw {
        shape: ParticleShape::Beam(Beam {
            base: origin,
            axis: Vec3::Y,
            length: c.height,
            r0: c.base_width * 0.5,
            r1: c.size * 0.8 * 0.5 * h.fade * h.pop,
            soft: (0.2 * c.height).min(1.0),
            visible: c.visible,
            view: None,
            gs_rgb: if art.bf1 { 0.5 } else { 1.0 },
            top: render::BEAM_TOP_ALPHA,
            cross: None,
        }),
        key: origin + Vec3::Y * (c.height * 0.5),
        color: rgba((255.0 * h.intensity * c.beam_intensity) as u8),
        texture: None,
        additive: true,
    });
    if let Some(flare) = flare_draw(h, c, origin, rgb, camera) {
        out.push(flare);
    }
    let icon = h
        .team
        .and(faction)
        .and_then(|f| art.icons.get(f.to_ascii_lowercase().as_str()));
    let (Some(parts), Some(alpha)) = (icon, icon_alpha(h, c, origin, camera)) else {
        return;
    };
    let m = icon_matrix(h, c, origin);
    out.extend(parts.iter().map(|part| {
        let scroll = scroll_uv([0.0; 2], part.scroll, h.clock);
        ParticleDraw {
            shape: ParticleShape::Mesh {
                tris: part
                    .tris
                    .iter()
                    .map(|(p, uv)| {
                        (
                            m.transform_point3(*p),
                            [uv[0] + scroll[0], uv[1] + scroll[1]],
                        )
                    })
                    .collect(),
            },
            key: origin + Vec3::Y * c.height,
            color: rgba(alpha),
            texture: part.texture,
            additive: false,
        }
    }));
}

/// A battle's holograms, one per post in the sim's order.
pub struct Holos {
    pub list: Vec<Holo>,
    /// The sim time the states stand at, seconds.
    clock: f64,
    rng: Lcg,
}

impl Default for Holos {
    fn default() -> Self {
        Holos {
            list: Vec::new(),
            clock: 0.0,
            rng: Lcg(0x0bf2_c0de),
        }
    }
}

impl Holos {
    /// Bring every hologram up to the sim's clock, [`STEP`] at a time (a
    /// new battle starts them steady at the posts' owners).
    pub fn advance(&mut self, walk: &WalkSim) {
        let cps = &walk.sim.cps;
        let now = walk.sim.tick as f64 * f64::from(core::TICK_DT);
        if self.list.len() != cps.len() || now < self.clock {
            *self = Holos {
                list: cps.iter().map(|cp| Holo::steady(cp.owner)).collect(),
                clock: now,
                ..Holos::default()
            };
        }
        while self.clock + f64::from(STEP) <= now {
            self.clock += f64::from(STEP);
            for (h, cp) in self.list.iter_mut().zip(cps) {
                step(h, cp, &CONTROLZONE, &mut self.rng);
            }
        }
    }

    /// Post `i`'s intensity I (1 before the first [`Holos::advance`]).
    pub fn intensity(&self, i: usize) -> f32 {
        self.list.get(i).map_or(1.0, |h| h.intensity)
    }

    /// This frame's draws for the player's view.
    pub fn draws(&self, art: &HoloArt, walk: &WalkSim, camera: &Camera) -> Vec<ParticleDraw> {
        let viewer = walk.player_team();
        let mut out = Vec::new();
        for (h, cp) in self.list.iter().zip(&walk.sim.cps) {
            let faction = h
                .team
                .and_then(|t| walk.teams.get(usize::from(t)))
                .map(|t| t.faction.as_str());
            let origin = art.origin(&cp.def.name, cp.def.pos);
            holo_draws(art, h, origin, faction, viewer, camera, &mut out);
        }
        out
    }
}

/// One update of a post's hologram: the post's switch, a change of owner
/// (the new team and icon, switched on: `FUN_00359c08`), the update.
pub fn step(h: &mut Holo, cp: &core::cp::CommandPost, c: &HoloClass, rng: &mut Lcg) {
    h.switch(switched_on(cp, c), c);
    if h.team != cp.owner {
        h.team = cp.owner;
        h.tilt = 0.0;
        h.switch(true, c);
    }
    h.update(STEP, c, rng);
}

/// The holograms of a render with no battle: every post of the conquest
/// mission steady at its starting owner, seen by team 1
/// (`crate::point_light::still_posts`).
pub fn still_draws(art: &HoloArt, scene: &Scene, camera: &Camera) -> Vec<ParticleDraw> {
    let mission = crate::point_light::still_mission(scene);
    let mut out = Vec::new();
    for cp in crate::point_light::still_posts(&scene.manifest, mission.as_ref()) {
        // Mission team numbers: 0 = neutral, 1 / 2 the sides.
        let team = (cp.team != 0).then_some(cp.team);
        let faction = mission
            .as_ref()
            .zip(team)
            .and_then(|(m, t)| m.teams.get(usize::from(t) - 1))
            .map(|t| t.faction.as_str());
        let origin = art.origin(&cp.name, cp.pos);
        holo_draws(
            art,
            &Holo::steady(team),
            origin,
            faction,
            1,
            camera,
            &mut out,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cp::{CommandPost, CpDef};
    use core::region::Region;

    fn post(owner: Option<u8>) -> CommandPost {
        CommandPost::new(
            CpDef {
                name: "cp1".into(),
                pos: Vec3::new(10.0, 2.0, -30.0),
                capture: Region::sphere(Vec3::ZERO, 8.0),
                control: None,
                value_bleed: 10,
                value_atk: [10; 2],
                value_def: [10; 2],
                spawn_nodes: Vec::new(),
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            owner,
        )
    }

    fn art() -> HoloArt {
        let tri = vec![
            (Vec3::new(0.2, 0.0, 0.0), [0.0, 0.0]),
            (Vec3::new(0.0, 0.3, 0.0), [1.0, 0.0]),
            (Vec3::new(0.0, 0.0, 0.1), [0.0, 1.0]),
        ];
        let mut a = HoloArt::default();
        a.icons.insert(
            "rep",
            vec![IconPart {
                texture: None,
                tris: tri,
                scroll: [0.0; 2],
            }],
        );
        a
    }

    fn cam() -> Camera {
        Camera {
            position: Vec3::new(10.0, 3.0, -10.0),
            ..Camera::default()
        }
    }

    fn draws_for(a: &HoloArt, owner: Option<u8>, viewer: u8) -> Vec<ParticleDraw> {
        let mut out = Vec::new();
        let p = post(owner);
        holo_draws(
            a,
            &Holo::steady(owner),
            p.def.pos + Vec3::Y * HEIGHT,
            Some("rep"),
            viewer,
            &cam(),
            &mut out,
        );
        out
    }

    /// The same post is blue for the view of its own team and red for the
    /// other's (BF2); BF1's own colour is green; a neutral post is grey.
    #[test]
    fn the_colour_is_the_viewers_relation() {
        let a = art();
        assert_eq!(draws_for(&a, Some(0), 0)[0].color, [1, 86, 213, 255]);
        assert_eq!(draws_for(&a, Some(0), 1)[0].color, [200, 100, 100, 255]);
        assert_eq!(draws_for(&a, None, 0)[0].color, [192, 192, 192, 255]);
        let bf1 = HoloArt { bf1: true, ..art() };
        assert_eq!(draws_for(&bf1, Some(1), 1)[0].color, [100, 200, 100, 255]);
        assert_eq!(draws_for(&bf1, Some(1), 0)[0].color, [200, 100, 100, 255]);
        // The icon takes the same colour at half the alpha.
        assert_eq!(draws_for(&a, Some(0), 1)[1].color, [200, 100, 100, 127]);
    }

    #[test]
    fn axial_flare_uses_class_factor_and_eighth_power() {
        let h = Holo::steady(Some(0));
        let c = HoloClass {
            flare_intensity: 0.5,
            ..CONTROLZONE
        };
        assert_eq!(
            flare_parameters(&h, &c, Vec3::ZERO, Vec3::Y * 10.0),
            Some((0.75, 128))
        );
        // Eye direction (0, 4, 3): g = -0.8, g^8 = 0.16777216.
        let (radius, alpha) =
            flare_parameters(&h, &c, Vec3::ZERO, Vec3::new(0.0, 4.0, 3.0)).unwrap();
        assert!((radius - (0.5 + 0.25 * 0.8_f32.powi(8))).abs() < 1e-6);
        assert_eq!(alpha, (255.0 * 0.5 * 0.8_f32.powi(8)).round() as u8);
        assert!(flare_parameters(&h, &c, Vec3::ZERO, Vec3::X * 10.0).is_none());
        assert!(flare_parameters(&h, &c, Vec3::ZERO, -Vec3::Y * 10.0).is_none());
        assert!(flare_parameters(&h, &c, Vec3::ZERO, Vec3::Y * 201.0).is_none());
        assert!(flare_parameters(
            &h,
            &HoloClass {
                flare_intensity: 0.0,
                ..c
            },
            Vec3::ZERO,
            Vec3::Y
        )
        .is_none());
    }

    #[test]
    fn above_post_draws_one_flare_and_side_draws_none() {
        let h = Holo::steady(None);
        let art = HoloArt::default();
        for (eye, pitch, want) in [
            (Vec3::Y * 10.0, -std::f32::consts::FRAC_PI_2, 1),
            (Vec3::Z * 10.0, 0.0, 0),
        ] {
            let camera = Camera {
                position: eye,
                pitch,
                ..Camera::default()
            };
            let mut out = Vec::new();
            holo_draws(&art, &h, Vec3::ZERO, None, 0, &camera, &mut out);
            let flares: Vec<_> = out
                .iter()
                .filter(|d| matches!(d.shape, ParticleShape::UntexturedShaded { .. }))
                .collect();
            assert_eq!(flares.len(), want);
            if let Some(d) = flares.first() {
                assert_eq!(d.color, [96, 96, 96, 255]);
                assert!(d.additive && d.texture.is_none());
                let ParticleShape::UntexturedShaded { tris } = &d.shape else {
                    unreachable!()
                };
                assert_eq!(tris.len(), 81);
                assert!(tris.iter().any(|v| v.2[3] == 0));
            }
        }
    }

    #[test]
    fn icon_scroll_uses_continuous_clock_not_random_turn() {
        let mut a = art();
        a.icons.get_mut("rep").unwrap()[0].scroll = [-1.0 / 255.0, 0.0];
        let h = Holo {
            clock: 127.5,
            spin: 21.0,
            ..Holo::steady(Some(0))
        };
        let mut out = Vec::new();
        holo_draws(
            &a,
            &h,
            Vec3::new(10.0, 2.0, -30.0),
            Some("rep"),
            0,
            &cam(),
            &mut out,
        );
        let ParticleShape::Mesh { tris } = &out.last().unwrap().shape else {
            panic!("icon");
        };
        assert_eq!(tris[0].1, [-0.5, 0.0]);
        assert_eq!(scroll_uv([0.25, 0.75], [0.0, -1.0], 10.5), [0.25, 0.25]);
        let c = HoloClass {
            pop_rate: 100.0,
            ..CONTROLZONE
        };
        let mut h = h;
        h.update(STEP, &c, &mut Lcg(0));
        assert_eq!(h.clock, 127.5 + STEP);
    }

    #[test]
    #[ignore = "requires a headless GPU adapter"]
    fn flare_reaches_the_headless_frame() {
        let mut renderer = Renderer::new_headless(64, 64).expect("headless GPU");
        let camera = Camera {
            position: Vec3::Y * 10.0,
            pitch: -std::f32::consts::FRAC_PI_2,
            ..Camera::default()
        };
        let mut draws = Vec::new();
        holo_draws(
            &HoloArt::default(),
            &Holo::steady(None),
            Vec3::ZERO,
            None,
            0,
            &camera,
            &mut draws,
        );
        renderer.set_particles(draws);
        renderer.render(&camera, &[], [0.0, 0.0, 0.0, 1.0]).unwrap();
        let (rgba, w, _) = renderer.capture_rgba().unwrap();
        if let Ok(path) = std::env::var("OPENBF_HOLO_CAPTURE") {
            image::save_buffer(path, &rgba, w, 64, image::ColorType::Rgba8).unwrap();
        }
        let center = ((32 * w + 32) * 4) as usize;
        assert!(
            rgba[center] > 120,
            "axial flare center: {:?}",
            &rgba[center..center + 4]
        );
        assert_eq!(&rgba[0..3], &[0, 0, 0], "flare does not wash the frame");
    }

    #[test]
    #[ignore = "requires original BF1 COMMON and BF2 INGAME data"]
    fn native_icon_scroll_survives_bake_and_runtime_load() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let tmp = std::env::temp_dir().join(format!("openbf-holo-{}.glb", std::process::id()));
        for (game, path, rates) in [
            (
                "bf1",
                "extracted/bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL",
                [[0.0, -0.9921875], [-0.0078125, -0.9921875]],
            ),
            (
                "bf2",
                "extracted/bf2/lvl/DATA/_LVL_PS2/INGAME.LVL",
                [[0.0, 0.0], [-1.0 / 255.0, 0.0]],
            ),
        ] {
            let bytes = std::fs::read(root.join(path)).expect("original LVL required");
            let chunks = mungefs::content_chunks(mungefs::open(&bytes).unwrap()).unwrap();
            let class = chunks
                .iter()
                .find(|c| c.tag == *b"entc" && c.child_str(b"TYPE") == Some("com_holo_controlzone"))
                .expect("native hologram class");
            let factor = class
                .children_tagged(b"PROP")
                .find(|p| {
                    p.payload
                        .starts_with(&mungefs::fnv1a_lowercase("HoloFlareIntensity").to_le_bytes())
                })
                .expect("authored flare factor");
            assert_eq!(&factor.payload[4..], b"1.0\0");
            assert_eq!(CONTROLZONE.flare_intensity, 1.0);
            for (_, side) in SIDES {
                let name = format!("com_icon_{side}");
                let chunk = chunks
                    .iter()
                    .find(|c| {
                        c.tag == *b"modl"
                            && c.child_str(b"NAME")
                                .is_some_and(|n| n.eq_ignore_ascii_case(&name))
                    })
                    .expect("native icon");
                let model = mungefs::model::Model::parse(chunk).unwrap();
                let count = if side == "republic" { 2 } else { 1 };
                assert_eq!(model.segments.len(), count, "{game} {name}");
                for (segment, want) in model.segments.iter().zip(rates) {
                    assert_eq!(
                        segment.uv_scroll().unwrap_or_default(),
                        want,
                        "{game} {name}"
                    );
                }
                let alphas = bake::alpha::AlphaTable::new(&[]).model(&model);
                let bytes = bake::glb::model_to_glb(&model, &alphas, false).unwrap();
                std::fs::write(&tmp, bytes).unwrap();
                let cpu = crate::assets::load_glb(&tmp).unwrap();
                assert_eq!(
                    cpu.scroll,
                    rates[..count],
                    "{game} {name}: baked/runtime rates"
                );
            }
        }
        std::fs::remove_file(tmp).unwrap();
    }

    /// The beam stands on the hologram's point, 3.5 m, 0.5 → 1.0 m, additive
    /// and without a sheet; an owned post adds its side's icon, source-over,
    /// and a neutral post has none.
    #[test]
    fn a_neutral_post_has_a_beam_and_no_icon() {
        let a = art();
        let owned = draws_for(&a, Some(0), 0);
        assert_eq!(owned.len(), 2);
        let ParticleShape::Beam(b) = &owned[0].shape else {
            panic!("the beam first");
        };
        assert_eq!(b.base, Vec3::new(10.0, 2.32, -30.0));
        assert_eq!((b.length, b.r0, b.r1, b.soft), (3.5, 0.5, 1.0, 0.7));
        assert_eq!(b.gs_rgb, 1.0, "BF2 modulates a white sheet");
        let bf1 = HoloArt { bf1: true, ..art() };
        let ParticleShape::Beam(b1) = &draws_for(&bf1, Some(0), 0)[0].shape else {
            panic!("the beam first");
        };
        assert_eq!(b1.gs_rgb, 0.5, "BF1 draws it untextured");
        assert!(owned[0].additive && owned[0].texture.is_none());
        assert!(!owned[1].additive);
        assert!(matches!(owned[1].shape, ParticleShape::Mesh { .. }));
        let neutral = draws_for(&a, None, 0);
        assert_eq!(neutral.len(), 1);
        assert!(matches!(neutral[0].shape, ParticleShape::Beam(_)));
    }

    /// The icon stands `HoloHeight` over the hologram's point at
    /// `HoloSize`, and is back where it was after 5 s (one turn).
    #[test]
    fn the_icon_turns_once_in_five_seconds() {
        let c = &CONTROLZONE;
        let mut h = Holo::steady(Some(0));
        let p = Vec3::new(0.2, 0.0, 0.0);
        let at = |h: &Holo| icon_matrix(h, c, Vec3::ZERO).transform_point3(p);
        assert!(
            at(&h).abs_diff_eq(Vec3::new(0.5, 3.5, 0.0), 1e-5),
            "{}",
            at(&h)
        );
        h.spin = 1.25;
        assert!(
            at(&h).abs_diff_eq(Vec3::new(0.0, 3.5, 0.5), 1e-5),
            "{}",
            at(&h)
        );
        h.spin = 5.0;
        assert!(
            at(&h).abs_diff_eq(Vec3::new(0.5, 3.5, 0.0), 1e-4),
            "{}",
            at(&h)
        );
        // A pop of 2: twice as wide, half as high.
        h.pop = 2.0;
        let q = icon_matrix(&h, c, Vec3::ZERO).transform_point3(Vec3::new(0.2, 0.2, 0.0));
        assert!(q.abs_diff_eq(Vec3::new(1.0, 3.75, 0.0), 1e-4), "{q}");
    }

    /// Across a capture: the hologram goes off 2.2 s before the meter's end,
    /// fades for 2 s, is dark at the flip, then shows the new owner and is
    /// back at full 2 s later. (No pops or dips: both rates 0.)
    #[test]
    fn the_hologram_fades_out_and_in_across_a_capture() {
        let c = HoloClass {
            pop_rate: 0.0,
            flicker_rate: 0.0,
            ..CONTROLZONE
        };
        let mut rng = Lcg(1);
        let mut cp = post(None);
        cp.capture_team = Some(1);
        let mut h = Holo::steady(None);
        let run = |h: &mut Holo, cp: &CommandPost, secs: f32, rng: &mut Lcg| {
            for _ in 0..(secs / STEP).round() as u32 {
                step(h, cp, &c, rng);
            }
        };
        cp.capture = 7.7;
        run(&mut h, &cp, 1.0, &mut rng);
        assert!(h.on && h.intensity == 1.0, "still on at 7.7 of 10: {h:?}");
        cp.capture = 7.9;
        run(&mut h, &cp, 1.0, &mut rng);
        assert!(!h.on && (h.intensity - 0.5).abs() < 0.02, "mid fade: {h:?}");
        run(&mut h, &cp, 1.1, &mut rng);
        assert_eq!((h.intensity, h.team), (0.0, None), "dark before the flip");
        // The flip: the port's capture sets the owner and zeroes the meters.
        cp.owner = Some(1);
        cp.capture = 0.0;
        run(&mut h, &cp, STEP, &mut rng);
        assert_eq!((h.team, h.on), (Some(1), true));
        assert!(h.intensity < 0.05, "at the flip: {h:?}");
        run(&mut h, &cp, 1.0, &mut rng);
        assert!((h.intensity - 0.5).abs() < 0.03, "fading in: {h:?}");
        run(&mut h, &cp, 1.0, &mut rng);
        assert_eq!(h.intensity, 1.0, "2 s after the flip");
        // An attacker who leaves in mid-fade: the fade turns round where it is.
        cp.neutral = 10.0;
        run(&mut h, &cp, 0.5, &mut rng);
        assert!(!h.on && (h.intensity - 0.75).abs() < 0.02, "{h:?}");
        cp.neutral = 9.0;
        run(&mut h, &cp, 0.25, &mut rng);
        assert!(h.on && (h.intensity - 0.875).abs() < 0.03, "{h:?}");
    }

    /// The random stream is the PS2's (two steps a draw, 23 bits), the dip
    /// stays inside U[0.4, 0.8] of the fade and lasts one update, the pop
    /// stays inside [0.5, 2) and eases back toward 1.
    #[test]
    fn dips_and_pops_stay_in_their_ranges() {
        let mut r = Lcg(0);
        // 0 → 0x3c6ef35f → 0x47502932: (0x3c6e | 0x47500000) >> 9.
        let u = r.unit();
        assert_eq!(r.0, 0x4750_2932);
        assert_eq!(
            u,
            f32::from_bits((0x4750_3c6e_u32 >> 9) | 0x3f80_0000) - 1.0
        );
        let c = CONTROLZONE;
        let mut h = Holo::steady(Some(0));
        let (mut dips, mut pops, mut run) = (0, 0, 0);
        for _ in 0..30 * 600 {
            let before = h.pop;
            h.update(STEP, &c, &mut r);
            if h.intensity < 1.0 {
                dips += 1;
                run += 1;
                assert!(run == 1, "a dip lasts one update");
                assert!((0.4..0.8).contains(&h.intensity), "{}", h.intensity);
            } else {
                run = 0;
            }
            if (h.pop - 1.0).abs() > (before - 1.0).abs() + 1e-6 {
                pops += 1;
                assert!((0.5..2.0).contains(&h.pop), "{}", h.pop);
            }
            assert!((0.0..=std::f32::consts::FRAC_PI_6).contains(&h.tilt));
        }
        // Hazard t × 0.005 an update: a dip or a pop about every 3.2 s.
        assert!((130..250).contains(&dips), "{dips} dips in 10 min");
        assert!((90..230).contains(&pops), "{pops} pops in 10 min");
    }

    #[test]
    fn the_large_post_carries_its_hologram_higher() {
        let h = bone_height;
        assert_eq!(
            h("com_bldg_controlzone", Some("com_bldg_controlzone"), false),
            0.32
        );
        assert_eq!(h("x", Some("com_bldg_controlzone_large"), false), 1.363);
        // The common classes are not in a map's own table: BF1's major post
        // is the large model, BF2's the ordinary one.
        assert_eq!(h("com_bldg_major_controlzone", None, true), 1.363);
        assert_eq!(h("com_bldg_major_controlzone", None, false), 0.32);
        assert_eq!(h("end_bldg_bunker_controlpanel", None, true), 0.67);
        assert_eq!(h("com_bldg_controlzone", None, true), 0.32);
    }
}
