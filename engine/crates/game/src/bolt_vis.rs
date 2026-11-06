//! Laser bolts as the PS2 draws them (card FXB, `analysis/re-bolt.md` §3–§4,
//! [O]): each in-flight bolt of a `bolt` / `laser` class becomes one
//! [`render::BoltDraw`] — its two ribbons' sizes, lengths and colours from
//! the class's [`LaserLook`]. Visual only: it reads the sim, never writes it.
//!
//! - **Length** (§4): the drawn length grows from 0 at the spawn point with
//!   the distance flown, up to the class cap max(LaserLength, GlowLength,
//!   BlurLength); the core is drawn over min(len, LaserLength), the glow
//!   over min(len, GlowLength).
//! - **After reaching the end** (§4): the PS2 keeps the bolt at its impact point for
//!   two more updates; a class with `FadeOutTime` (the snipers) instead
//!   fades in place — w → 0.1·LaserWidth, the core's alpha and the glow's
//!   bytes × the fade. The sim drops a bolt on its impact tick, so the port
//!   keeps these remnants here on impact and air expiry alike (BF1
//!   `FUN_002764c0`, B1:197673–197677; `FUN_0027c478`, B1:202176–202232).
//! - **Colours** (§3.1, §3.5): the core white × the fade; the glow
//!   `LaserGlowColor`, BF2's fade toward `EndLaserGlowColor` by seconds of
//!   flight, × the fade. BF2's `BonusColor` core tint (the
//!   `team_bonus_advanced_blasters` bonus) is not modelled: every live bolt
//!   in the RAM was white.
//! - **Where it ends**: at the sim's impact - the press's foreseen end for
//!   the map and the ground, else the `Impact` the tick's sweep made on a
//!   soldier, a hull or a mine (the bolt vanishes from `sim.bolts` on that
//!   tick, so its last stretch is the remnant's).
//! - **The light** (BF2 only, re-bolt §5.2 [O], card FXBL): a class with
//!   `LightRadius` and a `LightColor` gives each bolt an omni light at the
//!   midpoint of its last step (half a sim tick behind the drawn head), its
//!   colour fading to `EndLightColor` with the glow. The update whose move
//!   reaches the impact leaves it at the midpoint of that last step; the
//!   next (the hit) moves it to hit + 0.5·normal (the terrain's normal; the
//!   reverse of the flight for any other surface, PROVISIONAL — the sim
//!   keeps no hit normal) for one update, or for as long as a `FadeOutTime`
//!   remnant lives. The sim drops the bolt on the reaching tick, so both
//!   steps are the remnant's first two updates. Same-tick hits retain the
//!   matching impact event's surface identity. The lights go to the
//!   terrain through [`crate::bolt_light`]. BF1 bolts make none.
//! - **Textures**: the class's `LaserTexture` and `lightglow` from the
//!   common layer (`common_bf1` / `common_bf2`). BF1's remote droid names
//!   `purplelaser_D`, which no disc holds (the PS2 binds a fallback and
//!   draws VRAM garbage): the port draws `violetlaser_D` — PROVISIONAL, the
//!   intended art.
//! - **Missiles** (card UORD) are their own draw: a `missile` class has no
//!   `LaserLook` (`roster::laser_look` only replays `bolt` / `laser`), so a
//!   round of one is drawn as its `ordnance.geometry` model by
//!   `SoldierVisuals::weapon_draws` — [`missiles_in_flight`] is that path's
//!   read of `sim.bolts`. This module keeps no ribbon and no light for one,
//!   but its [`key`] is stable under steering all the same: the trail
//!   (`crate::fx::trail`) and the hit remnants identify rounds by it.

#[cfg(test)]
mod expiry_tests;
mod shots;

use crate::bolt_light::OmniLight;
use crate::walk::WalkSim;
use core::laser::LaserLook;
use core::projectile::Hit;
use glam::Vec3;
use render::{BoltDraw, Renderer, TextureHandle};
use shots::Shots;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

/// Updates a hit bolt stays drawn at its impact point (re-bolt §4).
const HIT_UPDATES: f32 = 2.0;

/// The glow's texture (fnv 0x716bd81a, re-bolt §3.1 step 5).
pub const GLOW_TEXTURE: &str = "lightglow";

/// Where a soldier drawn WITHOUT a model fires from, relative to the sim's
/// `Shot` origin (a bot's chest, `Unit::muzzle()`): PROVISIONAL, forward a rifle
/// length and out to the right. A modelled soldier fires from his weapon's
/// real fire point.
pub const MUZZLE_FORWARD: f32 = 1.2;
pub const MUZZLE_RIGHT: f32 = 0.3;

/// Flight signature and stable producer identity, shared with trail emitters.
pub(crate) type Key = (
    u32,
    u32,
    usize,
    [u32; 3],
    [u32; 3],
    Option<core::projectile::VisualShot>,
);

/// The round's identity across frames (see [`Key`]). A straight bolt keeps
/// its end point and its velocity for its whole flight; a missile steers, so
/// both are new every tick and it is known instead by what its launch fixed:
/// the line it left on and the two waver phases the class ctor drew for it
/// (`launch_phases`; the live ones advance every tick of waver, #133)
/// (`Missile::launch`, card UORD — an index-free id no other round shares,
/// and the same fields its replay state carries). Two rounds of one class
/// from one launcher therefore stay two rounds, however they curve.
pub(crate) fn key(b: &core::projectile::Bolt) -> Key {
    let bits = |v: Vec3| v.to_array().map(f32::to_bits);
    let (end, vel) = match &b.missile {
        Some(m) => (
            bits(m.launch),
            [
                m.launch_phases[0].to_bits(),
                m.launch_phases[1].to_bits(),
                m.top.to_bits(),
            ],
        ),
        None => (bits(b.end), bits(b.vel)),
    };
    (
        b.owner.index,
        b.owner.gen,
        b.weapon_class,
        end,
        vel,
        b.visual_shot,
    )
}

/// Whether the round draws as its `ordnance.geometry` model instead of a
/// ribbon: a missile (card UORD; the model path is
/// `SoldierVisuals::weapon_draws`, fed by [`missiles_in_flight`]). A missile
/// never has a `LaserLook` off the discs — `roster::laser_look` replays
/// `bolt` / `laser` alone — but the model is the preferred draw all the same
/// (the task's order: geometry, then tracer, then nothing).
fn modelled(walk: &WalkSim, b: &core::projectile::Bolt) -> bool {
    walk.sim
        .weapon_classes
        .get(b.weapon_class)
        .is_some_and(|w| w.ordnance.geometry.is_some())
}

/// The missiles in flight this frame as (their weapon class, the model's
/// transform): each at its sim place, the model's +Z (`Quat::from_rotation_arc`,
/// the engine's forward) along the way it flies. `SoldierVisuals::weapon_draws`
/// draws them with the other ordnance models — the `ordnance.geometry` its
/// table loads for every missile class. A round with no velocity (a stopped
/// or degenerate one) is not drawn rather than drawn at a random angle.
pub fn missiles_in_flight(walk: &WalkSim) -> Vec<(usize, glam::Mat4)> {
    walk.sim
        .bolts
        .iter()
        .filter(|b| modelled(walk, b))
        .filter_map(|b| {
            let dir = b.vel.try_normalize()?;
            Some((
                b.weapon_class,
                glam::Mat4::from_rotation_translation(
                    glam::Quat::from_rotation_arc(Vec3::Z, dir),
                    b.pos,
                ),
            ))
        })
        .collect()
}

#[derive(Debug, Clone)]
struct Tracked {
    key: Key,
    class: usize,
    /// The drawn line: from the fire point to the sim's end point.
    start: Vec3,
    end: Vec3,
    head: Vec3,
    /// The head the tick before (its start on the bolt's first tick): a
    /// frame between ticks draws the head between the two (card RINT).
    prev_head: Vec3,
    dir: Vec3,
    age: f32,
    struck: bool,
    remaining: f32,
    /// Where the spawn ray ends on the heightfield.
    terrain: bool,
    /// Metres the sim moves it per tick.
    step: f32,
    /// The sim's bolt: where it is and the way it flies (its next tick's
    /// sweep starts there).
    pos: Vec3,
    flight: Vec3,
}

#[derive(Debug, Clone)]
struct Remnant {
    class: usize,
    head: Vec3,
    dir: Vec3,
    len: f32,
    /// Seconds of flight, frozen at the hit (BF2's End fade reads it).
    age: f32,
    fade: f32,
    timer: f32,
    /// The impact was on the heightfield (the light's normal).
    terrain: bool,
    /// The head before its last step (the light's midpoint on the reach).
    prev: Vec3,
}

/// A bolt's head drawn `t` of the way from the last tick's head to the
/// newest (card RINT; `t` = 1 draws the tick's own).
pub fn drawn_head(prev: Vec3, head: Vec3, t: f32) -> Vec3 {
    prev.lerp(head, t)
}

/// The drawn start of a bolt spawned at `spawn` flying along `dir` toward
/// `end`: the fire point when the shooter is on foot — `fire`, the drawn
/// weapon's, else the PROVISIONAL offset (see the module doc) — unless that
/// would already be past the end.
pub fn drawn_start(spawn: Vec3, dir: Vec3, end: Vec3, on_foot: bool, fire: Option<Vec3>) -> Vec3 {
    if !on_foot {
        return spawn;
    }
    let s = fire.unwrap_or_else(|| {
        spawn + dir * MUZZLE_FORWARD + dir.cross(Vec3::Y).normalize_or_zero() * MUZZLE_RIGHT
    });
    if (end - s).dot(dir) > 0.0 {
        s
    } else {
        spawn
    }
}

pub fn drawn_from_the_gun(u: &core::unit::Unit) -> bool {
    u.mount.is_none() && !u.eye_view()
}

/// The tick's struck impacts (`Event::Impact`: one per bolt that struck a
/// soldier, a hull, a mine, a droid, a building or the ground), each taken
/// by at most one drawn bolt: where, the flight's direction, the class, on
/// the heightfield.
struct Struck(Vec<(Vec3, Vec3, usize, bool)>);

impl Struck {
    fn of(sim: &core::sim::Sim) -> Self {
        Self(
            sim.events
                .iter()
                .filter_map(|e| match *e {
                    core::sim::Event::Impact {
                        pos,
                        dir,
                        weapon_class,
                        surface,
                        ..
                    } => Some((
                        pos,
                        dir,
                        weapon_class as usize,
                        surface == core::impact::Surface::Terrain,
                    )),
                    _ => None,
                })
                .collect(),
        )
    }

    /// Match direction and ray before distance within this tick's step.
    /// The nearest impact wins only among otherwise identical flight rays.
    fn take_on(&mut self, from: Vec3, dir: Vec3, step: f32, class: usize) -> Option<(Vec3, bool)> {
        self.take_to(from, dir, step, class, None)
    }

    fn take_to(
        &mut self,
        from: Vec3,
        dir: Vec3,
        step: f32,
        class: usize,
        end: Option<Vec3>,
    ) -> Option<(Vec3, bool)> {
        let k = self
            .0
            .iter()
            .enumerate()
            .filter(|(_, &(p, d, c, _))| {
                let t = (p - from).dot(dir);
                c == class
                    && match end {
                        // Native provenance already owns this endpoint. Gravity
                        // or charged speed can change the first-step flight ray.
                        Some(end) => p.abs_diff_eq(end, 1e-3),
                        None => {
                            d.abs_diff_eq(dir, 1e-4)
                                && (-1e-3..=step + 1e-3).contains(&t)
                                && (p - (from + dir * t)).length_squared() < 1e-3
                        }
                    }
            })
            .min_by(|a, b| {
                let error = |&(p, d, _, _): &(Vec3, Vec3, usize, bool)| {
                    let t = (p - from).dot(dir);
                    (
                        d.distance_squared(dir),
                        (p - (from + dir * t)).length_squared(),
                        t,
                    )
                };
                let (ad, ar, at) = error(a.1);
                let (bd, br, bt) = error(b.1);
                ad.total_cmp(&bd)
                    .then(ar.total_cmp(&br))
                    .then(at.total_cmp(&bt))
            })
            .map(|(k, _)| k)?;
        let (p, _, _, terrain) = self.0.remove(k);
        Some((p, terrain))
    }
}

/// One bolt's draw from its look, head, direction, drawn length, seconds
/// of flight and fade (re-bolt §3.1).
pub fn bolt_draw(
    look: &LaserLook,
    head: Vec3,
    dir: Vec3,
    len: f32,
    age: f32,
    fade: f32,
    textures: [Option<TextureHandle>; 2],
) -> BoltDraw {
    let len = len.min(look.cap()).max(0.0);
    BoltDraw {
        head,
        dir,
        core_len: len.min(look.length),
        glow_len: len.min(look.glow_length),
        width: look.half_width(fade),
        core_texture: textures[0],
        glow_texture: textures[1],
        core_color: [255, 255, 255, (fade.clamp(0.0, 1.0) * 255.0) as u8],
        glow_color: look.glow_bytes(age, fade),
    }
}

pub fn beam_draw(
    look: &LaserLook,
    beam: &core::sim::Beam,
    textures: [Option<TextureHandle>; 2],
) -> BoltDraw {
    let len = beam.len + 1.0;
    let look = LaserLook {
        length: len,
        glow_length: len,
        ..look.clone()
    };
    let head = beam.origin + beam.dir * (beam.len + 0.5);
    bolt_draw(&look, head, beam.dir, len, 0.0, 1.0, textures)
}

/// The texture a `LaserTexture` name draws with (lower-cased, as baked).
pub fn texture_name(authored: &str) -> String {
    authored
        .trim()
        .to_ascii_lowercase()
        .replace("purplelaser", "violetlaser")
}

/// The game's bolts between frames: the in-flight ones it has seen and the
/// hit ones still drawn.
#[derive(Default)]
pub struct BoltVis {
    /// BF2: bolts light the terrain (re-bolt §5.2).
    lights: bool,
    dirs: Vec<PathBuf>,
    textures: HashMap<String, Option<TextureHandle>>,
    tracked: Vec<Tracked>,
    remnants: Vec<Remnant>,
    /// The drawn soldiers' weapon fire points, by unit index
    /// ([`BoltVis::set_fire_points`]).
    fire_points: Vec<Option<Vec3>>,
}

fn visual_look(walk: &WalkSim, class: usize) -> Option<&LaserLook> {
    crate::walker_shots::style(walk, class)
        .and_then(|s| s.tracer.as_ref())
        .or_else(|| walk.sim.weapon_classes.get(class)?.ordnance.laser.as_ref())
}

impl BoltVis {
    /// Textures come from `assets_dir`'s common layers, the map's game's
    /// first.
    pub fn new(assets_dir: &Path, source_game: &str) -> Self {
        let first = if source_game.eq_ignore_ascii_case("bf2") {
            ["common_bf2", "common_bf1"]
        } else {
            ["common_bf1", "common_bf2"]
        };
        Self {
            lights: source_game.eq_ignore_ascii_case("bf2"),
            dirs: first
                .iter()
                .map(|c| assets_dir.join(c).join("textures"))
                .collect(),
            ..Self::default()
        }
    }

    pub fn clear(&mut self) {
        self.tracked.clear();
        self.remnants.clear();
    }

    /// The drawn soldiers' weapon fire points, by unit index
    /// (`SoldierVisuals::fire_points`): where the next [`BoltVis::update`]
    /// starts a new soldier bolt's drawn line.
    pub fn set_fire_points(&mut self, points: Vec<Option<Vec3>>) {
        self.fire_points = points;
    }

    /// How many hit bolts are still drawn at their impact.
    pub fn remnant_count(&self) -> usize {
        self.remnants.len()
    }

    /// Every drawn bolt's line as (where it starts, its head): the ones in
    /// flight at the last tick's head, then the hit ones at their impact.
    pub fn lines(&self) -> Vec<(Vec3, Vec3)> {
        self.tracked
            .iter()
            .map(|t| (t.start, t.head))
            .chain(
                self.remnants
                    .iter()
                    .map(|m| (m.head - m.dir * m.len, m.head)),
            )
            .collect()
    }

    fn fire_point(&self, unit: u32) -> Option<Vec3> {
        self.fire_points.get(unit as usize).copied().flatten()
    }

    /// Follow the sim's bolts after `dt` seconds of play: one tick's worth
    /// ([`BoltVis::age`], then [`BoltVis::track`]). A frame loop that steps
    /// no tick or several calls the two itself — `age` once a frame, `track`
    /// once a tick (`WalkSim::advance_with`).
    pub fn update(&mut self, walk: &WalkSim, dt: f32) {
        self.age(walk, dt);
        self.track(walk);
    }

    /// `dt` seconds pass for the hit bolts still drawn.
    pub fn age(&mut self, walk: &WalkSim, dt: f32) {
        let look = |c: usize| visual_look(walk, c);
        for r in &mut self.remnants {
            r.timer += dt;
            if let Some(l) = look(r.class) {
                if l.fade_out_rate > 0.0 {
                    r.fade -= dt * l.fade_out_rate;
                }
            }
        }
        let hit_time = HIT_UPDATES * core::TICK_DT;
        self.remnants.retain(|r| match look(r.class) {
            Some(l) if l.fade_out_rate > 0.0 => r.fade > 0.0,
            Some(_) => r.timer < hit_time,
            None => false,
        });
    }

    /// Read the tick the sim just stepped: its bolts in flight, and the hits
    /// that become remnants. Once per tick — a second read of the same tick
    /// would keep each same-tick hit twice.
    pub fn track(&mut self, walk: &WalkSim) {
        let sim = &walk.sim;
        let look = |c: usize| visual_look(walk, c);
        let mut old: HashMap<_, VecDeque<Tracked>> = HashMap::new();
        for t in self.tracked.drain(..) {
            let velocity = Vec3::from_array(t.key.4.map(f32::from_bits));
            let pos = t.pos + velocity * core::TICK_DT;
            old.entry((
                t.key,
                (t.age + core::TICK_DT).to_bits(),
                pos.to_array().map(f32::to_bits),
            ))
            .or_default()
            .push_back(t);
        }
        let mut struck = Struck::of(sim);
        let shots = Shots::of(walk);
        for (bolt_index, b) in sim.bolts.iter().enumerate() {
            // A missile with a model is `SoldierVisuals`' (card UORD), and a
            // look-less round has no ribbon: neither is this module's.
            if look(b.weapon_class).is_none()
                || modelled(walk, b)
                || crate::walker_shots::style(walk, b.weapon_class)
                    .is_some_and(|s| s.tracer.is_some())
            {
                continue;
            }
            let k = key(b);
            let spawn = b.pos - b.vel * b.age;
            let old_t = old
                .get_mut(&(k, b.age.to_bits(), b.pos.to_array().map(f32::to_bits)))
                .and_then(VecDeque::pop_front);
            let start = match old_t.as_ref() {
                Some(t) => t.start,
                None => {
                    let dir = b.vel.normalize_or_zero();
                    let foot = sim.unit(b.owner).is_some_and(|u| {
                        self.uses_drawn_muzzle(sim, u, b.owner.index, b.weapon_class)
                    });
                    let point = shots
                        .for_bolt(bolt_index)
                        .and_then(|s| s.muzzle(walk))
                        .or(self.fire_point(b.owner.index));
                    drawn_start(spawn, dir, b.end, foot, point)
                }
            };
            let full = (b.end - spawn).length();
            let frac = if full > 0.0 {
                ((b.pos - spawn).length() / full).min(1.0)
            } else {
                1.0
            };
            let head = start + (b.end - start) * frac;
            let dir = (b.end - start)
                .try_normalize()
                .unwrap_or(b.vel.normalize_or_zero());
            self.tracked.push(Tracked {
                key: k,
                class: b.weapon_class,
                start,
                end: b.end,
                head,
                prev_head: old_t.as_ref().map_or(start, |t| t.head),
                dir,
                age: b.age,
                struck: !matches!(b.hit, Hit::None),
                remaining: b.remaining,
                terrain: matches!(b.hit, Hit::Terrain),
                step: b.vel.length() * core::TICK_DT,
                pos: b.pos,
                flight: b.vel.normalize_or_zero(),
            });
        }
        // A bolt gone this tick: its remnant at the impact or air expiry. The
        // map and the terrain were known at the press (`t.end`); a soldier,
        // a hull or a mine is met by the tick's sweep (`Sim::sweep_bolts`,
        // issue #41), which moves the end and lands it the same tick - its
        // `Impact` on the bolt's line within its last step says where.
        let mut gone: Vec<Tracked> = old.into_values().flatten().collect();
        gone.sort_by_key(|t| (t.key, t.age.to_bits(), t.pos.to_array().map(f32::to_bits)));
        for t in gone {
            // The nearest impact on its last step: a soldier the sweep met
            // short of a foreseen wall is the one it struck.
            let hit = struck
                .take_on(t.pos, t.flight, t.step, t.class)
                .or_else(|| t.struck.then_some((t.end, t.terrain)))
                .or_else(|| {
                    // A missing round alone is not expiry: require its final
                    // clipped step to reach the authored end this tick.
                    let end =
                        t.pos + t.flight * t.step * (t.remaining / core::TICK_DT).clamp(0.0, 1.0);
                    let expired = sim.events.iter().any(|e| matches!(e,
                        core::sim::Event::BoltExpired { pos, struck: false } if pos.abs_diff_eq(t.end, 1e-3)));
                    (expired && t.remaining <= core::TICK_DT && end.abs_diff_eq(t.end, 1e-3))
                        .then_some((t.end, false))
                });
            let Some((end, terrain)) = hit else {
                continue;
            };
            self.remnants.push(Remnant {
                class: t.class,
                head: end,
                dir: (end - t.start).try_normalize().unwrap_or(t.dir),
                len: (end - t.start).length(),
                age: t.age,
                fade: 1.0,
                timer: 0.0,
                terrain,
                prev: t.head,
            });
        }
        self.same_tick_hits(walk, &shots, &mut struck);
        self.walker_tracers(walk, &shots, &mut struck);
    }

    /// The tick's bolts that the sim spawned and resolved inside the one
    /// tick — a shot whose end lies within its first step, e.g. at the
    /// ground a few metres ahead, or a sniper's at a soldier inside 66 m
    /// (2000 m/s) — never reach `sim.bolts`. The PS2 draws such a bolt at
    /// its impact like any other (re-bolt §4) and, on BF2, lights the ground
    /// there (§5.2): each `Shot` of the tick with a struck `Impact` on its
    /// ray within one step becomes a remnant.
    fn walker_tracers(&mut self, walk: &WalkSim, shots: &Shots, struck: &mut Struck) {
        for shot in &shots.events {
            let class = shot.class;
            // Geometry owns the shot's draw, including walker cannons that
            // also author a tracer. Match the ordinary bolt path's priority.
            if walk.sim.weapon_classes[class].ordnance.geometry.is_some() {
                continue;
            }
            let Some(style) =
                crate::walker_shots::style(walk, class).filter(|s| s.tracer.is_some())
            else {
                continue;
            };
            let ordinal = crate::walker_shots::ordinal(walk, shot.unit, class, shot.event);
            if !crate::walker_shots::visible(walk, class, ordinal) {
                continue;
            }
            let start = shot.muzzle(walk).unwrap_or(shot.origin);
            let impact = if shot.bolt.is_none() {
                let step = walk.sim.weapon_classes[class].ordnance.velocity * core::TICK_DT;
                struck.take_to(
                    shot.origin,
                    shot.dir,
                    step,
                    class,
                    shot.flight.map(|f| f.end),
                )
            } else {
                None
            };
            let end = shot
                .flight
                .map(|f| f.end)
                .or_else(|| shot.bolt.map(|i| walk.sim.bolts[i].end))
                .or_else(|| impact.map(|h| h.0))
                .unwrap_or(shot.origin + shot.dir * style.tracer.as_ref().unwrap().length);
            let delta = end - start;
            let direction = delta.try_normalize().unwrap_or(shot.dir);
            let len = delta.length().min(style.tracer.as_ref().unwrap().length);
            self.remnants.push(Remnant {
                class,
                head: start + direction * len,
                dir: direction,
                len,
                age: 0.0,
                fade: 1.0,
                timer: 0.0,
                terrain: false,
                prev: start,
            });
        }
    }

    fn same_tick_hits(&mut self, walk: &WalkSim, shots: &Shots, struck: &mut Struck) {
        let sim = &walk.sim;
        for shot in &shots.events {
            if shot.bolt.is_some() {
                continue;
            }
            let Some(u) = sim.unit(shot.unit) else {
                continue;
            };
            let class = shot.class;
            let Some(w) = sim.weapon_classes.get(class) else {
                continue;
            };
            if w.ordnance.laser.is_none()
                || w.ordnance.velocity <= 0.0
                || crate::walker_shots::style(walk, class).is_some_and(|s| s.tracer.is_some())
            {
                continue;
            }
            let step = w.ordnance.velocity * core::TICK_DT;
            let Some((p, terrain)) = struck.take_to(
                shot.origin,
                shot.dir,
                step,
                class,
                shot.flight.map(|f| f.end),
            ) else {
                continue;
            };
            let start = drawn_start(
                shot.origin,
                shot.dir,
                p,
                self.uses_drawn_muzzle(sim, u, shot.unit.index, class),
                shot.muzzle(walk).or(self.fire_point(shot.unit.index)),
            );
            self.remnants.push(Remnant {
                class,
                head: p,
                dir: (p - start).try_normalize().unwrap_or(shot.dir),
                len: (p - start).length(),
                age: (p - shot.origin).length() / w.ordnance.velocity,
                fade: 1.0,
                timer: 0.0,
                terrain,
                prev: start,
            });
        }
    }

    fn uses_drawn_muzzle(
        &self,
        sim: &core::sim::Sim,
        u: &core::unit::Unit,
        unit_index: u32,
        weapon_class: usize,
    ) -> bool {
        (drawn_from_the_gun(u)
            || ((u.mount.is_some() || u.eye_view()) && self.fire_point(unit_index).is_some()))
            && !sim.is_item_gun(weapon_class)
    }

    /// Upload the glow and every bolt sheet `walk`'s weapons can fire (card
    /// HITCH): at the battle's load, so a weapon's first shot reads no file
    /// in a frame.
    pub fn preload(&mut self, r: &mut Renderer, walk: &WalkSim) {
        self.texture(r, GLOW_TEXTURE);
        let names: Vec<String> = walk
            .sim
            .weapon_classes
            .iter()
            .filter_map(|w| {
                w.ordnance
                    .laser
                    .as_ref()?
                    .texture
                    .as_deref()
                    .map(texture_name)
            })
            .collect();
        for n in names {
            self.texture(r, &n);
        }
    }

    fn texture(&mut self, r: &mut Renderer, name: &str) -> Option<TextureHandle> {
        if let Some(t) = self.textures.get(name) {
            return *t;
        }
        let t = self.dirs.iter().find_map(|d| {
            crate::viewer::upload_texture_file(r, d, name).unwrap_or_else(|e| {
                eprintln!("bolt texture {name}: {e:#}");
                None
            })
        });
        if t.is_none() {
            eprintln!("bolt texture {name}: not in the common layers");
        }
        self.textures.insert(name.to_owned(), t);
        t
    }

    /// This frame's bolts, uploading any texture not yet loaded. With the
    /// walk's render blend on (card RINT) a bolt in flight is drawn the
    /// frame's fraction of the way from its last head to its newest; a hit
    /// bolt's remnant stays at its impact.
    pub fn draws(&mut self, r: &mut Renderer, walk: &WalkSim) -> Vec<BoltDraw> {
        let glow = self.texture(r, GLOW_TEXTURE);
        let mut out = Vec::new();
        let blend = match walk.blend.on {
            true => crate::render_blend::fraction(walk),
            false => 1.0,
        };
        let items: Vec<(usize, Vec3, Vec3, f32, f32, f32)> = self
            .tracked
            .iter()
            .map(|t| {
                let head = drawn_head(t.prev_head, t.head, blend);
                (t.class, head, t.dir, (head - t.start).length(), t.age, 1.0)
            })
            .chain(
                self.remnants
                    .iter()
                    .map(|m| (m.class, m.head, m.dir, m.len, m.age, m.fade.clamp(0.0, 1.0))),
            )
            .collect();
        for (class, head, dir, len, age, fade) in items {
            let Some(look) = visual_look(walk, class) else {
                continue;
            };
            let core = look
                .texture
                .as_deref()
                .and_then(|n| self.texture(r, &texture_name(n)));
            out.push(bolt_draw(look, head, dir, len, age, fade, [core, glow]));
        }
        for beam in &walk.sim.beams {
            let Some(look) = visual_look(walk, beam.shooter.weapon_class) else {
                continue;
            };
            let core = look
                .texture
                .as_deref()
                .and_then(|n| self.texture(r, &texture_name(n)));
            out.push(beam_draw(look, beam, [core, glow]));
        }
        for tow in &walk.sim.tow_cables {
            out.extend(tow.points.windows(2).filter_map(|segment| {
                let travel = segment[1] - segment[0];
                let length = travel.length();
                let dir = travel.try_normalize()?;
                // [D] PS2 Hoth gunner captures (Vita FLYERS, cable look):
                // dark untextured cable approximately 3.5 cm across at the gun.
                Some(BoltDraw {
                    head: segment[1],
                    dir,
                    core_len: length,
                    glow_len: 0.0,
                    width: 0.0175,
                    core_texture: None,
                    glow_texture: None,
                    core_color: [37, 37, 38, 255],
                    glow_color: [0; 4],
                })
            }));
        }
        out
    }

    /// This frame's bolt lights (BF2; none on BF1), see the module doc.
    pub fn omni_lights(&self, walk: &WalkSim) -> Vec<OmniLight> {
        if !self.lights {
            return Vec::new();
        }
        let sim = &walk.sim;
        let look = |c: usize| visual_look(walk, c);
        let mut out = Vec::new();
        for t in &self.tracked {
            let Some((rgb, radius)) = look(t.class).and_then(|l| l.light(t.age)) else {
                continue;
            };
            let flown = (t.head - t.start).length();
            out.push(OmniLight {
                pos: t.head - t.dir * (t.step * 0.5).min(flown * 0.5),
                rgb,
                radius,
            });
        }
        for m in &self.remnants {
            let Some(l) = look(m.class) else { continue };
            let lit = if l.fade_out_rate > 0.0 {
                m.fade > 0.0
            } else {
                m.timer < 2.0 * core::TICK_DT
            };
            let Some((rgb, radius)) = l.light(m.age).filter(|_| lit) else {
                continue;
            };
            let pos = if m.timer < core::TICK_DT {
                (m.prev + m.head) * 0.5
            } else {
                let normal = match (&sim.heightfield, m.terrain) {
                    (Some(h), true) => Vec3::from(h.normal_at(m.head.x, m.head.z)),
                    _ => -m.dir,
                };
                m.head + normal * 0.5
            };
            out.push(OmniLight { pos, rgb, radius });
        }
        out
    }

    /// Hand this frame's bolts and (BF2) the terrain lights — the bolts'
    /// and `others` (the map's non-Static omnis, `crate::point_light`) —
    /// gathered for the eye at `eye`, to the renderer. Returns the bolts'
    /// lights, which light the models too.
    pub fn submit(
        &mut self,
        r: &mut Renderer,
        walk: &WalkSim,
        eye: Vec3,
        others: &[OmniLight],
    ) -> Vec<OmniLight> {
        let d = self.draws(r, walk);
        r.set_bolts(d);
        let shots = self.omni_lights(walk);
        let mut lights = shots.clone();
        if self.lights {
            lights.extend_from_slice(others);
        }
        r.set_terrain_lights(match &walk.sim.heightfield {
            Some(h) if !lights.is_empty() => crate::bolt_light::gather(&lights, h, eye),
            _ => render::TerrainLights::default(),
        });
        shots
    }
}

#[cfg(test)]
mod tests;
