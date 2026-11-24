//! The Windows application icon. `scripts/pack.py` renders the icon from the
//! player's own bake (the clone trooper's helmet, `docs/branding.md`), writes
//! `engine/branding_baked/windows/openbf.ico` with `scripts/make_icons.py`, and
//! names that file in `OPENBF_ICON` when it runs cargo. `rc.exe` compiles it into
//! `openbf.res` under `OUT_DIR`, and the `cargo:rustc-link-arg-bin` here hands that
//! resource to `link.exe`, so `game.exe` carries the icon in its resource directory
//! and Explorer, a shortcut and the taskbar show it instead of the generic one. The
//! icon is data the OS reads, not the game's: nothing changes at run time.
//!
//! No icon is committed. A build without `OPENBF_ICON`, or with it naming a file
//! that is not there yet (pack.py's first build, before the bake), embeds nothing
//! and prints nothing: a developer's plain `cargo build` stays quiet. Only when an
//! icon was asked for does a missing resource compiler give a `cargo:warning=`.
//! For a `windows` + `gnu` (MinGW) target, `windres` plays `rc.exe`'s part, found
//! through `WINDRES` and then `PATH`.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const BIN: &str = "game";

/// One icon group. The Win32 path APIs accept `/`, so the same text is read by both
/// `rc.exe` and `windres`; the `.rc` lives beside the `.res` in `OUT_DIR`.
fn rc_text(ico: &Path) -> String {
    let forward = ico.display().to_string().replace('\\', "/");
    format!("1 ICON \"{forward}\"\n")
}

/// A version directory name like `10.0.26100.0`, as four numbers.
fn version(name: &str) -> Option<[u32; 4]> {
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let numbers: Option<Vec<u32>> = parts.iter().map(|p| p.parse().ok()).collect();
    numbers?.try_into().ok()
}

/// The SDK version folders under a SDK `bin/`, newest first.
fn newest_versions(bin: &Path) -> Vec<String> {
    let mut found: Vec<([u32; 4], String)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(bin) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(v) = version(&name) {
                found.push((v, name));
            }
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    found.into_iter().map(|(_, name)| name).collect()
}

/// `rc.exe`, as the Windows SDK ships it: `WindowsSdkDir`, the known SDK folders,
/// then whatever `PATH` gives a shell that has already set the SDK up.
fn rc_tool(arch_dir: &str) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = std::env::vars_os()
        .filter(|(key, _)| {
            let key = key.to_string_lossy().to_lowercase();
            key == "windowssdkdir" || key == "programfiles(x86)" || key == "programfiles"
        })
        .map(|(_, value)| PathBuf::from(value))
        .collect();
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10"));
    roots.push(PathBuf::from(r"C:\Program Files\Windows Kits\10"));
    for root in roots {
        let bin = root.join("bin");
        if bin.is_dir() {
            for v in newest_versions(&bin) {
                for dir in [arch_dir, "x64"] {
                    let tool = bin.join(&v).join(dir).join("rc.exe");
                    if tool.is_file() {
                        return Some(tool);
                    }
                }
            }
        }
        // SDKs before 10 keep the tools in `<root>/x64`.
        let legacy = root.join("x64").join("rc.exe");
        if legacy.is_file() {
            return Some(legacy);
        }
    }
    if Command::new("rc.exe")
        .args(["/nologo", "/?"])
        .output()
        .is_ok()
    {
        return Some(PathBuf::from("rc.exe"));
    }
    None
}

/// `rc /nologo /fo <res> <rc>`, and report whether it worked.
fn rc_compile(tool: &Path, rc_src: &Path, res: &Path) -> bool {
    Command::new(tool)
        .arg("/nologo")
        .arg("/fo")
        .arg(res)
        .arg(rc_src)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// `windres -i <rc> -o <res>`, and report whether it worked.
fn windres_compile(tool: &PathBuf, rc_src: &Path, res: &Path) -> bool {
    let args = [
        "-i".to_string(),
        rc_src.to_string_lossy().into_owned(),
        "-o".to_string(),
        res.to_string_lossy().into_owned(),
    ];
    Command::new(tool)
        .args(&args)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Write the `.rc`, compile it, and ask the linker to add the `.res` to the game.
fn emit(ico: &Path, out: &Path) {
    let rc_src = out.join("openbf.rc");
    let res = out.join("openbf.res");
    if std::fs::write(&rc_src, rc_text(ico)).is_err() {
        println!(
            "cargo:warning=branding: the icon source cannot be written to {}",
            rc_src.display()
        );
        return;
    }
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let arch_dir = match std::env::var("CARGO_CFG_TARGET_ARCH")
        .unwrap_or_default()
        .as_str()
    {
        "aarch64" => "arm64",
        "x86" => "x86",
        _ => "x64",
    };
    let tool = match env.as_str() {
        "msvc" => rc_tool(arch_dir),
        "gnu" => Some(PathBuf::from(
            std::env::var("WINDRES").unwrap_or_else(|_| "windres".into()),
        )),
        _ => None,
    };
    let Some(tool) = tool else {
        println!(
            "cargo:warning=branding: no resource compiler for a {env} windows target, \
                  so the game keeps the generic icon"
        );
        return;
    };
    let ok = if env == "msvc" {
        rc_compile(&tool, &rc_src, &res)
    } else {
        windres_compile(&tool, &rc_src, &res)
    };
    if ok {
        // No quotes here: `link.exe` reads a path with them as `\path\` and asks for
        // `<path>.obj`. Cargo's other link arg in this file, `/STACK:8388608`, is plain too.
        println!("cargo:rustc-link-arg-bin={BIN}={}", res.display());
    } else {
        println!(
            "cargo:warning=branding: {} failed, so the game keeps the generic icon",
            tool.display()
        );
    }
}

/// The icon to embed: the target is Windows and `OPENBF_ICON` names a file that
/// is there. Anything else embeds nothing, silently.
fn icon_to_embed(target_os: Option<&str>, icon: Option<PathBuf>) -> Option<PathBuf> {
    if target_os != Some("windows") {
        return None;
    }
    icon.filter(|p| p.is_file())
}

pub fn link_icon() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").ok();
    if target_os.as_deref() != Some("windows") {
        return;
    }
    println!("cargo:rerun-if-env-changed=OPENBF_ICON");
    let named = std::env::var_os("OPENBF_ICON")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    if let Some(ico) = &named {
        // Watched even before it exists: pack.py builds, renders the icon, then
        // builds again, and this is what makes that second build embed it.
        println!("cargo:rerun-if-changed={}", ico.display());
    }
    let Some(ico) = icon_to_embed(target_os.as_deref(), named) else {
        return;
    };
    match std::env::var("OUT_DIR") {
        Ok(out) => emit(&ico, Path::new(&out)),
        Err(_) => {
            println!("cargo:warning=branding: OUT_DIR is not set, so the icon is not embedded")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_icon_embeds_only_a_named_icon_that_is_there_on_windows() {
        let dir = std::env::temp_dir().join(format!("openbf-build-icon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ico = dir.join("openbf.ico");
        std::fs::write(&ico, b"\0\0\x01\0").unwrap();
        let missing = dir.join("not-rendered.ico");
        assert_eq!(
            icon_to_embed(Some("windows"), Some(ico.clone())),
            Some(ico.clone())
        );
        assert_eq!(icon_to_embed(Some("windows"), Some(missing)), None);
        assert_eq!(icon_to_embed(Some("windows"), None), None);
        assert_eq!(icon_to_embed(Some("linux"), Some(ico.clone())), None);
        assert_eq!(icon_to_embed(Some("macos"), Some(ico)), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn build_icon_rc_names_the_icon_with_forward_slashes() {
        assert_eq!(
            rc_text(Path::new(
                r"C:\game\engine\branding_baked\windows\openbf.ico"
            )),
            "1 ICON \"C:/game/engine/branding_baked/windows/openbf.ico\"\n"
        );
    }
}
