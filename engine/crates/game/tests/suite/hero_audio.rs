//! Hero sound properties must reach real decoded takes and event consumers.

use core::sim::{heroes::Heroes, Event, PadState, Sim};
use game::{
    classdb::ClassDb,
    roster::{ClassRoster, FactionRoster},
    sounds::SoundLib,
    walk::{ClassVisual, TeamTemplate, WalkSim},
};
use glam::Vec3;
use std::path::{Path, PathBuf};

fn content() -> Option<(PathBuf, ClassDb)> {
    let root = game::assets::baked_root();
    if !root.join("bf2_tat3/sounds/sounds.ron").is_file()
        || !root.join("classes/bf2_rep.ron").is_file()
    {
        eprintln!("SKIP: actual BF2 hero classes and Mos Eisley sounds required");
        return None;
    }
    assert!(
        audio::Audio::backend_compiled(),
        "real decoding is required"
    );
    let db =
        game::roster::load_classdb(&root.join("classes"), "bf2", &["all", "imp", "rep", "cis"])
            .unwrap();
    Some((root, db))
}

fn library(root: &Path) -> SoundLib {
    SoundLib::load(&root.join("bf2_tat3")).expect("real map sound bank")
}

fn starts(lib: &mut SoundLib, name: &str) -> Vec<game::sounds::Start> {
    assert!(lib.has(name), "missing definition {name}");
    let mut out = Vec::new();
    lib.start(
        game::classdb::fnv1a_lowercase(name),
        Vec3::ZERO,
        Vec3::ZERO,
        &mut out,
    );
    for _ in 0..20 {
        lib.tick(0.1, Vec3::ZERO, &mut out);
    }
    assert!(!out.is_empty(), "no real take for {name}");
    for take in &out {
        let decoded = audio::decode(&take.path)
            .unwrap_or_else(|e| panic!("{name} {}: {e}", take.path.display()));
        assert!(
            decoded.frames() > 0 && decoded.sample_rate() > 0,
            "empty take {name}"
        );
        assert!(take.gain > 0.0, "inaudible request {name}");
        println!(
            "hero_audio_take {}",
            serde_json::json!({"name":name,"path":take.path,"frames":decoded.frames(),"rate":decoded.sample_rate(),"gain":take.gain,"pitch":take.pitch})
        );
    }
    out
}

/// The original GLOBAL stream index omits these authored references.
/// Keep the exact data assertion: another missing take is still a failure.
fn original_cut(lib: &SoundLib, name: &str) -> bool {
    let segments = match name {
        "hero_mundi_spawn" => vec![3_798_923_321, 3_748_590_464],
        "hero_mundi_exhausted" => vec![1_735_526_447],
        _ => return false,
    };
    assert_eq!(
        lib.audio_of_name(name),
        Some((Some(4_268_666_432), segments.clone(), vec![], vec![])),
        "original Mundi property changed; recheck source content"
    );
    for nth in 0..segments.len() {
        assert!(lib.take_path(name, nth).is_none());
    }
    println!(
        "hero_original_cut {}",
        serde_json::json!({"name":name,"stream":4_268_666_432u32,"segments":segments})
    );
    true
}

#[test]
fn every_actual_hero_sound_has_decodable_content_or_an_exact_original_cut() {
    let Some((root, db)) = content() else { return };
    let mut names: Vec<_> = db
        .names()
        .filter(|n| {
            n.split('_').nth(1) == Some("hero")
                && game::roster::FACTION_ORDER.contains(&n.split('_').next().unwrap_or(""))
        })
        .collect();
    names.sort();
    assert_eq!(names.len(), 22);
    let mut cuts = Vec::new();
    for name in names {
        let class = ClassRoster::build(&db, name, 0, Some(1)).unwrap();
        let hero = &class.cries.hero;
        for (role, sound) in [
            ("offer", &hero.offer),
            ("spawn", &hero.spawned),
            ("defeat", &hero.defeated),
            ("hum", &hero.saber_ambient),
            ("on", &hero.saber_on),
            ("off", &hero.saber_off),
            ("flight", &hero.saber_flight),
            ("hurt", &class.cries.hurt),
            ("death", &class.cries.death),
            ("breath", &class.cries.low_health),
        ] {
            if let Some(sound) = sound {
                println!("hero_audio_property {name} {role} {sound}");
                let mut lib = library(&root);
                if original_cut(&lib, sound) {
                    cuts.push(sound.clone());
                } else {
                    starts(&mut lib, sound);
                }
            }
        }
        let music = hero.music.as_ref().unwrap();
        let lib = library(&root);
        let track = lib
            .music(music)
            .unwrap_or_else(|| panic!("{name}: missing music {music}"));
        assert!(!track.takes.is_empty(), "{name}: no music takes");
        for take in &track.takes {
            assert!(take.frames() > 0 && take.frequency > 0);
            let mut reader = audio::TakeReader::open(take.clone()).unwrap();
            let mut decoded = 0;
            let mut nonzero = false;
            loop {
                let block = reader.next_block().unwrap();
                if block.is_empty() {
                    break;
                }
                decoded += block.len();
                assert!(block.iter().flatten().all(|s| s.is_finite()));
                if block.iter().flatten().any(|s| *s != 0.0) {
                    nonzero = true;
                    break;
                }
            }
            assert!(nonzero, "{name}: silent music take {}", take.path.display());
            println!(
                "hero_music_take {}",
                serde_json::json!({"hero":name,"name":music,"path":take.path,"offset":take.offset,"frames":take.frames(),"decoded_before_signal":decoded,"channels":take.channels,"rate":take.frequency})
            );
        }
    }
    cuts.sort();
    assert_eq!(cuts, ["hero_mundi_exhausted", "hero_mundi_spawn"]);
}

#[test]
fn every_actual_saber_combo_hit_and_deflect_sound_reaches_the_event_consumer() {
    fn names(fields: &[manifest::combo::ComboField], out: &mut std::collections::BTreeSet<String>) {
        for field in fields {
            if field.keyword == manifest::combo::ComboKeyword::Sound {
                out.insert(field.text(0).unwrap().to_owned());
            }
            names(&field.children, out);
        }
    }
    let Some((root, db)) = content() else { return };
    let mut tested = 0;
    for name in db.names().filter(|n| {
        n.split('_').nth(1) == Some("hero")
            && game::roster::FACTION_ORDER.contains(&n.split('_').next().unwrap_or(""))
    }) {
        let roster = ClassRoster::build(&db, name, 0, Some(1)).unwrap();
        let Some(saber) = &roster.weapon.saber else {
            continue;
        };
        let mut sounds = std::collections::BTreeSet::new();
        names(&saber.combo.fields, &mut sounds);
        sounds.extend(
            [&saber.hit_sound, &saber.deflect_sound]
                .into_iter()
                .flatten()
                .cloned(),
        );
        assert!(!sounds.is_empty(), "{name}");
        let mut walk = battle(&db, name);
        walk.sim.tick = 2;
        let weapon = walk.sim.units[walk.player.index as usize].weapon_class;
        let census = game::event_sounds::sound_names(&root.join("bf2_tat3"), &walk);
        for sound in sounds {
            let hash = game::classdb::fnv1a_lowercase(&sound);
            assert_eq!(census.get(&hash), Some(&sound));
            walk.sim.events = vec![Event::ComboSound {
                unit: walk.player,
                weapon_class: weapon as u32,
                sound_hash: hash,
            }];
            let mut played = Vec::new();
            game::event_sounds::each(&walk, Vec3::ZERO, |name, pos| {
                played.push((name.to_owned(), pos))
            });
            assert_eq!(
                played,
                [(
                    sound.clone(),
                    walk.sim.unit(walk.player).unwrap().soldier.pos
                )]
            );
            starts(&mut library(&root), &sound);
        }
        tested += 1;
    }
    assert_eq!(tested, 15);
}

fn battle(db: &ClassDb, name: &str) -> WalkSim {
    let roster = FactionRoster {
        faction: name.split('_').next().unwrap().into(),
        classes: vec![ClassRoster::build(db, name, 0, Some(1)).unwrap()],
    };
    let mut sim = Sim::empty(52);
    sim.ground_rules = core::soldier::GroundRules::Bf2;
    let registered = roster.classes[0].register(&mut sim, &mut Vec::new());
    sim.classes = [vec![registered.clone()], vec![registered.clone()]];
    let template = TeamTemplate::from_roster(
        &roster,
        &registered,
        vec![ClassVisual::from_roster(&roster.classes[0])],
    );
    sim.heroes = Some(Heroes::new([Some(0), None]));
    let floor = core::heightfield::Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    };
    WalkSim::with_teams(
        sim,
        Some(floor),
        Vec3::ZERO,
        [template.clone(), template],
        None,
    )
}

#[test]
fn actual_saber_sound_edges_follow_visibility_death_and_generation() {
    let Some((root, db)) = content() else { return };
    let mut walk = battle(&db, "rep_hero_obiwan");
    let mut lib = library(&root);
    let audio = walk.teams[0].class(0).cries.hero.clone();
    let on = audio.saber_on.unwrap();
    let off = audio.saber_off.unwrap();
    let hum = audio.saber_ambient.unwrap();
    let edges = lib.hero_blade_changes(&walk);
    assert_eq!(
        edges.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
        [on.as_str()]
    );
    starts(&mut lib, &edges[0].0);
    assert!(lib.hero_blade_changes(&walk).is_empty());
    assert!(game::event_sounds::hero_sample_asks(&walk)
        .iter()
        .any(|(_, n, _)| n == &hum));
    let i = walk.player.index as usize;
    walk.sim.units[i]
        .weapon
        .melee
        .get_or_insert_with(Default::default)
        .hidden_edges = u8::MAX;
    assert_eq!(lib.hero_blade_changes(&walk)[0].0, off);
    assert!(!game::event_sounds::hero_sample_asks(&walk)
        .iter()
        .any(|(_, n, _)| n == &hum));
    walk.sim.units[i]
        .weapon
        .melee
        .as_mut()
        .unwrap()
        .hidden_edges = 0;
    assert_eq!(lib.hero_blade_changes(&walk)[0].0, on);
    walk.sim.units[i].gen += 1;
    let edges = lib.hero_blade_changes(&walk);
    assert_eq!(
        edges.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
        [off.as_str(), on.as_str()]
    );
    let new = walk.sim.handle(i);
    assert!(game::event_sounds::hero_sample_asks(&walk)
        .iter()
        .all(|(h, _, _)| *h == new));
    walk.sim.units[i].alive = false;
    assert_eq!(lib.hero_blade_changes(&walk)[0].0, off);
    assert!(game::event_sounds::hero_sample_asks(&walk).is_empty());
    assert!(lib.hero_blade_changes(&walk).is_empty());
}

#[test]
fn actual_throw_sound_follows_the_paid_flight_and_ends_at_catch() {
    let Some((root, db)) = content() else { return };
    let mut walk = battle(&db, "rep_hero_obiwan");
    let i = walk.player.index as usize;
    let slot = walk.sim.units[i]
        .held
        .iter()
        .position(|w| walk.sim.weapon_classes[w.class].saber_throw.is_some())
        .unwrap();
    walk.sim.units[i].cur1 = Some(slot as u8);
    let sound = walk.teams[0]
        .class(0)
        .cries
        .hero
        .saber_flight
        .clone()
        .unwrap();
    let mut observed = false;
    let mut caught = false;
    for tick in 0..180 {
        let pad = PadState {
            buttons: if tick < 30 {
                core::sim::buttons::SECONDARY_FIRE
            } else {
                0
            },
            ..Default::default()
        };
        walk.step_tick(&[pad]);
        let flight = walk.sim.units[i].held[slot].state.saber_flight.as_ref();
        let asks = game::event_sounds::hero_sample_asks(&walk);
        let request = asks.iter().find(|(_, n, _)| n == &sound);
        if let Some(flight) = flight {
            assert_eq!(request.unwrap().2, flight.pos);
            if !observed {
                starts(&mut library(&root), &sound);
            }
            observed = true;
        } else if observed {
            assert!(request.is_none());
            caught = true;
            break;
        }
    }
    assert!(
        observed && caught,
        "actual input must launch and catch the saber"
    );
}

#[test]
fn actual_hero_music_binds_starts_and_releases_on_death() {
    let Some((root, db)) = content() else { return };
    let mut walk = battle(&db, "rep_hero_obiwan");
    let name = walk.teams[0].class(0).cries.hero.music.clone().unwrap();
    let lib = library(&root);
    let mut music =
        game::music::BattleMusic::new(&game::mission::MusicCalls::default(), true, [150; 2]);
    music.set_hero_music(vec![(0, 0, name.clone())]);
    assert!(music.bind(&lib).is_empty(), "actual SoldierMusic must bind");
    let changes = music.frame(&walk.sim, walk.player, core::TICK_DT);
    assert!(changes
        .iter()
        .any(|c| matches!(c, core::music::Change::Started { music: 0, .. })));
    let track = music.track(0).unwrap();
    let tail = track.def.min_playback + track.def.fade_out + 2.0;
    walk.sim.units[walk.player.index as usize].alive = false;
    for _ in 0..(tail * core::TICK_HZ as f32).ceil() as usize {
        music.frame(&walk.sim, walk.player, core::TICK_DT);
    }
    assert!(music.player.current().is_none());
}

#[test]
fn actual_hero_events_queue_authored_spawn_and_defeat_without_inventing_an_offer() {
    let Some((root, db)) = content() else { return };
    let mut walk = battle(&db, "rep_hero_obiwan");
    let hero = walk.teams[0].class(0).cries.hero.clone();
    let mut lib = library(&root);
    let mut commander = game::announce::Commander::new(
        core::announcer::BattleLines::default(),
        ["rep", "cis"],
        0,
        Vec::new(),
        Vec::new(),
    );
    assert!(
        hero.offer.is_none(),
        "the actual class authors no offer sound"
    );
    walk.sim.events = vec![Event::HeroOffered {
        unit: walk.player,
        class: 0,
    }];
    commander.calls(&walk.sim, &walk.teams);
    assert!(commander
        .queue
        .step(0.0, |_| panic!("no authored offer voice"))
        .is_none());
    let mut wait = 0.0;
    for (event, expected) in [
        (Event::Respawn { unit: walk.player }, hero.spawned.unwrap()),
        (
            Event::EnvDeath {
                unit: walk.player,
                cause: core::sim::DeathCause::Fall,
            },
            hero.defeated.unwrap(),
        ),
    ] {
        if matches!(event, Event::EnvDeath { .. }) {
            walk.sim.units[walk.player.index as usize].alive = false;
        }
        walk.sim.events = vec![event];
        commander.calls(&walk.sim, &walk.teams);
        let mut played_secs = 0.0;
        let line = commander.queue.step(wait, |line| {
            let (path,gain,secs)=lib.voice_take(&line.name).expect("queued real hero take");
            let data=audio::decode(&path).unwrap();
            assert!(data.frames()>0 && gain>0.0 && secs>0.0);
            println!("hero_voice_take {}",serde_json::json!({"name":line.name,"path":path,"frames":data.frames(),"secs":secs,"gain":gain}));
            played_secs=secs;
            Some(secs)
        }).unwrap();
        assert_eq!(line.name, expected);
        assert!(!line.mode_3d);
        wait = played_secs + core::announcer::VOICE_GAP_SECS + 0.01;
    }
}

#[test]
fn actual_vader_breath_waits_for_its_take_continues_at_full_health_and_stops_at_death() {
    let Some((root, db)) = content() else { return };
    let mut walk = battle(&db, "imp_hero_darthvader");
    let i = walk.player.index as usize;
    let name = walk.teams[0].class(0).cries.low_health.clone().unwrap();
    let mut lib = library(&root);
    walk.sim.units[i].health.cur = 1.0;
    let asks = game::event_sounds::hero_sample_asks(&walk);
    assert!(asks.iter().any(|(_, n, _)| n == &name));
    let first = lib.hero_samples(&asks, Vec3::ZERO);
    assert_eq!(
        first.len(),
        1,
        "hum loops stay separate from the breath sample"
    );
    let data = audio::decode(&first[0].1.path).unwrap();
    assert!(data.frames() > 0 && first[0].1.gain > 0.0);
    println!(
        "hero_breath_take {}",
        serde_json::json!({"name":name,"path":first[0].1.path,"frames":data.frames(),"secs":data.duration_secs()})
    );
    assert!(lib.hero_samples(&asks, Vec3::ZERO).is_empty());
    lib.tick(
        data.duration_secs() / first[0].1.pitch + 0.1,
        Vec3::ZERO,
        &mut Vec::new(),
    );
    assert_eq!(lib.hero_samples(&asks, Vec3::ZERO).len(), 1);
    walk.sim.units[i].health.cur = walk.sim.units[i].health.max;
    let healthy = game::event_sounds::hero_sample_asks(&walk);
    assert_eq!(walk.teams[0].class(0).cries.low_threshold, 1.0);
    assert!(
        healthy.iter().any(|(_, n, _)| n == &name),
        "Vader authors threshold1.0, including full health"
    );
    assert!(lib.hero_samples(&healthy, Vec3::ZERO).is_empty());
    walk.sim.units[i].alive = false;
    assert!(game::event_sounds::hero_sample_asks(&walk).is_empty());
}

mod missions;

mod identity;

/// Darth Maul "felt missing" (the maintainer, 2026-10-30): every saber hero's
/// authored `HitEffect` and `DeflectEffect` spawn at the blade's contact, and
/// the thrown blade's `HitSound` plays at its victim (B2 FUN_00340c60,
/// FUN_0033e8b8, FUN_0031d688).
#[test]
fn every_actual_saber_contact_spawns_its_authored_effect_and_the_throw_its_sound() {
    let Some((root, db)) = content() else { return };
    let mut tested = 0;
    for name in db.names().filter(|n| {
        n.split('_').nth(1) == Some("hero")
            && game::roster::FACTION_ORDER.contains(&n.split('_').next().unwrap_or(""))
    }) {
        let roster = ClassRoster::build(&db, name, 0, Some(1)).unwrap();
        let Some(saber) = &roster.weapon.saber else {
            continue;
        };
        let hit = saber.hit_effect.clone().expect(name);
        let deflect = saber.deflect_effect.clone().expect(name);
        assert_eq!(hit, "com_sfx_sabrehit", "{name}");
        assert_eq!(deflect, "com_sfx_sabredeflect", "{name}");
        let mut walk = battle(&db, name);
        let weapon = walk.sim.units[walk.player.index as usize].weapon_class as u32;
        let pos = Vec3::new(3.0, 1.0, -2.0);
        for (is_deflect, effect) in [(false, &hit), (true, &deflect)] {
            let mut fx = game::fx::Fx::for_walk(&root, &root.join("bf2_cor1"), "bf2", &walk);
            walk.sim.events = vec![Event::SaberContact {
                pos,
                weapon_class: weapon,
                deflect: is_deflect,
            }];
            fx.ingest(&walk);
            let live = fx.live();
            assert_eq!(live.len(), 1, "{name}");
            assert_eq!(live[0].name(), Some(effect.as_str()), "{name}");
            assert_eq!(live[0].frame.pos, pos, "{name}");
        }
        walk.sim.events = vec![Event::SaberThrowHit {
            pos,
            weapon_class: weapon,
        }];
        let mut played = Vec::new();
        game::event_sounds::each(&walk, Vec3::ZERO, |name, at| {
            played.push((name.to_owned(), at))
        });
        assert_eq!(
            played,
            [(saber.hit_sound.clone().expect(name), pos)],
            "{name}"
        );
        tested += 1;
    }
    assert_eq!(tested, 15);
}

/// [ours] (the maintainer, 2026-10-30): "when you do an attack either primary
/// or secondary it won't let you turn". Every shipped saber class turns under
/// the stick and the mouse through a whole attack string, and through a block.
#[test]
fn every_actual_saber_human_turns_through_its_attacks_and_blocks() {
    let Some((_, db)) = content() else { return };
    let mut tested = 0;
    for name in db.names().filter(|n| {
        n.split('_').nth(1) == Some("hero")
            && game::roster::FACTION_ORDER.contains(&n.split('_').next().unwrap_or(""))
    }) {
        let roster = ClassRoster::build(&db, name, 0, Some(1)).unwrap();
        if roster.weapon.saber.is_none() {
            continue;
        }
        use core::sim::buttons;
        let mut walk = battle(&db, name);
        let player = walk.player;
        let yaw = |w: &WalkSim| w.sim.unit(player).unwrap().soldier.yaw;
        let mut locked_ticks = 0;
        for (press, tag) in [(buttons::FIRE, "fire"), (buttons::RELOAD, "block")] {
            for tick in 0..90u32 {
                let mouse = tick % 2 == 0;
                let pad = PadState {
                    buttons: if press == buttons::RELOAD || tick % 10 == 0 {
                        press
                    } else {
                        0
                    } | if mouse { buttons::MOUSE_AIM } else { 0 },
                    rx: if mouse { 0 } else { 20000 },
                    mx: if mouse { 4000 } else { 0 },
                    ..Default::default()
                };
                let before = yaw(&walk);
                walk.step_tick(&[pad]);
                let u = walk.sim.unit(player).unwrap();
                if u.soldier.look_frozen() {
                    continue;
                }
                let state = u.weapon.melee.as_ref().expect(name);
                locked_ticks += usize::from(state.input_lock & core::combo::TURN != 0);
                assert_ne!(yaw(&walk), before, "{name} {tag} tick {tick}");
            }
        }
        assert!(locked_ticks > 0, "{name}: nothing authored the lock");
        tested += 1;
    }
    assert_eq!(tested, 15);
}

/// "keep attacking" (the maintainer, 2026-10-30): with the blade in the air a
/// hero's other secondary still fires, as on the PS2, and he keeps turning. A
/// grounded press of the same power is the control.
#[test]
fn a_real_force_secondary_fires_while_the_real_saber_flies() {
    use core::sim::buttons;
    let Some((_, db)) = content() else { return };
    let mut tested = 0;
    for name in db.names().filter(|n| {
        n.split('_').nth(1) == Some("hero")
            && game::roster::FACTION_ORDER.contains(&n.split('_').next().unwrap_or(""))
    }) {
        let mut walk = battle(&db, name);
        let player = walk.player;
        let classes = &walk.sim.weapon_classes;
        let held = &walk.sim.unit(player).unwrap().held;
        let throw = held
            .iter()
            .position(|h| classes[h.class].saber_throw.is_some());
        let power = held
            .iter()
            .position(|h| classes[h.class].force.is_some() && h.channel == 1);
        let (Some(throw), Some(power)) = (throw, power) else {
            continue;
        };
        let press = |walk: &mut WalkSim, slot: usize, hold: usize, ticks: usize| -> bool {
            let mut fired = false;
            for t in 0..ticks {
                let u = walk.sim.unit_mut(player).unwrap();
                u.cur1 = Some(slot as u8);
                u.soldier.energy = u.consts.energy_bar;
                let bar = u.consts.energy_bar;
                walk.step_tick(&[PadState {
                    // Most powers fire at the trigger's release.
                    buttons: if t < hold { buttons::SECONDARY_FIRE } else { 0 },
                    ..Default::default()
                }]);
                fired |= walk.sim.unit(player).unwrap().soldier.energy < bar - 1.0;
                if slot == power && fired {
                    break;
                }
            }
            fired
        };
        // The target stands off the blade's line, inside the power's cone.
        let aside = |w: &mut WalkSim| {
            let u = w.sim.unit(player).unwrap();
            let (s, c) = (u.soldier.yaw.sin(), u.soldier.yaw.cos());
            let at = u.soldier.pos + Vec3::new(-s, 0.0, -c) * 5.0 + Vec3::new(c, 0.0, -s) * 3.0;
            w.sim
                .spawn(&core::unit::UnitSpec::trooper(1, at))
                .expect("a target");
        };
        // The control: the same press with the saber in hand.
        let control = {
            let mut grounded = battle(&db, name);
            aside(&mut grounded);
            for _ in 0..20 {
                grounded.step_tick(&[PadState::default()]);
            }
            press(&mut grounded, power, 20, 40)
        };
        if !control {
            // A power that needs more than this staging proves nothing here.
            continue;
        }
        // The line is clear of the aside target.
        for _ in 0..20 {
            walk.step_tick(&[PadState::default()]);
        }
        press(&mut walk, throw, 8, 12);
        let flying = |w: &WalkSim| {
            w.sim.unit(player).unwrap().held[throw]
                .state
                .saber_flight
                .is_some()
        };
        assert!(flying(&walk), "{name}: the throw never left");
        aside(&mut walk);
        let yaw = walk.sim.unit(player).unwrap().soldier.yaw;
        assert!(
            press(&mut walk, power, 20, 40),
            "{name}: the power stayed silent"
        );
        assert!(flying(&walk), "{name}: the test outlived the flight");
        walk.step_tick(&[PadState {
            rx: 20000,
            ..Default::default()
        }]);
        assert_ne!(walk.sim.unit(player).unwrap().soldier.yaw, yaw, "{name}");
        tested += 1;
    }
    assert!(tested >= 3, "only {tested} heroes showed a working control");
}
