//! The original Xbox pad's button icons, from the player's own Xbox disc: the
//! `icon_duke_*` sheets `ui::input_icons` names, written beside the HUD's
//! other sheets. `game::input_icons` draws its own for any it does not find,
//! so a build without an Xbox disc still has the set. Nothing from a disc is
//! in the repo.
//!
//! The disc names its pictures for the PS2's buttons, read from the decoded
//! pictures of BF2's Xbox `common.lvl`: `btna` A, `btnb` B, `btnmisc` X,
//! `btnmisc2` Y, `btnl2` White, `btnr2` Black, the L and R plates under
//! `btn_l1` / `btn_r1` and each language's own copies of those.

use anyhow::{anyhow, bail, Context, Result};
use std::path::Path;

/// (sheet the game asks for, the disc's name for its picture).
pub const ICONS: [(&str, &str); 18] = [
    ("icon_duke_a", "btna"),
    ("icon_duke_b", "btnb"),
    ("icon_duke_x", "btnmisc"),
    ("icon_duke_y", "btnmisc2"),
    ("icon_duke_white", "btnl2"),
    ("icon_duke_black", "btnr2"),
    ("icon_duke_l", "_localize_english_btn_l1"),
    ("icon_duke_r", "_localize_english_btn_r1"),
    ("icon_duke_l3", "_localize_english_btn_analog_l_down"),
    ("icon_duke_r3", "_localize_english_btn_analog_r_down"),
    ("icon_duke_back", "btn_select"),
    ("icon_duke_start", "btn_start"),
    ("icon_duke_dpad", "btn_directional_pad"),
    ("icon_duke_dpad_lr", "btn_directional_pad_lr"),
    ("icon_duke_dpad_up", "btn_directional_pad_u"),
    ("icon_duke_dpad_down", "btn_directional_pad_d"),
    ("icon_duke_dpad_left", "btn_directional_pad_l"),
    ("icon_duke_dpad_right", "btn_directional_pad_r"),
];

/// Bake the icons of the Xbox `common.lvl` at `lvl_path` into
/// `<outdir>/textures`; answers how many it wrote. A LVL that is not an Xbox
/// one is refused, by its content and not its name.
pub fn bake_xbox_icons(lvl_path: &str, outdir: &str) -> Result<usize> {
    let buf = std::fs::read(lvl_path).with_context(|| lvl_path.to_string())?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{lvl_path}: {e}"))?;
    let content = mungefs::content_chunks(inner).map_err(|e| anyhow!("{lvl_path}: {e}"))?;
    let dir = Path::new(outdir).join("textures");
    let mut written = 0;
    let mut other_console = false;
    for (sheet, source) in ICONS {
        let found = content.iter().filter(|c| &c.tag == b"tex_").find(|c| {
            c.child_str(b"NAME")
                .is_some_and(|n| n.eq_ignore_ascii_case(source))
        });
        let Some(tex) = found else {
            continue;
        };
        if !mungefs::texture_xbox::is_xbox(tex) {
            other_console = true;
            continue;
        }
        let t = mungefs::texture::Texture::parse(tex).map_err(|e| anyhow!("{lvl_path}: {e}"))?;
        std::fs::create_dir_all(&dir).with_context(|| dir.display().to_string())?;
        crate::detail::write_ktx2(&dir, &format!("{sheet}.ktx2"), &t)?;
        written += 1;
    }
    if written == 0 {
        if other_console {
            bail!("{lvl_path}: not an Xbox common.lvl");
        }
        bail!("{lvl_path}: no Xbox button icons");
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Data-gated: every icon comes off the Xbox disc, A is green and Y is
    /// yellow-ish, as a wrong mapping or a bad decode would not be.
    #[test]
    fn the_xbox_disc_supplies_every_icon() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted/");
        let lvl = format!("{root}xbox_bf2/common.lvl");
        if !Path::new(&lvl).exists() {
            eprintln!("SKIP: no Xbox common.lvl");
            return;
        }
        let out = std::env::temp_dir().join(format!("openbf-xbox-icons-{}", std::process::id()));
        let written = bake_xbox_icons(&lvl, out.to_str().unwrap()).unwrap();
        std::fs::remove_dir_all(&out).ok();
        assert_eq!(written, ICONS.len());

        let buf = std::fs::read(&lvl).unwrap();
        let inner = mungefs::open(&buf).unwrap();
        let content = mungefs::content_chunks(inner).unwrap();
        // The mean colour of a sheet's opaque texels.
        let mean = |name: &str| {
            let tex = content
                .iter()
                .find(|c| &c.tag == b"tex_" && c.child_str(b"NAME") == Some(name))
                .unwrap();
            let rgba = mungefs::texture::Texture::parse(tex).unwrap().rgba8(0);
            let (mut sum, mut n) = ([0u32; 3], 0u32);
            for p in rgba.chunks_exact(4).filter(|p| p[3] > 128) {
                for k in 0..3 {
                    sum[k] += u32::from(p[k]);
                }
                n += 1;
            }
            assert!(n > 100, "{name}: {n} opaque texels");
            sum.map(|s| s / n)
        };
        let [r, g, b] = mean("btna");
        assert!(g > r && g > b, "A is green: {r} {g} {b}");
        let [r, g, b] = mean("btnb");
        assert!(r > g && r > b, "B is red: {r} {g} {b}");
        let [r, g, b] = mean("btnmisc");
        assert!(b > r && b > g, "X is blue: {r} {g} {b}");
        let [r, g, b] = mean("btnmisc2");
        assert!(r > b && g > b, "Y is yellow: {r} {g} {b}");
    }

    /// A PS2 COMMON.LVL has the same names and is refused by its content.
    #[test]
    fn a_ps2_common_lvl_is_refused() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted/");
        let lvl = format!("{root}bf2/lvl/DATA/_LVL_PS2/COMMON.LVL");
        if !Path::new(&lvl).exists() {
            eprintln!("SKIP: no PS2 COMMON.LVL");
            return;
        }
        let out = std::env::temp_dir().join(format!("openbf-ps2-icons-{}", std::process::id()));
        let err = bake_xbox_icons(&lvl, out.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("not an Xbox"), "{err}");
        assert!(!out.exists(), "nothing written");
    }
}
