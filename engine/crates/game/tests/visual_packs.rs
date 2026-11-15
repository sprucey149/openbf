//! Native import check for explicitly supplied local presentation candidates. No GPU or game window.
use std::path::PathBuf;

#[test]
#[ignore = "set OPENBF_VISUAL_PAYLOADS to a JSON list of local GLB/KTX2 paths"]
fn visual_pack_owned_payloads_import() {
    let paths: Vec<PathBuf> = serde_json::from_str(
        &std::env::var("OPENBF_VISUAL_PAYLOADS").expect("explicit local payload list"),
    )
    .unwrap();
    assert!(!paths.is_empty());
    for path in paths {
        match path.extension().and_then(|v| v.to_str()) {
            Some("glb") => {
                let mesh = game::assets::load_glb(&path).unwrap();
                assert!(!mesh.positions.is_empty());
                assert!(!mesh.indices.is_empty());
                assert!(mesh.sphere.is_some(), "retain baked sphere metadata");
                assert!(!mesh.contact_surface);
            }
            Some("ktx2") => {
                let raw = std::fs::read(path).unwrap();
                let texture = game::assets::load_ktx2(&raw).unwrap();
                assert_eq!(
                    texture.rgba.len(),
                    texture.width as usize * texture.height as usize * 4
                );
            }
            _ => panic!("unsupported candidate"),
        }
    }
}

#[test]
#[ignore = "set OPENBF_VISUAL_NATIVE_CASE to an owned assets/missions/mapping/out JSON object"]
fn visual_pack_owned_archive_roundtrip() {
    let case: serde_json::Value = serde_json::from_str(
        &std::env::var("OPENBF_VISUAL_NATIVE_CASE").expect("explicit owned fixture"),
    )
    .unwrap();
    let path = |key| PathBuf::from(case[key].as_str().unwrap());
    let assets = path("assets");
    let missions = path("missions");
    let mapping = path("mapping");
    let out = path("out");
    let replacements: Vec<(String, PathBuf)> =
        serde_json::from_slice(&std::fs::read(&mapping).unwrap()).unwrap();
    assert!(!replacements.is_empty());
    let mut stock = pak::collect(&assets, "assets_baked").unwrap();
    stock.extend(pak::collect(&missions, "missions").unwrap());
    let write = |with_mapping| {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_game"));
        command.arg("--write-pack").arg(&out);
        if with_mapping {
            command.arg("--replacements").arg(&mapping);
        }
        let result = command
            .arg(format!("assets_baked={}", assets.display()))
            .arg(format!("missions={}", missions.display()))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    for installed in [false, true, true, false] {
        write(installed);
        let archive = pak::Archive::open(&out).unwrap().unwrap();
        assert_eq!(archive.len(), stock.len());
        for (name, source) in &stock {
            let expected = if installed {
                replacements
                    .iter()
                    .find(|(target, _)| target == name)
                    .map_or(source, |(_, file)| file)
            } else {
                source
            };
            assert_eq!(
                archive.read_entry(name).unwrap(),
                std::fs::read(expected).unwrap(),
                "{name}"
            );
        }
    }
}
