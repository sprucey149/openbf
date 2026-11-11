//! Effects: the PS2's particle effects (card FXP2, `analysis/re-fx.md`
//! §5–§6, [O]) and the PROVISIONAL placeholder quads still standing in for
//! what nothing spawns yet.
//!
//! **Particle effects.** [`Library`] holds the baked `effects.ron` layers
//! (card FXP1) — the map's, its two sides', the common one — and looks an
//! effect up by `fnv1a_lowercase(name)` in that order, as the PS2's one
//! effect table resolves a name (re-fx §1); textures resolve the same way.
//! [`Fx::spawn`] starts one at a frame; each [`LiveEffect`] runs the
//! emitters (`emitter.rs`, §5) and [`Fx::submit`] hands the renderer this
//! frame's particles (`draw.rs`, §6), which it sorts far to near among the
//! laser bolts (`render::particles`).
//!
//! **What spawns them** (card FXP3, re-fx §7 [O]). A struck bolt
//! (`Event::Impact`) spawns its ordnance's `ImpactEffect*` for the surface
//! ([`core::impact::ImpactLook::effect`]) at the hit with world axes — riding
//! the hull when it struck a vehicle — then its `ExplosionName` class's
//! effect ([`Fx::explode`]); a destroyed vehicle (`VehicleDestroyed`)
//! detonates its class's explosion effect, a droid's chunk death
//! (`DroidBurst`) its class's `ExplosionName` effect. A name the running game's
//! effect table does not hold spawns nothing (§7: BF2's inherited BF1
//! names). The PS2 draws an explosion's effect only within 60 m of the
//! camera (B1 FUN_002701a0) — a PS2 budget the port drops (FEEL_NOTES).
//! A bolt that strikes water spawns `ImpactEffectWater` on the surface; a
//! blast at or below the water + 0.3 m spawns `WaterEffect` instead of
//! `Effect`; a round in flight carries its `TrailEffect` (`trail.rs`); a
//! soldier in a jet stance carries his class's `JetEffect` (`jet.rs`).
//! An emitter shot's links (`Event::Emitter`) draw their class's
//! `LightningEffect` as the PS2's short-lived lightning strip (`arc.rs`,
//! card UORDE); the flamethrowers name none and draw no arc.
//! Cosmetic droid and vehicle chunks launch separately (`chunks`, named deviations
//! in FEEL_NOTES). Still to come: vehicle Damage / ChunkTrail / Thrust / Wake / Stomp
//! effects.
//!
//! The muzzle flash is not a particle effect (§7): it is a model the held
//! weapon draws (card FXMF, `crate::soldier_vis`), handed to
//! [`Fx::submit_with`] to share the particles' far-to-near list.
//!
//! **Placeholders** (PROVISIONAL, chosen to read clearly, nothing claims to
//! match the PS2): with no effect data baked at all, a struck bolt, a unit
//! hit and a destroyed vehicle draw coloured camera-facing quads.
//!
//! Visual only — driven by sim events, never feeding back into the sim.

mod abilities;
mod arc;
mod children;
pub mod chunks;
pub mod draw;
pub mod emitter;
mod flame;
mod flyer_spray;
mod force;
mod jet;
mod props;
mod repair;
pub mod ripple;
mod sky;
mod space_dust;
mod storm;
mod trail;
mod turret;
mod vehicle_damage;
mod water_contact;

use crate::walk::WalkSim;
use core::impact::{ExplosionLook, Surface, SPLASH_LIFT, WATER_BLAST_DEPTH};
use core::sim::Sim;
use core::unit::UnitHandle;
use core::vehicle::VehicleHandle;
use emitter::{Era, Frame, Instance, Lcg, Variant};
use glam::{Mat4, Vec3};
use manifest::{FxEmitter, FxSet};
use render::{Camera, DrawCmd, MeshHandle, ParticleDraw, Renderer, TextureHandle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The rifle impact every showcase bolt spawns (re-fx §7).
pub const BF1_IMPACT: &str = "ord_explosion";
pub const BF2_IMPACT: &str = "com_sfx_ord_exp";

// How far an ordnance's explosion sits off the hit (the sim's blast sits
// there too).
use core::weapon::EXPLOSION_OFFSET;

/// PROVISIONAL sizes (m) and lifetimes (s).
const IMPACT_SIZE: f32 = 0.45;
const IMPACT_LIFE: f32 = 0.22;
const HIT_SIZE: f32 = 0.34;
const HIT_LIFE: f32 = 0.18;
/// A destroyed vehicle's blast (card VDTH): a slow fireball, until card FXC
/// reads the class's own `Effect`. Its size grows with the vehicle's blast
/// sphere (`Sim::blast_setup`), floored at [`BLAST_MIN_SIZE`].
const BLAST_LIFE: f32 = 0.9;
const BLAST_MIN_SIZE: f32 = 3.0;
/// How far up a hit unit's capsule the spark sits.
const HIT_HEIGHT: f32 = 1.1;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// Bolt struck the terrain or a world model (`BoltExpired { struck }`).
    Impact,
    /// Bolt hit a unit.
    Hit,
    /// A vehicle was destroyed (`VehicleDestroyed`); the payload is the
    /// fireball's full size (m).
    Blast(f32),
}

#[derive(Debug, Clone, Copy)]
struct Puff {
    pos: Vec3,
    kind: Kind,
    team: u8,
    age: f32,
}

impl Puff {
    fn life(&self) -> f32 {
        match self.kind {
            Kind::Impact => IMPACT_LIFE,
            Kind::Hit => HIT_LIFE,
            Kind::Blast(_) => BLAST_LIFE,
        }
    }

    /// (size, colour) at this age. The colour's ALPHA is the fade: these draw
    /// through the renderer's blend pass, so an effect can thin out to nothing
    /// instead of shrinking and dimming (which is what it had to do when the
    /// renderer only alpha-tested). Colours stay at full brightness for most
    /// of the life and the alpha does the work — a flash that dims and stays
    /// opaque reads as a grey card in front of the scene.
    fn look(&self) -> (f32, [f32; 4]) {
        let t = (self.age / self.life()).clamp(0.0, 1.0);
        let bolt = if self.team == 0 {
            [1.0, 0.45, 0.15]
        } else {
            [0.35, 1.0, 0.35]
        };
        match self.kind {
            // Impact: expands and cools toward the bolt colour.
            Kind::Impact => (
                IMPACT_SIZE * (0.6 + 0.9 * t),
                [
                    bolt[0] * 0.6 + 0.4,
                    bolt[1] * 0.6 + 0.3,
                    bolt[2] * 0.6 + 0.25,
                    1.0 - t * t,
                ],
            ),
            // Unit hit: a quick warm spark.
            Kind::Hit => (HIT_SIZE * (0.7 + 0.7 * t), [1.0, 0.5, 0.2, 1.0 - t * t]),
            // Blast: swells fast, cools from white-yellow to deep orange.
            Kind::Blast(size) => (
                size * (0.35 + 0.65 * (1.0 - (1.0 - t) * (1.0 - t))),
                [1.0, 0.85 - 0.5 * t, 0.5 - 0.4 * t, 1.0 - t * t],
            ),
        }
    }
}

/// One baked effect, ready to run: its emitters with every nested
/// sibling template flattened in (a sibling is a second instance of the
/// same effect, re-fx §2.1 +0x214) and, per emitter, BF1's 20 variants.
#[derive(Debug)]
pub struct EffectDef {
    pub hash: u32,
    pub name: Option<String>,
    pub emitters: Vec<FxEmitter>,
    era: Era,
    /// Empty for BF2 (every particle its own seed).
    caches: Vec<Vec<Variant>>,
    children: Vec<Option<Arc<EffectDef>>>,
}

fn flatten(e: &FxEmitter, out: &mut Vec<FxEmitter>) {
    let mut own = e.clone();
    let sibling = own.sibling.take();
    out.push(own);
    if let Some(s) = sibling {
        flatten(&s, out);
    }
}

impl EffectDef {
    fn new(
        hash: u32,
        name: Option<String>,
        emitters: Vec<FxEmitter>,
        era: Era,
        rng: &mut Lcg,
    ) -> Self {
        // S+0x128: a per-spawner random set at construction.
        let caches = match era {
            Era::Bf1 => emitters
                .iter()
                .map(|em| emitter::variant_cache(&em.spawner, rng.step()))
                .collect(),
            Era::Bf2 => Vec::new(),
        };
        let children = emitters
            .iter()
            .enumerate()
            .map(|(i, em)| {
                em.geometry.child.as_ref().map(|child| {
                    let mut emitters = Vec::new();
                    flatten(child, &mut emitters);
                    let hash = hash.wrapping_add(i as u32 + 1);
                    Arc::new(Self::new(hash, None, emitters, era, &mut Lcg(hash)))
                })
            })
            .collect();
        Self {
            hash,
            name,
            emitters,
            era,
            caches,
            children,
        }
    }

    fn cache(&self, i: usize) -> &[Variant] {
        self.caches.get(i).map_or(&[][..], Vec::as_slice)
    }
}

/// The baked effects of one map, in lookup order (see the module doc).
#[derive(Debug)]
pub struct Library {
    pub era: Era,
    effects: HashMap<u32, Arc<EffectDef>>,
    beams: HashMap<u32, manifest::FxBeam>,
    lightning: HashMap<u32, manifest::FxLightning>,
    /// The layers' `textures/` dirs, map first.
    texture_dirs: Vec<PathBuf>,
    model_dirs: Vec<PathBuf>,
}

impl Library {
    fn read_set(dir: &Path) -> Option<FxSet> {
        let path = dir.join("effects.ron");
        if !pak::exists(&path) {
            return None;
        }
        match pak::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|t| ron::from_str::<FxSet>(&t).map_err(anyhow::Error::from))
        {
            Ok(set) => Some(set),
            Err(e) => {
                eprintln!("effects {}: {e:#}", path.display());
                None
            }
        }
    }

    /// `sets` highest layer first: an effect in two layers is the first's.
    pub fn from_sets(era: Era, sets: Vec<FxSet>, rng: &mut Lcg) -> Library {
        let mut effects = HashMap::new();
        let mut beams = HashMap::new();
        let mut lightning = HashMap::new();
        for set in sets {
            for b in set.beams {
                beams.entry(b.hash).or_insert(b);
            }
            for b in set.lightning {
                lightning.entry(b.hash).or_insert(b);
            }
            let effect_era = match set.game.as_str() {
                "bf1" => Era::Bf1,
                "bf2" => Era::Bf2,
                _ => era,
            };
            for e in set.effects {
                effects.entry(e.hash).or_insert_with(|| {
                    let mut emitters = Vec::new();
                    for em in &e.emitters {
                        flatten(em, &mut emitters);
                    }
                    Arc::new(EffectDef::new(
                        e.hash,
                        e.name.clone(),
                        emitters,
                        effect_era,
                        rng,
                    ))
                });
            }
        }
        Library {
            era,
            effects,
            beams,
            lightning,
            texture_dirs: Vec::new(),
            model_dirs: Vec::new(),
        }
    }

    /// The layers under `assets_dir` for the map baked at `map_dir`, whose
    /// teams are `factions` ("rep", "cis", …; a side with no effects dir is
    /// skipped). A layer that is missing or unreadable is reported and
    /// skipped.
    pub fn load(assets_dir: &Path, map_dir: &Path, game: &str, factions: &[&str]) -> Library {
        let era = if game.eq_ignore_ascii_case("bf2") {
            Era::Bf2
        } else {
            Era::Bf1
        };
        let game = game.to_ascii_lowercase();
        let mut dirs = vec![map_dir.to_path_buf()];
        for f in factions {
            dirs.push(
                assets_dir
                    .join(format!("side_{game}"))
                    .join("effects")
                    .join(f.to_ascii_lowercase()),
            );
        }
        dirs.push(assets_dir.join(format!("common_{game}")));
        // [ours] Aliased BF1 droid effects follow native layers, preserving
        // the native effects' construction-time random variant caches.
        if factions.contains(&"cis") {
            dirs.push(assets_dir.join("droid_effects"));
        }
        // [ours] The optional DLC heroes' effects, after every retail layer: a
        // shared name stays retail's, and retail's variant caches are unmoved.
        if game == "bf2"
            && factions.iter().any(|f| matches!(*f, "rep" | "cis"))
            && crate::dlc::installed(assets_dir)
        {
            dirs.push(assets_dir.join("side_bf2/effects/dlc"));
        }
        let mut sets = Vec::new();
        for d in &dirs {
            if let Some(set) = Self::read_set(d) {
                sets.push(set);
            }
        }
        let sbd = assets_dir.join("side_bf2/effects/cis");
        if game == "bf1" && factions.contains(&"cis") {
            if let Some(mut set) = Self::read_set(&sbd) {
                let hash = crate::classdb::fnv1a_lowercase("cis_sfx_sbd_missiles_red");
                set.effects.retain(|e| e.hash == hash);
                sets.push(set);
                dirs.push(sbd);
                dirs.push(assets_dir.join("common_bf2"));
            }
        }
        let mut lib = Library::from_sets(era, sets, &mut Lcg(0x5eed_f00d));
        lib.model_dirs = dirs.iter().map(|d| d.join("models")).collect();
        lib.texture_dirs = dirs.iter().map(|d| d.join("textures")).collect();
        let side = assets_dir.join(format!("side_{game}"));
        // Keep the BF1 chunk sheet apart from BF2's living SBD sheet.
        lib.model_dirs.push(side.join("droid_chunks/weapons"));
        lib.texture_dirs.push(side.join("droid_chunks/textures"));
        lib.model_dirs.push(side.join("weapons"));
        lib.texture_dirs.push(side.join("textures"));
        let mut vehicles = pak::read_dir(side.join("vehicles")).unwrap_or_default();
        vehicles.sort();
        for v in vehicles {
            lib.model_dirs.push(v.join("models"));
            lib.texture_dirs.push(v.join("textures"));
        }
        lib
    }

    pub fn get(&self, name: &str) -> Option<&Arc<EffectDef>> {
        self.effects.get(&crate::classdb::fnv1a_lowercase(name))
    }

    pub fn beam(&self, name: &str) -> Option<&manifest::FxBeam> {
        self.beams.get(&crate::classdb::fnv1a_lowercase(name))
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}

/// One spawned effect: its clock, its frame and one instance per emitter.
#[derive(Debug)]
pub struct LiveEffect {
    def: Arc<EffectDef>,
    /// Seconds since the spawn (the PS2's E+0xfc).
    pub t: f32,
    pub frame: Frame,
    prev_frame: Option<Frame>,
    instances: Vec<Instance>,
    stopped: bool,
    /// The vehicle it rides and its offset in the hull's frame: the PS2
    /// spawns an impact in the struck object's frame (B1 FUN_00276810
    /// B1:198040–). PROVISIONAL: only the origin follows (world axes kept).
    ride: Option<(VehicleHandle, Vec3)>,
    unit: Option<(UnitHandle, Gain)>,
    round: Option<trail::Round>,
    repair: Option<(UnitHandle, usize)>,
    buff: Option<(UnitHandle, usize)>,
    /// The jet soldier whose `JetEffect` this is (card JETP2, `fx/jet.rs`).
    jet: Option<UnitHandle>,
    turret: Option<(u64, usize)>,
    station: Option<(usize, usize)>,
    vehicle: Option<(VehicleHandle, usize)>,
    burn: Option<(UnitHandle, usize, usize)>,
    float: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gain {
    Health,
    Ammo,
    Energy,
    Activation,
}

impl LiveEffect {
    fn follow_unit(&mut self, unit: &core::unit::Unit) {
        let delta = unit.soldier.pos - self.frame.pos;
        for instance in &mut self.instances {
            instance.translate(delta);
        }
        self.frame.pos = unit.soldier.pos;
        self.frame.vel = unit.soldier.vel;
    }

    pub fn new(def: Arc<EffectDef>, frame: Frame, rng: &mut Lcg) -> LiveEffect {
        let era = def.era;
        let instances = def
            .emitters
            .iter()
            .map(|em| Instance::new(em, era, rng.step(), rng.step()))
            .collect();
        let mut fx = LiveEffect {
            def,
            t: 0.0,
            frame,
            prev_frame: None,
            instances,
            stopped: false,
            ride: None,
            unit: None,
            round: None,
            repair: None,
            buff: None,
            jet: None,
            turret: None,
            station: None,
            vehicle: None,
            burn: None,
            float: None,
        };
        fx.advance(0.0);
        fx
    }

    /// Advance the clock `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        let t0 = self.t;
        self.t += dt;
        let f1 = self.frame;
        let f0 = self.prev_frame.unwrap_or(f1);
        let interpolate = dt > 1e-6 && (f1.pos - f0.pos).length_squared() > 1e-6;
        for (inst, em) in self.instances.iter_mut().zip(&self.def.emitters) {
            if interpolate {
                inst.update_path(em, self.t, self.stopped, |time| {
                    let alpha = ((time - t0) / dt).clamp(0.0, 1.0);
                    Frame {
                        rot: f1.rot,
                        pos: f0.pos.lerp(f1.pos, alpha),
                        vel: f0.vel.lerp(f1.vel, alpha),
                    }
                });
            } else {
                inst.update(em, self.t, f1, self.stopped);
            }
        }
        if dt > 0.0 {
            self.prev_frame = Some(f1);
        }
    }

    /// No more bursts; the effect ends once its particles have.
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    pub fn is_dead(&self) -> bool {
        self.instances.iter().all(Instance::is_dead)
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub fn name(&self) -> Option<&str> {
        self.def.name.as_deref()
    }

    /// Particles emitted so far, per emitter.
    pub fn emitted(&self) -> Vec<u32> {
        self.instances.iter().map(Instance::emitted).collect()
    }

    /// Every emitter's live particles, unthinned (`keep` all).
    pub fn particles(&self) -> Vec<Vec<emitter::Particle>> {
        let era = self.def.era;
        self.instances
            .iter()
            .enumerate()
            .map(|(i, inst)| {
                let em = &self.def.emitters[i];
                inst.particles(em, era, self.t, self.frame, self.def.cache(i), |_| true)
            })
            .collect()
    }

    /// This effect's draws for the view at `eye` looking along `forward`.
    pub fn draws(
        &self,
        eye: Vec3,
        forward: Vec3,
        texture: &mut dyn FnMut(&str) -> Option<TextureHandle>,
        out: &mut Vec<ParticleDraw>,
    ) {
        self.draws_with(eye, forward, 1.0, &mut TextureOnly(texture), out);
    }

    fn draws_with(
        &self,
        eye: Vec3,
        forward: Vec3,
        zoom: f32,
        assets: &mut dyn FxAssets,
        out: &mut Vec<ParticleDraw>,
    ) {
        self.draws_with_bias(eye, forward, zoom, 0.0, assets, out);
    }

    pub(super) fn draws_with_bias(
        &self,
        eye: Vec3,
        forward: Vec3,
        zoom: f32,
        bias: f32,
        assets: &mut dyn FxAssets,
        out: &mut Vec<ParticleDraw>,
    ) {
        let era = self.def.era;
        let d = (self.frame.pos - eye).dot(forward);
        for (i, inst) in self.instances.iter().enumerate() {
            if inst.is_dead() {
                continue;
            }
            let em = &self.def.emitters[i];
            let terms = draw::view_terms_with_bias(d, em.bounding_radius, zoom, bias);
            if terms.fade <= 0.0 {
                continue;
            }
            if em.geometry.kind == manifest::FxType::Emitter {
                children::draw(self, i, era, eye, forward, zoom, bias, assets, out);
                continue;
            }
            if em.geometry.blend == manifest::FxBlend::Blur
                && em.geometry.kind != manifest::FxType::Particle
            {
                continue;
            }
            let ps = inst.particles(em, era, self.t, self.frame, self.def.cache(i), |k| {
                terms.keep(k)
            });
            if ps.is_empty() {
                continue;
            }
            let tex = em
                .geometry
                .texture
                .as_deref()
                .and_then(|n| assets.texture(&n.to_ascii_lowercase()));
            if em.geometry.kind == manifest::FxType::Geometry {
                if let Some(model) = em
                    .geometry
                    .model
                    .as_deref()
                    .and_then(|n| assets.model(&n.to_ascii_lowercase()))
                {
                    for p in &ps {
                        if p.rgba[3] > 0.0 {
                            model.draws(
                                draw::geometry_matrix(p),
                                draw::bytes(p, terms.alpha(p.idx)),
                                self.frame.pos,
                                em.geometry.blend == manifest::FxBlend::Additive,
                                era,
                                out,
                            );
                        }
                    }
                }
            } else {
                draw::emitter_draws(
                    em,
                    era,
                    &ps,
                    terms,
                    self.frame.pos,
                    self.frame.rot,
                    eye,
                    forward,
                    tex,
                    out,
                );
            }
        }
    }
}

pub trait FxAssets {
    fn texture(&mut self, name: &str) -> Option<TextureHandle>;
    fn model(&mut self, name: &str) -> Option<Arc<draw::EffectModel>>;
}

struct TextureOnly<'a>(&'a mut dyn FnMut(&str) -> Option<TextureHandle>);
impl FxAssets for TextureOnly<'_> {
    fn texture(&mut self, name: &str) -> Option<TextureHandle> {
        (self.0)(name)
    }
    fn model(&mut self, _: &str) -> Option<Arc<draw::EffectModel>> {
        None
    }
}

struct Lookup<'a> {
    r: &'a mut Renderer,
    lib: &'a Library,
    textures: &'a mut HashMap<String, Option<TextureHandle>>,
    models: &'a mut HashMap<String, Option<Arc<draw::EffectModel>>>,
}

impl FxAssets for Lookup<'_> {
    fn texture(&mut self, name: &str) -> Option<TextureHandle> {
        sheet(self.r, &self.lib.texture_dirs, self.textures, name)
    }

    /// `<layer>/models/<name>.glb` from the first layer that holds it, its
    /// sheets from any layer; None (once, with a line) when none does.
    fn model(&mut self, name: &str) -> Option<Arc<draw::EffectModel>> {
        if let Some(m) = self.models.get(name) {
            return m.clone();
        }
        let file = format!("{name}.glb");
        let glb = self
            .lib
            .model_dirs
            .iter()
            .map(|d| d.join(&file))
            .find(|p| pak::exists(p));
        let cpu = match glb.as_deref().map(crate::assets::load_glb) {
            Some(Ok(c)) => Some(c),
            Some(Err(e)) => {
                eprintln!("effect model {name}: {e:#} (drawn as nothing)");
                None
            }
            None => {
                eprintln!("effect model {name}: in no layer (drawn as nothing)");
                None
            }
        };
        let m = cpu.map(|cpu| {
            let mut parts = Vec::new();
            for (start, count, material, _, flags) in &cpu.submeshes {
                let tex = sheet(
                    self.r,
                    &self.lib.texture_dirs,
                    self.textures,
                    &material.to_ascii_lowercase(),
                );
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
                parts.push((tex, tris, *flags));
            }
            Arc::new(draw::EffectModel { parts })
        });
        self.models.insert(name.to_owned(), m.clone());
        m
    }
}

/// Effect sheet `name` (lowercase) from the first of `dirs` that holds it,
/// uploaded once into `cache`; None (once, with a line) when none does.
fn sheet(
    r: &mut Renderer,
    dirs: &[std::path::PathBuf],
    cache: &mut HashMap<String, Option<TextureHandle>>,
    name: &str,
) -> Option<TextureHandle> {
    if let Some(t) = cache.get(name) {
        return *t;
    }
    let t = dirs.iter().find_map(|d| {
        crate::viewer::upload_texture_file(r, d, name).unwrap_or_else(|e| {
            eprintln!("effect texture {name}: {e:#}");
            None
        })
    });
    if t.is_none() {
        eprintln!("effect texture {name}: in no layer (drawn as nothing)");
    }
    cache.insert(name.to_owned(), t);
    t
}

fn splash_at(pos: Vec3, level: f32) -> Vec3 {
    Vec3::new(pos.x, level + SPLASH_LIFT, pos.z)
}

/// The game's effects: the particle effects running and the placeholder
/// puffs. `ingest` once per sim tick, `update` once per rendered frame.
#[derive(Default)]
pub struct Fx {
    vehicle_health: Vec<Option<(u32, f32)>>,
    flyer_phases: Vec<Option<(u32, core::vehicle::flyer::FlightPhase)>>,
    space_dust: Option<space_dust::SpaceDust>,
    puffs: Vec<Puff>,
    library: Option<Library>,
    live: Vec<LiveEffect>,
    rng: Option<Lcg>,
    textures: HashMap<String, Option<TextureHandle>>,
    models: HashMap<String, Option<Arc<draw::EffectModel>>>,
    /// The `SoundName` of each effect an explosion spawned since the last
    /// [`Fx::take_sounds`], and where (card WSND).
    sounds: Vec<(String, Vec3)>,
    rounds: trail::Rounds,
    /// The drawn jet frames by unit index ([`Fx::set_jet_frames`]).
    jet_frames: Vec<Option<Mat4>>,
    burn_frames: Vec<Vec<Option<Mat4>>>,
    water: ripple::Water,
    water_contact: water_contact::State,
    chunks: chunks::Chunks,
    attached: Vec<props::Attached>,
    beams: Vec<props::Glow>,
    /// Emitter shots' lightning links (`Event::Emitter`, `arc.rs`).
    arcs: Vec<arc::Arc>,
    /// The sim tick last ingested; the arcs' clock.
    tick: u64,
    force: force::State,
    sky: Vec<sky::SkyEffect>,
    storm: Option<storm::Storm>,
    pub particle_lod_bias: f32,
}

impl std::fmt::Debug for Fx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fx")
            .field("puffs", &self.puffs.len())
            .field("effects", &self.library.as_ref().map_or(0, Library::len))
            .field("live", &self.live.len())
            .field("arcs", &self.arcs.len())
            .finish()
    }
}

impl Fx {
    pub fn extend_test_classes(&mut self, root: &Path, map_dir: &Path, game: &str, walk: &WalkSim) {
        let added = Library::load(root, map_dir, game, &crate::roster::FACTION_ORDER);
        if let Some(lib) = self.library.as_mut() {
            for (key, value) in added.effects {
                lib.effects.entry(key).or_insert(value);
            }
            for (key, value) in added.beams {
                lib.beams.entry(key).or_insert(value);
            }
            for (key, value) in added.lightning {
                lib.lightning.entry(key).or_insert(value);
            }
            for dir in added.texture_dirs {
                if !lib.texture_dirs.contains(&dir) {
                    lib.texture_dirs.push(dir);
                }
            }
            for dir in added.model_dirs {
                if !lib.model_dirs.contains(&dir) {
                    lib.model_dirs.push(dir);
                }
            }
        } else {
            self.library = Some(added);
        }
        self.textures.retain(|_, texture| texture.is_some());
        self.models.retain(|_, model| model.is_some());
        if let Ok(db) =
            crate::roster::load_classdb(&root.join("classes"), game, &crate::roster::FACTION_ORDER)
        {
            self.force.extend(&db, &walk.sim);
            self.water_contact.load(&db);
            if let Some(lib) = self.library.as_ref() {
                let names: Vec<_> = walk
                    .teams
                    .iter()
                    .flat_map(|t| t.classes.iter().map(|c| c.name.clone()))
                    .collect();
                self.chunks.extend(&db, &names, &lib.model_dirs, lib.era);
            }
        }
    }

    /// Effects over `library` (no placeholder puffs yet).
    pub fn with_library(library: Library) -> Fx {
        Fx {
            library: Some(library),
            ..Fx::default()
        }
    }

    /// The map's effect layers (see [`Library::load`]).
    pub fn load(assets_dir: &Path, map_dir: &Path, game: &str, factions: &[&str]) -> Fx {
        let mut fx = Fx::with_library(Library::load(assets_dir, map_dir, game, factions));
        fx.space_dust = space_dust::SpaceDust::load(map_dir);
        fx
    }

    /// The effect layers of the map at `map_dir` for `walk`'s two sides.
    pub fn for_walk(assets_dir: &Path, map_dir: &Path, game: &str, walk: &WalkSim) -> Fx {
        let mut factions = vec![
            walk.teams[0].faction.as_str(),
            walk.teams[1].faction.as_str(),
        ];
        if game.eq_ignore_ascii_case("bf2")
            && (walk.sim.heroes.is_some() || walk.sim.deathmatch.is_some())
        {
            for side in ["all", "imp", "rep", "cis"] {
                if !factions.contains(&side) {
                    factions.push(side);
                }
            }
        }
        let mut fx = Fx::load(assets_dir, map_dir, game, &factions);
        fx.particle_lod_bias = walk.particle_lod_bias;
        if let (Some(lib), Ok(mut db)) = (
            fx.library.as_ref(),
            crate::roster::load_classdb(&assets_dir.join("classes"), game, &factions),
        ) {
            if let Err(e) = db.load_map(map_dir) {
                eprintln!("chunk classes: {e:#}");
            }
            let mut names = walk.vehicle_classes.clone();
            names.extend(
                walk.teams
                    .iter()
                    .flat_map(|t| t.classes.iter().map(|c| c.name.clone())),
            );
            names.extend(walk.sim.station_classes.iter().map(|s| s.name.clone()));
            names.sort();
            names.dedup();
            fx.chunks = chunks::Chunks::load_for(
                &db,
                &names,
                &lib.model_dirs,
                lib.era,
                walk.space_battle(),
            );
            fx.force = force::State::load(&db, &walk.sim);
            fx.water_contact.load(&db);
        }
        fx
    }

    /// Replace the static prop attachments when a scene or mode is loaded.
    pub fn set_scene_effects(&mut self, scene: &crate::assets::Scene) {
        (self.attached, self.beams) = self
            .library
            .as_ref()
            .map_or_else(Default::default, |lib| props::load(scene, lib));
        self.sky = self
            .library
            .as_ref()
            .map_or_else(Vec::new, |lib| sky::load(scene, lib));
        self.storm = storm::Storm::load(&scene.manifest);
    }

    pub fn library(&self) -> Option<&Library> {
        self.library.as_ref()
    }

    /// Start the effect `name` at `frame`; false when no layer holds it.
    pub fn spawn(&mut self, name: &str, frame: Frame) -> bool {
        self.spawn_hash(crate::classdb::fnv1a_lowercase(name), frame)
    }

    fn spawn_hash(&mut self, hash: u32, frame: Frame) -> bool {
        let Some(lib) = &self.library else {
            return false;
        };
        let Some(def) = lib.effects.get(&hash) else {
            return false;
        };
        let rng = self.rng.get_or_insert(Lcg(0x0bf2_5eed));
        self.live.push(LiveEffect::new(def.clone(), frame, rng));
        true
    }

    /// [`Fx::spawn`] riding vehicle `h` of `sim` (its origin follows the hull).
    fn spawn_riding(&mut self, name: &str, pos: Vec3, h: VehicleHandle, sim: &Sim) -> bool {
        if !self.spawn(name, Frame::at(pos)) {
            return false;
        }
        if let Some(v) = sim.vehicle(h) {
            let local = v.rot.inverse() * (pos - v.pos);
            if let Some(e) = self.live.last_mut() {
                e.ride = Some((h, local));
            }
        }
        true
    }

    fn spawn_floating(&mut self, name: &str, pos: Vec3, level: f32) -> bool {
        if !self.spawn(name, Frame::at(splash_at(pos, level))) {
            return false;
        }
        if let Some(e) = self.live.last_mut() {
            e.float = Some(level);
        }
        true
    }

    fn gain(&mut self, unit: UnitHandle, kind: Gain, names: [&str; 2], sim: &Sim) {
        if self
            .live
            .iter()
            .any(|e| e.unit == Some((unit, kind)) && !e.is_dead())
        {
            return;
        }
        let Some(u) = sim.unit(unit) else { return };
        let frame = Frame {
            vel: u.soldier.vel,
            ..Frame::at(u.soldier.pos)
        };
        let bf2 = self.library.as_ref().is_some_and(|l| l.era == Era::Bf2);
        for index in [usize::from(bf2), usize::from(!bf2)] {
            if !names[index].is_empty() && self.spawn(names[index], frame) {
                if let Some(e) = self.live.last_mut() {
                    e.unit = Some((unit, kind));
                }
                break;
            }
        }
    }

    /// Detonate an explosion class's look at `pos` (B1 FUN_002701a0
    /// B1:193756–193807): its `Effect` — BF2 one of its rows at random (B2
    /// FUN_0030c8f8), or at or below `water` + 0.3 m its `WaterEffect` on the
    /// surface, and only with a damage radius. False when nothing spawned.
    pub fn explode(&mut self, look: &ExplosionLook, pos: Vec3, water: Option<f32>) -> bool {
        let (spawned, pos) = match water.filter(|w| pos.y <= w + WATER_BLAST_DEPTH) {
            Some(w) => {
                if look.radius_outer <= 0.0 {
                    return false;
                }
                let s = (look.radius_outer * 0.5).min(0.4);
                self.water.spawn((3.0 + s, s), Vec3::new(pos.x, w, pos.z));
                let Some(name) = &look.water_effect else {
                    return false;
                };
                (self.spawn_floating(name, pos, w), splash_at(pos, w))
            }
            None => {
                let pick = match look.effects.len() {
                    0 => return false,
                    1 => 0,
                    n => {
                        let rng = self.rng.get_or_insert(Lcg(0x0bf2_5eed));
                        ((rng.float() * n as f32) as usize).min(n - 1)
                    }
                };
                (self.spawn(&look.effects[pick], Frame::at(pos)), pos)
            }
        };
        if !spawned {
            return false;
        }
        self.queue_last_effect_sound(pos);
        true
    }

    /// Queue the `SoundName` (`"<name> defer"`, re-weapon-sounds.md §3.1, §4.2)
    /// of the most recently spawned one-shot effect (explosions and surface/shield impacts).
    pub(crate) fn queue_last_effect_sound(&mut self, pos: Vec3) {
        let sound = self.live.last().and_then(|e| {
            let mut next = e.def.emitters.iter().collect::<Vec<_>>();
            while let Some(em) = next.pop() {
                // Most emitters carry an empty name.
                let name = em.sound_name.as_deref();
                if let Some(s) = name.and_then(|s| s.split_whitespace().next()) {
                    return Some(s.to_owned());
                }
                next.extend(em.sibling.as_deref());
                next.extend(em.geometry.child.as_deref());
            }
            None
        });
        if let Some(s) = sound {
            self.sounds.push((s, pos));
        }
    }

    /// The sounds the explosions' effects started since the last call, for
    /// the tick's sound pass (`crate::event_sounds`).
    pub fn take_sounds(&mut self) -> Vec<(String, Vec3)> {
        std::mem::take(&mut self.sounds)
    }

    /// Is any effect data baked? Without it the placeholder quads stand in.
    fn has_effects(&self) -> bool {
        self.library.as_ref().is_some_and(|l| !l.is_empty())
    }

    /// The running particle effects.
    pub fn live(&self) -> &[LiveEffect] {
        &self.live
    }

    pub fn chunk_models(&self) -> Vec<String> {
        let mut names: Vec<String> = self.chunks.models().map(str::to_ascii_lowercase).collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn set_lit_chunks(&mut self, names: impl IntoIterator<Item = String>) {
        self.chunks.set_lit(names);
    }

    pub fn lit_chunk_poses(&self) -> impl Iterator<Item = (&str, Mat4, f32)> {
        self.chunks.lit_poses()
    }

    /// Upload every sheet and model the library’s effects can draw (card HITCH): at the
    /// battle's load, so no effect's first spawn reads a file and makes a
    /// texture inside a frame. Returns how many were wanted.
    pub fn preload(&mut self, r: &mut Renderer) -> usize {
        let Some(lib) = &self.library else {
            return 0;
        };
        let mut names: Vec<String> = Vec::new();
        let mut models: Vec<String> = self.chunks.models().map(str::to_ascii_lowercase).collect();
        // The emitter arcs' sheets are not in the effect table (no
        // `LightningBolt` is baked), so the record's class name is not
        // reached by the loop below (card UORDE, `arc.rs`).
        names.push(arc::TEXTURES[usize::from(lib.era == Era::Bf2)].to_owned());
        for def in lib.effects.values() {
            let mut next: Vec<&FxEmitter> = def.emitters.iter().collect();
            while let Some(em) = next.pop() {
                if let Some(n) = em.geometry.texture.as_deref() {
                    let n = n.to_ascii_lowercase();
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
                if let Some(n) = &em.geometry.model {
                    models.push(n.to_ascii_lowercase());
                }
                next.extend(em.sibling.as_deref());
                next.extend(em.geometry.child.as_deref());
            }
        }
        names.extend(lib.lightning.values().filter_map(|b| b.texture.clone()));
        if self.water.active() {
            names.push(ripple::TEXTURE.to_owned());
        }
        names.sort();
        for n in &names {
            sheet(r, &lib.texture_dirs, &mut self.textures, n);
        }
        models.sort();
        models.dedup();
        let mut assets = Lookup {
            r,
            lib,
            textures: &mut self.textures,
            models: &mut self.models,
        };
        for name in models {
            assets.model(&name);
        }
        names.len()
    }

    /// The rifle impact of this library's game (see the module doc).
    fn impact_name(&self) -> &'static str {
        match self.library.as_ref().map(|l| l.era) {
            Some(Era::Bf2) => BF2_IMPACT,
            _ => BF1_IMPACT,
        }
    }

    /// Hand the renderer this frame's particles for the view `camera`,
    /// uploading any sheet not yet loaded.
    pub fn submit(&mut self, r: &mut Renderer, camera: &Camera) {
        self.submit_with(r, camera, Vec::new());
    }

    /// [`Fx::submit`] with `extra` draws sorted into the same list — the
    /// muzzle flashes (`SoldierVisuals::flash_draws`).
    pub fn submit_with(&mut self, r: &mut Renderer, camera: &Camera, extra: Vec<ParticleDraw>) {
        self.submit_zoomed(r, camera, 1.0, extra);
    }

    /// Submit a battle frame with camera zoom and attached trail heads at the
    /// same interpolated positions as the in-flight models. Tick frames remain intact.
    pub fn submit_walk(
        &mut self,
        r: &mut Renderer,
        camera: &Camera,
        walk: &WalkSim,
        extra: Vec<ParticleDraw>,
    ) {
        self.particle_lod_bias = walk.particle_lod_bias;
        let saved = self.drawn_round_frames(&walk.sim);
        self.submit_zoomed(r, camera, walk.zoom_s, extra);
        self.restore_round_frames(saved);
    }

    /// Submit with the camera's PS2 zoom scale (particle far fade and density).
    pub fn submit_zoomed(
        &mut self,
        r: &mut Renderer,
        camera: &Camera,
        zoom: f32,
        extra: Vec<ParticleDraw>,
    ) {
        let mut out = extra;
        let mut sky_out = Vec::new();
        let time = r.atmosphere_time();
        r.set_lightning(self.storm.as_mut().and_then(|storm| {
            storm.advance(time, camera, self.library.as_ref());
            storm.flash(r.lighting)
        }));
        let lift = self.ride_the_water(crate::water_anim::clock(r.water_time) as f32);
        let bias = self.particle_lod_bias;
        if let Some(lib) = &self.library {
            let mut assets = Lookup {
                r,
                lib,
                textures: &mut self.textures,
                models: &mut self.models,
            };
            if let Some(dust) = &self.space_dust {
                dust.draws(camera, &mut assets, &mut out);
            }
            for e in &self.live {
                e.draws_with_bias(
                    camera.position,
                    camera.forward(),
                    zoom,
                    bias,
                    &mut assets,
                    &mut out,
                );
            }
            self.chunks.draws(
                lib.era,
                camera.position,
                camera.forward(),
                zoom,
                bias,
                &mut assets,
                &mut out,
            );
            for attached in &self.attached {
                if let Some(effect) = &attached.live {
                    effect.draws_with_bias(
                        camera.position,
                        camera.forward(),
                        zoom,
                        bias,
                        &mut assets,
                        &mut out,
                    );
                }
            }
            for effect in &self.sky {
                effect.draws(camera, time, zoom, &mut assets, &mut sky_out);
            }
            if let Some(storm) = &self.storm {
                out.extend(storm.bolts.draws(camera, |name| assets.texture(name)));
            }
            let sheet = assets.texture(ripple::TEXTURE);
            self.water.draws(camera.position, lift, sheet, &mut out);
            // The emitter shots' links: the class's `LightningEffect` arc
            // (card UORDE, `arc.rs`).
            for a in &self.arcs {
                let tex = assets.texture(a.class());
                a.draws(self.tick, camera, tex, &mut out);
            }
        }
        out.extend(self.beams.iter().map(props::Glow::draw));
        if let Some(lib) = &self.library {
            out.extend(self.force.draws(camera, |name| {
                sheet(r, &lib.texture_dirs, &mut self.textures, name)
            }));
        }
        r.set_particles(out);
        r.set_sky_particles(sky_out);
    }

    /// Ingest the tick using the last drawn soldier pose for chunk launch bones.
    pub fn ingest_posed(
        &mut self,
        walk: &WalkSim,
        vis: Option<&crate::soldier_vis::SoldierVisuals>,
    ) {
        if let Some(vis) = vis {
            self.chunks.pose_bursts(walk, vis);
        }
        self.ingest(walk);
        if walk.sim.water_level.is_some() {
            if let Some(vis) = vis {
                self.soldier_water_feet(walk, &vis.water_feet(walk));
            }
        }
    }

    /// Read this tick's sim events into effects.
    pub fn ingest(&mut self, walk: &WalkSim) {
        self.ingest_sim(&walk.sim);
        self.soldier_water(walk);
        self.chunks.tick(walk, self.library.as_ref());
        self.follow_jets(&walk.sim, |u| {
            walk.team_template(u.team)
                .class(u.spec.class)
                .jet_look
                .effect
                .as_deref()
        });
    }

    pub fn update(&mut self, dt: f32) {
        self.update_with_round_frames(dt, Vec::new());
    }

    fn update_with_round_frames(&mut self, dt: f32, saved: Vec<(usize, Frame)>) {
        for p in &mut self.puffs {
            p.age += dt;
        }
        self.puffs.retain(|p| p.age < p.life());
        for e in &mut self.live {
            e.advance(dt);
        }
        for (index, frame) in saved {
            self.live[index].frame = frame;
        }
        self.live.retain(|e| !e.is_dead());
        self.water.update(dt);
        self.chunks.update(dt);
        self.force.update(dt);
        for effect in &mut self.attached {
            effect.step(dt);
        }
        for effect in &mut self.sky {
            effect.update(dt);
        }
    }

    /// Live placeholder puffs plus running particle effects.
    pub fn len(&self) -> usize {
        self.puffs.len()
            + self.live.len()
            + self.arcs.len()
            + self.chunks.len()
            + self.force_bolts()
            + self.attached.iter().filter(|a| a.live.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.puffs.clear();
        self.live.clear();
        self.sounds.clear();
        self.rounds = trail::Rounds::default();
        self.water.clear();
        self.water_contact.clear();
        self.chunks.clear();
        self.attached.clear();
        self.beams.clear();
        self.arcs.clear();
        self.force.clear();
        self.sky.clear();
        self.storm = None;
    }

    /// Camera-facing quads for the live effects. `quad` is the unit quad from
    /// `viewer::upload_billboard_quad` (its normals face the sun, so the
    /// lambert term is constant and an effect does not dim as you turn);
    /// `sprite` is `viewer::upload_fx_sprite`, a soft round falloff. Both are
    /// optional-free: pass what you uploaded.
    pub fn draws(
        &self,
        quad: MeshHandle,
        sprite: Option<TextureHandle>,
        camera: &Camera,
    ) -> Vec<DrawCmd> {
        self.puffs
            .iter()
            .map(|p| {
                let (model, color) = billboard(p, camera);
                DrawCmd {
                    mesh: quad,
                    texture: sprite,
                    color,
                    model,
                    // Composited, not alpha-tested: an effect is light, not
                    // a cutout. The blend pass sorts these back-to-front
                    // with the rest of the transparent draws and writes no
                    // depth, so overlapping puffs add up instead of one
                    // punching a hole in the others.
                    layer: render::DrawLayer::Blend,
                    additive: false,
                    // An effect puff is a near-scene draw and takes no
                    // far-scene cull; the map's cull is applied to the
                    // world, the terrain and the water (`viewer::far_cull`).
                    cull: None,
                    // and no near edge either — that is the `LOWR` sheet's
                    // alone (`viewer::lowres_range`).
                    cull_near: None,
                    lod: None,
                    light: render::LightPath::Lit,
                    fade: None,
                    lit: None,
                    region_lit: None,
                }
            })
            .collect()
    }
}

/// Transform + colour for one puff, facing `camera`.
fn billboard(p: &Puff, camera: &Camera) -> (Mat4, [f32; 4]) {
    let right = camera.right();
    let up = right.cross(camera.forward()).normalize_or_zero();
    let (size, color) = p.look();
    // Third column = the billboard's own normal (toward the camera), so the
    // matrix is not singular and the winding stays as authored.
    let basis = glam::Mat3::from_cols(right * size, up * size, -camera.forward() * size);
    (
        Mat4::from_translation(p.pos) * Mat4::from_mat3(basis),
        color,
    )
}

mod ingest;

#[cfg(test)]
mod tests;
