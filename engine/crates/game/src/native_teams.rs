//! Build native conquest rosters and connect their owned and allied command-post paths.

use crate::walk::WalkSim;

const NATIVE_SIDES: [&str; 6] = ["ewk", "wok", "gun", "geo", "des", "gam"];

pub fn native_side(
    classes_dir: &std::path::Path,
    game: &str,
    class: &str,
) -> Option<(&'static str, std::path::PathBuf)> {
    NATIVE_SIDES.iter().find_map(|&side| {
        let path = classes_dir.join(format!("{game}_{side}.ron"));
        if !pak::is_file(&path) {
            return None;
        }
        let mut db = crate::classdb::ClassDb::default();
        db.load(&path).ok()?;
        db.get(class).is_some().then_some((side, path))
    })
}

/// Every native side the bake carries supplies its class file and models.
/// Older bakes can still launch their missions before a native side has been
/// baked.
pub fn install(
    walk: &mut WalkSim,
    layout: &crate::layout::MapLayout,
    classes_dir: &std::path::Path,
    assets_dir: &std::path::Path,
    mission: &crate::mission::MissionDef,
    world: &crate::assets::Manifest,
) {
    if mission.local_teams.is_empty() {
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
        let Some(team) = authored.team.checked_sub(1).filter(|team| *team >= 2) else {
            continue;
        };
        if walk.native_teams.contains_key(&team) {
            continue;
        }
        let Some((faction, source)) = native_side(classes_dir, &mission.source_game, first) else {
            eprintln!(
                "native team {} unavailable (no baked native side carries {first})",
                authored.team
            );
            continue;
        };
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
        // roster's fill class takes the remainder here too. PROVISIONAL,
        // replace: trace the BF1 quota top-up and assigned-member class draws
        // across native waves (re-ai-class-selection.md).
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
        let posts = if faction == "gam" {
            native_spawn_posts(world, layout, mission, authored.team)
        } else {
            Vec::new()
        };
        walk.sim.add_native_team_with_posts(team, members, posts);
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
            let path = def.ally_path.as_deref().and_then(|name| layout.path(name));
            Some(core::sim::AllySpawn {
                post,
                count: def.ally_count?,
                nodes: path
                    .into_iter()
                    .flat_map(|path| path.nodes.iter().map(|node| (node.pos, node.yaw)))
                    .collect(),
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

/// Jabba's invisible NPC posts own spawn paths and capacity without a capture
/// region. They stay outside ObjectiveConquest's post roster and HUD markers.
fn native_spawn_posts(
    world: &crate::assets::Manifest,
    layout: &crate::layout::MapLayout,
    mission: &crate::mission::MissionDef,
    team: u8,
) -> Vec<core::sim::NativeSpawnPost> {
    use crate::layout::{PROP_ALLY_COUNT, PROP_CAPTURE_REGION, PROP_SPAWN_PATH, PROP_TEAM};
    world
        .instances
        .iter()
        .filter_map(|inst| {
            if !inst
                .class
                .eq_ignore_ascii_case("com_bldg_invisible_controlzone")
                || mission
                    .killed
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(&inst.name))
            {
                return None;
            }
            let prop = |key| {
                inst.props
                    .iter()
                    .rev()
                    .find(|p| p.0 == key)
                    .map(|p| p.1.as_str())
            };
            if prop(PROP_TEAM)?.trim().parse::<u8>().ok()? != team
                || prop(PROP_CAPTURE_REGION).is_some()
            {
                return None;
            }
            let path = layout.path(prop(PROP_SPAWN_PATH)?)?;
            Some(core::sim::NativeSpawnPost {
                count: prop(PROP_ALLY_COUNT)?.trim().parse().ok()?,
                nodes: path.nodes.iter().map(|node| (node.pos, node.yaw)).collect(),
            })
        })
        .collect()
}
