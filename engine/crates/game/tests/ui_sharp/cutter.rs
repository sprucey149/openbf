use super::*;
use game::hud_feed::HudView;
use sha2::{Digest, Sha256};
use std::path::Path;

const SIDES: [&str; 4] = ["rep", "cis", "all", "imp"];

fn classes(assets: &Path, game: &str) -> game::classdb::ClassDb {
    let mut files: Vec<_> = std::fs::read_dir(assets.join("classes"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&format!("{game}_"))
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "missing {game} classes");
    let mut db = game::classdb::ClassDb::default();
    for file in files {
        db.load(&file).unwrap();
    }
    db
}

fn weapon(db: &game::classdb::ClassDb, side: &str) -> ui::hud::HudWeapon {
    let class = format!("{side}_weap_inf_fusioncutter");
    assert_eq!(db.engine_base(&class), Some("repair"));
    game::hud_feed::hud_weapon(&game::roster::weapon_hud(db, &class), None, 1.0)
}

fn model(side: &str, cutter: ui::hud::HudWeapon) -> ui::hud::HudModel {
    ui::hud::HudModel {
        team_names: if matches!(side, "rep" | "cis") {
            ["rep".into(), "cis".into()]
        } else {
            ["all".into(), "imp".into()]
        },
        tickets: [150, 150],
        hp: 300.0,
        hp_max: 300.0,
        energy: 100.0,
        energy_max: 100.0,
        weapon: Some(cutter.clone()),
        weapon2: Some(cutter),
        ..Default::default()
    }
}

fn row_bounds(view: &HudView) -> [[f32; 4]; 2] {
    std::array::from_fn(|row| {
        let b = view
            .bounds(&format!("player1weapon{}mesh", row + 1))
            .expect("Cutter drawn");
        assert!(b.iter().all(|v| v.is_finite()) && b[0] < b[2] && b[1] < b[3]);
        b
    })
}

/// A required real-content witness: no missing-asset early returns. Run on
/// each owned profile; the parent selects BF1's icon in the second row.
#[test]
#[ignore = "requires UI_SHARP_ASSETS and UI_SHARP_OUT"]
fn cutter_selection_and_missing_bf2_fallback() {
    let (assets, out) = paths();
    let mut view = HudView::load(&assets, render::HEADLESS_FORMAT, None).unwrap();
    let mut receipts = Vec::new();
    for game in ["bf1", "bf2"] {
        let db = classes(&assets, game);
        for side in SIDES {
            let w = weapon(&db, side);
            assert_eq!(w.bf2, game == "bf2");
            if game == "bf1" {
                assert!(w.icon.is_some(), "BF1's fallback icon is present");
            }
            for row in [1, 2] {
                let (key, mesh) = view.weapon_mesh(row, &w);
                println!(
                    "{game} {side} row{row}: key={key}, model={mesh:?}, icon={:?}",
                    w.icon
                );
                assert_eq!(
                    mesh.as_deref(),
                    Some(w.class.as_str()),
                    "{game} {side} row{row}"
                );
            }
            let file = assets.join(format!("side_bf2/weapons/{}.glb", w.class));
            let geometry = game::assets::load_glb(&file).unwrap();
            assert!(!geometry.positions.is_empty() && !geometry.indices.is_empty());
            view.update(&model(side, w.clone()), 0.0, [1920, 1080]);
            let bounds = row_bounds(&view);
            assert!(bounds[1][3] < bounds[0][1], "rows are separate: {bounds:?}");
            if let Some(icon) = &w.icon {
                assert!(!view.drawn_sheets().any(|sheet| sheet == icon));
            }
            receipts.push(serde_json::json!({
                "game": game, "side": side, "key": w.key, "icon": w.icon,
                "bf2_model": file, "sha256": format!("{:x}", Sha256::digest(std::fs::read(&file).unwrap())),
                "vertices": geometry.positions.len(), "indices": geometry.indices.len(), "bounds": bounds,
            }));
        }
    }
    // A small owned copy retains BF1's same-name geometry and icon but has
    // no BF2 weapon models. Never remove anything from the source bake.
    let fallback = out.join("missing-bf2-cutter");
    for folder in ["common_bf2/hud", "common_bf2/fonts", "common_bf2/models"] {
        let target = fallback.join(folder);
        std::fs::create_dir_all(&target).unwrap();
        for entry in std::fs::read_dir(assets.join(folder)).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                std::fs::copy(&path, target.join(path.file_name().unwrap())).unwrap();
            }
        }
    }
    let w = weapon(&classes(&assets, "bf1"), "rep");
    let icon = format!("{}.ktx2", w.icon.as_ref().unwrap());
    let icon = [
        "common_bf2/textures",
        "side_bf2/textures",
        "common_bf1/textures",
        "side_bf1/textures",
    ]
    .into_iter()
    .map(|folder| format!("{folder}/{icon}"))
    .find(|relative| assets.join(relative).is_file())
    .expect("the source bake has BF1's Cutter icon");
    for relative in [format!("side_bf1/weapons/{}.glb", w.class), icon] {
        let target = fallback.join(&relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(assets.join(&relative), target).unwrap();
    }
    let mut old = HudView::load(&fallback, render::HEADLESS_FORMAT, None).unwrap();
    assert_eq!(old.weapon_mesh(1, &w).1.as_deref(), Some(w.class.as_str()));
    assert_eq!(
        old.weapon_mesh(2, &w).1,
        None,
        "keep the older BF1 icon fallback"
    );
    old.update(&model("rep", w.clone()), 0.0, [1920, 1080]);
    assert!(old
        .drawn_sheets()
        .any(|sheet| Some(sheet) == w.icon.as_deref()));
    row_bounds(&old);
    receipts.push(serde_json::json!({"fallback": "missing BF2 model", "icon": w.icon}));
    std::fs::write(
        out.join("cutter-selection.json"),
        serde_json::to_vec_pretty(&receipts).unwrap(),
    )
    .unwrap();
}

/// Both weapon rows, four stock factions, Clone Wars and Galactic Civil War,
/// and both games' weapon HUD feeds. BF1 must draw the same BF2 geometry.
#[test]
#[ignore = "requires GPU and UI_SHARP_ASSETS owned source bake"]
fn capture_cutter_rows() {
    let (assets, out) = paths();
    let bf1 = classes(&assets, "bf1");
    let bf2 = classes(&assets, "bf2");
    let mut mismatches = Vec::new();
    for size in [[640, 480], [1920, 1080]] {
        let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
        for side in SIDES {
            let mut previous = None;
            for (game, db) in [("bf1", &bf1), ("bf2", &bf2)] {
                let mut hud = HudView::for_renderer(&assets, &r).unwrap();
                let m = model(side, weapon(db, side));
                hud.update(&m, 0.0, size);
                hud.update(&m, 0.6, size);
                row_bounds(&hud);
                r.render_with_overlay(
                    &render::Camera::default(),
                    &[],
                    [0.06, 0.06, 0.06, 1.0],
                    Some(&mut |dev, q, enc, target, size| hud.paint(dev, q, enc, target, size)),
                )
                .unwrap();
                assert_all_sheets_bound(&hud);
                let rgba = r.capture_rgba().unwrap().0;
                if let Some(previous) = &previous {
                    if &rgba != previous {
                        mismatches.push(format!("{side} {size:?}: BF1 and BF2 Cutter art differs"));
                    }
                }
                save(&out, &format!("cutter_{game}_{side}"), &r);
                previous = Some(rgba);
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
