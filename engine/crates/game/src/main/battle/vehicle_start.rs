//! CLI vehicle selection: exact classes first, preserving pad capacity and occupancy.

use anyhow::Result;
use core::vehicle::VehicleHandle;
use game::walk::WalkSim;

#[derive(Clone, Copy)]
enum Candidate {
    Live(VehicleHandle),
    Pad(usize, u8),
}

pub(super) fn seat_player_in_vehicle(walk: &mut WalkSim, query: &str) -> Result<()> {
    let q = query.trim().to_ascii_lowercase();
    let player = walk
        .sim
        .unit(walk.player)
        .ok_or_else(|| anyhow::anyhow!("no live player to seat"))?;
    anyhow::ensure!(
        player.mount.is_none() && player.droideka.is_none(),
        "player cannot enter a vehicle"
    );
    let player_team = player.team;
    let match_name = |class: u32| {
        let name = walk.vehicle_classes.get(class as usize)?;
        let lower = name.to_ascii_lowercase();
        (!q.is_empty() && lower.contains(&q)).then(|| (name.clone(), lower == q))
    };
    let mut candidates = Vec::new();
    for (h, v) in walk
        .sim
        .alive_vehicles()
        .filter(|(_, v)| !v.seats.is_empty())
    {
        if let Some((name, exact)) = match_name(v.class) {
            let friendly = v.team == Some(player_team) || v.home_team == Some(player_team);
            let neutral = v.team.is_none() && v.home_team.is_none();
            candidates.push(((exact, friendly, neutral, true), Candidate::Live(h), name));
        }
    }
    for (pi, pad) in walk.sim.vehicle_pads.iter().enumerate() {
        let owner = pad
            .def
            .cp
            .map_or(pad.team, |ci| walk.sim.cps.get(ci).and_then(|c| c.owner));
        for team in 0..2u8 {
            let Some(class) = pad
                .def
                .class_for(Some(team))
                .filter(|c| !c.spec.seats.is_empty())
            else {
                continue;
            };
            if let Some((name, exact)) = match_name(class.spec.class) {
                candidates.push((
                    (exact, team == player_team, owner == Some(team), false),
                    Candidate::Pad(pi, team),
                    name,
                ));
            }
        }
    }
    if candidates.iter().any(|(rank, ..)| rank.0) {
        candidates.retain(|(rank, ..)| rank.0);
    }
    candidates.sort_by_key(|(rank, ..)| std::cmp::Reverse(*rank));
    for (_, candidate, name) in candidates {
        let (h, rollback) = match candidate {
            Candidate::Live(h) => (h, None),
            Candidate::Pad(pi, team) => {
                let before = walk.sim.vehicle_pads[pi].clone();
                let Some(h) = walk.sim.spawn_from_pad(pi, Some(team)) else {
                    continue;
                };
                (h, Some((pi, before)))
            }
        };
        if walk.sim.enter_vehicle(walk.player, h, 0) {
            println!("vehicle: seated player in {name} (team {player_team})");
            return Ok(());
        }
        if let Some((pi, before)) = rollback {
            walk.sim.remove_vehicle(h);
            walk.sim.vehicle_pads[pi] = before;
        }
    }
    let mut available: Vec<_> = walk
        .sim
        .alive_vehicles()
        .map(|(_, v)| v.class)
        .chain(
            walk.sim
                .vehicle_pads
                .iter()
                .flat_map(|p| p.def.classes.iter().flatten().map(|c| c.spec.class)),
        )
        .filter_map(|c| walk.vehicle_classes.get(c as usize))
        .collect();
    available.sort_unstable();
    available.dedup();
    let list = if available.is_empty() {
        "none".into()
    } else {
        available
            .into_iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    };
    anyhow::bail!("no available vehicle matching '{query}'; classes on map: {list}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use core::vehicle_pad::{PadClass, PadDef};
    use glam::Vec3;

    fn spec(class: u32) -> VehicleSpec {
        VehicleSpec {
            kind: VehicleKind::Hover,
            class,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(30.0, 0.0, 40.0),
            yaw: 0.0,
            max_health: 1000.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        }
    }

    fn world() -> WalkSim {
        let mut w = WalkSim::new(42, Vec3::ZERO, None);
        w.vehicle_classes = vec!["rep_hover_tank_variant".into(), "rep_hover_tank".into()];
        w
    }

    fn pad(pos: Vec3) -> PadDef {
        let class = |id| {
            Some(PadClass {
                spec: spec(id),
                radius: 3.0,
                landed_height: None,
                vis_box: Some((Vec3::splat(-3.0), Vec3::splat(3.0))),
            })
        };
        PadDef {
            name: "shared pad".into(),
            pos,
            yaw: 0.0,
            cp: None,
            team: Some(1),
            spawn_time: 10.0,
            decay_time: 10.0,
            spawn_count: 1,
            classes: [class(1), class(0)],
        }
    }

    #[test]
    fn exact_pad_class_beats_a_live_substring() {
        let mut w = world();
        w.sim.spawn_vehicle(&VehicleSpec {
            pos: Vec3::splat(300.0),
            ..spec(0)
        });
        w.sim.add_vehicle_pad(pad(Vec3::ZERO));
        seat_player_in_vehicle(&mut w, "rep_hover_tank").unwrap();
        let (h, _) = w.sim.seat_of(w.player).unwrap();
        assert_eq!(w.sim.vehicle(h).unwrap().class, 1);
    }

    #[test]
    fn occupied_pad_is_not_duplicated_or_reteamed() {
        let mut w = world();
        let pi = w.sim.add_vehicle_pad(pad(Vec3::ZERO));
        w.sim.spawn_from_pad(pi, Some(1)).unwrap();
        assert!(seat_player_in_vehicle(&mut w, "rep_hover_tank").is_err());
        assert_eq!(w.sim.alive_vehicles().count(), 1);
        assert_eq!(w.sim.vehicle_pads[pi].team, Some(1));
        assert_eq!(w.sim.vehicle_pads[pi].vehicles.len(), 1);
    }

    #[test]
    fn blocked_candidate_falls_back_to_a_free_pad() {
        let mut w = world();
        let blocked = w.sim.add_vehicle_pad(pad(Vec3::ZERO));
        let free = w.sim.add_vehicle_pad(pad(Vec3::X * 100.0));
        w.sim.spawn_from_pad(blocked, Some(1)).unwrap();
        seat_player_in_vehicle(&mut w, "rep_hover_tank").unwrap();
        let (h, _) = w.sim.seat_of(w.player).unwrap();
        assert_eq!(w.sim.vehicle(h).unwrap().spawn_pad, Some(free as u32));
    }

    #[test]
    fn unknown_class_lists_available_names() {
        let mut w = world();
        w.sim.add_vehicle_pad(pad(Vec3::ZERO));
        let message = seat_player_in_vehicle(&mut w, "gunship")
            .unwrap_err()
            .to_string();
        assert!(message.contains("rep_hover_tank") && message.contains("rep_hover_tank_variant"));
        assert_eq!(w.sim.alive_vehicles().count(), 0);
    }

    #[test]
    fn vehicle_start_loads_real_pads_without_ai_or_a_mission() {
        let assets = game::assets::baked_root();
        let map = assets.join("bf2_geo1");
        if !map.join("manifest.ron").exists() {
            eprintln!("SKIP: no Geonosis bake");
            return;
        }
        let (scene, _) = game::assets::load_battle_scene(&map, "con").unwrap();
        let setup = crate::BattleSetup {
            seed: 42,
            difficulty: Default::default(),
            spawn: (0.0, 0.0),
            spawn_set: false,
            targets: false,
            ai: None,
            ai_cap: None,
            team_size_pct: None,
            tickets: 250,
            tickets_set: false,
            ticket_pct: 100,
            time_limit: 0,
            space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
            cheats: Default::default(),
            teams: None,
            classes_dir: assets.join("classes"),
            mission: None,
            vehicle: Some("hailfire".into()),
            mission_file: None,
            player_team: 0,
            heroes: Some(core::sim::heroes::HeroRules::default()),
            dlc_heroes: false,
            score_limit: core::sim::deathmatch::ASSAULT_SCORE_LIMIT,
        };
        let walk = super::super::make_walk(&scene, &setup).unwrap();
        let (h, _) = walk.sim.seat_of(walk.player).expect("player starts seated");
        let vehicle = walk.sim.vehicle(h).unwrap();
        assert_eq!(
            walk.vehicle_classes[vehicle.class as usize],
            "cis_tread_hailfire"
        );
        assert_eq!(walk.sim.alive_units().count(), 1, "no implicit AI wave");
    }
    #[test]
    fn vehicle_flag_seats_player_and_lists_available_on_miss() {
        let mut walk = WalkSim::new(42, glam::Vec3::ZERO, None);
        walk.vehicle_classes = vec![
            "cis_hover_hailfiretank".to_owned(),
            "rep_walk_atte".to_owned(),
        ];
        let spec = |class, pos, home_team| core::vehicle::VehicleSpec {
            kind: core::vehicle::VehicleKind::Hover,
            class,
            home_team,
            neutral_when_empty: true,
            spawn_pad: None,
            pos,
            yaw: 0.0,
            max_health: 1000.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![core::vehicle::SeatKind::Pilot],
        };
        walk.sim
            .spawn_vehicle(&spec(0, glam::Vec3::new(10.0, 0.0, 0.0), Some(1)));
        let atte = walk
            .sim
            .spawn_vehicle(&spec(1, glam::Vec3::new(20.0, 0.0, 0.0), Some(0)));

        let err = seat_player_in_vehicle(&mut walk, "gunship").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cis_hover_hailfiretank"), "{msg}");
        assert!(msg.contains("rep_walk_atte"), "{msg}");

        seat_player_in_vehicle(&mut walk, "atte").unwrap();
        let (seated, _) = walk
            .sim
            .seat_of(walk.player)
            .expect("player should be seated");
        assert_eq!(seated, atte);
    }
}
