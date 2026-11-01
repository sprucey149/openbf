//! Laser bolts as the PS2 draws them (card FXB, `analysis/re-bolt.md` §3–§4,
//! [O]): each in-flight bolt of a `bolt` / `laser` class becomes one
//! [`render::BoltDraw`] — its two ribbons' sizes, lengths and colours from
//! the class's [`LaserLook`]. Visual only: it reads the sim, never writes it.
//!
//! - **Length** (§4): the drawn length grows from 0 at the spawn point with
//!   the distance flown, up to the class cap max(LaserLength, GlowLength,
//!   BlurLength); the core is drawn over min(len, LaserLength), the glow
//!   over min(len, GlowLength).
//! - **After the hit** (§4): the PS2 keeps the bolt at its impact point for
//!   two more updates; a class with `FadeOutTime` (the snipers) instead
//!   fades in place — w → 0.1·LaserWidth, the core's alpha and the glow's
//!   bytes × the fade. The sim drops a bolt on its impact tick, so the port
//!   keeps these remnants here. A bolt that runs out of life in the air
//!   leaves none [D: the hit path is the only one that stops the head].
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
/// (`Missile::launch`, card UORD — an index-free id no other round shares,
/// and the same fields its replay state carries). Two rounds of one class
/// from one launcher therefore stay two rounds, however they curve.
pub(crate) fn key(b: &core::projectile::Bolt) -> Key {
    let bits = |v: Vec3| v.to_array().map(f32::to_bits);
    let (end, vel) = match &b.missile {
        Some(m) => (
            bits(m.launch),
            [
                m.phases[0].to_bits(),
                m.phases[1].to_bits(),
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
                terrain: matches!(b.hit, Hit::Terrain),
                step: b.vel.length() * core::TICK_DT,
                pos: b.pos,
                flight: b.vel.normalize_or_zero(),
            });
        }
        // A bolt gone this tick that struck: its remnant at the impact. The
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
                .or_else(|| t.struck.then_some((t.end, t.terrain)));
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
mod tests {
    use super::*;
    use core::weapon::WeaponClass;

    use core::impact::Surface;
    use core::sim::{buttons, Event, PadState};

    fn surface_walk(slope: f32, building: bool, heightfield: bool, pitch: usize) -> WalkSim {
        let h = core::heightfield::Heightfield {
            size: 33,
            unit: 8.0,
            offset_x: 128.0,
            offset_z: 128.0,
            heights: (0..33 * 33)
                .map(|i| slope * ((i / 33) as f32 * 8.0 - 128.0))
                .collect(),
        };
        let mut walk = WalkSim::new(42, Vec3::ZERO, heightfield.then_some(h));
        if building {
            walk.sim.collision = core::collision::CollisionWorld::new(
                vec![core::collision::Body::new(
                    glam::Mat4::from_translation(Vec3::new(0.0, -0.4, 0.0)),
                    core::collision::Shape::Box {
                        half: Vec3::new(128.0, 0.5, 128.0),
                    },
                )],
                vec![],
            );
        }
        // The built-in rifle with the existing BF2 light, as in the flat control.
        for w in &mut walk.sim.weapon_classes {
            if let Some(l) = w.ordnance.laser.as_mut() {
                l.light_color = [148, 25, 25, 255];
                l.light_radius = 4.0;
            }
        }
        for _ in 0..pitch {
            walk.step_tick(&[PadState {
                ry: -32767,
                ..PadState::default()
            }]);
        }
        walk.step_tick(&[PadState {
            buttons: buttons::FIRE,
            ..PadState::default()
        }]);
        walk
    }

    // Public seating and real step_tick/FIRE events are the principal witness.
    // The gun deliberately differs in data as well as class identity.
    fn mounted_walk(different: bool, building: bool) -> WalkSim {
        mounted_fixture(different, building, |_, _, _| {})
    }

    fn mounted_fixture(
        different: bool,
        building: bool,
        before_fire: impl FnOnce(&mut WalkSim, usize, usize),
    ) -> WalkSim {
        use core::vehicle::gun::{Aimer, GunSetup, GunWeapon};
        use core::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
        let h = core::heightfield::Heightfield {
            size: 33,
            unit: 8.0,
            offset_x: 128.0,
            offset_z: 128.0,
            heights: vec![0.0; 33 * 33],
        };
        let mut walk = WalkSim::new(42, Vec3::ZERO, Some(h));
        let rider = walk.sim.unit(walk.player).unwrap().weapon_class;
        let rifle = &mut walk.sim.weapon_classes[rider];
        let look = rifle.ordnance.laser.as_mut().unwrap();
        look.light_color = [148, 25, 25, 255];
        look.light_radius = 4.0;
        let gun = if different {
            let mut gun = rifle.clone();
            gun.ordnance.velocity *= 1.25;
            let look = gun.ordnance.laser.as_mut().unwrap();
            look.light_radius = 9.0;
            look.light_color = [25, 148, 25, 255];
            look.width *= 2.0;
            look.length *= 2.0;
            walk.sim.add_weapon_class(gun)
        } else {
            rider
        };
        walk.sim.seat_setup = vec![Some(SeatSetup {
            radius: 5.0,
            body_active: vec![],
            boardable: vec![true],
            exit_drop: 0.0,
            ai_size: 1,
            guns: vec![Some(GunSetup {
                weapons: vec![GunWeapon {
                    class: gun,
                    infinite: true,
                    reserve_clips: 0,
                }],
                fire_point: Vec3::ZERO,
                pivot: Vec3::ZERO,
                eye: Vec3::ZERO,
                aimer: Aimer {
                    pitch: [-1.57, 1.57],
                    ..Aimer::default()
                },
                pilot: false,
                turn: None,
                framed: false,
                aimer_bind: glam::Quat::IDENTITY,
                gunner: None,
            })],
        })];
        let vehicle = walk.sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Turret,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::new(0.0, 2.0, 0.0),
            yaw: 0.0,
            max_health: 1000.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Gunner(0)],
        });
        assert!(walk.sim.enter_vehicle(walk.player, vehicle, 0));
        assert!(walk.sim.unit(walk.player).unwrap().mount.is_some());
        if building {
            walk.sim.collision = core::collision::CollisionWorld::new(
                vec![core::collision::Body::new(
                    glam::Mat4::from_translation(Vec3::new(0.0, -0.4, 0.0)),
                    core::collision::Shape::Box {
                        half: Vec3::new(128.0, 0.5, 128.0),
                    },
                )],
                vec![],
            );
        }
        for _ in 0..40 {
            walk.step_tick(&[PadState {
                ry: -32767,
                ..PadState::default()
            }]);
        }
        before_fire(&mut walk, rider, gun);
        walk.step_tick(&[PadState {
            buttons: buttons::FIRE,
            ..PadState::default()
        }]);
        let shots: Vec<_> = walk
            .sim
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Shot {
                    unit,
                    weapon_class,
                    origin,
                    dir,
                    ..
                } if *unit == walk.player => {
                    assert_eq!(*weapon_class as usize, gun);
                    Some((*origin, *dir))
                }
                _ => None,
            })
            .collect();
        assert_eq!(shots.len(), 1);
        if !walk.sim.bolts.is_empty() {
            assert_eq!(walk.sim.bolts.len(), 1);
            assert!(!walk
                .sim
                .events
                .iter()
                .any(|e| matches!(e, Event::BoltExpired { .. })));
            return walk;
        }
        let head = impact(
            &walk,
            if building {
                Surface::Building
            } else {
                Surface::Terrain
            },
        );
        assert!(walk.sim.events.iter().any(|e| matches!(e,
            Event::Impact { weapon_class, .. } if *weapon_class as usize == gun)));
        assert!(walk.sim.events.iter().any(|e| matches!(e,
            Event::BoltExpired { pos, struck: true } if *pos == head)));
        assert!(walk.sim.bolts.is_empty(), "actual same-tick gun hit");
        walk
    }

    fn mounted_light(mut walk: WalkSim, terrain: bool, label: &str) {
        let (origin, dir) = walk
            .sim
            .events
            .iter()
            .find_map(|e| match e {
                Event::Shot { origin, dir, .. } => Some((*origin, *dir)),
                _ => None,
            })
            .unwrap();
        let head = walk
            .sim
            .events
            .iter()
            .find_map(|e| match e {
                Event::BoltExpired { pos, struck: true } => Some(*pos),
                _ => None,
            })
            .unwrap();
        let mut vis = lit_vis();
        visual_update(&mut vis, &walk);
        assert_eq!(vis.remnant_count(), 1);
        let lights = vis.omni_lights(&walk);
        assert_eq!(lights.len(), 1);
        assert!(
            lights[0].pos.abs_diff_eq((origin + head) * 0.5, 1e-4),
            "reaching midpoint"
        );
        let normal = if terrain {
            Vec3::from(
                walk.sim
                    .heightfield
                    .as_ref()
                    .unwrap()
                    .normal_at(head.x, head.z),
            )
        } else {
            -dir
        };
        walk.step_tick(&[PadState::default()]);
        visual_update(&mut vis, &walk);
        let lights = vis.omni_lights(&walk);
        assert_eq!(lights.len(), 1);
        assert!(
            lights[0].pos.abs_diff_eq(head + normal * 0.5, 1e-4),
            "{label}: actual={:?}, expected={:?}, error={}",
            lights[0].pos,
            head + normal * 0.5,
            lights[0].pos.distance(head + normal * 0.5)
        );
        walk.step_tick(&[PadState::default()]);
        visual_update(&mut vis, &walk);
        assert_eq!(vis.remnant_count(), 0);
        assert!(vis.omni_lights(&walk).is_empty());
    }

    #[test]
    fn mounted_different_class_terrain_actual_step() {
        mounted_light(
            mounted_walk(true, false),
            true,
            "valid gun-class terrain impact",
        );
    }

    #[test]
    fn mounted_same_class_terrain_control() {
        mounted_light(
            mounted_walk(false, false),
            true,
            "same-class terrain control",
        );
    }

    #[test]
    fn mounted_building_control() {
        mounted_light(mounted_walk(true, true), false, "mounted Building");
    }

    fn mounted_decoy(field: usize) {
        let mut walk = mounted_walk(true, true);
        let rider = walk.sim.unit(walk.player).unwrap().weapon_class;
        let mut decoy = *walk
            .sim
            .events
            .iter()
            .find(|e| matches!(e, Event::Impact { .. }))
            .unwrap();
        if let Event::Impact {
            pos,
            dir,
            weapon_class,
            surface,
            ..
        } = &mut decoy
        {
            *surface = Surface::Terrain;
            match field {
                0 => *weapon_class = rider as u32,
                1 => *pos += Vec3::X,
                2 => *dir = -*dir,
                _ => unreachable!(),
            }
        }
        walk.sim.events.insert(0, decoy);
        mounted_light(
            walk,
            false,
            match field {
                0 => "reject unrelated rider-class terrain decoy",
                1 => "reject displaced gun-class terrain decoy",
                2 => "reject reversed gun-class terrain decoy",
                _ => unreachable!(),
            },
        );
    }

    #[test]
    fn mounted_decoy_class() {
        mounted_decoy(0);
    }
    #[test]
    fn mounted_decoy_position() {
        mounted_decoy(1);
    }
    #[test]
    fn mounted_decoy_direction() {
        mounted_decoy(2);
    }

    #[test]
    fn mounted_fired_class_appearance() {
        let walk = mounted_fixture(true, false, |walk, rider, gun| {
            for (class, width, length, glow, texture, color) in [
                (rider, 0.1, 0.4, 0.6, "Rider", [200, 20, 20, 100]),
                (gun, 0.3, 0.8, 1.2, "Gun", [20, 200, 20, 150]),
            ] {
                let l = walk.sim.weapon_classes[class]
                    .ordnance
                    .laser
                    .as_mut()
                    .unwrap();
                l.width = width;
                l.length = length;
                l.glow_length = glow;
                l.texture = Some(texture.into());
                l.glow_color = color;
                l.fade_light = false;
            }
        });
        let gun = walk
            .sim
            .events
            .iter()
            .find_map(|e| match e {
                Event::Shot { weapon_class, .. } => Some(*weapon_class as usize),
                _ => None,
            })
            .unwrap();
        let mut vis = lit_vis();
        visual_update(&mut vis, &walk);
        assert_eq!(vis.remnant_count(), 1);
        let m = &vis.remnants[0];
        assert_eq!(m.class, gun, "stored fired class");
        let light = vis.omni_lights(&walk)[0];
        assert_eq!(light.radius, 9.0, "gun light radius");
        assert_eq!(light.rgb, [25, 148, 25], "gun light color");
        assert!(
            (m.age - m.len / walk.sim.weapon_classes[gun].ordnance.velocity).abs() < 1e-6,
            "gun flight age"
        );
        let look = walk.sim.weapon_classes[m.class]
            .ordnance
            .laser
            .as_ref()
            .unwrap();
        assert_eq!(texture_name(look.texture.as_deref().unwrap()), "gun");
        let draw = bolt_draw(look, m.head, m.dir, m.len, m.age, m.fade, [None, None]);
        assert_eq!(draw.width, 0.3);
        assert_eq!(draw.core_len, 0.8);
        assert_eq!(draw.glow_len, 1.2);
        assert_eq!(draw.core_color, [255; 4]);
        assert_eq!(draw.glow_color, [20, 200, 20, 150]);
    }

    #[test]
    fn mounted_rider_without_laser() {
        let walk = mounted_fixture(true, false, |w, rider, _| {
            w.sim.weapon_classes[rider].ordnance.laser = None;
        });
        mounted_light(walk, true, "laser gun with nonlaser rider");
    }

    #[test]
    fn mounted_slow_rider() {
        let walk = mounted_fixture(true, false, |w, rider, _| {
            w.sim.weapon_classes[rider].ordnance.velocity = 0.1;
        });
        mounted_light(walk, true, "gun reach with slow rider");
    }

    #[test]
    fn mounted_gun_without_laser() {
        let walk = mounted_fixture(true, false, |w, _, gun| {
            w.sim.weapon_classes[gun].ordnance.laser = None;
        });
        let mut vis = lit_vis();
        visual_update(&mut vis, &walk);
        assert_eq!(vis.remnant_count(), 0, "nonlaser gun has no remnant");
        assert!(vis.omni_lights(&walk).is_empty());
    }

    #[test]
    fn mounted_slower_gun_rejects_distant_expiry() {
        let mut walk = mounted_fixture(true, false, |w, rider, gun| {
            w.sim.weapon_classes[rider].ordnance.velocity *= 2.0;
            w.sim.weapon_classes[gun].ordnance.velocity *= 0.1;
        });
        // The real shot has not reached its terrain hit. An unrelated struck
        // expiry on that ray lies inside only the rider's longer first step.
        let (origin, dir, gun) = walk
            .sim
            .events
            .iter()
            .find_map(|e| match e {
                Event::Shot {
                    origin,
                    dir,
                    weapon_class,
                    ..
                } => Some((*origin, *dir, *weapon_class as usize)),
                _ => None,
            })
            .unwrap();
        assert_eq!(walk.sim.bolts.len(), 1, "slow gun still in flight");
        let step = walk.sim.weapon_classes[gun].ordnance.velocity * core::TICK_DT;
        walk.sim.events.push(Event::BoltExpired {
            pos: origin + dir * (step * 2.0),
            struck: true,
        });
        let mut vis = lit_vis();
        visual_update(&mut vis, &walk);
        assert_eq!(vis.remnant_count(), 0, "gun reach rejects unrelated expiry");
    }

    #[test]
    fn mounted_gun_fade_and_bf1_light_control() {
        for building in [false, true] {
            let mut walk = mounted_fixture(true, building, |w, rider, gun| {
                w.sim.weapon_classes[rider]
                    .ordnance
                    .laser
                    .as_mut()
                    .unwrap()
                    .fade_out_rate = 60.0;
                let l = w.sim.weapon_classes[gun].ordnance.laser.as_mut().unwrap();
                l.fade_out_rate = 6.0;
                l.fade_light = true;
                l.fade_start = 0.0;
                l.fade_duration = 0.05;
                l.end_light_color = [100, 40, 200, 255];
                l.end_glow_color = [10, 30, 70, 100];
            });
            let mut vis = lit_vis();
            let mut bf1 = BoltVis::new(Path::new("unused-fixture-assets"), "bf1");
            visual_update(&mut vis, &walk);
            visual_update(&mut bf1, &walk);
            let m = vis.remnants[0].clone();
            let l = walk.sim.weapon_classes[m.class]
                .ordnance
                .laser
                .as_ref()
                .unwrap()
                .clone();
            assert_eq!(vis.omni_lights(&walk)[0].pos, (m.prev + m.head) * 0.5);
            assert_eq!(vis.omni_lights(&walk)[0].rgb, l.light(m.age).unwrap().0);
            assert!(bf1.omni_lights(&walk).is_empty());
            for tick in 1..=6 {
                walk.step_tick(&[PadState::default()]);
                visual_update(&mut vis, &walk);
                visual_update(&mut bf1, &walk);
                assert!(bf1.omni_lights(&walk).is_empty());
                if tick < 5 {
                    assert_eq!(vis.remnant_count(), 1, "gun fade lifetime tick {tick}");
                    let r = &vis.remnants[0];
                    assert!((r.fade - (1.0 - tick as f32 * core::TICK_DT * 6.0)).abs() < 1e-6);
                    assert_eq!(r.age, m.age, "flight age freezes at hit");
                    let draw = bolt_draw(&l, r.head, r.dir, r.len, r.age, r.fade, [None, None]);
                    assert_eq!(draw.glow_color, l.glow_bytes(m.age, r.fade));
                    assert_eq!(draw.width, l.half_width(r.fade));
                    let normal = if building { -m.dir } else { Vec3::Y };
                    assert!(vis.omni_lights(&walk)[0]
                        .pos
                        .abs_diff_eq(m.head + normal * 0.5, 1e-4));
                    assert_eq!(vis.omni_lights(&walk)[0].rgb, l.light(m.age).unwrap().0);
                }
            }
            assert_eq!(vis.remnant_count(), 0, "gun fade expires");
            assert_eq!(bf1.remnant_count(), 0);
        }
    }

    fn lit_vis() -> BoltVis {
        BoltVis::new(Path::new("unused-fixture-assets"), "bf2")
    }

    fn visual_update(vis: &mut BoltVis, walk: &WalkSim) {
        let hash = walk.sim.hash();
        let events = walk.sim.events.clone();
        vis.update(walk, core::TICK_DT);
        let _ = vis.omni_lights(walk);
        assert_eq!(
            walk.sim.hash(),
            hash,
            "visual update preserves simulation hash"
        );
        assert_eq!(walk.sim.events, events, "visual update preserves events");
    }

    fn impact(walk: &WalkSim, expected: Surface) -> Vec3 {
        let hits: Vec<_> = walk
            .sim
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Impact { pos, surface, .. } if *surface == expected => Some(*pos),
                _ => None,
            })
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "legal fixture must hit {expected:?}: {:?}",
            walk.sim.events
        );
        hits[0]
    }

    fn check_same_tick(mut walk: WalkSim, surface: Surface) {
        let head = impact(&walk, surface);
        assert!(walk.sim.bolts.is_empty(), "actual first-tick impact");
        let mut vis = lit_vis();
        visual_update(&mut vis, &walk);
        assert_eq!(vis.remnant_count(), 1);
        let prev = vis.remnants[0].prev;
        let lights = vis.omni_lights(&walk);
        assert_eq!(lights.len(), 1);
        assert!(lights[0].pos.abs_diff_eq((head + prev) * 0.5, 1e-4));
        vis.lights = false;
        assert!(vis.omni_lights(&walk).is_empty(), "BF1 no-light control");
        vis.lights = true;
        let normal = if surface == Surface::Terrain {
            Vec3::from(
                walk.sim
                    .heightfield
                    .as_ref()
                    .unwrap()
                    .normal_at(head.x, head.z),
            )
        } else {
            -vis.remnants[0].dir
        };
        walk.step_tick(&[PadState::default()]);
        visual_update(&mut vis, &walk);
        let lights = vis.omni_lights(&walk);
        assert_eq!(lights.len(), 1);
        assert!(
            lights[0].pos.abs_diff_eq(head + normal * 0.5, 1e-4),
            "surface={surface:?} head={head:?} actual={:?} expected={:?}",
            lights[0].pos,
            head + normal * 0.5
        );
        walk.step_tick(&[PadState::default()]);
        visual_update(&mut vis, &walk);
        assert_eq!(vis.remnant_count(), 0);
        assert!(vis.omni_lights(&walk).is_empty());
    }

    #[test]
    fn same_tick_steep_terrain_uses_actual_surface() {
        for pitch in 18..=22 {
            check_same_tick(surface_walk(-2.0, false, true, pitch), Surface::Terrain);
        }
    }

    #[test]
    fn same_tick_near_ground_building_uses_actual_surface() {
        for pitch in 19..=22 {
            check_same_tick(surface_walk(0.0, true, true, pitch), Surface::Building);
        }
    }

    #[test]
    fn same_tick_surface_controls() {
        check_same_tick(surface_walk(0.0, false, true, 22), Surface::Terrain);
        check_same_tick(surface_walk(-0.5, false, true, 20), Surface::Terrain);
        // Also exercise the renderer without a heightfield after a real hit.
        let mut no_heightfield = surface_walk(0.0, true, true, 22);
        no_heightfield.sim.heightfield = None;
        check_same_tick(no_heightfield, Surface::Building);
    }

    #[test]
    fn tracked_multitick_terrain_keeps_surface() {
        let mut walk = surface_walk(0.0, false, true, 10);
        assert_eq!(walk.sim.bolts.len(), 1, "fixture starts in flight");
        let mut vis = lit_vis();
        visual_update(&mut vis, &walk);
        assert_eq!(vis.tracked.len(), 1);
        for _ in 0..30 {
            walk.step_tick(&[PadState::default()]);
            visual_update(&mut vis, &walk);
            if vis.remnant_count() == 0 {
                continue;
            }
            let head = impact(&walk, Surface::Terrain);
            assert_eq!(vis.remnant_count(), 1);
            assert!(vis.omni_lights(&walk)[0]
                .pos
                .abs_diff_eq((vis.remnants[0].prev + head) * 0.5, 1e-4));
            walk.step_tick(&[PadState::default()]);
            visual_update(&mut vis, &walk);
            assert!(vis.omni_lights(&walk)[0]
                .pos
                .abs_diff_eq(head + Vec3::Y * 0.5, 1e-4));
            walk.step_tick(&[PadState::default()]);
            visual_update(&mut vis, &walk);
            assert_eq!(vis.remnant_count(), 0);
            return;
        }
        panic!("tracked fixture must reach terrain");
    }

    #[test]
    fn same_tick_fade_preserves_duration_and_light_offset() {
        let mut walk = surface_walk(-2.0, false, true, 20);
        let head = impact(&walk, Surface::Terrain);
        let normal = Vec3::from(
            walk.sim
                .heightfield
                .as_ref()
                .unwrap()
                .normal_at(head.x, head.z),
        );
        for w in &mut walk.sim.weapon_classes {
            if let Some(l) = w.ordnance.laser.as_mut() {
                l.fade_out_rate = 10.0;
            }
        }
        let mut vis = lit_vis();
        visual_update(&mut vis, &walk);
        for tick in 1..=4 {
            walk.step_tick(&[PadState::default()]);
            visual_update(&mut vis, &walk);
            if tick < 3 {
                assert_eq!(vis.remnant_count(), 1);
                assert!(
                    (vis.remnants[0].fade - (1.0 - tick as f32 * core::TICK_DT * 10.0)).abs()
                        < 1e-6
                );
                assert!(vis.omni_lights(&walk)[0]
                    .pos
                    .abs_diff_eq(head + normal * 0.5, 1e-4));
            }
        }
        assert_eq!(vis.remnant_count(), 0);
        assert!(vis.omni_lights(&walk).is_empty());
    }

    // Supplemental event controls deliberately put a terrain decoy before the
    // real building impact. Each input must independently reject that decoy.
    fn decoy_case(field: usize) {
        let mut walk = surface_walk(0.0, true, true, 22);
        let head = impact(&walk, Surface::Building);
        let mut decoy = *walk
            .sim
            .events
            .iter()
            .find(|e| matches!(e, Event::Impact { .. }))
            .unwrap();
        if let Event::Impact {
            pos,
            dir,
            weapon_class,
            surface,
            ..
        } = &mut decoy
        {
            *surface = Surface::Terrain;
            match field {
                0 => *pos += Vec3::X,
                1 => *dir = -*dir,
                2 => *weapon_class += 1,
                _ => unreachable!(),
            }
        }
        walk.sim.events.insert(0, decoy);
        let mut vis = lit_vis();
        visual_update(&mut vis, &walk);
        assert_eq!(vis.remnant_count(), 1);
        let dir = vis.remnants[0].dir;
        walk.step_tick(&[PadState::default()]);
        visual_update(&mut vis, &walk);
        assert!(
            vis.omni_lights(&walk)[0]
                .pos
                .abs_diff_eq(head - dir * 0.5, 1e-4),
            "decoy field {field} must not select terrain"
        );
    }

    #[test]
    fn impact_decoy_position() {
        decoy_case(0);
    }
    #[test]
    fn impact_decoy_direction() {
        decoy_case(1);
    }
    #[test]
    fn impact_decoy_weapon_class() {
        decoy_case(2);
    }

    /// A BF1 rifle bolt 2 m out draws 1 m of core and 2 m of glow; at 10 m
    /// the core 1 m and the glow 3 m (GlowLength), whatever BlurLength's cap.
    /// A sniper remnant at fade 0.5 is 0.55·w wide, core alpha 127, glow
    /// bytes halved.
    #[test]
    fn drawn_lengths_widths_and_fades() {
        let l = WeaponClass::bf1_rifle().ordnance.laser.unwrap();
        let d = bolt_draw(&l, Vec3::ZERO, Vec3::X, 2.0, 0.0, 1.0, [None, None]);
        assert_eq!((d.core_len, d.glow_len), (1.0, 2.0));
        assert!((d.width - 0.1).abs() < 1e-7);
        assert_eq!(d.core_color, [255, 255, 255, 255]);
        assert_eq!(d.glow_color, [253, 43, 43, 100]);
        let d = bolt_draw(&l, Vec3::ZERO, Vec3::X, 10.0, 0.0, 1.0, [None, None]);
        assert_eq!((d.core_len, d.glow_len), (1.0, 3.0));
        let d = bolt_draw(&l, Vec3::ZERO, Vec3::X, 10.0, 0.0, 0.5, [None, None]);
        assert!((d.width - 0.055).abs() < 1e-7, "{}", d.width);
        assert_eq!(d.core_color[3], 127);
        assert_eq!(d.glow_color, [126, 21, 21, 50]);
    }

    /// A soldier firing at the flat ground under his feet: the sim spawns
    /// and resolves each bolt inside one tick, and the vis still keeps it
    /// at its impact; on BF2 it lights the ground there, 0.5 m up the
    /// terrain's normal, for one update; BF1 lights nothing.
    #[test]
    fn a_shot_at_the_ground_underfoot_is_kept_and_lit() {
        use core::sim::{buttons, PadState};
        let flat = core::heightfield::Heightfield {
            size: 33,
            unit: 8.0,
            offset_x: 128.0,
            offset_z: 128.0,
            heights: vec![0.0; 33 * 33],
        };
        let mut walk = WalkSim::new(42, Vec3::ZERO, Some(flat));
        // The built-in trooper's BF1 rifle, given BF2's rifle light.
        for w in &mut walk.sim.weapon_classes {
            if let Some(l) = w.ordnance.laser.as_mut() {
                l.light_color = [148, 25, 25, 255];
                l.light_radius = 4.0;
            }
        }
        let mut vis = BoltVis {
            lights: true,
            ..BoltVis::default()
        };
        let down = PadState {
            ry: -32767,
            ..PadState::default()
        };
        for _ in 0..40 {
            walk.step_tick(&[down]);
        }
        let fire = PadState {
            buttons: buttons::FIRE,
            ..PadState::default()
        };
        let mut kept = None;
        for _ in 0..30 {
            walk.step_tick(&[fire]);
            vis.update(&walk, core::TICK_DT);
            if walk.sim.bolts.is_empty() && !vis.remnants.is_empty() {
                kept = Some(vis.remnants[0].head);
                break;
            }
        }
        let head = kept.expect("a same-tick hit kept as a remnant");
        let feet = walk.sim.unit(walk.player).unwrap().soldier.pos;
        assert!(
            // The terrain ray's crossing is good to a quarter of its 0.5 m
            // step along the bolt.
            head.y.abs() < 0.2 && head.distance(feet) < 8.0,
            "{head} {feet}"
        );
        // The reaching update: the midpoint of the one step it flew.
        let prev = vis.remnants[0].prev;
        let lights = vis.omni_lights(&walk);
        assert_eq!(lights.len(), 1);
        assert!(lights[0].pos.abs_diff_eq((prev + head) * 0.5, 1e-4));
        vis.lights = false;
        assert!(vis.omni_lights(&walk).is_empty(), "BF1: no light");
        vis.lights = true;
        // The hit: 0.5 m up the flat ground's normal; then gone.
        let idle = PadState::default();
        walk.step_tick(&[idle]);
        vis.update(&walk, core::TICK_DT);
        let lights = vis.omni_lights(&walk);
        assert_eq!(lights.len(), 1);
        assert!(lights[0].pos.abs_diff_eq(head + Vec3::Y * 0.5, 1e-4));
        walk.step_tick(&[idle]);
        vis.update(&walk, core::TICK_DT);
        assert!(vis.omni_lights(&walk).is_empty());
    }

    /// The drawn line starts at the fire point for a soldier, at the spawn
    /// for a rider or when the fire point would be past the end.
    #[test]
    fn a_soldiers_bolt_starts_at_the_fire_point() {
        let dir = Vec3::NEG_Z;
        let end = Vec3::new(0.0, 0.0, -50.0);
        let s = drawn_start(Vec3::ZERO, dir, end, true, None);
        assert!(s.abs_diff_eq(Vec3::new(0.3, 0.0, -1.2), 1e-6), "{s}");
        let fire = Vec3::new(0.2, 0.1, -0.9);
        assert_eq!(drawn_start(Vec3::ZERO, dir, end, true, Some(fire)), fire);
        assert_eq!(
            drawn_start(Vec3::ZERO, dir, end, false, Some(fire)),
            Vec3::ZERO
        );
        let near = Vec3::new(0.0, 0.0, -1.0);
        assert_eq!(drawn_start(Vec3::ZERO, dir, near, true, None), Vec3::ZERO);
        assert_eq!(texture_name("purplelaser_D"), "violetlaser_d");
        assert_eq!(texture_name("com_sfx_laser_red"), "com_sfx_laser_red");
    }

    #[test]
    fn a_turrets_bolt_is_drawn_from_its_own_gun_not_its_owners_hand() {
        let (mut walk, weapon, gun) = crate::walk::turret_test_walk();
        let hand = Vec3::new(0.3, 1.4, -0.5);
        let mut points = vec![None; walk.sim.units.len()];
        points[walk.player.index as usize] = Some(hand);
        let mut vis = BoltVis::default();
        let mut drawn = None;
        for _ in 0..10 * core::TICK_HZ {
            walk.step_tick(&[PadState::default()]);
            vis.set_fire_points(points.clone());
            vis.update(&walk, core::TICK_DT);
            let b = walk.sim.bolts.iter().find(|b| b.weapon_class == gun);
            if let (Some(b), Some(v)) = (b, vis.tracked.iter().find(|v| v.class == gun)) {
                drawn = Some((v.start, b.pos - b.vel * b.age));
                break;
            }
        }
        let (start, spawn) = drawn.expect("the turret never fired");
        assert!(walk.sim.is_item_gun(gun) && !walk.sim.is_item_gun(weapon));
        assert!(start.abs_diff_eq(spawn, 1e-3), "{start} vs spawn {spawn}");
        let centre = walk.sim.turrets[0].centre();
        assert!(start.distance(centre) < 0.5, "{start} vs turret {centre}");
        assert!(start.distance(hand) > 10.0, "{start} at the owner's hand");
    }

    /// A missile round of a fixture class (the REP torpedo's numbers, card
    /// UORD): the ctor's two waver phases come from `seed`, as the class's
    /// `Missile::launch` draws them.
    fn missile_round(seed: u32) -> (WeaponClass, core::projectile::Bolt) {
        let mut w = WeaponClass::bf1_rifle();
        let o = &mut w.ordnance;
        o.kind = core::weapon::OrdKind::Missile;
        o.velocity = 40.0;
        o.life_span = 4.0;
        o.missile = Some(core::missile::MissileClass {
            turn_rate: 2.0,
            ..core::missile::MissileClass::default()
        });
        let mut lcg = core::lcg::GameLcg::new(seed);
        let mut b = core::projectile::Bolt {
            visual_shot: None,
            pos: Vec3::new(1.0, 1.6, -2.0),
            vel: Vec3::NEG_Z * 40.0,
            remaining: 4.0,
            age: 0.0,
            owner: core::unit::UnitHandle { index: 3, gen: 2 },
            team: 0,
            weapon_class: 9,
            hit: core::projectile::Hit::None,
            end: Vec3::new(1.0, 1.6, -162.0),
            shooter_is_player: false,
            hulls: core::collision::HullFilter::All,
            hit_mult: 1.0,
            gravity: 0.0,
            charge: 1.0,
            missile: None,
            fat: None,
        };
        let mc = *o.missile.as_ref().unwrap();
        b.as_missile(o, &mc, None, &mut lcg);
        (w, b)
    }

    /// §4.3: the law turns a missile BEFORE it moves, so its `vel` and `end`
    /// are new values every tick. One missile must stay one round for the
    /// whole flight — one trail emitter, not one per tick of its curve — and
    /// two rounds of one class from one launcher must stay two.
    #[test]
    fn one_missile_stays_one_round_while_it_steers() {
        let (w, mut b) = missile_round(3);
        let mc = *w.ordnance.missile.as_ref().unwrap();
        let first = key(&b);
        let mut lcg = core::lcg::GameLcg::new(11);
        for tick in 0..20 {
            b.guide(
                &mc,
                Some(Vec3::new(30.0, 0.0, 0.0)),
                core::TICK_DT,
                &mut lcg,
            );
            b.advance(core::TICK_DT);
            assert_eq!(key(&b), first, "tick {tick} is the same round");
        }
        assert!(
            b.vel.normalize().angle_between(Vec3::NEG_Z) > 0.2,
            "the fixture did turn: {}",
            b.vel
        );
        // The end point is the probe's on a hit; the key does not move with
        // it either.
        b.end = Vec3::ZERO;
        assert_eq!(key(&b), first);
        // A second round of the same class, from the same launcher and the
        // same launch line, is its own round.
        let (_, other) = missile_round(4);
        assert_ne!(key(&other), first);
        // A straight bolt keeps the old identity: its line is its key.
        let mut plain = other;
        plain.missile = None;
        let k = key(&plain);
        plain.end += Vec3::X;
        assert_ne!(key(&plain), k, "a bolt's line is its identity");
    }

    /// The missile draw (card UORD): a round whose class has an
    /// `ordnance.geometry` model belongs to `SoldierVisuals` — this module
    /// keeps no ribbon and no remnant for it — and the model sits at the
    /// sim's place, its +Z the way the round flies. A round with no model
    /// still tracks on a class that authored a `LaserLook`, and a stopped
    /// one is not drawn at all.
    #[test]
    fn a_bullet_with_a_model_draws_as_its_model() {
        let mut walk = WalkSim::new(42, Vec3::ZERO, None);
        let (mut w, mut b) = missile_round(5);
        w.ordnance.missile = None;
        w.ordnance.geometry = Some("com_weap_missile".into());
        let class = walk.sim.add_weapon_class(w);
        b.missile = None;
        b.weapon_class = class;
        walk.sim.bolts.push(b);
        let listed = missiles_in_flight(&walk);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, class);
        let mut vis = BoltVis::default();
        vis.update(&walk, core::TICK_DT);
        assert!(vis.tracked.is_empty());
    }

    #[test]
    fn a_missile_draws_as_its_model_not_as_a_ribbon() {
        let mut walk = WalkSim::new(42, Vec3::ZERO, None);
        let (mut w, b) = missile_round(5);
        w.ordnance.laser = Some(core::laser::LaserLook::default());
        w.ordnance.geometry = Some("com_weap_missile".into());
        let class = walk.sim.add_weapon_class(w);
        let mut b = b;
        b.weapon_class = class;
        walk.sim.bolts.push(b);

        let mut vis = BoltVis::default();
        vis.update(&walk, core::TICK_DT);
        assert!(vis.tracked.is_empty(), "the model path draws it");
        assert!(vis.remnant_count() == 0 && vis.omni_lights(&walk).is_empty());
        let listed = missiles_in_flight(&walk);
        assert_eq!(listed.len(), 1);
        let (c, m) = listed[0];
        assert_eq!(c, class);
        assert!(m
            .transform_point3(Vec3::ZERO)
            .abs_diff_eq(Vec3::new(1.0, 1.6, -2.0), 1e-4));
        assert!(
            m.z_axis.truncate().abs_diff_eq(Vec3::NEG_Z, 1e-4),
            "{:?}",
            m.z_axis
        );

        // A stopped round is no draw and no panic (no direction to point).
        walk.sim.bolts[0].vel = Vec3::ZERO;
        assert!(missiles_in_flight(&walk).is_empty());

        // The same round with no model: the look this fixture authored gets
        // the ribbon path (nothing is silently dropped).
        walk.sim.bolts[0].vel = Vec3::NEG_Z * 40.0;
        walk.sim.weapon_classes[class].ordnance.geometry = None;
        let mut vis = BoltVis::default();
        vis.update(&walk, core::TICK_DT);
        assert_eq!(
            vis.tracked.len(),
            1,
            "an authored look is the fallback ribbon"
        );
        assert_eq!(vis.tracked[0].class, class);
    }

    #[test]
    fn a_beam_draws_from_its_origin_to_what_it_struck() {
        let look = core::laser::LaserLook {
            width: 0.1,
            ..Default::default()
        };
        let beam = core::sim::Beam {
            shooter: core::projectile::Shooter {
                owner: core::unit::UnitHandle { index: 0, gen: 0 },
                team: 0,
                weapon_class: 0,
                is_player: true,
            },
            hulls: core::collision::HullFilter::All,
            charge: 1.0,
            origin: Vec3::new(0.0, 1.0, 0.0),
            dir: Vec3::X,
            life: 0.1,
            len: 40.0,
            follow: true,
        };
        let d = beam_draw(&look, &beam, [None, None]);
        assert_eq!(d.head, Vec3::new(40.5, 1.0, 0.0));
        assert_eq!(d.dir, Vec3::X);
        assert_eq!((d.core_len, d.glow_len), (41.0, 41.0));
        assert!((d.width - 0.1).abs() < 1e-7);
    }
}
