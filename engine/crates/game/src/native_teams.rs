//! Build native conquest rosters and connect their owned and allied command-post paths.

use crate::walk::WalkSim;

const NATIVE_SIDES: [&str; 8] = ["ewk", "wok", "gun", "geo", "des", "gam", "gar", "jed"];
const AUXILIARY_FACTION_SIDES: [&str; 4] = ["all", "imp", "rep", "cis"];

fn side_with_class(
    classes_dir: &std::path::Path,
    game: &str,
    class: &str,
    sides: &[&'static str],
) -> Option<(&'static str, std::path::PathBuf)> {
    sides.iter().find_map(|&side| {
        let path = classes_dir.join(format!("{game}_{side}.ron"));
        if !pak::is_file(&path) {
            return None;
        }
        let mut db = crate::classdb::ClassDb::default();
        db.load(&path).ok()?;
        db.get(class).is_some().then_some((side, path))
    })
}

pub fn native_side(
    classes_dir: &std::path::Path,
    game: &str,
    class: &str,
) -> Option<(&'static str, std::path::PathBuf)> {
    side_with_class(classes_dir, game, class, &NATIVE_SIDES)
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
        let campaign = mission.mode.eq_ignore_ascii_case("campaign");
        let main_side = assets_dir.join(format!("side_{}", mission.source_game));
        let found = native_side(classes_dir, &mission.source_game, first).or_else(|| {
            campaign.then(|| {
                side_with_class(
                    classes_dir,
                    &mission.source_game,
                    first,
                    &AUXILIARY_FACTION_SIDES,
                )
            })?
        });
        let Some((faction, source)) = found else {
            eprintln!(
                "native team {} unavailable (no baked native side carries {first})",
                authored.team
            );
            continue;
        };
        let roster = (|| -> anyhow::Result<_> {
            let side_filter: &[&str] = if AUXILIARY_FACTION_SIDES.contains(&faction) {
                &[faction]
            } else {
                &[]
            };
            let mut db =
                crate::roster::load_classdb(classes_dir, &mission.source_game, side_filter)?;
            db.load(&source)?;
            let mut classes = authored
                .classes
                .iter()
                .map(|(name, min, max)| crate::roster::ClassRoster::build(&db, name, *min, *max))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let map_dir = assets_dir.join(&mission.map);
            let has_droideka = classes.iter().any(|c| c.droideka.is_some());
            let droideka_bakes = has_droideka.then(|| {
                crate::droideka::bakes(&map_dir, &mission.source_game.to_ascii_lowercase())
            });
            anyhow::ensure!(
                classes.iter().all(|class| {
                    if class.droideka.is_some() {
                        return droideka_bakes
                            .as_ref()
                            .is_some_and(|b| b.contains_key(&class.class.to_ascii_lowercase()))
                            || (campaign
                                && pak::is_file(
                                    main_side
                                        .join("vehicles")
                                        .join(faction)
                                        .join("walkers")
                                        .join("cis_walk_droideka.glb"),
                                ));
                    }
                    class.geometry.as_ref().is_some_and(|name| {
                        pak::is_file(side.join("soldiers").join(format!("{name}.glb")))
                            || (campaign
                                && pak::is_file(
                                    main_side.join("soldiers").join(format!("{name}.glb")),
                                ))
                    })
                }),
                "native soldier model missing"
            );
            if has_droideka {
                crate::droideka::install_classes(&mut classes, &map_dir, &mission.source_game);
            }
            // `[ours]` (maintainer, 2026-11-20, `crate::jawa`): every Jawa
            // carries the arc caster; outside Hunt it is toned down.
            if classes.iter().any(crate::jawa::needs_arc_caster) {
                match crate::jawa::donor(classes_dir) {
                    Ok(donor) => {
                        for class in &mut classes {
                            if let Err(e) = crate::jawa::arm(&donor, class) {
                                eprintln!("native Jawa keeps its own kit ({e:#})");
                            }
                        }
                    }
                    Err(e) => eprintln!("native Jawa keeps its own kit (no BF2 arc caster: {e:#})"),
                }
            }
            if !mission.is_hunt() {
                for class in &mut classes {
                    crate::jawa::tame(class);
                }
            }
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
        let mut table: Vec<_> = roster
            .classes
            .iter()
            .map(|class| class.register(&mut walk.sim, &mut names))
            .collect();
        // `[ours]` (maintainer, 2026-11-19): Jabba's guards never capture,
        // neutralise or contest a command post. On the disc they can (the
        // guard class sets CapturePosts and a locals team friendly to
        // neither army keeps its own capture affiliation,
        // analysis/re-cp-capture.md §2-3). Only the Gamorreans: the Dune Sea
        // Tuskens retake their own posts to keep spawning (§6.1).
        if faction == "gam" {
            for class in &mut table {
                class.capture_posts = false;
            }
        }
        // `[ours]` (maintainer, 2026-11-20, `crate::jawa`): the Jawas are
        // friendly to both armies on both discs, so they never took a post;
        // fighting both armies now, they still take none.
        let jawas = roster
            .classes
            .iter()
            .any(|c| crate::jawa::is_jawa(&c.class));
        for (class, built) in table.iter_mut().zip(&roster.classes) {
            if crate::jawa::is_jawa(&built.class) {
                class.capture_posts = false;
            }
        }
        let custom = crate::animator::custom::for_mission(classes_dir, mission).unwrap_or_default();
        let visuals = roster
            .classes
            .iter()
            .map(|c| {
                let mut v = crate::walk::ClassVisual::from_roster_with_held(c);
                v.anim.custom = custom.clone();
                v
            })
            .collect();
        let templates: Vec<_> = table
            .iter()
            .enumerate()
            .map(|(i, class)| {
                class.spec(
                    team,
                    glam::Vec3::ZERO,
                    0.0,
                    core::unit::Controller::Ai,
                    i as u8,
                    walk.sim.bot_skill(team),
                )
            })
            .collect();
        let limits: Vec<(u32, u32)> = table
            .iter()
            .map(|class| (class.min, class.max.unwrap_or(u32::MAX)))
            .collect();
        let mut members = Vec::new();
        let mut counts = vec![0u32; table.len()];
        let mut unassigned = 0u32;
        // Each member joins its team in order (`FUN_0034cde0`): class 0
        // (`FUN_0034f1f8(m, 0)`), then its respawn preparation `FUN_0034ffe0`
        // draws with `FUN_0036f198`, counting the members before it and
        // itself as class 0, with the clock under 60 s. `SpawnCharacter`
        // (`FUN_0034e1f0`) spawns the member's class, so an ambush's mix is
        // its members' draws (re-ai-class-selection.md). PROVISIONAL, trace:
        // the vtable caller of `FUN_0034cde0` at level start.
        for _ in 0..usize::from(member_count) {
            let mut seen = counts.clone();
            if let Some(first) = seen.first_mut() {
                *first += 1;
            }
            let Some(k) = core::sim::draw_class(&mut walk.sim.lcg, &seen, &limits, true) else {
                unassigned += 1;
                continue;
            };
            counts[k] += 1;
            members.push(templates[k].clone());
        }
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
        walk.sim
            .set_native_chunk_deaths(team, table.iter().map(|c| c.chunk.clone()).collect());
        walk.sim.set_native_class_draw(
            team,
            core::sim::NativeClassDraw {
                templates,
                limits,
                counts,
                unassigned,
            },
        );
        // The palace script's `AddAIGoal(3, "Deathmatch", 100)`: the guards
        // go after a random foe of either army, not just the nearest
        // (maintainer, 2026-11-19: hostile to both armies; the census of
        // 2026-11-20 had 63 % of the guards' kills on one army in the
        // Clone Wars era with the nearest-enemy hunt).
        if faction == "gam" {
            walk.sim.set_native_deathmatch(team);
        }
        // Outside Hunt the Jawas fight both armies (`crate::jawa`); their
        // authored friendship still gives them the posts' ally paths.
        if jawas && !mission.is_hunt() {
            walk.sim.set_native_neutral(team);
        }
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
    if mission.campaign.is_none() {
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
