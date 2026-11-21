//! `fx__` particle effects → `effects.ron` (card FXP1, `analysis/re-fx.md`
//! §1, [O]): each LVL layer's effects as `manifest::FxSet`, keyed by NAME hash
//! with the cracked name where known.
//!
//! The effects live in three layers and the bake carries all three (re-fx
//! §1: the showcase missions name 25 side-LVL and 37 map-LVL effects besides
//! the common ones): **common** (BF1 `COMMON.LVL` 109 of 128, BF2 `INGAME.LVL`
//! 162 of 292 — `crate::map::bake_common` writes `common_<game>/effects.ron`),
//! **maps** (`crate::map::bake_map` → `<map>/effects.ron`) and **sides**
//! ([`bake_side`] → `side_<game>/effects/<side>/`, BF2 only in practice: BF1's
//! side LVLs hold none). An effect is kept once per layer by NAME, first copy
//! wins (BF2 sides repeat one per class sub-level; the copies are compared
//! and a differing one is announced).
//!
//! Resources: the textures an effect names are drawn blended by their alpha
//! (GS ALPHA 0x44 / 0x48, re-fx §6.7), so the map bake keeps their disc alpha
//! ([`Collected::textures`] joins `keep_alpha`'s `consumer_named` set) and the
//! side bake writes them with it. A name this layer's LVL does not hold is
//! reported — the runtime resolves it from a lower layer (map → side →
//! common), as the PS2's one texture table does.

use anyhow::{anyhow, Result};
use manifest::{
    FxBeam, FxBlend, FxColorNode, FxColorOp, FxEffect, FxEmitter, FxGeometry, FxLightning,
    FxPositionNode, FxPositionOp, FxRange, FxScalarNode, FxScalarOp, FxSet, FxShape, FxShield,
    FxSpawner, FxTransformer, FxType,
};
use mungefs::fx::LightningBoltConfig;
use mungefs::lightbeam::LightBeamConfig;
use mungefs::particle::{
    BlendMode, ColorMode, ColorOp, Effect, Emitter, GeometryType, PositionOp, Range, ScalarOp,
    Shape,
};
use mungefs::shield::ShieldConfig;
use std::collections::HashMap;
use std::path::Path;

/// The file every layer writes.
pub const EFFECTS_FILE: &str = "effects.ron";

/// The cracked effect names (`effect_names.txt`), by hash.
pub fn names() -> HashMap<u32, &'static str> {
    include_str!("effect_names.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|n| (mungefs::fnv1a_lowercase(n), n))
        .collect()
}

/// One layer's particle effects, deduplicated by NAME.
#[derive(Default)]
pub struct Collected {
    pub effects: Vec<Effect>,
    pub beams: Vec<LightBeamConfig>,
    pub lightning: Vec<LightningBoltConfig>,
    /// `Shield` records (`mungefs::shield`). Collected always, written only
    /// when [`Collected::write_shields`] is set (the DLC layer): a retail
    /// layer's droideka shell is drawn from its class, and its file keeps
    /// the bytes it had.
    pub shields: Vec<ShieldConfig>,
    pub write_shields: bool,
    /// Copies of an already-seen NAME whose tree differs from the first.
    pub differing_copies: Vec<u32>,
}

impl Collected {
    /// Every `fx__` particle effect among `chunks`, in order.
    pub fn from_chunks<'a>(
        chunks: impl IntoIterator<Item = &'a mungefs::Chunk<'a>>,
    ) -> Result<Collected> {
        let mut out = Collected::default();
        for c in chunks {
            if &c.tag != b"fx__" {
                continue;
            }
            for bolt in LightningBoltConfig::parse_all(c)
                .map_err(|err| anyhow!("fx__ 0x{:X}: {err}", c.offset))?
            {
                if !out
                    .lightning
                    .iter()
                    .any(|b| b.name.eq_ignore_ascii_case(&bolt.name))
                {
                    out.lightning.push(bolt);
                }
            }
            if let Some(sh) =
                ShieldConfig::parse(c).map_err(|err| anyhow!("fx__ 0x{:X}: {err}", c.offset))?
            {
                if !out
                    .shields
                    .iter()
                    .any(|x| x.name.eq_ignore_ascii_case(&sh.name))
                {
                    out.shields.push(sh);
                }
            }
            if let Some(b) =
                LightBeamConfig::parse(c).map_err(|err| anyhow!("fx__ 0x{:X}: {err}", c.offset))?
            {
                if !out
                    .beams
                    .iter()
                    .any(|x| x.name.eq_ignore_ascii_case(&b.name))
                {
                    out.beams.push(b);
                }
                continue;
            }
            let Some(e) =
                Effect::parse(c).map_err(|err| anyhow!("fx__ 0x{:X}: {err}", c.offset))?
            else {
                continue;
            };
            match out.effects.iter().find(|x| x.name_hash == e.name_hash) {
                Some(first) => {
                    if *first != e && !out.differing_copies.contains(&e.name_hash) {
                        out.differing_copies.push(e.name_hash);
                    }
                }
                None => out.effects.push(e),
            }
        }
        Ok(out)
    }

    /// Every texture the effects draw, lowercased, deduplicated.
    pub fn textures(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for e in &self.effects {
            for t in e.resources().0 {
                let t = t.to_ascii_lowercase();
                if !out.contains(&t) {
                    out.push(t);
                }
            }
        }
        for b in &self.lightning {
            if let Some(t) = &b.texture {
                let t = t.to_ascii_lowercase();
                if !out.contains(&t) {
                    out.push(t);
                }
            }
        }
        out
    }

    /// Every model the effects draw (Type GEOMETRY), lowercased.
    pub fn models(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for e in &self.effects {
            for m in e.resources().1 {
                let m = m.to_ascii_lowercase();
                if !out.contains(&m) {
                    out.push(m);
                }
            }
        }
        out
    }

    /// Write `<outdir>/effects.ron` (removed when there is no effect) and print
    /// the layer's count, its differing copies and the resources `have` (the
    /// layer's own textures / models, lowercased) lacks.
    pub fn write(
        &self,
        outdir: &Path,
        game: &str,
        have_textures: &[String],
        have_models: &[String],
    ) -> Result<Option<FxSet>> {
        let shields: Vec<FxShield> = if self.write_shields {
            self.shields.iter().map(shield).collect()
        } else {
            Vec::new()
        };
        if self.effects.is_empty()
            && self.beams.is_empty()
            && self.lightning.is_empty()
            && shields.is_empty()
        {
            // A rebake into a used directory must not keep a stale file.
            let _ = std::fs::remove_file(outdir.join(EFFECTS_FILE));
            return Ok(None);
        }
        let names = names();
        let set = FxSet {
            game: game.to_owned(),
            effects: self.effects.iter().map(|e| effect(e, &names)).collect(),
            beams: self.beams.iter().filter_map(beam).collect(),
            lightning: self.lightning.iter().map(lightning).collect(),
            shields,
        };
        crate::write_ron(&outdir.join(EFFECTS_FILE), &set)?;
        let emitters: usize = self.effects.iter().map(|e| e.all_emitters().len()).sum();
        let named = set.effects.iter().filter(|e| e.name.is_some()).count();
        eprintln!(
            "effects: {} particle effect(s), {emitters} emitter(s), {named} named -> {EFFECTS_FILE}",
            set.effects.len()
        );
        if !set.beams.is_empty() {
            eprintln!("effects: {} light beam(s)", set.beams.len());
        }
        if !set.lightning.is_empty() {
            eprintln!("effects: {} lightning definition(s)", set.lightning.len());
        }
        if !set.shields.is_empty() {
            eprintln!("effects: {} shield record(s)", set.shields.len());
        }
        for b in self.beams.iter().filter(|b| b.color.is_none()) {
            eprintln!("effects: light beam {} authors no Color, not drawn", b.name);
        }
        if !self.differing_copies.is_empty() {
            eprintln!(
                "effects: {} NAME(s) have a later copy that differs from the first (first kept): {:08x?}",
                self.differing_copies.len(),
                self.differing_copies
            );
        }
        let missing_t: Vec<String> = self
            .textures()
            .into_iter()
            .filter(|t| !have_textures.contains(t))
            .collect();
        let missing_m: Vec<String> = self
            .models()
            .into_iter()
            .filter(|m| !have_models.contains(m))
            .collect();
        if !missing_t.is_empty() || !missing_m.is_empty() {
            eprintln!(
                "effects: named here, held by a lower layer (or by no LVL): textures {missing_t:?}, models {missing_m:?}"
            );
        }
        Ok(Some(set))
    }
}

/// A side LVL's effects → `<outdir>/effects.ron` + the `textures/` and
/// `models/` they name that the side LVL holds (every sub-level flattened,
/// `mungefs::content_chunks`). PS2 textures only: the classic-plus tier is
/// not looked up for effect sheets. Writes nothing for a side with none.
pub fn bake_side(lvl_path: &str, outdir: &str, game: &str) -> Result<Option<FxSet>> {
    bake_side_with(lvl_path, outdir, game, false)
}

/// [`bake_side`], also writing the layer's `Shield` records when `shields`
/// (the DLC heroes' layer: the Force ball's `fisto_bubble`).
pub fn bake_side_with(
    lvl_path: &str,
    outdir: &str,
    game: &str,
    shields: bool,
) -> Result<Option<FxSet>> {
    let buf = std::fs::read(lvl_path).map_err(|e| anyhow!("{lvl_path}: {e}"))?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
    let chunks = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
    let mut fx = Collected::from_chunks(chunks.iter())?;
    fx.write_shields = shields;
    if fx.effects.is_empty() && fx.beams.is_empty() && fx.lightning.is_empty() {
        eprintln!("effects: {lvl_path} holds no particle effect");
        // As `Collected::write`: no stale file from an earlier bake.
        let _ = std::fs::remove_file(Path::new(outdir).join(EFFECTS_FILE));
        return Ok(None);
    }
    let out = Path::new(outdir);
    std::fs::create_dir_all(out.join("models"))?;

    // Models first: their segments' textures are wanted too.
    let wanted_models = fx.models();
    let mut models: Vec<mungefs::model::Model> = Vec::new();
    for c in chunks.iter().filter(|c| &c.tag == b"modl") {
        let m =
            mungefs::model::Model::parse(c).map_err(|e| anyhow!("modl 0x{:X}: {e}", c.offset))?;
        let lower = m.name.to_ascii_lowercase();
        if wanted_models.contains(&lower)
            && !models.iter().any(|k| k.name.eq_ignore_ascii_case(&m.name))
        {
            models.push(m);
        }
    }
    let effect_tex = fx.textures();
    let model_tex: Vec<String> = models
        .iter()
        .flat_map(|m| m.segments.iter())
        .filter_map(crate::alpha::segment_texture)
        .map(str::to_ascii_lowercase)
        .collect();
    let mut decoded: Vec<mungefs::texture::Texture> = Vec::new();
    for c in chunks.iter().filter(|c| &c.tag == b"tex_") {
        if let Ok(t) = mungefs::texture::Texture::parse(c) {
            if model_tex.contains(&t.name.to_ascii_lowercase())
                && !decoded.iter().any(|k| k.name.eq_ignore_ascii_case(&t.name))
            {
                decoded.push(t);
            }
        }
    }
    let table = crate::alpha::AlphaTable::new(&decoded);
    let mut have_models = Vec::new();
    for m in &models {
        let alphas = table.model(m);
        let glb = crate::glb::model_to_glb(m, &alphas, false)?;
        std::fs::write(out.join("models").join(format!("{}.glb", m.name)), glb)?;
        have_models.push(m.name.to_ascii_lowercase());
    }
    // An effect sheet keeps its alpha (the blend reads it), and so does an
    // effect model's: a segment blends by it whenever the tint is not opaque.
    let mut wanted = effect_tex.clone();
    for t in &model_tex {
        if !wanted.contains(t) {
            wanted.push(t.clone());
        }
    }
    let flags: HashMap<String, u32> = wanted
        .iter()
        .map(|t| (t.clone(), crate::detail::ALPHA_IS_MEANINGFUL))
        .collect();
    let have_textures = crate::soldier::bake_textures_flagged(inner, &wanted, &flags, out, None)?;
    fx.write(out, game, &have_textures, &have_models)
}

fn shield(s: &ShieldConfig) -> FxShield {
    FxShield {
        hash: mungefs::fnv1a_lowercase(&s.name),
        name: s.name.clone(),
        shape: s.shape.clone(),
        texture: s.texture.clone(),
        color: s.color,
        specular_color: s.specular_color,
        radius: s.radius,
        height: s.height,
        turn_on_time: s.turn_on_time,
        turn_off_time: s.turn_off_time,
        scroll_rate: s.scroll_rate,
        unread_b28af6f4: s.unread_b28af6f4,
    }
}

fn lightning(b: &LightningBoltConfig) -> FxLightning {
    FxLightning {
        hash: mungefs::fnv1a_lowercase(&b.name),
        name: b.name.clone(),
        texture: b.texture.clone(),
        width: b.width,
        fade_time: b.fade_time,
        break_distance: b.break_distance,
        texture_size: b.texture_size,
        spread_factor: b.spread_factor,
        max_branches: b.max_branches,
        branch_factor: b.branch_factor,
        branch_spread_factor: b.branch_spread_factor,
        branch_length: b.branch_length,
        interpolation_speed: b.interpolation_speed,
        num_children: b.num_children,
        child_break_distance: b.child_break_distance,
        child_texture_size: b.child_texture_size,
        child_width: b.child_width,
        child_spread_factor: b.child_spread_factor,
        color: b.color,
        child_color: b.child_color,
    }
}

fn beam(b: &LightBeamConfig) -> Option<FxBeam> {
    Some(FxBeam {
        hash: mungefs::fnv1a_lowercase(&b.name),
        name: b.name.clone(),
        color: b.color?,
        flare_intensity: b.flare_intensity,
        length: b.length,
        initial_radius: b.initial_radius,
        final_radius: b.final_radius,
        fade_length: b.fade_length,
        fade_factor: b.fade_factor,
        draw_distance: b.draw_distance,
        position: [b.position[0], b.position[1], -b.position[2]],
        direction: [b.direction[0], b.direction[1], -b.direction[2]],
    })
}

fn range(r: Range) -> FxRange {
    FxRange(r.base, r.span)
}

fn ranges<const N: usize>(r: [Range; N]) -> [FxRange; N] {
    r.map(range)
}

fn effect(e: &Effect, names: &HashMap<u32, &'static str>) -> FxEffect {
    FxEffect {
        hash: e.name_hash,
        name: names.get(&e.name_hash).map(|n| n.to_string()),
        emitters: e.emitters.iter().map(emitter).collect(),
    }
}

fn emitter(e: &Emitter) -> FxEmitter {
    let s = &e.spawner;
    let t = &e.transformer;
    let g = &e.geometry;
    FxEmitter {
        label: e.label.clone(),
        start_delay: range(e.start_delay),
        burst_delay: range(e.burst_delay),
        burst_count: range(e.burst_count),
        max_particles: range(e.max_particles),
        color: ranges(e.color),
        hsv: e.color_mode == ColorMode::Hsv,
        size: range(e.size),
        spawner: FxSpawner {
            shape: match s.shape {
                Shape::None => FxShape::None,
                Shape::Spread => FxShape::Spread,
                Shape::Vortex => FxShape::Vortex,
                Shape::Circle => FxShape::Circle,
                Shape::Type4 => FxShape::Type4,
            },
            shape_ranges: ranges(s.shape_ranges),
            offset: ranges(s.offset),
            position_scale: range(s.position_scale),
            velocity_scale: range(s.velocity_scale),
            inherit_velocity_factor: range(s.inherit_velocity_factor),
            size: range(s.size),
            color: ranges(s.color),
            hsv: s.color_mode == ColorMode::Hsv,
            fade_in_time: s.fade_in_time,
            start_rotation: range(s.start_rotation),
            rotation_velocity: range(s.rotation_velocity),
        },
        transformer: FxTransformer {
            lifetime: t.lifetime,
            position: t
                .position
                .iter()
                .map(|n| FxPositionNode {
                    lifetime: n.lifetime,
                    op: match n.op {
                        PositionOp::None => FxPositionOp::None,
                        PositionOp::Accelerate => FxPositionOp::Accelerate,
                        PositionOp::Reach => FxPositionOp::Reach,
                        PositionOp::Scale => FxPositionOp::Scale,
                    },
                    value: n.value,
                })
                .collect(),
            size: t.size.iter().map(scalar).collect(),
            rotation: t.rotation.iter().map(scalar).collect(),
            color: t
                .color
                .iter()
                .map(|n| FxColorNode {
                    lifetime: n.lifetime,
                    op: match n.op {
                        ColorOp::None => FxColorOp::None,
                        ColorOp::Move => FxColorOp::Move,
                        ColorOp::Reach => FxColorOp::Reach,
                    },
                    value: n.value,
                })
                .collect(),
        },
        geometry: FxGeometry {
            blend: match g.blend {
                BlendMode::Normal => FxBlend::Normal,
                BlendMode::Additive => FxBlend::Additive,
                BlendMode::Blur => FxBlend::Blur,
            },
            kind: match g.kind {
                GeometryType::Particle => FxType::Particle,
                GeometryType::Streak => FxType::Streak,
                GeometryType::Emitter => FxType::Emitter,
                GeometryType::Animated => FxType::Animated,
                GeometryType::Geometry => FxType::Geometry,
                GeometryType::Billboard => FxType::Billboard,
                GeometryType::Spark => FxType::Spark,
            },
            texture: g.texture.clone(),
            model: g.model.clone(),
            child: g.child.as_deref().map(|c| Box::new(emitter(c))),
            time_per_frame: g.time_per_frame,
            frame_size: g.frame_size,
            total_frames: g.total_frames,
            blur_value: g.blur_value,
            blur_res: g.blur_res,
            spark_length: g.spark_length,
            looping: g.looping,
        },
        min_lod_dist: e.min_lod_dist,
        max_lod_dist: e.max_lod_dist,
        bounding_radius: e.bounding_radius,
        no_register_step: e.no_register_step,
        world_aligned: e.world_aligned,
        sibling: e.sibling.as_deref().map(|c| Box::new(emitter(c))),
        sound_name: e.sound_name.clone(),
    }
}

fn scalar(n: &mungefs::particle::ScalarNode) -> FxScalarNode {
    FxScalarNode {
        lifetime: n.lifetime,
        op: match n.op {
            ScalarOp::None => FxScalarOp::None,
            ScalarOp::Move => FxScalarOp::Move,
            ScalarOp::Reach => FxScalarOp::Reach,
            ScalarOp::Scale => FxScalarOp::Scale,
        },
        value: n.value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name line hashes to itself once, and `ord_explosion` resolves.
    #[test]
    fn the_names_file_is_unique_and_resolves() {
        let n = names();
        let lines = include_str!("effect_names.txt")
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .count();
        assert_eq!(n.len(), lines, "two names share a hash");
        assert_eq!(n.len(), 382);
        assert_eq!(n.get(&0xc42b_8278), Some(&"ord_explosion"));
    }

    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = tag.to_vec();
        v.extend((payload.len() as u32).to_le_bytes());
        v.extend(payload);
        while v.len() % 4 != 0 {
            v.push(0);
        }
        v
    }

    fn data(name: &str, nums: &[f32], strs: &[&str]) -> Vec<u8> {
        let argc = (nums.len() + strs.len()) as u8;
        let mut blob = Vec::new();
        let mut slots: Vec<u32> = Vec::new();
        for s in strs {
            slots.push(4 * u32::from(argc) + blob.len() as u32);
            blob.extend(s.as_bytes());
            blob.push(0);
        }
        slots.extend(nums.iter().map(|n| n.to_bits()));
        let mut p = mungefs::fnv1a_lowercase(name).to_le_bytes().to_vec();
        p.push(argc);
        for s in &slots {
            p.extend(s.to_le_bytes());
        }
        p.extend((blob.len() as u32).to_le_bytes());
        p.extend(blob);
        chunk(b"DATA", &p)
    }

    /// The DLC's `fisto_bubble` (built from its authored field values) joins
    /// the DLC layer's file as a shield record, and only there: a retail
    /// layer keeps the file it wrote.
    #[test]
    fn the_force_balls_shield_record_is_written_by_the_dlc_layer_only() {
        let scope = [
            data("Type", &[], &["droideka"]),
            data("Texture", &[], &["cis_inf_droideka_shield"]),
            data("Color", &[0.0, 192.0, 220.0, 255.0], &[]),
            data("SpecularColor", &[140.0, 150.0, 100.0], &[]),
            data("Radius", &[0.75], &[]),
            data("Height", &[0.75], &[]),
            data("TurnOnTime", &[2.0], &[]),
            data("TurnOffTime", &[0.15], &[]),
            data("ScrollRate", &[1.0], &[]),
        ]
        .concat();
        let mut body = chunk(
            b"NAME",
            &mungefs::fnv1a_lowercase("fisto_bubble").to_le_bytes(),
        );
        body.extend(data("Shield", &[], &["fisto_bubble"]));
        body.extend(chunk(b"SCOP", &scope));
        let buf = chunk(b"fx__", &body);
        let c = mungefs::chunks(&buf).next().unwrap().unwrap();
        let mut fx = Collected::from_chunks([&c]).unwrap();
        assert!(
            fx.effects.is_empty() && fx.beams.is_empty(),
            "not a particle effect"
        );
        assert_eq!(fx.shields.len(), 1);
        let dir = std::env::temp_dir().join(format!("openbf_fx_shield_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            fx.write(&dir, "bf2", &[], &[]).unwrap().is_none(),
            "retail: no file"
        );
        assert!(!dir.join(EFFECTS_FILE).exists());
        fx.write_shields = true;
        let set = fx.write(&dir, "bf2", &[], &[]).unwrap().unwrap();
        let s = &set.shields[0];
        assert_eq!(s.hash, 0xcb11_ec3f);
        assert_eq!((s.radius, s.color), (0.75, [0, 192, 220, 255]));
        assert_eq!(
            (s.specular_color, s.turn_on_time),
            ([140, 150, 100, 255], 2.0)
        );
        let back: FxSet =
            ron::from_str(&std::fs::read_to_string(dir.join(EFFECTS_FILE)).unwrap()).unwrap();
        assert_eq!(back.shields, set.shields);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
