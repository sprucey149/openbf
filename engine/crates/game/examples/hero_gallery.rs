//! Offscreen BF2 hero body/cloth/texture sheets through the normal gallery.
//! Usage: hero_gallery <assets_baked> <output directory> [model substring]
//! These four ordinary animation-bank poses inspect assets, not saber combat
//! or matched PS2 camera parity. No golden files are read or changed.

use anyhow::{anyhow, Result};
use std::{collections::BTreeSet, path::Path};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if !(3..=4).contains(&args.len()) {
        return Err(anyhow!(
            "usage: hero_gallery <assets_baked> <output directory> [model substring]"
        ));
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out)?;
    game::assets::set_texture_tier(game::assets::TextureTier::Ps2);
    let db =
        game::roster::load_classdb(&root.join("classes"), "bf2", &game::roster::FACTION_ORDER)?;
    let models: BTreeSet<_> = db
        .names()
        .filter(|name| matches!(name.split('_').next(), Some("all" | "imp" | "rep" | "cis")))
        .filter(|name| name.split('_').nth(1) == Some("hero"))
        .filter_map(|name| db.prop(name, "GeometryName"))
        .filter(|model| args.get(3).is_none_or(|f| model.contains(f)))
        .map(str::to_owned)
        .collect();
    if models.is_empty() {
        return Err(anyhow!("no BF2 hero models selected"));
    }
    let mut written = BTreeSet::new();
    for (key, body, weapon, set) in game::gallery::discover_units(root)? {
        let Some(model) = body.file_stem().and_then(|n| n.to_str()) else {
            continue;
        };
        if !key.starts_with("bf2_") || !models.contains(model) {
            continue;
        }
        for (suffix, heading) in [
            ("", std::f32::consts::PI * 0.75),
            ("_front", std::f32::consts::PI * 0.25),
        ] {
            let (rgba, width, height) =
                game::gallery::unit_sheet_clothed(&body, weapon.as_deref(), &set, heading)?;
            let image = image::RgbaImage::from_raw(width, height, rgba)
                .ok_or_else(|| anyhow!("{model}: bad capture size"))?;
            let path = out.join(format!("{model}{suffix}.png"));
            image.save(&path)?;
            println!(
                "{}: {}x{} family={} bank={}",
                path.display(),
                width,
                height,
                set.family,
                set.bank
            );
        }
        written.insert(model.to_owned());
    }
    if written != models {
        return Err(anyhow!(
            "hero bodies without captures: {:?}",
            models.difference(&written).collect::<Vec<_>>()
        ));
    }
    println!("{} distinct BF2 hero body sheets rendered", written.len());
    Ok(())
}
