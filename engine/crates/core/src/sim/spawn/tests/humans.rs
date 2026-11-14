use super::*;
use crate::sim::{Cheats, DeathCause, JoinError, MAX_PLAYER_SLOTS};

// ---- Issue #168: several humans in one sim -----------------------------

/// Two teams with a post each (team 0 at x = 0, team 1 at x = -300), two
/// classes per team (weapon 0 and 1), and two bots a side.
fn lan_sim() -> Sim {
    let mut s = Sim::empty(11);
    s.heightfield = Some(flat_ground());
    s.add_weapon_class(WeaponClass::bf2_rifle());
    for t in 0..2 {
        s.classes[t] = vec![class_w(1, None, 0), class_w(1, None, 1)];
    }
    for (x, team) in [(0.0, 0u8), (-300.0, 1)] {
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
    for (team, x) in [(0u8, 0.0), (1, -300.0)] {
        for k in 0..2 {
            let pos = Vec3::new(x + 3.0 * k as f32, 0.0, 10.0);
            s.spawn_ai(team, pos, 0.0).expect("a bot");
        }
    }
    s
}

/// One edge of `bit` on slot `slot`'s pad, then a released tick.
fn tap_slot(s: &mut Sim, slot: usize, bit: u32) {
    let mut pads = vec![PadState::default(); slot + 1];
    pads[slot].buttons = bit;
    s.step(&pads);
    s.step(&vec![PadState::default(); slot + 1]);
}

fn tickets(s: &Sim) -> [u32; 2] {
    [s.battle.teams[0].tickets(), s.battle.teams[1].tickets()]
}

/// LAN §5: slot 1's cheats are slot 1's. Slot 0 without its own set keeps
/// reading the session's (`Sim::cheats`), exactly as the single player.
#[test]
fn humans_slot_cheats_reach_only_their_own_slot() {
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat_ground());
    let p0 = s
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    let p1 = s
        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(5.0, 0.0, 0.0)))
        .unwrap();
    let god = Cheats {
        invincible: true,
        infinite_ammo: true,
        ..Cheats::default()
    };
    s.set_slot_cheats(1, Some(god));
    assert_eq!(s.cheats_for_slot(0), Cheats::default());
    assert_eq!(s.cheats_for_slot(1), god);
    assert_eq!(
        s.cheats_for_slot(7),
        s.cheats,
        "an unknown slot reads the session's"
    );
    assert_eq!(s.ammo_cheats(p0.index as usize), (false, false));
    assert_eq!(s.ammo_cheats(p1.index as usize), (true, false));
    s.kill_environment(p0, DeathCause::Water);
    s.kill_environment(p1, DeathCause::Water);
    assert!(s.unit(p0).is_none(), "slot 0 has no cheat");
    assert!(s.unit(p1).is_some(), "slot 1's invincibility spares him");

    // The session set still reaches a slot without its own, and putting
    // slot 1 back on it (None) gives him the session's again.
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat_ground());
    let p0 = s
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    let p1 = s
        .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(5.0, 0.0, 0.0)))
        .unwrap();
    s.cheats.invincible = true;
    s.set_slot_cheats(1, Some(Cheats::default()));
    s.kill_environment(p0, DeathCause::Water);
    s.kill_environment(p1, DeathCause::Water);
    assert!(s.unit(p0).is_some() && s.unit(p1).is_none());
    s.set_slot_cheats(1, None);
    assert!(s.cheats_for_slot(1).invincible);
}

/// LAN §4.1: each human's medal career is his own: progress on slot 1's
/// life moves slot 1's counts only, and his medals reach his kit only.
/// Slot 0's career hashes as the single career did; slot 1's adds bytes.
#[test]
fn humans_each_slot_keeps_its_own_medal_career() {
    let build = || {
        let mut s = Sim::empty(3);
        s.heightfield = Some(flat_ground());
        let p0 = s
            .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
            .unwrap();
        let p1 = s
            .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(5.0, 0.0, 0.0)))
            .unwrap();
        s.install_medal_career(p0, [0; 9]);
        (s, p0, p1)
    };
    let (mut s, p0, p1) = build();
    let (only0, _, _) = build();
    assert_eq!(s.hash(), only0.hash());
    let mut counts = [0; 9];
    counts[0] = 64;
    s.install_medal_career(p1, counts);
    assert_ne!(
        s.hash(),
        only0.hash(),
        "an installed second career is hashed"
    );
    assert_eq!(s.medal_counts(), Some([0; 9]));
    assert_eq!(s.slot_medal_counts(1), Some(counts));
    // Legendary Gunslinger (64) is slot 1's weapon medal, not slot 0's.
    let u0 = &s.units[p0.index as usize];
    let u1 = &s.units[p1.index as usize];
    assert_eq!(s.award_bits(u0) & 1, 0);
    assert_eq!(s.award_bits(u1) & 1, 1);
    s.score_medal_points(p1, 100);
    assert_eq!(s.medal_counts(), Some([0; 9]));
    let after = s.slot_medal_counts(1).unwrap();
    assert_eq!(&after[6..], &[1, 1, 1]);
    s.score_medal_points(p0, 100);
    assert_eq!(&s.medal_counts().unwrap()[6..], &[1, 1, 1]);
    assert_eq!(s.slot_medal_counts(1), Some(after));
    s.set_slot_career_award_bonuses(1, false);
    assert!(s.career_award_bonuses() && !s.slot_career_award_bonuses(1));
    // Dropping slot 1's career takes its bytes out of the hash again.
    let (mut t, q0, _) = build();
    t.score_medal_points(q0, 100);
    s.remove_slot_medal_career(1);
    assert_eq!(s.hash(), t.hash());
}

/// LAN §4.5-§4.6: a human joins mid-round behind the spawn screens, at no
/// ticket cost; accepting a post and a class puts him on the field; when
/// he leaves, a bot of his team takes his place where he stood, and no
/// reinforcement is spent.
#[test]
fn humans_join_mid_round_and_a_bot_takes_over_when_one_leaves() {
    let mut s = lan_sim();
    for _ in 0..60 {
        s.step(&[]);
    }
    let before = tickets(&s);
    let h = s.join_human(1, Some(0)).expect("slot 1 is free");
    assert_eq!(s.players[1], Some(h));
    assert!(s.spawn_screens);
    assert!(!s.units[h.index as usize].alive, "behind the screens");
    assert_eq!(s.units[h.index as usize].spec.player_slot, Some(1));
    assert_eq!(
        s.spawn_stage[1],
        SpawnStage::Map,
        "a given team skips the side select"
    );
    assert_eq!(s.join_human(1, None), Err(JoinError::SlotTaken));
    assert_eq!(
        s.join_human(MAX_PLAYER_SLOTS, None),
        Err(JoinError::BadSlot)
    );
    assert_eq!(s.join_human(2, Some(2)), Err(JoinError::BadTeam));
    assert_eq!(tickets(&s), before, "a join costs nothing");

    tap_slot(&mut s, 1, buttons::SPAWN_ACCEPT); // the post
    assert_eq!(s.spawn_stage[1], SpawnStage::Class);
    tap_slot(&mut s, 1, buttons::SPAWN_ACCEPT); // the class
    let human = s.players[1].unwrap();
    let u = s.unit(human).expect("on the field");
    assert!(u.is_player && u.team == 0);
    assert_eq!(s.battle.teams[0].alive_players, 1);
    // Walk a little so his spot is his own, not the spawn node.
    let mut fwd = vec![PadState::default(); 2];
    fwd[1].ly = i16::MAX;
    for _ in 0..10 {
        s.step(&fwd);
    }
    let u = s.unit(human).unwrap();
    let (pos, yaw) = (u.soldier.pos, u.soldier.yaw);
    let (ai, spent) = (s.battle.teams[0].alive_ai, tickets(&s));

    let bot = s.leave_human(1).expect("a bot takes over");
    let b = s.unit(bot).expect("on the field");
    assert!(!b.is_player && b.controller == Controller::Ai && b.team == 0);
    assert_eq!((b.soldier.pos.x, b.soldier.pos.z), (pos.x, pos.z));
    assert_eq!(b.soldier.yaw, yaw);
    assert_eq!(s.players[1], None);
    assert_eq!(s.spawn_stage[1], SpawnStage::Accepted);
    assert_eq!(tickets(&s), spent, "leaving spends no reinforcement");
    assert_eq!(s.battle.teams[0].alive_players, 0);
    assert_eq!(s.battle.teams[0].alive_ai, ai + 1);
    assert!(s.leave_human(1).is_none(), "an empty slot leaves nothing");
    for _ in 0..120 {
        s.step(&[]);
    }
    assert!(
        s.units.iter().all(|u| !(u.alive && u.is_player)),
        "the leaver never comes back"
    );
}

/// A human who leaves from behind the screens (or dead) leaves no bot, and
/// his held unit never respawns. With no team asked for, a joiner waits on
/// the side select, on the team with fewer humans.
#[test]
fn humans_leaving_from_the_screens_leaves_no_bot() {
    let mut s = lan_sim();
    let a = s.join_human(0, Some(0)).unwrap();
    let b = s.join_human(3, None).unwrap();
    assert_eq!(s.units[b.index as usize].team, 1, "team 1 had fewer humans");
    assert_eq!(s.spawn_stage[3], SpawnStage::Side);
    assert_eq!(s.players.len(), 4);
    assert!(s.target_hints.len() >= 4);
    let units = s.units.len();
    assert_eq!(s.leave_human(3), None);
    assert_eq!(s.players[3], None);
    assert_eq!(s.units.len(), units, "no bot is spawned");
    for _ in 0..200 {
        s.step(&[]);
    }
    assert!(!s.units[b.index as usize].alive || s.units[b.index as usize].gen != b.gen);
    assert_eq!(s.players[0], Some(a), "slot 0 is untouched");
}

/// With a combatant target, a joining human's reserved place sheds one
/// surplus bot of his team at once, without a ticket: the population stays.
#[test]
fn humans_a_join_keeps_the_combatant_target() {
    let mut s = lan_sim();
    s.set_combatants([3, 3]);
    assert_eq!(s.battle.teams[0].alive_ai, 2);
    let before = tickets(&s);
    s.join_human(1, Some(0)).unwrap();
    assert_eq!(s.battle.teams[0].alive_ai, 2, "room for one human");
    s.join_human(2, Some(0)).unwrap();
    assert_eq!(
        s.battle.teams[0].alive_ai, 1,
        "the second human sheds a bot"
    );
    assert_eq!(s.battle.teams[1].alive_ai, 2);
    assert_eq!(tickets(&s), before);
}
