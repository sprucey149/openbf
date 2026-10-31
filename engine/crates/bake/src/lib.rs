//! Bake library: PS2 LVL → open formats (glTF/KTX2/RON). The `bake` binary is
//! a thin CLI over this.

pub mod adpcm;
pub mod alpha;
pub mod anim_sets;
pub mod attached_effects;
pub mod beep;
pub mod bind_pose;
pub mod classes;
pub mod cloth;
pub mod combo;
pub mod donor;
pub mod door;
pub mod droid_chunks;
pub mod droideka;
pub mod effects;
pub mod entity_light;
pub mod far_lowd;
pub mod foley;
pub mod foliage;
pub mod glb;
pub mod global_sound;
pub mod gltf_bin;
pub mod hardpoints;
pub mod heroes;
pub mod hud;
pub mod ktx2w;
pub mod leafpatch;
pub mod lighting;
pub mod loadscreen;
pub mod lowres_glb;
pub mod map;
pub mod ogg;
pub mod shell;
pub mod side_classes;
pub mod skin_glb;
pub mod soldier;
pub mod sound_names;
pub mod sounds;
pub mod spec;
pub mod terrain_glb;
pub mod tier;
pub mod turret_rig;
pub mod vehicle;
pub mod walker;
pub mod water_glb;

pub mod detail;

/// Write a RON artifact the way every bake output is written: `to_string_pretty`
/// with `PrettyConfig::default()`. The seven call sites had this open-coded, and
/// the config is what makes the file byte-reproducible — one place to change it,
/// and one place a reader has to look to know what the shipped RON's shape is.
pub fn write_ron<T: serde::Serialize>(path: &std::path::Path, value: &T) -> anyhow::Result<()> {
    use anyhow::Context;
    let text = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())?;
    // A fresh tree has no output directories yet (card PACK: `bake classes` into an empty assets_baked/).
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
    }
    std::fs::write(path, text).with_context(|| path.display().to_string())?;
    Ok(())
}

/// Remove one owned optional output; absence is already the desired state.
pub(crate) fn remove_optional_file(path: &std::path::Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn find_entry_ci(dir: &std::path::Path, name: &str) -> Option<std::ffi::OsString> {
    std::fs::read_dir(dir).ok()?.find_map(|e| {
        let file_name = e.ok()?.file_name();
        file_name
            .to_str()
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
            .then_some(file_name)
    })
}

pub fn join_ci(base: &std::path::Path, rel: &str) -> std::path::PathBuf {
    let direct = base.join(rel);
    if direct.exists() {
        return direct;
    }
    let mut cur = base.to_path_buf();
    for part in rel.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            cur.pop();
            continue;
        }
        let next = cur.join(part);
        cur = if next.exists() {
            next
        } else {
            find_entry_ci(&cur, part).map_or(next, |n| cur.join(n))
        };
    }
    cur
}

pub fn resolve_ci(path: &str) -> String {
    if path == "-" || std::path::Path::new(path).exists() {
        return path.to_owned();
    }
    let p = std::path::Path::new(path);
    let mut cur = std::path::PathBuf::new();
    for comp in p.components() {
        use std::path::Component;
        match comp {
            Component::Prefix(pre) => cur.push(pre.as_os_str()),
            Component::RootDir => cur.push(comp.as_os_str()),
            Component::CurDir => {
                if cur.as_os_str().is_empty() {
                    cur.push(".");
                }
            }
            Component::ParentDir => cur.push(".."),
            Component::Normal(part) => {
                let base = if cur.as_os_str().is_empty() {
                    std::path::Path::new(".")
                } else {
                    cur.as_path()
                };
                let next = cur.join(part);
                cur = if next.exists() {
                    next
                } else if let Some(s) = part.to_str() {
                    find_entry_ci(base, s).map_or(next, |name| {
                        if cur.as_os_str().is_empty() {
                            std::path::PathBuf::from(name)
                        } else {
                            cur.join(name)
                        }
                    })
                } else {
                    next
                };
            }
        }
    }
    cur.to_string_lossy().into_owned()
}

/// The bake this working tree ships (`engine/assets_baked`), for the tests in
/// this crate that read it. TEST-ONLY and crate-private on purpose:
/// `game::assets::baked_root` is the same path by the same rule, and `bake` is
/// only a DEV-dependency of `game` (game/Cargo.toml:7-8), so the two cannot
/// share one definition without making `game`'s shipped binary link `bake`.
/// FOR THE LEAD: that dependency edge is the reason for the duplication.
#[cfg(test)]
pub(crate) const BAKED_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets_baked");
