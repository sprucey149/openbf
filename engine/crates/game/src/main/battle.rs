//! The battle every mode builds and shows: a baked map loaded and uploaded
//! with its common assets ([`load_and_upload`]), the two teams from the class
//! data, the `WalkSim` set up from the CLI or the shell ([`make_walk`]), and
//! what a frame draws of it besides the scene — the marker, target, bolt and
//! post boxes ([`walk_draws`]), the squad icons, and the HUD and spawn-screen
//! models the `ui` crate paints.

use super::BattleSetup;
use anyhow::Result;
use game::viewer::{upload_scene, SceneGpu};
use game::walk::WalkSim;
use render::{DrawCmd, MeshHandle, Renderer};

/// Walk-mode soldier marker tint (orange — reads on sand and rock).
const MARKER_COLOR: [f32; 4] = [0.9, 0.45, 0.1, 1.0];

pub(super) fn load_and_upload(
    r: &mut Renderer,
    dir: &std::path::Path,
    mode: &str,
) -> Result<(SceneGpu, game::assets::Scene)> {
    let (scene, load) = game::assets::load_battle_scene(dir, mode)?;
    if let Some(layer) = &load.layer {
        println!("mode layer '{layer}' merged");
    }
    match &load.common {
        Some((cd, Ok(n))) => println!("common assets merged from {} (+{n} models)", cd.display()),
        Some((cd, Err(e))) => eprintln!("common assets at {} unreadable: {e:#}", cd.display()),
        None => {}
    }
    let gpu = upload_scene(r, &scene)?;
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
    Ok((gpu, scene))
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
    let player_team = walk.sim.units[walk.player.index as usize].team;
    if walk.player_on_foot() && !walk.player_body_hidden() && !has_model(player_team) {
        draws.push(marker_draw(cube, walk));
    }
    for t in walk.target_draws() {
        if has_model(t.team) {
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
                // Heroes Off: no hero class at all (Session Options).
                if b.heroes.is_some() {
                    hero_classes[team] = game::heroes::append_with(
                        &mut r,
                        &b.classes_dir,
                        b.mission.as_ref(),
                        team,
                        b.dlc_heroes,
                    );
                }
                let mut table = Vec::new();
                let mut visuals = Vec::new();
                for c in &r.classes {
                    let uc = c.register(sim, &mut names);
                    println!(
                        "team {} class {}: {} — min {} — {} HP, {:.1} m/s, {} ({} dmg, {} m/s bolts){}",
                        r.faction,
                        c.class,
                        c.label,
                        c.min,
                        c.max_health,
                        c.consts.max_speed,
                        c.weapon.name,
                        c.weapon.ordnance.max_damage,
                        c.weapon.ordnance.velocity,
                        if c.jet { "  [jet pack]" } else { "" }
                    );
                    table.push(uc);
                    visuals.push(game::walk::ClassVisual::from_roster_with_held(c));
                }
                sim.classes[team] = table;
                team += 1;
                game::walk::TeamTemplate::from_roster(&r, &sim.classes[team - 1][0], visuals)
            });
            if let Some(rules) = b.heroes {
                game::heroes::install_with(sim, hero_classes, rules);
            }
            Ok(templates)
        }
        Err(e) => {
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
    if !walk.sim.open_spawn_screens(0, true) {
        eprintln!("spawn screens: slot 0 holds no live unit; the respawn stays automatic");
        return;
    }
    walk.end_shots = mission.map(|m| m.camera_shots()).unwrap_or_default();
    walk.backdrop = mission.and_then(|m| {
        game::spawn_backdrop::Backdrop::new(m.camera_shots(), game::spawn_backdrop::LCG_RESEED)
    });
    walk.follow_backdrop();
}

/// The end of the round as the HUD draws it (issue #33): the result, then
/// the stats pages (issue #50) once they are made.
fn round_end_model(r: &game::round_end::RoundEnd) -> ui::hud::RoundEndModel {
    use game::round_end::Result;
    ui::hud::RoundEndModel {
        result: match r.result {
            Result::Victory => 0,
            Result::Defeat => 1,
            Result::Draw => 2,
        },
        stats: r.stats.clone(),
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

/// Refuse an Assault round without its matching mission. Other modes retain
/// optional missions and allow copied or renamed asset directories.
fn validate_mission(
    scene: &game::assets::Scene,
    mission: Option<&game::mission::MissionDef>,
) -> Result<()> {
    let assault = scene.manifest.spawn_only_posts;
    anyhow::ensure!(
        !assault || mission.is_some(),
        "Hero Assault requires a mission"
    );
    if let Some(m) = mission.filter(|m| assault || m.is_hero_assault()) {
        anyhow::ensure!(
            assault == m.is_hero_assault(),
            "mission mode {} differs from the loaded map mode",
            m.mode
        );
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
    let mut sim = core::sim::Sim::empty(b.seed);
    // The tier is set BEFORE `setup_conquest` spawns anyone: every bot takes
    // its skill at spawn (D2).
    sim.difficulty = b.difficulty;
    // BF1 has no 10 m omni in the view cone (RE §9.1); BF2 does.
    sim.omni_radius = core::sim::omni_radius_for(&scene.manifest.source_game);
    scene.install_water(&mut sim);
    scene.install_damage_regions(&mut sim);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.item_pool = game::roster::item_pool(&scene.manifest.source_game, &scene.manifest.map);
    sim.cheats = b.cheats;
    // Map defaults belong to a conquest (a mission); a plain map walk has none.
    // The mission carries the game's unique key (bf1_nab2), not the manifest's
    // world name (naboo2).
    let map_defaults = b
        .mission
        .as_ref()
        .and_then(|m| game::conquest_defaults::for_map(&m.map, m.era.as_deref() == Some("cw")));
    let defaults = b.team_size_pct.and(map_defaults);
    let resolved = defaults.map(|d| d.scaled(b.team_size_pct.unwrap_or(100), b.ticket_pct));
    let tickets = if b.tickets_set {
        [b.tickets; 2]
    } else if let Some(d) = resolved {
        d.tickets
    } else {
        b.mission
            .as_ref()
            .map_or([b.tickets; 2], |m| m.tickets())
            .map(|n| core::sim::scale_reinforcements(n, b.ticket_pct))
    };
    sim.battle = core::battle::Battle::new(tickets);
    sim.time_limit = (b.time_limit > 0).then(|| b.time_limit * 60);
    if let Some(m) = &b.mission {
        sim.ai_alive_cap = m.ai_alive_cap;
        m.apply_death_drains(&mut sim.battle);
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
    // Only a battle that uses percentages can change them in the pause menu.
    walk.conquest_defaults = defaults;
    walk.team_size_pct = b.team_size_pct.unwrap_or(100);
    walk.reinforcement_pct = b.ticket_pct;
    walk.class_picker = Some(game::class_picker::ClassPicker::new(&b.classes_dir, scene));
    // Model collision: the scene has already had its mode layer applied (see
    // `Scene::collision_world` — building it earlier misses every BF2 conquest
    // instance).
    scene.install_collision(&mut walk.sim);
    walk.camera_blockers = game::camera_collide::CameraBlockers::from_scene(scene);
    if b.targets {
        walk.spawn_dummy_targets();
    }
    // Percentages use the map's combatant slots. A bench's explicit counts
    // win, then --ai; legacy CLI launches without percentages use authored
    // mission counts.
    let ai_counts = counts.or(if let Some(n) = b.ai {
        Some([n; 2])
    } else if let Some(d) = resolved {
        Some(d.initial_ai(b.player_team))
    } else {
        b.mission
            .as_ref()
            .map(|m| [m.initial_ai(0, seat == 0), m.initial_ai(1, seat == 1)])
    });
    let mut vehicle_classes: Vec<String> = Vec::new();
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
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
        // The mission's AI settings (card MAIS), its capture bans by post.
        if let Some(m) = &b.mission {
            m.apply_ai(&mut walk.sim);
        }
        // The vehicle spawn pads follow the posts just added (card VPAD) and
        // go in before the opening wave, which spawns at the command walkers
        // they are about to build (card CWSP).
        let factions = [walk.teams[0].faction.clone(), walk.teams[1].faction.clone()];
        let space = b.mission.as_ref().filter(|m| m.mode == "ass").map(|m| {
            (
                m.space_assault.clone().unwrap_or_default(),
                b.space_score_limit,
            )
        });
        match game::vehicle_pads::install_with_objective(
            &mut walk.sim,
            &scene.manifest,
            &scene.dir,
            &b.classes_dir,
            &factions,
            space.as_ref().map(|(def, limit)| (def, *limit)),
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
                walk.load_walker_views(&scene.manifest, scene.dir.parent().unwrap_or(&scene.dir));
                walk.load_vehicle_tops(
                    &scene.manifest,
                    &scene.dir,
                    scene.dir.parent().unwrap_or(&scene.dir),
                );
                walk.vehicle_riders = p.riders;
                walk.vehicle_engines = p.engines;
                walk.vehicle_labels = p.labels;
                walk.vehicle_classes = p.classes.clone();
                vehicle_classes = p.classes;
            }
            Err(e) => eprintln!("vehicle pads unavailable ({e:#})"),
        }
        walk.opening_wave(counts, !b.spawn_set);
        // The commander speaks the mission's lines over the posts and the
        // battle just set up.
        if let Some(m) = &b.mission {
            let assets_dir = scene.dir.parent().unwrap_or(&scene.dir);
            game::native_teams::install(&mut walk, &layout, &b.classes_dir, assets_dir, m);
            walk.sound_levels = m.sound_levels();
            walk.space_round = m.space_assault.as_ref().map(|d| {
                let mut round = game::space_assault::SpaceRound::new(d);
                round.sheet_dir = Some(assets_dir.join(&m.map).join("textures"));
                std::sync::Arc::new(round)
            });
            walk.voice = Some(game::announce::Commander::for_battle(m, &layout, &walk));
            // ...and the music is the script's too, by the reinforcements
            // the teams start with.
            let tickets = [0, 1].map(|t| walk.sim.battle.teams[t].tickets());
            let bf2 = m.source_game == "bf2";
            walk.music = Some(game::music::BattleMusic::new(&m.music, bf2, tickets));
            let names = [0u8, 1].map(|team| {
                walk.sim.hero_class(team).and_then(|class| {
                    walk.teams[usize::from(team)]
                        .class(class)
                        .cries
                        .hero
                        .music
                        .clone()
                })
            });
            walk.music.as_mut().unwrap().set_hero_music(names);
        }
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

/// HUD model from the battle state. Takes `feed`'s new event lines: each
/// model must reach the HUD (`HudView::update`) or its lines are lost.
fn hud_lock(
    walk: &WalkSim,
    feed: &game::event_feed::EventFeed,
    slot: usize,
    aspect: f32,
) -> Option<ui::hud::HudLock> {
    let (target, pos) = walk.sim.lock_on_target(slot)?;
    let hud = feed.lock_target(&walk.sim, target)?;
    let camera = walk.camera();
    let clip = camera.view_proj(aspect) * pos.extend(1.0);
    let w = clip.w.abs().max(1.0e-4);
    let (mut x, mut y) = (clip.x / w, -clip.y / w);
    let bound = x.abs().max(y.abs());
    const EDGE: f32 = 0.9;
    let on_screen = clip.w > 0.0 && bound <= EDGE;
    if !on_screen && bound > 0.0 {
        x *= EDGE / bound;
        y *= EDGE / bound;
    }
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
        visible: walk.sim.lock_on_visible(slot),
        ready: [
            walk.sim.lock_on_ready(slot, 0),
            walk.sim.lock_on_ready(slot, 1),
        ],
        flyer: true,
        pulse: walk.sim.tick as f32 / core::TICK_HZ as f32,
    })
}

pub(super) fn hud_model(
    walk: &WalkSim,
    feed: &mut game::event_feed::EventFeed,
    aspect: f32,
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
        (Some((r, c)), _) => (r.health, c.health, r.energy, c.energy),
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
    let [weapon, weapon2] = game::hud_feed::weapon_rows(walk);
    let gauge = game::hud_feed::weapon_gauge(walk);
    let leaving = walk
        .sim
        .is_leaving(walk.player)
        .then(|| feed.leaving_text());
    let reticle =
        (on_foot || droid.is_some() || guided.is_some() || game::hud_feed::armed_seat(walk)).then(
            || {
                walk.sim
                    .reticle_pick
                    .first()
                    .copied()
                    .flatten()
                    .map_or(0, |(_, rel)| rel)
            },
        );
    ui::hud::HudModel {
        team_names: [walk.teams[0].faction.clone(), walk.teams[1].faction.clone()],
        tickets: [b.teams[0].tickets(), b.teams[1].tickets()],
        bleed: [b.teams[0].bleed_per_sec, b.teams[1].bleed_per_sec],
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
        reticle,
        // The reticle's pick by name and health (card HTGT).
        target: on_foot.then(|| feed.target(&walk.sim, slot)).flatten(),
        lock: hud_lock(walk, feed, slot, aspect),
        reticle_y: walk.reticle_y(),
        dead: !me.alive,
        // A human waits for no spawn wave (card SSELA): its timer only.
        respawn_in: (!me.alive).then(|| {
            me.respawn_at.map_or(0.0, |t| {
                t.saturating_sub(walk.sim.tick) as f32 / core::TICK_HZ as f32
            })
        }),
        team_points: walk.sim.space_team_points().or_else(|| {
            walk.sim
                .team_points()
                .map(|(points, limit)| (points.map(|p| p as i32), limit))
        }),
        space: space_status,
        outcome: walk.sim.outcome,
        countdown: walk
            .sim
            .domination_left()
            .map(|(holder, secs)| (holder == me.team, secs)),
        round_end: walk.round_end.as_ref().map(round_end_model),
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
            _ => walk.space_round.is_none().then(|| m.model(walk)),
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
    }
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
    let me = &walk.sim.units[walk.player.index as usize];
    let slot = me.spec.player_slot.map_or(0, usize::from);
    // Issue #28: a live player has a spawn screen only while the class
    // screen he opened at a friendly post is up.
    let alive_screen = walk.sim.spawn_stage.get(slot) == Some(&core::sim::SpawnStage::Alive);
    if (me.alive && !alive_screen) || walk.sim.outcome.is_some() {
        return None;
    }
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
            tickets: 250,
            tickets_set: false,
            ticket_pct: 100,
            time_limit: 0,
            space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
            cheats: Default::default(),
            teams: None,
            classes_dir: dir.join("absent_sides"),
            mission: None,
            mission_file: None,
            player_team: 0,
            heroes: Some(core::sim::heroes::HeroRules::default()),
            dlc_heroes: false,
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
            tickets: 250,
            tickets_set: false,
            ticket_pct: 100,
            time_limit: 0,
            space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
            cheats: Default::default(),
            teams: None,
            classes_dir: assets.join("classes"),
            mission: None,
            mission_file: None,
            player_team: 1,
            heroes: Some(core::sim::heroes::HeroRules::default()),
            dlc_heroes: false,
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
        assert_eq!(walk.sim.team_combatants, Some([19, 21]));
        assert_eq!(
            (
                walk.sim.battle.teams[0].alive(),
                walk.sim.battle.teams[1].alive()
            ),
            (19, 21)
        );
        assert_eq!((walk.sim.ai_limit(0), walk.sim.ai_limit(1)), (19, 20));
        assert_eq!(
            walk.sim
                .units
                .iter()
                .filter(|u| u.is_player && u.alive)
                .count(),
            1
        );
        let d = game::conquest_defaults::for_map("bf1_nab2", false).unwrap();
        assert_eq!(walk.sim.battle.teams.map(|t| t.tickets()), d.tickets);
        let before = walk.sim.battle.teams.map(|t| t.tickets());
        walk.set_team_size_percent(50);
        assert_eq!(walk.sim.battle.teams.map(|t| t.alive()), [10, 11]);
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
        assert_eq!((walk.sim.ai_limit(0), walk.sim.ai_limit(1)), (19, 20));
        assert_eq!(
            walk.sim.battle.teams.map(|t| t.alive()),
            [10, 11],
            "raising the cap must not bypass respawn waves"
        );
        walk.set_team_size_percent(150);
        assert_eq!(walk.sim.team_combatants, Some([29, 32]));
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
        assert_eq!(walk.sim.ai_limit(1), 32);
        assert_eq!(
            roster(&walk, 1),
            32,
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
                tickets: 250,
                tickets_set: false,
                ticket_pct: 100,
                time_limit: 0,
                space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
                cheats: Default::default(),
                teams: Some(mission.factions()),
                classes_dir: assets.join("classes"),
                mission: Some(mission.clone()),
                mission_file: None,
                player_team: 0,
                heroes: game::heroes::rules(&session),
                dlc_heroes: false,
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
}
