//! UI sheets from the player's input, with larger registered donor art when
//! present and edge-preserving reconstruction of small sliced sheets.
mod filter;
use anyhow::{Context, Result};
use manifest::hud::{HudIndex, HudLayout};
use mungefs::texture::Texture;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use crate::tier::{self, Resolved, Usage};

const FILES: [&str; 3] = ["INGAME.LVL", "COMMON.LVL", "CORE.LVL"];
const HUD_PIECES: [&str; 7] = [
    "border_3_pieces",
    "optionl",
    "optionr",
    "hud_jetpack_and_energybar_blankfill",
    "hud_clipbar_segment",
    "hud_secondary_refire_segment",
    "hud_reinforcement_icon",
];

fn reconstruct(texture: &Texture) -> Option<Resolved> {
    let name = texture.name.as_str();
    if !super::SCREEN_SHEETS.contains(&name) && !HUD_PIECES.contains(&name) {
        return None;
    }
    let (w, h) = (texture.info.width, texture.info.height);
    if !(2..=256).contains(&w) || !(2..=256).contains(&h) || texture.info.detail_compressed {
        return None;
    }
    let base = texture.rgba8(0);
    if base.len() != (w * h * 4) as usize || base.chunks_exact(4).all(|p| p == &base[..4]) {
        return None;
    }
    // Four samples per texel cover the ordinary 4K magnification of the
    // console layout. Keep the original mips for smaller on-screen sizes.
    let mut result = Resolved::from_ps2(texture);
    let mut mips = [4, 2]
        .map(|scale| filter::upsample(&base, w as usize, h as usize, scale))
        .to_vec();
    mips.append(&mut result.mips);
    result.mips = mips;
    result.width *= 4;
    result.height *= 4;
    Some(result)
}

/// Sliced panels use the base sheet's normalized UVs. Check both the aspect
/// ratio and the opacity image, since the ordinary colour gate assumes the
/// draw will carry the original mask over the donor colour.
fn registered(base: &Texture, alpha: &[u8], donor: &Resolved) -> bool {
    let (w, h) = (base.info.width, base.info.height);
    if u64::from(w) * u64::from(donor.height) != u64::from(h) * u64::from(donor.width) {
        return false;
    }
    let coverage = alpha_image(&donor.mips[0]);
    // A sparse icon can disappear without moving image-wide averages.
    // Require the same nonzero and half-coverage support on the base grid.
    let down = tier::box_down(&coverage, donor.width, donor.height, w, h);
    if alpha
        .chunks_exact(4)
        .zip(down.chunks_exact(4))
        .any(|(a, b)| (a[0] == 0, a[0] >= 128) != (b[0] == 0, b[0] >= 128))
    {
        return false;
    }
    let strict = Usage {
        sky: true,
        alpha_carried: false,
    };
    tier::same_art(
        &tier::compare(alpha, w, h, &coverage, donor.width, donor.height, strict),
        strict,
    )
}

fn alpha_image(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|p| [p[3], p[3], p[3], 255])
        .collect()
}

pub(super) fn bake(core: &Path, out: &Path, donors: &[PathBuf]) -> Result<usize> {
    let hud_dir = out.join("hud");
    let selection = hud_dir.join("native-art.ron");
    crate::remove_optional_file(&selection)?;
    let index: HudIndex = ron::from_str(&std::fs::read_to_string(hud_dir.join("index.ron"))?)?;
    let mut wanted = BTreeSet::from_iter(super::SCREEN_SHEETS.map(str::to_owned));
    for name in index.layouts {
        let layout: HudLayout = ron::from_str(&std::fs::read_to_string(
            hud_dir.join(format!("{name}.ron")),
        )?)?;
        super::strings(&layout.nodes, &mut wanted);
    }

    let mut donor_files = Vec::new();
    for core in donors {
        let dir = core.parent().context("donor UI core has no parent")?;
        donor_files.extend(
            FILES
                .map(|name| crate::join_ci(dir, name))
                .into_iter()
                .filter(|file| file.is_file()),
        );
    }
    let donors = (!donor_files.is_empty())
        .then(|| {
            tier::PcTextures::load(
                &donor_files
                    .iter()
                    .map(|p| p.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(","),
            )
        })
        .transpose()?;

    let mut fingerprints = Vec::new();
    for file in &donor_files {
        fingerprints.push(format!("{:x}", Sha256::digest(std::fs::read(file)?)));
    }
    let dir = core.parent().context("UI core has no parent")?;
    let mut base = BTreeMap::new();
    for src in FILES {
        let file = crate::join_ci(dir, src);
        let raw = std::fs::read(&file).with_context(|| file.display().to_string())?;
        fingerprints.push(format!("{:x}", Sha256::digest(&raw)));
        for chunk in mungefs::content_chunks(mungefs::open(&raw)?)? {
            if &chunk.tag != b"tex_" {
                continue;
            }
            let Some(name) = chunk.child_str(b"NAME").map(str::to_ascii_lowercase) else {
                continue;
            };
            if !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
                || !wanted.remove(&name)
                || !out.join("textures").join(format!("{name}.ktx2")).is_file()
            {
                continue;
            }
            base.insert(name, Texture::parse(&chunk)?);
        }
    }

    let dest = hud_dir.join("native-art");
    std::fs::create_dir_all(&dest)?;
    let mut names = Vec::new();
    let mut choices = Vec::new();
    for (name, texture) in base {
        if texture.mips.is_empty() || texture.info.width == 0 || texture.info.height == 0 {
            continue;
        }
        let base = texture.rgba8(0);
        let pixels = u64::from(texture.info.width) * u64::from(texture.info.height);
        if !base.len().is_multiple_of(4) || (base.len() / 4) as u64 != pixels {
            continue;
        }
        let alpha = alpha_image(&base);
        let usage = Usage {
            sky: true,
            alpha_carried: true,
        };
        let donor = tier::tier2_matching(&texture, donors.as_ref(), usage, |r| {
            registered(&texture, &alpha, r)
        });
        let (image, source) = match donor {
            Some(best) => {
                let source = best.tier.as_str();
                (best, source)
            }
            None => {
                let Some(image) = reconstruct(&texture) else {
                    continue;
                };
                (image, "reconstructed")
            }
        };
        image.write_ktx2(&dest, &format!("{name}.ktx2"))?;
        choices.push(serde_json::json!({
            "name": name,
            "base": [texture.info.width, texture.info.height],
            "chosen": [image.width, image.height],
            "source": source,
        }));
        names.push(name);
    }
    std::fs::write(
        hud_dir.join("native-art.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "source_sha256": fingerprints,
            "sheets": choices,
        }))?,
    )?;
    crate::write_ron(&selection, &names)?;
    Ok(names.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_art_requires_registered_opacity_and_aspect() {
        let make = |width, height, clear| Resolved {
            width,
            height,
            mips: vec![[255, 255, 255, clear].repeat((width * height) as usize)],
            tier: tier::Tier::Pc,
        };
        let base = make(16, 16, 0).as_texture("cursor");
        let alpha = alpha_image(&base.rgba8(0));
        assert!(registered(&base, &alpha, &make(32, 32, 0)));
        assert!(!registered(&base, &alpha, &make(32, 64, 0)));
        assert!(!registered(&base, &alpha, &make(32, 32, 255)));

        for opacity in [255, 127] {
            let mut sparse = make(64, 64, 0);
            for i in [0, 1, 64, 65] {
                sparse.mips[0][4 * i + 3] = opacity;
            }
            let base = sparse.as_texture("marker");
            let alpha = alpha_image(&base.rgba8(0));
            assert!(!registered(&base, &alpha, &make(128, 128, 0)), "{opacity}");
        }
    }
}
