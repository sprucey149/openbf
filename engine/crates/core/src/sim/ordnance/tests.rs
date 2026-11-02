use super::*;
use crate::sim::tests::flat_ground;
use crate::sticky::StickyClass;
use crate::unit::{SlotSpec, UnitHandle, UnitSpec};
use crate::weapon::{ExplosionClass, OrdnanceClass, Thrown, WeaponClass};
use crate::TICK_DT;

/// Same-owner warnings are linked to instances, even when contacts share
/// a tick and unrelated warnings are compacted out of the vector.
#[test]
fn same_tick_grenades_keep_their_own_warnings() {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    let owner = s
        .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 40.0)))
        .unwrap();
    let mut wc = thermal(true);
    let class = wc.ordnance.sticky.as_mut().unwrap();
    class.stick_mask = 0;
    class.explosion = None;
    let class = class.clone();
    let first_class = s.add_weapon_class(wc.clone());
    wc.ordnance.life_span = 1.5;
    let second_class = s.add_weapon_class(wc);
    s.add_hazard(Vec3::ZERO, 1);
    let mut a = Sticky::new(
        Vec3::new(-5.0, 0.1, 0.0),
        Vec3::new(2.0, -5.0, 0.0),
        &class,
        0.6,
        owner,
        0,
        first_class,
    );
    let mut b = Sticky::new(
        Vec3::new(5.0, 0.1, 0.0),
        Vec3::new(-2.0, -9.0, 0.0),
        &class,
        1.5,
        owner,
        0,
        second_class,
    );
    assert!(s.tick_sticky(&mut a, TICK_DT));
    assert!(s.tick_sticky(&mut b, TICK_DT));
    assert_eq!(a.touched, Some(0));
    assert_eq!(a.touched, b.touched);
    assert!(a.moving && b.moving, "first contacts bounce");
    assert_ne!(a.warning_id, b.warning_id);
    let (mut alive_a, mut alive_b) = (true, true);
    let (mut rested_a, mut rested_b, mut first_expired_alone) = (false, false, false);
    for tick in 1..600 {
        s.tick = tick;
        // Compact the unrelated warning. Keep grenade warnings through
        // their burn tick so expiry matching can be inspected even if a
        // rounded deadline becomes due one tick before the float fuse.
        s.hazards.retain(|z| z.warning_id.is_some());
        let (was_moving_a, was_moving_b) = (a.moving, b.moving);
        if alive_a {
            alive_a = s.tick_sticky(&mut a, TICK_DT);
        }
        if alive_b {
            alive_b = s.tick_sticky(&mut b, TICK_DT);
        }
        for (g, alive, was_moving, rested) in [
            (&a, alive_a, was_moving_a, &mut rested_a),
            (&b, alive_b, was_moving_b, &mut rested_b),
        ] {
            // A's warning is deliberately compacted after detonation.
            if !alive && *rested {
                continue;
            }
            let z = s
                .hazards
                .iter()
                .find(|z| z.warning_id == g.warning_id)
                .expect("own warning before detonation compaction");
            assert_eq!(z.since, 0, "reaction clock stays at first contact");
            assert_eq!(z.owner, Some(owner));
            if alive && was_moving && !g.moving {
                *rested = true;
                assert_eq!(z.pos, g.pos, "rest corrects only its own warning");
                let span = s.weapon_classes[g.weapon_class].ordnance.life_span;
                assert_eq!(
                    z.until,
                    tick + (g.life.min(span) * crate::TICK_HZ as f32).ceil() as u64
                );
            }
        }
        if !alive_a && alive_b && !first_expired_alone {
            let a_warning = s
                .hazards
                .iter()
                .find(|z| z.warning_id == a.warning_id)
                .unwrap();
            assert_eq!(a_warning.until, tick, "A expires its own warning");
            let b_warning = s
                .hazards
                .iter()
                .find(|z| z.warning_id == b.warning_id)
                .unwrap();
            assert!(
                b_warning.until > tick,
                "B retains its separate later warning"
            );
            s.hazards.retain(|z| z.warning_id != a.warning_id);
            first_expired_alone = true;
        }
        if !alive_a && !alive_b {
            let b_warning = s
                .hazards
                .iter()
                .find(|z| z.warning_id == b.warning_id)
                .unwrap();
            assert_eq!(
                b_warning.until, tick,
                "B expires its own warning after compaction"
            );
            break;
        }
    }
    assert!(rested_a && rested_b && first_expired_alone);
    assert!(!alive_a && !alive_b);
}

#[test]
fn warning_links_and_allocator_are_observed_by_the_hash() {
    let mut s = Sim::empty(7);
    let baseline = s.hash();
    s.add_hazard(Vec3::ZERO, 10);
    let unlinked = s.hash();
    s.hazards[0].warning_id = Some(1);
    assert_ne!(s.hash(), unlinked);
    let linked = s.hash();
    s.hazards[0].warning_id = Some(2);
    assert_ne!(s.hash(), linked);
    s.hazards.clear();
    assert_eq!(s.hash(), baseline, "external warnings issue no identity");
    s.last_warning_id = 1;
    assert_ne!(
        s.hash(),
        baseline,
        "allocator history survives empty vectors"
    );
    let c = StickyClass::new(true);
    let mut g = Sticky::new(
        Vec3::ZERO,
        Vec3::Y,
        &c,
        1.0,
        UnitHandle { index: 0, gen: 1 },
        0,
        0,
    );
    let bytes = |g: &Sticky| {
        let mut bytes = Vec::new();
        g.hash_into(&mut |part| bytes.extend_from_slice(part));
        bytes
    };
    let no_link = bytes(&g);
    g.warning_id = Some(1);
    let first_link = bytes(&g);
    assert_ne!(first_link, no_link);
    g.warning_id = Some(2);
    assert_ne!(bytes(&g), first_link);
}

/// The thermal detonator as the discs author it (`re-grenades.md` §9):
/// BF1 ShotDelay 1.4, wind-up 0.3, fuse 0.9; BF2 1.3, 0.51, 0.6.
fn thermal(bf2: bool) -> WeaponClass {
    WeaponClass {
        name: "thermal".into(),
        rounds_per_clip: 1,
        reload_time: 0.01,
        trigger_single: true,
        shot_delay: if bf2 { [1.3; 2] } else { [1.4; 2] },
        initial_salvo_delay: if bf2 { 0.51 } else { 0.3 },
        thrown: Some(Thrown {
            shot_elevate: 0.21,
            hide_on_fire: true,
            hand: bf2.then_some(Thrown::BF2_HAND),
        }),
        ordnance: OrdnanceClass {
            velocity: 25.0,
            life_span: if bf2 { 0.6 } else { 0.9 },
            sticky: Some(StickyClass {
                gravity: -12.25,
                stick_mask: if bf2 { 0x7fff_ffd8 } else { 0x7fff_ff80 },
                explosion: Some(ExplosionClass {
                    damage: 320.0,
                    damage_inner: 4.5,
                    damage_outer: 6.5,
                    ..ExplosionClass::default()
                }),
                ..StickyClass::new(bf2)
            }),
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    }
}

/// A player at the origin facing −Z holding the rifle, a pistol (slot 1,
/// channel 0), the thermal (slot 2, channel 1, 4 clips) and a second
/// grenade (slot 3, channel 1, 2 clips).
fn grenadier(bf2: bool) -> (Sim, UnitHandle) {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    let mut pistol = WeaponClass::bf1_rifle();
    pistol.name = "pistol".into();
    let pistol = s.add_weapon_class(pistol);
    let thermal = s.add_weapon_class(thermal(bf2));
    let slot = |slot, channel, weapon_class, clips| SlotSpec {
        slot,
        channel,
        weapon_class,
        clips,
    };
    let p = s
        .spawn(&UnitSpec {
            slots: std::sync::Arc::new([
                slot(1, 0, pistol, 0),
                slot(2, 1, thermal, 4),
                slot(3, 1, thermal, 2),
            ]),
            ..UnitSpec::player(0, Vec3::ZERO)
        })
        .unwrap();
    // A level aim (the tests' launch directions are from it).
    s.units[p.index as usize].soldier.pitch = 0.0;
    (s, p)
}

fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..Default::default()
    }
}

fn throws(s: &Sim) -> usize {
    s.events
        .iter()
        .filter(|e| matches!(e, Event::Throw { .. }))
        .count()
}

/// Step with `buttons` until a throw; the number of steps taken.
fn steps_to_throw(s: &mut Sim, buttons: u32, limit: u32) -> Option<u32> {
    for n in 1..=limit {
        s.events.clear();
        s.step(&[pad(buttons)]);
        if throws(s) > 0 {
            return Some(n);
        }
    }
    None
}

/// The cycle (§3.2 [M]): the grenade leaves the hand at the first update
/// at or past the wind-up (BF1 0.30 s, BF2 0.51 s -> 0.533), a second
/// press is ignored until 1.70 / 1.81 s after the first, the count goes
/// 4 -> 3, and a held trigger throws once.
#[test]
fn the_grenade_cycle_matches_the_ps2_timings() {
    for (bf2, windup, cycle) in [(false, 0.30f32, 1.70f32), (true, 0.51, 1.81)] {
        let (mut s, p) = grenadier(bf2);
        let count = |s: &Sim| {
            let u = s.unit(p).unwrap();
            let w = &u.held[u.cur1.unwrap() as usize];
            w.state.rounds(&s.weapon_classes[w.class]).unwrap()
        };
        assert_eq!(count(&s), 4, "WeaponAmmo 4 = one in the hand + three");
        // Held all the way through: one throw.
        let n = steps_to_throw(&mut s, buttons::SECONDARY_FIRE, 60).expect("a throw");
        let t = n as f32 * TICK_DT;
        assert!(
            t >= windup - 1e-3 && t < windup + 2.0 * TICK_DT,
            "bf2 {bf2}: thrown {t} s after the press, wind-up {windup}"
        );
        assert_eq!(count(&s), 3);
        assert_eq!(s.unit(p).unwrap().active_channel, 1);
        let mut ticks = n;
        while (ticks as f32) * TICK_DT < cycle + 1.0 {
            s.events.clear();
            s.step(&[pad(buttons::SECONDARY_FIRE)]);
            ticks += 1;
            assert_eq!(throws(&s), 0, "a held trigger throws once");
        }
        assert_eq!(count(&s), 3);

        // Tapped again and again from the first press: the second throw
        // winds up only once the cycle (wind-up + ShotDelay) has run out.
        let (mut s, _) = grenadier(bf2);
        let first = steps_to_throw(&mut s, buttons::SECONDARY_FIRE, 60).unwrap();
        let mut n = first;
        let second = loop {
            // A fresh press every other tick.
            let b = if n.is_multiple_of(2) {
                buttons::SECONDARY_FIRE
            } else {
                0
            };
            s.events.clear();
            s.step(&[pad(b)]);
            n += 1;
            if throws(&s) > 0 {
                break n;
            }
            assert!(n < 200, "no second throw");
        };
        let pressed = (second - first) as f32 * TICK_DT - windup;
        // The second wind-up started `pressed` s after the first throw:
        // not before the cycle's ShotDelay, and within a few ticks of it
        // (the reload's tick and the tap's phase).
        let shot_delay = cycle - windup;
        assert!(
            pressed >= shot_delay - 2.0 * TICK_DT && pressed <= shot_delay + 5.0 * TICK_DT,
            "bf2 {bf2}: second press took {pressed} s after the throw, ShotDelay {shot_delay}"
        );
        assert_eq!(count(&s), 2);
    }
}

/// Channel 1 wins (§2.3): while the grenade's fire state runs the
/// primary trigger is swallowed, and it fires again afterwards.
#[test]
fn the_secondary_swallows_the_primary_trigger() {
    let (mut s, p) = grenadier(false);
    let both = buttons::FIRE | buttons::SECONDARY_FIRE;
    let shots = |s: &Sim| {
        s.events
            .iter()
            .filter(|e| matches!(e, Event::Shot { .. }))
            .count()
    };
    let mut fired = 0;
    for _ in 0..30 {
        s.events.clear();
        s.step(&[pad(both)]);
        fired += shots(&s);
    }
    assert_eq!(
        fired, 0,
        "the rifle is silent while the grenade has the soldier"
    );
    assert_eq!(s.unit(p).unwrap().active_channel, 1);
    // Let the cycle and the reload run out with FIRE alone held.
    for _ in 0..90 {
        s.events.clear();
        s.step(&[pad(buttons::FIRE)]);
        fired += shots(&s);
    }
    assert!(fired > 0, "the rifle fires once channel 1 lets go");
    assert_eq!(s.unit(p).unwrap().active_channel, 0);
}

/// The empty click (card WSND, re-weapon-sounds.md §7.3): a human's
/// trigger PRESS on a weapon that is reloading - once per press, not
/// while held, never for a bot.
#[test]
fn a_trigger_press_on_a_reloading_weapon_clicks_for_a_human() {
    let clicks = |s: &Sim| {
        s.events
            .iter()
            .filter(|e| matches!(e, Event::FireEmpty { .. }))
            .count()
    };
    for human in [true, false] {
        let (mut s, p) = grenadier(false);
        let i = p.index as usize;
        s.units[i].is_player = human;
        // A shot, then a reload of the part-used clip.
        s.step(&[pad(buttons::FIRE)]);
        assert_eq!(clicks(&s), 0, "a weapon that fires does not click");
        s.step(&[pad(0)]);
        s.step(&[pad(buttons::RELOAD)]);
        assert!(s.units[i].weapon.reloading());
        s.step(&[pad(0)]);
        let mut n = 0;
        for _ in 0..3 {
            s.step(&[pad(buttons::FIRE)]);
            n += clicks(&s);
        }
        assert!(s.units[i].weapon.reloading());
        assert_eq!(n, usize::from(human), "human {human}");
    }
}

/// The overheated click (§7.3): the class that refused it, at the soldier,
/// once per press, never for a bot, on both channels' current weapons.
#[test]
fn a_press_on_an_overheated_weapon_clicks_for_a_human_on_either_channel() {
    let clicks = |s: &Sim| {
        s.events
            .iter()
            .filter_map(|e| match e {
                Event::FireEmpty {
                    unit,
                    weapon_class,
                    pos,
                } => Some((*unit, *weapon_class, *pos)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for human in [true, false] {
        let (mut s, p) = grenadier(false);
        let i = p.index as usize;
        s.units[i].is_player = human;
        let class = s.units[i].weapon_class as u32;
        s.units[i].weapon.heat = 2.0;
        s.step(&[pad(0)]);
        assert!(s.units[i].weapon.overheated);
        s.step(&[pad(buttons::FIRE)]);
        let at = s.units[i].soldier.pos;
        let want = if human { vec![(p, class, at)] } else { vec![] };
        assert_eq!(clicks(&s), want, "human {human}");
        s.step(&[pad(buttons::FIRE)]);
        assert!(clicks(&s).is_empty(), "held: one click per press");

        s.step(&[pad(0)]);
        s.step(&[pad(0)]);
        let k = s.units[i].cur1.unwrap() as usize;
        let class = s.units[i].held[k].class as u32;
        s.units[i].held[k].state.heat = 2.0;
        s.step(&[pad(0)]);
        assert!(s.units[i].held[k].state.overheated);
        s.step(&[pad(buttons::SECONDARY_FIRE)]);
        let n = clicks(&s);
        assert_eq!(n.len(), usize::from(human), "human {human}");
        assert!(n.iter().all(|c| c.1 == class));
    }
}

/// Nothing left to throw: the press never reaches the weapon's cycle, and
/// it still clicks, once, for a human. With something left, it throws.
#[test]
fn a_press_on_an_empty_channel_one_weapon_clicks_for_a_human() {
    let clicks = |s: &Sim| {
        s.events
            .iter()
            .filter(|e| matches!(e, Event::FireEmpty { .. }))
            .count()
    };
    for human in [true, false] {
        let (mut s, p) = grenadier(false);
        let i = p.index as usize;
        s.units[i].is_player = human;
        s.step(&[pad(buttons::SECONDARY_FIRE)]);
        assert_eq!(clicks(&s), 0, "a grenade in hand does not click");

        let (mut s, p) = grenadier(false);
        let i = p.index as usize;
        s.units[i].is_player = human;
        let k = s.units[i].cur1.unwrap() as usize;
        s.units[i].held[k].state.clip = 0;
        s.units[i].held[k].state.spare = 0;
        s.step(&[pad(0)]);
        let mut n = 0;
        for _ in 0..3 {
            s.step(&[pad(buttons::SECONDARY_FIRE)]);
            n += clicks(&s);
        }
        assert_eq!(n, usize::from(human), "human {human}");
        assert_eq!(throws(&s), 0);
    }
}

/// The two switch inputs (§2.4): each steps its own channel, wraps, and
/// does nothing while a trigger is down.
#[test]
fn each_switch_steps_its_own_channel() {
    let (mut s, p) = grenadier(false);
    let slots = |s: &Sim| {
        let u = s.unit(p).unwrap();
        (u.weapon_slot, u.held[u.cur1.unwrap() as usize].slot)
    };
    let tap = |s: &mut Sim, b: u32| {
        s.step(&[pad(b)]);
        s.step(&[pad(0)]);
    };
    assert_eq!(slots(&s), (0, 2));
    // Each switch says which weapon was selected (its
    // WeaponChangeSound, card WSND), on the tick of the press.
    let selected = |s: &mut Sim, b: u32| -> Vec<u32> {
        s.step(&[pad(b)]);
        let got = s
            .events
            .iter()
            .filter_map(|e| match e {
                Event::WeaponSelect { unit, weapon_class } if *unit == p => Some(*weapon_class),
                _ => None,
            })
            .collect();
        s.step(&[pad(0)]);
        assert!(!s
            .events
            .iter()
            .any(|e| matches!(e, Event::WeaponSelect { .. })));
        got
    };
    assert_eq!(selected(&mut s, buttons::SWITCH_WEAPON), [1]);
    assert_eq!(slots(&s), (1, 2), "channel 0 -> the pistol");
    assert_eq!(s.unit(p).unwrap().weapon_class, 1);
    assert_eq!(selected(&mut s, buttons::SWITCH_SECONDARY), [2]);
    assert_eq!(slots(&s), (1, 3), "channel 1 -> the second grenade");
    tap(&mut s, buttons::SWITCH_WEAPON);
    tap(&mut s, buttons::SWITCH_SECONDARY);
    assert_eq!(slots(&s), (0, 2), "both wrap");
    // Held is one step, not one a tick.
    s.step(&[pad(buttons::SWITCH_WEAPON)]);
    s.step(&[pad(buttons::SWITCH_WEAPON)]);
    assert_eq!(slots(&s), (1, 2));
    s.step(&[pad(0)]);
    // Not while a trigger is down.
    s.step(&[pad(buttons::FIRE)]);
    s.step(&[pad(buttons::FIRE | buttons::SWITCH_WEAPON)]);
    assert_eq!(slots(&s), (1, 2));
    // The grenade thrown is the selected one's: 2 clips -> 1.
    tap(&mut s, 0);
    tap(&mut s, buttons::SWITCH_SECONDARY);
    assert!(steps_to_throw(&mut s, buttons::SECONDARY_FIRE, 60).is_some());
    let u = s.unit(p).unwrap();
    let rounds: Vec<u32> = u
        .held
        .iter()
        .filter(|w| w.channel == 1)
        .map(|w| w.state.rounds(&s.weapon_classes[w.class]).unwrap())
        .collect();
    assert_eq!(rounds, [4, 1]);
}

/// A throw on flat ground, through the sim: launched from the fire point
/// (BF1) or the hand (BF2) at Velocity along the elevated aim, it
/// bounces, rests, then burns its fuse and its blast hurts an enemy
/// standing where it lies — and the bots' warning sits there meanwhile.
#[test]
fn a_thrown_grenade_rests_then_blasts() {
    for bf2 in [false, true] {
        let (mut s, p) = grenadier(bf2);
        assert!(steps_to_throw(&mut s, buttons::SECONDARY_FIRE, 60).is_some());
        let Some(Event::Throw { origin, .. }) = s
            .events
            .iter()
            .find(|e| matches!(e, Event::Throw { .. }))
            .cloned()
        else {
            unreachable!()
        };
        let want = if bf2 {
            Vec3::new(0.0, Thrown::BF2_HAND.0, -Thrown::BF2_HAND.1)
        } else {
            s.unit(p).unwrap().muzzle()
        };
        assert!((origin - want).length() < 1e-4, "bf2 {bf2}: from {origin}");
        let g = &s.stickies[0];
        let launch = sticky::throw_dir(Vec3::NEG_Z, 0.21) * 25.0;
        // One flight step has already run: only gravity has touched it.
        assert!((g.vel - launch - Vec3::Y * (-12.25 * TICK_DT)).length() < 1e-3);

        let mut rest: Option<(u64, Vec3)> = None;
        let mut blast: Option<(u64, Vec3)> = None;
        let mut victim = None;
        for _ in 0..400 {
            s.events.clear();
            s.step(&[pad(0)]);
            if rest.is_none() {
                if let Some(g) = s.stickies.first().filter(|g| !g.moving) {
                    rest = Some((s.tick, g.pos));
                    assert!(g.armed && g.vel == Vec3::ZERO);
                    assert_eq!(s.hazards.len(), 1);
                    assert!((s.hazards[0].pos - g.pos).length() < 1e-6);
                    // An enemy walks up to it.
                    victim = s.spawn(&UnitSpec::trooper(1, g.pos + Vec3::X));
                }
            }
            if let Some(Event::StickyBlast { pos, .. }) = s
                .events
                .iter()
                .find(|e| matches!(e, Event::StickyBlast { .. }))
            {
                blast = Some((s.tick, *pos));
                break;
            }
        }
        let (rest_tick, rest_pos) = rest.expect("it came to rest");
        let (blast_tick, blast_pos) = blast.expect("it went off");
        assert_eq!(blast_pos, rest_pos);
        assert!(rest_pos.z < -15.0, "bf2 {bf2}: it flew, {rest_pos}");
        let fuse = (blast_tick - rest_tick) as f32 * TICK_DT;
        let life = if bf2 { 0.6 } else { 0.9 };
        assert!(
            (fuse - life).abs() <= TICK_DT + 1e-4,
            "bf2 {bf2}: {fuse} s from rest to blast, LifeSpan {life}"
        );
        assert!(s.stickies.is_empty());
        // 320 at 1 m (inside the inner radius) kills a 300-health trooper,
        // and the thrower has the kill.
        assert!(s.unit(victim.unwrap()).is_none(), "the blast killed it");
        assert_eq!(s.unit(p).unwrap().kills, 1);
        s.step(&[pad(0)]);
        assert!(s.hazards.is_empty(), "the warning ends with the blast");
    }
}

/// The PS2's own throws (card GRNP, re-grenades §13 [M]): seven BF2
/// thermal detonators on Mos Eisley's level street, the grenade's RAM row
/// after each update. From each throw's first row the port's grenade is
/// the PS2's row for row — every bounce, the rest update and the blast.
#[test]
fn sticky_ps2_bf2_throws_match_row_for_row() {
    let mut worst = 0.0f32;
    let mut throws = 0;
    let text = include_str!("../../../tests/data/grenade_ps2_bf2.txt");
    let mut lines = text.lines().filter(|l| !l.starts_with('#')).peekable();
    while let Some(head) = lines.next() {
        let mut h = head.split_whitespace();
        assert_eq!(h.next(), Some("throw"), "{head}");
        let name = h.next().unwrap();
        let life: f32 = h.next().unwrap().parse().unwrap();
        let mut rows = Vec::new();
        while let Some(l) = lines.next_if(|l| !l.starts_with("throw")) {
            let v: Vec<f32> = l.split_whitespace().map(|t| t.parse().unwrap()).collect();
            rows.push(v);
        }
        let (mut s, p) = grenadier(true);
        let wc = s.units[p.index as usize].held[1].class;
        let sc = s.weapon_classes[wc].ordnance.sticky.clone().unwrap();
        let v3 = |r: &[f32], k: usize| Vec3::new(r[k], r[k + 1], r[k + 2]);
        let first = &rows[0];
        s.stickies
            .push(Sticky::new(v3(first, 1), v3(first, 4), &sc, life, p, 0, wc));
        let mut update = first[0] as u32;
        let mut rest = None;
        for r in &rows[1..] {
            while update < r[0] as u32 {
                s.tick_stickies(TICK_DT);
                update += 1;
            }
            let g = s
                .stickies
                .first()
                .unwrap_or_else(|| panic!("{name}: gone at update {update}, the PS2's is there"));
            let e = (g.pos - v3(r, 1)).length();
            assert!(
                e <= 0.002,
                "{name} update {update}: {} against the PS2's {} ({e} m)",
                g.pos,
                v3(r, 1)
            );
            worst = worst.max(e);
            assert!(
                (g.vel - v3(r, 4)).length() <= 0.01,
                "{name} update {update}"
            );
            assert_eq!(
                (g.bounces, g.armed),
                (r[7] as u8, r[8] != 0.0),
                "{name} update {update}"
            );
            if g.armed && rest.is_none() {
                rest = Some(update);
            }
        }
        // The PS2's last row is the update before its blast: the fuse is
        // 18 updates from the rest.
        assert_eq!(update, rest.expect("it rested") + 17, "{name}");
        s.tick_stickies(TICK_DT);
        assert!(s.stickies.is_empty(), "{name}: the blast");
        throws += 1;
    }
    assert_eq!(throws, 7);
    println!("7 throws, worst row {worst} m");
}

/// Card GRNP: a BF1 thermal detonator thrown as BF2 throws
/// ([`Thrown::throw_as_bf2`]) leaves the hand on the same tick as BF2's,
/// flies, bounces and rests tick for tick where BF2's does and goes off
/// on BF2's tick — with BF1's own blast.
#[test]
fn a_bf1_grenade_thrown_as_bf2_is_bf2s_throw() {
    let run = |class: WeaponClass| {
        let mut s = Sim::empty(7);
        s.heightfield = Some(flat_ground());
        let wc = s.add_weapon_class(class);
        let p = s
            .spawn(&UnitSpec {
                slots: std::sync::Arc::new([SlotSpec {
                    slot: 2,
                    channel: 1,
                    weapon_class: wc,
                    clips: 4,
                }]),
                ..UnitSpec::player(0, Vec3::ZERO)
            })
            .unwrap();
        s.units[p.index as usize].soldier.pitch = 0.0;
        let mut rows = Vec::new();
        let (mut thrown, mut blast) = (None, None);
        for tick in 1..=200u32 {
            s.events.clear();
            s.step(&[pad(if tick <= 4 {
                buttons::SECONDARY_FIRE
            } else {
                0
            })]);
            for e in &s.events {
                match e {
                    Event::Throw { .. } => thrown = Some(tick),
                    Event::StickyBlast { .. } => blast = Some(tick),
                    _ => {}
                }
            }
            rows.push(s.stickies.first().map(|g| (g.pos, g.moving)));
        }
        (rows, thrown.unwrap(), blast.unwrap())
    };
    let mut bf1 = thermal(false);
    Thrown::throw_as_bf2(&mut bf1);
    assert_eq!(
        bf1.ordnance.sticky.as_ref().unwrap().stick_mask,
        0x7fff_ffd8
    );
    // BF1's own blast and cycle stay.
    assert_eq!(bf1.shot_delay, [1.4; 2]);
    let (a, a_thrown, a_blast) = run(bf1);
    let (b, b_thrown, b_blast) = run(thermal(true));
    // The PS2's BF2 release: the 16th update from the press (§13.1).
    assert_eq!((a_thrown, a_blast), (b_thrown, b_blast));
    assert_eq!(a_thrown, 16);
    assert_eq!(a, b, "row for row");
    // The plain BF1 throw is another: its 0.3 s wind-up, the fire point.
    let (c, c_thrown, _) = run(thermal(false));
    assert_eq!(c_thrown, 10);
    assert_ne!(c, a);
    // Level ground, a level aim: at rest about where the PS2's BF2
    // throw at aim +0.5 deg lies (35.49 m, 74 updates after the release
    // — the half degree moves the bounces a few updates; the fixture
    // test above is the exact one), the blast 18 updates after the rest.
    let rest = a.iter().position(|r| r.is_some_and(|r| !r.1)).unwrap();
    let at = a[rest].unwrap().0;
    let rest = rest as u32 + 1;
    assert!((at.length() - 35.5).abs() < 0.6, "rest at {at}");
    assert!(rest.abs_diff(a_thrown + 74) <= 4, "rest tick {rest}");
    assert_eq!(a_blast, rest + 18);
}

/// Card GRFL: a grenade thrown down at 48 deg onto a level MESH floor (a
/// two-triangle sheet, either winding) bounces and rests on it. The steep
/// impact leaves along the normal (f = 1), so the next sweep starts
/// [`sticky::PUSH_OFF`] over the face: a ray pulled back by
/// [`sticky::SWEEP_BACK`] met the two-sided face from behind and the
/// bounce put the grenade under the floor (the maintainer's Kamino decks).
#[test]
fn a_grenade_thrown_steeply_at_a_mesh_floor_rests_on_it() {
    use crate::collision::{trimesh, Body, CollisionWorld, Shape};
    const FLOOR: f32 = 10.0;
    let corners = [
        [-30.0, FLOOR, -30.0],
        [30.0, FLOOR, -30.0],
        [30.0, FLOOR, 30.0],
        [-30.0, FLOOR, 30.0],
    ];
    for (winding, bf2) in [(0, true), (1, true), (0, false), (1, false)] {
        let indices: [u32; 6] = if winding == 0 {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        let sheet = Body::new(
            glam::Mat4::IDENTITY,
            Shape::Mesh(trimesh(&corners, &indices).expect("mesh")),
        );
        let mut s = Sim::empty(7);
        s.collision = CollisionWorld::new(vec![sheet], Vec::new());
        let wc = s.add_weapon_class(thermal(bf2));
        let owner = s
            .spawn(&UnitSpec::player(0, Vec3::new(20.0, FLOOR, 20.0)))
            .unwrap();
        let class = s.weapon_classes[wc].ordnance.sticky.clone().unwrap();
        let a = 48.0f32.to_radians();
        let vel = Vec3::new(a.cos(), -a.sin(), 0.0) * 12.0;
        s.stickies.push(Sticky::new(
            Vec3::new(0.0, FLOOR + 2.0, 0.0),
            vel,
            &class,
            1.0e6,
            owner,
            0,
            wc,
        ));
        let mut low = f32::MAX;
        for _ in 0..600 {
            s.step(&[pad(0)]);
            // Under the floor it falls to the world's floor and is gone.
            let Some(g) = s.stickies.first() else { break };
            low = low.min(g.pos.y);
            if !g.moving {
                break;
            }
        }
        let case = format!("winding {winding} bf2 {bf2}");
        assert!(
            low > FLOOR,
            "{case}: it went under the floor, lowest y {low}"
        );
        let g = &s.stickies[0];
        assert!(!g.moving && g.bounces > 0, "{case}: it came to rest, {g:?}");
        assert!(
            (g.pos.y - FLOOR).abs() <= sticky::PUSH_OFF + 1e-3,
            "{case}: it lies on the floor, y {}",
            g.pos.y
        );
    }
}

/// Issue #37 (BF2 Jabba's Palace, the cell corridor's step): a grenade
/// that meets the underside of a step's lip, 2.5 cm over the floor, is
/// pushed off along the lip's down-facing normal. The PS2's full 0.1 m
/// put it under the floor; the push stops short of the floor now.
#[test]
fn a_grenade_under_a_steps_lip_stays_over_the_floor() {
    use crate::collision::{trimesh, Body, CollisionWorld, Shape};
    const FLOOR: f32 = 10.0;
    let floor = [
        [-30.0, FLOOR, -30.0],
        [30.0, FLOOR, -30.0],
        [30.0, FLOOR, 30.0],
        [-30.0, FLOOR, 30.0],
    ];
    // The lip: its bottom edge 2.5 cm over the floor at z 2, rising
    // toward -z, facing down and toward the thrower (the traced normal).
    let n = Vec3::new(0.0, -0.45, -0.89).normalize();
    let up = n.cross(Vec3::X).normalize() * -1.0;
    let edge = Vec3::new(0.0, FLOOR + 0.025, 2.0);
    let lip = [
        edge - Vec3::X * 5.0,
        edge + Vec3::X * 5.0,
        edge + Vec3::X * 5.0 + up * 0.5,
        edge - Vec3::X * 5.0 + up * 0.5,
    ]
    .map(|v| v.to_array());
    for (winding, bf2) in [(0, true), (1, true), (0, false), (1, false)] {
        let quad: [u32; 6] = if winding == 0 {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        for k in 0..9 {
            let mut s = Sim::empty(7);
            let sheet = |c: &[[f32; 3]; 4]| {
                Body::new(
                    glam::Mat4::IDENTITY,
                    Shape::Mesh(trimesh(c, &quad).expect("mesh")),
                )
            };
            s.collision = CollisionWorld::new(vec![sheet(&floor), sheet(&lip)], Vec::new());
            let wc = s.add_weapon_class(thermal(bf2));
            let owner = s
                .spawn(&UnitSpec::player(0, Vec3::new(20.0, FLOOR, 20.0)))
                .unwrap();
            let class = s.weapon_classes[wc].ordnance.sticky.clone().unwrap();
            // Level throws 0.5 m short of the lip, low enough to meet it
            // within the PS2 push-off's reach of the floor.
            let start = Vec3::new(0.0, FLOOR + 0.04 + 0.01 * k as f32, 1.5);
            let g = Sticky::new(start, Vec3::new(0.0, 0.0, 6.0), &class, 1.0e6, owner, 0, wc);
            s.stickies.push(g);
            let mut low = f32::MAX;
            for _ in 0..300 {
                s.step(&[pad(0)]);
                let Some(g) = s.stickies.first() else { break };
                low = low.min(g.pos.y);
                if !g.moving {
                    break;
                }
            }
            assert!(
                low > FLOOR,
                "winding {winding} bf2 {bf2} from y {}: under the floor, lowest {low}",
                start.y
            );
        }
    }
}

/// A unit with one weapon retains its channels, held slots and ordnance.
#[test]
fn a_one_weapon_unit_has_no_channels() {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    for _ in 0..10 {
        s.step(&[pad(buttons::SECONDARY_FIRE | buttons::SWITCH_WEAPON)]);
    }
    let u = s.unit(p).unwrap();
    assert!(u.held.is_empty() && u.cur1.is_none() && u.active_channel == 0);
    assert!(s.stickies.is_empty());
}

/// Issue #29: a charge weapon fires the stage its trigger reached. A tap
/// fires the root's round; held past the stage's threshold (charge =
/// pressed / MaxPressedTime), the stage's own class, which its round
/// carries.
#[test]
fn a_held_charge_fires_its_stage_class() {
    use crate::weapon::{ChargeStage, WeaponClass};
    for (hold, want_stage) in [(2u32, false), (40, true)] {
        let mut s = Sim::empty(5);
        let mut root = WeaponClass::bf1_rifle();
        root.max_pressed_time = 1.0;
        let mut heavy = root.clone();
        heavy.ordnance.velocity = 600.0;
        let stage = s.add_weapon_class(heavy.clone());
        root.stages = vec![ChargeStage {
            threshold: 0.5,
            class: Box::new(heavy),
            index: stage,
        }];
        let wc = s.add_weapon_class(root);
        let me = s
            .spawn(&UnitSpec {
                weapon_class: wc,
                ..UnitSpec::player_in_slot(0, 0, Vec3::ZERO)
            })
            .unwrap();
        let _ = me;
        let mut fired = Vec::new();
        for t in 0..hold + 10 {
            s.step(&[pad(if t < hold { buttons::FIRE } else { 0 })]);
            fired.extend(s.events.iter().filter_map(|e| match e {
                Event::Shot { weapon_class, .. } => Some(*weapon_class as usize),
                _ => None,
            }));
        }
        assert_eq!(fired.len(), 1, "hold {hold}: one shot on release");
        assert_eq!(fired[0], if want_stage { stage } else { wc }, "hold {hold}");
        let speed = s.bolts.first().map(|b| b.vel.length());
        if want_stage {
            assert!(speed.is_none_or(|v| (v - 600.0).abs() < 1.0), "{speed:?}");
        }
    }
}

/// Issue #29, the shell (`analysis/re-ordnance-kinds.md` §6): it falls at
/// Gravity x 9.8 and meets the world on its arc - fired level from the
/// muzzle it lands where its arc meets the floor, not on its straight
/// line's end; out of life in the air it bursts with its ExplosionExpire.
#[test]
fn a_shell_falls_on_its_arc_and_bursts_when_its_life_runs_out() {
    use crate::weapon::{ExplosionClass, OrdKind, WeaponClass};
    let shell = |life: f32| {
        let mut w = WeaponClass::bf1_rifle();
        let o = &mut w.ordnance;
        (o.kind, o.gravity, o.velocity, o.life_span) = (OrdKind::Shell, 1.75, 40.0, life);
        o.expire = Some(ExplosionClass {
            damage: 100.0,
            damage_inner: 2.0,
            damage_outer: 4.0,
            ..Default::default()
        });
        w
    };
    let flat = || crate::heightfield::Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    };
    // Level: the first bolt's impact on the floor.
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat());
    let wc = s.add_weapon_class(shell(3.0));
    let me = s
        .spawn(&UnitSpec {
            weapon_class: wc,
            ..UnitSpec::player_in_slot(0, 0, Vec3::ZERO)
        })
        .unwrap();
    let mut landed = None;
    for t in 0..60 {
        s.step(&[pad(if t == 0 { buttons::FIRE } else { 0 })]);
        for e in &s.events {
            if let Event::BoltExpired { pos, struck: true } = e {
                landed.get_or_insert(*pos);
            }
        }
    }
    let feet = s.unit(me).unwrap().soldier.pos;
    let at = landed.expect("the shell lands");
    let run = Vec3::new(at.x - feet.x, 0.0, at.z - feet.z).length();
    println!("level shell lands {run:.1} m out at y {:.2}", at.y);
    assert!(at.y.abs() < 0.3, "on the floor: {at}");
    assert!(
        (10.0..25.0).contains(&run),
        "on its arc, not 120 m out: {run}"
    );
    // Up at 1 s of life: it bursts in the air over a soldier.
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat());
    let wc = s.add_weapon_class(shell(1.0));
    let me = s
        .spawn(&UnitSpec {
            weapon_class: wc,
            ..UnitSpec::player_in_slot(0, 0, Vec3::ZERO)
        })
        .unwrap();
    s.unit_mut(me).unwrap().soldier.pitch = 1.2;
    // Where a 40 m/s round at 1.2 rad is after 1 s of this fall.
    let (sp, cp) = (1.2f32.sin(), 1.2f32.cos());
    let burst = Vec3::new(0.0, 1.6 + 40.0 * sp - 0.5 * 17.15, -40.0 * cp);
    let under = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, burst.z)))
        .unwrap();
    let hp = s.unit(under).unwrap().health.cur;
    println!("burst near {burst}");
    for t in 0..45 {
        s.step(&[pad(if t == 0 { buttons::FIRE } else { 0 })]);
        // Held where the burst comes (a soldier on a ledge there).
        let u = s.unit_mut(under).unwrap();
        (u.soldier.pos, u.soldier.vel) = (burst - Vec3::Y, Vec3::ZERO);
    }
    assert!(s.bolts.is_empty(), "the shell is gone");
    let now = s.unit(under).unwrap().health.cur;
    assert!(now < hp, "the air burst hurt him: {hp} -> {now}");
}

/// The BF1 mortar launcher's classes as the disc authors them
/// (re-weapon-charge §3: a `grenade` class, ShotElevate 0.1,
/// MaxPressedTime 0.75; its `_ord1` shell 40 m/s, Gravity 1.75, 3 s).
fn mortar_shell() -> WeaponClass {
    let mut w = WeaponClass::bf1_rifle();
    w.name = "mortar".into();
    w.trigger_single = true;
    w.max_pressed_time = 0.75;
    (
        w.spread_per_shot,
        w.posture_spread,
        w.spread_threshold,
        w.spread_limit,
    ) = (0.0, [[0.0; 3]; 3], 0.0, 0.0);
    w.shot_delay = [1.4, 1.4];
    w.reload_time = 2.0;
    w.initial_salvo_delay = 0.3;
    w.ai_range = [10.0, 32.0, 48.0];
    w.thrown = Some(Thrown {
        shot_elevate: 0.1,
        hide_on_fire: true,
        hand: None,
    });
    w.ordnance = OrdnanceClass {
        velocity: 40.0,
        life_span: 3.0,
        max_damage: 150.0,
        kind: OrdKind::Shell,
        gravity: 1.75,
        ..OrdnanceClass::default()
    };
    w
}

/// Issue #29, the shell's launch (re-grenades §3.3, B1
/// `FUN_0028ef70` 215847-215862): the direction is the aimer's aim, with
/// `ShotElevate` added to y and renormalised for a HUMAN owner only - "a
/// bot's direction is used as it is" (§7.3). The mortar is a `grenade`
/// class and its shell leaves through the same launcher.
#[test]
fn a_shell_is_lifted_by_shot_elevate_for_a_human_only() {
    let spec = ShotSpec {
        tilt: (0.0, 0.0),
        charge: 0.0,
        stage: 0,
        pitch_spread: 0.0,
        yaw_spread: 0.0,
    };
    let dir = |human: bool| {
        let mut s = Sim::empty(3);
        s.heightfield = Some(flat_ground());
        let wc = s.add_weapon_class(mortar_shell());
        // The SAME bot path both ways (the muzzle and the aim frame);
        // only the owner's player flag differs, which is what the PS2's
        // launcher asks (a proxy human is no slotted player either).
        let u = s
            .spawn(&UnitSpec {
                controller: crate::unit::Controller::Ai,
                weapon_class: wc,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        s.units[u.index as usize].is_player = human;
        s.units[u.index as usize].soldier.pitch = 0.0;
        s.soldier_shots(u.index as usize, None, &[spec]);
        s.events
            .iter()
            .find_map(|e| match *e {
                Event::Shot { dir, .. } => Some(dir),
                _ => None,
            })
            .expect("the shell left")
    };
    // normalise((0, 0, -1) + (0, 0.1, 0)): y = 0.1 / sqrt(1.01).
    let want = 0.1 / 1.01f32.sqrt();
    let human = dir(true);
    assert!((human.length() - 1.0).abs() < 1e-5, "{human}");
    assert!(
        (human.y - want).abs() < 1e-4,
        "a human's shell carries ShotElevate: {human} against {want}"
    );
    // The same class in a bot's hands: the aim, no lift.
    let bot = dir(false);
    assert!(bot.y.abs() < 1e-6, "a bot's shell is not lifted: {bot}");
}

/// Issue #29: the mortar's charge-stage swap still separates the two
/// ordnance classes - a tap fires the root's `_ord1` SHELL, a hold past
/// 0.9 x MaxPressedTime fires the stage's `_ord` sticky
/// (`analysis/re-ordnance-kinds.md` §6, `FUN_00285108`).
#[test]
fn the_mortar_taps_a_shell_and_holds_a_sticky() {
    use crate::weapon::ChargeStage;
    for (hold, shell) in [(1u32, true), (40, false)] {
        let mut s = Sim::empty(5);
        s.heightfield = Some(flat_ground());
        let mut stage = mortar_shell();
        stage.ordnance = OrdnanceClass {
            velocity: 40.0,
            life_span: 0.5,
            ..thermal(false).ordnance
        };
        assert!(stage.ordnance.sticky.is_some());
        let stage_index = s.add_weapon_class(stage.clone());
        let mut root = mortar_shell();
        root.stages = vec![ChargeStage {
            threshold: 0.9,
            class: Box::new(stage),
            index: stage_index,
        }];
        let wc = s.add_weapon_class(root);
        let p = s
            .spawn(&UnitSpec {
                weapon_class: wc,
                ..UnitSpec::player_in_slot(0, 0, Vec3::ZERO)
            })
            .unwrap();
        s.units[p.index as usize].soldier.pitch = 0.0;
        let (mut shots, mut thrown) = (Vec::new(), Vec::new());
        for t in 0..hold + 12 {
            s.step(&[pad(if t < hold { buttons::FIRE } else { 0 })]);
            for e in &s.events {
                match *e {
                    Event::Shot { weapon_class, .. } => shots.push(weapon_class as usize),
                    Event::Throw { weapon_class, .. } => thrown.push(weapon_class as usize),
                    _ => {}
                }
            }
        }
        assert_eq!(
            shots.len() + thrown.len(),
            1,
            "hold {hold}: {shots:?} {thrown:?}"
        );
        if shell {
            assert_eq!((shots[0], thrown.len()), (wc, 0), "a tap fires the shell");
        } else {
            assert_eq!(
                (thrown[0], shots.len()),
                (stage_index, 0),
                "a hold fires the sticky stage"
            );
        }
    }
}
