//! Build native conquest rosters and connect their allied command-post paths.

use crate::walk::WalkSim;

/// Endor and Kashyyyk supply complete native side assets. Older bakes can
/// still launch their missions before a native side has been baked.
pub fn install(
    walk: &mut WalkSim,
    layout: &crate::layout::MapLayout,
    classes_dir: &std::path::Path,
    assets_dir: &std::path::Path,
    mission: &crate::mission::MissionDef,
) {
    if mission.local_teams.is_empty() || mission.local_team_relations.is_empty() {
        return;
    }
    let side = assets_dir.join(format!("side_{}/locals", mission.source_game));
    let mut names: Vec<_> = walk
        .sim
        .weapon_classes
        .iter()
        .enumerate()
        .map(|(i, w)| (w.name.clone(), i))
        .collect();
    for authored in &mission.local_teams {
        let Ok(member_count) = u8::try_from(authored.units) else {
            eprintln!("native team {} has an oversized member pool", authored.team);
            continue;
        };
        if authored.classes.len() > usize::from(u8::MAX) + 1 {
            eprintln!("native team {} has too many classes", authored.team);
            continue;
        }
        let Some((first, _, _)) = authored.classes.first() else {
            continue;
        };
        let faction = match first.split('_').next() {
            Some(prefix @ ("ewk" | "wok")) => prefix,
            _ => continue,
        };
        let Some(team) = authored.team.checked_sub(1).filter(|team| *team >= 2) else {
            continue;
        };
        if walk.native_teams.contains_key(&team) {
            continue;
        }
        let source = classes_dir.join(format!("{}_{faction}.ron", mission.source_game));
        if !pak::is_file(&source) {
            continue;
        }
        let roster = (|| -> anyhow::Result<_> {
            let mut db = crate::roster::load_classdb(classes_dir, &mission.source_game, &[])?;
            db.load(&source)?;
            let classes = authored
                .classes
                .iter()
                .map(|(name, min, max)| crate::roster::ClassRoster::build(&db, name, *min, *max))
                .collect::<anyhow::Result<Vec<_>>>()?;
            anyhow::ensure!(
                classes
                    .iter()
                    .all(|class| class.geometry.as_ref().is_some_and(|name| {
                        pak::is_file(side.join("soldiers").join(format!("{name}.glb")))
                    })),
                "native soldier model missing"
            );
            Ok(crate::roster::FactionRoster {
                faction: faction.to_owned(),
                classes,
            })
        })();
        let roster = match roster {
            Ok(roster) => roster,
            Err(e) => {
                eprintln!("native team {} unavailable ({e:#})", authored.team);
                continue;
            }
        };
        let table: Vec<_> = roster
            .classes
            .iter()
            .map(|class| class.register(&mut walk.sim, &mut names))
            .collect();
        let visuals = roster
            .classes
            .iter()
            .map(crate::walk::ClassVisual::from_roster_with_held)
            .collect();
        let mut members = Vec::new();
        let cap = usize::from(member_count);
        for (i, class) in table.iter().enumerate() {
            for _ in 0..(class.min as usize).min(cap.saturating_sub(members.len())) {
                members.push(class.spec(
                    team,
                    glam::Vec3::ZERO,
                    0.0,
                    core::unit::Controller::Ai,
                    i as u8,
                    walk.sim.bot_skill(team),
                ));
            }
        }
        // The member pool is distinct from the authored minimums. The normal
        // roster's fill class takes the remainder here too.
        members.resize_with(cap, || {
            table[0].spec(
                team,
                glam::Vec3::ZERO,
                0.0,
                core::unit::Controller::Ai,
                0,
                walk.sim.bot_skill(team),
            )
        });
        walk.native_teams.insert(
            team,
            crate::walk::TeamTemplate::from_roster(&roster, &table[0], visuals),
        );
        walk.sim.add_native_team(team, members);
    }
    if walk.native_teams.is_empty() {
        return;
    }
    for &(from, to, friendly) in &mission.local_team_relations {
        if let (Some(from), Some(to)) = (from.checked_sub(1), to.checked_sub(1)) {
            if walk.native_teams.contains_key(&from) || walk.native_teams.contains_key(&to) {
                walk.sim.set_team_relation(from, to, friendly);
            }
        }
    }
    let ally_spawns = walk
        .sim
        .cps
        .iter()
        .enumerate()
        .filter_map(|(post, cp)| {
            let def = layout.cps.iter().find(|def| def.name == cp.def.name)?;
            let path = layout.path(def.ally_path.as_deref()?)?;
            Some(core::sim::AllySpawn {
                post,
                count: def.ally_count?,
                nodes: path.nodes.iter().map(|node| (node.pos, node.yaw)).collect(),
            })
        })
        .collect();
    walk.sim.set_ally_spawns(ally_spawns);
    walk.sim.populate_native_teams();
    for (&team, template) in &walk.native_teams {
        let alive = walk
            .sim
            .alive_units()
            .filter(|(_, u)| u.team == team)
            .count();
        println!(
            "native team {} ({}): {} on field",
            team + 1,
            template.faction,
            alive
        );
    }
}
