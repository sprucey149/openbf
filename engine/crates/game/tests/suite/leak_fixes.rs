//! Issue #182, the leak closure on the real bake (data-gated: skipped when
//! the bake is absent): the solids `game::collision_fixes` adds are in the
//! world where their comments say, and they close the sites they name.
//!
//! - BF1 Rhen Var Citadel's ravine bridge: inside a pier's shaft (where a
//!   soldier sliding down the ravine hung for good) is now solid.
//! - Its leaning obelisk: inside a groove between two ribs (where one
//!   wedged) is now solid, and the open ground beside the obelisk is not.

use core::sim::{Embed, Sim};
use core::soldier::Stance;
use glam::Vec3;

fn rhn2() -> Option<Sim> {
    let root = game::assets::baked_root();
    let path = root.join("bf1_rhn2");
    if !path.join("manifest.ron").exists() {
        eprintln!("skipped: no bf1_rhn2 bake");
        return None;
    }
    let (scene, _) = game::assets::load_battle_scene(&path, "con").unwrap();
    let mut sim = Sim::empty(1);
    sim.heightfield = scene.heightfield();
    scene.install_collision(&mut sim);
    Some(sim)
}

fn solid(e: Embed) -> bool {
    matches!(e, Embed::InPrim(_) | Embed::InClosedMesh(_))
}

#[test]
fn the_rhen_var_citadel_pier_shaft_and_obelisk_groove_are_solid() {
    let Some(sim) = rhn2() else { return };
    // Where the census's soldiers hung in the ravine pier (feet on the
    // terrain inside the shaft): in the pier's box.
    for feet in [
        Vec3::new(-279.5, -14.43, -192.5),
        Vec3::new(-279.5, -12.82, -199.5),
    ] {
        let e = sim.embed_at(feet, Stance::Stand);
        assert!(solid(e), "{feet}: {e:?}");
    }
    // Where one wedged in the obelisk's groove: the oracle still names the
    // obelisk's own open shell first, but a closed hull of the obelisk's
    // instance (the fill) now stands round the point.
    let groove = Vec3::new(-309.75, -7.0, -138.12);
    let shell = match sim.embed_at(groove, Stance::Stand) {
        Embed::InOpenShell(b) => b,
        e => panic!("groove: {e:?}"),
    };
    let inst = sim.collision.bodies()[shell].instance;
    let hull = sim.collision.bodies().iter().any(|b| {
        b.instance == inst
            && b.closed
            && b.aabb.min.cmple(groove).all()
            && groove.cmple(b.aabb.max).all()
    });
    assert!(hull, "no closed fill round the groove");
    // Open snow beside the obelisk stays open.
    let open = Vec3::new(
        -300.0,
        sim.ground_under(Vec3::new(-300.0, 40.0, -138.0), false),
        -138.0,
    );
    assert!(!solid(sim.embed_at(open, Stance::Stand)), "{open}");
}
