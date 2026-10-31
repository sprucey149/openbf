//! Soldier and hand-held weapon output: GLBs, animation metadata and textures.
//! The parent owns model/rig conversion and the existing regression tests.

use super::{
    clip_to_tracks, find_clip, find_soldier, model_skins, pose_rig, to_skinned_model, BakedSoldier,
    ClipMeta, SoldierAnims, SEAT_CLIPS, SOLDIER_CLIPS,
};
use crate::anim_sets;
use crate::skin_glb::{skinned_to_glb, Clip};
use anyhow::{anyhow, Context, Result};
use mungefs::anim::{crc32_lower, AnimBank};
use mungefs::model::Model;
use std::path::Path;

/// The BF1 rifle prone clips a BF2 soldier borrows (`bake_soldier_anims`,
/// issue #40): the moves the animator plays and the four transitions.
pub const BORROWED_PRONE: &[&str] = &[
    "human_rifle_prone_idle_emote",
    "human_rifle_prone_idle_checkweapon",
    "human_rifle_prone_idle_lookaround",
    "human_rifle_prone_turnleft",
    "human_rifle_prone_turnright",
    "human_rifle_prone_walkforward",
    "human_rifle_prone_walkbackward",
    "human_rifle_prone_walkleft",
    "human_rifle_prone_walkright",
    "human_rifle_prone_shoot",
    "human_rifle_prone_reload",
    "human_rifle_stand_getdown_prone",
    "human_rifle_crouch_getdown_prone",
    "human_rifle_prone_getup_stand",
    "human_rifle_prone_getup_crouch",
];

/// The BF2 throw clips a BF1 soldier borrows (`bake_soldier_anims`, card
/// GTHRW): BF2's offhand throw (`shoot_secondary` underhand,
/// `shoot_secondary2` overhand) is played on every map, a named deviation
/// (BF1's own is `grenade_stand_shoot3` on the grenade's set,
/// `analysis/re-grenade-throw-anim.md` §0).
pub const BORROWED_THROW: &[&str] = &[
    "human_rifle_stand_shoot_secondary_full",
    "human_rifle_stand_shoot_secondary2_full",
];

/// Bake one soldier model (+ textures) from a SIDE LVL.
pub fn bake_soldier(lvl_path: &str, model: &str, outdir: &str) -> Result<BakedSoldier> {
    bake_soldier_anims(lvl_path, model, None, outdir, None)
}

/// As `bake_soldier`, plus the soldier's clips from `anim_lvl` (BF1
/// COMMON.LVL / BF2 INGAME.LVL, and for BF2 the side LVLs beside `lvl_path`)
/// as GLB animations, `<model>.anims.ron` (what was baked) and
/// `<model>.sets.ron` (the PS2's animation sets over them,
/// `anim_sets::SoldierSets`).
pub fn bake_soldier_anims(
    lvl_path: &str,
    model: &str,
    anim_lvl: Option<&str>,
    outdir: &str,
    pc_lvl: Option<&str>,
) -> Result<BakedSoldier> {
    let lvl_resolved = crate::resolve_ci(lvl_path);
    let lvl_path = lvl_resolved.as_str();
    let pc_resolved = pc_lvl.map(crate::resolve_ci);
    let pc = pc_resolved
        .as_deref()
        .map(crate::tier::PcTextures::load)
        .transpose()?;
    let buf = std::fs::read(lvl_path).with_context(|| lvl_path.to_string())?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
    let (m, mut skel) = find_soldier(inner, model)?;
    let mut sm = to_skinned_model(&m, &skel)?;
    // BF2's cloth, rest shape pinned to its bones (card CLOTH; the step is
    // CLOTH2). A sheet only a cloth draws is opaque unless the class says
    // Transparent.
    let mut flags = bake_flags(&m);
    let cloths = crate::cloth::worn_cloths(inner, &m.name, &skel)?;
    for c in &cloths {
        let alpha = if c.segment.blend {
            crate::detail::TRANSPARENT
        } else {
            0
        };
        *flags
            .entry(c.segment.texture.to_ascii_lowercase())
            .or_default() |= alpha;
    }
    let cloth_names: Vec<String> = cloths.iter().map(|c| c.name.clone()).collect();
    sm.segments.extend(cloths.into_iter().map(|c| c.segment));
    let out = Path::new(outdir);
    std::fs::create_dir_all(out.join("soldiers"))?;
    std::fs::create_dir_all(out.join("textures"))?;

    let mut clips = Vec::new();
    let mut meta = SoldierAnims::default();
    if let Some(anim_path) = anim_lvl {
        let anim_resolved = crate::resolve_ci(anim_path);
        let abuf = std::fs::read(&anim_resolved).with_context(|| anim_resolved.clone())?;
        let ainner = mungefs::open(&abuf).map_err(|e| anyhow!("{e}"))?;
        let sanm = mungefs::sanm::find_sanm(ainner).map_err(|e| anyhow!("{e}"))?;
        // BF2's sets take every soldier side's classes and banks (a level
        // loads two sides; `anim_sets::bf2::Registry::register`).
        let sides: Vec<Vec<u8>> = match sanm {
            Some(_) => ["ALL", "IMP", "REP", "CIS"]
                .iter()
                .filter_map(|s| {
                    let parent = Path::new(lvl_path).parent()?;
                    std::fs::read(crate::join_ci(parent, &format!("{s}.LVL"))).ok()
                })
                .collect(),
            None => Vec::new(),
        };
        let mut banks = mungefs::anim::find_banks(ainner).map_err(|e| anyhow!("{e}"))?;
        let anim_banks = banks.len();
        // The requested side can also be JED or a native side, outside the
        // four faction LVLs loaded below.
        banks.extend(mungefs::anim::find_banks(inner).map_err(|e| anyhow!("{e}"))?);
        for side in &sides {
            let inner = mungefs::open(side).map_err(|e| anyhow!("{e}"))?;
            banks.extend(mungefs::anim::find_banks(inner).map_err(|e| anyhow!("{e}"))?);
        }
        // The pose rig (card USKN): the model's classes are in its own side
        // LVL (BF2: and the sides beside it); the `zaf_` rigs in the side
        // LVLs and the animation LVL.
        let mut classes = anim_sets::bf2::side_classes(&buf)?;
        let mut rigs = mungefs::anim::find_rigs(inner).map_err(|e| anyhow!("{e}"))?;
        rigs.extend(mungefs::anim::find_rigs(ainner).map_err(|e| anyhow!("{e}"))?);
        for side in &sides {
            classes.extend(anim_sets::bf2::side_classes(side)?);
            rigs.extend(
                mungefs::anim::find_rigs(mungefs::open(side).map_err(|e| anyhow!("{e}"))?)
                    .map_err(|e| anyhow!("{e}"))?,
            );
        }
        let pose = pose_rig(&classes, &m.name, &rigs, sanm.is_some());
        if let Some(pose) = &pose {
            if super::append_rig_attachment(&mut skel, &pose.rig, "hp_weapons")? {
                sm.bones = super::bones_of(&skel);
            }
        }
        let bones: Vec<String> = skel.bones.iter().map(|b| b.name.clone()).collect();
        let plan = match &sanm {
            None => anim_sets::plan_bf1(&anim_sets::bf1::inventory(&banks), &bones),
            Some(sanm) => {
                let mut classes = anim_sets::bf2::side_classes(&abuf)?;
                for side in &sides {
                    classes.extend(anim_sets::bf2::side_classes(side)?);
                }
                let mut reg = anim_sets::bf2::Registry::shipped();
                let soldiers = reg.register(&classes);
                // The families of the classes on this model; a model no
                // class names is posed as `human`.
                let mut families: Vec<usize> = soldiers
                    .iter()
                    .filter(|c| c.geometry.eq_ignore_ascii_case(&m.name))
                    .map(|c| c.family)
                    .collect();
                families.sort_unstable();
                families.dedup();
                if families.is_empty() {
                    families.push(0);
                }
                let inv = anim_sets::Inventory::new(&banks);
                anim_sets::plan_bf2(&inv, &reg, &families, sanm, &bones)
            }
        };
        // The clips `game::animator` plays today (card ANIML replaces the
        // list by the sets), then every clip a set names.
        let human: Vec<&AnimBank> = banks[..anim_banks]
            .iter()
            .filter(|b| b.name.starts_with("human"))
            .collect();
        let mut wanted: Vec<(String, &AnimBank, usize)> = Vec::new();
        for name in SOLDIER_CLIPS.iter().chain(SEAT_CLIPS) {
            match find_clip(&human, name) {
                Some((full, bank, k)) => wanted.push((full, bank, k)),
                None => meta.missing.push((*name).to_string()),
            }
        }
        for (c, bank) in plan.sets.clips.iter().zip(&plan.banks) {
            if wanted.iter().any(|w| w.0 == c.name) {
                continue;
            }
            // The first bank of that name that holds the clip: the one the
            // inventory took (two side LVLs may carry a bank of one name).
            let found = banks
                .iter()
                .filter(|b| b.name == *bank)
                .find_map(|b| Some((b, b.clip_index(&c.name)?)));
            let (b, k) = found.ok_or_else(|| anyhow!("{}: not in bank {bank}", c.name))?;
            wanted.push((c.name.clone(), b, k));
        }
        if sanm.is_some() {
            // Force choke uses a custom animation outside the ordinary set
            // slots: B2 FUN_00239850 defaults to CRC 144ebf32 (human_choking).
            // Include a model family's authored equivalent when it exists.
            let all: Vec<&AnimBank> = banks.iter().collect();
            let families: std::collections::BTreeSet<_> = plan
                .sets
                .bf2
                .iter()
                .map(|f| f.family.as_str())
                .chain(["human"])
                .collect();
            for family in families {
                if let Some((name, bank, k)) = find_clip(&all, &format!("{family}_choking")) {
                    if !wanted.iter().any(|w| w.0 == name) {
                        wanted.push((name, bank, k));
                    }
                }
            }
            let bindings = crate::combo::model_bindings(&classes, &m.name);
            if !bindings.is_empty() {
                let own = crate::combo::baked_combos_from_lvl(&buf)?;
                let inv = anim_sets::Inventory::new(&banks);
                let mut model_combos = manifest::combo::ComboSet {
                    scope_provenance: own.scope_provenance,
                    ..manifest::combo::ComboSet::default()
                };
                for binding in bindings {
                    let combo = own
                        .combos
                        .iter()
                        .find(|c| c.name_hash == binding.name_hash)
                        .with_context(|| {
                            format!("{}: missing combo 0x{:08x}", m.name, binding.name_hash)
                        })?;
                    let (extra, missing) =
                        crate::combo::resolve_animation_clips(&inv, &binding, combo);
                    meta.missing.extend(
                        missing
                            .into_iter()
                            .map(|s| format!("combo:{}:{s}", binding.family)),
                    );
                    for clip in extra {
                        if wanted.iter().any(|w| w.0 == clip.name) {
                            continue;
                        }
                        let (bank, index) = banks
                            .iter()
                            .filter(|b| b.name == clip.bank)
                            .find_map(|b| Some((b, b.clip_index(&clip.name)?)))
                            .with_context(|| format!("{}: not in bank {}", clip.name, clip.bank))?;
                        wanted.push((clip.name, bank, index));
                    }
                    if !model_combos.combos.contains(combo) {
                        model_combos.combos.push(combo.clone());
                    }
                }
                crate::write_ron(
                    &out.join("soldiers").join(format!("{}.combos.ron", m.name)),
                    &model_combos,
                )?;
            }
        }
        let mut sets = plan.sets;
        // BF2 has no prone clips (`missing`); BF1's rifle prone clips are
        // borrowed when `BF1_ANIMS` names BF1's COMMON.LVL (issue #40, one
        // gameplay: a BF2 soldier goes prone too). A BF1 soldier borrows
        // BF2's throw when `BF2_ANIMS` names BF2's INGAME.LVL (card GTHRW,
        // BF2's throw on every map). The two games' soldier bones share names.
        let (lender, lent) = match &sanm {
            Some(_) => ("BF1_ANIMS", BORROWED_PRONE),
            None => ("BF2_ANIMS", BORROWED_THROW),
        };
        let lender_buf = match std::env::var_os(lender) {
            Some(p) => {
                let resolved = crate::resolve_ci(&p.to_string_lossy());
                Some(std::fs::read(&resolved).with_context(|| format!("{p:?}"))?)
            }
            None => None,
        };
        let lender_banks = match &lender_buf {
            Some(b) => {
                let inner = mungefs::open(b).map_err(|e| anyhow!("{e}"))?;
                mungefs::anim::find_banks(inner).map_err(|e| anyhow!("{e}"))?
            }
            None => Vec::new(),
        };
        let lender_human: Vec<&AnimBank> = lender_banks
            .iter()
            .filter(|b| b.name.starts_with("human"))
            .collect();
        let mut borrowed = Vec::new();
        for name in lent {
            if wanted.iter().any(|w| w.0 == *name) || lender_human.is_empty() {
                continue;
            }
            if let Some((full, bank, k)) = find_clip(&lender_human, name) {
                meta.missing.retain(|m| m != name);
                borrowed.push(full.clone());
                wanted.push((full, bank, k));
            }
        }
        let mut motions = manifest::anim_sets::ClipMotions::new();
        for (name, bank, k) in wanted {
            let mut decoded = bank.decode_clip(k).map_err(|e| anyhow!("{name}: {e}"))?;
            let root = decoded
                .tracks
                .iter()
                .find(|t| t.bone_crc == crc32_lower("DummyRoot"));
            motions.insert(
                name.clone(),
                root.map_or_else(
                    || vec![[0.0; 3]; usize::from(decoded.frames)],
                    |t| t.translations.clone(),
                ),
            );
            if borrowed.contains(&name) {
                // The lender's bone lengths are not this model's: a bone the clip
                // does not move keeps the model's own rest offset - but the
                // root and the pelvis, which carry the body's height on the
                // floor (a prone idle holds them low and still).
                let height = ["dummyroot", "bone_root", "bone_pelvis"].map(crc32_lower);
                for t in decoded
                    .tracks
                    .iter_mut()
                    .filter(|t| !t.translated && !height.contains(&t.bone_crc))
                {
                    if let Some(b) = skel.bones.iter().find(|b| b.crc == t.bone_crc) {
                        t.translations.fill(b.t);
                    }
                }
            }
            if plan.measured.contains(&name) {
                if let Some(v) = anim_sets::root_speed(&name, &decoded, sanm.is_some()) {
                    sets.root_speed.insert(name.clone(), v);
                }
            }
            let (tracks, root_motion) = clip_to_tracks(&decoded, &skel, pose.as_ref());
            clips.push(Clip {
                name: name.clone(),
                fps: 30.0,
                frames: u32::from(decoded.frames),
                tracks,
            });
            meta.clips.push(ClipMeta {
                name,
                frames: u32::from(decoded.frames),
                bank: bank.name.clone(),
                root_motion,
            });
        }
        crate::write_ron(
            &out.join("soldiers").join(format!("{}.anims.ron", m.name)),
            &meta,
        )?;
        crate::write_ron(&manifest::anim_sets::path(out, &m.name), &sets)?;
        crate::write_ron(&manifest::anim_sets::motion_path(out, &m.name), &motions)?;
    }
    let glb = skinned_to_glb(&sm, &clips)?;
    std::fs::write(out.join("soldiers").join(format!("{}.glb", m.name)), glb)?;

    let mut wanted: Vec<String> = sm
        .segments
        .iter()
        .map(|s| s.texture.to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    // The class skins (card USKN): the model's override groups and every
    // override texture its side's classes name, drawn with the flags of
    // the textures they replace.
    let skins = model_skins(&m, &anim_sets::bf2::side_classes(&buf)?);
    for (tex, group) in &skins.overrides {
        let replaced = skins.groups.get(group).into_iter().flatten();
        let f = replaced.filter_map(|t| flags.get(t)).fold(0, |a, f| a | f);
        flags.insert(tex.clone(), f);
        wanted.push(tex.clone());
    }
    let file = manifest::SoldierSkins {
        groups: skins.groups.clone(),
        overrides: skins.overrides.iter().map(|o| o.0.clone()).collect(),
    };
    crate::write_ron(
        &out.join("soldiers").join(format!("{}.skins.ron", m.name)),
        &file,
    )?;
    let textures = bake_textures_flagged(inner, &wanted, &flags, out, pc.as_ref())?;
    for (tex, _) in &skins.overrides {
        if !textures.contains(tex) {
            eprintln!(
                "soldier {}: override texture {tex} is not in the LVL",
                m.name
            );
        }
    }

    let (min, max) = crate::gltf_bin::min_max3(sm.segments.iter().flat_map(|s| s.positions.iter()));
    Ok(BakedSoldier {
        name: m.name.clone(),
        clips: clips.len(),
        missing_clips: meta.missing.clone(),
        bones: skel.bones.len(),
        vertices: sm.segments.iter().map(|s| s.positions.len()).sum(),
        triangles: sm.segments.iter().map(|s| s.triangles.len()).sum(),
        textures,
        cloths: cloth_names,
        aabb: (min, max),
    })
}

/// Per-texture OR of the material flags of the segments that use it.
pub fn texture_flags(m: &mungefs::model::Model) -> std::collections::HashMap<String, u32> {
    let mut out: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for seg in &m.segments {
        for (_, t) in &seg.textures {
            *out.entry(t.to_ascii_lowercase()).or_default() |= seg.material_flags;
        }
    }
    out
}

/// The flags [`bake_textures_flagged`] decides a sheet's alpha by: the
/// per-texture OR ([`texture_flags`]), plus `ALPHA_IS_MEANINGFUL` on every
/// sheet a specular-pass segment draws, whose alpha is the pass's gloss weight
/// (`crate::spec::pass_textures`, queue DSHN). Only the texture bake sees the
/// extra bit: the GLB materials' modes still come from `texture_flags` alone
/// (`crate::alpha::legacy_per_texture_or`).
fn bake_flags(m: &mungefs::model::Model) -> std::collections::HashMap<String, u32> {
    let mut flags = texture_flags(m);
    for t in crate::spec::pass_textures([m]) {
        *flags.entry(t).or_default() |= crate::detail::ALPHA_IS_MEANINGFUL;
    }
    flags
}

/// Bake the `tex_` chunks named in `wanted` (lowercase) from an LVL to
/// `<out>/textures/<name>.ktx2`; side LVLs repeat textures, so each is
/// written once. Returns the names written.
///
/// `tex_flags` carries the per-texture material flags gathered from the models
/// being baked: a texture no material marks cutout or transparent is written
/// opaque (crate::detail::ALPHA_IS_MEANINGFUL). Passing an empty map would
/// therefore bake EVERY texture opaque, which is why there is no convenience
/// wrapper that does it.
///
/// `pc` is the classic-plus second tier — the PC install's own SIDE LVL,
/// whose texture is written BESIDE the PS2 one as `<name>.tier2.ktx2`
/// wherever `crate::tier::tier2` accepts it. This is the SECOND write site in
/// the bake (the first is `crate::map`) and it is where the proportional gain
/// is largest, because a first-person weapon sheet is 128x64 on the PS2 and
/// 512x256 on the PC. The resolver is used unchanged, same-art test and all
/// three exclusions included.
pub fn bake_textures_flagged(
    inner: &[u8],
    wanted: &[String],
    tex_flags: &std::collections::HashMap<String, u32>,
    out: &Path,
    pc: Option<&crate::tier::PcTextures>,
) -> Result<Vec<String>> {
    let dir = out.join("textures");
    std::fs::create_dir_all(&dir)?;
    // Decode everything first: a wanted texture's `_dtl` companion can sit
    // anywhere in the LVL, including before the base (crate::detail).
    let all: Vec<mungefs::texture::Texture> = mungefs::content_chunks(inner)
        .map_err(|e| anyhow!("{e}"))?
        .iter()
        .filter(|c| &c.tag == b"tex_")
        .filter_map(|c| mungefs::texture::Texture::parse(c).ok())
        .collect();

    let mut textures: Vec<String> = Vec::new();
    let mut tier2_written = 0usize;
    for t in &all {
        let name = t.name.to_ascii_lowercase();
        if !wanted.contains(&name) || textures.contains(&name) {
            continue;
        }
        let flags = tex_flags
            .get(&name)
            .copied()
            .unwrap_or(crate::detail::ALPHA_IS_MEANINGFUL);
        // Both tiers, side by side, exactly as `crate::map:1004-1033` writes
        // them (PLAN §Classic-plus assets, the maintainer 2026-09-03): the base file is
        // ALWAYS the PS2 disc's pixels — the sim reads only that — and the
        // classic-plus override, where the chain has one, is written beside it
        // as `<name>.tier2.ktx2` for the runtime to pick between.
        //
        // The alpha state is the PS2 texture's on BOTH files. The side bake
        // decides it here from the per-texture OR of the material flags (the
        // map bake decides it per draw), so the flags land on whichever
        // pixels are written; and where a draw does want alpha, the override
        // takes the PS2 mask instead of its own channel, which is gloss/spec
        // on several shipped sheets — a MASK draw would otherwise cut out
        // along a mask nobody authored and the tier flip would change the
        // silhouette.
        let mut r = crate::tier::Resolved::from_ps2(t);
        let mut tier2 = crate::tier::tier2(t, pc);
        if flags & crate::detail::ALPHA_IS_MEANINGFUL == 0 {
            r.make_opaque();
            if let Some(o) = tier2.as_mut() {
                o.make_opaque();
            }
        } else if let Some(o) = tier2.as_mut() {
            o.take_alpha_from(&r);
        }
        r.write_pair(&dir, &name, tier2.as_ref())?;
        if tier2.is_some() {
            tier2_written += 1;
        }
        // The detail companion stays PS2: `tier::tier2` refuses a
        // detail-compressed base outright (exclusion 1), so a base and its
        // companion cannot come from two different tiers.
        crate::detail::write_detail_for(&dir, t, &all)?;
        textures.push(name);
    }
    if pc.is_some() {
        eprintln!(
            "  tier 2 written beside the PS2 base: {tier2_written} of {}",
            textures.len()
        );
    }
    Ok(textures)
}

pub struct BakedWeapon {
    pub name: String,
    /// `skel` nodes written to `weapons/<model>.nodes.ron` (card FXMF).
    pub nodes: usize,
    pub vertices: usize,
    pub triangles: usize,
    pub textures: Vec<String>,
}

/// Bake a hand-held weapon model (a static, model-space `modl` whose single
/// BNAM bone sits at identity — re-softskin.md §6; a bone-space one is posed
/// at its bind pose) to
/// `<outdir>/weapons/<model>.glb` + its textures. `lvl_path` may list several
/// LVLs, comma-joined: the model comes from the first that holds it and its
/// textures from any of them.
pub fn bake_weapon(
    lvl_path: &str,
    model: &str,
    outdir: &str,
    pc_lvl: Option<&str>,
) -> Result<BakedWeapon> {
    let pc_resolved = pc_lvl.map(crate::resolve_ci);
    let pc = pc_resolved
        .as_deref()
        .map(crate::tier::PcTextures::load)
        .transpose()?;
    let bufs = lvl_path
        .split(',')
        .map(|p| {
            let resolved = crate::resolve_ci(p);
            std::fs::read(&resolved).with_context(|| p.to_string())
        })
        .collect::<Result<Vec<_>>>()?;
    let inners = bufs
        .iter()
        .map(|b| mungefs::open(b).map_err(|e| anyhow!("{e}")))
        .collect::<Result<Vec<_>>>()?;
    let mut found = None;
    for (k, inner) in inners.iter().enumerate() {
        let chunks = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
        let m = chunks
            .iter()
            .filter(|c| &c.tag == b"modl")
            .filter_map(|c| Model::parse(c).ok())
            .find(|m| m.name.eq_ignore_ascii_case(model));
        if let Some(m) = m {
            found = Some((k, chunks, m));
            break;
        }
    }
    let (home, chunks, m) = found.ok_or_else(|| anyhow!("modl {model} not found"))?;
    let m = if m.bone_space {
        crate::bind_pose::BindPose::of(&chunks)
            .pose(&m)
            .map_err(|e| anyhow!("{model}: {e}"))?
            .ok_or_else(|| anyhow!("{model} is a bone-space model with no skeleton"))?
    } else {
        m
    };
    let out = Path::new(outdir);
    std::fs::create_dir_all(out.join("weapons"))?;
    std::fs::write(
        out.join("weapons").join(format!("{}.glb", m.name)),
        if m.name.to_ascii_lowercase().starts_with("hud_") {
            crate::glb::hud_model_to_glb(&m, &crate::alpha::legacy_per_texture_or(&m))?
        } else {
            crate::glb::model_to_glb(&m, &crate::alpha::legacy_per_texture_or(&m), false)?
        },
    )?;
    let wanted: Vec<String> = m
        .segments
        .iter()
        .filter_map(crate::alpha::segment_texture)
        .map(|n| n.to_ascii_lowercase())
        .collect();
    let flags = bake_flags(&m);
    let mut textures = Vec::new();
    for k in std::iter::once(home).chain((0..inners.len()).filter(|&k| k != home)) {
        let rest: Vec<String> = wanted
            .iter()
            .filter(|n| !textures.contains(*n))
            .cloned()
            .collect();
        if rest.is_empty() {
            break;
        }
        textures.extend(bake_textures_flagged(
            inners[k],
            &rest,
            &flags,
            out,
            pc.as_ref(),
        )?);
    }
    let nodes = weapon_nodes(&chunks, &m.name);
    std::fs::write(
        manifest::weapon_nodes_path(out, &m.name),
        ron::ser::to_string_pretty(&nodes, ron::ser::PrettyConfig::default())?,
    )?;
    Ok(BakedWeapon {
        name: m.name.clone(),
        nodes: nodes.len(),
        vertices: m.segments.iter().map(|s| s.positions.len()).sum(),
        triangles: m.segments.iter().map(|s| s.triangles.len()).sum(),
        textures,
    })
}

/// A weapon model's `skel` nodes, each at its origin in its parent's frame
/// (the node's own translation): where `FirePointName` puts the muzzle flash
/// (card FXMF, `analysis/re-muzzle-flash.md` §4.1 — B1 `FUN_00496b70`).
/// Empty when the LVL holds no skeleton for the model.
pub fn weapon_nodes(chunks: &[mungefs::Chunk<'_>], model: &str) -> manifest::WeaponNodes {
    crate::hardpoints::Skeletons::of(chunks)
        .find(model)
        .map(|s| {
            s.bones
                .iter()
                .map(|b| manifest::ManifestHardpoint {
                    name: b.name.clone(),
                    pos: b.t,
                    axes: Some(b.rows),
                })
                .collect()
        })
        .unwrap_or_default()
}
