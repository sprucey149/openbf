//! Card PWUP's referees (`analysis/re-powerups.md` §3.1, §3.2, §4, §5): the
//! table census, the pool cap, the item's life, who takes what.
use super::*;
use crate::lcg::GameLcg;
use crate::sim::tests::flat_ground;
use crate::sim::PadState;
use crate::TICK_HZ;

mod dispensers;

const DT: f32 = 1.0 / TICK_HZ as f32;

/// Item indices of the fixtures below.
const AMMO: u8 = 0;
const H25: u8 = 1;
const H100: u8 = 2;
const DUAL: u8 = 3;
const ENERGY: u8 = 4;
const OFFENSE: u8 = 5;
const DEFENSE: u8 = 6;

/// The disc's item classes (§2.1) for one game.
fn items(bf2: bool) -> Vec<ItemClass> {
    let (h25, h100, dual) = if bf2 {
        (100.0, 200.0, 200.0)
    } else {
        (50.0, 75.0, 100.0)
    };
    let mut v = vec![
        ItemClass::basic("com_item_powerup_ammo", 0.0, 1),
        ItemClass::basic("com_item_powerup_health25", h25, 0),
        ItemClass::basic("com_item_powerup_health100", h100, 0),
        ItemClass::basic("com_item_powerup_dual", dual, 2),
    ];
    if bf2 {
        v.push(ItemClass {
            energy: 150.0,
            ..ItemClass::basic("com_item_powerup_energy", 0.0, 0)
        });
        v.push(ItemClass {
            offense: (12.0, 1.5),
            ..ItemClass::basic("com_item_powerup_offense", 0.0, 0)
        });
        v.push(ItemClass {
            defense: (20.0, 0.5),
            ..ItemClass::basic("com_item_powerup_defense", 0.0, 0)
        });
    }
    v
}

fn sim(bf2: bool) -> Sim {
    let mut s = Sim::empty(11);
    s.heightfield = Some(flat_ground());
    s.ground_rules = if bf2 {
        GroundRules::Bf2
    } else {
        GroundRules::Bf1
    };
    for c in items(bf2) {
        s.add_item_class(c);
    }
    s
}

#[test]
fn hero_refuses_health_items_and_wastes_buffs_but_keeps_energy_and_ammo() {
    use crate::soldier::GroundRules;
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        let mut s = sim(true);
        s.ground_rules = rules;
        s.heroes = Some(crate::sim::heroes::Heroes::new([Some(1), None]));
        let h = s
            .spawn(&UnitSpec {
                class: 1,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        let ui = h.index as usize;
        s.units[ui].health.cur = 100.0;
        lay(&mut s, H100, Vec3::ZERO);
        step(&mut s, 1);
        assert_eq!(s.unit(h).unwrap().health.cur, 100.0);
        assert_eq!(
            s.pickups.len(),
            1,
            "a refused health item stays on the ground"
        );
        s.pickups.clear();
        lay(&mut s, OFFENSE, Vec3::ZERO);
        step(&mut s, 1);
        assert!(s.pickups.is_empty(), "a hero still consumes the buff item");
        assert_eq!(s.unit(h).unwrap().buffs.offense.0, 0.0);
        s.units[ui].soldier.energy = 0.0;
        lay(&mut s, ENERGY, Vec3::ZERO);
        step(&mut s, 1);
        assert!(s.pickups.is_empty());
        assert!(s.unit(h).unwrap().soldier.energy > 0.0);
        s.units[ui].weapon.spare = 0;
        lay(&mut s, AMMO, Vec3::ZERO);
        step(&mut s, 1);
        assert!(s.pickups.is_empty());
        assert!(s.unit(h).unwrap().weapon.spare > 0);
    }
}

/// A trooper that always drops `item`.
fn dropper(team: u8, pos: Vec3, item: u8) -> UnitSpec {
    UnitSpec {
        drops: DropTable::from_pairs(&[(item, 1.0)]),
        ..UnitSpec::trooper(team, pos)
    }
}

fn step(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.step(&[PadState::default()]);
    }
}

/// An item of `class` lying landed at `pos`.
fn lay(s: &mut Sim, class: u8, pos: Vec3) {
    let life = s.item_classes[usize::from(class)].life_span;
    s.pickups.push(Pickup {
        source: None,
        class,
        pos,
        vel: Vec3::ZERO,
        yaw: 0.0,
        life,
        landed: true,
    });
}

/// 10,000 deaths per table: every item's share within 0.015 of its entry.
#[test]
fn table_census_matches_the_tables() {
    let tables: [(&str, &[(u8, f32)]); 3] = [
        ("BF1 A", &[(AMMO, 0.40), (H100, 0.30), (DUAL, 0.10)]),
        ("BF1 B", &[(AMMO, 0.05), (H25, 0.10), (H100, 0.10)]),
        (
            "BF2 regular",
            &[
                (AMMO, 0.25),
                (H25, 0.15),
                (OFFENSE, 0.02),
                (DEFENSE, 0.02),
                (H100, 0.08),
                (DUAL, 0.08),
                (ENERGY, 0.10),
            ],
        ),
    ];
    for (name, pairs) in tables {
        let t = DropTable::from_pairs(pairs);
        let mut lcg = GameLcg::new(0x1234_5678);
        let mut got = [0u32; 8];
        let mut none = 0u32;
        for _ in 0..10_000 {
            match t.pick(lcg.unit()) {
                Some(i) => got[usize::from(i)] += 1,
                None => none += 1,
            }
        }
        let mut line = format!("{name}: none {:.4}", none as f32 / 1e4);
        for (item, prob) in pairs {
            let share = got[usize::from(*item)] as f32 / 1e4;
            line += &format!(", item {item} {share:.4} ({prob})");
            assert!((share - prob).abs() < 0.015, "{name} item {item}: {share}");
        }
        println!("{line}");
    }
}

#[test]
fn an_untouched_item_lies_31_seconds() {
    let mut s = sim(false);
    let h = s.spawn(&dropper(1, Vec3::ZERO, H100)).unwrap();
    s.kill_environment(h, DeathCause::DeathRegion);
    assert_eq!(s.pickups.len(), 1);
    assert!((s.pickups[0].pos.y - DROP_HEIGHT).abs() < 1e-6);
    s.units[h.index as usize].respawn_at = None;
    let mut ticks = 0;
    let mut landed_at = None;
    while !s.pickups.is_empty() {
        step(&mut s, 1);
        ticks += 1;
        if landed_at.is_none() && s.pickups.first().is_some_and(|p| p.landed) {
            landed_at = Some(ticks);
        }
        assert!(ticks < 2000);
    }
    println!("gone after {ticks} ticks, landed at tick {landed_at:?}");
    let want = (31.0 * TICK_HZ as f32) as i32;
    assert!((ticks - want).abs() <= 1, "{ticks} ticks, want {want}");
    // A 1 m fall under 9.8 m/s2 is 0.45 s.
    let landed = landed_at.unwrap() as f32 * DT;
    assert!((0.40..0.55).contains(&landed), "landed after {landed} s");
}

#[test]
fn a_full_pool_drops_nothing() {
    let mut s = sim(false);
    s.item_pool = 2;
    let hs: Vec<_> = (0..3)
        .map(|i| {
            s.spawn(&dropper(1, Vec3::new(10.0 * i as f32, 0.0, 0.0), AMMO))
                .unwrap()
        })
        .collect();
    let before = s.lcg.state();
    for h in hs {
        s.kill_environment(h, DeathCause::DeathRegion);
    }
    assert_eq!(s.pickups.len(), 2);
    // The third death still spent its roll (§3.2): three draws, two steps each.
    let mut l = GameLcg::new(before);
    for _ in 0..6 {
        l.step();
    }
    assert_eq!(s.lcg.state(), l.state());
}

#[test]
fn no_table_spends_no_roll() {
    let mut s = sim(false);
    let h = s.spawn(&UnitSpec::trooper(1, Vec3::ZERO)).unwrap();
    let before = s.lcg.state();
    s.kill_environment(h, DeathCause::DeathRegion);
    assert!(s.pickups.is_empty());
    assert_eq!(s.lcg.state(), before);
}

/// A full-health unit leaves a health item; a hurt ENEMY of the dropper
/// takes it, by each game's amount, capped.
#[test]
fn health_is_taken_only_by_a_unit_it_heals() {
    for (bf2, class, hurt_to, want) in [
        (false, H100, 100.0, 175.0),
        (false, H25, 100.0, 150.0),
        (false, DUAL, 100.0, 200.0),
        (false, DUAL, 240.0, 300.0),
        (true, H100, 50.0, 250.0),
        (true, H25, 50.0, 150.0),
        (true, DUAL, 50.0, 250.0),
        (true, H100, 200.0, 300.0),
    ] {
        let mut s = sim(bf2);
        // The dropper is team 1; the taker is team 0.
        let d = s.spawn(&dropper(1, Vec3::ZERO, class)).unwrap();
        s.kill_environment(d, DeathCause::DeathRegion);
        s.units[d.index as usize].respawn_at = None;
        let item = Vec3::new(0.0, 0.0, 0.0);
        s.pickups[0].vel = Vec3::ZERO;
        let t = s.spawn(&UnitSpec::trooper(0, item)).unwrap();
        // Full health, standing on it: left alone (run past the landing).
        step(&mut s, 30);
        assert_eq!(s.pickups.len(), 1, "a full-health unit leaves it");
        s.units[t.index as usize].health.cur = hurt_to;
        step(&mut s, 1);
        assert!(s.pickups.is_empty(), "the hurt enemy takes it");
        let got = s.units[t.index as usize].health.cur;
        assert_eq!(got, want, "bf2 {bf2} class {class}");
        assert!(s.events.iter().any(|e| matches!(
            e,
            Event::ItemTaken { unit, health: true, .. } if *unit == t
        )));
    }
}

#[test]
fn ammo_adds_a_clip_up_to_the_cap() {
    let mut s = sim(false);
    let t = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let rpc = s.weapon_classes[0].rounds_per_clip;
    let full = s.units[t.index as usize].weapon.spare;
    assert_eq!(full, 5 * rpc);
    // Full reserve: an ammo item is left.
    lay(&mut s, AMMO, Vec3::ZERO);
    step(&mut s, 1);
    assert_eq!(s.pickups.len(), 1);
    // Two and a half clips down: +1 clip.
    s.units[t.index as usize].weapon.spare = full - 2 * rpc - rpc / 2;
    step(&mut s, 1);
    assert!(s.pickups.is_empty());
    assert_eq!(s.units[t.index as usize].weapon.spare, full - rpc - rpc / 2);
    // A dual gives 2, capped.
    lay(&mut s, DUAL, Vec3::ZERO);
    step(&mut s, 1);
    assert!(s.pickups.is_empty());
    assert_eq!(s.units[t.index as usize].weapon.spare, full);
}

#[test]
fn a_partly_loaded_magazine_uses_the_same_reserve_allowance_as_a_full_one() {
    use crate::unit::SlotSpec;
    let c = WeaponClass {
        rounds_per_clip: 55,
        ..WeaponClass::bf1_rifle()
    };
    let spec = UnitSpec {
        total_clips: 5,
        slots: vec![SlotSpec {
            slot: 1,
            channel: 0,
            weapon_class: 0,
            clips: 5,
        }]
        .into(),
        ..UnitSpec::player(0, Vec3::ZERO)
    };
    // Five 55-round magazines: an empty weapon may refill 275 reserve rounds;
    // any nonempty magazine occupies one allowance, including after one shot.
    for (loaded, expected_reserve) in [(0, 275), (1, 220), (54, 220), (55, 220)] {
        let mut u = spec.build(0, std::slice::from_ref(&c));
        u.weapon.clip = loaded;
        u.weapon.spare = 0;
        u.held[0].state.clip = loaded;
        u.held[0].state.spare = 0;
        assert!(give_ammo(std::slice::from_ref(&c), &mut u, 99));
        assert_eq!(u.weapon.spare, expected_reserve, "primary loaded={loaded}");
        assert_eq!(
            u.held[0].state.spare, expected_reserve,
            "held loaded={loaded}"
        );
        assert!(!give_ammo(std::slice::from_ref(&c), &mut u, 1));
    }
}

#[test]
fn empty_primary_and_held_weapons_can_refill_their_entire_issue() {
    use crate::unit::SlotSpec;
    for clips in [1, 5, 6] {
        let c = WeaponClass {
            rounds_per_clip: 55,
            ..WeaponClass::bf1_rifle()
        };
        let spec = UnitSpec {
            total_clips: clips,
            slots: vec![SlotSpec {
                slot: 1,
                channel: 0,
                weapon_class: 0,
                clips,
            }]
            .into(),
            ..UnitSpec::player(0, Vec3::ZERO)
        };
        let mut u = spec.build(0, std::slice::from_ref(&c));
        u.weapon.clip = 0;
        u.weapon.spare = 0;
        u.held[0].state.clip = 0;
        u.held[0].state.spare = 0;
        assert!(give_ammo(std::slice::from_ref(&c), &mut u, 99));
        assert_eq!(u.weapon.rounds(&c), Some(55 * clips));
        assert_eq!(u.held[0].state.rounds(&c), Some(55 * clips));
        assert!(!give_ammo(std::slice::from_ref(&c), &mut u, 1));
        assert!(u.weapon.start_reload(&c));
        for _ in 0..47 {
            u.weapon.update(
                DT,
                &c,
                crate::weapon::FireInput::default(),
                &mut GameLcg::new(1),
                &mut Vec::new(),
            );
        }
        assert_eq!((u.weapon.clip, u.weapon.spare), (55, 55 * (clips - 1)));
        assert!(!give_ammo(std::slice::from_ref(&c), &mut u, 1));
    }
}

#[test]
fn an_item_in_flight_waits_half_a_second() {
    let mut s = sim(false);
    let t = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    s.units[t.index as usize].health.cur = 10.0;
    // In the air at the taker's chest, held up by no gravity to speak of.
    let life = s.item_classes[usize::from(H100)].life_span;
    s.pickups.push(Pickup {
        source: None,
        class: H100,
        pos: Vec3::new(0.0, 6.0, 0.0),
        vel: Vec3::ZERO,
        yaw: 0.0,
        life,
        landed: false,
    });
    let mut taken_at = None;
    for i in 1..=60 {
        // Keep it beside the body, never landed.
        if let Some(p) = s.pickups.first_mut() {
            p.pos.y = 0.8;
            p.vel = Vec3::ZERO;
        }
        step(&mut s, 1);
        if s.pickups.is_empty() {
            taken_at = Some(i);
            break;
        }
    }
    let age = taken_at.unwrap() as f32 * DT;
    println!("taken in flight at age {age:.3} s");
    assert!(age > TAKE_AGE && age < TAKE_AGE + 0.1, "{age}");
}

/// BF2 (§3.4): a victim standing still sends its item 3 m/s to its LEFT; a
/// fast one gives it its own velocity. BF1: always the victim's.
#[test]
fn the_drop_velocity_is_each_games() {
    let mut s = sim(true);
    let h = s.spawn(&dropper(1, Vec3::ZERO, AMMO)).unwrap();
    // Yaw 0 faces -Z; its left is -X.
    s.kill_environment(h, DeathCause::DeathRegion);
    assert_eq!(s.pickups[0].vel, Vec3::new(-DROP_SIDE_SPEED, 0.0, 0.0));
    let h = s.spawn(&dropper(1, Vec3::X * 50.0, AMMO)).unwrap();
    s.units[h.index as usize].soldier.vel = Vec3::new(0.0, 0.0, -5.0);
    s.kill_environment(h, DeathCause::DeathRegion);
    assert_eq!(s.pickups[1].vel, Vec3::new(0.0, 0.0, -5.0));

    let mut s = sim(false);
    let h = s.spawn(&dropper(1, Vec3::ZERO, AMMO)).unwrap();
    s.kill_environment(h, DeathCause::DeathRegion);
    assert_eq!(s.pickups[0].vel, Vec3::ZERO);
}

/// §5.7: head-on the item is taken with the feet 0.7-1.0 m from it.
#[test]
fn the_reach_head_on_is_within_the_ps2s_bracket() {
    let mut reach = None;
    for cm in (40..=140).rev() {
        let mut s = sim(false);
        // Facing -Z, the item ahead.
        let t = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
        s.units[t.index as usize].health.cur = 10.0;
        lay(&mut s, H100, Vec3::new(0.0, 0.0, -(cm as f32) / 100.0));
        step(&mut s, 1);
        if s.pickups.is_empty() {
            reach = Some(cm as f32 / 100.0);
            break;
        }
    }
    let reach = reach.unwrap();
    println!("head-on reach {reach:.2} m");
    assert!((0.7..=1.0).contains(&reach), "{reach}");
}

/// BF2 (§5.5): energy +150 to the bar; offense x1.5 for 12 s on damage
/// dealt, defense x0.5 for 20 s on damage taken; a buff item is always taken.
#[test]
fn bf2_energy_and_buffs() {
    let mut s = sim(true);
    let t = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let ti = t.index as usize;
    // Full energy: left.
    lay(&mut s, ENERGY, Vec3::ZERO);
    step(&mut s, 1);
    assert_eq!(s.pickups.len(), 1);
    s.units[ti].soldier.energy = 10.0;
    step(&mut s, 1);
    assert!(s.pickups.is_empty());
    let bar = s.units[ti].consts.energy_bar;
    assert!(s.units[ti].soldier.energy >= bar - 1.0, "capped at the bar");

    lay(&mut s, OFFENSE, Vec3::ZERO);
    lay(&mut s, DEFENSE, Vec3::ZERO);
    let before = s.hash();
    step(&mut s, 1);
    assert!(s.pickups.is_empty(), "a buff item is always taken");
    assert_eq!(s.units[ti].buffs.offense, (12.0, 1.5));
    assert_eq!(s.units[ti].buffs.defense, (20.0, 0.5));
    assert_ne!(s.hash(), before);
    let other = s.spawn(&UnitSpec::trooper(1, Vec3::X * 30.0)).unwrap();
    assert_eq!(s.buff_scale(Some(t), other), 1.5);
    assert_eq!(s.buff_scale(Some(other), t), 0.5);
    assert_eq!(s.buff_scale(None, other), 1.0);
    // The timers run down: offense gone after 12 s, defense after 20 s.
    step(&mut s, (12.0 * TICK_HZ as f32) as usize);
    assert_eq!(s.units[ti].buffs.dealt(), 1.0);
    assert_eq!(s.units[ti].buffs.taken(), 0.5);
    step(&mut s, (8.0 * TICK_HZ as f32) as usize + 1);
    assert_eq!(s.units[ti].buffs.taken(), 1.0);
    assert!(!s.units[ti].buffs.active());
}

/// A sim with no item hashes as it did before the card.
#[test]
fn an_item_is_hashed() {
    let mut s = sim(false);
    let a = s.hash();
    lay(&mut s, AMMO, Vec3::new(40.0, 0.0, 0.0));
    let b = s.hash();
    assert_ne!(a, b);
    s.pickups[0].life -= 1.0;
    assert_ne!(s.hash(), b);
}

/// The constant empty table is the default one, and drops nothing.
#[test]
fn the_empty_table_is_the_default() {
    assert_eq!(DropTable::NONE, DropTable::default());
    assert_eq!(DropTable::NONE.pick(0.5), None);
}
