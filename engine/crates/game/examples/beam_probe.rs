//! A seat's beam tick by tick (the IFT-X turret on BF2 Kashyyyk by
//! default): the player boards, turns while firing one shot, and each tick
//! prints the live beams (origin, direction, length, life left), the
//! impacts the tick raised and the effects they left alive:
//!
//!   beam_probe [mission] [class] [seat] [ticks]
//!
//! It also prints each seat's fire ring and every shot's origin in the
//! hover's frame; `PROBE_HOLD=n` holds fire n ticks, `PROBE_SECONDARY`
//! fires the second weapon.
#[path = "support/walker_battle.rs"]
mod walker_battle;
use anyhow::Result;
use core::sim::{buttons, Event, PadState};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mission = args.next().unwrap_or_else(|| "bf2_kas2_con_cw".into());
    let class = args
        .next()
        .unwrap_or_else(|| "rep_hover_fightertank".into());
    let seat: u8 = args.next().map_or(Ok(1), |s| s.parse())?;
    let ticks: u32 = args.next().map_or(Ok(30), |s| s.parse())?;
    let root = game::assets::baked_root();
    let (mut walk, scene, h) = walker_battle::board_vehicle(&root, &mission, &class, seat)?;
    let slot = h.index as usize;
    let down = PadState {
        ry: -12000,
        ..Default::default()
    };
    for _ in 0..30 {
        walk.step_tick(&[down]);
    }
    if let Some(setup) = walk
        .sim
        .seat_setup
        .get(walk.sim.vehicles[slot].class as usize)
        .and_then(Option::as_ref)
    {
        for (k, g) in setup.guns.iter().enumerate() {
            if let Some(g) = g {
                println!(
                    "seat {k}: fire_point {:?} pivot {:?}",
                    g.fire_point, g.pivot
                );
                for w in &g.weapons {
                    println!(
                        "   {} points {:?}",
                        walk.sim.weapon_classes[w.class].name, w.fire_points
                    );
                }
            }
        }
    }
    let mut fx = game::fx::Fx::for_walk(&root, &scene.dir, &scene.manifest.source_game, &walk);
    for k in 0..ticks {
        let held = std::env::var("PROBE_HOLD").map_or(2, |v| v.parse().unwrap_or(2));
        let fire = if std::env::var("PROBE_SECONDARY").is_ok() {
            buttons::SECONDARY_FIRE
        } else {
            buttons::FIRE
        };
        let pad = PadState {
            rx: 20000,
            buttons: if k < held { fire } else { 0 },
            ..Default::default()
        };
        walk.step_tick(&[pad]);
        fx.ingest_sim(&walk.sim);
        fx.update(core::TICK_DT);
        let impacts: Vec<_> = walk
            .sim
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Impact { pos, surface, .. } => Some((*pos, *surface)),
                _ => None,
            })
            .collect();
        let v = &walk.sim.vehicles[slot];
        for e in &walk.sim.events {
            if let Event::Shot {
                origin,
                weapon_class,
                dir,
                ..
            } = e
            {
                println!(
                    "{k:3} shot {} from {:.2?} along {:.3?} in the hover",
                    walk.sim.weapon_classes[*weapon_class as usize].name,
                    v.rot.inverse() * (*origin - v.pos),
                    v.rot.inverse() * *dir
                );
            }
        }
        let muzzle = v
            .guns
            .get(seat as usize)
            .and_then(Option::as_ref)
            .map(|g| g.aim);
        for b in &walk.sim.beams {
            println!(
                "{k:3} beam from {:.2?} ({:.2?} in the hover) dir {:.3?} len {:.1} life {:.2} follow {}",
                b.origin,
                v.rot.inverse() * (b.origin - v.pos),
                b.dir,
                b.len,
                b.life,
                b.follow
            );
        }
        let names: Vec<_> = fx.live().iter().map(|e| e.name().unwrap_or("?")).collect();
        println!(
            "{k:3} aim {muzzle:.3?} impacts {impacts:.2?} live {:?}",
            names
        );
    }
    Ok(())
}
