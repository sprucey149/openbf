//! Offscreen inspection of the two selectively baked DLC hero bodies.
//! Usage: dlc_gallery <asset root> <output directory>
//! Sequential GPU creation; no window, pack mission, script or localization.

use anyhow::{anyhow, Context, Result};
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err(anyhow!(
            "usage: dlc_gallery <asset root> <output directory>"
        ));
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out)?;
    game::assets::set_texture_tier(game::assets::TextureTier::Ps2);
    let mut db = game::roster::load_classdb(&root.join("classes"), "bf2", &["rep", "cis"])?;
    db.load(&root.join("classes/bf2_dlc.ron"))?;
    let side = root.join("side_bf2");
    for class in ["dlc_hero_fisto", "dlc_hero_ventress"] {
        let geometry = db
            .prop(class, "GeometryName")
            .context("DLC hero geometry")?;
        let anim = game::roster::class_anim(&db, class, &db.weapon_slots(class));
        let (primary, _) = game::roster::primary_weapon(&db, class).context("DLC hero weapon")?;
        let set = game::gallery::UnitSet {
            bank: anim.bank_of(&primary).to_owned(),
            family: anim.family,
        };
        let body = side.join("soldiers").join(format!("{geometry}.glb"));
        let weapon = game::roster::weapon_geometry(&db, class)
            .map(|name| side.join("weapons").join(format!("{name}.glb")));
        let (rgba, width, height) = game::gallery::unit_sheet_clothed(
            &body,
            weapon.as_deref(),
            &set,
            std::f32::consts::FRAC_PI_4,
        )?;
        let image = image::RgbaImage::from_raw(width, height, rgba)
            .context("invalid DLC gallery dimensions")?;
        let path = out.join(format!("{class}_front.png"));
        image.save(&path)?;
        println!(
            "{}: {width}x{height}, family={} bank={}",
            path.display(),
            set.family,
            set.bank
        );
    }
    Ok(())
}
