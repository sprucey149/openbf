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
