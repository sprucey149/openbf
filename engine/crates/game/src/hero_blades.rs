//! BF2 saber edges from the authored ODF groups and the baked clip poses.
//! Collision tracks use the same bones, bind transforms and scaled animation
//! translations as the rendered model. They contain no guessed hit cone.

use crate::{
    assets,
    classdb::ClassDb,
    skin::{Clip, Pose, Skeleton},
};
use anyhow::{anyhow, bail, Context, Result};
use core::hero_weapon::{Blade, BladeSegment, SaberClass, SaberClip};
use glam::{Mat4, Vec3};
use manifest::combo::{ComboDef, ComboField, ComboKeyword as K, ComboSet};
use std::{collections::BTreeSet, path::Path};

mod physical;
mod root_speed;
mod throw_bind;

/// The attachment shared by the rendered hilt/blade and the collision track.
#[derive(Debug, Clone)]
pub struct BladeBinding {
    pub blade: Blade,
    pub geometry: Option<String>,
    pub glow_texture: Option<String>,
    pub local: BladeSegment,
}

impl BladeBinding {
    pub fn segment(&self, attachment: Mat4) -> BladeSegment {
        BladeSegment {
            start: attachment.transform_point3(self.local.start),
            end: attachment.transform_point3(self.local.end),
        }
    }
}

#[derive(Clone)]
struct Edge {
    blade: Blade,
    geometry: Option<String>,
    glow_texture: Option<String>,
    offset: Vec3,
    direction: Vec3,
}

fn scalar<'a>(db: &'a ClassDb, class: &str, key: &str) -> Option<&'a str> {
    let hash = crate::classdb::fnv1a_lowercase(key);
    db.chain(class).into_iter().find_map(|c| {
        c.props
            .iter()
            .rev()
            .find(|p| p.0 == hash)
            .map(|p| p.1.as_str())
    })
}

/// Preserve repeated blade groups. An offhand point without its second word
/// attaches at bone_l_hand (B2 FUN_00342528, 336912-336934; literal at 343aac).
/// AttachedFirePoint carries its own direction and offset in the named bone.
fn edges(db: &ClassDb, weapon: &str) -> Result<Vec<Edge>> {
    let hash = crate::classdb::fnv1a_lowercase;
    let mut result: Vec<Edge> = Vec::new();
    let mut geometry = None;
    let mut bone = "hp_weapons".to_owned();
    let mut node = "hp_fire".to_owned();
    let mut offset = Vec3::ZERO;
    let mut direction = Vec3::Z;
    let mut count = None;
    for class in db.chain(weapon).iter().rev() {
        for (key, value) in &class.props {
            let words: Vec<_> = value.split_whitespace().collect();
            if *key == hash("NumDamageEdges") {
                let declared = value.trim().parse::<usize>()?;
                if declared > core::hero_weapon::MAX_BLADE_EDGES {
                    bail!(
                        "{weapon}: NumDamageEdges {declared} exceeds the supported {} blade edges (visibility mask)",
                        core::hero_weapon::MAX_BLADE_EDGES
                    );
                }
                count = Some(declared);
            } else if *key == hash("GeometryName") || *key == hash("OffhandGeometryName") {
                geometry = Some(value.trim().to_ascii_lowercase());
                bone = if *key == hash("GeometryName") {
                    "hp_weapons"
                } else {
                    "bone_l_hand"
                }
                .into();
                node = "hp_fire".into();
                offset = Vec3::ZERO;
                direction = Vec3::Z;
            } else if *key == hash("FirePointName") || *key == hash("OffhandFirePointName") {
                node = words
                    .first()
                    .ok_or_else(|| anyhow!("{weapon}: empty fire point"))?
                    .to_ascii_lowercase();
                bone = if *key == hash("FirePointName") {
                    "hp_weapons"
                } else {
                    words.get(1).copied().unwrap_or("bone_l_hand")
                }
                .to_ascii_lowercase();
                offset = Vec3::ZERO;
                direction = Vec3::Z;
            } else if *key == hash("AttachedFirePoint") {
                if words.len() != 7 {
                    bail!("{weapon}: AttachedFirePoint needs a bone, direction and offset");
                }
                geometry = None;
                bone = words[0].to_ascii_lowercase();
                node.clear();
                let values = words[1..]
                    .iter()
                    .map(|v| v.parse::<f32>())
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if values.iter().any(|v| !v.is_finite()) {
                    bail!("{weapon}: attached blade direction/offset must be finite");
                }
                direction = Vec3::new(values[0], values[1], values[2]).normalize_or_zero();
                offset = Vec3::new(values[3], values[4], values[5]);
                if direction == Vec3::ZERO {
                    bail!("{weapon}: zero attached blade direction");
                }
            } else if [
                hash("LightSaberLength"),
                hash("LightSaberWidth"),
                hash("LightSaberTexture"),
                hash("LightSaberGlowTexture"),
                hash("LightSaberTrailColor"),
                hash("DamageEdgeLength"),
                hash("DamageEdgeWidth"),
            ]
            .contains(key)
            {
                let existing = result.iter().position(|e| {
                    e.geometry == geometry && e.blade.bone == bone && e.blade.node == node
                });
                let i = match existing {
                    Some(i) => i,
                    None => {
                        if result.len() == core::hero_weapon::MAX_BLADE_EDGES {
                            bail!(
                                "{weapon}: more than {} authored blade edges exceed the visibility mask; reduce the blade groups",
                                core::hero_weapon::MAX_BLADE_EDGES
                            );
                        }
                        result.push(Edge {
                            blade: Blade {
                                node: node.clone(),
                                bone: bone.clone(),
                                length: 0.0,
                                width: 0.0,
                                texture: String::new(),
                                trail_color: None,
                            },
                            geometry: geometry.clone(),
                            glow_texture: Some("lightsabreglow".into()),
                            offset,
                            direction,
                        });
                        result.len() - 1
                    }
                };
                let edge = &mut result[i];
                if *key == hash("LightSaberTrailColor") {
                    edge.blade.trail_color = if value.trim().eq_ignore_ascii_case("none") {
                        None
                    } else {
                        let rgba = words
                            .iter()
                            .map(|v| v.parse::<u8>())
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        let rgba: [u8; 4] = rgba
                            .try_into()
                            .map_err(|_| anyhow!("{weapon}: trail color needs RGBA"))?;
                        (rgba[3] != 0).then_some(rgba)
                    };
                } else if *key == hash("LightSaberGlowTexture") {
                    let name = value.trim().to_ascii_lowercase();
                    edge.glow_texture =
                        (!matches!(name.as_str(), "none" | "notexture" | "null")).then_some(name);
                } else if *key == hash("LightSaberTexture") {
                    let texture = value.trim().to_ascii_lowercase();
                    edge.blade.texture = if matches!(texture.as_str(), "none" | "notexture") {
                        String::new()
                    } else {
                        texture
                    };
                } else if *key == hash("LightSaberLength") || *key == hash("DamageEdgeLength") {
                    edge.blade.length = value.trim().parse()?;
                } else {
                    edge.blade.width = value.trim().parse()?;
                }
            }
        }
    }
    if result.is_empty() {
        bail!("{weapon}: no authored blade edges");
    }
    if count.is_some_and(|count| count != result.len()) {
        bail!(
            "{weapon}: NumDamageEdges {count:?} differs from {} parsed edges",
            result.len()
        );
    }
    for edge in &result {
        if !edge.blade.length.is_finite()
            || edge.blade.length < 0.0
            || (edge.blade.length == 0.0 && !edge.blade.texture.is_empty())
            || !edge.blade.width.is_finite()
            || edge.blade.width <= 0.0
        {
            bail!(
                "{weapon}: edge {} has no positive authored length/width",
                edge.blade.node
            );
        }
    }
    Ok(result)
}

/// Parse the authored edges and resolve the corresponding weapon node frames.
/// A pre-hero bake with positions alone must be rebaked before enabling melee.
pub fn blade_bindings(root: &Path, db: &ClassDb, weapon: &str) -> Result<Vec<BladeBinding>> {
    edges(db, weapon)?
        .into_iter()
        .map(|edge| {
            // Wampa paw and head edges have zero base length. Their combo
            // supplies an absolute attack length, so retain the posed axis.
            let track_length = if edge.blade.length == 0.0 {
                1.0
            } else {
                edge.blade.length
            };
            let local = match &edge.geometry {
                Some(model) => {
                    let side = root.join("side_bf2");
                    let mut path = manifest::weapon_nodes_path(&side, model);
                    if !pak::is_file(&path) {
                        path = manifest::weapon_nodes_path(&side.join("locals"), model);
                    }
                    let nodes: manifest::WeaponNodes = ron::from_str(
                        &pak::read_to_string(&path)
                            .with_context(|| format!("{} (rebake BF2 heroes)", path.display()))?,
                    )?;
                    let node = nodes
                        .iter()
                        .find(|n| n.name.eq_ignore_ascii_case(&edge.blade.node))
                        .ok_or_else(|| anyhow!("{model}: no node {}", edge.blade.node))?;
                    let axes = node.axes.ok_or_else(|| {
                        anyhow!("{model}: blade node axes missing; rebake BF2 heroes")
                    })?;
                    if node
                        .pos
                        .iter()
                        .chain(axes.iter().flatten())
                        .any(|n| !n.is_finite())
                    {
                        bail!("{model}: nonfinite physical blade node {}", node.name);
                    }
                    let start = Vec3::from(node.pos);
                    BladeSegment {
                        start,
                        end: start + Vec3::from(axes[2]) * track_length,
                    }
                }
                None => BladeSegment {
                    start: edge.offset,
                    end: edge.offset + edge.direction * track_length,
                },
            };
            physical::edge(local)?;
            Ok(BladeBinding {
                blade: edge.blade,
                geometry: edge.geometry,
                glow_texture: edge.glow_texture,
                local,
            })
        })
        .collect()
}

fn animation_names(combo: &ComboDef) -> BTreeSet<String> {
    fn visit(fields: &[ComboField], names: &mut BTreeSet<String>) {
        for field in fields {
            if matches!(
                field.keyword,
                K::Animation
                    | K::OffhandAnimation
                    | K::ThrowAnimation
                    | K::DeflectAnimation
                    | K::BlendTimeTo
            ) {
                if let Some(name) = field.text(0) {
                    names.insert(name.to_ascii_lowercase());
                }
            }
            visit(&field.children, names);
        }
    }
    let mut names = BTreeSet::new();
    visit(&combo.fields, &mut names);
    names
}

pub(crate) fn clip_for<'a>(
    clips: &'a [Clip],
    family: &str,
    bank: &str,
    name: &str,
    layer: &str,
) -> Option<&'a Clip> {
    if (name.ends_with("_upper") && layer == "lower")
        || (name.ends_with("_lower") && layer == "upper")
    {
        return None;
    }
    for family in [family, "human"] {
        let base = format!("{family}_{bank}_{name}");
        let scoped = ["_full", "_upper", "_lower"]
            .iter()
            .any(|s| name.ends_with(s));
        let names = if scoped {
            match base
                .strip_suffix("_upper")
                .or_else(|| base.strip_suffix("_lower"))
            {
                Some(stem) => vec![base.clone(), format!("{stem}_full")],
                None => vec![base],
            }
        } else {
            vec![
                base.clone(),
                format!("{base}_{layer}"),
                format!("{base}_full"),
            ]
        };
        if let Some(clip) = names
            .iter()
            .find_map(|n| clips.iter().find(|c| c.name.eq_ignore_ascii_case(n)))
        {
            return Some(clip);
        }
    }
    None
}

fn sampled_edges(
    skel: &Skeleton,
    pose: &Pose,
    bindings: &[BladeBinding],
    hp: Option<usize>,
) -> Result<(Vec<BladeSegment>, Mat4)> {
    physical::pose(pose)?;
    let palette = skel.palette(&pose.locals());
    let edges = bindings
        .iter()
        .map(|binding| {
            let bone = skel.index_of(&binding.blade.bone).ok_or_else(|| {
                anyhow!("skeleton has no blade attachment {}", binding.blade.bone)
            })?;
            let segment = binding.segment(palette[bone] * skel.global_bind[bone]);
            physical::edge(segment)?;
            Ok(segment)
        })
        .collect::<Result<_>>()?;
    let attachment = hp.map_or(Mat4::IDENTITY, |hp| palette[hp] * skel.global_bind[hp]);
    physical::matrix(attachment)?;
    Ok((edges, attachment))
}

/// Build deterministic saber collision tracks from the same baked GLB used
/// for drawing. The tracks are model-space; the sim supplies the unit transform.
pub fn saber_class(db: &ClassDb, soldier: &str, weapon: &str) -> Result<SaberClass> {
    let root = db
        .assets_root()
        .ok_or_else(|| anyhow!("{soldier}: class database has no asset root"))?;
    let geometry =
        scalar(db, soldier, "GeometryName").ok_or_else(|| anyhow!("{soldier}: no GeometryName"))?;
    let stock = root.join("side_bf2");
    let combo_path = Path::new("soldiers").join(format!("{geometry}.combos.ron"));
    let side = if pak::is_file(stock.join(&combo_path)) {
        stock
    } else {
        stock.join("locals")
    };
    let path = side.join(combo_path);
    let combos: ComboSet = ron::from_str(
        &pak::read_to_string(&path)
            .with_context(|| format!("{} (rebake BF2 heroes)", path.display()))?,
    )?;
    let bank = scalar(db, weapon, "ComboAnimationBank")
        .ok_or_else(|| anyhow!("{weapon}: no ComboAnimationBank"))?;
    let words: Vec<_> = bank.split_whitespace().collect();
    if words.len() != 3 {
        bail!("{weapon}: malformed ComboAnimationBank {bank:?}");
    }
    let combo_hash = crate::classdb::fnv1a_lowercase(words[2]);
    let combo = combos
        .combos
        .into_iter()
        .find(|c| c.name_hash == combo_hash)
        .ok_or_else(|| anyhow!("{weapon}: combo {} is not baked for {geometry}", words[2]))?;
    let family = scalar(db, soldier, "AnimationName")
        .or_else(|| scalar(db, soldier, "SkeletonName"))
        .and_then(|s| s.split_whitespace().next())
        .unwrap_or("human")
        .to_ascii_lowercase();
    let bank = words[0].split_once('_').map_or(words[0], |(_, b)| b);
    let asset = assets::load_skinned_glb(&side.join("soldiers").join(format!("{geometry}.glb")))?;
    physical::skeleton(&asset.skeleton)?;
    let sets: manifest::anim_sets::SoldierSets = ron::from_str(&pak::read_to_string(
        manifest::anim_sets::path(&side, geometry),
    )?)?;
    let motions: manifest::anim_sets::ClipMotions = ron::from_str(&pak::read_to_string(
        manifest::anim_sets::motion_path(&side, geometry),
    )?)?;
    let bindings = blade_bindings(root, db, weapon)?;
    let hp = asset.skeleton.index_of("hp_weapons");
    // Native group zero is the first registered owner-body attachment.
    // Four-arm rigs can author that node without hp_weapons.
    let group_zero = bindings
        .first()
        .and_then(|b| asset.skeleton.index_of(&b.blade.bone));
    let throw_edges: Vec<BladeSegment> = bindings
        .iter()
        .filter(|b| b.blade.length > 0.0 && b.blade.bone.eq_ignore_ascii_case("hp_weapons"))
        .map(|b| b.local)
        .collect();
    let deflect_edges = bindings.first().map_or_else(Vec::new, |first| {
        bindings
            .iter()
            .filter(|b| {
                b.blade.length > 0.0 && b.blade.bone.eq_ignore_ascii_case(&first.blade.bone)
            })
            .map(|b| b.local)
            .collect()
    });
    let weapon_bind = group_zero.map_or(Mat4::IDENTITY, |bone| asset.skeleton.global_bind[bone]);
    if hp.is_none() && !throw_edges.is_empty() {
        bail!("{soldier}: primary throw edges have no hp_weapons attachment");
    }
    let mut names = animation_names(&combo);
    // IDLE can omit Animation. Keep an authored standing pose for launches.
    if let Some(idle) = ["stand_idle", "stand_idle_emote"].into_iter().find(|name| {
        clip_for(&asset.clips, &family, bank, name, "upper").is_some()
            || clip_for(&asset.clips, &family, bank, name, "lower").is_some()
    }) {
        names.insert(idle.into());
    }
    let mut clips = Vec::new();
    for name in names {
        let upper = clip_for(&asset.clips, &family, bank, &name, "upper");
        let lower = clip_for(&asset.clips, &family, bank, &name, "lower");
        let frames = upper
            .into_iter()
            .chain(lower)
            .map(|c| c.frames)
            .max()
            .ok_or_else(|| anyhow!("{soldier}: combo clip {name} not baked"))?;
        let mut samples = Vec::new();
        let mut weapon_frames = Vec::new();
        for frame in 0..frames {
            let mut pose = lower
                .or(upper)
                .unwrap()
                .sample_frame(&asset.skeleton, frame);
            if let Some(upper) = upper {
                let up = upper.sample_frame(&asset.skeleton, frame);
                for bone in &sets.upper_bones {
                    if let Some(i) = asset.skeleton.index_of(bone) {
                        pose.0[i] = up.0[i];
                    }
                }
            }
            let (edges, attachment) = sampled_edges(&asset.skeleton, &pose, &bindings, group_zero)?;
            samples.push(edges);
            weapon_frames.push(attachment);
        }
        let root_clip = lower.or(upper).unwrap();
        let root = motions
            .get(&root_clip.name)
            .ok_or_else(|| anyhow!("{soldier}: no original root keys for {}", root_clip.name))?;
        if root.len() != root_clip.frames {
            bail!(
                "{soldier}: root key/frame count differs for {}",
                root_clip.name
            );
        }
        let first = root.first().copied().map(Vec3::from).unwrap_or(Vec3::ZERO);
        let root_frames: Vec<_> = (0..frames)
            .map(|frame| {
                root.get(frame.min(root.len().saturating_sub(1)))
                    .copied()
                    .map(Vec3::from)
                    .unwrap_or(Vec3::ZERO)
                    - first
            })
            .collect();
        let root_speed = match sets.root_speed.get(&root_clip.name).copied() {
            Some(speed) => root_speed::packed(speed)?,
            None => root_speed::measure(root)?,
        };
        clips.push(SaberClip {
            name,
            root_speed,
            root_frames,
            weapon_frames,
            frames: samples,
        });
    }
    // The idle state has no Animation on the disc. Keep the neutral blade
    // pose available to rendering without inventing a damaging attack.
    clips.push(SaberClip {
        name: String::new(),
        root_speed: [0.0; 2],
        root_frames: vec![Vec3::ZERO],
        weapon_frames: vec![weapon_bind],
        frames: vec![
            sampled_edges(
                &asset.skeleton,
                &Pose::bind(&asset.skeleton),
                &bindings,
                group_zero,
            )?
            .0,
        ],
    });
    let state_seconds = combo
        .states()
        .map(|state| {
            state
                .find(K::Animation)
                .and_then(|a| a.text(0))
                .and_then(|name| clips.iter().find(|c| c.name.eq_ignore_ascii_case(name)))
                .map_or(0.0, SaberClip::seconds)
        })
        .collect();
    let throw_bindings = throw_bind::bindings(db, soldier, &combo, &asset, &sets, &family, bank)?;
    let class = SaberClass {
        combo,
        blades: bindings.into_iter().map(|b| b.blade).collect(),
        throw_edges,
        deflect_edges,
        weapon_bind,
        throw_bindings,
        clips,
        state_seconds,
        damage: scalar(db, weapon, "Damage")
            .map(str::parse::<f32>)
            .transpose()?
            .unwrap_or(0.0),
        hit_sound: db.prop_hash(weapon, 0xc5bd_d741).map(str::to_owned),
        deflect_sound: scalar(db, weapon, "DeflectSound").map(str::to_owned),
        hit_effect: db.prop_hash(weapon, 0xc092_9477).map(str::to_owned),
        deflect_effect: db.prop_hash(weapon, 0xf182_0751).map(str::to_owned),
    };
    physical::class(&class).with_context(|| format!("{soldier}: invalid physical saber data"))?;
    Ok(class)
}

#[cfg(test)]
mod tests;
