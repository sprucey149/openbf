use super::*;
use crate::soldier::GroundRules;

#[test]
fn testing_picker_preserves_team_position_tickets_and_bot_mix() {
    for team in [0, 1] {
        let mut s = change_sim(GroundRules::Bf2);
        s.units[0].team = team;
        s.units[0].spec.team = team;
        s.battle.teams[0].alive_players = u32::from(team == 0);
        s.battle.teams[1].alive_players = u32::from(team == 1);
        let h = s.players[0].unwrap();
        let position = s.units[0].soldier.pos;
        let tickets = s.battle.teams.map(|t| t.tickets());
        let weights = s.ai_class_weights(team);
        let kills = s.units[0].kills;
        s.begin_test_classes(team);
        s.classes[usize::from(team)].push(s.classes[usize::from(team)][1].clone());
        assert!(s.test_change_class(0, 2, true));
        let next = s.players[0].unwrap();
        assert_ne!(next, h);
        let u = s.unit(next).unwrap();
        assert_eq!(
            (u.team, u.soldier.pos, u.spec.class, u.kills),
            (team, position, 2, kills)
        );
        assert_eq!(u.health.cur, u.health.max);
        assert!(s.is_hero(next));
        assert_eq!(s.battle.teams.map(|t| t.tickets()), tickets);
        assert_eq!(&s.ai_class_weights(team)[..2], &weights);
        assert_eq!(s.ai_class_weights(team)[2], 0.0);
        assert_eq!(s.spawn_class_indices(0, team), vec![0, 1]);
        assert!(s.test_change_class(0, 1, false));
        assert!(!s.is_hero(s.players[0].unwrap()));
        assert_eq!(s.hero_class(team), None);
        let next = s.players[0].unwrap();
        s.units[next.index as usize].weapon.clip = 0;
        assert!(s.test_change_class(0, 1, false));
        assert!(s.unit(s.players[0].unwrap()).unwrap().weapon.clip > 0);
        assert!(!s.test_change_class(0, 255, false));
    }
}

#[test]
fn testing_picker_keeps_the_original_hero_out_of_bot_spawns() {
    let mut s = change_sim(GroundRules::Bf2);
    s.heroes = Some(crate::sim::heroes::Heroes::new([Some(1), None]));
    let weights = s.ai_class_weights(0);
    s.begin_test_classes(0);
    s.classes[0].push(s.classes[0][1].clone());
    assert!(s.test_change_class(0, 2, true));
    assert_eq!(&s.ai_class_weights(0)[..2], &weights);
    assert_eq!(s.spawn_class_indices(0, 0), vec![0]);
    assert!(s.test_change_class(0, 0, false));
    assert_eq!(s.hero_class(0), Some(1));
    assert!(s.test_change_class(0, 2, true));
    s.units[0].alive = false;
    s.step(&[PadState::default()]);
    assert_eq!(s.hero_class(0), Some(1));
}

// ---- Issue #28: change class at a friendly post ------------------------

/// Two classes per team: 0 a 300-health rifleman (weapon 0), 1 a
/// 200-health class with another weapon (weapon 1). Team 0's post at the
/// origin, team 1's at x = 100, a neutral one at x = -100 (each a sphere
/// of 8 m). Slot 0's human (class 0, team 0) stands 4 m south of team 0's
/// post facing it; a team-0 bot stands beside him, also facing it.
fn change_sim(rules: GroundRules) -> Sim {
    let mut s = Sim::empty(11);
    s.ground_rules = rules;
    s.heightfield = Some(flat_ground());
    let w1 = s.add_weapon_class(WeaponClass::bf2_rifle());
    for t in 0..2 {
        let mut second = class_w(1, None, w1);
        second.max_health = 200.0;
        s.classes[t] = vec![class_w(1, None, 0), second];
    }
    for (x, owner) in [(0.0, Some(0u8)), (100.0, Some(1)), (-100.0, None)] {
        let pos = Vec3::new(x, 0.0, 0.0);
        s.add_cp(
            CpDef {
                name: format!("cp{x}"),
                pos,
                capture: Region::sphere(pos, 8.0),
                control: None,
                value_bleed: 1,
                value_atk: [1, 1],
                value_def: [1, 1],
                spawn_nodes: vec![(pos + Vec3::new(0.0, 0.0, 6.0), 0.0)],
                capture_time: 10.0,
                neutralize_time: 10.0,
            },
            owner,
        );
    }
    // Yaw 0 looks down -z: from z = +4 that is at the post.
    let me = s.classes[0][0].spec(
        0,
        Vec3::new(0.0, 0.0, 4.0),
        0.0,
        Controller::Player { slot: 0 },
        0,
        0,
    );
    s.spawn(&me).unwrap();
    let bot = s.classes[0][0].spec(0, Vec3::new(1.0, 0.0, 4.0), 0.0, Controller::Idle, 0, 0);
    s.spawn(&bot).unwrap();
    s.units[0].soldier.pitch = 0.0;
    s
}

fn me(s: &Sim) -> &crate::unit::Unit {
    &s.units[s.players[0].unwrap().index as usize]
}

fn tap(s: &mut Sim, bit: u32) {
    s.step(&[press(bit)]);
    s.step(&[PadState::default()]);
}

/// Open the screen with the use button, step to class `to`, accept.
fn change_to(s: &mut Sim, to: u8) {
    tap(s, buttons::ACTION | buttons::SPAWN_BACK);
    assert_eq!(
        s.spawn_stage[0],
        SpawnStage::Alive,
        "the class screen opened"
    );
    while s.spawn_choice[0] != to {
        tap(s, buttons::SPAWN_CLASS_NEXT);
        while s.class_carousel[0].slide > 0 {
            s.step(&[PadState::default()]);
        }
    }
    tap(s, buttons::SPAWN_ACCEPT | buttons::JUMP);
    assert_eq!(s.spawn_stage[0], SpawnStage::Accepted);
    assert_eq!(me(s).spec.class, to);
}

fn place(s: &mut Sim, pos: Vec3, yaw: f32) {
    let i = s.players[0].unwrap().index as usize;
    s.units[i].soldier.pos = pos;
    s.units[i].soldier.yaw = yaw;
}

#[test]
fn the_use_button_at_a_friendly_post_opens_the_class_screen_and_swaps_in_place() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        let mut s = change_sim(rules);
        let before = s.players[0].unwrap();
        let tickets = s.battle.teams[0].tickets();
        let alive = s.battle.teams[0].alive_players;
        let at = me(&s).soldier.pos;
        change_to(&mut s, 1);
        let after = s.players[0].unwrap();
        assert_eq!(after.index, before.index);
        assert_eq!(
            after.gen,
            before.gen + 1,
            "a new unit, as the PS2's new object"
        );
        assert!(me(&s).alive);
        assert!(
            (me(&s).soldier.pos - at).length() < 0.5,
            "{rules:?}: changed where he stood"
        );
        assert_eq!(
            s.battle.teams[0].tickets(),
            tickets,
            "no reinforcement spent"
        );
        assert_eq!(s.battle.teams[0].alive_players, alive);
        assert_eq!(me(&s).weapon_class, 1);
    }
}

#[test]
fn out_of_range_or_facing_away_the_use_button_does_nothing() {
    let mut s = change_sim(GroundRules::Bf2);
    // 9 m out: outside the 8 m region.
    place(&mut s, Vec3::new(0.0, 0.0, 9.0), 0.0);
    assert_eq!(s.class_change_post(0), None);
    tap(&mut s, buttons::ACTION);
    assert_eq!(s.spawn_stage[0], SpawnStage::Accepted);
    // Inside but facing away (yaw pi looks down +z).
    place(&mut s, Vec3::new(0.0, 0.0, 4.0), std::f32::consts::PI);
    tap(&mut s, buttons::ACTION);
    assert_eq!(s.spawn_stage[0], SpawnStage::Accepted);
    // Inside and facing it: opens.
    place(&mut s, Vec3::new(0.0, 0.0, 7.5), 0.0);
    assert_eq!(s.class_change_post(0), Some(0));
    tap(&mut s, buttons::ACTION);
    assert_eq!(s.spawn_stage[0], SpawnStage::Alive);
}

#[test]
fn enemy_and_neutral_posts_refuse() {
    let mut s = change_sim(GroundRules::Bf2);
    for x in [100.0, -100.0] {
        place(&mut s, Vec3::new(x, 0.0, 4.0), 0.0);
        assert_eq!(s.class_change_post(0), None, "post at x = {x}");
        tap(&mut s, buttons::ACTION);
        assert_eq!(s.spawn_stage[0], SpawnStage::Accepted, "post at x = {x}");
        assert_eq!(me(&s).spec.class, 0);
    }
}

#[test]
fn bots_are_not_affected() {
    let mut s = change_sim(GroundRules::Bf2);
    let bot = s.units[1].gen;
    assert!(s.cps[0].def.capture.contains(s.units[1].soldier.pos));
    assert_eq!(s.class_change_post(1), None, "a bot never qualifies");
    change_to(&mut s, 1);
    assert_eq!(s.units[1].gen, bot);
    assert_eq!(s.units[1].spec.class, 0);
}

#[test]
fn back_returns_to_play_on_the_same_unit() {
    let mut s = change_sim(GroundRules::Bf2);
    let h = s.players[0].unwrap();
    tap(&mut s, buttons::ACTION | buttons::SPAWN_BACK);
    assert_eq!(s.spawn_stage[0], SpawnStage::Alive);
    // The soldier gets no pad while it is up.
    let at = me(&s).soldier.pos;
    for _ in 0..30 {
        s.step(&[PadState {
            ly: i16::MAX,
            ..press(buttons::SPAWN_CLASS_NEXT)
        }]);
    }
    assert!((me(&s).soldier.pos - at).length() < 1e-3, "did not walk");
    tap(&mut s, buttons::ACTION | buttons::SPAWN_BACK);
    assert_eq!(s.spawn_stage[0], SpawnStage::Accepted);
    assert_eq!(s.players[0], Some(h), "no swap");
    assert_eq!(s.spawn_choice[0], 0, "the choice is the class he plays");
}

#[test]
fn health_carries_both_ways_as_a_share() {
    let mut s = change_sim(GroundRules::Bf2);
    let i = s.players[0].unwrap().index as usize;
    s.units[i].health.cur = 150.0; // half of 300
    change_to(&mut s, 1);
    assert_eq!(me(&s).health.max, 200.0);
    assert!(
        (me(&s).health.cur - 100.0).abs() < 1e-3,
        "{}",
        me(&s).health.cur
    );
    s.units[i].health.cur = 50.0; // a quarter of 200
    change_to(&mut s, 0);
    assert!(
        (me(&s).health.cur - 75.0).abs() < 1e-3,
        "{}",
        me(&s).health.cur
    );
}

#[test]
fn a_fresh_class_is_full_and_a_swap_back_restores_its_ammo() {
    let mut s = change_sim(GroundRules::Bf2);
    let i = s.players[0].unwrap().index as usize;
    let full = (s.units[i].weapon.clip, s.units[i].weapon.spare);
    assert!(full.0 > 0 && full.1 > 0);
    // Out of ammo.
    s.units[i].weapon.clip = 0;
    s.units[i].weapon.spare = 0;
    change_to(&mut s, 1);
    let fresh = crate::weapon::WeaponState::with_clips(&s.weapon_classes[1], true, 6);
    assert_eq!(
        (me(&s).weapon.clip, me(&s).weapon.spare),
        (fresh.clip, fresh.spare),
        "a class not played this life starts full"
    );
    s.units[i].weapon.clip = 3;
    change_to(&mut s, 0);
    assert_eq!(
        (me(&s).weapon.clip, me(&s).weapon.spare),
        (0, 0),
        "no refill"
    );
    change_to(&mut s, 1);
    assert_eq!((me(&s).weapon.clip, me(&s).weapon.spare), (3, fresh.spare));
}

#[test]
fn a_death_forgets_every_class() {
    let mut s = change_sim(GroundRules::Bf2);
    let i = s.players[0].unwrap().index as usize;
    s.units[i].weapon.clip = 0;
    s.units[i].weapon.spare = 0;
    change_to(&mut s, 1);
    assert_eq!(s.class_ammo[0].len(), 1);
    let h = s.players[0].unwrap();
    s.kill_environment(h, crate::sim::DeathCause::Suicide);
    assert!(s.class_ammo[0].is_empty(), "death clears the memory");
    // The next life: back as class 1, then to class 0 - full again.
    s.respawn_delay = 0.0;
    for _ in 0..crate::TICK_HZ * 20 {
        s.step(&[PadState::default()]);
        if me(&s).alive {
            break;
        }
    }
    assert!(me(&s).alive);
    place(&mut s, Vec3::new(0.0, 0.0, 4.0), 0.0);
    s.units[i].soldier.pitch = 0.0;
    let to = 1 - me(&s).spec.class;
    change_to(&mut s, to);
    let fresh = crate::weapon::WeaponState::with_clips(&s.weapon_classes[usize::from(to)], true, 6);
    assert_eq!(
        (me(&s).weapon.clip, me(&s).weapon.spare),
        (fresh.clip, fresh.spare)
    );
    assert!((me(&s).health.cur - me(&s).health.max).abs() < 1e-3);
}

#[test]
fn a_death_with_the_screen_up_closes_it() {
    let mut s = change_sim(GroundRules::Bf2);
    tap(&mut s, buttons::ACTION);
    assert_eq!(s.spawn_stage[0], SpawnStage::Alive);
    let h = s.players[0].unwrap();
    s.kill_environment(h, crate::sim::DeathCause::Suicide);
    assert_eq!(s.spawn_stage[0], SpawnStage::Accepted);
}

/// Kills scored after a change land on the new class's life (the career
/// count, #19, reads `spec.class` of the slot's unit): the swap keeps the
/// tallies and the class is the new one.
#[test]
fn kills_carry_and_the_class_is_the_new_one() {
    let mut s = change_sim(GroundRules::Bf2);
    let i = s.players[0].unwrap().index as usize;
    s.units[i].kills = 4;
    s.units[i].deaths = 2;
    change_to(&mut s, 1);
    assert_eq!((me(&s).kills, me(&s).deaths, me(&s).spec.class), (4, 2, 1));
}

/// The ammo memory is sim state: a change moves the hash, and a replay
/// of the same pads reproduces it.
#[test]
fn a_class_change_replays() {
    let run = || {
        let mut s = change_sim(GroundRules::Bf2);
        change_to(&mut s, 1);
        s.hash()
    };
    assert_eq!(run(), run());
}
