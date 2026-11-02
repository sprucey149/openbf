use super::*;

#[test]
fn catalogue_and_live_switch_cover_both_discs_and_enemy_heroes() {
    let root = crate::assets::baked_root();
    let map = root.join("bf2_kam1");
    if !map.join("manifest.ron").is_file() {
        eprintln!("SKIP: local baked assets required");
        return;
    }
    let scene = crate::assets::Scene {
        dir: map.clone(),
        manifest: ron::from_str(&pak::read_to_string(map.join("manifest.ron")).unwrap()).unwrap(),
        meshes: Default::default(),
        terrain: None,
        water: None,
        lowres: None,
        terrain_layers: None,
        textures: Default::default(),
        collision: Default::default(),
        cutters: Vec::new(),
    };
    let mut picker = ClassPicker::new(&root.join("classes"), &scene);
    picker.load();
    println!("class picker catalogue: {} choices", picker.choices.len());
    for name in [
        "gun_inf_defender",
        "gun_inf_rider",
        "gun_inf_soldier",
        "imp_droid_probe",
        "all_hero_luke_storm",
        "rep_hero_cloakedanakin",
        "rep_hero_macewindu_obi",
    ] {
        assert!(
            !picker.choices.iter().any(|c| c.class.class == name),
            "NPC or unused class exposed: {name}"
        );
    }
    for (id, choice) in picker.choices.iter().enumerate() {
        assert!(picker
            .model(choice.tab)
            .choices
            .iter()
            .any(|(i, _)| *i == id));
        assert!((0..TABS.len())
            .filter(|&tab| tab != choice.tab)
            .all(|tab| !picker.model(tab).choices.iter().any(|(i, _)| *i == id)));
    }
    for (game, name) in [
        ("bf1", "all_inf_soldier"),
        ("bf2", "cis_inf_droideka"),
        ("bf2", "rep_hero_anakin"),
        ("bf2", "imp_hero_darthvader"),
        ("bf2", "cis_inf_pilot"),
        ("bf2", "cis_inf_marine"),
    ] {
        assert!(
            picker
                .choices
                .iter()
                .any(|c| c.game == game && c.class.class == name),
            "missing {game}/{name}"
        );
    }
    let mut unavailable = picker
        .choices
        .iter()
        .find(|c| c.class.droideka.is_none())
        .unwrap()
        .class
        .clone();
    unavailable.geometry = Some("nab_inf_gungan".into());
    assert!(!picker.has_player_assets("bf1", &unavailable));
    let mut walk = WalkSim::new(4, glam::Vec3::ZERO, None);
    let db = crate::roster::load_classdb(&root.join("classes"), "bf1", &["all"]).unwrap();
    let base = ClassRoster::build(&db, "all_inf_soldier", 0, None)
        .unwrap()
        .register(&mut walk.sim, &mut Vec::new());
    walk.sim.classes = [vec![base.clone()], vec![base]];
    let team = walk.sim.unit(walk.player).unwrap().team;
    let position = walk.soldier().pos;
    for name in ["rep_hero_anakin", "cis_inf_droideka", "all_inf_soldier"] {
        let index = picker
            .choices
            .iter()
            .position(|c| c.class.class == name)
            .unwrap();
        assert!(picker.select(&mut walk, index));
        let u = walk.sim.unit(walk.player).unwrap();
        assert_eq!((u.team, u.soldier.pos), (team, position));
        assert_eq!(walk.teams[usize::from(team)].class(u.spec.class).name, name);
        assert!(walk
            .sim
            .ai_class_weights(team)
            .iter()
            .skip(1)
            .all(|w| *w == 0.0));
        assert_eq!(walk.sim.spawn_class_indices(0, team), vec![0]);
    }
}

fn picker_on(map: &str) -> Option<ClassPicker> {
    let root = crate::assets::baked_root();
    let dir = root.join(map);
    if !dir.join("manifest.ron").is_file() {
        eprintln!("SKIP: local baked assets required");
        return None;
    }
    let scene = crate::assets::Scene {
        dir: dir.clone(),
        manifest: ron::from_str(&pak::read_to_string(dir.join("manifest.ron")).unwrap()).unwrap(),
        meshes: Default::default(),
        terrain: None,
        water: None,
        lowres: None,
        terrain_layers: None,
        textures: Default::default(),
        collision: Default::default(),
        cutters: Vec::new(),
    };
    let mut picker = ClassPicker::new(&root.join("classes"), &scene);
    picker.load();
    Some(picker)
}

fn grenade_of<'a>(picker: &'a ClassPicker, game: &str, class: &str) -> &'a crate::roster::HeldSlot {
    picker
        .choices
        .iter()
        .find(|c| c.game == game && c.class.class == class)
        .unwrap_or_else(|| panic!("{game} {class} on the picker"))
        .class
        .held
        .iter()
        .find(|h| is_hand_grenade(&h.weapon, h.channel))
        .unwrap_or_else(|| panic!("{game} {class} throws"))
}

/// The maintainer's rule (2026-11-01): a BF2 class fielded on a BF1 map throws
/// BF1's grenade by BF1's numbers, at a BF1 count, and shows BF2's HUD.
#[test]
fn a_bf2_class_on_a_bf1_map_throws_its_sides_bf1_grenade() {
    let Some(picker) = picker_on("bf1_geo1") else {
        return;
    };
    let root = crate::assets::baked_root();
    let bf1 =
        crate::roster::load_classdb(&root.join("classes"), "bf1", &["all", "imp", "rep", "cis"])
            .unwrap();
    for (class, thermal) in [
        ("rep_inf_ep3_jettrooper", "rep_weap_inf_thermaldetonator"),
        ("rep_inf_ep2_rifleman", "rep_weap_inf_thermaldetonator"),
        ("imp_inf_rifleman", "imp_weap_inf_thermaldetonator"),
        ("all_inf_wookiee", "all_weap_inf_thermaldetonator"),
        ("cis_inf_marine", "cis_weap_inf_thermaldetonator"),
        ("all_hero_leia", "all_weap_inf_thermaldetonator"),
    ] {
        let held = grenade_of(&picker, "bf2", class);
        let own = crate::roster::thrown_weapon_from_db(&bf1, thermal)
            .unwrap()
            .unwrap();
        assert_eq!(held.weapon, own, "{class}: BF1's class, unchanged");
        assert_eq!(held.weapon.initial_salvo_delay, 0.3, "{class}");
        assert_eq!(held.weapon.ordnance.life_span, 0.9, "{class}");
        assert_eq!(held.weapon.thrown.unwrap().hand, None, "{class}");
        assert!(
            !held.weapon.ordnance.sticky.as_ref().unwrap().bf2,
            "{class}"
        );
        assert!(!held.weapon.offhand_weapon, "{class}");
        assert_eq!(held.clips, 3, "{class}: BF1's plain thermal count");
        assert_eq!(held.hud.key, crate::roster::BF2_GRENADE_HUD_KEY, "{class}");
    }
}

/// A BF2 map is what it was: its own classes keep BF2's thermal and counts,
/// and a BF1 class picked there still throws as BF2 throws.
#[test]
fn a_bf2_map_keeps_bf2s_grenades_and_throw() {
    let Some(picker) = picker_on("bf2_kam1") else {
        return;
    };
    for (class, clips) in [
        ("rep_inf_ep3_jettrooper", 4),
        ("rep_inf_ep3_rifleman", 4),
        ("rep_inf_ep3_rocketeer", 2),
    ] {
        let held = grenade_of(&picker, "bf2", class);
        assert_eq!(held.clips, clips, "{class}");
        assert_eq!(
            held.weapon.initial_salvo_delay,
            Thrown::BF2_WIND_UP,
            "{class}"
        );
        assert_eq!(held.weapon.ordnance.life_span, Thrown::BF2_FUSE, "{class}");
        assert_eq!(
            held.weapon.thrown.unwrap().hand,
            Some(Thrown::BF2_HAND),
            "{class}"
        );
        assert!(held.weapon.offhand_weapon, "{class}");
        assert_eq!(held.hud.key, "rep_weap_inf_thermaldetonator", "{class}");
        assert!(held.hud.bf2, "{class}");
    }
    for (class, clips) in [("rep_inf_jet_trooper", 6), ("imp_inf_storm_trooper", 4)] {
        let held = grenade_of(&picker, "bf1", class);
        assert_eq!(held.clips, clips, "{class}: its disc's count");
        assert_eq!(
            held.weapon.initial_salvo_delay,
            Thrown::BF2_WIND_UP,
            "{class}"
        );
        assert_eq!(held.weapon.ordnance.life_span, Thrown::BF2_FUSE, "{class}");
        assert_eq!(
            held.weapon.thrown.unwrap().hand,
            Some(Thrown::BF2_HAND),
            "{class}"
        );
        assert!(held.weapon.ordnance.sticky.as_ref().unwrap().bf2, "{class}");
    }
}
