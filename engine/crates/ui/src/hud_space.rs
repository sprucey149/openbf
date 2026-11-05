use crate::hud_draw::{color, Ctx, Quad, Screen};
use crate::hud_tree::Elem;
use glam::{Mat4, Vec3};
use std::path::PathBuf;

pub const GROUP: &str = "player1spaceassaultgroup";
const SYSTEM_SIZE: f32 = 0.15;

#[derive(Debug, Clone, PartialEq)]
pub struct SpaceSystem {
    pub own: bool,
    pub u: f32,
    pub v: f32,
    pub rgb: [f32; 3],
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpaceStatus {
    pub sheet_dir: Option<PathBuf>,
    pub ship: [String; 2],
    pub shield: [String; 2],
    pub ship_rgb: [[f32; 3]; 2],
    pub shield_rgba: [[f32; 4]; 2],
    pub system: String,
    pub systems: Vec<SpaceSystem>,
}

impl SpaceStatus {
    pub fn textures(&self) -> impl Iterator<Item = &str> {
        self.ship
            .iter()
            .chain(&self.shield)
            .chain(std::iter::once(&self.system))
            .map(String::as_str)
    }
}

impl Ctx<'_> {
    pub(crate) fn space_status(&mut self, status: &SpaceStatus, e: &Elem, i: usize, s: &Screen) {
        let (w, h) = (e.rect.w, e.rect.h);
        let half = w * 0.5;
        let m: Mat4 = e.local();
        let picture = |ctx: &mut Self, x: f32, texture: &str, rgba: [f32; 4]| {
            ctx.quad(
                i,
                &m,
                s,
                &Quad {
                    rect: [x, 0.0, x + half, h],
                    uv: [0.0, 0.0, 1.0, 1.0],
                    color: rgba,
                    texture,
                    additive: false,
                    shadow: false,
                    boxed: false,
                },
            );
        };
        for (k, x) in [half, 0.0].into_iter().enumerate() {
            let rgb = status.ship_rgb[k];
            picture(self, x, &status.ship[k], color(rgb, 1.0));
        }
        for (k, x) in [half, 0.0].into_iter().enumerate() {
            let [r, g, b, a] = status.shield_rgba[k];
            picture(self, x, &status.shield[k], color([r, g, b], a / 255.0));
        }
        let (sw, sh) = (w * SYSTEM_SIZE, h * SYSTEM_SIZE);
        for system in &status.systems {
            let x = if system.own { half } else { 0.0 } + half * system.u;
            let y = h * system.v;
            let at = m * Mat4::from_translation(Vec3::new(x, y, 0.0));
            self.quad(
                i,
                &at,
                s,
                &Quad {
                    rect: [-sw * 0.5, -sh * 0.5, sw * 0.5, sh * 0.5],
                    uv: [0.0, 0.0, 1.0, 1.0],
                    color: color(system.rgb, 1.0),
                    texture: &status.system,
                    additive: false,
                    shadow: false,
                    boxed: false,
                },
            );
        }
    }
}
