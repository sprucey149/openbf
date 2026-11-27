//! The battle every mode builds and shows: a baked map loaded and uploaded
//! with its common assets ([`load_and_upload`]), the two teams from the class
//! data, the `WalkSim` set up from the CLI or the shell ([`make_walk`]), and
//! what a frame draws of it besides the scene — the marker, target, bolt and
//! post boxes ([`walk_draws`]), the squad icons, and the HUD and spawn-screen
//! models the `ui` crate paints.

#[path = "battle/ctf_hud.rs"]
mod ctf_hud;
use ctf_hud::{hud_hints, marker_screen};
pub(super) use ctf_hud::{upload_world_marker_assets, world_objective_draws, WorldMarkerAssets};

#[path = "battle/vehicle_start.rs"]
mod vehicle_start;
use vehicle_start::seat_player_in_vehicle;

use super::BattleSetup;
use anyhow::Result;
use game::viewer::{upload_scene, SceneGpu};
use game::walk::WalkSim;
use render::{DrawCmd, MeshHandle, Renderer};

/// Walk-mode soldier marker tint (orange — reads on sand and rock).
const MARKER_COLOR: [f32; 4] = [0.9, 0.45, 0.1, 1.0];

#[cfg(test)]
pub(super) fn load_and_upload(
    r: &mut Renderer,
    dir: &std::path::Path,
    mode: &str,
    kills: &[String],
) -> Result<(SceneGpu, game::assets::Scene)> {
    load_and_upload_with_layers(r, dir, mode, &[], kills)
}

pub(super) fn load_and_upload_with_layers(
    r: &mut Renderer,
    dir: &std::path::Path,
    mode: &str,
    layers: &[String],
    kills: &[String],
) -> Result<(SceneGpu, game::assets::Scene)> {
    let (mut scene, load) = game::assets::load_battle_scene_with_layers(dir, mode, layers)?;
    scene.kill_objects(kills);
    let gpu = upload_battle_scene(r, &scene, &load)?;
    Ok((gpu, scene))
}

/// Event-thread half shared by direct launch and the shell's prepared scene.
pub(super) fn upload_battle_scene(
    r: &mut Renderer,
    scene: &game::assets::Scene,
    load: &game::assets::BattleLoad,
) -> Result<SceneGpu> {
    if let Some(layer) = &load.layer {
        println!("mode layer '{layer}' merged");
    }
    match &load.common {
        Some((cd, Ok(n))) => println!("common assets merged from {} (+{n} models)", cd.display()),
        Some((cd, Err(e))) => eprintln!("common assets at {} unreadable: {e:#}", cd.display()),
        None => {}
    }
    let gpu = upload_scene(r, scene)?;
    println!(
        "map '{}': {} draws ({} instances, {} skipped classes)",
        scene.manifest.map,
        gpu.draws.len(),
        scene.manifest.instances.len(),
        gpu.skipped.len()
    );
    for (class, n) in &gpu.skipped {
        println!("  skipped {n:3}x {class} (no drawable model)");
    }
    Ok(gpu)
}

/// The two squad-command icons (brief D11), both out of `common_bf1`: BF2
/// ships neither, so a BF2 map shows the BF1 icon — `PROVISIONAL (ours)` —
/// and a bake without them shows nothing at all.
///
/// **OFF in normal play** (card SQMK): loaded only when
/// `game::squad_vis::markers_on_env` says so — dev mode (on by default; `OPENBF_DEV=0` is off), or
/// `OPENBF_SQUAD_MARKER=1` for this overlay alone (`=0` hides it in dev mode).
/// The icon's shape is its disc alpha (`analysis/re-hud-icons.md` §2 [V] —
/// 1,594 of `squad_cmd_highlight`'s 4,096 texels at 0), which the bake keeps.
///
/// The PLACEMENT is the recorded PROVISIONAL deviation: the PS2 draws these
/// two sheets as its 2D D-pad command MENU at HUD (−120, −50) — the cross at
/// the centre, one highlight arm rotated four ways around it (§2 "Where") —
/// and never over a bot in the world. PLAN §DESIGN SPECS Phase D decision 3
/// named the sheets, not that placement; the in-world marker is ours.
pub(super) fn squad_icons(
    r: &mut render::Renderer,
    assets_dir: &std::path::Path,
) -> (Option<render::TextureHandle>, Option<render::TextureHandle>) {
    if !game::squad_vis::markers_on_env() {
        return (None, None);
    }
    let dir = assets_dir.join("common_bf1").join("textures");
    let load = |r: &mut render::Renderer, n: &str| {
        game::viewer::upload_texture_file(r, &dir, n).ok().flatten()
    };
    (load(r, "squad_cmd_highlight"), load(r, "squad_cmd_cross"))
}

fn marker_draw(cube: MeshHandle, walk: &WalkSim) -> DrawCmd {
    DrawCmd::flat(cube, MARKER_COLOR, walk.marker_transform())
}

/// Soldier marker + enemy boxes (magnet target highlighted red, health
/// darkens the box) + in-flight bolts (team-tinted).
pub fn walk_draws(
    cube: MeshHandle,
    walk: &WalkSim,
    draws: &mut Vec<DrawCmd>,
    visuals: Option<&game::soldier_vis::SoldierVisuals>,
) {
    let has_model = |team| visuals.is_some_and(|v| v.has_model(team));
    let player_u = &walk.sim.units[walk.player.index as usize];
    if walk.player_on_foot()
        && !walk.player_body_hidden()
        && !has_model(player_u.team)
        && !(visuals.is_some() && player_u.droideka.is_some())
    {
        draws.push(marker_draw(cube, walk));
    }
    for t in walk.target_draws() {
        if has_model(t.team) || (visuals.is_some() && t.droideka) {
            continue; // drawn as an animated model instead
        }
        let base = if t.magnet {
            [0.85, 0.15, 0.15]
        } else {
            [0.35, 0.35, 0.4]
        };
        let k = 0.4 + 0.6 * t.health_frac;
        draws.push(DrawCmd::flat(
            cube,
            [base[0] * k, base[1] * k, base[2] * k, 1.0],
            t.transform,
        ));
    }
    for (m, team) in walk.bolt_draws() {
        let color = if team == 0 {
            [1.0, 0.25, 0.1, 1.0]
        } else {
            [0.3, 1.0, 0.3, 1.0]
        };
        draws.push(DrawCmd::flat(cube, color, m));
    }
}

/// Team templates from the baked class data (factions default to the map's
/// source game); falls back to the built-in BF1 trooper with a warning.
fn team_templates(
    scene: &game::assets::Scene,
    b: &BattleSetup,
    sim: &mut core::sim::Sim,
) -> Result<[game::walk::TeamTemplate; 2]> {
    let hunt = b.mission.as_ref().filter(|m| m.is_hunt());
    let defaults = game::roster::default_factions(&scene.manifest.source_game);
    let factions: [&str; 2] = match &b.teams {
        Some(t) => [t[0].as_str(), t[1].as_str()],
        None => defaults,
    };
    match game::roster::FactionRoster::pair(
        &b.classes_dir,
        &scene.manifest.source_game,
        factions,
        b.mission.as_ref(),
    ) {
        Ok((mut rosters, warnings)) => {
            for w in &warnings {
                eprintln!("{w}");
            }
            if let Some(m) = b.mission.as_ref().filter(|m| m.is_hero_assault()) {
                for (team, roster) in rosters.iter_mut().enumerate() {
                    game::heroes::prepare_assault(roster, &b.classes_dir, m, team, b.dlc_heroes)?;
                }
            }
            game::droideka::install(&mut rosters, &scene.dir, &scene.manifest.source_game);
            // One `Sim::weapon_classes` index per weapon NAME (a Vec scan —
            // the sim never iterates a hash map), and the per-team class
            // TABLE the AI mix and the spawn-select both read.
            let mut names: Vec<(String, usize)> = Vec::new();
            let mut team = 0usize;
            let mut hero_classes = [None; 2];
            let templates = rosters.map(|mut r| {
                // Campaign scripts choose their heroes independently of the
                // Instant Action session's Heroes setting.
                if (b.heroes.is_some() || b.mission.as_ref().is_some_and(|m| m.is_campaign())) && hunt.is_none() {
                    hero_classes[team] = game::heroes::append(
                        &mut r,
                        &b.classes_dir,
                        b.mission.as_ref(),
                        team,
                    );
                }
                let mut table = Vec::new();
                let mut visuals = Vec::new();
                for c in &mut r.classes {
                    let uc = c.register(sim, &mut names);
                    // The speed the weapon fires at: a charge stage's, as
                    // `WeaponClass::shot_ordnance` takes it (the BF1 spear's
                    // root says 5 m/s, its throw leaves at 25), the root's
                    // beside it when they differ.
                    let fired = c.weapon.shot_ordnance();
                    let root = c.weapon.ordnance.velocity;
                    println!(
                        "team {} class {}: {} — min {} — {} HP, {:.1} m/s, {} ({} dmg, {} m/s {}{}){}",
                        r.faction,
                        c.class,
                        c.label,
                        c.min,
                        c.max_health,
                        c.consts.max_speed,
                        c.weapon.name,
                        // What a hit takes off a soldier.
                        fired.damage * fired.scales[0],
                        fired.velocity,
                        if c.weapon.thrown.is_some() || c.weapon.ordnance.sticky.is_some() {
                            "thrown"
                        } else {
                            "bolts"
                        },
                        if (fired.velocity - root).abs() > 1e-3 {
                            format!(", root {root} m/s")
                        } else {
                            String::new()
                        },
                        if c.jet { "  [jet pack]" } else { "" }
                    );
                    table.push(uc);
                    visuals.push(game::walk::ClassVisual::from_roster_with_held(c));
                }
                sim.classes[team] = table;
                team += 1;
                game::walk::TeamTemplate::from_roster(&r, &sim.classes[team - 1][0], visuals)
            });
            if let Some(mut rules) = b
                .heroes
                .or_else(|| {
                    b.mission
                        .as_ref()
                        .filter(|m| m.is_campaign())
                        .map(|_| Default::default())
                })
                .filter(|_| hunt.is_none())
            {
                if b.mission.as_ref().is_some_and(|m| m.is_campaign()) {
                    // Only the campaign's UnlockHeroForTeam enables an offer;
                    // PS2 campaign ruleset forbids AI candidates and hero respawn.
                    rules.unlock = core::sim::heroes::HeroUnlock::Points(i32::MAX);
                    rules.respawn_ticks = None;
                    rules.ai_candidates = false;
                }
                game::heroes::install_with(sim, hero_classes, rules);
            }
            Ok(templates)
        }
        Err(e) => {
            if hunt.is_some() {
                return Err(e.context("Hunt roster unavailable"));
            }
            if b.mission.as_ref().is_some_and(|m| m.is_hero_assault()) {
                return Err(e.context("Hero Assault roster unavailable"));
            }
            eprintln!("class data unavailable ({e:#}); using built-in BF1 troopers");
            let t = game::walk::TeamTemplate::default_trooper();
            Ok([t.clone(), t])
        }
    }
}

/// Card SSELA: the shell battle's player starts behind the spawn screens -
/// off the field until it accepts a side, a post and a class, and back after
/// a death only through the spawn map and the class accept
/// (`Sim::open_spawn_screens`). Every shell battle is Instant Action, where
/// the PS2 offers the side select ("can switch sides", re-spawn-flow §1), so
/// the screens open on it; the side the player accepts is the player's team
/// (`mission::pick`'s `player_team` only orders the armies). The CLI walk,
/// the headless tools and the golden scenes keep the auto-accept spawn. The
/// screens stand over the mission's camera shots until the first spawn (card SSELB,
/// `game::spawn_backdrop`).
pub(super) fn open_spawn_screens(walk: &mut WalkSim, mission: Option<&game::mission::MissionDef>) {
    let side_choice = mission.map_or(true, |m| m.allows_side_select());
    if !walk.sim.open_spawn_screens(0, side_choice) {
        eprintln!("spawn screens: slot 0 holds no live unit; the respawn stays automatic");
        return;
    }
    walk.end_shots = mission.map(|m| m.camera_shots()).unwrap_or_default();
    walk.backdrop = mission.and_then(|m| {
        game::spawn_backdrop::Backdrop::new(m.camera_shots(), game::spawn_backdrop::LCG_RESEED)
    });
    walk.follow_backdrop();
}

/// Debug splitscreen (issue #194, `--local-players N`): seat `players - 1` more people on this
/// walk, each behind their own spawn screens as the first player is, and give each the mission's
/// camera shots as a backdrop. A slot that cannot be seated is reported and left out.
pub(super) fn seat_local_players(
    walk: &mut WalkSim,
    players: usize,
    mission: Option<&game::mission::MissionDef>,
) {
    let side_choice = mission.map_or(true, |m| m.allows_side_select());
    for slot in 1..players.min(4) as u8 {
        if let Err(e) = walk.add_local_player(slot, None) {
            eprintln!("splitscreen: player slot {slot} could not be seated: {e:?}");
            break;
        }
        if !walk.sim.open_spawn_screens(usize::from(slot), side_choice) {
            eprintln!("spawn screens: slot {slot} holds no live unit; its respawn stays automatic");
            continue;
        }
        let i = walk.local_count() - 1;
        walk.with_local(i, |w| {
            w.backdrop = mission.and_then(|m| {
                game::spawn_backdrop::Backdrop::new(
                    m.camera_shots(),
                    game::spawn_backdrop::LCG_RESEED,
                )
            });
            w.follow_backdrop();
        });
    }
}

/// The end of the round as the HUD draws it (issue #33): the result, then
/// the stats pages (issue #50) once they are made.
fn round_end_model(r: &game::round_end::RoundEnd) -> ui::hud::RoundEndModel {
    ui::hud::RoundEndModel {
        result: r.result.banner(),
        stats: r.stats.clone(),
        fade: r.fade(),
    }
}

/// The stats pages of `walk`'s battle (issue #50, `game::round_stats`): the
/// feed's names and texts, the profile's `career` (this round in it).
pub(super) fn stats_pages(
    walk: &WalkSim,
    feed: &game::event_feed::EventFeed,
    career: &game::profile::Career,
) -> ui::stats::StatsScreens {
    let sim = &walk.sim;
    let unit = |i: usize| feed.unit_name(sim, sim.handle(i));
    let class = |team: u8, c: u8| feed.class_name(team, c).unwrap_or_default().to_owned();
    let weapon = |w: usize| {
        sim.weapon_classes.get(w).map_or_else(String::new, |c| {
            feed.weapon_name(&c.name).unwrap_or_else(|| c.name.clone())
        })
    };
    // A `#` key is a text the ELF names by its hash.
    let text = |key: &str, en: &str| {
        let t = match key.strip_prefix('#') {
            Some(h) => feed.text_by_hash(u32::from_str_radix(h, 16).unwrap_or(0), en),
            None => feed.screen_text(key, en),
        };
        t.trim().to_owned()
    };
    let names = game::round_stats::Names {
        unit: &unit,
        class: &class,
        weapon: &weapon,
        text: &text,
        sides: feed.sides(),
    };
    game::round_stats::screens(sim, feed.round(), walk.player, career, &names)
}

/// Spawn-only rounds require their matching mission.
fn validate_mission(
    scene: &game::assets::Scene,
    mission: Option<&game::mission::MissionDef>,
) -> Result<()> {
    let spawn_only = scene.manifest.spawn_only_posts;
    anyhow::ensure!(
        !spawn_only || mission.is_some(),
        "Hero Assault and Hunt require a mission"
    );
    if let Some(m) = mission {
        let requires_spawn_only = m.is_hero_assault() || m.is_hunt();
        anyhow::ensure!(
            spawn_only == requires_spawn_only,
            "mission mode {} differs from the loaded map mode",
            m.mode
        );
        if requires_spawn_only {
            anyhow::ensure!(
                scene
                    .dir
                    .file_name()
                    .is_some_and(|key| m.map.eq_ignore_ascii_case(&key.to_string_lossy()))
                    && m.source_game
                        .eq_ignore_ascii_case(&scene.manifest.source_game),
                "mission does not describe this map and game"
            );
        }
    }
    Ok(())
}

/// Build the walk/battle sim for a loaded scene per the CLI flags.
pub(super) fn make_walk(scene: &game::assets::Scene, b: &BattleSetup) -> Result<WalkSim> {
    make_walk_with_counts(scene, b, None)
}

/// The same construction order with explicit opening AI counts for the
/// benchmark's total-combatant fixture (a human replaces one AI slot).
pub(super) fn make_walk_with_counts(
    scene: &game::assets::Scene,
    b: &BattleSetup,
    counts: Option<[u32; 2]>,
) -> Result<WalkSim> {
    validate_mission(scene, b.mission.as_ref())?;
    let hunt = b.mission.as_ref().is_some_and(|m| m.is_hunt());
    let mut sim = core::sim::Sim::empty(b.seed);
    // The tier is set BEFORE `setup_conquest` spawns anyone: every bot takes
    // its skill at spawn (D2).
    sim.difficulty = b.difficulty;
    // A LAN round's rows (issue #171's LAN marker), before anyone spawns too.
    sim.netplay_rows = b.netplay_rows;
    // BF1 has no 10 m omni in the view cone (RE §9.1); BF2 does.
    sim.omni_radius = core::sim::omni_radius_for(&scene.manifest.source_game);
    scene.install_water(&mut sim);
    scene.install_damage_regions(&mut sim);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    // BF2-only builds omit BF1's borrowed prone animations; disable prone so
    // soldiers cannot enter a prone hitbox without prone animations.
    if let Some(assets_root) = scene.dir.parent() {
        sim.prone_off =
            pak::is_dir(assets_root.join("side_bf2")) && !pak::is_dir(assets_root.join("side_bf1"));
    }
    sim.item_pool = game::roster::item_pool(&scene.manifest.source_game, &scene.manifest.map);
    sim.cheats = b.cheats;
    // Map defaults belong to a conquest (a mission); a plain map walk has none.
    // The mission carries the game's unique key (bf1_nab2), not the manifest's
    // world name (naboo2).
    let map_defaults = b
        .mission
        .as_ref()
        .filter(|m| !b.xl && !m.is_hunt() && !m.is_campaign())
        .and_then(|m| game::conquest_defaults::for_map(&m.map, m.era.as_deref() == Some("cw")));
    let defaults = b.team_size_pct.and(map_defaults);
    let team_pct = b.team_size_pct.unwrap_or(100);
    let resolved = if b.xl {
        let base = b
            .mission
            .as_ref()
            .and_then(|m| ui::population::xl_team_size(&m.map))
            .ok_or_else(|| anyhow::anyhow!("XL has no team size for this map"))?;
        let combatants = ui::population::xl_team_counts(base, team_pct);
        Some(game::conquest_defaults::Defaults {
            combatants,
            // Battle tracks spawn capacity in this field even with unlimited respawns.
            tickets: combatants,
        })
    } else {
        defaults.map(|d| d.scaled(team_pct, b.ticket_pct))
    };
    let tickets = if b.tickets_set {
        [b.tickets; 2]
    } else if let Some(d) = resolved.filter(|_| b.xl) {
        // XL's spawn capacity: its own team sizes, never the mission's tickets.
        d.tickets
    } else if let (Some(d), Some(m)) = (defaults, b.mission.as_ref()) {
        // Issue #171: the map's pair with a lone human's side shifted for this
        // tier (`conquest_defaults::HUMAN_SHIFT`), then the reinforcement
        // percentage. The round's humans: slot 0 on its seat plus a LAN
        // round's others, the same on every peer.
        let mut humans = b.opening_humans;
        let seat = usize::from(b.player_team.min(1));
        humans[seat] = humans[seat].saturating_add(1);
        let base = game::conquest_defaults::human_tickets(
            &m.map,
            m.era.as_deref() == Some("cw"),
            b.difficulty,
            d.tickets,
            humans,
        );
        ui::population::tickets(base, b.ticket_pct)
    } else {
        b.mission
            .as_ref()
            .map_or([b.tickets; 2], |m| m.tickets())
            .map(|n| core::sim::scale_reinforcements(n, b.ticket_pct))
    };
    sim.battle = core::battle::Battle::new(tickets);
    if let Some(c) = b.mission.as_ref().and_then(|m| m.campaign.as_ref()) {
        use core::fmath::FloatExt;
        sim.battle.campaign_rules = true;
        sim.battle.unlimited_teams = c.infinite_tickets;
        for t in 0..2 {
            if c.infinite_tickets[t] {
                let min_pool =
                    b.mission
                        .as_ref()
                        .map_or(100, |m| m.teams[t].units.max(100)) as f32;
                sim.battle.teams[t].reinforcements =
                    sim.battle.teams[t].reinforcements.fmax(min_pool);
            }
        }
    }
    sim.time_limit = (b.time_limit > 0).then(|| b.time_limit * 60);
    if let Some(m) = &b.mission {
        sim.ai_alive_cap = m.ai_alive_cap;
        if m.is_hunt() {
            let tier = usize::from(b.difficulty.bytes()[0]);
            anyhow::ensure!(m.teams.len() == 2, "Hunt requires two teams");
            sim.battle.unlimited_reinforcements = true;
            sim.time_limit = Some(if b.time_limit == 0 {
                300
            } else {
                b.time_limit * 60
            });
            // Each side's alive count, total units, points to win and bot
            // skill scale: the mission's per-tier Hunt rows (OURS, the
            // maintainer 2026-11-20 late: big sides, targets to match).
            let round = m
                .hunt_round(tier, b.score_limit)
                .ok_or_else(|| anyhow::anyhow!("Hunt mission without Hunt rules"))?;
            anyhow::ensure!(
                round.units.into_iter().all(|n| n > 0),
                "Hunt requires positive team sizes"
            );
            sim.team_combatants = Some(round.alive);
            anyhow::ensure!(
                round.targets.into_iter().all(|n| n > 0),
                "Hunt requires positive points to win"
            );
            sim.deathmatch = Some(
                core::sim::deathmatch::Deathmatch::hunt_with_targets(round.targets, round.units)
                    .with_skill_pct(round.skill_pct),
            );
        }
        if m.is_hero_assault() {
            sim.time_limit = None;
            sim.deathmatch = Some(core::sim::deathmatch::Deathmatch::new(
                b.score_limit,
                [m.teams[0].units, m.teams[1].units],
            ));
        }
    }
    // --ai-cap wins over the mission, and applies with or without one.
    if let Some(cap) = b.ai_cap {
        sim.ai_alive_cap = cap;
    }
    if let Some(d) = resolved {
        sim.team_combatants = Some(d.combatants);
    }
    if let Some(d) = resolved.filter(|_| b.xl) {
        sim.battle.unlimited_reinforcements = true;
        sim.deathmatch = Some(core::sim::deathmatch::Deathmatch::xl(d.combatants));
    }
    let teams = team_templates(scene, b, &mut sim)?;
    let seat = usize::from(b.player_team.min(1));
    let mut walk = WalkSim::with_teams_seat(
        sim,
        scene.heightfield(),
        glam::Vec3::new(b.spawn.0, 0.0, b.spawn.1),
        teams,
        game::layout::nav_graph(&scene.manifest, &scene.dir),
        b.player_team,
    );
    // The pause menu's population calibration is for standard Conquest.
    walk.conquest_defaults = defaults;
    walk.team_size_pct = b.team_size_pct.unwrap_or(100);
    walk.reinforcement_pct = b.ticket_pct;
    walk.class_picker = Some(game::class_picker::ClassPicker::new(&b.classes_dir, scene));
    // Model collision: the scene has already had its mode layer applied (see
    // `Scene::collision_world` — building it earlier misses every BF2 conquest
    // instance).
    scene.install_collision(&mut walk.sim);
    // A Hunt's KillObject targets leave the scene at load
    // (`Scene::kill_objects`); a caller that loaded the scene without them
    // still gets a world without their bodies.
    if let Some(kills) = b
        .mission
        .as_ref()
        .map(|m| m.scene_kills())
        .filter(|k| !k.is_empty())
    {
        scene.install_collision_without(&mut walk.sim, kills);
    }
    walk.camera_blockers = game::camera_collide::CameraBlockers::from_scene(scene);
    if b.targets {
        walk.spawn_dummy_targets();
    }
    // Percentages use the map's combatant slots. A bench's explicit counts
    // win, then --ai; legacy CLI launches without percentages use authored
    // mission counts.
    let ai_counts = counts
        .or(if hunt {
            Some([walk.sim.ai_limit(0), walk.sim.ai_limit(1)])
        } else if let Some(n) = b.ai {
            Some([n; 2])
        } else if let Some(d) = resolved {
            Some(d.initial_ai(b.player_team))
        } else {
            b.mission
                .as_ref()
                .map(|m| [m.initial_ai(0, seat == 0), m.initial_ai(1, seat == 1)])
        })
        .or_else(|| b.vehicle.is_some().then_some([0, 0]));
    let mut vehicle_classes: Vec<String> = Vec::new();
    let mut layout = game::layout::MapLayout::from_mission_manifest_mode(
        &scene.manifest,
        hunt,
        b.mission.as_ref().is_some_and(|m| m.has_script()),
        b.mission.as_ref().is_some_and(|m| m.is_ctf()),
    );
    if let Some(m) = b.mission.as_ref().filter(|m| m.is_hunt()) {
        // Hoth and Theed register hazards by name in the Hunt scripts.
        // Their baked shapes also occur in the map's shared design worlds.
        // Reuse the named shapes without importing the other modes' posts.
        for name in &m.death_regions {
            if layout.region(name).is_some() {
                continue;
            }
            if let Some(region) = scene
                .manifest
                .layers
                .iter()
                .flat_map(|layer| &layer.regions)
                .find(|region| region.name.eq_ignore_ascii_case(name))
                .and_then(|r| {
                    core::region::Region::from_rows(&r.shape, r.rotation, r.translation, r.size)
                })
            {
                layout.regions.push((name.clone(), region));
            }
        }
    }
    // The mission owns the command-post roster, the posts it kills, the
    // ownership it re-authors and its death regions (game::layout).
    if let Some(m) = &b.mission {
        let a = layout.apply_mission(m);
        walk.sim.death_regions = a.death_regions.clone();
        walk.sim.fly_ceiling = a.fly_ceiling;
        walk.sim.flight_limits = a.flight_limits;
        walk.sim.landing_regions = a.landing_regions.clone();
        walk.sim.ground_flyer_map = m.ground_flyer_map;
        m.apply_door_locks(&mut walk.sim.doors, &scene.manifest);
        let blocked = m.apply_arc_blocks(walk.sim.nav.as_mut());
        if blocked != 0 {
            println!("mission: route groups {blocked:#b} blocked (plnp)");
        }
        let switched = m.apply_barriers(walk.sim.nav.as_mut());
        if switched != 0 {
            println!("mission: {switched} barriers switched (Enable/DisableBarriers)");
        }
        println!(
            "mission: {} posts (roster dropped {}, killed {}), {} re-owned, {} death regions{}",
            a.kept,
            a.dropped_by_roster,
            a.killed,
            a.reassigned,
            a.death_regions.len(),
            if a.missing_death_regions.is_empty() {
                String::new()
            } else {
                format!(" ({} missing)", a.missing_death_regions.len())
            }
        );
    }
    if let Some(counts) = ai_counts {
        walk.add_posts(&layout);
        let saved_cp_owners = if let Some(m) = b.mission.as_ref().filter(|m| m.has_script()) {
            game::campaign_runtime::CampaignRuntime::kill_startup_posts(&mut walk.sim, &m.killed)
        } else {
            Vec::new()
        };
        if hunt || b.mission.as_ref().is_some_and(|m| m.is_campaign()) {
            // Class change at your own posts (issue #161): each post's
            // class radius, from the map's shared and own classes.
            let posts = game::walk::hunt_post_classes(
                &b.classes_dir,
                &scene.dir,
                &scene.manifest.source_game,
            )?;
            walk.set_hunt_class_change(&layout, &posts, &scene.manifest.source_game);
            for team in (0..2).filter(|_| hunt) {
                anyhow::ensure!(
                    !walk.sim.spawn_points(team).is_empty(),
                    "Hunt team {} has no spawn points",
                    team + 1
                );
            }
        }
        // The mission's AI settings (card MAIS), its capture bans by post.
        if let Some(m) = &b.mission {
            m.apply_ai(&mut walk.sim);
        }
        // The vehicle spawn pads follow the posts just added (card VPAD) and
        // go in before the opening wave, which spawns at the command walkers
        // they are about to build (card CWSP).
        let factions = [walk.teams[0].faction.clone(), walk.teams[1].faction.clone()];
        let space = b
            .mission
            .as_ref()
            .and_then(|m| m.space_round())
            .map(|def| (def, b.space_score_limit));
        match game::vehicle_pads::install_for_mission(
            &mut walk.sim,
            &scene.manifest,
            &scene.dir,
            &b.classes_dir,
            &factions,
            space.as_ref().map(|(def, limit)| (def, *limit)),
            b.mission.as_ref().map_or(&[][..], |m| &m.class_load_order),
        ) {
            Ok(p) => {
                println!(
                    "vehicle pads: {} ({} classes){}",
                    p.pads.len(),
                    p.classes.len(),
                    if p.absent.is_empty() {
                        String::new()
                    } else {
                        format!(", no class for {:?}", p.absent)
                    }
                );
                walk.vehicle_geometry = p.geometry;
                walk.vehicle_seats = p.seats;
                walk.vehicle_weapon_hud = p.weapon_hud;
                walk.walker_shot_styles = p.walker_shot_styles;
                walk.flyer_exhaust = p.flyer_exhaust;
                walk.hover_water = p.hover_water;
                walk.hover_treads = p.hover_treads;
                walk.load_walker_views(&scene.manifest, scene.dir.parent().unwrap_or(&scene.dir));
                walk.load_vehicle_tops(
                    &scene.manifest,
                    &scene.dir,
                    scene.dir.parent().unwrap_or(&scene.dir),
                );
                walk.vehicle_riders = p.riders;
                walk.vehicle_engines = p.engines;
                walk.vehicle_sounds = p.sounds;
                walk.sound_triggers = game::ambience::sound_triggers(&scene.manifest);
                walk.vehicle_labels = p.labels;
                walk.vehicle_classes = p.classes.clone();
                vehicle_classes = p.classes;
            }
            Err(e) => eprintln!("vehicle pads unavailable ({e:#})"),
        }
        if let Some(m) = &b.mission {
            walk.particle_lod_bias = m.particle_lod_bias.unwrap_or(0.0);
            if !m.class_properties.is_empty() {
                let team_classes: [Vec<String>; 2] = [0, 1].map(|t| {
                    walk.teams[t]
                        .classes
                        .iter()
                        .map(|c| c.name.clone())
                        .collect()
                });
                for (cls, prop, val) in &m.class_properties {
                    game::campaign_runtime::apply_class_property(
                        &mut walk.sim,
                        &team_classes,
                        cls,
                        prop,
                        val,
                    );
                }
            }
            for t in 0..2usize {
                walk.sim
                    .set_ai_spawn_allowed(t as u8, m.teams[t].allows_ai_spawn_initially());
                let hero = walk.sim.hero_class(t as u8).map(usize::from);
                let n = walk.sim.classes[t].len();
                walk.sim.class_enabled[t] = (0..n)
                    .map(|ci| {
                        let name = walk.teams[t]
                            .classes
                            .get(ci)
                            .map(|cv| cv.name.as_str())
                            .unwrap_or("");
                        if Some(ci) == hero {
                            !m.teams[t]
                                .class_blacklist
                                .iter()
                                .any(|c| c.eq_ignore_ascii_case(name))
                        } else {
                            m.teams[t].is_class_initially_enabled(name)
                        }
                    })
                    .collect();
            }
            let assets_dir = scene.dir.parent().unwrap_or(&scene.dir);
            game::native_teams::install(
                &mut walk,
                &layout,
                &b.classes_dir,
                assets_dir,
                m,
                &scene.manifest,
            );
            let script_spec = m.campaign.clone().or_else(|| {
                m.script.clone().map(|script| {
                    let mut spec = game::mission::CampaignMissionDef::default();
                    spec.script = Some(script);
                    spec.libs = m.loaded_script_libs.clone();
                    spec
                })
            });
            if let Some(spec) = script_spec {
                let team_classes: [Vec<String>; 2] = [0, 1].map(|t| {
                    walk.teams[t]
                        .classes
                        .iter()
                        .map(|c| c.name.clone())
                        .collect()
                });
                let mut rt = game::campaign_runtime::CampaignRuntime::new(
                    spec,
                    &layout,
                    &scene.manifest,
                    &walk.post_keys,
                    saved_cp_owners,
                    &walk.sim,
                );
                rt.set_is_campaign(m.is_campaign());
                rt.set_script_setup(m.script_setup);
                rt.set_team_rosters(
                    team_classes,
                    if m.is_campaign() {
                        [0, 0]
                    } else {
                        [m.teams[0].units, m.teams[1].units]
                    },
                );
                rt.set_team_factions([
                    walk.teams[0].faction.as_str(),
                    walk.teams[1].faction.as_str(),
                ]);
                rt.set_native_rosters(
                    m.local_teams
                        .iter()
                        .filter_map(|lt| {
                            let team = lt.team.checked_sub(1).filter(|t| *t >= 2)?;
                            let classes = lt.classes.iter().map(|c| c.0.clone()).collect();
                            Some((team, (classes, lt.units)))
                        })
                        .collect(),
                );
                rt.load_world_anims(&scene.dir, &scene.manifest, &walk.sim);
                rt.start(&mut walk.sim);
                // The third teams the script filled while it ran (a hero it
                // picked at random) join once their classes are known.
                let requested = rt.take_native_requests();
                if m.script.is_some() && !requested.is_empty() {
                    let mut with = m.clone();
                    with.local_teams = requested;
                    game::native_teams::install(
                        &mut walk,
                        &layout,
                        &b.classes_dir,
                        assets_dir,
                        &with,
                        &scene.manifest,
                    );
                }
                // ScriptPostLoad requests the same opening clip played by the
                // campaign shell before loading the battle.
                rt.take_movies();
                walk.camera_blockers.mark_moving(rt.animated_instances());
                walk.camera_blockers
                    .set_excluded(rt.excluded_camera_instances());
                walk.campaign = Some(rt);
            } else if !m.world_events.is_empty() {
                let rt = game::campaign_runtime::WorldEventRuntime::new(
                    m.world_events.clone(),
                    scene,
                    &layout,
                    &walk.post_keys,
                    &walk.sim,
                );
                walk.camera_blockers.mark_moving(rt.animated_instances());
                walk.world_events = Some(rt);
            }
        }
        // A LAN round's other humans are seated after this wave
        // (`round_setup::build_round`), but its bots take their skill now:
        // they count for the DTEAM rows while it spawns (issue #171).
        let seated = walk.sim.extra_humans;
        walk.sim.extra_humans = [0, 1].map(|t| seated[t].saturating_add(b.opening_humans[t]));
        // Campaigns with spawn screens defer their opening AI wave until the
        // human player spawns so `onfirstspawn` (e.g. Kamino switching Team 1
        // from `boba` to `cp5`) updates CP ownership before allies spawn.
        let opening_counts = if b.mission.as_ref().is_some_and(|m| m.is_campaign()) {
            [0, 0]
        } else {
            counts
        };
        walk.opening_wave(opening_counts, !b.spawn_set);
        // The bots enter over the first 10 s, each on its own seeded delay
        // (`Sim::stagger_opening_ai`); the humans spawn as before.
        walk.sim.stagger_opening_ai();
        walk.sim.extra_humans = seated;
        // The commander speaks the mission's lines over the posts and the
        // battle just set up.
        if let Some(m) = &b.mission {
            let assets_dir = scene.dir.parent().unwrap_or(&scene.dir);
            if let Some(rt) = walk.campaign.as_mut() {
                let team_classes: [Vec<String>; 2] = [0, 1].map(|t| {
                    walk.teams[t]
                        .classes
                        .iter()
                        .map(|c| c.name.clone())
                        .collect()
                });
                rt.set_team_rosters(team_classes, counts);
                rt.defer_opening_wave();
            }
            walk.sound_levels = m.sound_levels();
            walk.space_round = m.space_assault.as_ref().map(|d| {
                let mut round = game::space_assault::SpaceRound::new(d);
                round.sheet_dir = Some(assets_dir.join(&m.map).join("textures"));
                std::sync::Arc::new(round)
            });
            if let Some(ctf_def) = &m.ctf {
                let score_limit = ctf_def
                    .score_limit
                    .unwrap_or(core::sim::DEFAULT_CTF_SCORE_LIMIT);
                let find_region = |name: &str| -> (glam::Vec3, Option<f32>) {
                    if let Some(r) = scene
                        .manifest
                        .regions
                        .iter()
                        .find(|r| r.name.eq_ignore_ascii_case(name))
                    {
                        let r_obj = core::region::Region::from_rows(
                            &r.shape,
                            r.rotation,
                            r.translation,
                            r.size,
                        );
                        let radius = r_obj.map(|reg| match reg.shape {
                            core::region::Shape::Sphere => reg.w,
                            core::region::Shape::Cylinder => reg.size.x,
                            core::region::Shape::Box => {
                                core::fmath::FloatExt::fmax(reg.size.x, reg.size.z)
                            }
                        });
                        return (
                            glam::Vec3::from(r.translation),
                            radius.map(|rad| rad.clamp(2.5, 25.0)),
                        );
                    }
                    let pos = scene
                        .manifest
                        .instances
                        .iter()
                        .find(|i| i.name.eq_ignore_ascii_case(name))
                        .map(|i| glam::Vec3::from(i.translation))
                        .unwrap_or(glam::Vec3::ZERO);
                    (pos, None)
                };
                let mission_factions = [m.teams[0].faction.as_str(), m.teams[1].faction.as_str()];
                let sim_factions = [
                    walk.teams[0].faction.as_str(),
                    walk.teams[1].faction.as_str(),
                ];
                let find_instance = |name: &str| -> Option<glam::Vec3> {
                    (!name.is_empty())
                        .then(|| {
                            scene
                                .manifest
                                .instances
                                .iter()
                                .find(|i| i.name.eq_ignore_ascii_case(name))
                                .map(|i| glam::Vec3::from(i.translation))
                        })
                        .flatten()
                };
                let swapped = mission_factions[0].eq_ignore_ascii_case(sim_factions[1])
                    && mission_factions[1].eq_ignore_ascii_case(sim_factions[0]);
                let is_two_flag = ctf_def.two_flag || ctf_def.flags.len() >= 2;
                let mut ctf_state = if is_two_flag && ctf_def.flags.len() >= 2 {
                    let (f0, f1) = if swapped {
                        (&ctf_def.flags[1], &ctf_def.flags[0])
                    } else {
                        (&ctf_def.flags[0], &ctf_def.flags[1])
                    };
                    let home0 =
                        find_instance(&f0.name).unwrap_or_else(|| find_region(&f0.home_region).0);
                    let home1 =
                        find_instance(&f1.name).unwrap_or_else(|| find_region(&f1.home_region).0);
                    let (goal0, r0) = find_region(&f1.capture_region);
                    let (goal1, r1) = find_region(&f0.capture_region);
                    let goal0 = if f1.capture_region.is_empty() {
                        home0
                    } else {
                        goal0
                    };
                    let goal1 = if f0.capture_region.is_empty() {
                        home1
                    } else {
                        goal1
                    };
                    let mut st =
                        core::sim::CtfState::new_two_flag(score_limit, home0, goal0, home1, goal1);
                    if let Some(rad) = r0 {
                        st.capture_regions[0].radius = rad;
                    }
                    if let Some(rad) = r1 {
                        st.capture_regions[1].radius = rad;
                    }
                    st
                } else {
                    let (home_pos, _) = find_region(&ctf_def.home_region);
                    let (att_pos, att_r) = find_region(&ctf_def.capture_region_att);
                    let (def_pos, def_r) = find_region(&ctf_def.capture_region_def);
                    let goals = ctf_capture_goals(mission_factions, sim_factions, att_pos, def_pos);
                    let radii = if swapped {
                        [def_r, att_r]
                    } else {
                        [att_r, def_r]
                    };
                    let space_map = m.map.contains("_spa")
                        || scene
                            .manifest
                            .instances
                            .iter()
                            .any(|i| i.class.eq_ignore_ascii_case("com_icon_spaceflag"));
                    let mut st = if space_map {
                        core::sim::CtfState::new_space_one_flag(
                            score_limit,
                            home_pos,
                            goals[0],
                            goals[1],
                        )
                    } else {
                        core::sim::CtfState::new_ground_one_flag(
                            score_limit,
                            home_pos,
                            goals[0],
                            goals[1],
                        )
                    };
                    for (region, rad) in st.capture_regions.iter_mut().zip(radii) {
                        if let Some(rad) = rad {
                            region.radius = rad;
                        }
                    }
                    st
                };
                let flag_names: Vec<&str> = if ctf_state.mode == core::sim::CtfMode::TwoFlag {
                    let names = [
                        ctf_def.flags[0].name.as_str(),
                        ctf_def.flags[1].name.as_str(),
                    ];
                    if swapped {
                        vec![names[1], names[0]]
                    } else {
                        names.to_vec()
                    }
                } else {
                    vec![ctf_def.flag.as_str()]
                };
                // FUN_0035d2f8 (B2:343280-343287, PC 0x57e7b7) starts a homed
                // flag's return timer at its class Lifespan.
                let flag_classes = game::walk::hunt_post_classes(
                    &b.classes_dir,
                    &scene.dir,
                    &scene.manifest.source_game,
                )?;
                for (flag, name) in ctf_state.flags.iter_mut().zip(flag_names) {
                    let lifespan = scene
                        .manifest
                        .instances
                        .iter()
                        .find(|i| i.name.eq_ignore_ascii_case(name))
                        .and_then(|i| flag_classes.prop_f32(&i.class, "Lifespan"));
                    if let Some(lifespan) = lifespan {
                        flag.drop_timeout = lifespan;
                    }
                }
                let space_ctf = ctf_state.is_space();
                if ctf_def.hide_cps {
                    for cp in &mut walk.sim.cps {
                        cp.capturable = false;
                    }
                }
                if !space_ctf && walk.sim.ai_goals.idle() {
                    if ctf_state.mode == core::sim::CtfMode::TwoFlag {
                        let g0 = ctf_state.capture_regions[0].pos;
                        let g1 = ctf_state.capture_regions[1].pos;
                        walk.sim.ai_goals.add(
                            0,
                            core::goals::GoalKind::CtfOffense,
                            70.0,
                            Some(core::goals::GoalTarget::Ctf {
                                region: g0,
                                flag: Some(1),
                            }),
                        );
                        walk.sim.ai_goals.add(
                            0,
                            core::goals::GoalKind::CtfDefense,
                            30.0,
                            Some(core::goals::GoalTarget::Flag(0)),
                        );
                        walk.sim.ai_goals.add(
                            1,
                            core::goals::GoalKind::CtfOffense,
                            70.0,
                            Some(core::goals::GoalTarget::Ctf {
                                region: g1,
                                flag: Some(0),
                            }),
                        );
                        walk.sim.ai_goals.add(
                            1,
                            core::goals::GoalKind::CtfDefense,
                            30.0,
                            Some(core::goals::GoalTarget::Flag(1)),
                        );
                    } else {
                        let g0 = ctf_state.capture_regions[0].pos;
                        let g1 = ctf_state.capture_regions[1].pos;
                        walk.sim.ai_goals.add(
                            0,
                            core::goals::GoalKind::CtfOffense,
                            80.0,
                            Some(core::goals::GoalTarget::Ctf {
                                region: g0,
                                flag: Some(0),
                            }),
                        );
                        walk.sim.ai_goals.add(
                            1,
                            core::goals::GoalKind::CtfOffense,
                            80.0,
                            Some(core::goals::GoalTarget::Ctf {
                                region: g1,
                                flag: Some(0),
                            }),
                        );
                    }
                }
                // Drop ground flags straight onto settled floors at round start.
                if !space_ctf {
                    for flag in &mut ctf_state.flags {
                        if let Some((floor, _)) = walk
                            .sim
                            .ground_point_hit_under_n(flag.home_pos, flag.home_pos.y + 0.5)
                        {
                            let settled_y = floor + core::sim::ctf::GROUND_FLAG_CLEARANCE;
                            flag.home_pos.y = settled_y;
                            flag.current_pos.y = settled_y;
                            flag.landed = true;
                        }
                    }
                }
                walk.sim.install_ctf(ctf_state);
                walk.sim.flyer_bots = space_ctf;
            }
            walk.voice = Some(game::announce::Commander::for_battle(m, &layout, &walk));
            // ...and the music is the script's too, by the reinforcements
            // the teams start with.
            let tickets = [0, 1].map(|t| walk.sim.battle.teams[t].tickets());
            let bf2 = m.source_game == "bf2";
            walk.music = Some(game::music::BattleMusic::new(&m.music, bf2, tickets));
            // Conquest: the team's hero; Hero Assault: every class.
            let assault = walk.sim.hero_assault();
            let names = [0u8, 1]
                .into_iter()
                .flat_map(|team| {
                    let classes = &walk.teams[usize::from(team)].classes;
                    let hero = walk.sim.hero_class(team);
                    (0..classes.len() as u8)
                        .filter(move |&c| assault || hero == Some(c))
                        .filter_map(move |c| {
                            let music = classes[usize::from(c)].cries.hero.music.clone()?;
                            Some((team, c, music))
                        })
                })
                .collect();
            walk.music.as_mut().unwrap().set_hero_music(names);
        }
        // Issue #182: seated riders are hit where their own size and pose
        // draw them (after the native teams join).
        game::seated_bodies::install(
            &mut walk,
            scene.dir.parent().unwrap_or(&scene.dir),
            &scene.manifest.source_game,
        );
        println!(
            "conquest: {} posts, the player on team {} ({}), AI {}/{}, tickets {}/{}, alive-AI limit {}/{}, difficulty {}, omni {:.0} m",
            layout.cps.len(),
            seat + 1,
            walk.teams[seat].faction,
            counts[0],
            counts[1],
            walk.sim.battle.teams[0].tickets(),
            walk.sim.battle.teams[1].tickets(),
            walk.sim.ai_limit(0),
            walk.sim.ai_limit(1),
            walk.sim.difficulty.label(),
            walk.sim.omni_radius,
        );
    } else if !b.spawn_set {
        // No conquest setup and no pinned spawn: the player is standing at
        // the authored default, which is off the walkable surface on seven
        // BF2 maps (`WalkSim::rescue_spawn`).
        if let Some((post, y)) = walk.rescue_spawn(&layout) {
            println!(
                "spawn: no ground at the authored point — moved to command post {post} (y {y:.2})"
            );
        }
    }
    walk.minimap = Some(game::minimap::MapStatic::load(
        scene,
        &b.classes_dir,
        &walk,
        &vehicle_classes,
        b.mission.as_ref().and_then(|m| m.era.as_deref()),
    ));
    if let Some(query) = b.vehicle.as_deref() {
        seat_player_in_vehicle(&mut walk, query)?;
    }
    Ok(walk)
}

/// BF2's HUD from the bake at `assets_dir`, painted into `r`'s frames;
/// `None` — said once — when the bake holds none.
pub(super) fn load_hud_view(
    assets_dir: &std::path::Path,
    r: &render::Renderer,
) -> Option<game::hud_feed::HudView> {
    match game::hud_feed::HudView::for_renderer(assets_dir, r) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("hud: no BF2 HUD in the bake ({e:#}); bars and counts not drawn");
            None
        }
    }
}

/// The capture region each sim team must carry the flag into.
///
/// Every retail One-Flag script names mission team 1 the attacker and team 2
/// the defender (`teamATT = 1`, `teamDEF = 2`, or the `IMP`/`ALL`/`REP`/`CIS`
/// globals that resolve to those numbers). `Start` activates
/// `captureRegionATT` for the attacker and `captureRegionDEF` for the
/// defender, and the two `OnEnterRegion` handlers accept only a carrier whose
/// team matches (`objectiveoneflagctf.luac` proto 5's registrations, protos
/// 14 and 15). The sim's team order can differ from the mission's when
/// `--teams` overrides the pair, so match the mission team by faction.
fn ctf_capture_goals(
    mission_factions: [&str; 2],
    sim_factions: [&str; 2],
    att: glam::Vec3,
    def: glam::Vec3,
) -> [glam::Vec3; 2] {
    let mut goals = [att, def];
    for (slot, faction) in sim_factions.iter().enumerate() {
        match mission_factions
            .iter()
            .position(|f| f.eq_ignore_ascii_case(faction))
        {
            Some(0) => goals[slot] = att,
            Some(_) => goals[slot] = def,
            None => {}
        }
    }
    goals
}

/// Flag mesh specs (`(loose_geometry, carried_geometry)`) aligned to sim team slots.
pub(super) fn ctf_flag_specs(
    mission: Option<&game::mission::MissionDef>,
    walk: &WalkSim,
) -> Option<Vec<(Option<String>, Option<String>)>> {
    let m = mission?;
    let ctf = m.ctf.as_ref()?;
    if ctf.flags.is_empty() {
        return None;
    }
    let swapped = ctf.flags.len() >= 2
        && m.teams[0]
            .faction
            .eq_ignore_ascii_case(&walk.teams[1].faction)
        && m.teams[1]
            .faction
            .eq_ignore_ascii_case(&walk.teams[0].faction);
    let ordered: Vec<&game::mission::CtfFlagDef> = if swapped {
        vec![&ctf.flags[1], &ctf.flags[0]]
    } else {
        ctf.flags.iter().collect()
    };
    Some(
        ordered
            .into_iter()
            .map(|f| (f.geometry.clone(), f.carried_geometry.clone()))
            .collect(),
    )
}

/// Each sim flag's placed class, in `walk.sim.ctf`'s flag order (the
/// battle setup's: a 2-flag pair swapped like [`ctf_flag_specs`]), by its
/// instance name. `None` for a flag with no placed instance.
pub(super) fn ctf_flag_classes(
    mission: Option<&game::mission::MissionDef>,
    walk: &WalkSim,
    scene: &game::assets::Scene,
) -> Vec<Option<String>> {
    let Some((m, ctf)) = mission.and_then(|m| Some((m, m.ctf.as_ref()?))) else {
        return Vec::new();
    };
    let mut names: Vec<&str> = if ctf.flags.is_empty() {
        vec![ctf.flag.as_str()]
    } else {
        ctf.flags.iter().map(|f| f.name.as_str()).collect()
    };
    let swapped = ctf.flags.len() >= 2
        && m.teams[0]
            .faction
            .eq_ignore_ascii_case(&walk.teams[1].faction)
        && m.teams[1]
            .faction
            .eq_ignore_ascii_case(&walk.teams[0].faction);
    if swapped {
        names.swap(0, 1);
    }
    names
        .into_iter()
        .map(|name| {
            scene
                .manifest
                .instances
                .iter()
                .find(|i| i.name.eq_ignore_ascii_case(name))
                .map(|i| i.class.clone())
        })
        .collect()
}

fn hud_lock(
    walk: &WalkSim,
    feed: &game::event_feed::EventFeed,
    slot: usize,
    aspect: f32,
) -> Option<ui::hud::HudLock> {
    // A flyer's lock, else the launcher's on foot (`core::sim::foot_lock`).
    let flyer = walk.sim.lock_on_target(slot);
    let (target, pos) = flyer.or_else(|| walk.sim.foot_lock_target(slot))?;
    let flyer = flyer.is_some();
    let hud = feed.lock_target(&walk.sim, target)?;
    let camera = walk.camera();
    let ([x, y], on_screen) = marker_screen(camera.view_proj(aspect), pos);
    let off = pos - camera.position;
    let distance = off.length();
    let toward = off.normalize_or_zero();
    let view = camera.view();
    let up = glam::Vec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y).dot(toward);
    let direction = (up.abs() > 0.2).then_some(if up > 0.0 { 0.0 } else { 180.0 });
    Some(ui::hud::HudLock {
        target: hud,
        distance,
        direction,
        ndc: [x, y],
        on_screen,
        visible: if flyer {
            walk.sim.lock_on_visible(slot)
        } else {
            walk.sim.foot_lock_visible(slot)
        },
        charge: if flyer {
            [
                walk.sim.lock_on_charge(slot, 0),
                walk.sim.lock_on_charge(slot, 1),
            ]
        } else {
            [walk.sim.foot_lock_charge(slot), None]
        },
        flyer,
        pulse: walk.sim.tick as f32 / core::TICK_HZ as f32,
    })
}

/// HUD model from the battle state. Takes `feed`'s new event lines: each
/// model must reach the HUD (`HudView::update`) or its lines are lost.
pub(super) fn hud_model(
    walk: &WalkSim,
    feed: &mut game::event_feed::EventFeed,
    aspect: f32,
) -> ui::hud::HudModel {
    hud_model_for_view(walk, feed, aspect, None)
}

/// [`hud_model`] with an optional stereo VR view camera/aspect for hint projection.
pub(super) fn hud_model_for_view(
    walk: &WalkSim,
    feed: &mut game::event_feed::EventFeed,
    aspect: f32,
    stereo_view: Option<(&render::Camera, f32)>,
) -> ui::hud::HudModel {
    let me = &walk.sim.units[walk.player.index as usize];
    let b = &walk.sim.battle;
    let droid = walk
        .sim
        .controlled_droid(walk.player)
        .and_then(|d| Some((d, walk.sim.droid_class(d)?)));
    let guided = walk
        .sim
        .controlled_guided(walk.player)
        .and_then(|r| Some((r, walk.sim.guided_class(r)?)));
    let on_foot = me.alive && me.mount.is_none() && droid.is_none() && guided.is_none();
    let space_status = walk
        .space_round
        .as_ref()
        .filter(|_| me.alive)
        .and_then(|round| feed.space_status(&walk.sim, round, me.team));
    let (stamina, stamina_max) = me.stamina_bar();
    let (hp, hp_max, energy, energy_max) = match (guided, droid) {
        (Some((r, c)), _) => (r.health, c.health, r.energy(), c.flight.energy_max),
        // [ours] Flying a droid, the stamina bar runs down its 90 s.
        (None, Some((d, c))) => (
            d.health,
            c.max_health,
            d.time_left(),
            core::remote::LIFETIME,
        ),
        (None, None) => (me.health.cur, me.health.max, stamina, stamina_max),
    };
    let spawn = spawn_model(walk, feed).and_then(|s| s.page);
    let slot = me.spec.player_slot.map_or(0, usize::from);
    // On foot the soldier's rows; flying, the droid's.
    let [weapon, weapon2] = game::hud_feed::weapon_rows(walk).map(|row| {
        row.map(|mut weapon| {
            weapon.name = feed
                .weapon_name(&weapon.class)
                .or_else(|| feed.weapon_name(&weapon.key))
                .unwrap_or_else(|| weapon.class.clone());
            weapon
        })
    });
    let gauge = game::hud_feed::weapon_gauge(walk);
    let leaving = walk
        .sim
        .is_leaving(walk.player)
        .then(|| feed.leaving_text());
    let reticle =
        (on_foot || droid.is_some() || guided.is_some() || game::hud_feed::armed_seat(walk))
            .then(|| game::hud_feed::reticle_relation(&walk.sim, slot));
    ui::hud::HudModel {
        team_names: [walk.teams[0].faction.clone(), walk.teams[1].faction.clone()],
        tickets: [b.teams[0].tickets(), b.teams[1].tickets()],
        unlimited_tickets: [0, 1].map(|t| b.unlimited_reinforcements || b.unlimited_teams[t]),
        popup: walk.campaign.as_ref().and_then(|c| c.popup()).map(|p| {
            use game::campaign_runtime::Popup;
            match p {
                Popup::Hint(t) => ui::hud::HudPopup::Hint(feed.localize_or_raw(t)),
                Popup::Objective(t) => ui::hud::HudPopup::Objective(feed.localize_or_raw(t)),
                Popup::Selection(t) => ui::hud::HudPopup::Selection(feed.localize_or_raw(t)),
            }
        }),
        objectives_updated: walk
            .campaign
            .as_ref()
            .is_some_and(|c| c.objectives_updated()),
        bleed: if walk.sim.ctf.is_some() {
            [0.0, 0.0]
        } else {
            [b.teams[0].bleed_per_sec, b.teams[1].bleed_per_sec]
        },
        hp,
        hp_max,
        hero: walk.sim.is_hero(walk.player),
        hero_offer: walk.sim.hero_offer(slot).map(|(class, seconds)| {
            let name = feed
                .class_names(me.team)
                .get(usize::from(class))
                .cloned()
                .unwrap_or_else(|| {
                    walk.teams[usize::from(me.team.min(1))]
                        .class(class)
                        .label
                        .clone()
                });
            (
                feed.text_by_hash(0xadd5_467c, "Play as %s?")
                    .replacen("%s", &name, 1),
                seconds,
            )
        }),
        energy,
        energy_max,
        jet: (on_foot && me.consts.has_jet())
            .then_some((me.soldier.fuel, me.consts.jet_fuel_min_border)),
        weapon,
        weapon2,
        gauge,
        leaving,
        spectate: spectate_label(walk, feed, KEYBOARD_SPECTATE_HINT),
        reticle,
        // The reticle's pick by name and health (card HTGT).
        target: (on_foot || game::hud_feed::armed_seat(walk))
            .then(|| feed.target(&walk.sim, slot))
            .flatten(),
        lock: hud_lock(walk, feed, slot, aspect),
        incoming_missile: walk.sim.incoming_missile(slot),
        hints: hud_hints(walk, feed, slot, aspect, stereo_view),
        flag_timers: ctf_hud::flag_timers(walk, aspect, stereo_view),
        reticle_y: walk.reticle_y(),
        dead: !me.alive,
        seat_health: me
            .mount
            .and_then(|m| walk.sim.vehicle(m.vehicle))
            .map(|v| (v.health.cur, v.health.max)),
        // A campaign shows team points only where its script turned them
        // on (`ShowTeamPoints`); its flags are the script's, not a CTF round.
        team_points: match walk.campaign.as_ref() {
            Some(c) => c.team_points().map(|p| (p, [0; 2])),
            None => walk
                .sim
                .space_team_points()
                .map(|(points, limit)| (points, [limit; 2]))
                .or_else(|| {
                    walk.sim.ctf.as_ref().map(|c| {
                        (
                            [c.team_points[0] as i32, c.team_points[1] as i32],
                            [c.score_limit; 2],
                        )
                    })
                })
                .or_else(|| {
                    walk.sim
                        .team_points()
                        .map(|(points, limits)| (points.map(|p| p as i32), limits))
                }),
        },
        ctf: walk.sim.ctf.is_some() && walk.campaign.is_none(),
        flag_status: walk
            .sim
            .ctf
            .as_ref()
            .filter(|_| walk.campaign.is_none())
            .and_then(|c| {
                if c.mode == core::sim::CtfMode::OneFlag {
                    c.flags.first().and_then(|flag| match flag.status {
                        core::sim::FlagStatus::AtHome => None,
                        core::sim::FlagStatus::Dropped { .. } => {
                            Some(ui::hud_flag::FlagStatus::Dropped)
                        }
                        core::sim::FlagStatus::Carried { carrier } => {
                            walk.sim.unit(carrier).map(|unit| {
                                ui::hud_flag::FlagStatus::Carried {
                                    friendly: walk.sim.allied(me.team, unit.team),
                                    player_first_person: carrier == walk.player
                                        && walk.first_person(),
                                }
                            })
                        }
                    })
                } else {
                    let player_carrying = c.flags.iter().any(|f| {
                        matches!(f.status, core::sim::FlagStatus::Carried { carrier } if carrier == walk.player)
                    });
                    if player_carrying {
                        return Some(ui::hud_flag::FlagStatus::Carried {
                            friendly: true,
                            player_first_person: walk.first_person(),
                        });
                    }
                    let friendly_carried = c.flags.iter().any(|f| {
                        f.team != me.team
                            && matches!(f.status, core::sim::FlagStatus::Carried { carrier } if walk.sim.unit(carrier).is_some_and(|u| walk.sim.allied(me.team, u.team)))
                    });
                    if friendly_carried {
                        return Some(ui::hud_flag::FlagStatus::Carried {
                            friendly: true,
                            player_first_person: false,
                        });
                    }
                    let enemy_carried = c.flags.iter().any(|f| {
                        f.team == me.team
                            && matches!(f.status, core::sim::FlagStatus::Carried { carrier } if walk.sim.unit(carrier).is_some_and(|u| !walk.sim.allied(me.team, u.team)))
                    });
                    if enemy_carried {
                        return Some(ui::hud_flag::FlagStatus::Carried {
                            friendly: false,
                            player_first_person: false,
                        });
                    }
                    c.flags
                        .iter()
                        .any(|f| matches!(f.status, core::sim::FlagStatus::Dropped { .. }))
                        .then_some(ui::hud_flag::FlagStatus::Dropped)
                }
            }),
        space: space_status,
        outcome: walk.sim.outcome,
        countdown: walk
            .sim
            .domination_left()
            .map(|(holder, secs)| (holder == me.team, secs)),
        round_end: walk.round_end.as_ref().map(round_end_model),
        // Only the round's own time limit draws a countdown. The campaign's
        // `ShowTimer` objectives have no HUD timer in the original - the egui
        // panel was a port addition that rendered badly over the battle - so
        // mission timers stay off the HUD.
        time_left: walk.sim.time_limit.map(|l| {
            (u64::from(l) * u64::from(core::TICK_HZ)).saturating_sub(walk.sim.tick) as f32
                / core::TICK_HZ as f32
        }),
        messages: feed.take(),
        ambush: feed.take_ambush(),
        damage_flash: feed.damage_flash(walk.sim.tick, walk.player),
        // The sniper headshot's reticle mark (hitmark).
        hit_critical: feed.hit_critical(walk.sim.tick),
        // Every enemy body hit's plain flash (2026-10-29).
        hit_body: feed.hit_body(walk.sim.tick),
        stats: Some(feed.stats(&walk.sim, walk.player)),
        player_team: me.team,
        map: walk.minimap.as_ref().and_then(|m| match &spawn {
            // The spawn map page shows the Map element's Spawn view.
            Some(ui::spawn_draw::SpawnPage::Map { .. }) => Some(m.spawn_model(walk, slot)),
            // A space map's dead player keeps no minimap.
            _ => (walk.space_round.is_none() && (me.alive || !walk.space_battle()))
                .then(|| m.model_looking(walk, stereo_view.map(|(view, _)| view.yaw))),
        }),
        squad: feed.squad(walk.sim.tick),
        // The spawn screen up, drawn in the HUD's frame (card SSELB).
        spawn,
        // The sniper scope's overlay (`ui::hud_scope`).
        scope: game::hud_feed::scope_view(walk),
        capture: game::capture_feed::for_unit(
            &walk.sim,
            walk.player.index as usize,
            game::capture_feed::Rules::PORT,
        ),
        // Filled by the window's frame for a LAN or split view (`events.rs`).
        name_tags: Vec::new(),
    }
}

/// Issue #182: the teammate view's switch hint on keyboard and mouse: a
/// click steps on, G (or the right button, secondary fire) steps back
/// (`game::walk` spectate's `PREV_BITS`).
pub(super) const KEYBOARD_SPECTATE_HINT: &str = "Click / G / Right click: switch teammate";

/// Issue #182: the teammate view's switch hint on a controller, by family,
/// when the bindings leave a trigger unbound
/// (`game::input_icons::spectate_pad_hint` names the bound ones)
/// (`[ours]`: R2 steps on, L2 back; the spawn screens' own buttons stay
/// theirs, as the sim still reads them behind the view).
pub(super) fn pad_spectate_hint(xbox: bool) -> &'static str {
    if xbox {
        "LT / RT: switch teammate"
    } else {
        "L2 / R2: switch teammate"
    }
}

/// Issue #182: the teammate view's label while the player watches: who, by
/// the name the kill feed gives him, and `hint`.
pub(super) fn spectate_label(
    walk: &WalkSim,
    feed: &game::event_feed::EventFeed,
    hint: &str,
) -> Option<ui::hud::HudSpectate> {
    if !walk.spectating() {
        return None;
    }
    Some(match walk.spectate_target() {
        Some(h) => ui::hud::HudSpectate {
            name: format!("Watching {}", feed.unit_name(&walk.sim, h)),
            hint: hint.to_owned(),
        },
        None => ui::hud::HudSpectate {
            name: "No teammates left to watch".to_owned(),
            hint: String::new(),
        },
    })
}

/// Whether [`spawn_model`] has a spawn screen for `walk.player` this frame
/// (it is `Some` exactly when this is true), without building it.
pub(super) fn spawn_screen_up(walk: &WalkSim) -> bool {
    let me = &walk.sim.units[walk.player.index as usize];
    let slot = me.spec.player_slot.map_or(0, usize::from);
    // Issue #28: a live player has a spawn screen only while the class
    // screen he opened at a friendly post is up.
    let alive_screen = walk.sim.spawn_stage.get(slot) == Some(&core::sim::SpawnStage::Alive);
    // A script popup hides the live class screen until it closes (B2
    // `FUN_0038c7a0` → `FUN_00397840` on state 4; `FUN_0038cb28` reopens).
    let hidden = alive_screen && walk.campaign.as_ref().is_some_and(|c| c.popup().is_some());
    if (me.alive && !alive_screen) || hidden || walk.sim.outcome.is_some() {
        return false;
    }
    // Issue #182: the teammate view hides the spawn screens until a spawn
    // is possible again.
    if walk.spectating() {
        return false;
    }
    // Ground death views wait for the native spawn map, including a dead
    // rider's held seat camera. No legacy waiting panel covers either view.
    !(!me.alive
        && !walk.space_battle()
        && walk.sim.spawn_screens
        && walk.sim.spawn_stage.get(slot) == Some(&core::sim::SpawnStage::Accepted))
}

/// Spawn screen model while the player is dead and the round is live; None
/// otherwise. The post list is the sim's ([`core::sim::Sim::player_spawn_posts`]:
/// the team's posts, then its live command walkers) and the selection is the
/// sim's spawn-map pick for this player's slot (card SPSEL), as is the class
/// row; the class names come from the disc's text through `feed`.
pub(super) fn spawn_model(
    walk: &WalkSim,
    feed: &game::event_feed::EventFeed,
) -> Option<ui::spawn::SpawnModel> {
    if !spawn_screen_up(walk) {
        return None;
    }
    let me = &walk.sim.units[walk.player.index as usize];
    let slot = me.spec.player_slot.map_or(0, usize::from);
    let alive_screen = walk.sim.spawn_stage.get(slot) == Some(&core::sim::SpawnStage::Alive);
    let b = &walk.sim.battle;
    let list = walk.sim.player_spawn_posts(me.team);
    let selected = walk
        .sim
        .spawn_post_selected(slot)
        .and_then(|p| list.iter().position(|q| q.0 == p));
    // A walker's post is named by its class `Label` (card CWSP,
    // re-command-walker-post §5); a command post by its own name.
    let posts: Vec<ui::spawn::SpawnPost> = list
        .iter()
        .map(|&(post, pos)| match post {
            core::unit::SpawnPost::Post(i) => {
                let c = &walk.sim.cps[usize::from(i)];
                ui::spawn::SpawnPost {
                    name: feed
                        .post_names()
                        .get(usize::from(i))
                        .cloned()
                        .unwrap_or_else(|| c.def.name.clone()),
                    // Enemies are neutralizing it right now.
                    contested: c.neutral > 0.0,
                    distance: (pos - me.soldier.pos).length(),
                }
            }
            core::unit::SpawnPost::Walker(h) => ui::spawn::SpawnPost {
                name: walk
                    .sim
                    .vehicle(h)
                    .and_then(|v| walk.vehicle_labels.get(v.class as usize))
                    .and_then(Clone::clone)
                    .unwrap_or_else(|| "Command walker".to_owned()),
                contested: false,
                distance: (pos - me.soldier.pos).length(),
            },
        })
        .collect();
    // A human waits for no spawn wave (card SSELA): its timer only.
    let countdown = {
        let secs = me.respawn_at.map_or(0.0, |t| {
            t.saturating_sub(walk.sim.tick) as f32 / core::TICK_HZ as f32
        });
        (secs > 0.0).then_some(secs)
    };
    // The sim's spawn screen for this slot (card SSELA), when it runs them.
    let screen = (walk.sim.spawn_screens || alive_screen).then(|| {
        use core::sim::{SideChoice, SpawnStage};
        match walk.sim.spawn_stage.get(slot).copied().unwrap_or_default() {
            SpawnStage::Side => ui::spawn::SpawnScreen::Side {
                rows: [
                    walk.teams[0].faction.clone(),
                    walk.teams[1].faction.clone(),
                    "Auto Assign".to_owned(),
                ],
                selected: match walk.sim.side_choice.get(slot).copied().unwrap_or_default() {
                    SideChoice::Team1 => 0,
                    SideChoice::Team2 => 1,
                    SideChoice::Auto => 2,
                },
            },
            SpawnStage::Map => ui::spawn::SpawnScreen::Map,
            SpawnStage::Class | SpawnStage::Alive => ui::spawn::SpawnScreen::Class,
            SpawnStage::Accepted => ui::spawn::SpawnScreen::Waiting,
        }
    });
    let page = game::spawn_page::page(walk, feed, slot);
    // The class row comes from the SIM: the table's order and the selection
    // `Sim::spawn_choice` for this player's slot, which the pad moves and the
    // respawn reads; each name is the disc's (`EventFeed::class_names`), else
    // the roster's label.
    let team = &walk.teams[usize::from(me.team.min(1))];
    let classes = walk.sim.spawn_class_indices(slot, me.team);
    let choice = walk.sim.spawn_class_choice(slot, me.team);
    let names = feed.class_names(me.team);
    Some(ui::spawn::SpawnModel {
        team_names: [walk.teams[0].faction.clone(), walk.teams[1].faction.clone()],
        player_team: me.team,
        tickets: [b.teams[0].tickets(), b.teams[1].tickets()],
        posts,
        selected,
        classes: classes
            .iter()
            .map(|&c| {
                names
                    .get(c)
                    .cloned()
                    .unwrap_or_else(|| team.classes[c].label.clone())
            })
            .collect(),
        selected_class: classes.iter().position(|&c| Some(c) == choice).unwrap_or(0),
        countdown,
        screen,
        page,
        icons: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctf_capture_regions_follow_the_mission_faction_not_the_slot() {
        let (att, def) = (glam::Vec3::X, glam::Vec3::Z);
        // The mission's first team is the attacker on every retail script.
        assert_eq!(
            ctf_capture_goals(["imp", "all"], ["imp", "all"], att, def),
            [att, def]
        );
        // A `--teams` override can put the armies the other way round.
        assert_eq!(
            ctf_capture_goals(["imp", "all"], ["all", "imp"], att, def),
            [def, att]
        );
        // Armies the mission does not name keep the mission's slot order.
        assert_eq!(
            ctf_capture_goals(["imp", "all"], ["rep", "cis"], att, def),
            [att, def]
        );
        assert_eq!(
            ctf_capture_goals(["rep", "cis"], ["CIS", "REP"], att, def),
            [def, att]
        );
    }

    #[test]
    fn a_target_behind_the_camera_keeps_its_side() {
        let view_proj = glam::Mat4::perspective_rh(60f32.to_radians(), 1.0, 0.1, 1000.0);
        // Right-handed: +Z is behind the camera. The marker for a target on
        // the right must sit on the right edge, not its mirror.
        let (right, on_screen) = marker_screen(view_proj, glam::Vec3::new(10.0, 0.0, 10.0));
        assert!(!on_screen);
        assert!(right[0] > 0.0, "{right:?}");
        let (left, _) = marker_screen(view_proj, glam::Vec3::new(-10.0, 0.0, 10.0));
        assert!(left[0] < 0.0, "{left:?}");
    }

    /// Issue #173, through the HUD model: a LAN guest's view (slot 1) with
    /// nothing under his own aim is not red while the host's (slot 0) aim
    /// is on an enemy. The same fixture on slot 0 is red.
    #[test]
    fn hud_reticle_colours_from_the_views_own_slot() {
        use core::sim::AimTarget;
        let mut walk = WalkSim::new(1, glam::Vec3::ZERO, None);
        let enemy = walk
            .sim
            .spawn(&core::unit::UnitSpec::trooper(
                1,
                glam::Vec3::new(0.0, 0.0, -20.0),
            ))
            .unwrap();
        walk.sim.reticle_pick = vec![Some((AimTarget::Unit(enemy), -1)), None];
        let mut feed = game::event_feed::EventFeed::default();
        let player = walk.player.index as usize;
        walk.sim.units[player].spec.player_slot = Some(1);
        let guest = hud_model(&walk, &mut feed, 16.0 / 9.0).reticle;
        assert_eq!(guest, Some(0), "the guest's reticle is not red");
        walk.sim.units[player].spec.player_slot = Some(0);
        let host = hud_model(&walk, &mut feed, 16.0 / 9.0).reticle;
        assert_eq!(host, Some(-1), "the host's reticle is red");
    }

    #[test]
    fn renamed_conquest_and_optional_mission_views_keep_existing_launch_paths() {
        let dir = std::env::temp_dir().join(format!("openbf-launch-alias-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("manifest.ron"),
            r#"(map: "tat2", source_game: "bf2", terrain_name: None, sky_name: None,
            terrain_glb: None, models: [], textures: [], classes: [], geometries: [],
            regions: [], instances: [], layers: [
                (name: "tat2_design", worlds: ["tat2_design"], regions: [], instances: [])
            ])"#,
        )
        .unwrap();
        let mission: game::mission::MissionDef = ron::from_str(
            r#"(map: "tat2", mode: "con", source_game: "bf2", teams: [
                (faction: "all", units: 20, tickets: 150),
                (faction: "imp", units: 20, tickets: 150)],
                ai_alive_cap: 16, spawn_delay: 10.0, spawn_interval: 0.25)"#,
        )
        .unwrap();
        let (scene, _) = game::assets::load_battle_scene(&dir, "con").unwrap();
        assert_ne!(dir.file_name().unwrap(), mission.map.as_str());
        validate_mission(&scene, Some(&mission)).unwrap();
        // Existing views can load a design fallback without a mission. The
        // timed runner and sim-trace enforce their stricter mode contract.
        let (scene, load) = game::assets::load_battle_scene(&dir, "ass").unwrap();
        assert_eq!(load.layer.as_deref(), Some("tat2_design"));
        validate_mission(&scene, None).unwrap();
        assert!(game::assets::load_battle_scene(&dir, "eli").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    mod native_spawn_tests {
        include!("battle/native_spawn_tests.rs");
    }

    mod space_ctf_kill_floor_tests {
        include!("battle/space_ctf_kill_floor_tests.rs");
    }

    /// HTGTW: use the battle's actual setup/install and feed constructors.
    /// The authored building is off its pivot: a radius-zero fallback cannot
    /// stand in for the collision bounds installed for acquisition.
    #[test]
    fn htgtw_battle_installs_turret_bounds_and_readout_names() {
        use core::sim::{AimTarget, PadState};
        let dir = std::env::temp_dir().join(format!("openbf-htgtw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // No models or textures: this fixture exercises the setup, not GPU upload.
        std::fs::write(
            dir.join("manifest.ron"),
            r#"(
            map: "htgtw", source_game: "bf2", terrain_name: None,
            sky_name: None, terrain_glb: None, models: [], textures: [],
            classes: [(name: "fixture_dish", base: "armedbuilding", geometry: Some("dish"))],
            geometries: [], regions: [],
            instances: [(class: "fixture_dish", name: "dish1",
                rotation: ((1,0,0),(0,1,0),(0,0,1)),
                translation: (0,0,-15), props: [])],
            collision: [(geometry: "dish", glb: None, prims: [(
                name: "dish_body", kind: "box", dims: (2,2,2),
                rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (1.5,1.5,0)
            )])]
        )"#,
        )
        .unwrap();
        let props = [
            ("BuildingSection", "TURRET1"),
            ("WeaponName", "fixture_gun"),
            ("MaxHealth", "300"),
            ("Label", "Fixture Dish"),
        ];
        let defs = vec![game::classdb::ClassDef {
            name: "fixture_dish".into(),
            kind: "entc".into(),
            base: "armedbuilding".into(),
            props: props
                .into_iter()
                .map(|(k, v)| (mungefs::fnv1a_lowercase(k), v.into()))
                .collect(),
        }];
        std::fs::write(dir.join("classes.ron"), ron::to_string(&defs).unwrap()).unwrap();
        let mut scene = game::assets::load_scene(&dir).unwrap();
        // A flat synthetic terrain grid grounds the player through make_walk.
        scene.terrain = Some(game::assets::CpuMesh {
            positions: vec![
                [-32.0, 0.0, -32.0],
                [32.0, 0.0, -32.0],
                [-32.0, 0.0, 32.0],
                [32.0, 0.0, 32.0],
            ],
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            submeshes: Vec::new(),
            spec: Vec::new(),
            light: Vec::new(),
            scroll: Vec::new(),
            groups: Vec::new(),
            sphere: None,
            vis_top: None,
            render_grid: None,
            contact_surface: false,
        });
        let setup = BattleSetup {
            seed: 42,
            difficulty: Default::default(),
            spawn: (0.0, 0.0),
            spawn_set: true,
            targets: false,
            ai: Some(0),
            ai_cap: None,
            team_size_pct: None,
            xl: false,
            tickets: 250,
            tickets_set: false,
            ticket_pct: 100,
            time_limit: 0,
            space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
            cheats: Default::default(),
            teams: None,
            classes_dir: dir.join("absent_sides"),
            mission: None,
            vehicle: None,
            mission_file: None,
            player_team: 0,
            heroes: Some(core::sim::heroes::HeroRules::default()),
            dlc_heroes: false,
            netplay_rows: false,
            opening_humans: [0; 2],
            score_limit: core::sim::deathmatch::ASSAULT_SCORE_LIMIT,
        };
        let mut walk = make_walk(&scene, &setup).unwrap();
        assert_eq!(walk.vehicle_classes, ["fixture_dish"]);
        let (h, v) = walk.sim.alive_vehicles().next().expect("installed turret");
        assert_eq!(v.instance, Some(0));
        let (centre, radius) = v.aim_sphere.expect("building sphere stored by install");
        // The fixture box is [-0.5,3.5] on x/y and [-2,2] on z.
        assert!(centre.distance(glam::Vec3::new(1.5, 1.5, 0.0)) < 1e-5);
        assert!((radius - 12.0_f32.sqrt()).abs() < 1e-5);
        let blast = walk.sim.blast_setup[v.class as usize].as_ref().unwrap();
        assert_eq!(blast.sphere_radius, 0.0, "no side-baked blast bounds");
        let player = &mut walk.sim.units[walk.player.index as usize];
        player.soldier.pitch = 0.0;
        player.soldier.yaw = 0.0;
        walk.sim.step(&[PadState::default()]);
        assert_eq!(
            walk.sim.reticle_pick[0],
            Some((AimTarget::Vehicle(h), 0)),
            "picked from 15 m"
        );
        let feed = game::event_feed::EventFeed::for_walk(&dir, "bf2", &walk);
        let target = feed.target(&walk.sim, 0).expect("picked turret readout");
        assert_eq!(target.name, "Fixture Dish");
        assert_eq!(target.health_fraction, 1.0);
        walk.sim.vehicles[h.index as usize].health.cur = 75.0;
        assert_eq!(feed.target(&walk.sim, 0).unwrap().health_fraction, 0.25);
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn instant_action_real_map_uses_ratios_tickets_and_one_human_slot() {
        let assets = game::assets::baked_root();
        let path = assets.join("bf1_nab2");
        if !path.join("manifest.ron").exists() {
            eprintln!("SKIP: no real bake");
            return;
        }
        let mut scene = game::assets::load_scene(&path).unwrap();
        scene.apply_mode_layer("con");
        let mut setup = BattleSetup {
            seed: 42,
            difficulty: Default::default(),
            spawn: (0.0, 0.0),
            spawn_set: false,
            targets: false,
            ai: None,
            ai_cap: None,
            team_size_pct: Some(100),
            xl: false,
            tickets: 250,
            tickets_set: false,
            ticket_pct: 100,
            time_limit: 0,
            space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
            cheats: Default::default(),
            teams: None,
            classes_dir: assets.join("classes"),
            mission: None,
            vehicle: None,
            mission_file: None,
            player_team: 1,
            heroes: Some(core::sim::heroes::HeroRules::default()),
            dlc_heroes: false,
            netplay_rows: false,
            opening_humans: [0; 2],
            score_limit: core::sim::deathmatch::ASSAULT_SCORE_LIMIT,
        };
        // Resolve missions independently of the bake location.
        setup.mission = Some(
            game::mission::MissionDef::load(&std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../missions/bf1_nab2_con.ron"
            )))
            .unwrap(),
        );
        assert_eq!(scene.manifest.map, "naboo2");
        assert_eq!(setup.mission.as_ref().unwrap().map, "bf1_nab2");
        let mut walk = make_walk(&scene, &setup).unwrap();
        // The map's own row (`conquest_defaults`): its team sizes and tickets.
        let d = game::conquest_defaults::for_map("bf1_nab2", false).unwrap();
        let c = d.combatants;
        assert_eq!(walk.sim.team_combatants, Some(c));
        assert_eq!(walk.sim.battle.teams.map(|t| t.alive()), c);
        // The human takes one of team 1's slots.
        assert_eq!(
            (walk.sim.ai_limit(0), walk.sim.ai_limit(1)),
            (c[0], c[1] - 1)
        );
        assert_eq!(
            walk.sim
                .units
                .iter()
                .filter(|u| u.is_player && u.alive)
                .count(),
            1
        );
        assert_eq!(walk.sim.battle.teams.map(|t| t.tickets()), d.tickets);
        // Issue #171: at Hard the humans' side (team 1 here) gets the map's
        // `HUMAN_SHIFT`, then the reinforcement percentage scales the pair.
        setup.difficulty = core::skill::Difficulty::Hard;
        setup.ticket_pct = 50;
        let hard = make_walk(&scene, &setup).unwrap();
        let shifted = game::conquest_defaults::human_tickets(
            "bf1_nab2",
            false,
            core::skill::Difficulty::Hard,
            d.tickets,
            [0, 1],
        );
        assert_ne!(shifted, d.tickets, "bf1_nab2 GCW has a Hard shift");
        assert_eq!(
            hard.sim.battle.teams.map(|t| t.tickets()),
            ui::population::tickets(shifted, 50)
        );
        setup.difficulty = Default::default();
        setup.ticket_pct = 100;
        let before = walk.sim.battle.teams.map(|t| t.tickets());
        // Team sizes scale by the percentage (capped at 32 a side).
        let pct = |p: u32| ui::population::team_counts(c, p);
        walk.set_team_size_percent(50);
        assert_eq!(walk.sim.battle.teams.map(|t| t.alive()), pct(50));
        assert_eq!(walk.sim.battle.teams.map(|t| t.tickets()), before);
        let pending: Vec<_> = walk
            .sim
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| !u.alive && u.respawn_at.is_some())
            .map(|(i, u)| (i, u.gen, u.respawn_at))
            .collect();
        walk.set_team_size_percent(100);
        assert_eq!(
            (walk.sim.ai_limit(0), walk.sim.ai_limit(1)),
            (c[0], c[1] - 1)
        );
        assert_eq!(
            walk.sim.battle.teams.map(|t| t.alive()),
            pct(50),
            "raising the cap must not bypass respawn waves"
        );
        walk.set_team_size_percent(150);
        assert_eq!(walk.sim.team_combatants, Some(pct(150)));
        for (i, gen, at) in pending {
            let u = &walk.sim.units[i];
            assert_eq!((u.gen, u.respawn_at), (gen, at));
            assert!(!u.alive);
        }
        assert_eq!(walk.sim.battle.teams.map(|t| t.tickets()), before);
        // The human changes side: his old team gets a bot in his slot.
        let roster = |w: &WalkSim, t: u8| {
            w.sim
                .units
                .iter()
                .filter(|u| !u.is_player && u.team == t && (u.alive || u.respawn_at.is_some()))
                .count() as u32
        };
        walk.step_tick(&[core::sim::PadState::default()]);
        assert_eq!(roster(&walk, 1), walk.sim.ai_limit(1));
        let p = walk.player.index as usize;
        walk.sim.units[p].team = 0;
        walk.sim.units[p].spec.team = 0;
        walk.step_tick(&[core::sim::PadState::default()]);
        assert_eq!(walk.sim.ai_limit(1), pct(150)[1]);
        assert_eq!(
            roster(&walk, 1),
            pct(150)[1],
            "the old team refills the human's slot"
        );
        // A plain map walk (no mission) takes no map defaults, even with a
        // percentage set: the scene folder's name must not start a conquest.
        setup.mission = None;
        setup.ai = Some(0);
        let walk = make_walk(&scene, &setup).unwrap();
        assert_eq!(walk.conquest_defaults, None);
        assert_eq!(walk.sim.team_combatants, None);
        assert_eq!(walk.sim.battle.teams.map(|t| t.tickets()), [250; 2]);
    }

    /// Session Options' Heroes row reaches the battle: Off installs no
    /// hero (no class appended, no manager); On installs the session's rules.
    #[test]
    fn heroes_off_installs_nothing_and_on_carries_the_session_rules() {
        let assets = game::assets::baked_root();
        let path = assets.join("bf2_cor1");
        let missions =
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
        if !path.join("manifest.ron").exists() || !missions.join("bf2_cor1_con_cw.ron").exists() {
            eprintln!("SKIP: no real BF2 bake or missions");
            return;
        }
        let mut scene = game::assets::load_scene(&path).unwrap();
        scene.apply_mode_layer("con");
        let mission =
            game::mission::MissionDef::load(&missions.join("bf2_cor1_con_cw.ron")).unwrap();
        let mut session = ui::shell::SessionOptions::default();
        let mut walks = Vec::new();
        for on in [false, true] {
            session.heroes = on;
            session.hero_unlock = 5;
            session.hero_unlock_val = 42;
            session.hero_respawn = -1;
            let setup = BattleSetup {
                seed: 42,
                difficulty: Default::default(),
                spawn: (0.0, 0.0),
                spawn_set: false,
                targets: false,
                ai: None,
                ai_cap: None,
                team_size_pct: Some(100),
                xl: false,
                tickets: 250,
                tickets_set: false,
                ticket_pct: 100,
                time_limit: 0,
                space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
                cheats: Default::default(),
                teams: Some(mission.factions()),
                classes_dir: assets.join("classes"),
                mission: Some(mission.clone()),
                vehicle: None,
                mission_file: None,
                player_team: 0,
                heroes: game::heroes::rules(&session),
                dlc_heroes: false,
                netplay_rows: false,
                opening_humans: [0; 2],
                score_limit: core::sim::deathmatch::ASSAULT_SCORE_LIMIT,
            };
            walks.push(make_walk(&scene, &setup).unwrap());
        }
        let (off, on) = (&walks[0], &walks[1]);
        assert!(off.sim.heroes.is_none(), "Off: no manager");
        assert_eq!(off.sim.hero_class(0), None);
        for t in 0..2 {
            assert_eq!(
                on.sim.classes[t].len(),
                off.sim.classes[t].len() + 1,
                "Off: no hero class"
            );
        }
        let rules = on.sim.heroes.as_ref().unwrap().rules;
        assert_eq!(rules.unlock, core::sim::heroes::HeroUnlock::Timer(42));
        assert_eq!(rules.respawn_ticks, None);
    }

    /// A held seat view keeps the waiting panel in space. On ground the
    /// native spawn map is the first menu after death.
    #[test]
    fn death_without_a_camera_keeps_the_waiting_panel_only_in_space() {
        use core::sim::SpawnStage;
        let flat = core::heightfield::Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        };
        let feed = game::event_feed::EventFeed::default();
        let mut walk = WalkSim::new(1, glam::Vec3::ZERO, Some(flat));
        walk.sim.spawn_screens = true;
        walk.sim.spawn_stage = vec![SpawnStage::Accepted];
        walk.step_tick(&[core::sim::PadState::default()]);
        walk.sim
            .kill_environment(walk.player, core::sim::DeathCause::Suicide);
        walk.step_tick(&[core::sim::PadState::default()]);
        assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Accepted);
        walk.death_cam = None;
        assert!(spawn_model(&walk, &feed).is_none());

        walk.sim
            .landing_regions
            .push(core::region::Region::sphere(glam::Vec3::ZERO, 10.0));
        assert!(matches!(
            spawn_model(&walk, &feed).and_then(|m| m.screen),
            Some(ui::spawn::SpawnScreen::Waiting)
        ));
    }

    /// Playtest A7: no spawn panel over the death camera; the spawn map
    /// comes when the timer runs out. A space map keeps its panel.
    #[test]
    fn no_spawn_panel_while_the_death_camera_runs() {
        use core::sim::SpawnStage;
        let flat = || core::heightfield::Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        };
        let feed = game::event_feed::EventFeed::default();
        for space in [false, true] {
            let mut walk = WalkSim::new(1, glam::Vec3::ZERO, Some(flat()));
            if space {
                // Only the space scripts add landing regions.
                let region = core::region::Region::sphere(glam::Vec3::ZERO, 10.0);
                walk.sim.landing_regions.push(region);
            }
            walk.sim.spawn_screens = true;
            walk.sim.spawn_stage = vec![SpawnStage::Accepted];
            walk.step_tick(&[core::sim::PadState::default()]);
            assert!(spawn_model(&walk, &feed).is_none(), "alive");
            walk.sim
                .kill_environment(walk.player, core::sim::DeathCause::Suicide);
            let mut ticks = 0;
            while walk.sim.spawn_stage[0] == SpawnStage::Accepted {
                walk.step_tick(&[core::sim::PadState::default()]);
                ticks += 1;
                assert!(ticks < 200, "the spawn map never opened");
                let model = spawn_model(&walk, &feed);
                assert_eq!(model.is_some(), spawn_screen_up(&walk), "tick {ticks}");
                if walk.sim.spawn_stage[0] == SpawnStage::Accepted {
                    match space {
                        false => assert!(model.is_none(), "tick {ticks}"),
                        true => assert!(matches!(
                            model.and_then(|m| m.screen),
                            Some(ui::spawn::SpawnScreen::Waiting)
                        )),
                    }
                }
            }
            assert!(ticks >= 4 * core::TICK_HZ as usize, "{ticks} ticks");
            let screen = spawn_model(&walk, &feed).and_then(|m| m.screen);
            assert!(matches!(screen, Some(ui::spawn::SpawnScreen::Map)));
        }
    }

    /// Issue #182: with no post to spawn at, the spawn map gives way to the
    /// teammate view after its hold (no spawn page, a "Watching" label), and
    /// comes back when a post does.
    #[test]
    fn the_teammate_view_hides_the_spawn_map_until_a_post_is_back() {
        use core::sim::SpawnStage;
        let flat = core::heightfield::Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        };
        let mut feed = game::event_feed::EventFeed::default();
        let mut walk = WalkSim::new(1, glam::Vec3::ZERO, Some(flat));
        let mate = walk
            .sim
            .spawn(&core::unit::UnitSpec::trooper(
                0,
                glam::Vec3::new(5.0, 0.0, -5.0),
            ))
            .unwrap();
        walk.sim.spawn_screens = true;
        walk.sim.spawn_stage = vec![SpawnStage::Accepted];
        let idle = [core::sim::PadState::default()];
        walk.step_tick(&idle);
        walk.sim
            .kill_environment(walk.player, core::sim::DeathCause::Suicide);
        let mut ticks = 0;
        while walk.sim.spawn_stage[0] == SpawnStage::Accepted {
            walk.step_tick(&idle);
            ticks += 1;
            assert!(ticks < 300, "the spawn map never opened");
        }
        // The spawn map opens first; the view takes over after the hold.
        assert!(!walk.spectating());
        assert!(spawn_model(&walk, &feed).is_some());
        for _ in 0..(game::walk::SPECTATE_HOLD_TICKS + 1) {
            walk.step_tick(&idle);
        }
        assert!(walk.spectating());
        assert_eq!(walk.spectate_target(), Some(mate));
        assert!(
            spawn_model(&walk, &feed).is_none(),
            "no spawn page behind it"
        );
        let hud = hud_model(&walk, &mut feed, 16.0 / 9.0);
        assert!(hud.spawn.is_none());
        let label = hud.spectate.expect("the label");
        assert!(label.name.starts_with("Watching "), "{}", label.name);
        assert_eq!(label.hint, KEYBOARD_SPECTATE_HINT);
        // A post of his team's with a spawn node: the spawn map is back.
        let at = glam::Vec3::new(0.0, 0.0, 20.0);
        walk.sim.add_cp(
            core::cp::CpDef {
                name: "cp".into(),
                pos: at,
                capture: core::region::Region::sphere(at, 8.0),
                control: None,
                value_bleed: 1,
                value_atk: [1, 1],
                value_def: [1, 1],
                spawn_nodes: vec![(at + glam::Vec3::X * 4.0, 0.0)],
                capture_time: 10.0,
                neutralize_time: 10.0,
            },
            Some(0),
        );
        walk.step_tick(&idle);
        assert!(!walk.spectating());
        assert!(hud_model(&walk, &mut feed, 16.0 / 9.0).spectate.is_none());
        assert!(matches!(
            spawn_model(&walk, &feed).and_then(|m| m.screen),
            Some(ui::spawn::SpawnScreen::Map)
        ));
    }
}
