//! The shared AI consumes actual hero secondary classes through ordinary input.

use core::{
    ai::AiState,
    perception::{Contact, Foe},
    sim::{buttons, PadState, Sim},
    unit::{Controller, UnitHandle, UnitSpec},
};
use glam::Vec3;

fn scene(name: &str, distance: f32) -> Option<(Sim, UnitHandle, UnitHandle)> {
    let root = game::assets::baked_root();
    if !root.join("classes/bf2_rep.ron").is_file() {
        eprintln!("SKIP: actual BF2 hero classes required");
        return None;
    }
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &["rep", "imp"]).unwrap();
    let roster = game::roster::ClassRoster::build(&db, name, 0, Some(1)).unwrap();
    let mut sim = Sim::empty(5);
    sim.ground_rules = core::soldier::GroundRules::Bf2;
    sim.heightfield = Some(core::heightfield::Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    let class = roster.register(&mut sim, &mut Vec::new());
    let bot = sim
        .spawn(&class.spec(0, Vec3::ZERO, 0.0, Controller::Ai, 0, 17))
        .unwrap();
    let target = sim
        .spawn(&UnitSpec::trooper(1, Vec3::NEG_Z * distance))
        .unwrap();
    sim.units[target.index as usize].aim.err = 0.0;
    let direction = (sim.unit(target).unwrap().capsule().center()
        - sim.unit(bot).unwrap().muzzle())
    .normalize();
    sim.units[bot.index as usize].soldier.pitch = core::fmath::asin(direction.y);
    sim.tick = 90;
    Some((sim, bot, target))
}

fn input(
    sim: &mut Sim,
    bot: UnitHandle,
    target: UnitHandle,
    slot: u8,
    seen: bool,
    choose: bool,
) -> (PadState, AiState) {
    let u = sim.unit(bot).unwrap();
    let e = sim.unit(target).unwrap();
    let mut state = AiState::default();
    state.target = Some(Foe::Unit(target));
    state.contacts[0] = Some(Contact {
        handle: target.into(),
        first_seen: 0,
        last_seen: sim.tick,
        last_pos: e.soldier.pos,
        seen_now: seen,
        heard: false,
        told: false,
    });
    state.next_think = if choose { sim.tick } else { u64::MAX };
    state.last_think = sim.tick;
    state.aim_dir = (e.capsule().center() - u.muzzle()).normalize();
    state.arm.pick = Some(slot);
    state.arm.in_range = true;
    // Retain the native tap gate across consecutive wanting ticks. The
    // first tick may only release/rearm; resetting AiState would hide it.
    let mut pad = PadState::default();
    for _ in 0..3 {
        (pad, _) = core::ai::think(sim, bot, &mut state);
        if pad.buttons
            & (buttons::FIRE
                | buttons::SECONDARY_FIRE
                | buttons::SWITCH_WEAPON
                | buttons::SWITCH_SECONDARY)
            != 0
        {
            break;
        }
        sim.tick += 1;
    }
    (pad, state)
}

#[test]
fn hero_chosen_power_switches_and_fires_its_secondary_with_real_range_and_energy() {
    for name in [
        "rep_hero_macewindu",
        "imp_hero_darthvader",
        "imp_hero_emperor",
    ] {
        let Some((mut sim, bot, target)) = scene(name, 10.0) else {
            return;
        };
        let ui = bot.index as usize;
        let k = sim.units[ui]
            .held
            .iter()
            .position(|w| sim.weapon_classes[w.class].force.is_some())
            .unwrap();
        let slot = sim.units[ui].held[k].slot;
        sim.units[ui].cur1 = None;
        let (pad, _) = input(&mut sim, bot, target, slot, true, false);
        assert_ne!(
            pad.buttons & buttons::SWITCH_SECONDARY,
            0,
            "{name}: switch first"
        );
        assert_eq!(pad.buttons & (buttons::FIRE | buttons::SECONDARY_FIRE), 0);
        sim.units[ui].cur1 = Some(k as u8);
        let before = (sim.units[ui].soldier.pos, sim.units[ui].soldier.energy);
        let (pad, _) = input(&mut sim, bot, target, slot, true, false);
        assert_eq!(pad.buttons & buttons::FIRE, 0, "{name}: primary stays up");
        let class = &sim.weapon_classes[sim.units[ui].held[k].class];
        let admitted = class.ai_target_kind & (1 << sim.unit(target).unwrap().kind()) != 0;
        assert_eq!(
            admitted,
            name != "rep_hero_macewindu",
            "the disc disables ordinary Push AI target masks"
        );
        assert_eq!(
            pad.buttons & buttons::SECONDARY_FIRE != 0,
            admitted,
            "{name}: chosen power honors its actual target-kind mask"
        );
        assert_eq!(
            (sim.units[ui].soldier.pos, sim.units[ui].soldier.energy),
            before
        );
        sim.units[ui].soldier.energy = 0.0;
        assert_eq!(
            input(&mut sim, bot, target, slot, true, false).0.buttons & buttons::SECONDARY_FIRE,
            0
        );
        sim.units[ui].soldier.energy = before.1;
        assert_eq!(
            input(&mut sim, bot, target, slot, false, false).0.buttons & buttons::SECONDARY_FIRE,
            0
        );
        let range = sim.weapon_classes[sim.units[ui].held[k].class]
            .force
            .as_ref()
            .unwrap()
            .range;
        sim.units[target.index as usize].soldier.pos.z = -range - 2.0;
        assert_eq!(
            input(&mut sim, bot, target, slot, true, false).0.buttons & buttons::SECONDARY_FIRE,
            0
        );
    }
}

#[test]
fn actual_choke_rate_geometry_and_cost_reach_the_runtime_class() {
    let Some((sim, bot, _)) = scene("imp_hero_darthvader", 10.0) else {
        return;
    };
    let power = sim
        .unit(bot)
        .unwrap()
        .held
        .iter()
        .find_map(|held| sim.weapon_classes[held.class].force.as_ref())
        .unwrap();
    let core::hero_weapon::ForceKind::Choke {
        angle,
        damage_per_second,
        height,
        falloff,
        drain_rate,
    } = power.kind
    else {
        panic!("Vader must hold the actual area Choke class")
    };
    assert_eq!(angle.to_bits(), (10.0 * core::weapon::DEG).to_bits());
    assert!(
        power.affect_friends,
        "native physical selection includes friends"
    );
    assert_eq!(
        (power.range, power.energy, power.max_targets),
        (20.0, 40.0, 1)
    );
    assert_eq!(
        (damage_per_second, height, falloff, drain_rate),
        (150.0, 4.0, 1.0, 15.0)
    );
    assert_eq!(
        power.cone_cos,
        core::fmath::sin_cos(10.0 * core::weapon::DEG).1
    );
}

#[test]
fn hero_actual_choice_keeps_sword_inside_four_metres_and_uses_throw_outside() {
    for distance in [2.0, 10.0] {
        let Some((mut sim, bot, target)) = scene("rep_hero_macewindu", distance) else {
            return;
        };
        let u = sim.unit(bot).unwrap();
        let thrown = u
            .held
            .iter()
            .find(|w| sim.weapon_classes[w.class].saber_throw.is_some())
            .unwrap();
        let primary_slot = u.weapon_slot;
        let thrown_slot = thrown.slot;
        let (pad, state) = input(&mut sim, bot, target, thrown_slot, true, true);
        if distance < 4.0 {
            assert_eq!(state.arm.pick, Some(primary_slot));
        } else {
            assert_eq!(state.arm.pick, Some(thrown_slot));
            assert_ne!(pad.buttons & buttons::SECONDARY_FIRE, 0);
            assert_eq!(pad.buttons & buttons::FIRE, 0);
        }
    }
}

#[test]
fn hero_actual_melee_uses_the_native_single_trigger_tap_policy_to_repeat() {
    let Some((mut sim, bot, target)) = scene("rep_hero_macewindu", 2.0) else {
        return;
    };
    let u = sim.unit(bot).unwrap();
    let slot = u.weapon_slot;
    let before_energy = u.soldier.energy;
    assert!(
        sim.weapon_classes[u.weapon_class].trigger_single,
        "native melee default must reach the actual class"
    );
    let (first, mut state) = input(&mut sim, bot, target, slot, true, false);
    let mut presses = usize::from(first.buttons & buttons::FIRE != 0);
    let mut releases = 0;
    for tick in sim.tick + 1..sim.tick + 300 {
        sim.tick = tick;
        let (pad, _) = core::ai::think(&sim, bot, &mut state);
        presses += usize::from(pad.buttons & buttons::FIRE != 0);
        releases += usize::from(pad.buttons & buttons::FIRE == 0);
    }
    assert!(
        presses > 2 && releases > presses,
        "presses{presses}, releases{releases}"
    );
    assert_eq!(
        sim.unit(bot).unwrap().soldier.energy,
        before_energy,
        "the gunner produces input; it never grants or spends energy itself"
    );
}

#[test]
fn hero_melee_trigger_default_preserves_explicit_authored_false() {
    let path = std::env::temp_dir().join(format!("bf2_hero_trigger_{}.ron", std::process::id()));
    let defs: Vec<game::classdb::ClassDef> = [None, Some("0"), Some("1")]
        .into_iter()
        .enumerate()
        .map(|(i, value)| game::classdb::ClassDef {
            kind: "wpnc".into(),
            name: format!("test{i}"),
            base: "melee".into(),
            props: value
                .map(|v| vec![(game::classdb::fnv1a_lowercase("TriggerSingle"), v.into())])
                .unwrap_or_default(),
        })
        .collect();
    std::fs::write(&path, ron::to_string(&defs).unwrap()).unwrap();
    let mut db = game::classdb::ClassDb::default();
    db.load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    for (name, expected) in [("test0", true), ("test1", false), ("test2", true)] {
        assert_eq!(
            game::roster::weapon_from_db(&db, name)
                .unwrap()
                .trigger_single,
            expected
        );
    }
}
