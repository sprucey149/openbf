use std::path::Path;
use ui::hud::{HudGauge, HudModel, HudStats, HudTarget, HudWeapon};
use ui::hud_tree::EventValue;

pub const SIZES: [[u32; 2]; 3] = [[640, 480], [1280, 720], [3840, 2252]];
pub const ELEMENTS: &[&str] = &[
    "player1health_fill",
    "player1energy_fill",
    "player1jetpack_fill",
    "player1jetpack_threshold",
    "player1vehiclehealth_fill",
    "player1weapon1ammo",
    "player1weapon2ammo",
    "player1weapon1ammoInfinite",
    "player1weapon2ammoInfinite",
    "player1weapon1mesh",
    "player1weapon2mesh",
    "player1weapon1backdrop",
    "player1weapon2backdrop",
    "player1weapon1clipbar",
    "player1weapon1heatbar",
    "player1reticule_bullseye",
    "player1targethealth_fill",
    "player1targetinfo_classNameNEW",
    "player1teams_team1reinforcements",
    "player1teams_team2reinforcements",
    "player1statskills.icon",
    "player1statspoints.icon",
    "player1statskills",
    "player1statspoints",
    "victorytimer",
    "defeattimer",
];

pub fn model(assets: &Path, game: &str, state: u8) -> HudModel {
    let mut scene =
        game::assets::load_scene(&assets.join(if game == "bf2" { "bf2_hot1" } else { "tat1" }))
            .expect("chosen input scene");
    scene.apply_mode_layer("con");
    let weapon = |key: &str, count| HudWeapon {
        key: key.into(),
        total: count,
        clip_fraction: 0.7,
        bf2: game == "bf2",
        ..Default::default()
    };
    let mut model = HudModel {
        team_names: ["rep".into(), "cis".into()],
        tickets: [147, 145],
        hp: 220.0,
        hp_max: 300.0,
        energy: 64.0,
        energy_max: 100.0,
        jet: Some((0.6, 0.2)),
        weapon: Some(weapon(
            if state == 1 {
                "hud_vehicle_lasers"
            } else {
                "rep_weap_inf_rifle"
            },
            if state == 1 { None } else { Some(250) },
        )),
        weapon2: Some(weapon(
            if state == 1 {
                "hud_vehicle_missile"
            } else {
                "rep_weap_inf_thermaldetonator"
            },
            Some(4),
        )),
        reticle: Some(0),
        reticle_y: game::walk::ps2_reticle_y(game),
        gauge: HudGauge {
            ring: 0.5,
            ..Default::default()
        },
        target: Some(HudTarget {
            id: 1,
            name: "Target".into(),
            health_fraction: 0.75,
            ..Default::default()
        }),
        map: Some(game::minimap::still(&scene, glam::Vec3::ZERO, 0.0)),
        countdown: Some((state == 0, 27.0)),
        stats: Some(HudStats::default()),
        dead: state == 2,
        ..Default::default()
    };
    if game == "bf1" && state == 0 {
        model.weapon2 = Some(HudWeapon {
            key: "com_weap_inf_grenadethermal".into(),
            class: "rep_weap_inf_thermaldetonator".into(),
            icon: Some("hud_com_grenade_icon".into()),
            total: Some(4),
            clip_fraction: 1.0,
            bf2: false,
            ..Default::default()
        });
    }
    model
}

pub fn settle(view: &mut game::hud_feed::HudView, model: &HudModel, state: u8, size: [u32; 2]) {
    if state == 2 {
        let mut alive = model.clone();
        alive.dead = false;
        view.update(&alive, 0.0, size);
    }
    view.update(model, 0.0, size);
    if state == 1 {
        view.send("player1.vehicle.health", EventValue::Number(500.0));
        view.send("player1.vehicle.healthFraction", EventValue::Number(0.6));
    }
    let mut scored = model.clone();
    // The initial settle deliberately expires native transient gauges.
    // A fuel change makes the jet bar visible again during this measurement.
    scored.jet = Some((0.55, 0.2));
    scored.stats = Some(HudStats {
        kills: 2,
        points: 2,
        ..Default::default()
    });
    view.update(&scored, 1.0 / 30.0, size);
    view.update(&scored, 1.0, size);
}
