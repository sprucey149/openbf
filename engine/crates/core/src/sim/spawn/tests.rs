use super::*;
use crate::cp::CpDef;
use crate::region::Region;
use crate::sim::input::PadState;
use crate::sim::state::Event;
use crate::sim::tests::{duel, fire, flat_ground, sphere_cp};
use crate::sim::InputLog;
use crate::unit::{Controller, SpawnPost, UnitSpec};
use crate::weapon::WeaponClass;
use crate::TICK_DT;
use glam::Vec2;

#[test]
fn spawn_waves_release_slots_in_order() {
    let mut w = SpawnWaves::mission_default();
    assert_eq!(w.slots(), 40);
    // Two deaths at t = 0.5 s (wave 0): slots (1,0) and (1,1).
    for _ in 0..15 {
        w.tick(TICK_DT);
    }
    let a = w.assign();
    let b = w.assign();
    assert_eq!((a, b), ((1, 0), (1, 1)));
    assert!(!w.ready(a));
    assert!((w.countdown(a) - 9.75).abs() < 0.05, "{}", w.countdown(a));
    // Advance to just past 10.25 s: slot 0 released, slot 1 not yet.
    while w.wave < 1 || w.cur_slot() < 1 {
        w.tick(TICK_DT);
    }
    assert!(w.ready(a) && !w.ready(b));
    while w.cur_slot() < 2 {
        w.tick(TICK_DT);
    }
    assert!(w.ready(b));
    // A death in wave 1 waits for wave 2; slot numbering restarted.
    let c = w.assign();
    assert_eq!(c, (2, 0));
    assert!(!w.ready(c));
}

#[test]
fn a_respawned_bot_re_derives_the_tiers_skill() {
    // D2: an AI respawn takes the CURRENT tier's skill, so a difficulty
    // change between lives reaches the bot; a player's stays 0.
    let mut s = Sim::empty(9);
    s.difficulty = crate::skill::Difficulty::Easy;
    s.heightfield = Some(flat_ground());
    s.add_cp(
        CpDef {
            name: "cp1".into(),
            pos: Vec3::ZERO,
            capture: Region::sphere(Vec3::ZERO, 8.0),
            control: None,
            value_bleed: 1,
            value_atk: [1, 1],
            value_def: [1, 1],
            spawn_nodes: vec![(Vec3::new(4.0, 0.0, 0.0), 0.0)],
            capture_time: 10.0,
            neutralize_time: 10.0,
        },
        Some(1),
    );
    let mut ai = UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0));
    ai.controller = Controller::Ai;
    let h = s.spawn(&ai).unwrap();
    assert_eq!(s.unit(h).unwrap().skill, 14);
    // Kill it, raise the tier, and let the respawn loop rebuild the slot.
    let i = h.index as usize;
    s.units[i].alive = false;
    s.units[i].respawn_at = Some(0);
    s.units[i].spawn_slot = None;
    s.difficulty = crate::skill::Difficulty::Hard;
    s.respawn_slot(i);
    assert!(s.units[i].alive, "the slot came back");
    assert_eq!(s.units[i].skill, 20, "the NEW tier's skill");
    assert_eq!(s.units[i].spec.skill, 20);
}

/// A class table entry with the given mission counts (everything else is
/// the built-in trooper's, so a class differs only in what is tested).
fn class(min: u32, max: Option<u32>) -> crate::unit::UnitClass {
    class_w(min, max, 0)
}

/// `class`, but firing weapon class `weapon_class` — what a respawn has
/// to carry over for a class change to be observable.
fn class_w(min: u32, max: Option<u32>, weapon_class: usize) -> crate::unit::UnitClass {
    crate::unit::UnitClass {
        consts: crate::soldier::SoldierConstants::default(),
        max_health: 300.0,
        health_type: crate::weapon::HealthType::Person,
        weapon_class,
        total_clips: 6,
        weapon_slot: 0,
        slots: std::sync::Arc::new([]),
        jet: false,
        capture_posts: true,
        drops: crate::sim::DropTable::default(),
        chunk: None,
        min,
        max,
        unlock_points: 0,
        droideka: None,
    }
}

/// A sim on flat ground with one post to spawn at and `table` as team 0's
/// class list.
fn class_sim(table: Vec<crate::unit::UnitClass>) -> Sim {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    s.classes[0] = table;
    s.add_cp(
        CpDef {
            name: "cp1".into(),
            pos: Vec3::ZERO,
            capture: Region::sphere(Vec3::ZERO, 8.0),
            control: None,
            value_bleed: 1,
            value_atk: [1, 1],
            value_def: [1, 1],
            spawn_nodes: vec![(Vec3::new(0.0, 0.0, 5.0), 0.0)],
            capture_time: 10.0,
            neutralize_time: 10.0,
        },
        Some(0),
    );
    s
}

/// The maintainer's class weights (2026-10-01): with nobody alive the default
/// class is twice as likely as a standard one and four times a special
/// one; one alive takes a unit off its class's weight; a class at its
/// share keeps only the floor, the same for every class; a class at its
/// authored max is out. Sixteen spawns of a BF2 table come out near
/// 5 / 3 / 3 / 3 / 1 / 1.
#[test]
fn ai_classes_are_drawn_by_share_and_by_what_is_not_yet_alive() {
    let mut s = class_sim(vec![class(0, None); 6]);
    s.ai_alive_cap = 16;
    // 4 + 2 + 2 + 2 + 1 + 1 parts of 16, plus 5 % of an even share.
    let floor = 0.05 * 16.0 / 6.0;
    let w = s.ai_class_weights(0);
    for (c, share) in [
        16.0 / 3.0,
        8.0 / 3.0,
        8.0 / 3.0,
        8.0 / 3.0,
        4.0 / 3.0,
        4.0 / 3.0,
    ]
    .into_iter()
    .enumerate()
    {
        assert!((w[c] - (share + floor)).abs() < 1e-4, "class {c}: {w:?}");
    }
    let h = s.spawn_ai(0, Vec3::new(0.0, 0.0, 5.0), 0.0).expect("spawn");
    let c = usize::from(s.units[h.index as usize].spec.class);
    let w1 = s.ai_class_weights(0);
    assert!(
        (w[c] - w1[c] - 1.0).abs() < 1e-4,
        "one alive: one unit less ({w1:?})"
    );
    let mut n = [0u32; 6];
    n[c] += 1;
    for _ in 1..16 {
        let h = s.spawn_ai(0, Vec3::new(0.0, 0.0, 5.0), 0.0).expect("spawn");
        n[usize::from(s.units[h.index as usize].spec.class)] += 1;
    }
    assert!(
        n[0] >= 4 && n[0] > n[4] && n[0] > n[5] && n[1] + n[2] + n[3] >= 6,
        "the default class leads, the standard ones follow: {n:?}"
    );
    assert!(n[4] <= 3 && n[5] <= 3, "the special ones are few: {n:?}");
    // A class at its share keeps the floor only.
    let w = s.ai_class_weights(0);
    if n[0] >= 6 {
        assert!((w[0] - floor).abs() < 1e-4, "{n:?} {w:?}");
    }
    // Over many teams the shares hold: 33 / 17 / 17 / 17 / 8 / 8 %.
    let mut total = [0u32; 6];
    for seed in 0..200 {
        let mut s = class_sim(vec![class(0, None); 6]);
        s.ai_alive_cap = 16;
        s.lcg = crate::lcg::GameLcg::new(seed * 7919 + 1);
        for _ in 0..16 {
            let h = s.spawn_ai(0, Vec3::new(0.0, 0.0, 5.0), 0.0).expect("spawn");
            total[usize::from(s.units[h.index as usize].spec.class)] += 1;
        }
    }
    let pct: Vec<f32> = total.iter().map(|&k| k as f32 / 32.0).collect();
    assert!((pct[0] - 33.3).abs() < 3.0, "default: {pct:?}");
    assert!(
        pct[1..4].iter().all(|p| (p - 16.7).abs() < 3.0),
        "standard: {pct:?}"
    );
    assert!(
        pct[4..].iter().all(|p| (p - 8.3).abs() < 3.0),
        "special: {pct:?}"
    );
    let mut s = class_sim(vec![class(0, Some(1)), class(0, None)]);
    s.ai_alive_cap = 16;
    let mut got = [0u32; 2];
    for _ in 0..8 {
        let h = s.spawn_ai(0, Vec3::new(0.0, 0.0, 5.0), 0.0).expect("spawn");
        got[usize::from(s.units[h.index as usize].spec.class)] += 1;
    }
    // [ours] (the maintainer 2026-10-29): the authored max no longer caps
    // the roll; class 0 (the default, 4 parts) fields past its max of 1.
    assert!(
        got[0] > 1,
        "the authored max does not cap the roll: {got:?}"
    );
    assert_eq!(s.ai_spawned, [8, 0]);
    // The cursor only advances on a spawn that happened.
    assert_eq!(s.spawn_ai(1, Vec3::ZERO, 0.0), None, "team 1 has no table");
    assert_eq!(s.ai_spawned, [8, 0]);
}

/// S0: `vacate_player_slot` un-spawns slot 0's idle human without
/// spending a ticket, and the bot that fills the hole is an ordinary AI
/// out of the class table — the headless referees' slot-0 rule.
#[test]
fn vacating_a_player_slot_frees_it_without_a_ticket() {
    let mut s = class_sim(vec![class(1, None), class(1, None)]);
    let node = Vec3::new(0.0, 0.0, 5.0);
    let p = s
        .spawn(&UnitSpec::player(0, node))
        .expect("the player spawns");
    s.spawn_ai(0, node, 0.0).expect("one bot beside it");
    let before = s.battle.teams[0].tickets();
    assert_eq!(
        (s.battle.teams[0].alive_players, s.battle.teams[0].alive_ai),
        (1, 1)
    );

    let (pos, yaw) = s.vacate_player_slot(0).expect("slot 0 held the player");
    assert_eq!((pos, yaw), (s.units[p.index as usize].soldier.pos, 0.0));
    // The pool is untouched — this is not a death.
    assert_eq!(s.battle.teams[0].tickets(), before);
    assert_eq!(
        (s.battle.teams[0].alive_players, s.battle.teams[0].alive_ai),
        (0, 1)
    );
    assert_eq!(s.players[0], None);
    assert!(s.unit(p).is_none(), "off the field");
    assert_eq!(s.units[p.index as usize].respawn_at, None, "and stays off");
    // Idempotent: an empty slot vacates to `None`.
    assert_eq!(s.vacate_player_slot(0), None);

    // The replacement is an ordinary bot: it reuses the freed unit slot,
    // takes the class table's next class, and is not a player.
    let h = s.spawn_ai(0, pos, yaw).expect("the filler spawns");
    assert_eq!(h.index, p.index);
    let u = &s.units[h.index as usize];
    assert!(!u.is_player && u.spec.player_slot.is_none());
    assert_eq!(u.spec.class, 1, "the table's next class, not the player's");
    assert_eq!(
        (s.battle.teams[0].alive_players, s.battle.teams[0].alive_ai),
        (0, 2)
    );
    // Stepping the sim does not resurrect the vacated player: with no
    // pad-driven unit left, `players` is empty of it and nothing respawns.
    for _ in 0..90 {
        s.step(&[PadState::default()]);
    }
    assert!(s.units.iter().all(|u| !u.is_player || !u.alive));
}

/// [ours] (the maintainer 2026-10-29): a class at or past its authored
/// `max` keeps at least the floor weight, so the roll can field more of it.
#[test]
fn a_class_at_its_max_keeps_the_floor() {
    let mut s = class_sim(vec![class(1, None), class(4, Some(1)), class(1, Some(1))]);
    let node = Vec3::new(0.0, 0.0, 5.0);
    let floor = CLASS_FLOOR * s.ai_alive_cap.max(1) as f32 / 3.0;
    let mut n = [0u32; 3];
    for _ in 0..12 {
        let h = s.spawn_ai(0, node, 0.0).expect("spawn");
        n[usize::from(s.units[h.index as usize].spec.class)] += 1;
        let w = s.ai_class_weights(0);
        assert!(w.iter().all(|&x| x >= floor), "{w:?} {n:?}");
    }
    // Two of class 2 (max 1) alive still leave it its floor.
    for u in s
        .units
        .iter_mut()
        .filter(|u| u.alive && !u.is_player)
        .take(2)
    {
        u.spec.class = 2;
    }
    assert!(s.ai_class_weights(0)[2] >= floor);
}

/// Two sims built the same way from the same table hash alike, and the
/// three new pieces of STATE all move the hash (the class tables
/// themselves are setup data and do not).
#[test]
fn the_spawn_select_state_is_part_of_the_hash() {
    let table = || vec![class(1, None), class(1, None)];
    let mut a = class_sim(table());
    let mut b = class_sim(table());
    for s in [&mut a, &mut b] {
        s.spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
            .unwrap();
        s.spawn_ai(0, Vec3::new(0.0, 0.0, 5.0), 0.0).unwrap();
    }
    assert_eq!(a.hash(), b.hash(), "same table, same spawns, same hash");

    let base = a.hash();
    a.spawn_choice[0] = 1;
    assert_ne!(a.hash(), base, "spawn_choice is hashed");
    a.spawn_choice[0] = 0;
    assert_eq!(a.hash(), base);

    a.prev_spawn_bits[0] = buttons::SPAWN_CLASS_NEXT;
    assert_ne!(a.hash(), base, "prev_spawn_bits is hashed");
    a.prev_spawn_bits[0] = 0;
    assert_eq!(a.hash(), base);

    a.ai_spawned[1] += 1;
    assert_ne!(a.hash(), base, "ai_spawned is hashed");
    a.ai_spawned[1] -= 1;
    assert_eq!(a.hash(), base);

    // A unit's CLASS is state too, even between two classes that are
    // otherwise identical.
    let i = a.units.iter().position(|u| !u.is_player).unwrap();
    a.units[i].spec.class = 1;
    assert_ne!(a.hash(), base, "UnitSpec::class is hashed");
}

/// A two-class sim with a live player in slot 0, dead and waiting.
fn spawn_select_sim() -> Sim {
    let mut s = class_sim(vec![class_w(1, None, 0), class_w(1, None, 1)]);
    s.add_weapon_class(WeaponClass::bf2_rifle());
    s.spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    s
}

fn kill_player(s: &mut Sim) {
    let i = s.players[0].unwrap().index as usize;
    s.units[i].alive = false;
    s.units[i].respawn_at = Some(u64::MAX);
}

fn press(bit: u32) -> PadState {
    PadState {
        buttons: bit,
        ..PadState::default()
    }
}

/// D5: a dead player's NEXT press steps the class ONCE however long it is
/// held; PREV wraps the other way; an ALIVE player's press does nothing.
#[test]
fn the_spawn_screen_steps_the_class_on_the_press_edge_only() {
    let mut s = spawn_select_sim();
    // Alive: the bits are ignored (and do not arm an edge for later).
    for _ in 0..5 {
        s.step(&[press(buttons::SPAWN_CLASS_NEXT)]);
    }
    assert_eq!(s.spawn_choice[0], 0, "an alive player cannot re-class");

    kill_player(&mut s);
    for _ in 0..5 {
        s.step(&[press(buttons::SPAWN_CLASS_NEXT)]);
    }
    assert_eq!(
        s.spawn_choice[0], 0,
        "a button held across the death is not a new press"
    );
    s.step(&[PadState::default()]);
    for _ in 0..5 {
        s.step(&[press(buttons::SPAWN_CLASS_NEXT)]);
    }
    assert_eq!(s.spawn_choice[0], 1, "held for five ticks = one step");
    s.step(&[PadState::default()]);
    for _ in 0..5 {
        s.step(&[press(buttons::SPAWN_CLASS_PREV)]);
    }
    assert_eq!(s.spawn_choice[0], 0);
    // PREV off 0 wraps to the last class.
    s.step(&[PadState::default()]);
    s.step(&[press(buttons::SPAWN_CLASS_PREV)]);
    assert_eq!(s.spawn_choice[0], 1);
    // Both at once: NEXT wins.
    s.step(&[PadState::default()]);
    s.step(&[press(buttons::SPAWN_CLASS_NEXT | buttons::SPAWN_CLASS_PREV)]);
    assert_eq!(s.spawn_choice[0], 0);
}

/// The choice reaches the RESPAWN: the player comes back on the chosen
/// class's weapon.
#[test]
fn the_respawn_takes_the_chosen_class() {
    let mut s = spawn_select_sim();
    let i = s.players[0].unwrap().index as usize;
    assert_eq!(s.units[i].weapon_class, 0);
    kill_player(&mut s);
    s.step(&[press(buttons::SPAWN_CLASS_NEXT)]);
    assert_eq!(s.spawn_choice[0], 1);
    // Let the respawn loop take it.
    s.units[i].respawn_at = Some(s.tick);
    s.step(&[PadState::default()]);
    let h = s.players[0].expect("still slot 0");
    let u = s.unit(h).expect("respawned");
    assert!(u.alive);
    assert_eq!(u.spec.class, 1, "rebuilt from the chosen class");
    assert_eq!(u.weapon_class, 1, "and with that class's weapon");
}

/// Slot 1's pad does not move slot 0's choice.
#[test]
fn the_choice_is_per_slot() {
    let mut s = class_sim(vec![class(1, None), class(1, None), class(1, None)]);
    for slot in 0..2u8 {
        s.spawn(&UnitSpec::player_in_slot(
            slot,
            0,
            Vec3::new(f32::from(slot) * 20.0, 0.0, 0.0),
        ))
        .unwrap();
    }
    for slot in 0..2usize {
        let i = s.players[slot].unwrap().index as usize;
        s.units[i].alive = false;
        s.units[i].respawn_at = Some(u64::MAX);
    }
    s.step(&[PadState::default(), press(buttons::SPAWN_CLASS_NEXT)]);
    assert_eq!(s.spawn_choice, [0, 1]);
    // A slot past the end of the pad slice idles (P9's rule).
    s.step(&[press(buttons::SPAWN_CLASS_NEXT)]);
    assert_eq!(s.spawn_choice, [1, 1]);
}

/// A log that presses the spawn-select bits replays byte-identically.
#[test]
fn a_spawn_select_log_replays_identically() {
    let log = InputLog {
        seed: 0x5EED_0C1A,
        slots: 1,
        ticks: (0..400)
            .map(|i| PadState {
                lx: ((i * 53) % 65536 - 32768) as i16,
                buttons: match i % 37 {
                    0..=4 => buttons::SPAWN_CLASS_NEXT,
                    9..=11 => buttons::SPAWN_CLASS_PREV,
                    20..=22 => buttons::FIRE,
                    _ => 0,
                },
                ..Default::default()
            })
            .collect(),
    };
    let run = || {
        let mut s = spawn_select_sim();
        kill_player(&mut s);
        for pads in log.frames() {
            s.step(pads);
        }
        (s.hash(), s.spawn_choice[0])
    };
    let (h1, c1) = run();
    let (h2, c2) = run();
    assert_eq!(h1, h2);
    assert_eq!(c1, c2);
    assert!(c1 == 0 || c1 == 1);
}

/// BF2's AT-AT class points (x, z, angle°), `imp_walk_atat`.
const ATAT_POINTS: [(f32, f32, f32); 6] = [
    (8.0, 0.0, 5.0),
    (8.0, 4.0, 5.0),
    (8.0, -4.0, 5.0),
    (-8.0, 0.0, 355.0),
    (-8.0, 4.0, 355.0),
    (-8.0, -4.0, 355.0),
];

/// A sim on flat ground whose vehicle class 0 is a command walker with
/// the AT-AT's six points, and a team-0 pad at `pad_pos` that builds it.
fn command_walker_sim(pad_pos: Vec3, pad_yaw: f32) -> Sim {
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::vehicle_pad::{PadClass, PadDef};
    use crate::weapon::HealthType;
    let mut s = Sim::empty(11);
    s.heightfield = Some(flat_ground());
    s.walker_params = vec![Some(crate::vehicle::walker::WalkerParams {
        spawn_points: ATAT_POINTS
            .iter()
            .map(|&(x, z, a)| (Vec3::new(x, 0.0, z), a.to_radians()))
            .collect(),
        ..crate::vehicle::walker::tests::atst()
    })];
    s.add_vehicle_pad(PadDef {
        name: "vehiclespawn_21".into(),
        pos: pad_pos,
        yaw: pad_yaw,
        cp: None,
        team: Some(0),
        spawn_time: 2.0,
        decay_time: 20.0,
        spawn_count: 1,
        classes: [
            Some(PadClass {
                landed_height: None,
                vis_box: None,
                spec: VehicleSpec {
                    kind: VehicleKind::Walker,
                    class: 0,
                    home_team: None,
                    neutral_when_empty: false,
                    spawn_pad: None,
                    pos: Vec3::ZERO,
                    yaw: 0.0,
                    max_health: 60000.0,
                    health_type: HealthType::Vehicle,
                    seats: vec![SeatKind::Pilot],
                },
                radius: 3.0,
            }),
            None,
        ],
    });
    s
}

/// Card CWSP's test vector (re-command-walker-post §10, BF2 Hoth RAM,
/// `cp_ram2.py`): the walker at (228.79, −6.00, 51.06) with right
/// (0.308, 0, 0.952) and forward (−0.952, 0, 0.308) puts point 0
/// (8, 0, 0) at (231.26, −6.00, 58.68) and point 5 (−8, 0, −4) at
/// (230.14, −6.00, 42.22); all six are its team's and nobody else's.
#[test]
fn the_ram_walker_gives_its_six_points() {
    let mut s = command_walker_sim(Vec3::ZERO, 0.0);
    s.step(&[]);
    let (h, _) = s.command_walkers(0).next().expect("the pad built it");
    let origin = Vec3::new(228.79, -6.00, 51.06);
    let (right, fwd) = (Vec3::new(0.308, 0.0, 0.952), Vec3::new(-0.952, 0.0, 0.308));
    let heading = crate::fmath::atan2(fwd.x, fwd.z);
    {
        let v = s.vehicle_mut(h).unwrap();
        v.pos = origin;
        v.rot = crate::vehicle::hover::quat_y(heading);
        assert!((v.rot * Vec3::X - right).length() < 1e-3, "the RAM frame");
    }
    let got = s.spawn_points(0);
    assert_eq!(got.len(), 6);
    for (i, &(x, z, a)) in ATAT_POINTS.iter().enumerate() {
        // The transform by the RAM rows themselves.
        let want = origin + right * x + fwd * z;
        assert!(
            (got[i].0 - want).length() < 0.01,
            "point {i}: {:?} / {want:?}",
            got[i].0
        );
        assert!((got[i].1 - (heading + a.to_radians())).abs() < 1e-5);
    }
    // The tool's own world points. The rows above are RAM's to three
    // decimals (1.0006 long), so 8 m along them is good to 0.015 m.
    let ram = [
        (0, Vec3::new(231.26, -6.00, 58.68)),
        (5, Vec3::new(230.14, -6.00, 42.22)),
    ];
    for (i, want) in ram {
        assert!(
            (got[i].0 - want).length() < 0.015,
            "RAM point {i}: {:?} / {want:?}",
            got[i].0
        );
    }
    assert_eq!(s.walker_spawn_points(0), got, "no fixed post here");
    assert!(s.spawn_points(1).is_empty(), "never the enemy's");
    // A walker of the other team gives team 0 nothing.
    s.vehicle_mut(h).unwrap().team = Some(1);
    assert!(s.spawn_points(0).is_empty());
    assert_eq!(s.spawn_points(1).len(), 6);
}

/// The post lives and dies with its walker (§0.6): destroyed, its points
/// leave the pool at once; the pad's next walker brings them back. And
/// before the pad's first walker the opening wave reads the same points
/// off the pad's pose.
#[test]
fn a_dead_walker_gives_none_and_the_rebuilt_one_gives_again() {
    let pad = Vec3::new(40.0, 0.0, -10.0);
    let mut s = command_walker_sim(pad, 0.7);
    assert!(s.spawn_points(0).is_empty(), "no walker yet");
    let opening = s.pad_walker_spawn_points(0);
    assert_eq!(opening.len(), 6);
    assert!(s.pad_walker_spawn_points(1).is_empty());
    s.step(&[]);
    let live = s.spawn_points(0);
    assert_eq!(live.len(), 6);
    for (o, l) in opening.iter().zip(&live) {
        let d = o.0 - l.0;
        assert!(d.x.abs() < 0.2 && d.z.abs() < 0.2, "{o:?} / {l:?}");
        assert!((o.1 - l.1).abs() < 0.02);
    }
    assert!(
        s.pad_walker_spawn_points(0).is_empty(),
        "the pad has built its walker"
    );
    // A respawn lands on one of the six.
    let mut ai = UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -200.0));
    ai.controller = Controller::Ai;
    let u = s.spawn(&ai).unwrap();
    let i = u.index as usize;
    s.units[i].alive = false;
    s.respawn_slot(i);
    let at = s.units[i].soldier.pos;
    assert!(s.units[i].alive);
    assert!(
        live.iter()
            .any(|p| (p.0.x - at.x).abs() < 1e-3 && (p.0.z - at.z).abs() < 1e-3),
        "{at:?}"
    );
    // Kill the walker: the points go with it, and nobody respawns.
    let (h, _) = s.command_walkers(0).next().unwrap();
    s.destroy_vehicle(h, None);
    assert!(s.spawn_points(0).is_empty(), "a dead walker has no post");
    s.units[i].alive = false;
    s.battle.teams[0].alive_ai -= 1;
    s.respawn_slot(i);
    assert!(!s.units[i].alive, "nowhere to spawn");
    // The pad rebuilds it after SpawnTime (2 s here).
    for _ in 0..90 {
        s.step(&[]);
    }
    assert_eq!(s.spawn_points(0).len(), 6, "the new walker's post");
}

/// Death → 4 s respawn at a friendly post (new generation), ticket paid;
/// no friendly post → no respawn.
#[test]
fn respawn_at_friendly_cp_on_the_next_wave() {
    let (mut s, _p, t) = duel();
    let cp = s.add_cp(
        sphere_cp("cp2", Vec3::new(60.0, 0.0, -60.0), 8.0, false),
        Some(1),
    );
    let mut death_tick = None;
    for _ in 0..90 {
        s.step(&[fire()]);
        if s.events.iter().any(|e| matches!(e, Event::Kill { .. })) {
            death_tick = Some(s.tick);
            break;
        }
    }
    let d = death_tick.unwrap();
    assert_eq!(s.battle.teams[1].alive(), 0);
    let mut respawn_tick = None;
    for _ in 0..400 {
        s.step(&[PadState::default()]);
        if s.events.iter().any(|e| matches!(e, Event::Respawn { .. })) {
            respawn_tick = Some(s.tick);
            break;
        }
    }
    let r = respawn_tick.expect("respawned");
    // 4 s member timer, then the next wave: wave 1 opens at 10.0 s and
    // slot 0 passes at 10.25 s → tick 308 whatever the death tick.
    assert!(r - d >= 120, "member timer: {} ticks", r - d);
    assert!(
        (307..=309).contains(&r),
        "next-wave slot 0 at ~10.25 s: tick {r}"
    );
    let h2 = s.handle(t.index as usize);
    assert_ne!(h2.gen, t.gen);
    let u = s.unit(h2).unwrap();
    assert!(
        (u.soldier.pos - s.cps[cp].def.pos).length() < 5.0,
        "at the post's node"
    );
    assert_eq!(u.health.cur, 300.0);
    assert_eq!(u.deaths, 1);
    assert_eq!(s.battle.teams[1].alive(), 1);
    assert_eq!(s.battle.teams[1].tickets(), 249);

    // Without a friendly post the dead stay dead.
    let (mut s2, _, t2) = duel();
    for _ in 0..300 {
        s2.step(&[fire()]);
    }
    assert!(s2.unit(t2).is_none());
    assert!(s2.units[t2.index as usize].respawn_at.is_some());
    assert!(s2
        .events
        .iter()
        .all(|e| !matches!(e, Event::Respawn { .. })));
}

/// Slot reuse + generation guard: a handle to a dead unit never resolves
/// to its replacement.
#[test]
fn dead_handles_never_alias_new_units() {
    let (mut s, _p, t) = duel();
    for _ in 0..90 {
        s.step(&[fire()]);
        if s.unit(t).is_none() {
            break;
        }
    }
    assert!(s.unit(t).is_none());
    let t2 = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(5.0, 0.0, -30.0)))
        .unwrap();
    assert_eq!(t2.index, t.index, "slot reused");
    assert_ne!(t2.gen, t.gen);
    assert!(s.unit(t).is_none());
    assert!(s.unit(t2).is_some());
}

/// A closed box of triangles on the ground: a solid prop as the bake
/// gives it — walls and a top, nothing inside.
fn shell(c: Vec3, half: Vec3) -> crate::collision::Body {
    let v = |i: u32| {
        let s = |b: u32| if i & b == 0 { -1.0 } else { 1.0 };
        [
            s(1) * half.x,
            if i & 2 == 0 { 0.0 } else { 2.0 * half.y },
            s(4) * half.z,
        ]
    };
    let quads = [
        [0, 1, 3, 2],
        [4, 6, 7, 5],
        [0, 4, 5, 1],
        [2, 3, 7, 6],
        [0, 2, 6, 4],
        [1, 5, 7, 3],
    ];
    let idx: Vec<u32> = quads
        .iter()
        .flat_map(|q| [q[0], q[1], q[2], q[0], q[2], q[3]])
        .collect();
    let pos: Vec<[f32; 3]> = (0..8).map(v).collect();
    crate::collision::Body::new(
        glam::Mat4::from_translation(c),
        crate::collision::Shape::Mesh(crate::collision::trimesh(&pos, &idx).expect("a mesh")),
    )
}

fn wall(c: Vec3, half: Vec3) -> crate::collision::Body {
    crate::collision::Body::new(
        glam::Mat4::from_translation(c),
        crate::collision::Shape::Box { half },
    )
}

/// A post at the origin whose one spawn node is `node`.
fn node_post(s: &mut Sim, node: Vec3) {
    let mut def = sphere_cp("cp1", Vec3::ZERO, 6.0, false);
    def.spawn_nodes = vec![(node, 0.0)];
    s.add_cp(def, Some(0));
}

/// Card LLEG: a post's spawn node inside a solid body (BF1 Naboo Plains'
/// cp3 node in its fallen column, 3 m thick) spawns its unit ON the body,
/// as the PS2 does (y -104.24 over a node at -108.5), not on the ground
/// inside the mesh, where it walked the walls until it was given up.
#[test]
fn a_spawn_node_inside_a_solid_body_spawns_on_top_of_it() {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    s.collision
        .push_body(shell(Vec3::new(20.0, 0.0, 0.0), Vec3::new(1.5, 1.5, 4.5)));
    let node = Vec3::new(20.0, 0.0, 3.5);
    // The pen: the ground under the node is inside the walls.
    assert!(s.ground_under(node, false).abs() < 0.05);
    assert!(s.floor_reach(node, PEN_CELLS) < 40, "a pen");
    node_post(&mut s, node);
    assert_eq!(s.spawn_seats.len(), 1);
    assert!((s.spawn_ground(node) - 3.0).abs() < 0.05, "on the body");
    let h = s
        .spawn(&crate::unit::UnitSpec::trooper(0, node))
        .expect("spawned");
    let y = s.unit(h).expect("alive").soldier.pos.y;
    assert!((y - 3.0).abs() < 0.05, "the unit stands on it: {y}");
    // Only a post's own node: a point beside it is seated as before.
    assert!(s.spawn_ground(node + Vec3::X * 0.5).abs() < 0.05);
}

/// Card LLEG: a node in a room with a doorway, a node under a bare roof
/// and a node on a low crate are not penned: each keeps its floor.
#[test]
fn a_spawn_node_with_a_way_out_keeps_its_floor() {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    // A room 6 m x 6 m, walls 3 m, a roof, a 2 m doorway in its +x wall.
    let c = Vec3::new(40.0, 0.0, 0.0);
    let side = |dx: f32, dz: f32, hx: f32, hz: f32| {
        wall(c + Vec3::new(dx, 1.5, dz), Vec3::new(hx, 1.5, hz))
    };
    for b in [
        side(-3.0, 0.0, 0.2, 3.2),
        side(0.0, 3.0, 3.2, 0.2),
        side(0.0, -3.0, 3.2, 0.2),
        side(3.0, 2.0, 0.2, 1.0),
        side(3.0, -2.0, 0.2, 1.0),
        wall(c + Vec3::Y * 3.2, Vec3::new(3.2, 0.2, 3.2)),
        // A bare roof over open ground.
        wall(Vec3::new(60.0, 3.2, 0.0), Vec3::new(3.0, 0.2, 3.0)),
    ] {
        s.collision.push_body(b);
    }
    // A crate 1.2 m high, the node on its top.
    s.collision
        .push_body(shell(Vec3::new(80.0, 0.0, 0.0), Vec3::new(1.0, 0.6, 1.0)));
    assert!(s.floor_reach(c, PEN_CELLS) >= PEN_CELLS, "out by the door");
    for (node, floor) in [
        (c, 0.0),
        (Vec3::new(60.0, 0.0, 0.0), 0.0),
        (Vec3::new(80.0, 1.2, 0.0), 1.2),
    ] {
        node_post(&mut s, node);
        assert!((s.spawn_ground(node) - floor).abs() < 0.05, "{node}");
    }
    assert!(s.spawn_seats.is_empty(), "{:?}", s.spawn_seats);
}

/// A sim on flat ground with three team-0 posts of 1, 2 and 6 spawn
/// nodes (post k's nodes at x = 100 k, z = the node's number).
fn three_post_sim() -> Sim {
    let mut s = Sim::empty(23);
    s.heightfield = Some(flat_ground());
    for (k, n) in [1usize, 2, 6].into_iter().enumerate() {
        let pos = Vec3::new(100.0 * k as f32, 0.0, 0.0);
        s.add_cp(
            CpDef {
                name: format!("cp{k}"),
                pos,
                capture: Region::sphere(pos, 8.0),
                control: None,
                value_bleed: 1,
                value_atk: [1, 1],
                value_def: [1, 1],
                spawn_nodes: (0..n)
                    .map(|j| (pos + Vec3::new(0.0, 0.0, j as f32), 0.0))
                    .collect(),
                capture_time: 10.0,
                neutralize_time: 10.0,
            },
            Some(0),
        );
    }
    s
}

/// Unit `i` dies and comes back `n` times; the spawns per post (by the
/// node it stands on) and, for the six-node post, per node.
fn respawn_shares(s: &mut Sim, i: usize, n: u32) -> ([u32; 3], [u32; 6]) {
    let (mut posts, mut nodes) = ([0u32; 3], [0u32; 6]);
    let is_player = s.units[i].is_player;
    for _ in 0..n {
        s.units[i].alive = false;
        let t = &mut s.battle.teams[0];
        if is_player {
            t.alive_players -= 1;
        } else {
            t.alive_ai -= 1;
        }
        s.respawn_slot(i);
        assert!(s.units[i].alive);
        let at = s.units[i].soldier.pos;
        let k = (at.x / 100.0).round() as usize;
        posts[k] += 1;
        if k == 2 {
            nodes[at.z.round() as usize] += 1;
        }
    }
    (posts, nodes)
}

/// Card BSPN (re-command-walker-post §6 [O]): a bot draws a post, then
/// one of THAT post's nodes — posts of 1, 2 and 6 nodes take a third of
/// the spawns each (a flat draw over the nine nodes gives 11 / 22 /
/// 67 %), and the six nodes of the big post a sixth of its spawns each.
#[test]
fn a_bot_draws_a_post_then_one_of_its_nodes() {
    let mut s = three_post_sim();
    let mut ai = UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -200.0));
    ai.controller = Controller::Ai;
    let i = s.spawn(&ai).unwrap().index as usize;
    let (posts, nodes) = respawn_shares(&mut s, i, 3000);
    for (k, &n) in posts.iter().enumerate() {
        assert!((900..=1100).contains(&n), "post {k}: {posts:?}");
    }
    for (j, &n) in nodes.iter().enumerate() {
        let want = posts[2] as f32 / 6.0;
        assert!((n as f32 - want).abs() < 0.3 * want, "node {j}: {nodes:?}");
    }
    assert_eq!(s.units[i].spawn_post, None, "a live bot holds no choice");
}

/// BF2's `AISpawnWeight`: a post of weight 1000 among two of 100 takes
/// 1000 / 1200 of the spawns (the disc's one case, KAS2's `baserush`);
/// a post of weight 0 takes none.
#[test]
fn the_post_draw_goes_by_ai_spawn_weight() {
    let mut s = three_post_sim();
    s.cps[0].ai_spawn_weight = 1000.0;
    let mut ai = UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -200.0));
    ai.controller = Controller::Ai;
    let i = s.spawn(&ai).unwrap().index as usize;
    let (posts, _) = respawn_shares(&mut s, i, 3000);
    assert!((2400..=2600).contains(&posts[0]), "{posts:?}");
    assert!(posts[1] > 180 && posts[2] > 180, "{posts:?}");
    s.cps[0].ai_spawn_weight = 0.0;
    let (posts, _) = respawn_shares(&mut s, i, 600);
    assert_eq!(posts[0], 0, "{posts:?}");
    assert!(posts[1] > 240 && posts[2] > 240, "{posts:?}");
}

/// The draw is kept from the end of the respawn delay until the spawn,
/// and made again only when the team can no longer spawn there: the post
/// captured, or the command walker destroyed.
#[test]
fn a_bots_post_is_kept_until_it_stops_being_eligible() {
    let mut s = three_post_sim();
    let mut ai = UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -200.0));
    ai.controller = Controller::Ai;
    let i = s.spawn(&ai).unwrap().index as usize;
    s.units[i].alive = false;
    let first = s.keep_or_draw_spawn_post(i).expect("three posts");
    let lcg = s.lcg.state();
    for _ in 0..50 {
        assert_eq!(s.keep_or_draw_spawn_post(i), Some(first));
    }
    assert_eq!(s.lcg.state(), lcg, "a kept post costs no draw");
    // The choice decides where the next life starts: hashed state.
    let held = s.hash();
    s.units[i].spawn_post = None;
    assert_ne!(s.hash(), held, "the kept post is part of the hash");
    s.units[i].spawn_post = Some(first);
    assert_eq!(s.hash(), held);
    // The enemy takes it: the next ask draws one of the other two.
    let SpawnPost::Post(k) = first else {
        panic!("no walker here")
    };
    s.cps[usize::from(k)].owner = Some(1);
    let second = s.keep_or_draw_spawn_post(i).expect("two posts left");
    assert_ne!(second, first);
    assert!(s.spawn_posts(0).iter().any(|p| p.0 == second));
    // And the spawn is at the kept post's nodes.
    s.battle.teams[0].alive_ai -= 1;
    s.respawn_slot(i);
    let SpawnPost::Post(k2) = second else {
        panic!("no walker here")
    };
    let at = s.units[i].soldier.pos;
    assert_eq!((at.x / 100.0).round() as u16, k2, "{at:?}");

    // A command walker is a post like any other; dead, it is drawn over.
    let mut s = command_walker_sim(Vec3::new(40.0, 0.0, -10.0), 0.7);
    s.step(&[]);
    let (h, _) = s.command_walkers(0).next().expect("the pad built it");
    let i = s.spawn(&ai).unwrap().index as usize;
    s.units[i].alive = false;
    assert_eq!(s.keep_or_draw_spawn_post(i), Some(SpawnPost::Walker(h)));
    assert_eq!(s.spawn_post_nodes(0, SpawnPost::Walker(h)).len(), 6);
    assert!(s.spawn_post_nodes(1, SpawnPost::Walker(h)).is_empty());
    s.destroy_vehicle(h, None);
    assert_eq!(s.keep_or_draw_spawn_post(i), None, "nowhere left");
    assert_eq!(s.units[i].spawn_post, None);
}

/// A human who never picked a post on the spawn map (card SPSEL) draws
/// one node from the flat list of every post's, as before card BSPN, so
/// the 1 / 2 / 6-node posts take 1/9, 2/9 and 6/9.
#[test]
fn a_human_still_draws_from_the_flat_node_list() {
    let mut s = three_post_sim();
    let me = UnitSpec::player(0, Vec3::new(0.0, 0.0, -200.0));
    let i = s.spawn(&me).unwrap().index as usize;
    let (posts, _) = respawn_shares(&mut s, i, 2700);
    for (k, want) in [300u32, 600, 1800].into_iter().enumerate() {
        let d = posts[k].abs_diff(want);
        assert!(d < want / 5, "post {k}: {posts:?}");
    }
    assert_eq!(s.units[i].spawn_post, None);
}

// ---- card SPSEL: the spawn map's post pick ----

fn at(x: f32, z: f32) -> Vec3 {
    Vec3::new(x, 0.0, z)
}

const RIGHT: Vec2 = Vec2::new(1.0, 0.0);
const LEFT: Vec2 = Vec2::new(-1.0, 0.0);
const DOWN: Vec2 = Vec2::new(0.0, 1.0);
const UP: Vec2 = Vec2::new(0.0, -1.0);

/// The PS2's pick (re-command-walker-post §5): four posts on a plane,
/// each of the four pushes from post 0 takes the post the formula gives;
/// `along + 0.25 × perp²` lets a near post off the axis beat a far one on
/// it, and loses to it once the offset grows; nothing ahead = `None`.
#[test]
fn the_spawn_map_pick_is_along_plus_a_quarter_perp_squared() {
    let p = |i: u16| SpawnPost::Post(i);
    let posts = [
        (p(0), at(0.0, 0.0)),
        (p(1), at(100.0, 10.0)),
        (p(2), at(-60.0, 50.0)),
        (p(3), at(20.0, -90.0)),
    ];
    let pick = |d| Sim::pick_spawn_post(&posts, Some(p(0)), at(0.0, 0.0), d);
    // Right: 1 = 100 + 25 against 3 = 20 + 2025.
    assert_eq!(pick(RIGHT), Some(p(1)));
    assert_eq!(pick(LEFT), Some(p(2)));
    // Down: 2 = 50 + 900 against 1 = 10 + 2500.
    assert_eq!(pick(DOWN), Some(p(2)));
    assert_eq!(pick(UP), Some(p(3)));
    // From post 2 nothing lies further left: the caller keeps 2.
    assert_eq!(
        Sim::pick_spawn_post(&posts, Some(p(2)), at(-60.0, 50.0), LEFT),
        None
    );
    // A zero push picks nothing.
    assert_eq!(pick(Vec2::ZERO), None);
    // The quarter: (40, 25) scores 40 + 156.25 and beats (200, 0); at
    // (40, 30) it scores 265 and loses.
    for (z, want) in [(25.0, 2), (30.0, 1)] {
        let posts = [
            (p(0), at(0.0, 0.0)),
            (p(1), at(200.0, 0.0)),
            (p(2), at(40.0, z)),
        ];
        let got = Sim::pick_spawn_post(&posts, Some(p(0)), at(0.0, 0.0), RIGHT);
        assert_eq!(got, Some(p(want)), "z {z}");
    }
}

/// The live BF2 Hoth spawn map (`lead_tmp/cwsp/ps2/c2_s*.p2s`, the
/// Empire's posts read by `cp_ram2.py`, the AT-ATs at their live position
/// in the state BEFORE each press): from cp6 (list index 3) the presses
/// Right Right Right Down Down Left left the human member's chosen post
/// at 7 7 7 8 4 4. Right = +x, Down = +z; post 0 (no SpawnPath, no class
/// points) is not a spawn post and is not in the list.
#[test]
fn the_pick_replays_the_live_bf2_hoth_spawn_map() {
    let p = |i: u16| SpawnPost::Post(i);
    // (walker 7, walker 8) per state s00..s05.
    let walkers = [
        ((179.01, 63.19), (144.35, 121.01)),
        ((136.37, 72.27), (105.64, 133.94)),
        ((86.56, 82.76), (56.19, 143.30)),
        ((36.37, 93.32), (7.25, 149.62)),
        ((17.19, 97.36), (-27.09, 152.20)),
        ((-8.46, 102.75), (-53.78, 153.65)),
    ];
    let presses = [RIGHT, RIGHT, RIGHT, DOWN, DOWN, LEFT];
    let want = [7u16, 7, 7, 8, 4, 4];
    let mut cur = p(3);
    for (k, ((w7, w8), d)) in walkers.iter().zip(presses).enumerate() {
        let posts = [
            (p(3), at(16.3, -15.8)),
            (p(4), at(-213.0, 192.0)),
            (p(7), at(w7.0, w7.1)),
            (p(8), at(w8.0, w8.1)),
        ];
        let origin = posts.iter().find(|q| q.0 == cur).unwrap().1;
        cur = Sim::pick_spawn_post(&posts, Some(cur), origin, d).unwrap_or(cur);
        assert_eq!(cur, p(want[k]), "press {k}");
    }
}

fn dead_player_on_three_posts() -> (Sim, usize) {
    let mut s = three_post_sim();
    let i = s
        .spawn(&UnitSpec::player(0, Vec3::new(-150.0, 0.0, 0.0)))
        .unwrap()
        .index as usize;
    (s, i)
}

/// The post bits move the choice only while the player is dead, once per
/// press; the first push starts from the post nearest the body; a push
/// with nothing ahead keeps the selection.
#[test]
fn the_post_bits_move_the_choice_while_dead_on_the_edge() {
    let (mut s, i) = dead_player_on_three_posts();
    let p = |k: u16| Some(SpawnPost::Post(k));
    s.step(&[press(buttons::SPAWN_POST_RIGHT)]);
    assert_eq!(s.spawn_post_choice[0], None, "alive: no spawn map");
    s.units[i].alive = false;
    s.units[i].respawn_at = Some(u64::MAX);
    // Held from before the death: no edge.
    s.step(&[press(buttons::SPAWN_POST_RIGHT)]);
    assert_eq!(s.spawn_post_choice[0], None);
    s.step(&[PadState::default()]);
    // Body at x = -150: nearest post 0, Right takes post 1.
    for _ in 0..5 {
        s.step(&[press(buttons::SPAWN_POST_RIGHT)]);
    }
    assert_eq!(s.spawn_post_choice[0], p(1), "held = one step");
    s.step(&[PadState::default()]);
    s.step(&[press(buttons::SPAWN_POST_RIGHT)]);
    assert_eq!(s.spawn_post_choice[0], p(2));
    s.step(&[PadState::default()]);
    s.step(&[press(buttons::SPAWN_POST_RIGHT)]);
    assert_eq!(s.spawn_post_choice[0], p(2), "nothing further right");
    s.step(&[PadState::default()]);
    s.step(&[press(buttons::SPAWN_POST_LEFT)]);
    assert_eq!(s.spawn_post_choice[0], p(1));
    assert_eq!(s.spawn_post_selected(0), Some(SpawnPost::Post(1)));
    // Up / down: every post is on z = 0, nothing ahead, kept.
    s.step(&[PadState::default()]);
    s.step(&[press(buttons::SPAWN_POST_DOWN)]);
    assert_eq!(s.spawn_post_choice[0], p(1));
}

/// A dead player with post K chosen comes back on a node of post K, for
/// each of the three posts; when K is taken before the respawn, at the
/// eligible post nearest K, and that becomes the choice.
#[test]
fn the_respawn_takes_the_chosen_post() {
    for k in 0..3u16 {
        let (mut s, i) = dead_player_on_three_posts();
        s.spawn_post_choice[0] = Some(SpawnPost::Post(k));
        let (posts, _) = respawn_shares(&mut s, i, 60);
        let mut want = [0u32; 3];
        want[usize::from(k)] = 60;
        assert_eq!(posts, want, "chosen post {k}");
    }
    let (mut s, i) = dead_player_on_three_posts();
    s.spawn_post_choice[0] = Some(SpawnPost::Post(2));
    s.cps[2].owner = Some(1);
    let (posts, _) = respawn_shares(&mut s, i, 30);
    assert_eq!(posts, [0, 30, 0], "post 2 lost: the nearest, post 1");
    assert_eq!(s.spawn_post_choice[0], Some(SpawnPost::Post(1)));
}

/// The choice is hashed once it leaves `None` (and not before, so the
/// pinned hashes of players who never touch the spawn map hold); a pad
/// log with two post changes replays to the same hash, and differs from
/// the same log without them.
#[test]
fn the_post_choice_is_hashed_and_replays() {
    let (mut s, _) = dead_player_on_three_posts();
    let base = s.hash();
    s.spawn_post_choice[0] = Some(SpawnPost::Post(1));
    let one = s.hash();
    assert_ne!(one, base, "a pick is hashed");
    s.spawn_post_choice[0] = Some(SpawnPost::Post(2));
    let two = s.hash();
    assert_ne!(two, base);
    assert_ne!(two, one, "and which post it is");
    s.spawn_post_choice[0] = None;
    assert_eq!(s.hash(), base, "no pick, nothing eaten");

    let run = |log: &InputLog| {
        let (mut s, i) = dead_player_on_three_posts();
        s.units[i].alive = false;
        s.units[i].respawn_at = Some(40);
        s.battle.teams[0].alive_players -= 1;
        for pads in log.frames() {
            s.step(pads);
        }
        (s.hash(), s.units[i].soldier.pos)
    };
    let mut log = InputLog {
        seed: 23,
        slots: 1,
        ticks: vec![PadState::default(); 60],
    };
    let idle = run(&log);
    log.ticks[5] = press(buttons::SPAWN_POST_RIGHT);
    log.ticks[9] = press(buttons::SPAWN_POST_RIGHT);
    let a = run(&log);
    assert_eq!(a, run(&log), "replays");
    assert_ne!(a.0, idle.0);
    assert_eq!((a.1.x / 100.0).round(), 2.0, "spawned at post 2: {:?}", a.1);
}

mod class_change;
mod screens;
mod walker_placement;

#[test]
fn bots_retry_an_authored_node_when_the_picked_spawn_has_no_walkable_exit() {
    use crate::nav::{Arc, Hub, NavGraph, RouteTable};

    let mut sim = Sim::empty(7);
    sim.heightfield = Some(flat_ground());
    let shelf = Vec3::ZERO;
    let exit = Vec3::X * 16.0;
    let approach = Vec3::X * 32.0;
    sim.nav = NavGraph::new(
        [shelf, exit, approach]
            .into_iter()
            .map(|pos| Hub {
                pos,
                radius: 3.0,
                arcs: vec![],
            })
            .collect(),
        [(0, 1), (1, 2)]
            .into_iter()
            .map(|(a, b)| Arc {
                a,
                b,
                type_mask: 1,
                flags: 0,
            })
            .collect(),
        RouteTable::empty(3),
        vec![],
        vec![],
    );
    let cp = sim.add_cp(sphere_cp("shelf", shelf, 3.0, false), Some(0));
    let other = sim.add_cp(sphere_cp("other shelf", approach, 3.0, false), Some(0));
    let draw = sim.lcg.clone().index(2) as usize;
    let mut points = vec![(shelf, 0.0), (exit, 1.0)];
    points.swap(0, draw);
    sim.cps[cp].def.spawn_nodes = points;
    sim.nav_floors = vec![0.0; 3];
    sim.nav_unwalkable = vec![3, 0];
    sim.nav_trapped = vec![true, false, false];

    let mut spec = UnitSpec::trooper(0, shelf);
    spec.controller = Controller::Ai;
    let bot = sim.spawn(&spec).unwrap();
    assert_eq!(sim.unit(bot).unwrap().soldier.pos, exit);
    assert_eq!(sim.unit(bot).unwrap().soldier.yaw, 1.0);

    let player = sim.spawn(&UnitSpec::player(0, shelf)).unwrap();
    assert_eq!(sim.unit(player).unwrap().soldier.pos, shelf);
    let mut slotted = UnitSpec::player_in_slot(1, 0, shelf);
    slotted.controller = Controller::Ai;
    let player = sim.spawn(&slotted).unwrap();
    assert_eq!(sim.unit(player).unwrap().soldier.pos, shelf);

    let i = bot.index as usize;
    sim.units[i].alive = false;
    sim.battle.teams[0].alive_ai -= 1;
    sim.respawn_slot(i);
    assert_eq!(sim.units[i].soldier.pos, exit);
    assert_eq!(sim.units[i].gen, bot.gen + 1);

    sim.death_regions.push(Region::sphere(exit, 0.5));
    let retained = sim.spawn(&spec).unwrap();
    assert_eq!(sim.unit(retained).unwrap().soldier.pos, shelf);
    sim.death_regions.pop();

    sim.cps[cp].def.spawn_nodes = vec![(shelf, 0.0)];
    let retained = sim.spawn(&spec).unwrap();
    assert_eq!(sim.unit(retained).unwrap().soldier.pos, shelf);

    sim.cps[cp].def.spawn_nodes.push((exit, 1.0));
    let mut points = vec![(shelf, 0.0), (approach, 2.0)];
    let mut draw = sim.lcg.clone();
    points.swap(0, draw.index(2) as usize);
    sim.cps[other].def.spawn_nodes = points;
    let ambiguous = sim.spawn(&spec).unwrap();
    assert_eq!(sim.unit(ambiguous).unwrap().soldier.pos, shelf);
    let i = ambiguous.index as usize;
    sim.units[i].alive = false;
    sim.units[i].spawn_post = Some(SpawnPost::Post(other as u16));
    sim.battle.teams[0].alive_ai -= 1;
    sim.respawn_slot(i);
    assert_eq!(sim.units[i].soldier.pos, approach);
    assert_eq!(sim.lcg.state(), draw.state(), "one original draw");
}
