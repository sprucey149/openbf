#[path = "../game/build_icon.rs"]
mod build_icon;

use std::path::PathBuf;

fn main() {
    build_icon::link_icon("openbf-launcher");

    println!("cargo:rerun-if-env-changed=OPENBF_EMBED_FFMPEG");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let generated = out_dir.join("embedded_ffmpeg.rs");
    let body = match std::env::var_os("OPENBF_EMBED_FFMPEG")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        Some(path) => {
            let canon = std::fs::canonicalize(&path).unwrap_or_else(|e| {
                panic!("OPENBF_EMBED_FFMPEG={}: {e}", path.display());
            });
            println!("cargo:rerun-if-changed={}", canon.display());
            let lit = canon.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
            format!("pub const EMBEDDED_FFMPEG: Option<&[u8]> = Some(include_bytes!(\"{lit}\"));\n")
        }
        None => "pub const EMBEDDED_FFMPEG: Option<&[u8]> = None;\n".to_owned(),
    };
    std::fs::write(&generated, body).expect("write embedded_ffmpeg.rs");
}
