//! The Jawa rule (maintainer, 2026-11-20, `game::jawa`): every Jawa carries
//! the arc caster and the fusion cutter on both games' maps; Hunt keeps the
//! arc caster at full strength. Needs the shared bake and the generated
//! missions; skips without them.

use game::jawa::{ARC_CASTER, NATIVE_ARC_CASTER_SCALE};
use game::mission::MissionDef;
use game::roster::{ClassRoster, FactionRoster};
use std::path::{Path, PathBuf};

const CUTTER: &str = "tat_weap_inf_jawa_fusioncutter";

fn engine_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn classes() -> Option<PathBuf> {
    let dir = game::assets::baked_root().join("classes");
    if !dir.join("bf1_des.ron").is_file() || !dir.join("bf2_des.ron").is_file() {
        eprintln!("SKIP: both games' desert sides must be baked");
        return None;
    }
    Some(dir)
}

fn native_jawa(classes: &Path, game: &str) -> ClassRoster {
    let (_, file) = game::native_teams::native_side(classes, game, game::jawa::CLASS).unwrap();
    let mut db = game::roster::load_classdb(classes, game, &[]).unwrap();
    db.load(&file).unwrap();
    ClassRoster::build(&db, game::jawa::CLASS, 1, None).unwrap()
}

fn weapons(class: &ClassRoster) -> Vec<&core::weapon::WeaponClass> {
    std::iter::once(&class.weapon)
        .chain(class.held.iter().map(|h| &h.weapon))
        .collect()
}

fn arc(class: &ClassRoster) -> &core::weapon::WeaponClass {
    weapons(class)
        .into_iter()
        .find(|w| w.name == ARC_CASTER)
        .unwrap_or_else(|| panic!("{}: no arc caster", class.class))
}

#[test]
fn every_jawa_carries_the_bf2_arc_caster_and_its_fusion_cutter() {
    let Some(classes) = classes() else { return };
    let donor = game::jawa::donor(&classes).unwrap();
    let mut armed = Vec::new();
    for game in ["bf1", "bf2"] {
        let mut jawa = native_jawa(&classes, game);
        assert_eq!(
            game::jawa::needs_arc_caster(&jawa),
            game == "bf1",
            "{game}: BF1's Jawa authors only the repair tool"
        );
        game::jawa::arm(&donor, &mut jawa).unwrap();
        assert!(!game::jawa::needs_arc_caster(&jawa), "{game}");
        assert_eq!(
            jawa.weapon.name, ARC_CASTER,
            "{game}: the arc caster is the primary"
        );
        assert!(
            weapons(&jawa).iter().any(|w| w.name == CUTTER),
            "{game}: the fusion cutter stays"
        );
        assert_eq!(weapons(&jawa).len(), 2, "{game}");
        let before = jawa.clone();
        game::jawa::arm(&donor, &mut jawa).unwrap();
        assert_eq!(
            weapons(&jawa).len(),
            weapons(&before).len(),
            "{game}: idempotent"
        );
        armed.push(jawa);
    }
    assert_eq!(
        arc(&armed[0]).ordnance,
        arc(&armed[1]).ordnance,
        "one arc caster on both games' maps"
    );
    assert_eq!(arc(&armed[0]).emitter, arc(&armed[1]).emitter);
}

#[test]
fn outside_hunt_the_arc_caster_is_scaled_and_the_cutter_untouched() {
    let Some(classes) = classes() else { return };
    let donor = game::jawa::donor(&classes).unwrap();
    let mut jawa = native_jawa(&classes, "bf2");
    game::jawa::arm(&donor, &mut jawa).unwrap();
    let full = arc(&jawa).clone();
    let cutter = weapons(&jawa)
        .into_iter()
        .find(|w| w.name == CUTTER)
        .unwrap()
        .clone();
    game::jawa::tame(&mut jawa);
    let tamed = arc(&jawa);
    assert!(full.ordnance.max_damage > 0.0);
    assert_eq!(
        tamed.ordnance.max_damage,
        full.ordnance.max_damage * NATIVE_ARC_CASTER_SCALE
    );
    let (fe, te) = (
        full.emitter.as_ref().unwrap(),
        tamed.emitter.as_ref().unwrap(),
    );
    assert_eq!(
        te.damage_threshold,
        fe.damage_threshold * NATIVE_ARC_CASTER_SCALE
    );
    assert_eq!(
        te.jump_deduction,
        fe.jump_deduction * NATIVE_ARC_CASTER_SCALE
    );
    assert_eq!(te.cone_length, fe.cone_length);
    assert_eq!(
        tamed.shot_delay, full.shot_delay,
        "the rate of fire is kept"
    );
    let after = weapons(&jawa)
        .into_iter()
        .find(|w| w.name == CUTTER)
        .unwrap();
    assert_eq!(after, &cutter);
}

#[test]
fn hunt_jawas_carry_the_arc_caster_at_full_strength_on_both_games_maps() {
    let Some(classes) = classes() else { return };
    let donor = game::jawa::donor(&classes).unwrap();
    let full = game::roster::weapon_from_db(&donor, ARC_CASTER).unwrap();
    for key in ["bf1_tat2_hunt", "tat2_hunt"] {
        let path = engine_dir().join(format!("missions/{key}.ron"));
        if !path.is_file() {
            eprintln!("SKIP: {key} is not generated");
            continue;
        }
        let m = MissionDef::load(&path).unwrap();
        let factions = [m.teams[0].faction.as_str(), m.teams[1].faction.as_str()];
        let (rosters, _) =
            FactionRoster::pair(&classes, &m.source_game, factions, Some(&m)).unwrap();
        let team = rosters.iter().find(|r| r.faction == "jawa").expect(key);
        for class in &team.classes {
            assert_eq!(class.weapon.name, ARC_CASTER, "{key}");
            assert_eq!(arc(class).ordnance, full.ordnance, "{key}: full strength");
            assert!(weapons(class).iter().any(|w| w.name == CUTTER), "{key}");
        }
    }
}
