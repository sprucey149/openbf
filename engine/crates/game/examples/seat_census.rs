//! Every vehicle class's seats, and whether a soldier in each can be shot.
//!
//! ```text
//! seat_census census          the seat table of both games, as markdown
//! seat_census shoot <map> <faction> <faction>
//!                             real pads: a bolt at a rider in each kind of seat
//! ```
use core::collision::HullFilter;
use core::projectile::{Bolt, Shooter};
use core::sim::Sim;
use core::unit::UnitSpec;
use core::vehicle::SeatKind;
use game::classdb::ClassDb;
use game::vehicle_class::{self, GunnerDraw};
use glam::Vec3;
use std::collections::BTreeSet;

fn seat_name(k: SeatKind) -> String {
    match k {
        SeatKind::Pilot => "pilot".into(),
        SeatKind::Gunner(i) => format!("TURRET{}", i + 1),
        SeatKind::Passenger(i) => format!("passenger {}", i + 1),
    }
}

/// A map's game, by its manifest's `source_game` line.
fn source_game(map: &std::path::Path) -> String {
    let text = std::fs::read_to_string(map.join("manifest.ron")).unwrap_or_default();
    text.lines()
        .find_map(|l| l.trim().strip_prefix("source_game:"))
        .map_or(String::new(), |v| {
            v.trim().trim_matches(|c| c == ',' || c == '"').to_owned()
        })
}

fn census(game: &str) -> anyhow::Result<()> {
    let root = game::assets::baked_root();
    let mut db = ClassDb::default();
    let prefix = format!("{game}_");
    for e in std::fs::read_dir(root.join("classes"))? {
        let p = e?.path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name.ends_with(".ron") {
            db.load(&p)?;
        }
    }
    let mut maps = 0;
    for e in std::fs::read_dir(&root)? {
        let p = e?.path();
        if !p.join("classes.ron").exists() || !p.join("manifest.ron").exists() {
            continue;
        }
        let m = source_game(&p);
        if m.eq_ignore_ascii_case(game) {
            db.load_map(&p)?;
            maps += 1;
        }
    }
    let bf2 = game == "bf2";
    let mut classes: BTreeSet<String> = BTreeSet::new();
    for c in db.names() {
        if db
            .engine_base(c)
            .and_then(game::vehicle_pads::vehicle_kind)
            .is_some()
        {
            classes.insert(c.to_ascii_lowercase());
        }
    }
    println!("\n## {game} ({} classes, {maps} maps)\n", classes.len());
    println!("| class | kind | seat | pilot type | pose | IsPilotExposed | can be shot |");
    println!("|---|---|---|---|---|---|---|");
    let (mut seats, mut exposed, mut enclosed, mut none) = (0, 0, 0, 0);
    for c in &classes {
        let base = db.engine_base(c).unwrap_or("?").to_ascii_lowercase();
        let ring = vehicle_class::seats(&db, c);
        let setup = game::vehicle_pads::seat_setup(&db, c, &ring, game);
        for (i, s) in ring.iter().enumerate() {
            seats += 1;
            let shot = setup.body_active[i];
            let soldier = s.gunner_draw(bf2) != GunnerDraw::NoSoldier;
            match (soldier, shot) {
                (false, _) => none += 1,
                (true, true) => exposed += 1,
                (true, false) => enclosed += 1,
            }
            println!(
                "| {c} | {base} | {} | {:?} | {} | {} | {} |",
                seat_name(s.kind),
                s.pilot_type,
                s.pilot_pose.as_ref().map_or("-".into(), |p| p.clip()),
                u8::from(s.pilot_exposed),
                if !soldier {
                    "no soldier"
                } else if shot {
                    "yes"
                } else {
                    "no"
                },
            );
        }
    }
    println!("\n{seats} seats: {exposed} exposed, {enclosed} enclosed, {none} take no soldier.");
    Ok(())
}

fn shoot(map: &str, factions: [String; 2]) -> anyhow::Result<()> {
    let root = game::assets::baked_root();
    let (scene, _) = game::assets::load_battle_scene(&root.join(map), "con")?;
    let mut sim = Sim::empty(1);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    let team = game::walk::TeamTemplate::default_trooper();
    let mut w = game::walk::WalkSim::with_teams(
        sim,
        scene.heightfield(),
        Vec3::ZERO,
        [team.clone(), team],
        None,
    );
    scene.install_collision(&mut w.sim);
    let build = game::vehicle_pads::install(
        &mut w.sim,
        &scene.manifest,
        &scene.dir,
        &root.join("classes"),
        &factions,
    )?;
    println!(
        "{map}: frames installed {}",
        w.sim.hero_rider_frames.is_some()
    );
    let wc = w
        .sim
        .add_weapon_class(core::weapon::WeaponClass::bf2_rifle());
    for _ in 0..30 {
        w.sim.step(&[]);
    }
    let mut done = BTreeSet::new();
    for vi in 0..w.sim.vehicles.len() {
        let v = &w.sim.vehicles[vi];
        if !v.alive {
            continue;
        }
        let vh = core::vehicle::VehicleHandle {
            index: vi as u32,
            gen: v.gen,
        };
        let class = v.class as usize;
        let name = build.classes[class].clone();
        let Some(setup) = w.sim.seat_setup[class].clone() else {
            continue;
        };
        let (pos, rot) = (v.pos, v.rot);
        for seat in 0..setup.boardable.len() {
            if !setup.boardable[seat] || !done.insert((name.clone(), setup.body_active[seat])) {
                continue;
            }
            let rider = w.sim.spawn(&UnitSpec::trooper(1, pos)).unwrap();
            if !w.sim.seat_unit(rider, vh, seat as u8) {
                continue;
            }
            let body = w
                .sim
                .hero_rider_frames
                .clone()
                .and_then(|f| f.frame(w.sim.vehicle(vh).unwrap(), seat as u8))
                .map_or(pos, |m| m.w_axis.truncate());
            // The rider's place, or his chest there, from straight above: a
            // ray down meets him before the hull under him where he sits out.
            let aim = body + rot * (Vec3::Y * 1.0);
            let from = aim + Vec3::Y * 8.0;
            let shooter = w.sim.spawn(&UnitSpec::player(0, from)).unwrap();
            let before = (
                w.sim.units[rider.index as usize].health.cur,
                w.sim.vehicle(vh).unwrap().health.cur,
            );
            w.sim.bolts.push(Bolt::fire(
                from,
                Vec3::NEG_Y,
                100.0,
                0.25,
                Shooter {
                    owner: shooter,
                    team: 0,
                    weapon_class: wc,
                    is_player: false,
                },
                w.sim.heightfield.as_ref(),
                w.sim.water_plane(),
                &w.sim.collision,
                HullFilter::All,
            ));
            for _ in 0..8 {
                w.sim.step(&[]);
            }
            println!(
                "{name} seat {seat} exposed={}: body {body} rider {} -> {}, hull {} -> {}",
                setup.body_active[seat],
                before.0,
                w.sim.units[rider.index as usize].health.cur,
                before.1,
                w.sim.vehicle(vh).map_or(-1.0, |v| v.health.cur),
            );
            if w.sim.units[rider.index as usize].alive {
                w.sim.exit_vehicle(rider);
            }
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("census") => {
            census("bf1")?;
            census("bf2")
        }
        Some("shoot") if a.len() == 4 => shoot(&a[1], [a[2].clone(), a[3].clone()]),
        _ => anyhow::bail!("seat_census census | shoot <map> <faction> <faction>"),
    }
}
