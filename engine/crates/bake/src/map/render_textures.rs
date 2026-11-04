//! Recover donor drawing dependencies and verified larger versions of coarse
//! atlases. Called only for corrected donor inputs; PS2 output stays unchanged.
use crate::tier::{PcTextures, Resolved, Usage};
use anyhow::{anyhow, Result};
use mungefs::{texture::Texture, Chunk};
use std::{
    fs::{self, File, Metadata},
    io::{self, Read},
    path::Path,
};

const MUSTAFAR_DISPLAY: &str = "mus1_prop_console_fx";
const TANTIVE_CONSOLE: &str = "tan4_prop_console";
const DISPLAY_CROP: [u32; 4] = [64, 112, 192, 160];
// Whole retail Xbox BF2 TAN1.LVL size. This optional read needs no larger file.
const TANTIVE_LVL_MAX_BYTES: u64 = 10_228_324;

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
/// A map-local exact sheet wins. The fingerprinted PC substitutes are
/// checked before looking in supplied donor LVLs.
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
            MUSTAFAR_DISPLAY,
            TANTIVE_CONSOLE,
            512,
            512,
            0xb40f_7442_ecff_a762,
            Some(DISPLAY_CROP),
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
        textures.push(replacement(t, missing, crop));
        eprintln!("render texture replacement: {missing} <- own {source}, crop {crop:?}");
    }
}

fn replacement(source: &Texture, name: &str, crop: Option<[u32; 4]>) -> Texture {
    let mut image = Resolved::from_ps2(source);
    if let Some([x, y, w, h]) = crop {
        let pixels = &image.mips[0];
        let mut cropped = Vec::with_capacity((w * h * 4) as usize);
        for row in y..y + h {
            let start = ((row * image.width + x) * 4) as usize;
            cropped.extend_from_slice(&pixels[start..start + (w * 4) as usize]);
        }
        image.width = w;
        image.height = h;
        image.mips = vec![cropped];
    }
    image.as_texture(name)
}

/// OURS: the Xbox models request a sheet absent from all 185 Xbox BF2 LVLs.
/// Use the blue display crop from the same disc's Tantive IV atlas.
/// replace: an authenticated exact Xbox Mustafar sheet if one is found.
pub(super) fn recover_xbox_mustafar(
    lvl: &Path,
    original: &[u8],
    textures: &mut Vec<Texture>,
) -> Result<()> {
    let Some(mus) = lvl.parent().filter(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("MUS"))
    }) else {
        return Ok(());
    };
    if !lvl
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("MUS1.LVL"))
        || textures
            .iter()
            .any(|t| t.name.eq_ignore_ascii_case(MUSTAFAR_DISPLAY))
        || crate::gameplay::digest(original)
            != "a239cc10e6f57d22203d6754060caf32c96cc09cec913f6a3902deb4bca0f76c"
    {
        return Ok(());
    }
    let Some(root) = mus.parent() else {
        return Ok(());
    };
    let root = if root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        root
    };
    let path = crate::join_ci(root, "TAN/TAN1.LVL");
    let bytes = match read_tantive_atlas(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("render texture replacement: {}: {error}", path.display());
            return Ok(());
        }
    };
    let atlas = mungefs::open(&bytes)
        .and_then(mungefs::content_chunks)
        .ok()
        .and_then(|items| {
            items.into_iter().find(|c| {
                &c.tag == b"tex_"
                    && c.child_string(b"NAME")
                        .is_some_and(|name| name.eq_ignore_ascii_case(TANTIVE_CONSOLE))
            })
        })
        .filter(|c| {
            // The full Xbox TEX_ payload pins the 512x512 DXT1 format,
            // all mip data, and the layout used by DISPLAY_CROP.
            crate::gameplay::digest(c.payload)
                == "87929229ae8fba633c25cec1c3b51bc52f154f5179426fd8b37d9fd55e995d1a"
        });
    let Some(atlas) = atlas else {
        eprintln!(
            "render texture replacement: unrecognized Xbox atlas in {}",
            path.display()
        );
        return Ok(());
    };
    let atlas = Texture::parse(&atlas).map_err(|e| anyhow!("{e}"))?;
    textures.push(replacement(&atlas, MUSTAFAR_DISPLAY, Some(DISPLAY_CROP)));
    eprintln!(
        "render texture replacement: {MUSTAFAR_DISPLAY} <- own {TANTIVE_CONSOLE} from {}, crop {DISPLAY_CROP:?}",
        path.display()
    );
    Ok(())
}

fn read_tantive_atlas(path: &Path) -> io::Result<Vec<u8>> {
    let valid = |meta: Metadata| meta.is_file() && meta.len() <= TANTIVE_LVL_MAX_BYTES;
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "expected a regular file no larger than the retail Xbox TAN1.LVL",
        )
    };
    if !valid(fs::symlink_metadata(path)?) {
        return Err(invalid());
    }
    let file = File::open(path)?;
    if !valid(file.metadata()?) {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    file.take(TANTIVE_LVL_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > TANTIVE_LVL_MAX_BYTES {
        return Err(invalid());
    }
    Ok(bytes)
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
    #[ignore = "owned Xbox and PS2 BF2 roots and an empty OPENBF_MUSTAFAR_TEST_OUT"]
    fn owned_xbox_mustafar_uses_only_the_known_tantive_display() {
        use std::{fs, path::PathBuf};

        let input = |key| PathBuf::from(std::env::var_os(key).expect(key));
        let xbox = input("OPENBF_MUSTAFAR_XBOX_ROOT");
        let ps2 = input("OPENBF_MUSTAFAR_PS2_ROOT");
        let out = input("OPENBF_MUSTAFAR_TEST_OUT");
        fs::create_dir(&out).unwrap();
        let mus = crate::join_ci(&xbox, "MUS/MUS1.LVL");
        let exact = crate::join_ci(&ps2, "MUS/MUS1.LVL");
        let run = |input: &std::path::Path, label, donor: Option<&std::path::Path>| {
            let dest = out.join(label);
            super::super::bake_map(
                input.to_str().unwrap(),
                dest.to_str().unwrap(),
                donor.map(|p| p.to_str().unwrap()),
            )
            .unwrap();
            dest.join("textures/mus1_prop_console_fx.ktx2")
        };

        let with_exact = fs::read(run(&mus, "exact", Some(&exact))).unwrap();
        let with_exact = ktx2::Reader::new(&with_exact).unwrap().header();
        assert_eq!((with_exact.pixel_width, with_exact.pixel_height), (64, 64));

        let changed = out.join("changed-source/_LVL_XBOX");
        for rel in ["core.lvl", "mission.lvl", "mus/mus1.lvl", "side/tur.lvl"] {
            let dst = changed.join(rel);
            fs::create_dir_all(dst.parent().unwrap()).unwrap();
            fs::copy(crate::join_ci(&xbox, rel), dst).unwrap();
        }
        let mut tan = fs::read(crate::join_ci(&xbox, "TAN/TAN1.LVL")).unwrap();
        let items = mungefs::content_chunks(mungefs::open(&tan).unwrap()).unwrap();
        let atlas = items
            .iter()
            .find(|c| {
                &c.tag == b"tex_" && c.child_string(b"NAME").as_deref() == Some("tan4_prop_console")
            })
            .unwrap();
        // Mutate a DXT1 endpoint used by the first texel. Chunk offsets are
        // relative to the inner UCFB payload.
        let endpoint = atlas.child(b"BODY").unwrap().offset + 16;
        tan[endpoint] ^= 0x1f;
        fs::create_dir(changed.join("tAn")).unwrap();
        let donor = changed.join("tAn/TaN1.LvL");
        let changed_mus = changed.join("mus/mus1.lvl");
        fs::write(&donor, tan).unwrap();
        assert!(!run(&changed_mus, "changed", None).exists());
        fs::rename(&donor, changed.join("tAn/mutated.lvl")).unwrap();
        assert!(!run(&changed_mus, "missing", None).exists());
        fs::create_dir(&donor).unwrap();
        assert!(!run(&changed_mus, "non-file", None).exists());
        fs::rename(&donor, changed.join("tAn/directory")).unwrap();
        fs::File::create(&donor)
            .unwrap()
            .set_len(TANTIVE_LVL_MAX_BYTES + 1)
            .unwrap();
        assert!(!run(&changed_mus, "oversize", None).exists());

        let baked = run(&mus, "recovered", None);
        assert!(baked.is_file(), "the Xbox console display is still missing");
        let baked = fs::read(baked).unwrap();
        let ktx = ktx2::Reader::new(&baked).unwrap();
        let header = ktx.header();
        assert_eq!((header.pixel_width, header.pixel_height), (192, 160));
        let rgba = ktx.levels().next().unwrap().data;
        assert_eq!(
            crate::gameplay::digest(rgba),
            "a9ce1118020e68aeb0f212bd1a7654100ab19cbe50f5726c0fce645e24ed2932"
        );
    }

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
