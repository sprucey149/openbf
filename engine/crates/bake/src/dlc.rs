//! Optional DLC heroes only. No addon missions, scripts, maps or core strings
//! are read. Shared class definitions are namespaced so they cannot shadow
//! the retail class database when a hero is selected.

use crate::classes::{BakedClass, ClassProps};
use crate::heroes::{HeroAssets, HeroBake};
use anyhow::{anyhow, bail, Context, Result};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

pub const HEROES: [(&str, &str); 2] = [("dlc_hero_fisto", "rep"), ("dlc_hero_ventress", "cis")];

/// The two heroes' own sounds. The voices appear only in a PS2-format source,
/// where they play from a stream [`keep_hero_sounds`] does not carry.
const SOUNDS: [&str; 11] = [
    "hero_weap_energyBall_throw",
    "hero_weap_energyBall_stdy",
    "hero_weap_energyBall_explo",
    "hero_kit_spawn",
    "hero_kit_exhausted",
    "hero_kit_AcquiredTarget",
    "hero_kit_KillingSpree4",
    "hero_ventress_spawn",
    "hero_ventress_exhausted",
    "hero_ventress_AcquiredTarget",
    "hero_ventress_KillingSpree4",
];

fn chain<'a>(classes: &'a [BakedClass], name: &str) -> Vec<&'a BakedClass> {
    let mut out = Vec::new();
    let mut name = name;
    while let Some(c) = classes.iter().find(|c| c.name.eq_ignore_ascii_case(name)) {
        if out.iter().any(|p: &&BakedClass| p.name == c.name) {
            break;
        }
        out.push(c);
        name = &c.base;
    }
    out
}

fn scalar<'a>(classes: &'a [BakedClass], name: &str, key: &str) -> Option<&'a str> {
    chain(classes, name).into_iter().find_map(|c| c.prop(key))
}

/// Only the two hero roots and their class-valued dependency closure. An
/// unrelated addon soldier, map prop or script cannot join this output.
fn hero_classes(classes: &[BakedClass]) -> Result<Vec<BakedClass>> {
    for (hero, _) in HEROES {
        let rows = chain(classes, hero);
        if rows.first().is_none_or(|c| c.kind != "entc")
            || rows.last().is_none_or(|c| c.base != "soldier")
        {
            bail!("DLC must contain soldier class {hero}");
        }
    }
    let mut wanted = BTreeSet::new();
    let mut pending: Vec<_> = HEROES.iter().map(|h| h.0.to_owned()).collect();
    while let Some(name) = pending.pop() {
        if !wanted.insert(name.clone()) {
            continue;
        }
        let Some(c) = classes.iter().find(|c| c.name == name) else {
            continue;
        };
        for reference in
            std::iter::once(c.base.as_str()).chain(c.props.iter().map(|p| p.1.as_str()))
        {
            if classes.iter().any(|c| c.name == reference) {
                pending.push(reference.to_owned());
            }
        }
    }
    Ok(classes
        .iter()
        .filter(|c| wanted.contains(&c.name))
        .cloned()
        .collect())
}

/// The PC copy's Ventress answers squad orders with the Republic's line, where
/// the PS2 copy names the CIS's. She is CIS, so she takes that one in both.
fn answer_for_her_own_side(classes: &mut [BakedClass]) {
    const REPUBLIC: &str = "rep_off_response_hero_command";
    const CIS: &str = "cis_off_response_hero_command";
    let voice = mungefs::fnv1a_lowercase("VOSound");
    for class in classes.iter_mut().filter(|c| c.name == "dlc_hero_ventress") {
        for (_, value) in class.props.iter_mut().filter(|p| p.0 == voice) {
            if let Some(event) = value.strip_prefix(REPUBLIC) {
                *value = format!("{CIS}{event}");
            }
        }
    }
}

/// Kit Fisto's Force ball: the add-on (PC and PS2 copies alike) tags it with
/// the droideka's `hud_droideka_shield`, a droideka figure whose shield is a
/// faint arc. `[ours]` it takes its own key, drawn by [`fisto_bubble_icon`]
/// (`game::hud_feed`'s `fisto` places it where the droideka icon sits).
/// Droidekas keep theirs.
const FORCE_BUBBLE: &str = "dlc_weap_force_bubble";
const DROIDEKA_HUD: &str = "hud_droideka_shield";
pub const FISTO_BUBBLE_HUD: &str = "hud_fisto_bubble";

fn fisto_hud_tag(classes: &mut [BakedClass]) {
    let tag = mungefs::fnv1a_lowercase("HUDTag");
    for class in classes.iter_mut().filter(|c| c.name == FORCE_BUBBLE) {
        for (_, value) in class.props.iter_mut().filter(|p| p.0 == tag) {
            if value.trim().eq_ignore_ascii_case(DROIDEKA_HUD) {
                *value = FISTO_BUBBLE_HUD.to_owned();
            }
        }
    }
}

/// `[ours]` the Force ball's HUD sheet, 64x64 like the weapon icons it sits
/// among and in their ink: opaque, dark strokes on white. A ball (an outline)
/// with a short shine arc inside its upper left; no droideka. Pure pixel
/// code, so every bake writes the same bytes.
pub fn fisto_bubble_icon() -> Vec<u8> {
    const SIZE: usize = 64;
    const INK: f32 = 14.0;
    // Stroke coverage for a pixel whose centre is `d` from the stroke's
    // middle line, `half` its half-width: a one-pixel ramp at the edges.
    let cover = |d: f32, half: f32| (half + 0.5 - d.abs()).clamp(0.0, 1.0);
    let centre = (SIZE as f32 - 1.0) * 0.5;
    let mut out = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (x as f32 - centre, y as f32 - centre);
            let r = (dx * dx + dy * dy).sqrt();
            let ring = cover(r - 22.0, 2.5);
            // The shine: radius 15, from 200 to 250 degrees (screen y down,
            // so the upper left), its ends rounded off over 6 degrees.
            let angle = dy.atan2(dx).to_degrees().rem_euclid(360.0);
            let along = ((angle - 200.0).min(250.0 - angle) / 6.0).clamp(0.0, 1.0);
            let shine = cover(r - 15.0, 1.5) * along;
            let ink = ring.max(shine);
            let v = (255.0 - (255.0 - INK) * ink).round() as u8;
            out.extend([v, v, v, 255]);
        }
    }
    out
}

/// The flat sheet the Force ball's inner shell draws where the source copy
/// lacks the `spec` texture it names (the PC `dlc.lvl` has none, nor do the
/// PC core and common LVLs; the PS2 copy's is used whenever it is the source).
/// `[ours]` the PS2 copy's sheet is one flat cyan, about (0, 192, 220) at
/// alpha 125..178: this is that colour at its least alpha, 8x8.
/// replace: none while a PC-only source lacks the sheet.
pub fn fisto_spec_sheet() -> Vec<u8> {
    [0u8, 192, 220, 125].repeat(64)
}

fn namespace(classes: &mut [BakedClass]) {
    let names: HashMap<_, _> = classes
        .iter()
        .map(|c| {
            let name = if HEROES.iter().any(|h| h.0 == c.name) {
                c.name.clone()
            } else {
                format!("dlch_{}", c.name)
            };
            (c.name.clone(), name)
        })
        .collect();
    for c in classes {
        c.name = names[&c.name].clone();
        if let Some(base) = names.get(&c.base) {
            c.base = base.clone();
        }
        for (_, value) in &mut c.props {
            if let Some(name) = names.get(value) {
                *value = name.clone();
            }
        }
    }
}

/// Bake into an owned asset root (the directory containing `classes` and
/// `side_bf2`). `lvl` is a content-detected PC/PS2 side; `ingame` supplies the
/// normal PS2 animation registry. The manifest is written last: it enables
/// runtime selection only after all required assets have succeeded.
pub fn bake(lvl: &Path, ingame: &Path, out: &Path, pc: Option<&Path>) -> Result<HeroBake> {
    let destination = out.join("side_bf2");
    if out.ancestors().any(Path::is_symlink)
        || destination.is_symlink()
        || out.join("classes").is_symlink()
        || out.join("classes/bf2_dlc.ron").is_symlink()
    {
        bail!("DLC output must be owned directories, not shared symlinks");
    }
    // Disable an old installation even when the replacement cannot be parsed.
    crate::remove_optional_file(&destination.join("dlc_heroes.ron"))?;
    let staging = crate::dlc_output::Staging::new()?;
    let side = staging.path();
    let pc_path = pc.map(|p| p.to_string_lossy());
    crate::gameplay::exempt_addon(lvl);
    let bytes = std::fs::read(lvl).with_context(|| lvl.display().to_string())?;
    let all = crate::classes::read_classes(
        &bytes,
        &crate::classes::CLASS_TAGS,
        crate::classes::MalformedClass::Strict,
    )?;
    let mut classes = hero_classes(&all)?;
    answer_for_her_own_side(&mut classes);
    fisto_hud_tag(&mut classes);
    let mut result = HeroBake::default();
    // The sheets the bodies were written with: a hero's weapon model that
    // draws one of them (Kit's fists and foot draw his body sheet) bakes it
    // again, and must keep the alpha the body decided.
    let mut body_flags: HashMap<String, u32> = HashMap::new();
    for (hero, faction) in HEROES {
        let geometry = scalar(&classes, hero, "GeometryName")
            .with_context(|| format!("{hero}: no GeometryName"))?
            .to_ascii_lowercase();
        let expected = if faction == "rep" {
            "rep_inf_kitfisto"
        } else {
            "cis_inf_asajjventress"
        };
        if geometry != expected {
            bail!("{hero}: unsupported geometry {geometry}");
        }
        crate::remove_optional_file(&side.join("soldiers").join(format!("{geometry}.combos.ron")))?;
        let baked = crate::soldier::bake_soldier_anims(
            &lvl.to_string_lossy(),
            &geometry,
            Some(&ingame.to_string_lossy()),
            &side.to_string_lossy(),
            pc_path.as_deref(),
        )
        .with_context(|| format!("DLC {hero}"))?;
        if baked.clips == 0
            || !side
                .join("soldiers")
                .join(format!("{geometry}.combos.ron"))
                .is_file()
        {
            bail!("{hero}: missing playable animations/weapon combo");
        }
        result.heroes.push(HeroAssets {
            class: hero.into(),
            geometry: geometry.clone(),
            side: faction.into(),
        });
        result.soldiers.push(geometry);
        result.textures.extend(baked.textures);
        for (name, flags) in baked.texture_flags {
            *body_flags.entry(name).or_default() |= flags;
        }
    }
    let inner = mungefs::open(&bytes)?;
    let chunks = mungefs::content_chunks(inner)?;
    let mut models = BTreeSet::new();
    let mut textures = BTreeSet::new();
    for c in &classes {
        for key in [
            "GeometryName",
            "OffhandGeometryName",
            "DroppedItemGeometryName",
            "HUDTag",
            "IconTexture",
            "LightSaberTexture",
        ] {
            let hash = mungefs::fnv1a_lowercase(key);
            for (_, value) in c.props.iter().filter(|p| p.0 == hash) {
                let value = value.trim().to_ascii_lowercase();
                if result.soldiers.contains(&value) {
                    continue;
                }
                if chunks.iter().any(|c| {
                    &c.tag == b"modl"
                        && c.child_string(b"NAME")
                            .is_some_and(|n| n.eq_ignore_ascii_case(&value))
                }) {
                    models.insert(value);
                } else if chunks.iter().any(|c| {
                    &c.tag == b"tex_"
                        && c.child_string(b"NAME")
                            .is_some_and(|n| n.eq_ignore_ascii_case(&value))
                }) {
                    textures.insert(value);
                } else if matches!(
                    key,
                    "GeometryName" | "OffhandGeometryName" | "LightSaberTexture"
                ) && !value.is_empty()
                    && value != "none"
                {
                    return Err(anyhow!("{}: missing {key} asset {value}", c.name));
                }
            }
        }
    }
    let mut weapon_textures = Vec::new();
    for model in models {
        let baked = crate::soldier::bake_weapon_with(
            &lvl.to_string_lossy(),
            &model,
            &side.to_string_lossy(),
            pc_path.as_deref(),
            &body_flags,
        )?;
        weapon_textures.extend(baked.textures);
        result.weapons.push(model);
    }
    // The Force ball's inner shell names `spec`; a source without it gets
    // [`fisto_spec_sheet`] rather than an untextured solid ball.
    if result.weapons.iter().any(|w| w == "bubblev3")
        && !weapon_textures.iter().any(|t| t == "spec")
    {
        std::fs::create_dir_all(side.join("textures"))?;
        std::fs::write(
            side.join("textures/spec.ktx2"),
            crate::ktx2w::write_rgba8(8, 8, &[fisto_spec_sheet()]),
        )?;
        result.textures.push("spec".to_owned());
    }
    std::fs::create_dir_all(side.join("textures"))?;
    std::fs::write(
        side.join("textures")
            .join(format!("{FISTO_BUBBLE_HUD}.ktx2")),
        crate::ktx2w::write_rgba8(64, 64, &[fisto_bubble_icon()]),
    )?;
    result.textures.push(FISTO_BUBBLE_HUD.to_owned());
    let flags = textures
        .iter()
        .map(|t| (t.clone(), crate::detail::ALPHA_IS_MEANINGFUL))
        .collect();
    let pc_textures = pc_path
        .as_deref()
        .map(crate::tier::PcTextures::load)
        .transpose()?;
    result
        .textures
        .extend(crate::soldier::bake_textures_flagged(
            inner,
            &textures.into_iter().collect::<Vec<_>>(),
            &flags,
            side,
            pc_textures.as_ref(),
        )?);
    result.textures.sort();
    result.textures.dedup();
    // Their effects stay in a layer of their own, read after every retail one.
    crate::effects::bake_side_with(
        &lvl.to_string_lossy(),
        &side.join("effects").to_string_lossy(),
        "bf2",
        true,
    )?;
    crate::dlc_output::install(side, &destination, &mut classes, &mut result)?;
    crate::dlc_output::replace_tree(&side.join("effects"), &destination.join("effects/dlc"))?;
    namespace(&mut classes);
    crate::write_ron(&out.join("classes/bf2_dlc.ron"), &classes)?;
    crate::write_ron(&destination.join("dlc_heroes.ron"), &result)?;
    Ok(result)
}

/// Only the two heroes' sounds that play from a sample of this bank, and only
/// those samples.
fn keep_hero_sounds(mut sounds: crate::sounds::SoundsRon) -> crate::sounds::SoundsRon {
    let wanted: Vec<u32> = SOUNDS.iter().map(|n| mungefs::fnv1a_lowercase(n)).collect();
    sounds
        .props
        .retain(|p| wanted.contains(&p.name_hash) && p.stream.is_none() && !p.samples.is_empty());
    let used: BTreeSet<u32> = sounds
        .props
        .iter()
        .flat_map(|p| p.samples.iter().map(|s| s.0))
        .collect();
    sounds.samples.retain(|(id, _)| used.contains(id));
    sounds.banks.clear();
    sounds.engines.clear();
    sounds.music.clear();
    sounds.streamed = 0;
    sounds
}

/// Bake the heroes' sound bank into `side_bf2/dlc_sounds`. Everything else in
/// the bank stays behind; the game reads this folder only beside the marker.
pub fn bake_sounds(lvl: &Path, core: &Path, out: &Path) -> Result<usize> {
    let destination = out.join("side_bf2/dlc_sounds");
    if out.ancestors().any(Path::is_symlink) || destination.is_symlink() {
        bail!("DLC output must be owned directories, not shared symlinks");
    }
    let staging = crate::dlc_output::Staging::new()?;
    let source = staging.path().join("source");
    let sounds = keep_hero_sounds(crate::sounds::bake_sounds(
        &lvl.to_string_lossy(),
        &source.to_string_lossy(),
        Some(&core.to_string_lossy()),
    )?);
    if sounds.props.is_empty() {
        bail!("{}: no sample of the DLC heroes' sounds", lvl.display());
    }
    let keep = staging.path().join("keep");
    for (_, sample) in &sounds.samples {
        let target = keep.join(sample);
        std::fs::create_dir_all(target.parent().context("sample has no parent")?)?;
        std::fs::copy(source.join("sounds").join(sample), target)?;
    }
    crate::write_ron(&keep.join("sounds.ron"), &sounds)?;
    crate::dlc_output::replace_tree(&keep, &destination)?;
    Ok(sounds.props.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_heroes_own_sampled_sounds_leave_the_bank() {
        let prop = |name: &str, sample: u32, stream: Option<u32>| {
            let mut prop: crate::sounds::SoundPropRon = ron::from_str(
                "(name_hash:0,kind:0,gain:1.0,pitch:1.0,min_distance:1.0,max_distance:2.0,\
                 rolloff:1.0,priority:0.5,looping:false,samples:[],layers:[])",
            )
            .unwrap();
            prop.name_hash = mungefs::fnv1a_lowercase(name);
            prop.samples = vec![(sample, 1.0)];
            prop.stream = stream;
            prop
        };
        let sounds = crate::sounds::SoundsRon {
            source: "hero".into(),
            banks: vec![(1, 3)],
            samples: vec![
                (1, "a.wav".into()),
                (2, "b.wav".into()),
                (3, "c.wav".into()),
            ],
            props: vec![
                prop("hero_weap_energyBall_throw", 1, None),
                prop("rep_inf_soldier_chatter", 2, None),
                prop("hero_kit_spawn", 3, Some(9)),
            ],
            odd_frequency: 0,
            streamed: 1,
            engines: vec![],
            music: vec![],
            triggers: vec![],
        };
        let kept = keep_hero_sounds(sounds);
        assert_eq!(kept.props.len(), 1);
        assert_eq!(kept.samples, [(1, "a.wav".to_owned())]);
        assert_eq!(kept.streamed, 0);
    }

    fn class(name: &str, base: &str, props: &[(&str, &str)]) -> BakedClass {
        BakedClass {
            name: name.into(),
            kind: "entc".into(),
            base: base.into(),
            props: props
                .iter()
                .map(|(k, v)| (mungefs::fnv1a_lowercase(k), (*v).into()))
                .collect(),
        }
    }

    #[test]
    fn only_two_heroes_and_dependencies_can_escape_pack() {
        let source = vec![
            class("dlc_hero_fisto", "shared", &[("WeaponName", "saber")]),
            class("dlc_hero_ventress", "shared", &[]),
            class("shared", "soldier", &[]),
            class("saber", "melee", &[]),
            class("unrelated_map_prop", "prop", &[]),
            class("dlc_hero_fisto_splitscreen", "shared", &[]),
        ];
        let mut selected = hero_classes(&source).unwrap();
        assert_eq!(selected.len(), 4);
        namespace(&mut selected);
        assert_eq!(selected[0].base, "dlch_shared");
        assert_eq!(selected[0].prop("WeaponName"), Some("dlch_saber"));
        assert_eq!(selected[2].base, "soldier");
        assert!(hero_classes(&source[1..]).is_err());
    }

    #[test]
    fn ventress_answers_orders_in_her_own_sides_voice() {
        let voice = "VOSound";
        let mut classes = vec![
            class(
                "dlc_hero_ventress",
                "shared",
                &[
                    (voice, "rep_off_response_hero_command      SC_Follow"),
                    (voice, "cis_off_response_hero_command      SC_GetIn"),
                    ("GeometryName", "rep_off_response_hero_command"),
                ],
            ),
            class(
                "dlc_hero_fisto",
                "shared",
                &[(voice, "rep_off_response_hero_command      SC_Follow")],
            ),
        ];
        answer_for_her_own_side(&mut classes);
        assert_eq!(
            classes[0].props[0].1,
            "cis_off_response_hero_command      SC_Follow"
        );
        assert_eq!(
            classes[0].props[1].1,
            "cis_off_response_hero_command      SC_GetIn"
        );
        assert_eq!(classes[0].props[2].1, "rep_off_response_hero_command");
        assert_eq!(
            classes[1].props[0].1,
            "rep_off_response_hero_command      SC_Follow"
        );
    }

    #[test]
    fn force_bubble_hud_tag_is_rewritten() {
        let tag = "HUDTag";
        let mut classes = vec![
            class(
                "dlc_weap_force_bubble",
                "com_weap_inf_force_pull",
                &[(tag, "hud_droideka_shield"), ("Label", "Shield")],
            ),
            class(
                "cis_weap_inf_droideka_shield",
                "cannon",
                &[(tag, "hud_droideka_shield")],
            ),
            class(
                "com_weap_inf_force_pull",
                "cannon",
                &[(tag, "hud_force_pull")],
            ),
        ];
        fisto_hud_tag(&mut classes);
        assert_eq!(classes[0].prop(tag), Some(FISTO_BUBBLE_HUD));
        assert_eq!(
            classes[0].prop("Label"),
            Some("Shield"),
            "the original name stays"
        );
        assert_eq!(classes[1].prop(tag), Some("hud_droideka_shield"));
        assert_eq!(classes[2].prop(tag), Some("hud_force_pull"));
        namespace(&mut classes);
        assert_eq!(
            classes[0].prop(tag),
            Some(FISTO_BUBBLE_HUD),
            "a key, not a class"
        );
    }

    #[test]
    fn fisto_bubble_icon_is_an_opaque_ball_in_the_weapon_icons_ink() {
        let icon = fisto_bubble_icon();
        assert_eq!(icon, fisto_bubble_icon(), "deterministic");
        assert_eq!(icon.len(), 64 * 64 * 4);
        let px = |x: usize, y: usize| &icon[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert!(icon
            .chunks(4)
            .all(|p| p[3] == 255 && p[0] == p[1] && p[1] == p[2]));
        assert_eq!(px(32, 32), [255, 255, 255, 255], "clear inside");
        assert_eq!(px(1, 1), [255, 255, 255, 255], "clear outside");
        // The ring crosses the row through the centre 22 px out.
        assert!(px(31 + 22, 31)[0] < 40 && px(31 - 22 + 1, 32)[0] < 40);
        // The shine is in the upper left only.
        let ink = |x: usize, y: usize| px(x, y)[0] < 128;
        let r = 15.0f32;
        let at = |deg: f32| {
            let (s, c) = deg.to_radians().sin_cos();
            (
                (31.5 + r * c).round() as usize,
                (31.5 + r * s).round() as usize,
            )
        };
        let (x, y) = at(225.0);
        assert!(ink(x, y), "shine at 225 degrees");
        for deg in [45.0, 135.0, 315.0] {
            let (x, y) = at(deg);
            assert!(!ink(x, y), "no shine at {deg} degrees");
        }
        let dark = icon.chunks(4).filter(|p| p[0] < 128).count();
        assert!((500..1200).contains(&dark), "{dark} ink texels");
    }

    #[test]
    fn failed_source_read_disables_previous_installation() {
        let root = std::env::temp_dir().join(format!("dlc_failed_bake_{}", std::process::id()));
        std::fs::create_dir_all(root.join("side_bf2")).unwrap();
        let marker = root.join("side_bf2/dlc_heroes.ron");
        std::fs::write(&marker, "previous installation").unwrap();
        let missing = root.join("missing.lvl");
        assert!(bake(&missing, &missing, &root, None).is_err());
        assert!(!marker.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
