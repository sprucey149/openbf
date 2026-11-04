//! Recover donor drawing dependencies and verified larger versions of coarse
//! atlases. Called only for corrected donor inputs; PS2 output stays unchanged.
use crate::tier::{PcTextures, Resolved, Usage};
use anyhow::{anyhow, Result};
use mungefs::{texture::Texture, Chunk};

pub(super) fn recover(
    expanded: &[Chunk<'_>],
    pc: Option<&PcTextures>,
    all_textures: &mut Vec<Texture>,
    primary_pc: bool,
) -> Result<()> {
    if primary_pc {
        own_pc_replacements(all_textures);
    }
    // Only donor gameplay streams enter this path; a PS2 bake stays identical.
    // Import dependencies before alpha classification, so a recovered glow or
    // cutout uses its real mask rather than the missing-image default.
    if let Some(donors) = pc {
        let mut wanted = std::collections::BTreeSet::new();
        for c in expanded.iter().filter(|c| &c.tag == b"modl") {
            let model = mungefs::model::Model::parse(c).map_err(|e| anyhow!("{e}"))?;
            wanted.extend(
                model
                    .segments
                    .iter()
                    .flat_map(|s| s.textures.iter())
                    .map(|(_, n)| n.to_ascii_lowercase())
                    .filter(|n| !n.is_empty()),
            );
        }
        for name in wanted {
            if !all_textures
                .iter()
                .any(|t| t.name.eq_ignore_ascii_case(&name))
            {
                if let Some(t) = donors.missing(&name) {
                    eprintln!("render texture recovery: {name} from supplied donor LVL");
                    all_textures.push(t);
                }
            }
        }
    }
    // The Xbox's chunk asks for the PS2 atlas name, but its intact turret
    // carries the same painting and UV layout under this name (measured
    // luma correlation .99165, layout .99976). Only known corrected inputs
    // reach this branch, and an exact supplied sheet always wins.
    if expanded.iter().any(|c| {
        &c.tag == b"modl"
            && c.child_string(b"NAME")
                .is_some_and(|n| n.eq_ignore_ascii_case("imp_bldg_boxturret_chunk1"))
    }) && !all_textures
        .iter()
        .any(|t| t.name.eq_ignore_ascii_case("spa1_weap_boxturret"))
    {
        if let Some(t) = all_textures
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case("imp_bldg_boxturret"))
        {
            // Both PS2 names decode identically; this Xbox sheet is the measured
            // larger copy. Unknown artwork must remain unresolved.
            let hash = t.rgba8(0).iter().fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            });
            if (t.info.width, t.info.height, hash) != (256, 256, 0x25774f31b853322a) {
                return Ok(());
            }
            let alias = crate::tier::Resolved::from_ps2(t).as_texture("spa1_weap_boxturret");
            eprintln!("render texture recovery: spa1_weap_boxturret <- own imp_bldg_boxturret");
            all_textures.push(alias);
        }
    }
    Ok(())
}

/// The PC models themselves name two sheets absent from both retail PC
/// installs. Deliberate presentation replacements: the map's snow painting
/// and a crop of its blue console display. These are not same-art aliases.
/// Exact supplied sheets win; changed source paintings remain unresolved.
fn own_pc_replacements(textures: &mut Vec<Texture>) {
    for (missing, source, width, height, hash, crop) in [
        (
            "rhn_bldg_snow_2",
            "rhn2_main_1",
            256,
            256,
            0x67e6_ca4f_ccab_11fa,
            None,
        ),
        (
            "mus1_prop_console_fx",
            "tan4_prop_console",
            512,
            512,
            0xb40f_7442_ecff_a762,
            Some([64, 112, 192, 160]),
        ),
    ] {
        if textures
            .iter()
            .any(|t| t.name.eq_ignore_ascii_case(missing))
        {
            continue;
        }
        let Some(t) = textures
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(source))
        else {
            continue;
        };
        let pixels = t.rgba8(0);
        let actual = pixels.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3)
        });
        if (t.info.width, t.info.height, actual) != (width, height, hash) {
            continue;
        }
        let mut replacement = Resolved::from_ps2(t);
        if let Some([x, y, w, h]) = crop {
            let mut cropped = Vec::with_capacity((w * h * 4) as usize);
            for row in y..y + h {
                let start = ((row * width + x) * 4) as usize;
                cropped.extend_from_slice(&pixels[start..start + (w * 4) as usize]);
            }
            replacement.width = w;
            replacement.height = h;
            replacement.mips = vec![cropped];
        }
        textures.push(replacement.as_texture(missing));
        eprintln!("render texture replacement: {missing} <- own {source}, crop {crop:?}");
    }
}

pub(super) fn larger_own(
    lvl_path: &str,
    t: &Texture,
    all_textures: &[Texture],
    usage: Usage,
    mut tier2: Option<Resolved>,
) -> Result<Option<Resolved>> {
    let lower = t.name.to_ascii_lowercase();
    // Some recovered/far surfaces still name their copy's coarse atlas.
    // Offer the full atlas as a modern tier only after the same-art
    // comparison; a similarly named but differently laid out sheet fails.
    if !t.info.detail_compressed {
        if let Some(full) = lower
            .strip_suffix("_lowrez")
            .and_then(|n| all_textures.iter().find(|x| x.name.eq_ignore_ascii_case(n)))
        {
            let larger = !full.info.detail_compressed
                && full.info.width > t.info.width
                && full.info.height > t.info.height;
            let better = tier2
                .as_ref()
                .is_none_or(|r| full.info.width > r.width && full.info.height > r.height);
            if larger
                && better
                && crate::tier::same_art(
                    &crate::tier::compare(
                        &t.rgba8(0),
                        t.info.width,
                        t.info.height,
                        &full.rgba8(0),
                        full.info.width,
                        full.info.height,
                        usage,
                    ),
                    usage,
                )
            {
                let mut own = crate::tier::Resolved::from_ps2(full);
                own.tier = if crate::xbox::lvl_is_xbox(lvl_path)? {
                    crate::tier::Tier::Xbox
                } else {
                    crate::tier::Tier::Pc
                };
                eprintln!(
                    "render texture recovery: {lower} tier 2 <- own {}",
                    full.name
                );
                tier2 = Some(own);
            }
        }
    }
    Ok(tier2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "owned PC maps: OPENBF_PC_RHN2 and OPENBF_PC_MUS1"]
    fn owned_pc_replacements_preserve_exact_sheets_and_refuse_changed_art() {
        for (key, missing, source, size) in [
            (
                "OPENBF_PC_RHN2",
                "rhn_bldg_snow_2",
                "rhn2_main_1",
                (256, 256),
            ),
            (
                "OPENBF_PC_MUS1",
                "mus1_prop_console_fx",
                "tan4_prop_console",
                (192, 160),
            ),
        ] {
            let bytes = std::fs::read(std::env::var(key).expect(key)).unwrap();
            let items = mungefs::content_chunks(mungefs::open(&bytes).unwrap()).unwrap();
            let source = items
                .iter()
                .filter(|c| &c.tag == b"tex_")
                .filter_map(|c| Texture::parse(c).ok())
                .find(|t| t.name == source)
                .unwrap();
            let mut textures = vec![Resolved::from_ps2(&source).as_texture(&source.name)];
            own_pc_replacements(&mut textures);
            assert_eq!(textures.len(), 2);
            assert_eq!(textures[1].name, missing);
            assert_eq!(textures[1].mip_dims(0), size);
            let exact = textures[1].rgba8(0);
            own_pc_replacements(&mut textures);
            assert_eq!(textures.len(), 2);
            assert_eq!(textures[1].rgba8(0), exact);
            textures.truncate(1);
            textures[0].mips[0][100] ^= 1;
            own_pc_replacements(&mut textures);
            assert_eq!(textures.len(), 1, "changed {key}");
        }
    }

    #[test]
    #[ignore = "owned Xbox/PS2 space LVLs: OPENBF_DONOR_TURRET and OPENBF_PS2_TURRET"]
    fn owned_turret_alias_refuses_changed_art() {
        let read = |key| std::fs::read(std::env::var(key).expect(key)).unwrap();
        let bytes = read("OPENBF_DONOR_TURRET");
        let items = mungefs::content_chunks(mungefs::open(&bytes).unwrap()).unwrap();
        let textures = || {
            items
                .iter()
                .filter(|c| &c.tag == b"tex_")
                .filter_map(|c| Texture::parse(c).ok())
                .collect::<Vec<_>>()
        };
        let reference = read("OPENBF_PS2_TURRET");
        let reference = mungefs::content_chunks(mungefs::open(&reference).unwrap()).unwrap();
        let find = |name: &str| {
            reference
                .iter()
                .filter(|c| &c.tag == b"tex_")
                .filter_map(|c| Texture::parse(c).ok())
                .find(|t| t.name.eq_ignore_ascii_case(name))
                .unwrap()
        };
        let canonical = find("spa1_weap_boxturret");
        assert_eq!(canonical.rgba8(0), find("imp_bldg_boxturret").rgba8(0));
        let mut good = textures();
        assert!(!good.iter().any(|t| t.name == "spa1_weap_boxturret"));
        recover(&items, None, &mut good, false).unwrap();
        let alias = good
            .iter()
            .find(|t| t.name == "spa1_weap_boxturret")
            .unwrap();
        assert_eq!(
            alias.rgba8(0),
            good.iter()
                .find(|t| t.name == "imp_bldg_boxturret")
                .unwrap()
                .rgba8(0)
        );
        for index in [0, 200, 4095] {
            let mut bad = textures();
            let original = bad
                .iter_mut()
                .find(|t| t.name == "imp_bldg_boxturret")
                .unwrap();
            *original = Resolved::from_ps2(original).as_texture("imp_bldg_boxturret");
            original.mips[0][index] ^= 1;
            recover(&items, None, &mut bad, false).unwrap();
            assert!(!bad.iter().any(|t| t.name == "spa1_weap_boxturret"));
        }
        let mut exact = textures();
        exact.push(find("spa1_weap_boxturret"));
        recover(&items, None, &mut exact, false).unwrap();
        assert_eq!(
            exact
                .iter()
                .filter(|t| t.name == "spa1_weap_boxturret")
                .count(),
            1
        );
        assert_eq!(
            exact
                .iter()
                .find(|t| t.name == "spa1_weap_boxturret")
                .unwrap()
                .rgba8(0),
            canonical.rgba8(0)
        );
    }
}
