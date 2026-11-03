use super::*;

// ---- Card SSELA: the spawn screens -------------------------------------

/// Two teams, two posts each (team 0 at x = 0 and 100, team 1 at
/// x = -300 and -200, in that list order), two classes per team (weapon
/// 0 and 1); slot 0's player held behind the screens, the side screen up
/// when `side`.
fn screens_sim(seed: u32, side: bool) -> Sim {
    let mut s = Sim::empty(seed);
    s.heightfield = Some(flat_ground());
    s.add_weapon_class(WeaponClass::bf2_rifle());
    for t in 0..2 {
        s.classes[t] = vec![class_w(1, None, 0), class_w(1, None, 1)];
    }
    for (x, team) in [(0.0, 0u8), (100.0, 0), (-300.0, 1), (-200.0, 1)] {
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
                spawn_nodes: vec![(pos + Vec3::new(0.0, 0.0, 1.0), 0.0)],
                capture_time: 10.0,
                neutralize_time: 10.0,
            },
            Some(team),
        );
    }
    s.spawn(&UnitSpec::player_in_slot(0, 0, Vec3::new(50.0, 0.0, 30.0)))
        .unwrap();
    assert!(s.open_spawn_screens(0, side));
    s
}

/// One press of `bit` on its own tick, then a released tick (an edge each
/// time); returns whether slot 0 was on the field after the press tick.
fn tap(s: &mut Sim, bit: u32) -> bool {
    s.step(&[press(bit)]);
    let on = s.units[s.players[0].unwrap().index as usize].alive;
    s.step(&[PadState::default()]);
    on
}

fn slot0(s: &Sim) -> &crate::unit::Unit {
    &s.units[s.players[0].unwrap().index as usize]
}

/// Acceptance 1: a slot that never accepts is not on the field after
/// 60 s and has cost nothing; Down, Accept (team 2), Right + Accept
/// (post), Back, Accept, Left + Accept (class) puts it on team 2 at the
/// chosen post as the chosen class ON the tick of the last accept.
#[test]
fn a_slot_is_off_the_field_until_it_accepts_side_post_and_class() {
    let mut s = screens_sim(5, true);
    let tickets = [s.battle.teams[0].tickets(), s.battle.teams[1].tickets()];
    assert_eq!(s.spawn_stage[0], SpawnStage::Side);
    for _ in 0..60 * crate::TICK_HZ {
        s.step(&[PadState::default()]);
    }
    assert!(!slot0(&s).alive, "never accepted: no unit on the field");
    assert_eq!(
        s.battle.teams[0].alive_players + s.battle.teams[1].alive_players,
        0
    );
    let now = [s.battle.teams[0].tickets(), s.battle.teams[1].tickets()];
    assert_eq!(now, tickets, "no reinforcement spent");
    // Back does nothing on the side screen.
    tap(&mut s, buttons::SPAWN_BACK);
    assert_eq!(s.spawn_stage[0], SpawnStage::Side);
    tap(&mut s, buttons::SPAWN_POST_DOWN);
    assert_eq!(s.side_choice[0], SideChoice::Team2);
    assert!(!tap(&mut s, buttons::SPAWN_ACCEPT));
    assert_eq!(s.spawn_stage[0], SpawnStage::Map);
    assert_eq!(slot0(&s).team, 1, "the side accept sets the team");
    // The map opens on team 2's first post; Right takes the other.
    assert_eq!(s.spawn_post_selected(0), Some(SpawnPost::Post(2)));
    tap(&mut s, buttons::SPAWN_POST_RIGHT);
    assert_eq!(s.spawn_post_selected(0), Some(SpawnPost::Post(3)));
    // Back does nothing on the spawn map (offline).
    tap(&mut s, buttons::SPAWN_BACK);
    assert_eq!(s.spawn_stage[0], SpawnStage::Map);
    assert!(!tap(&mut s, buttons::SPAWN_ACCEPT));
    assert_eq!(s.spawn_stage[0], SpawnStage::Class);
    // Back on the class screen returns to the map, the post kept.
    assert!(!tap(&mut s, buttons::SPAWN_BACK));
    assert_eq!(s.spawn_stage[0], SpawnStage::Map);
    assert_eq!(s.spawn_post_selected(0), Some(SpawnPost::Post(3)));
    assert!(!tap(&mut s, buttons::SPAWN_ACCEPT));
    // The carousel: Left = the next class.
    tap(&mut s, buttons::SPAWN_POST_LEFT);
    assert_eq!(s.spawn_choice[0], 1);
    assert_eq!(
        now,
        [s.battle.teams[0].tickets(), s.battle.teams[1].tickets()]
    );
    assert!(
        tap(&mut s, buttons::SPAWN_ACCEPT),
        "on the field on the accept's tick"
    );
    let u = slot0(&s);
    assert_eq!(s.spawn_stage[0], SpawnStage::Accepted);
    assert_eq!((u.team, u.spec.class, u.weapon_class), (1, 1, 1));
    assert!(
        (u.soldier.pos.x + 200.0).abs() < 0.01,
        "at post 3: {:?}",
        u.soldier.pos
    );
    assert_eq!(s.battle.teams[1].alive_players, 1);
}

/// Card SSELB (`FUN_00399dd8`, re-spawn-flow §4.1): on the class screen
/// a push steps once and re-arms only when let go; a fresh push while
/// the line-up slides (11 ticks) is dropped, and a push HELD through the
/// slide steps again in the tick after it ends.
#[test]
fn a_class_step_during_the_slide_is_dropped() {
    let mut s = screens_sim(6, false);
    let c = s.classes[0][0].clone();
    s.classes[0] = vec![c; 5];
    tap(&mut s, buttons::SPAWN_POST_RIGHT);
    tap(&mut s, buttons::SPAWN_ACCEPT);
    assert_eq!(s.spawn_stage[0], SpawnStage::Class);
    assert_eq!(s.spawn_choice[0], 0);
    // A tap (tick 0 pressed, tick 1 let go), then a second push on
    // ticks 2..=9: still sliding, dropped.
    tap(&mut s, buttons::SPAWN_POST_LEFT);
    assert_eq!(s.spawn_choice[0], 1, "Left = the next class");
    for _ in 0..8 {
        s.step(&[press(buttons::SPAWN_POST_LEFT)]);
        assert_eq!(s.spawn_choice[0], 1, "a step during the slide is dropped");
    }
    // Still held: the slide ended at tick 10, the held push fires at 11.
    s.step(&[press(buttons::SPAWN_POST_LEFT)]);
    assert_eq!(s.spawn_choice[0], 1);
    s.step(&[press(buttons::SPAWN_POST_LEFT)]);
    assert_eq!(
        s.spawn_choice[0], 2,
        "held through the slide: steps at its end"
    );
    // Held on: armed, no repeat.
    for _ in 0..30 {
        s.step(&[press(buttons::SPAWN_POST_LEFT)]);
    }
    assert_eq!(s.spawn_choice[0], 2, "a held push steps once");
    // Straight from Left to Right without letting go: still armed.
    s.step(&[press(buttons::SPAWN_POST_RIGHT)]);
    assert_eq!(s.spawn_choice[0], 2, "no let-go, no step");
    // Let go, then Right: back one, wrapping past 0 to the last.
    s.step(&[PadState::default()]);
    tap(&mut s, buttons::SPAWN_POST_RIGHT);
    assert_eq!(s.spawn_choice[0], 1);
    for _ in 0..CLASS_SLIDE_TICKS {
        s.step(&[PadState::default()]);
    }
    tap(&mut s, buttons::SPAWN_CLASS_PREV);
    for _ in 0..CLASS_SLIDE_TICKS {
        s.step(&[PadState::default()]);
    }
    tap(&mut s, buttons::SPAWN_POST_RIGHT);
    assert_eq!(s.spawn_choice[0], 4, "Right off class 0 wraps to the last");
    // The accept during a slide takes the class already stepped to.
    assert!(tap(&mut s, buttons::SPAWN_ACCEPT));
    assert_eq!(s.spawn_choice[0], 4);
}

/// Card SSELB (re-spawn-flow §4.3, the PS2's `FUN_00364290`): a class
/// that costs points refuses the accept until the slot has them (the
/// panel's count of points still needed); `[ours]` a class at its authored
/// max is never refused.
#[test]
fn a_locked_class_refuses_the_accept_until_the_points_are_there() {
    let mut s = screens_sim(5, false);
    s.classes[0][0].max = Some(0);
    s.classes[0][1].unlock_points = 8;
    s.classes[0][1].max = Some(1);
    tap(&mut s, buttons::SPAWN_ACCEPT);
    assert_eq!(s.spawn_stage[0], SpawnStage::Class);
    assert_eq!(s.class_refusal(0, 0, 0), None, "no cost, max 0: open");
    tap(&mut s, buttons::SPAWN_POST_LEFT);
    assert_eq!(s.spawn_choice[0], 1);
    assert_eq!(s.class_refusal(0, 0, 1), Some(ClassRefusal::Points(8)));
    assert!(!tap(&mut s, buttons::SPAWN_ACCEPT), "refused: stays");
    assert_eq!(s.spawn_stage[0], SpawnStage::Class);
    s.set_spawn_points(0, 5);
    assert_eq!(s.class_refusal(0, 0, 1), Some(ClassRefusal::Points(3)));
    assert!(!tap(&mut s, buttons::SPAWN_ACCEPT));
    s.set_spawn_points(0, 8);
    assert!(tap(&mut s, buttons::SPAWN_ACCEPT), "unlocked");
    assert_eq!(slot0(&s).spec.class, 1);
    assert_eq!(s.class_count(0, 1), 1);
    assert_eq!(s.class_refusal(0, 0, 1), None, "at its max: still open");
}

/// Card UNLK3: the slot's career cost replaces the class's own in the
/// points test (half, then none).
#[test]
fn a_career_cost_replaces_the_points() {
    let mut s = screens_sim(5, false);
    s.classes[0][1].unlock_points = 12;
    s.classes[0][1].max = Some(1);
    assert_eq!(s.spawn_cost(0, 0, 1), 12, "no career: the class's own");
    s.set_spawn_costs(0, 0, vec![0, 6]);
    s.set_spawn_points(0, 5);
    assert_eq!(s.class_refusal(0, 0, 1), Some(ClassRefusal::Points(1)));
    s.set_spawn_points(0, 6);
    assert_eq!(s.class_refusal(0, 0, 1), None);
    s.set_spawn_costs(0, 0, vec![0, 0]);
    s.set_spawn_points(0, 0);
    assert_eq!(s.class_refusal(0, 0, 1), None, "unlocked for good");
    tap(&mut s, buttons::SPAWN_ACCEPT);
    tap(&mut s, buttons::SPAWN_POST_LEFT);
    assert!(tap(&mut s, buttons::SPAWN_ACCEPT));
    assert_eq!(slot0(&s).spec.class, 1);
    assert_eq!(s.class_refusal(0, 0, 1), None, "at its max: still open");
    assert_eq!(s.spawn_cost(1, 0, 1), 12, "another slot: the class's own");
}

/// Acceptance 2: after a death the slot is not back before 4.0 s nor
/// without the class accept, and IS back on the accept's tick while its
/// team's spawn wave is not ready (the human skips the wave); the post
/// and the class of the last life are kept.
#[test]
fn after_a_death_the_timer_then_map_and_class_and_no_wave() {
    let mut s = screens_sim(6, false);
    assert_eq!(
        s.spawn_stage[0],
        SpawnStage::Map,
        "no side choice: the map first"
    );
    tap(&mut s, buttons::SPAWN_POST_RIGHT);
    tap(&mut s, buttons::SPAWN_ACCEPT);
    tap(&mut s, buttons::SPAWN_CLASS_NEXT);
    assert!(tap(&mut s, buttons::SPAWN_ACCEPT));
    let h = s.players[0].unwrap();
    s.kill_environment(h, crate::sim::DeathCause::Fall);
    let died = s.tick;
    let wave_slot = slot0(&s).spawn_slot.expect("a death takes a wave slot");
    let delay = u64::from(4 * crate::TICK_HZ);
    // Accept pressed on every other tick while the timer runs: nothing.
    while s.tick + 1 < died + delay {
        assert!(!tap(&mut s, buttons::SPAWN_ACCEPT), "back before 4.0 s");
        assert_eq!(s.spawn_stage[0], SpawnStage::Accepted);
    }
    for _ in 0..3 {
        s.step(&[PadState::default()]);
    }
    assert_eq!(
        s.spawn_stage[0],
        SpawnStage::Map,
        "the timer opened the map"
    );
    for _ in 0..crate::TICK_HZ {
        s.step(&[PadState::default()]);
    }
    assert!(!slot0(&s).alive, "not without the accepts");
    assert_eq!(
        s.spawn_post_selected(0),
        Some(SpawnPost::Post(1)),
        "post kept"
    );
    assert!(!tap(&mut s, buttons::SPAWN_ACCEPT));
    assert_eq!(s.spawn_stage[0], SpawnStage::Class);
    assert_eq!(s.spawn_choice[0], 1, "class kept");
    assert!(!s.waves[0].ready(wave_slot), "the wave is not ready");
    assert!(
        tap(&mut s, buttons::SPAWN_ACCEPT),
        "back on the accept's tick"
    );
    let u = slot0(&s);
    assert!((u.soldier.pos.x - 100.0).abs() < 0.01 && u.spec.class == 1);
}

/// Without the screens (the referees, `--spawn`) a dead human still
/// comes back by itself when its timer runs out - and does not wait for
/// the spawn wave either.
#[test]
fn auto_accept_respawns_on_the_timer_without_the_wave() {
    let mut s = spawn_select_sim();
    let h = s.players[0].unwrap();
    s.kill_environment(h, crate::sim::DeathCause::Fall);
    let slot = slot0(&s).spawn_slot.unwrap();
    for _ in 0..4 * crate::TICK_HZ + 1 {
        s.step(&[PadState::default()]);
    }
    assert!(slot0(&s).alive, "back after 4.0 s");
    assert!(!s.waves[0].ready(slot), "while its wave is not ready");
}

/// Acceptance 3: Auto Assign flips the PS2's coin on the sim's LCG -
/// over 32 seeds both teams occur; the X that spawns is not a jump.
#[test]
fn auto_assign_draws_both_teams_over_32_seeds() {
    let mut teams = [0u32; 2];
    for seed in 0..32 {
        let mut s = screens_sim(seed, true);
        tap(&mut s, buttons::SPAWN_POST_UP);
        assert_eq!(
            s.side_choice[0],
            SideChoice::Auto,
            "Up wraps to Auto Assign"
        );
        tap(&mut s, buttons::SPAWN_ACCEPT);
        let t = slot0(&s).team;
        teams[usize::from(t)] += 1;
        assert_eq!(
            s.spawn_post_selected(0),
            Some(SpawnPost::Post(if t == 0 { 0 } else { 2 }))
        );
        // Held X through the spawn: the soldier does not jump.
        tap(&mut s, buttons::SPAWN_ACCEPT);
        let both = buttons::SPAWN_ACCEPT | buttons::JUMP;
        s.step(&[press(both)]);
        s.step(&[press(both)]);
        assert!(slot0(&s).alive);
        assert!(slot0(&s).soldier.grounded, "seed {seed}: the accept jumped");
    }
    assert!(teams[0] > 0 && teams[1] > 0, "both teams: {teams:?}");
}

/// Acceptance 4: stage, side row and the switch are hashed (two
/// different set values apart, not only set against unset); a pad log
/// through the screens replays to the same hash, and two logs that pick
/// different sides hash apart.
#[test]
fn the_screens_are_hashed_and_replay() {
    let mut s = spawn_select_sim();
    let base = s.hash();
    s.sync_slot_tables();
    assert_eq!(s.hash(), base, "defaults eat nothing");
    s.side_choice[0] = SideChoice::Team2;
    let a = s.hash();
    s.side_choice[0] = SideChoice::Auto;
    let b = s.hash();
    assert!(a != base && b != base && a != b, "the side row");
    s.side_choice[0] = SideChoice::Team1;
    s.spawn_stage[0] = SpawnStage::Map;
    let m = s.hash();
    s.spawn_stage[0] = SpawnStage::Class;
    let c = s.hash();
    assert!(m != base && c != base && m != c, "the stage");
    s.spawn_stage[0] = SpawnStage::Accepted;
    assert_eq!(s.hash(), base);
    s.spawn_screens = true;
    assert_ne!(s.hash(), base, "the switch");

    let run = |log: &InputLog| {
        let mut s = screens_sim(log.seed, true);
        for pads in log.frames() {
            s.step(pads);
        }
        (s.hash(), slot0(&s).team, slot0(&s).alive)
    };
    let mut log = InputLog {
        seed: 41,
        slots: 1,
        ticks: vec![PadState::default(); 40],
    };
    for (t, b) in [
        (4, buttons::SPAWN_ACCEPT),
        (8, buttons::SPAWN_ACCEPT),
        (12, buttons::SPAWN_ACCEPT),
    ] {
        log.ticks[t] = press(b);
    }
    let one = run(&log);
    assert_eq!(one, run(&log), "replays");
    assert_eq!((one.1, one.2), (0, true));
    log.ticks[2] = press(buttons::SPAWN_POST_DOWN);
    let two = run(&log);
    assert_eq!((two.1, two.2), (1, true));
    assert_ne!(one.0, two.0, "two sides hash apart");
}

// ---- PR #102: the mouse picks come in the pad ---------------------------

fn pick(p: crate::sim::ScreenPick) -> PadState {
    PadState {
        pick: Some(p),
        ..PadState::default()
    }
}

fn pick_accept(p: crate::sim::ScreenPick) -> PadState {
    PadState {
        pick: Some(p),
        ..press(buttons::SPAWN_ACCEPT)
    }
}

/// A side pick takes its row on the side screen; with the accept on the
/// same pad (a double click) the accept reads the picked row. Off its
/// screen a pick does nothing.
#[test]
fn a_side_pick_is_read_by_the_accept_on_its_own_pad() {
    use crate::sim::ScreenPick;
    let mut s = screens_sim(5, true);
    s.step(&[pick(ScreenPick::Side(2))]);
    assert_eq!(s.side_choice[0], SideChoice::Auto);
    s.step(&[pick(ScreenPick::Post(0))]);
    assert_eq!(s.side_choice[0], SideChoice::Auto, "a post is no side");
    assert_eq!(s.spawn_post_choice[0], None, "no post off the map");
    s.step(&[pick_accept(ScreenPick::Side(1))]);
    assert_eq!(s.spawn_stage[0], SpawnStage::Map);
    assert_eq!(slot0(&s).team, 1, "the picked row's team");
    // The map: a side row is ignored there, and so is the other team's post.
    s.step(&[pick(ScreenPick::Side(0))]);
    assert_eq!(s.side_choice[0], SideChoice::Team2);
    let opened = s.spawn_post_choice[0];
    s.step(&[pick(ScreenPick::Post(0))]);
    assert_eq!(
        s.spawn_post_choice[0], opened,
        "team 1's post is not team 2's"
    );
    s.step(&[pick(ScreenPick::Post(3))]);
    assert_eq!(s.spawn_post_choice[0], Some(SpawnPost::Post(3)));
    // Double click on the map goes to the class screen with that post.
    s.step(&[PadState::default()]);
    s.step(&[pick_accept(ScreenPick::Post(3))]);
    assert_eq!(s.spawn_stage[0], SpawnStage::Class);
    assert_eq!(s.spawn_post_choice[0], Some(SpawnPost::Post(3)));
    // The class screen ignores picks.
    s.step(&[pick(ScreenPick::Post(2))]);
    assert_eq!(s.spawn_post_choice[0], Some(SpawnPost::Post(3)));
}

/// The pick reaches only the slot whose pad carries it.
#[test]
fn a_pick_moves_only_its_own_slot() {
    use crate::sim::ScreenPick;
    let mut s = screens_sim(5, true);
    s.spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(60.0, 0.0, 30.0)))
        .unwrap();
    assert!(s.open_spawn_screens(1, true));
    s.step(&[PadState::default(), pick(ScreenPick::Side(1))]);
    assert_eq!(s.side_choice[0], SideChoice::Team1);
    assert_eq!(s.side_choice[1], SideChoice::Team2);
}

/// The lane's proof: a session of mouse picks recorded as an input log
/// (the picks and the double clicks' accepts only, no D-pad), written to
/// RON and read back, replays to the same hash on every tick, and spawns
/// the player on the picked team at the picked post. Without its picks
/// the same log ends elsewhere, so the picks are what the replay carries.
#[test]
fn a_recorded_mouse_pick_replays_to_the_same_hash() {
    use crate::sim::ScreenPick;
    let mut log = InputLog {
        seed: 23,
        slots: 1,
        ticks: vec![PadState::default(); 60],
    };
    // Click team 2, double click it; click post 3, double click it;
    // accept the class.
    log.ticks[3] = pick(ScreenPick::Side(1));
    log.ticks[9] = pick_accept(ScreenPick::Side(1));
    log.ticks[15] = pick(ScreenPick::Post(3));
    log.ticks[21] = pick_accept(ScreenPick::Post(3));
    log.ticks[27] = press(buttons::SPAWN_ACCEPT);
    let run = |log: &InputLog| {
        let mut s = screens_sim(log.seed, true);
        let mut hashes = Vec::new();
        for pads in log.frames() {
            s.step(pads);
            hashes.push(s.hash());
        }
        (
            hashes,
            slot0(&s).team,
            slot0(&s).alive,
            slot0(&s).soldier.pos,
        )
    };
    let live = run(&log);
    let text = ron::to_string(&log).unwrap();
    assert!(text.contains("pick:Some(Side(1))"), "{text}");
    let back: InputLog = ron::from_str(&text).unwrap();
    let replay = run(&back);
    assert_eq!(replay.0, live.0, "every tick's hash");
    assert_eq!((live.1, live.2), (1, true), "on the field for team 2");
    assert!(
        (live.3.x - -200.0).abs() < 10.0,
        "at post 3 (x = -200): {:?}",
        live.3
    );
    // Strip the picks: the accepts alone go to team 1.
    let mut bare = log.clone();
    bare.ticks.iter_mut().for_each(|p| p.pick = None);
    let without = run(&bare);
    assert_eq!(without.1, 0, "the accepts alone keep team 1");
    assert_ne!(without.0.last(), live.0.last());
    // A pad with no pick serializes as before.
    let idle = ron::to_string(&PadState::default()).unwrap();
    assert!(!idle.contains("pick"), "{idle}");
}
