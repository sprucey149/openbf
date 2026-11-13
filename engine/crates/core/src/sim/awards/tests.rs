use super::*;
use crate::sim::input::{buttons, PadState};
use crate::sim::tests::flat_ground;
use crate::unit::{SlotSpec, UnitHandle, UnitSpec};
use crate::weapon::WeaponClass;
use glam::Vec3;

fn class(name: &str, unlock: Option<i8>, lock: Option<i8>) -> WeaponClass {
    WeaponClass {
        name: name.into(),
        medal_unlock: unlock,
        medal_lock: lock,
        ..WeaponClass::bf1_rifle()
    }
}

fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..Default::default()
    }
}

/// A sim with BF2's rifleman kit: rifle (lock 1) in hand, pistol (lock 0),
/// award pistol (unlock 0) and award rifle (unlock 1); a player and a bot.
fn rifleman() -> (Sim, UnitHandle, UnitHandle) {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    let rifle = s.add_weapon_class(class("rifle", None, Some(1)));
    let pistol = s.add_weapon_class(class("pistol", None, Some(0)));
    let award_pistol = s.add_weapon_class(class("award_pistol", Some(0), None));
    let award_rifle = s.add_weapon_class(class("award_rifle", Some(1), None));
    let slot = |slot, weapon_class| SlotSpec {
        slot,
        channel: 0,
        weapon_class,
        clips: 4,
    };
    let kit = |spec: UnitSpec| UnitSpec {
        weapon_class: rifle,
        weapon_slot: 1,
        slots: std::sync::Arc::new([slot(2, pistol), slot(4, award_pistol), slot(5, award_rifle)]),
        ..spec
    };
    let p = s.spawn(&kit(UnitSpec::player(0, Vec3::ZERO))).unwrap();
    let b = s
        .spawn(&kit(UnitSpec::trooper(1, Vec3::new(10.0, 0.0, 0.0))))
        .unwrap();
    (s, p, b)
}

/// Make the player's class its team's hero class, as the hero manager does.
fn make_hero(s: &mut Sim, p: UnitHandle) {
    let class = s.unit(p).unwrap().spec.class;
    s.heroes = Some(crate::sim::heroes::Heroes::new([Some(class), None]));
}

/// No medal row reaches a hero: the hero manager's class, or any class in a
/// Hero Assault round, where every roster class is a hero.
#[test]
fn a_hero_never_gets_a_medal_effect() {
    use crate::sim::buffs::{GUARDIAN, GUARDIAN_MEDAL, WAR_HERO, WAR_HERO_MEDAL};
    let medals = [
        WAR_HERO_MEDAL,
        GUARDIAN_MEDAL,
        ENDURANCE_MEDAL,
        TECHNICIAN_MEDAL,
    ];
    let on = |s: &mut Sim| {
        s.cheats.damage_award = true;
        s.cheats.defense_award = true;
        s.cheats.energy_award = true;
        s.cheats.vehicle_award = true;
    };
    let (mut s, p, _) = rifleman();
    on(&mut s);
    let u = s.unit(p).unwrap();
    assert!(medals.iter().all(|&m| s.medal_effect(u, m)), "a trooper");
    s.step(&[pad(0)]);
    assert_eq!(s.unit(p).unwrap().buffs.offense, WAR_HERO);
    assert_eq!(s.unit(p).unwrap().buffs.defense, GUARDIAN);

    type Hero = fn(&mut Sim, UnitHandle);
    let paths: [(Hero, &str); 2] = [
        (make_hero, "the hero manager's class"),
        (
            |s, _| s.deathmatch = Some(crate::sim::deathmatch::Deathmatch::new(180, [8, 8])),
            "a Hero Assault class",
        ),
    ];
    for (hero, what) in paths {
        let (mut s, p, _) = rifleman();
        on(&mut s);
        hero(&mut s, p);
        let u = s.unit(p).unwrap();
        assert!(medals.iter().all(|&m| !s.medal_effect(u, m)), "{what}");
        s.step(&[pad(0)]);
        let b = &s.unit(p).unwrap().buffs;
        assert_eq!((b.dealt(), b.taken()), (1.0, 1.0), "{what}");
    }
}

fn in_hand(s: &Sim, h: UnitHandle) -> &str {
    &s.weapon_classes[s.unit(h).unwrap().weapon_class].name
}

/// The names one SWITCH press at a time reaches, starting after the one
/// in hand, until the cycle comes back to it.
fn cycle(s: &mut Sim, p: UnitHandle) -> Vec<String> {
    let start = in_hand(s, p).to_owned();
    let mut seen = Vec::new();
    for _ in 0..6 {
        s.step(&[pad(buttons::SWITCH_WEAPON)]);
        s.step(&[pad(0)]);
        let now = in_hand(s, p).to_owned();
        if now == start {
            break;
        }
        seen.push(now);
    }
    seen
}

#[test]
fn award_cheat_swaps_the_players_kit_only() {
    let (mut s, p, b) = rifleman();
    s.step(&[pad(0)]);
    assert_eq!(in_hand(&s, p), "rifle");
    assert_eq!(cycle(&mut s, p), ["pistol"], "no medals: the standard kit");

    s.cheats.all_awards = true;
    s.step(&[pad(0)]);
    assert_eq!(
        in_hand(&s, p),
        "award_rifle",
        "the rifle's award takes its place"
    );
    assert_eq!(
        cycle(&mut s, p),
        ["award_pistol"],
        "the awards replace both"
    );

    // The bot holds no medal: its kit is the standard one, the awards
    // locked for its weapon choice too.
    let bot = s.unit(b).unwrap();
    assert_eq!(in_hand(&s, b), "rifle");
    for w in &bot.held {
        let name = &s.weapon_classes[w.class].name;
        assert_eq!(
            s.weapon_locked(bot, w.class),
            name.starts_with("award"),
            "{name}"
        );
    }

    s.cheats.all_awards = false;
    s.step(&[pad(0)]);
    assert_eq!(
        in_hand(&s, p),
        "rifle",
        "the cheat off gives the rifle back"
    );
    assert_eq!(cycle(&mut s, p), ["pistol"]);
}

/// A pistol that names a lock with no award to replace it (BF2's Han
/// pistol) stays usable with the cheat on.
#[test]
fn a_lock_with_no_award_carried_keeps_the_weapon() {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    let pistol = s.add_weapon_class(class("pistol", None, Some(0)));
    let rifle = s.add_weapon_class(class("rifle", None, None));
    let p = s
        .spawn(&UnitSpec {
            weapon_class: pistol,
            weapon_slot: 1,
            slots: std::sync::Arc::new([SlotSpec {
                slot: 2,
                channel: 0,
                weapon_class: rifle,
                clips: 4,
            }]),
            ..UnitSpec::player(0, Vec3::ZERO)
        })
        .unwrap();
    s.cheats.all_awards = true;
    s.step(&[pad(0)]);
    assert_eq!(in_hand(&s, p), "pistol");
    assert!(!s.weapon_locked(s.unit(p).unwrap(), pistol));
    assert_eq!(cycle(&mut s, p), ["rifle"]);
}

/// The space modes keep their kits as they are (a landing region is only
/// authored by the space scripts).
#[test]
fn the_award_cheat_does_nothing_in_space() {
    let (mut s, p, _) = rifleman();
    s.landing_regions.push(crate::region::Region::sphere(
        Vec3::new(0.0, -500.0, 0.0),
        1.0,
    ));
    s.cheats.all_awards = true;
    s.step(&[pad(0)]);
    assert_eq!(in_hand(&s, p), "rifle");
    assert_eq!(s.award_bits(s.unit(p).unwrap()), 0);
    assert_eq!(cycle(&mut s, p), ["pistol"]);
}

/// The damage award is its own cheat: the player deals x1.5 while it is
/// on, a bot and a hero never, and nothing in space. The award-weapon cheat
/// alone leaves damage as it was.
#[test]
fn the_damage_award_cheat_gives_x1_5() {
    use crate::sim::buffs::WAR_HERO;
    let (mut s, p, b) = rifleman();
    s.step(&[pad(0)]);
    assert_eq!(s.unit(p).unwrap().buffs.dealt(), 1.0);

    s.cheats.all_awards = true;
    s.step(&[pad(0)]);
    assert_eq!(s.unit(p).unwrap().buffs.dealt(), 1.0, "weapons only");
    s.cheats.all_awards = false;

    s.cheats.damage_award = true;
    s.step(&[pad(0)]);
    assert_eq!(s.unit(p).unwrap().buffs.offense, WAR_HERO);
    assert_eq!(s.unit(p).unwrap().buffs.dealt(), 1.5);
    assert_eq!(s.buff_scale(Some(p), b), 1.5);
    assert_eq!(
        s.unit(b).unwrap().buffs.dealt(),
        1.0,
        "a bot holds no medal"
    );
    assert_eq!(
        s.unit(p).unwrap().buffs.taken(),
        1.0,
        "damage taken is as it was"
    );
    // It never runs down while the medal is held.
    for _ in 0..600 {
        s.step(&[pad(0)]);
    }
    assert_eq!(s.unit(p).unwrap().buffs.offense, WAR_HERO);

    s.cheats.damage_award = false;
    s.step(&[pad(0)]);
    assert_eq!(s.unit(p).unwrap().buffs.dealt(), 1.0, "gone with the medal");
    assert!(!s.unit(p).unwrap().buffs.active());

    // A hero gets none (the PS2 clears a hero's).
    s.cheats.damage_award = true;
    make_hero(&mut s, p);
    s.step(&[pad(0)]);
    assert_eq!(s.unit(p).unwrap().buffs.dealt(), 1.0);

    let (mut s, p, _) = rifleman();
    s.landing_regions.push(crate::region::Region::sphere(
        Vec3::new(0.0, -500.0, 0.0),
        1.0,
    ));
    s.cheats.damage_award = true;
    s.step(&[pad(0)]);
    assert_eq!(
        s.unit(p).unwrap().buffs.dealt(),
        1.0,
        "space keeps its numbers"
    );
}

/// Each award row gives its own medals and no other: "Award weapons" the
/// five weapon medals, each stat row its one. A bot holds none, and nothing
/// is held in space.
#[test]
fn each_award_row_sets_only_its_medal() {
    use crate::sim::state::Cheats;
    type Row = (fn(&mut Cheats), u16);
    let rows: [Row; 5] = [
        (|c| c.all_awards = true, 0b11_0111),
        (|c| c.damage_award = true, 1 << 8),
        (|c| c.defense_award = true, 1 << 7),
        (|c| c.energy_award = true, 1 << 6),
        (|c| c.vehicle_award = true, 1 << 3),
    ];
    for (set, bits) in rows {
        let (mut s, p, b) = rifleman();
        set(&mut s.cheats);
        assert_eq!(s.award_bits(s.unit(p).unwrap()), bits);
        assert_eq!(s.award_bits(s.unit(b).unwrap()), 0, "a bot holds none");
        s.landing_regions.push(crate::region::Region::sphere(
            Vec3::new(0.0, -500.0, 0.0),
            1.0,
        ));
        assert_eq!(s.award_bits(s.unit(p).unwrap()), 0, "none in space");
    }
    // Every row's id toggles its own field and no other.
    let ids = Cheats::default().entries().map(|e| e.0);
    for (k, id) in ids.iter().enumerate() {
        let mut c = Cheats::default();
        c.toggle(id);
        let on: Vec<bool> = c.entries().iter().map(|e| e.2).collect();
        assert!(on.iter().enumerate().all(|(j, &v)| v == (j == k)), "{id}");
    }
}

/// The defense award (Guardian) halves the damage the player takes while
/// it is on; a bot and a hero never get it, an officer's shorter Rally does
/// not replace it, and it goes with the medal.
#[test]
fn the_damage_reduction_cheat_halves_damage_taken() {
    use crate::sim::buffs::GUARDIAN;
    let (mut s, p, b) = rifleman();
    s.cheats.all_awards = true;
    s.cheats.damage_award = true;
    s.step(&[pad(0)]);
    assert_eq!(s.unit(p).unwrap().buffs.taken(), 1.0, "other rows: no");

    s.cheats.defense_award = true;
    s.step(&[pad(0)]);
    assert_eq!(s.unit(p).unwrap().buffs.defense, GUARDIAN);
    assert_eq!(s.unit(p).unwrap().buffs.taken(), 0.5);
    assert_eq!(s.buff_scale(Some(b), p), 0.5);
    assert_eq!(s.unit(b).unwrap().buffs.taken(), 1.0, "a bot holds none");

    // A 20 s Rally is shorter: Guardian stays.
    s.units[p.index as usize]
        .buffs
        .apply_multipliers((0.0, 1.0), (20.0, 0.5));
    for _ in 0..600 {
        s.step(&[pad(0)]);
    }
    assert_eq!(s.unit(p).unwrap().buffs.defense, GUARDIAN);

    s.cheats.defense_award = false;
    s.step(&[pad(0)]);
    assert_eq!(s.unit(p).unwrap().buffs.taken(), 1.0, "gone with the medal");

    // A hero gets none.
    s.cheats.defense_award = true;
    make_hero(&mut s, p);
    s.step(&[pad(0)]);
    assert_eq!(s.unit(p).unwrap().buffs.taken(), 1.0);
}

/// The energy award (Endurance) refills the player's energy at 1.5 x the
/// class rate, standing still; a drain is untouched.
#[test]
fn the_energy_regen_award_refills_energy_1_5x() {
    let gain = |on: bool| {
        let (mut s, p, _) = rifleman();
        s.cheats.energy_award = on;
        s.step(&[pad(0)]);
        let c = s.unit(p).unwrap().consts;
        s.unit_mut(p).unwrap().soldier.energy = 10.0;
        for _ in 0..30 {
            s.step(&[pad(0)]);
        }
        (
            s.unit(p).unwrap().soldier.energy - 10.0,
            c.energy_restore_idle,
        )
    };
    let (plain, idle) = gain(false);
    let (award, _) = gain(true);
    assert!((plain - idle).abs() < 0.01, "{plain} against {idle}");
    assert!((award - 1.5 * idle).abs() < 0.01, "{award} against {idle}");
    assert_eq!(super::endurance_rate(-5.0), -5.0, "a drain as it was");
    assert_eq!(super::endurance_rate(10.0), 15.0);
}

/// Difficulty never meets a medal: with every award row on, a bot and the
/// human proxy (a bot the sim treats as a human target) hold none and get
/// no effect, so the tiers are tuned with every award off.
#[test]
fn a_bot_and_the_proxy_never_hold_a_medal() {
    let (mut s, p, b) = rifleman();
    s.units[b.index as usize].proxy_human = Some(crate::ai::proxy::Strength::Good.profile());
    let c = &mut s.cheats;
    c.all_awards = true;
    c.damage_award = true;
    c.defense_award = true;
    c.energy_award = true;
    c.vehicle_award = true;
    s.step(&[pad(0)]);
    let bot = s.unit(b).unwrap();
    assert!(bot.is_human(), "the proxy counts as a human target");
    assert_eq!(s.award_bits(bot), 0);
    for medal in [3, 6, 7, 8] {
        assert!(!s.medal_effect(bot, medal), "medal {medal}");
        assert!(s.medal_effect(s.unit(p).unwrap(), medal), "medal {medal}");
    }
    assert_eq!((bot.buffs.dealt(), bot.buffs.taken()), (1.0, 1.0));
    assert!(!bot.buffs.active());
}

#[test]
fn earned_medals_unlock_at_veteran_for_one_life_and_at_legendary_for_every_life() {
    use crate::weapon::HealthType;
    let (mut s, p, bot) = rifleman();
    let mut counts = [0; 9];
    counts[1] = 3;
    counts[6] = 63;
    s.install_medal_career(p, counts);
    let class = s.unit(p).unwrap().weapon_class;
    s.weapon_classes[class].medals_type = Some(1);
    s.medal_weapon_hit(bot, class, HealthType::Person, true, false);
    for _ in 0..12 {
        s.medal_weapon_hit(p, class, HealthType::Person, false, false);
    }
    assert_eq!(s.medal_counts().unwrap(), counts);
    for _ in 0..13 {
        s.medal_weapon_hit(p, class, HealthType::Person, true, false);
    }
    assert_eq!(s.medal_counts().unwrap()[1], 4, "only one award per life");
    assert_ne!(s.award_bits(s.unit(p).unwrap()) & (1 << 1), 0);
    assert_eq!(s.award_bits(s.unit(bot).unwrap()), 0);
    s.score_medal_points(p, 8);
    assert_eq!(s.medal_counts().unwrap()[6], 63);
    s.score_medal_points(p, 1);
    assert_eq!(s.medal_counts().unwrap()[6], 64);
    s.units[p.index as usize].gen += 1;
    let next = s.handle(p.index as usize);
    assert_eq!(s.award_bits(s.unit(next).unwrap()), 1 << 6);
    s.medal_weapon_hit(p, class, HealthType::Person, true, false);
    assert_eq!(
        s.medal_counts().unwrap()[1],
        4,
        "old projectiles cannot earn in the new life"
    );
}

#[test]
fn career_award_bonuses_can_be_disabled_while_progress_continues() {
    use crate::sim::buffs::{GUARDIAN, GUARDIAN_MEDAL, WAR_HERO, WAR_HERO_MEDAL};
    use crate::weapon::HealthType;

    let (mut s, player, _) = rifleman();
    let mut counts = [0; 9];
    counts[1] = 64;
    counts[6] = 3;
    counts[usize::from(GUARDIAN_MEDAL)] = 64;
    counts[usize::from(WAR_HERO_MEDAL)] = 64;
    s.install_medal_career(player, counts);
    let rifle = s.unit(player).unwrap().weapon_class;
    s.weapon_classes[rifle].medals_type = Some(1);
    s.step(&[pad(0)]);
    let u = s.unit(player).unwrap();
    assert_eq!(s.weapon_classes[u.weapon_class].name, "award_rifle");
    assert_eq!((u.buffs.offense, u.buffs.defense), (WAR_HERO, GUARDIAN));

    s.set_career_award_bonuses(false);
    let u = s.unit(player).unwrap();
    assert_eq!(s.award_bits(u), 0);
    assert_eq!(u.weapon_class, rifle);
    assert_eq!((u.buffs.dealt(), u.buffs.taken()), (1.0, 1.0));
    for _ in 0..medal_threshold(1, counts[1]) {
        s.medal_weapon_hit(player, rifle, HealthType::Person, true, false);
    }
    s.score_medal_points(player, 12);
    counts[1] = 65;
    counts[6] = 4;
    assert_eq!(s.medal_counts(), Some(counts));
    s.step(&[pad(0)]);
    let u = s.unit(player).unwrap();
    assert_eq!(s.award_bits(u), 0);
    assert_eq!(u.weapon_class, rifle);
    assert_eq!((u.buffs.dealt(), u.buffs.taken()), (1.0, 1.0));

    s.cheats.all_awards = true;
    assert_eq!(s.award_bits(s.unit(player).unwrap()), WEAPON_MEDALS);
    s.cheats.all_awards = false;
    s.set_career_award_bonuses(true);
    let u = s.unit(player).unwrap();
    assert_eq!(s.weapon_classes[u.weapon_class].name, "award_rifle");
    assert_eq!((u.buffs.offense, u.buffs.defense), (WAR_HERO, GUARDIAN));
    assert!(s.medal_effect(u, ENDURANCE_MEDAL));
    assert_eq!(s.medal_counts(), Some(counts));
}
