//! BF2's award weapons on both games' classes (`analysis/award-weapons.md`):
//! each keeps BF2's advantage over the weapon it stands for on the shared
//! damage scale, BF1 classes carry them, and the award cheat puts them in
//! the player's hands on a BF1 kit. Real classes from the bake; each case
//! skips without it.

use core::heightfield::Heightfield;
use core::sim::{buttons, Event, PadState, Sim};
use core::unit::{Controller, UnitHandle};
use core::weapon::{HealthType, WeaponClass};
use game::classdb::ClassDb;
use game::roster::awards::{bf1_award, BF1_AWARDS};
use game::roster::{load_classdb, ClassRoster, FACTION_ORDER};
use glam::Vec3;

fn db(game: &str) -> Option<ClassDb> {
    let dir = game::assets::baked_root().join("classes");
    if !dir.join("bf1_all.ron").is_file() || !dir.join("bf2_all.ron").is_file() {
        eprintln!("SKIP: class bakes absent");
        return None;
    }
    Some(load_classdb(&dir, game, &FACTION_ORDER).expect("class db"))
}

fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..Default::default()
    }
}

/// What one round takes off a soldier's body.
fn to_person(w: &WeaponClass) -> f32 {
    w.ordnance.max_damage * w.ordnance.scales[HealthType::Person as usize]
}

/// Every weapon a class can hold, the primary first.
fn kit(r: &ClassRoster) -> Vec<&WeaponClass> {
    std::iter::once(&r.weapon)
        .chain(r.held.iter().map(|h| &h.weapon))
        .collect()
}

/// The award for medal `t` in a kit and the weapon it replaces.
fn pair<'a>(kit: &[&'a WeaponClass], t: i8) -> Option<(&'a WeaponClass, &'a WeaponClass)> {
    let award = kit.iter().find(|w| w.medal_unlock == Some(t))?;
    let base = kit.iter().find(|w| w.medal_lock == Some(t))?;
    Some((award, base))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-4 * b.abs().max(1.0)
}

/// Award damage to a person over its base weapon's, per medal: BF2's own
/// class data, and the same on a BF1 class on the shared scale.
#[test]
fn award_damage_keeps_bf2s_advantage_over_its_base() {
    let (Some(bf1), Some(bf2)) = (db("bf1"), db("bf2")) else {
        return;
    };
    // (medal, award to a person, base to a person): BF2's disc.
    let disc = |side: &str, t: i8| match (t, side) {
        (0, _) => (150.0, 60.0),
        (1, "imp") => (150.0, 70.0),
        (1, _) => (150.0, 80.0),
        (4, _) => (600.0, 300.0),
        (5, _) => (85.0, 65.0),
        _ => unreachable!(),
    };
    let mut seen = [0usize; 2];
    for (g, db) in [(0, &bf2), (1, &bf1)] {
        let mut names: Vec<&str> = db.names().collect();
        names.sort_unstable();
        for class in names {
            let side = class.split('_').next().unwrap_or_default();
            if !FACTION_ORDER.contains(&side) || !game::roster::is_soldier_chain(db, class) {
                continue;
            }
            let Ok(r) = ClassRoster::build(db, class, 1, None) else {
                continue;
            };
            let kit = kit(&r);
            for t in [0i8, 1, 4, 5] {
                let Some((award, base)) = pair(&kit, t) else {
                    continue;
                };
                // The super battle droid's wrist blasters are their own pair.
                if award.name.contains("wrist") {
                    assert_eq!((to_person(award), to_person(base)), (150.0, 100.0));
                    continue;
                }
                let (a, b) = disc(side, t);
                assert_eq!(to_person(award), a, "{class} {}", award.name);
                // BF1's Blast Cannon is its own gun (five 86.67 bolts), not
                // BF2's shotgun: the flechette award is the same on both.
                let b = if g == 1 && t == 5 {
                    65.0 * 4.0 / 3.0
                } else {
                    b
                };
                assert!(
                    close(to_person(base), b),
                    "{class} {}: {} to a person, want {b}",
                    base.name,
                    to_person(base)
                );
                let far = |w: &WeaponClass| w.ordnance.falloff.map(|f| f.end_damage);
                if t == 1 {
                    assert_eq!(far(award), Some(75.0), "{class}: far damage");
                }
                seen[g] += 1;
            }
        }
    }
    println!("pairs checked: bf2 {}, bf1 {}", seen[0], seen[1]);
    assert!(seen[0] >= 60 && seen[1] >= 50, "{seen:?}");
}

/// A BF1 class carries BF2's award for each weapon that has one, after its
/// own slots, with BF2's clips and the BF1 weapon's looks.
#[test]
fn bf1_classes_carry_their_award_slots() {
    let (Some(bf1), Some(bf2)) = (db("bf1"), db("bf2")) else {
        return;
    };
    let build = |class: &str| ClassRoster::build(&bf1, class, 1, None).expect(class);
    let awards = |r: &ClassRoster| -> Vec<(String, u8, u32)> {
        r.held
            .iter()
            .filter(|h| h.weapon.unlock_medal().is_some())
            .map(|h| (h.weapon.name.clone(), h.slot, h.clips))
            .collect()
    };
    let named = |rows: &[(&str, u8, u32)]| -> Vec<(String, u8, u32)> {
        rows.iter().map(|(n, s, c)| ((*n).into(), *s, *c)).collect()
    };
    let trooper = build("rep_inf_clone_trooper");
    assert_eq!(
        awards(&trooper),
        named(&[
            ("rep_weap_award_rifle", 4, 4),
            ("rep_weap_award_pistol", 5, 6)
        ])
    );
    assert_eq!(trooper.weapon.medal_lock, Some(1), "the rifle gives way");
    assert_eq!(
        awards(&build("all_inf_marksman")),
        named(&[
            ("all_weap_award_sniper_rifle", 4, 6),
            ("all_weap_award_pistol", 5, 6)
        ])
    );
    assert_eq!(
        awards(&build("imp_inf_dark_trooper")),
        named(&[
            ("imp_weap_award_shotgun", 3, 6),
            ("imp_weap_award_pistol", 4, 6)
        ])
    );
    assert_eq!(
        awards(&build("all_inf_pilot")),
        named(&[
            ("all_weap_award_shotgun", 4, 4),
            ("all_weap_award_pistol", 5, 6)
        ])
    );
    // The guided launcher is an extra slot: the launcher has no lock.
    let heavy = build("imp_inf_shock_trooper");
    assert_eq!(
        awards(&heavy),
        named(&[
            ("imp_weap_award_rocket_launcher", 4, 4),
            ("imp_weap_award_pistol", 5, 6)
        ])
    );
    assert_eq!(heavy.weapon.medal_lock, None);
    let guided = &heavy.held.iter().find(|h| h.slot == 4).unwrap().weapon;
    assert!(guided.guided.is_some());
    assert_eq!(
        awards(&build("rep_inf_jet_trooper")),
        named(&[("rep_weap_award_pistol", 3, 6)])
    );
    // A class with no weapon BF2 has an award for gains none; the super
    // battle droid keeps the BF2 kit it already takes whole.
    assert_eq!(awards(&build("all_inf_smuggler")), []);
    assert_eq!(
        awards(&build("cis_inf_battledroid")),
        named(&[("cis_weap_award_wrist_blaster", 3, 4)])
    );

    // Every award is BF2's class as BF2 builds it, in everything the sim
    // reads, and looks and sounds like the BF1 weapon it stands for.
    let mut names: Vec<&str> = bf1.names().collect();
    names.sort_unstable();
    let mut checked = 0;
    for class in names {
        if !game::roster::is_soldier_chain(&bf1, class) {
            continue;
        }
        let Ok(r) = ClassRoster::build(&bf1, class, 1, None) else {
            continue;
        };
        if game::roster::WEAPON_DONORS.iter().any(|d| d.class == class) {
            continue;
        }
        let bases: Vec<(String, WeaponClass)> = kit(&r)
            .into_iter()
            .filter_map(|w| Some((bf1_award(&w.name)?.1, w.clone())))
            .collect();
        assert_eq!(
            awards(&r).len(),
            bases.len(),
            "{class}: one award a base weapon"
        );
        for (award, base) in bases {
            let h = r
                .held
                .iter()
                .find(|h| h.weapon.name == award)
                .unwrap_or_else(|| panic!("{class} holds {award}"));
            let own = game::roster::weapon_from_db(&bf2, &award).unwrap();
            let w = &h.weapon;
            assert_eq!(h.channel, 0);
            assert_eq!(
                (w.medal_unlock, w.rounds_per_clip, w.shot_delay),
                (own.medal_unlock, own.rounds_per_clip, own.shot_delay),
                "{class} {award}"
            );
            assert_eq!(
                (w.salvo_count, w.shots_per_salvo, w.ordnance.max_damage),
                (
                    own.salvo_count,
                    own.shots_per_salvo,
                    own.ordnance.max_damage
                ),
                "{class} {award}"
            );
            assert_eq!(w.ordnance.scales, own.ordnance.scales, "{class} {award}");
            assert_eq!(w.ordnance.laser, own.ordnance.laser, "{class} {award}");
            assert_eq!(w.fire_sound, base.fire_sound, "{class} {award}");
            assert_eq!(w.change_sound, base.change_sound, "{class} {award}");
            if w.guided.is_none() && w.ordnance.beam.is_none() {
                assert_eq!(w.ordnance.impact, base.ordnance.impact, "{class} {award}");
                assert_eq!(w.flash, base.flash, "{class} {award}");
            }
            assert_eq!(
                r.anim.bank_of(&award),
                r.anim.bank_of(&base.name),
                "{class} {award}"
            );
            let row = bf1_award(&base.name).unwrap().0;
            assert_eq!(
                base.medal_lock,
                row.replaces.then_some(row.medal),
                "{class} {}",
                base.name
            );
            assert_eq!(w.unlock_medal(), Some(row.medal), "{class} {award}");
            checked += 1;
        }
    }
    assert!(checked >= 60, "{checked} BF1 award slots checked");
    // Each row's award is a BF2 class on all four sides.
    for row in BF1_AWARDS {
        for side in FACTION_ORDER {
            let award = format!("{side}{}", row.award);
            assert!(bf2.resolve(&award).is_some(), "BF2 has {award}");
        }
    }
}

struct Range {
    sim: Sim,
    player: UnitHandle,
    target: UnitHandle,
}

/// `shooter` (the player) facing an idle `target` `dist` m off on flat
/// ground.
fn range(db: &ClassDb, shooter: &str, target: &str, dist: f32) -> Range {
    let mut sim = Sim::empty(42);
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    let mut names = Vec::new();
    let mut class = |sim: &mut Sim, name: &str| {
        ClassRoster::build(db, name, 1, None)
            .expect(name)
            .register(sim, &mut names)
    };
    let (s, t) = (class(&mut sim, shooter), class(&mut sim, target));
    let player = sim
        .spawn(&s.spec(0, Vec3::ZERO, 0.0, Controller::Player { slot: 0 }, 0, 0))
        .unwrap();
    let at = Vec3::new(0.0, 0.0, -dist);
    let target = sim
        .spawn(&t.spec(1, at, 0.0, Controller::Idle, 0, 0))
        .unwrap();
    for _ in 0..40 {
        sim.step(&[pad(0)]);
    }
    Range {
        sim,
        player,
        target,
    }
}

impl Range {
    fn in_hand(&self) -> &str {
        &self.sim.weapon_classes[self.sim.unit(self.player).unwrap().weapon_class].name
    }

    /// One SWITCH press at a time until `name` is in hand.
    fn take(&mut self, name: &str) {
        for _ in 0..8 {
            if self.in_hand() == name {
                break;
            }
            self.sim.step(&[pad(buttons::SWITCH_WEAPON)]);
            for _ in 0..45 {
                self.sim.step(&[pad(0)]);
            }
        }
        assert_eq!(self.in_hand(), name);
    }

    /// The names the switch reaches from the one in hand.
    fn cycle(&mut self) -> Vec<String> {
        let start = self.in_hand().to_owned();
        let mut seen = Vec::new();
        for _ in 0..8 {
            self.sim.step(&[pad(buttons::SWITCH_WEAPON)]);
            for _ in 0..45 {
                self.sim.step(&[pad(0)]);
            }
            if self.in_hand() == start {
                break;
            }
            seen.push(self.in_hand().to_owned());
        }
        seen
    }

    /// One trigger pull at the target's body: the health each hit took.
    fn shoot(&mut self) -> Vec<f32> {
        self.sim.unit_mut(self.player).unwrap().soldier.pitch = -0.03;
        let mut hits = Vec::new();
        for t in 0..60 {
            self.sim.unit_mut(self.target).unwrap().health.cur = 100_000.0;
            self.sim.step(&[pad(if t < 2 { buttons::FIRE } else { 0 })]);
            for e in &self.sim.events {
                if let Event::Hit { victim, damage, .. } = e {
                    if *victim == self.target {
                        hits.push(*damage);
                    }
                }
            }
        }
        hits
    }
}

/// The award cheat on a BF1 kit: the Elite rifle and the award pistol take
/// the rifle's and the pistol's place, and each hit is BF2's 150; the
/// damage award's cheat puts x1.5 over it; the cheat off gives the kit back.
#[test]
fn the_award_cheat_swaps_a_bf1_kit() {
    let Some(bf1) = db("bf1") else { return };
    let mut r = range(
        &bf1,
        "rep_inf_clone_trooper",
        "cis_inf_battledroid_inf",
        10.0,
    );
    assert_eq!(r.in_hand(), "rep_weap_inf_rifle");
    assert_eq!(r.cycle(), ["rep_weap_inf_pistol"], "the disc's kit");
    assert_eq!(r.shoot(), [80.0], "the shared rifle");

    r.sim.cheats.all_awards = true;
    r.sim.step(&[pad(0)]);
    assert_eq!(r.in_hand(), "rep_weap_award_rifle");
    assert_eq!(r.cycle(), ["rep_weap_award_pistol"]);
    // One pull is a three-round burst.
    assert_eq!(r.shoot(), [150.0; 3], "150 a round");
    r.sim.cheats.damage_award = true;
    r.sim.step(&[pad(0)]);
    assert_eq!(r.shoot(), [225.0; 3], "150 a round x 1.5");
    r.take("rep_weap_award_pistol");
    assert_eq!(r.shoot(), [225.0]);
    // The target is a bot: its awards stay locked.
    let bot = r.sim.unit(r.target).unwrap();
    assert!(bot.held.iter().any(|w| r.sim.weapon_locked(bot, w.class)));
    assert_eq!(bot.buffs.dealt(), 1.0);

    r.sim.cheats.all_awards = false;
    r.sim.cheats.damage_award = false;
    r.sim.step(&[pad(0)]);
    assert_eq!(r.in_hand(), "rep_weap_inf_pistol", "the pistol is back");
    assert_eq!(r.cycle(), ["rep_weap_inf_rifle"]);
    r.take("rep_weap_inf_rifle");
    assert_eq!(r.shoot(), [80.0], "and the damage award is gone");
}

/// The same pull on BF2's own kit: the two games' troopers get the same
/// award rifle (the Republic's 80 rifle against 150 a round).
#[test]
fn a_bf1_award_rifle_is_bf2s() {
    let (Some(bf1), Some(bf2)) = (db("bf1"), db("bf2")) else {
        return;
    };
    let pull = |db: &ClassDb, shooter: &str, target: &str, award: &str| {
        let mut r = range(db, shooter, target, 10.0);
        let base = r.shoot();
        r.sim.cheats.all_awards = true;
        r.sim.step(&[pad(0)]);
        assert_eq!(r.in_hand(), award);
        (base, r.shoot())
    };
    let one = pull(
        &bf1,
        "rep_inf_clone_trooper",
        "cis_inf_battledroid_inf",
        "rep_weap_award_rifle",
    );
    let two = pull(
        &bf2,
        "rep_inf_ep3_rifleman",
        "cis_inf_marine",
        "rep_weap_award_rifle",
    );
    assert_eq!(one, two);
    assert_eq!(one, (vec![80.0], vec![150.0; 3]));
    // The Empire's lighter rifle: 70 a bolt under the same 150 award.
    let imp = pull(
        &bf1,
        "imp_inf_storm_trooper",
        "all_inf_soldier",
        "imp_weap_award_rifle",
    );
    assert_eq!(imp, (vec![70.0], vec![150.0; 3]));
}

/// The beam rifle and the flechette shotgun on BF1 kits: a held beam takes
/// BF2's 600 off a body, and each flechette that lands takes BF2's 85.
#[test]
fn bf1_snipers_and_pilots_fire_bf2s_awards() {
    let Some(bf1) = db("bf1") else { return };
    let mut r = range(&bf1, "all_inf_marksman", "imp_inf_storm_trooper", 10.0);
    assert_eq!(r.shoot(), [300.0], "the shared sniper rifle");
    r.sim.cheats.all_awards = true;
    r.sim.step(&[pad(0)]);
    assert_eq!(r.in_hand(), "all_weap_award_sniper_rifle");
    let beam: f32 = r.shoot().iter().sum();
    // 600 over the beam's 0.15 s: the fifth tick's share overshoots a
    // little, as on BF2 maps.
    assert!((600.0..=667.0).contains(&beam), "beam took {beam}");

    let mut r = range(&bf1, "all_inf_pilot", "imp_inf_storm_trooper", 5.0);
    r.sim.cheats.all_awards = true;
    r.sim.step(&[pad(0)]);
    assert_eq!(r.in_hand(), "all_weap_award_shotgun");
    let pellets = r.shoot();
    assert!(!pellets.is_empty() && pellets.len() <= 7, "{pellets:?}");
    for p in pellets {
        // A flechette on the body, or x3 on the head.
        assert!(p == 85.0 || p == 255.0, "{p}");
    }
}
