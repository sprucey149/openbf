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
