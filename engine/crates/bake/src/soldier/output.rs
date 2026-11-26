//! Soldier and hand-held weapon output: GLBs, animation metadata and textures.
//! The parent owns model/rig conversion and the existing regression tests.

use super::{
    clip_to_tracks, find_clip, find_soldier, model_skins, pose_rig, rigid_or_skinned_model,
    BakedSoldier, ClipMeta, LvlSheets, Plan, Request, SoldierAnims, SEAT_CLIPS, SOLDIER_CLIPS,
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
/// `shoot_secondary2` overhand), played only when the class picker fields him
/// on a BF2 map. On a BF1 map he plays BF1's own, `grenade_stand_shoot3` on the
/// grenade's set (`analysis/re-grenade-throw-anim.md` §0).
pub const BORROWED_THROW: &[&str] = &[
    "human_rifle_stand_shoot_secondary_full",
    "human_rifle_stand_shoot_secondary2_full",
];

const BORROWED_BF2_STATUS: &[&str] = &["human_choking"];

/// Bake one soldier model (+ textures) from a SIDE LVL.
pub fn bake_soldier(lvl_path: &str, model: &str, outdir: &str) -> Result<BakedSoldier> {
    bake_soldier_anims(lvl_path, model, None, outdir, None)
}

/// As `bake_soldier`, plus the soldier's clips from `anim_lvl` (BF1
/// COMMON.LVL / BF2 INGAME.LVL, and for BF2 the side LVLs beside `lvl_path`)
/// as GLB animations, `<model>.anims.ron` (what was baked) and
/// `<model>.sets.ron` (the PS2's animation sets over them,
/// `anim_sets::SoldierSets`).
/// Lender LVLs set in-process ([`set_lender`]) before the `BF1_ANIMS` /
/// `BF2_ANIMS` environment variables the stock bake script sets.
static LENDERS: std::sync::Mutex<Vec<(&'static str, std::ffi::OsString)>> =
    std::sync::Mutex::new(Vec::new());

/// Name the LVL a soldier bake borrows clips from (`"BF1_ANIMS"`: BF1's
/// COMMON.LVL, lending prone to BF2; `"BF2_ANIMS"`: BF2's INGAME.LVL, lending
/// the throw to BF1), or `None` to borrow nothing. A mod conversion sets the
/// copies the stock bake used, so a replaced soldier borrows what the stock
/// one did.
pub fn set_lender(name: &'static str, path: Option<&Path>) {
    let mut l = LENDERS.lock().unwrap_or_else(|e| e.into_inner());
    l.retain(|(n, _)| *n != name);
    l.push((name, path.map(|p| p.as_os_str().to_owned()).unwrap_or_default()));
}

fn lender_path(name: &str) -> Option<std::ffi::OsString> {
    let l = LENDERS.lock().unwrap_or_else(|e| e.into_inner());
    match l.iter().find(|(n, _)| *n == name) {
        Some((_, p)) if p.is_empty() => None,
        Some((_, p)) => Some(p.clone()),
        None => std::env::var_os(name),
    }
}

/// Pre-loaded side LVL, PC texture table, and animation/class/rig context
/// shared across every soldier and weapon baked from one side LVL.
///
/// Avoids re-reading gigabyte-scale side LVLs and re-parsing all four faction
/// LVLs + animation banks for every individual soldier/weapon model.
pub struct SideBakeCache {
    buf: Vec<u8>,
    original: Option<Vec<u8>>,
    pc: Option<crate::tier::PcTextures>,
    own_classes: Vec<manifest::ClassDef>,
    own_combos: Option<manifest::combo::ComboSet>,
    anim: Option<AnimBakeCache>,
    low_same_sheets: bool,
    sheets: std::sync::OnceLock<std::result::Result<LvlSheets, String>>,
}

struct AnimBakeCache {
    abuf: Vec<u8>,
    sides: Vec<Vec<u8>>,
    lender_buf: Option<Vec<u8>>,
    sanm: Option<mungefs::sanm::SoldierAnimations>,
    bf1: bool,
    classes_pose: Vec<manifest::ClassDef>,
    classes_plan: Vec<manifest::ClassDef>,
    rigs: Vec<mungefs::anim::Rig>,
    human_rig: Option<mungefs::anim::Rig>,
}

impl SideBakeCache {
    pub fn load(
        lvl_path: &str,
        anim_lvl: Option<&str>,
        pc_lvl: Option<&str>,
    ) -> Result<Self> {
        let lvl_resolved = crate::resolve_ci(lvl_path);
        let lvl_path = lvl_resolved.as_str();
        let pc_resolved = pc_lvl.map(crate::resolve_ci);
        let pc = pc_resolved
            .as_deref()
            .map(crate::tier::PcTextures::load)
            .transpose()?;
        let (buf, original) =
            crate::gameplay::read_with_original(lvl_path).with_context(|| lvl_path.to_string())?;
        let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
        let own_classes = anim_sets::bf2::side_classes(&buf)?;
        let mut own_combos = None;

        let anim = if let Some(anim_path) = anim_lvl {
            let anim_resolved = crate::resolve_ci(anim_path);
            let abuf =
                crate::gameplay::read(&anim_resolved).with_context(|| anim_resolved.clone())?;
            let ainner = mungefs::open(&abuf).map_err(|e| anyhow!("{e}"))?;
            let sanm = mungefs::sanm::find_sanm(ainner).map_err(|e| anyhow!("{e}"))?;
            let bf1 = sanm.is_none();
            if sanm.is_some() {
                own_combos = crate::combo::baked_combos_from_lvl(&buf).ok();
            }
            let sides: Vec<Vec<u8>> = match sanm {
                Some(_) => ["ALL", "IMP", "REP", "CIS"]
                    .iter()
                    .filter_map(|s| {
                        Path::new(lvl_path)
                            .parent()
                            .map(|parent| crate::join_ci(parent, &format!("{s}.LVL")))
                    })
                    .filter(|path| path.is_file())
                    .map(crate::gameplay::read)
                    .collect::<Result<_>>()?,
                None => Vec::new(),
            };
            let mut classes_pose = own_classes.clone();
            let anim_rigs = mungefs::anim::find_rigs(ainner).map_err(|e| anyhow!("{e}"))?;
            let mut rigs = mungefs::anim::find_rigs(inner).map_err(|e| anyhow!("{e}"))?;
            let human_rig = super::human_rig(anim_rigs.iter().chain(&rigs)).cloned();
            rigs.extend(anim_rigs);
            let mut classes_plan = if sanm.is_some() {
                anim_sets::bf2::side_classes(&abuf)?
            } else {
                Vec::new()
            };
            for side in &sides {
                let s_inner = mungefs::open(side).map_err(|e| anyhow!("{e}"))?;
                let s_classes = anim_sets::bf2::side_classes(side)?;
                classes_pose.extend(s_classes.iter().cloned());
                if sanm.is_some() {
                    classes_plan.extend(s_classes);
                }
                rigs.extend(mungefs::anim::find_rigs(s_inner).map_err(|e| anyhow!("{e}"))?);
            }
            if sanm.is_some() {
                for class in &own_classes {
                    if !classes_plan
                        .iter()
                        .any(|c| c.kind == class.kind && c.name == class.name)
                    {
                        classes_plan.push(class.clone());
                    }
                }
            }
            let lender = if sanm.is_some() {
                "BF1_ANIMS"
            } else {
                "BF2_ANIMS"
            };
            let lender_buf = match lender_path(lender) {
                Some(p) => {
                    let resolved = crate::resolve_ci(&p.to_string_lossy());
                    Some(crate::gameplay::read(&resolved).with_context(|| format!("{p:?}"))?)
                }
                None => None,
            };
            Some(AnimBakeCache {
                abuf,
                sides,
                lender_buf,
                sanm,
                bf1,
                classes_pose,
                classes_plan,
                rigs,
                human_rig,
            })
        } else {
            None
        };

        Ok(Self {
            buf,
            original,
            pc,
            own_classes,
            own_combos,
            anim,
            low_same_sheets: false,
            sheets: std::sync::OnceLock::new(),
        })
    }

    /// Bake a soldier's low model only when it shares a sheet with its high
    /// model (a replacement side; `lod::bake`).
    pub fn low_needs_same_sheets(mut self) -> Self {
        self.low_same_sheets = true;
        self
    }

    pub fn pc_textures(&self) -> Option<&crate::tier::PcTextures> {
        self.pc.as_ref()
    }

    /// Free the LVLs, animation banks and sheets, keeping the PC textures.
    pub fn into_pc_textures(self) -> Option<crate::tier::PcTextures> {
        self.pc
    }

    /// The LVL's own sheets, decoded on first use and kept.
    pub(crate) fn sheets(&self) -> Result<&LvlSheets> {
        self.sheets
            .get_or_init(|| {
                mungefs::open(&self.buf)
                    .map_err(|e| anyhow!("{e}"))
                    .and_then(LvlSheets::parse)
                    .map_err(|e| format!("{e:#}"))
            })
            .as_ref()
            .map_err(|e| anyhow!("{e}"))
    }

    /// Write what `requests` ask of the LVL's sheets into `<outdir>/textures`.
    fn write_sheets(&self, requests: &[Request], outdir: &str) -> Result<()> {
        let sheets = self.sheets()?;
        let mut plan = Plan::default();
        for request in requests {
            plan.add(sheets, request);
        }
        sheets.write(&plan, &Path::new(outdir).join("textures"), self.pc.as_ref())
    }

    pub fn bake_soldier(&self, model: &str, outdir: &str) -> Result<BakedSoldier> {
        let (baked, requests) = self.soldier_model(model, outdir)?;
        self.write_sheets(&requests, outdir)?;
        Ok(baked)
    }

    /// [`Self::bake_soldier`] without its sheets: they are returned as
    /// requests, in the order to apply them.
    pub(crate) fn soldier_model(
        &self,
        model: &str,
        outdir: &str,
    ) -> Result<(BakedSoldier, Vec<Request>)> {
        let inner = mungefs::open(&self.buf).map_err(|e| anyhow!("{e}"))?;
        let (m, mut skel) = find_soldier(inner, model)?;
        crate::dlc_attachment::canonical_bind(inner, &mut skel)?;
        let mut sm = if let Some(original) = &self.original {
            let original_inner = mungefs::open(original).map_err(|e| anyhow!("{e}"))?;
            let (_, donor_skel) = find_soldier(original_inner, model)?;
            super::skinned_model_from_donor(&m, &skel, &donor_skel)?
        } else {
            rigid_or_skinned_model(&m, &skel)?
        };
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
        let first_cloth = sm.segments.len();
        let mut physics = Vec::with_capacity(cloths.len());
        for (k, c) in cloths.into_iter().enumerate() {
            physics.push(manifest::cloth::Cloth {
                part: first_cloth + k,
                ..c.physics
            });
            sm.segments.push(c.segment);
        }
        crate::dlc_attachment::append_fisto_tentacles(inner, &mut sm)?;
        let out = Path::new(outdir);
        std::fs::create_dir_all(out.join("soldiers"))?;
        std::fs::create_dir_all(out.join("textures"))?;

        let mut clips = Vec::new();
        let mut bf1 = false;
        let mut meta = SoldierAnims::default();
        if let Some(anim) = &self.anim {
            bf1 = anim.bf1;
            let ainner = mungefs::open(&anim.abuf).map_err(|e| anyhow!("{e}"))?;
            let mut banks = mungefs::anim::find_banks(ainner).map_err(|e| anyhow!("{e}"))?;
            let anim_banks = banks.len();
            banks.extend(mungefs::anim::find_banks(inner).map_err(|e| anyhow!("{e}"))?);
            for side in &anim.sides {
                let s_inner = mungefs::open(side).map_err(|e| anyhow!("{e}"))?;
                banks.extend(mungefs::anim::find_banks(s_inner).map_err(|e| anyhow!("{e}"))?);
            }
            let lender_banks = match &anim.lender_buf {
                Some(b) => {
                    let l_inner = mungefs::open(b).map_err(|e| anyhow!("{e}"))?;
                    mungefs::anim::find_banks(l_inner).map_err(|e| anyhow!("{e}"))?
                }
                None => Vec::new(),
            };
            let pose = pose_rig(&anim.classes_pose, &m.name, &anim.rigs, anim.sanm.is_some());
            let attached = match &pose {
                Some(pose) => super::append_rig_attachment(&mut skel, &pose.rig, "hp_weapons")?,
                None => false,
            };
            let folded = match pose.as_ref().map(|p| &p.rig).or(anim.human_rig.as_ref()) {
                Some(rig) => super::fold_unrigged_ancestors(&mut skel, rig, &sm),
                None => false,
            };
            if attached || folded {
                sm.bones = super::bones_of(&skel);
            }
            let bones: Vec<String> = skel.bones.iter().map(|b| b.name.clone()).collect();
            let plan = match &anim.sanm {
                None => anim_sets::plan_bf1(&anim_sets::bf1::inventory(&banks), &bones),
                Some(sanm) => {
                    let mut reg = anim_sets::bf2::Registry::shipped();
                    let soldiers = reg.register(&anim.classes_plan);
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
                let found = banks
                    .iter()
                    .filter(|b| b.name == *bank)
                    .find_map(|b| Some((b, b.clip_index(&c.name)?)));
                let (b, k) = found.ok_or_else(|| anyhow!("{}: not in bank {bank}", c.name))?;
                wanted.push((c.name.clone(), b, k));
            }
            if anim.sanm.is_some() {
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
                let bindings = crate::combo::model_bindings(&anim.classes_pose, &m.name);
                if !bindings.is_empty() {
                    let own = match &self.own_combos {
                        Some(c) => c.clone(),
                        None => crate::combo::baked_combos_from_lvl(&self.buf)?,
                    };
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
                                .with_context(|| {
                                    format!("{}: not in bank {}", clip.name, clip.bank)
                                })?;
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
            let lent = if anim.sanm.is_some() {
                BORROWED_PRONE
            } else {
                BORROWED_THROW
            };
            let lender_human: Vec<&AnimBank> = lender_banks
                .iter()
                .filter(|b| b.name.starts_with("human"))
                .collect();
            let mut borrowed = Vec::new();
            for name in lent.iter().chain(if anim.sanm.is_none() {
                BORROWED_BF2_STATUS
            } else {
                &[]
            }) {
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
                    if let Some(v) = anim_sets::root_speed(&name, &decoded, anim.sanm.is_some()) {
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
        let companion = manifest::cloth::path(out, &m.name);
        if physics.is_empty() {
            crate::remove_optional_file(&companion)?;
        } else {
            crate::write_ron(&companion, &physics)?;
        }
        let render_path = out
            .join("soldiers")
            .join(format!("{}.render-clips.glb", m.name));
        if let Some(corrected) = super::render_clips::corrected_clips(
            m.segments.iter().any(|s| s.pc_geometry),
            bf1,
            &sm,
            &clips,
        )? {
            let skeleton = crate::skin_glb::SkinnedModel {
                segments: Vec::new(),
                ..sm.clone()
            };
            std::fs::write(&render_path, skinned_to_glb(&skeleton, &corrected)?)?;
        } else if render_path.exists() {
            std::fs::remove_file(&render_path)?;
        }

        let mut wanted: Vec<String> = sm
            .segments
            .iter()
            .map(|s| s.texture.to_ascii_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        let skins = model_skins(&m, &self.own_classes);
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
        let mut requests = vec![Request {
            wanted,
            flags: flags.clone(),
        }];
        requests.extend(super::lod::bake_or_skip(
            inner,
            &self.buf,
            &m,
            &skel,
            out,
            self.low_same_sheets,
        ));
        let sheets = self.sheets()?;
        let mut textures: Vec<String> = Vec::new();
        for request in &requests {
            for texture in sheets.names(&request.wanted) {
                if !textures.contains(&texture) {
                    textures.push(texture);
                }
            }
        }
        for (tex, _) in &skins.overrides {
            if !textures.contains(tex) {
                eprintln!(
                    "soldier {}: override texture {tex} is not in the LVL",
                    m.name
                );
            }
        }

        let (min, max) =
            crate::gltf_bin::min_max3(sm.segments.iter().flat_map(|s| s.positions.iter()));
        let baked = BakedSoldier {
            name: m.name.clone(),
            clips: clips.len(),
            missing_clips: meta.missing.clone(),
            bones: skel.bones.len(),
            vertices: sm.segments.iter().map(|s| s.positions.len()).sum(),
            triangles: sm.segments.iter().map(|s| s.triangles.len()).sum(),
            textures,
            cloths: cloth_names,
            aabb: (min, max),
            texture_flags: flags,
        };
        Ok((baked, requests))
    }

    pub fn bake_weapon(
        &self,
        model: &str,
        outdir: &str,
        prior: &std::collections::HashMap<String, u32>,
    ) -> Result<BakedWeapon> {
        let (baked, mut request) = self.weapon_model(model, outdir)?;
        request.flags = merge_prior(request.flags, prior);
        self.write_sheets(&[request], outdir)?;
        Ok(baked)
    }

    /// [`Self::bake_weapon`] without its sheets: they are returned as a
    /// request carrying the model's own flags, for [`weapon_flags`]'s prior
    /// to be ORed in.
    pub(crate) fn weapon_model(&self, model: &str, outdir: &str) -> Result<(BakedWeapon, Request)> {
        let inner = mungefs::open(&self.buf).map_err(|e| anyhow!("{e}"))?;
        let chunks = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
        let m = chunks
            .iter()
            .filter(|c| &c.tag == b"modl")
            .filter_map(|c| Model::parse(c).ok())
            .find(|m| m.name.eq_ignore_ascii_case(model))
            .ok_or_else(|| anyhow!("modl {model} not found"))?;
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
                crate::glb::model_to_glb(&m, &crate::alpha::weapon_alphas(&m), false)?
            },
        )?;
        let wanted: Vec<String> = m
            .segments
            .iter()
            .filter_map(crate::alpha::segment_texture)
            .map(|n| n.to_ascii_lowercase())
            .collect();
        let textures = self.sheets()?.names(&wanted);
        let request = Request {
            wanted,
            flags: bake_flags(&m),
        };
        let nodes = weapon_nodes(&chunks, &m.name);
        std::fs::write(
            manifest::weapon_nodes_path(out, &m.name),
            ron::ser::to_string_pretty(&nodes, crate::pretty())?,
        )?;
        let baked = BakedWeapon {
            name: m.name.clone(),
            nodes: nodes.len(),
            vertices: m.segments.iter().map(|s| s.positions.len()).sum(),
            triangles: m.segments.iter().map(|s| s.triangles.len()).sum(),
            textures,
        };
        Ok((baked, request))
    }
}

pub fn bake_soldier_anims(
    lvl_path: &str,
    model: &str,
    anim_lvl: Option<&str>,
    outdir: &str,
    pc_lvl: Option<&str>,
) -> Result<BakedSoldier> {
    SideBakeCache::load(lvl_path, anim_lvl, pc_lvl)?.bake_soldier(model, outdir)
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
pub(crate) fn bake_flags(m: &mungefs::model::Model) -> std::collections::HashMap<String, u32> {
    let mut flags = texture_flags(m);
    for t in crate::spec::pass_textures([m]) {
        *flags.entry(t).or_default() |= crate::detail::ALPHA_IS_MEANINGFUL;
    }
    flags
}

/// Bake the `tex_` chunks named in `wanted` (lowercase) from an LVL to
/// `<out>/textures/<name>.ktx2`; side LVLs repeat textures, so each is
/// written once. Returns the names written. A sheet a PC bump material of
/// the LVL draws gets its normal map beside it (`crate::bump`).
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
    let sheets = LvlSheets::parse(inner)?;
    let request = Request {
        wanted: wanted.to_vec(),
        flags: tex_flags.clone(),
    };
    let mut plan = Plan::default();
    let textures = plan.add(&sheets, &request);
    sheets.write(&plan, &out.join("textures"), pc)?;
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
    bake_weapon_with(
        lvl_path,
        model,
        outdir,
        pc_lvl,
        &std::collections::HashMap::new(),
    )
}

/// [`bake_flags`] of a weapon model, ORed with `prior`: the flags an earlier
/// bake into the same folder already decided its sheets by. A weapon that
/// draws a sheet a soldier also draws (Kit Fisto's fist and foot models
/// name his body sheet with plain 0x1) must not rewrite it opaque: the
/// body's specular pass reads that alpha as its gloss weight.
pub fn weapon_flags(
    m: &mungefs::model::Model,
    prior: &std::collections::HashMap<String, u32>,
) -> std::collections::HashMap<String, u32> {
    merge_prior(bake_flags(m), prior)
}

/// `flags` with each sheet's `prior` flags ORed in; a sheet `prior` does not
/// name keeps its own.
pub(crate) fn merge_prior(
    mut flags: std::collections::HashMap<String, u32>,
    prior: &std::collections::HashMap<String, u32>,
) -> std::collections::HashMap<String, u32> {
    for (name, f) in flags.iter_mut() {
        if let Some(p) = prior.get(name) {
            *f |= p;
        }
    }
    flags
}

/// [`bake_weapon`] after a soldier bake into the same folder: `prior` holds
/// the per-sheet flags that bake wrote its textures with
/// ([`crate::soldier::BakedSoldier::texture_flags`]), so a sheet both draw
/// keeps the alpha the soldier needs ([`weapon_flags`]).
pub fn bake_weapon_with(
    lvl_path: &str,
    model: &str,
    outdir: &str,
    pc_lvl: Option<&str>,
    prior: &std::collections::HashMap<String, u32>,
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
            crate::gameplay::read(&resolved).with_context(|| p.to_string())
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
            crate::glb::model_to_glb(&m, &crate::alpha::weapon_alphas(&m), false)?
        },
    )?;
    let wanted: Vec<String> = m
        .segments
        .iter()
        .filter_map(crate::alpha::segment_texture)
        .map(|n| n.to_ascii_lowercase())
        .collect();
    let flags = weapon_flags(&m, prior);
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
        ron::ser::to_string_pretty(&nodes, crate::pretty())?,
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
