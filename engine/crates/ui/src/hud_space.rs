use crate::hud::HudTarget;
use crate::hud_draw::{color, Ctx, Quad, Screen};
use crate::hud_tree::{Elem, EventValue, Hud};
use glam::{Mat4, Vec3};
use std::path::PathBuf;

pub const GROUP: &str = "player1spaceassaultgroup";
const SYSTEM_SIZE: f32 = 0.15;

#[derive(Debug, Clone)]
pub struct HudLock {
    pub target: HudTarget,
    pub distance: f32,
    pub direction: Option<f32>,
    pub ndc: [f32; 2],
    pub on_screen: bool,
    pub visible: bool,
    pub ready: [bool; 2],
    pub flyer: bool,
    pub pulse: f32,
}

pub fn feed_lock(
    hud: &mut Hud,
    lock: Option<&HudLock>,
    m_space: Option<SpaceStatus>,
    has_mesh: &dyn Fn(&str) -> bool,
) {
    const ID: &str = "hud.lockOn.id";
    const WEAPON: [(&str, &str); 2] = [
        (
            "player1.weapon1.lockOnPosition",
            "player1.weapon1.lockOnDisable",
        ),
        (
            "player1.weapon2.lockOnPosition",
            "player1.weapon2.lockOnDisable",
        ),
    ];
    let id = EventValue::Text(lock.map(|l| l.target.id.to_string()).unwrap_or_default());
    if hud.last(ID) != Some(&id) {
        let first = hud.last(ID).is_none();
        hud.publish(ID, id, has_mesh);
        if !first {
            hud.send("player1.lockOnDisable", EventValue::None, has_mesh);
            hud.send("player1.lockOnDirectionDisable", EventValue::None, has_mesh);
        }
        for e in [
            "player1.lockOnClassName",
            "player1.lockOnHealthFraction",
            "player1.lockOnTeamColor",
            "player1.lockOnDirection",
        ] {
            hud.forget(e);
        }
    }
    hud.space = m_space;
    hud.lock_marker = lock.map(|l| crate::hud_tree::LockMarker {
        pulse: l.pulse,
        ndc: l.ndc,
        on_screen: l.on_screen,
        visible: l.visible,
        distance: l.distance,
        flyer: l.flyer,
    });
    for (k, (position, disable)) in WEAPON.iter().enumerate() {
        let shown = lock.filter(|l| l.ready[k] && l.on_screen);
        match shown {
            Some(l) => hud.publish(
                position,
                EventValue::Position([l.ndc[0] * 0.5 + 0.5, l.ndc[1] * 0.5 + 0.5]),
                has_mesh,
            ),
            None => {
                if hud.last(position).is_some() {
                    hud.forget(position);
                    hud.send(disable, EventValue::None, has_mesh);
                }
            }
        }
    }
    let Some(l) = lock else {
        return;
    };
    let mut put = |event: &str, v: EventValue| hud.publish(event, v, has_mesh);
    put(
        "player1.lockOnClassName",
        EventValue::Text(l.target.name.clone()),
    );
    put(
        "player1.lockOnTeamColor",
        EventValue::Color(l.target.color()),
    );
    put(
        "player1.lockOnHealthFraction",
        EventValue::Number(l.target.health_fraction.clamp(0.0, 1.0)),
    );
    put("player1.lockOnDistance", EventValue::Number(l.distance));
    match l.direction {
        Some(z) => put("player1.lockOnDirection", EventValue::Number(z)),
        None => {
            if hud.last("player1.lockOnDirection").is_some() {
                hud.forget("player1.lockOnDirection");
                hud.send("player1.lockOnDirectionDisable", EventValue::None, has_mesh);
            }
        }
    }
}

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
