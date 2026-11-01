//! Phase C's tuning instrument and acceptance referee (D14): a seeded N-v-N
//! bot battle, run once per difficulty tier, printing one comparable row each.
//!
//! ```text
//! cargo run --release -p game --example arena -- [ticks] [seed=42] [flat|<map_key>] [tier=all]
//! cargo run --release -p game --example arena -- [ticks] [seed=42] [flat|<map_key>] vs <a>:<b>|all [seeds=1]
//! ```
//!
//! `ticks` defaults to 3,000 on the flat field and 9,000 on a map. The target
//! defaults to `flat`; the tier to `all`, which runs the four in order and
//! adds a `monotonic:` verdict line.
//!
//! ## The two fields
//! - **flat** — a bare [`core::sim::Sim`] on a 400 m flat heightfield (the
//!   `core::ai` test rig's shape), NO command posts and NO nav graph, 8 v 8
//!   `UnitSpec::trooper` bots on two lines 30 m apart facing each other. 30 m
//!   is inside every tier's view distance (Easy and Normal see least: 49 m).
//!   With no posts there are no spawn points, so it is a ONE-LIFE fight and
//!   the run ends at `ticks` or when a team is wiped out. `omni_radius` 10 m
//!   (the BF2 value) — no manifest to read it from.
//! - **a map key** — `round_to_outcome`'s conquest setup, kept at 16 v 16
//!   (the sweep now plays each map's own sizes) with the idle player, the same map for every tier so the rows compare. The setup
//!   below is COPIED from `round_to_outcome::run_map`, not shared: cargo
//!   examples cannot import each other. `SWEEP_SLOT0=bot` makes slot 0 a bot
//!   here too (the sweep's default; see `slot0_is_bot` for why this example's
//!   default is the other one).
//!
//! The `all` rows put ONE tier on both teams, so `kills/min` measures how
//! deadly a fight is, not which tier wins it: a tier that is harder to hit
//! lowers both sides' kills (card TIER: Elite's flat-field kills / min
//! fell under Hard's). Its `monotonic:` line is a row of the determinism gate,
//! not a verdict on the tiers; the head-to-head below is.
//!
//! ## The head-to-head (card ARVS)
//! `vs a:b` puts tier `a` on team 0 and tier `b` on team 1
//! ([`Sim::team_difficulty`]; equal tiers set no override, so `vs normal:normal`
//! is `normal`'s run hash for hash) and prints one row per seed with each
//! side's kills, hits / shots, first kill and the margin (tickets left on a
//! map, survivors on the flat field). `vs all` runs every neighbouring pair
//! (easy:normal, normal:hard, hard:elite) both ways round on each seed, so the
//! spawn side cancels, and ends with one `beats:` line per pair: the higher
//! tier's share of the pair's kills (both runs of a seed summed), its paired t
//! against an even 0.5 over the seeds, how many seeds it won and its mean
//! margin. The verdict is pre-set: t >= 2 `yes`, t <= -2 `REVERSED`, else
//! `coin flip`. `[seeds]` runs `seed .. seed + seeds`, in parallel
//! (`ARENA_JOBS`, default 8); the output is in seed order whatever the jobs.
//!
//! ## The row
//! `tier  kills/min  hit_rate  ttfk_s  shots_per_kill  ai_shots  unperceived  hash`
//! folded from `sim.events` tick by tick (`Sim::step` clears them each tick):
//! kills are `Event::Kill` with BOTH ends AI — the idle player's deaths and
//! the AI's hits on it are excluded and printed separately as `player_hits`,
//! because a map row would otherwise mix a stationary free target into the
//! bots' hit rate. `ai_shots` and `unperceived` are `NavStats`' own counters;
//! the event fold's shot count is printed beside them when the two disagree,
//! which would be a bug in one of them.
//!
//! ## The human proxy (card DPRX)
//! `ARENA_PROXY=<team>:<average|good>` makes the first bot of `<team>` on
//! foot a HUMAN PROXY (`core::ai::proxy`: a bot the sim treats as a human for
//! every human-only rule, with a fixed human profile) on either field, and
//! prints one stderr `proxy` line per run with its tally (kills, deaths,
//! shots, hits, accuracy, seconds alive per life; `game::blackbox::session`'s
//! definitions). Its kills still count in the row's AI columns.
//!
//! Two runs with the same arguments print byte-identical STDOUT — wall time,
//! the one column that cannot be, goes to STDERR, as the sweep's progress
//! does — and a different seed prints a different fight. The `#[cfg(test)]`
//! module at the bottom pins that in process; `game/Cargo.toml` declares this
//! example with `test = true` so `cargo test` runs it.

use core::sim::{PadState, Sim};
use core::skill::Difficulty;
use core::unit::{Controller, UnitSpec};
use game::walk::WalkSim;
use glam::Vec3;
use std::path::Path;

/// Ticks per second — the fixed sim rate every rate column divides by.
const HZ: f64 = core::TICK_HZ as f64;

/// One tier's row.
struct Row {
    /// Team 0's and team 1's tier (equal outside `vs`).
    tiers: [Difficulty; 2],
    /// Per team: AI-on-AI kills, AI shots, AI hits on AI, first kill tick.
    side: [Side; 2],
    /// Per team: tickets left on a map, units alive on the flat field.
    margin: [f64; 2],
    kills: u64,
    ai_shots_events: u64,
    ai_hits: u64,
    ttfk: Option<u64>,
    player_hits: u64,
    player_deaths: u64,
    nav_ai_shots: u64,
    unperceived: u64,
    hash: u64,
    ticks: u64,
    wall_s: f32,
    alive: [usize; 2],
}

impl Row {
    fn kills_per_min(&self) -> f64 {
        self.kills as f64 / (self.ticks as f64 / HZ / 60.0)
    }

    fn hit_rate(&self) -> f64 {
        if self.ai_shots_events == 0 {
            0.0
        } else {
            self.ai_hits as f64 / self.ai_shots_events as f64
        }
    }

    fn shots_per_kill(&self) -> f64 {
        if self.kills == 0 {
            f64::INFINITY
        } else {
            self.ai_shots_events as f64 / self.kills as f64
        }
    }

    fn print(&self) {
        let ttfk = match self.ttfk {
            Some(t) => format!("{:8.2}", t as f64 / HZ),
            None => format!("{:>8}", "-"),
        };
        let spk = if self.kills == 0 {
            format!("{:>8}", "-")
        } else {
            format!("{:8.1}", self.shots_per_kill())
        };
        println!(
            "{:<7} {:9.2} {:9.3} {ttfk} {spk} {:8} {:6} {:016x}  \
             ticks {}  alive {}/{}  player_hits {}  player_deaths {}",
            self.tiers[0].label(),
            self.kills_per_min(),
            self.hit_rate(),
            self.nav_ai_shots,
            self.unperceived,
            self.hash,
            self.ticks,
            self.alive[0],
            self.alive[1],
            self.player_hits,
            self.player_deaths,
        );
        if self.ai_shots_events != self.nav_ai_shots {
            println!(
                "        NOTE event-folded AI shots {} != NavStats::ai_shots {}",
                self.ai_shots_events, self.nav_ai_shots
            );
        }
        // The acceptance is a `cmp` of two runs' STDOUT, so the one column
        // that cannot be identical — wall time — goes to stderr, exactly as
        // the sweep puts its progress there.
        eprintln!("arena {} {:.1} s", self.tiers[0].label(), self.wall_s);
    }

    /// One `vs` row: seed, the two tiers, each side's kills, hits / shots and
    /// first kill, the margin and the hash.
    fn print_vs(&self, seed: u32) {
        let side = |t: usize| {
            let s = &self.side[t];
            let ttfk = s
                .ttfk
                .map_or_else(|| "-".to_owned(), |t| format!("{:.2}", t as f64 / HZ));
            format!(
                "{:<6} kills {:3} hits {:5}/{:<6} ttfk {:>6}",
                self.tiers[t].label(),
                s.kills,
                s.hits,
                s.shots,
                ttfk
            )
        };
        println!(
            "{seed:>5}  {}  |  {}  |  margin {:.1}/{:.1}  ticks {}  {:016x}",
            side(0),
            side(1),
            self.margin[0],
            self.margin[1],
            self.ticks,
            self.hash
        );
        if self.unperceived > 0 {
            println!(
                "        PERCEPTION ASSERTION FAILED: {} unperceived AI shots",
                self.unperceived
            );
        }
    }
}

/// One team's AI-on-AI fight (`vs`).
#[derive(Clone, Copy, Default)]
struct Side {
    kills: u64,
    shots: u64,
    hits: u64,
    ttfk: Option<u64>,
}

/// The per-tick event fold both fields share: AI-on-AI kills, AI shots and
/// AI hits, the first kill's tick, and the idle player's side of it kept
/// apart.
struct Fold {
    kills: u64,
    shots: u64,
    hits: u64,
    ttfk: Option<u64>,
    player_hits: u64,
    player_deaths: u64,
    side: [Side; 2],
    /// Card TOKN's human measure (stderr only): the health the bots took
    /// off the idle player, the ticks he was alive, and each hit on him by
    /// how many distinct bots hit him in the 3 s up to it (1, 2, 3, 4+).
    player_dmg: f64,
    player_alive: u64,
    shooters: [u64; 4],
    recent: Vec<(u64, core::unit::UnitHandle)>,
}

impl Fold {
    fn new() -> Self {
        Fold {
            kills: 0,
            shots: 0,
            hits: 0,
            ttfk: None,
            player_hits: 0,
            player_deaths: 0,
            side: [Side::default(); 2],
            player_dmg: 0.0,
            player_alive: 0,
            shooters: [0; 4],
            recent: Vec::new(),
        }
    }

    /// One tick's events. A handle's `is_player` is read off `sim.units`
    /// directly — the victim of a `Kill` is already dead, so `Sim::unit`
    /// would answer `None`.
    fn eat(&mut self, sim: &Sim) {
        let human = |h: core::unit::UnitHandle| {
            sim.units.get(h.index as usize).is_some_and(|u| u.is_player)
        };
        // A unit keeps its slot's team for the round (the respawn reuses it).
        let side = |h: core::unit::UnitHandle| {
            sim.units
                .get(h.index as usize)
                .map(|u| u.team)
                .filter(|&t| t < 2)
                .map(usize::from)
        };
        self.player_alive += u64::from(sim.alive_units().any(|(_, u)| u.is_player));
        let tick = sim.tick;
        self.recent
            .retain(|(t, _)| tick < t + 3 * core::TICK_HZ as u64);
        for e in &sim.events {
            match *e {
                core::sim::Event::Shot { unit, .. } if !human(unit) => {
                    self.shots += 1;
                    if let Some(t) = side(unit) {
                        self.side[t].shots += 1;
                    }
                }
                core::sim::Event::Hit {
                    shooter, victim, ..
                } if !human(shooter) => {
                    if human(victim) {
                        self.player_hits += 1;
                        if let core::sim::Event::Hit { damage, .. } = *e {
                            self.player_dmg += f64::from(damage);
                        }
                        self.recent.push((tick, shooter));
                        let mut who: Vec<_> = self.recent.iter().map(|r| r.1).collect();
                        who.sort_by_key(|h| (h.index, h.gen));
                        who.dedup();
                        self.shooters[who.len().clamp(1, 4) - 1] += 1;
                    } else {
                        self.hits += 1;
                        if let Some(t) = side(shooter) {
                            self.side[t].hits += 1;
                        }
                    }
                }
                core::sim::Event::Kill { shooter, victim } if !human(shooter) => {
                    if human(victim) {
                        self.player_deaths += 1;
                    } else {
                        self.kills += 1;
                        self.ttfk.get_or_insert(sim.tick);
                        if let Some(t) = side(shooter) {
                            self.side[t].kills += 1;
                            self.side[t].ttfk.get_or_insert(sim.tick);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Is the map arm's slot 0 a BOT this run? `SWEEP_SLOT0` (env) is the SAME
/// switch `round_to_outcome` reads, and this example defaults the OTHER way:
/// `bot`/`1` opts in, anything else — and UNSET — keeps the idle player.
/// The historical arena tables (analysis/PLAN_LOG_2026.md) were baselined with
/// the human in slot 0 and a tier row is only comparable to the rows beside
/// it, so the default cannot move without the lead re-baselining all eight.
/// The flat arena has no player at all and is unaffected either way.
fn slot0_is_bot() -> bool {
    matches!(std::env::var("SWEEP_SLOT0").as_deref(), Ok("bot" | "1"))
}

/// Card TOKN's human proxy, `ARENA_HUMAN=shadow` (map field, slot 0 the
/// idle player): before every tick the player is put 1.5 m to the right of
/// team 0's first living bot on foot, looking its way, at full health - so
/// he stands in the fights the bots walk into, never shoots back and never
/// dies, and the bots' hits on him (the stderr `human` line) measure the
/// pressure on a human. A measuring rig, not a model of a player.
fn shadow_human() -> bool {
    std::env::var("ARENA_HUMAN").as_deref() == Ok("shadow")
}

/// Card DPRX: `ARENA_PROXY=<team>:<strength>`.
fn arena_proxy() -> Option<(u8, core::ai::proxy::Strength)> {
    let v = std::env::var("ARENA_PROXY").ok()?;
    let (t, s) = v.split_once(':')?;
    Some((
        t.trim().parse().ok().filter(|t: &u8| *t < 2)?,
        core::ai::proxy::Strength::parse(s)?,
    ))
}

/// The proxy's slot and tally, after making the first bot of its team on
/// foot one.
fn make_proxy(sim: &mut Sim) -> Option<(usize, game::blackbox::session::Tally, String)> {
    let (team, strength) = arena_proxy()?;
    let h = sim
        .alive_units()
        .find(|(_, u)| u.team == team && u.controller == Controller::Ai && u.mount.is_none())
        .map(|(h, _)| h)?;
    sim.set_proxy(h, Some(strength.profile()));
    Some((
        h.index as usize,
        game::blackbox::session::Tally::default(),
        format!("{team} {}", strength.label()),
    ))
}

/// The stderr `proxy` line.
fn proxy_line(field: &str, seed: u32, p: &(usize, game::blackbox::session::Tally, String)) {
    let s = &p.1.out;
    eprintln!(
        "proxy {field} seed {seed}: team/strength {} kills {} deaths {} shots {} hits {} acc {:.3} life_s {:.1}",
        p.2,
        s.kills,
        s.deaths,
        s.shots,
        s.hits,
        f64::from(s.hits) / f64::from(s.shots.max(1)),
        s.alive_ticks as f64 / HZ / s.lives.len().max(1) as f64
    );
}

fn shadow_step(sim: &mut Sim) {
    let Some((pos, yaw)) = sim
        .alive_units()
        .find(|(_, u)| u.team == 0 && u.controller == Controller::Ai && u.mount.is_none())
        .map(|(_, u)| (u.soldier.pos, u.soldier.yaw))
    else {
        return;
    };
    let Some(i) = sim.units.iter().position(|u| u.alive && u.is_player) else {
        return;
    };
    let (sy, cy) = (yaw.sin(), yaw.cos());
    let u = &mut sim.units[i];
    if u.mount.is_some() {
        return;
    }
    // Right of a soldier facing (-sin yaw, -cos yaw).
    u.soldier.pos = pos + Vec3::new(cy, 0.0, -sy) * 1.5;
    u.soldier.vel = Vec3::ZERO;
    u.soldier.yaw = yaw;
    u.health.cur = u.health.max;
}

/// Card DFOCUS's second proxy, `ARENA_HUMAN=lone`: the idle player stands
/// apart from his team-mates, [`LONE_BACK`] behind team 0's front (its bot on
/// foot nearest an enemy), on a standable point of the position table at
/// least [`LONE_APART`] from any team-mate, in sight of that enemy where one
/// is, facing it, at full health. The spot is chosen again every second. In
/// the shadow proxy the fire aimed at the team-mate beside him hits him; here
/// what hits him was aimed at him.
fn lone_human() -> bool {
    std::env::var("ARENA_HUMAN").as_deref() == Ok("lone")
}

const LONE_BACK: f32 = 15.0;
const LONE_APART: f32 = 6.0;

fn lone_step(sim: &mut Sim, spot: &mut Option<(Vec3, f32)>) {
    let Some(i) = sim.units.iter().position(|u| u.alive && u.is_player) else {
        return;
    };
    if sim.units[i].mount.is_some() {
        return;
    }
    if spot.is_none() || sim.tick.is_multiple_of(core::TICK_HZ as u64) {
        // The front: team 0's bot on foot nearest a living enemy.
        let mut front: Option<(f32, Vec3, Vec3)> = None;
        for (_, a) in sim
            .alive_units()
            .filter(|(_, u)| u.team == 0 && !u.is_player && u.mount.is_none())
        {
            for (_, e) in sim.alive_units().filter(|(_, u)| u.team == 1) {
                let d = (e.soldier.pos - a.soldier.pos).length_squared();
                if front.is_none_or(|f| d < f.0) {
                    front = Some((d, a.soldier.pos, e.muzzle()));
                }
            }
        }
        if let Some((_, at, eye)) = front {
            let dir = Vec3::new(eye.x - at.x, 0.0, eye.z - at.z).normalize_or(Vec3::Z);
            let want = at - dir * LONE_BACK;
            let mut near = Vec::new();
            sim.positions.within(want, 10.0, &mut near);
            let mut pts: Vec<Vec3> = near
                .iter()
                .map(|&k| sim.positions.positions[k as usize].pos)
                .filter(|p| {
                    sim.alive_units().all(|(_, u)| {
                        u.team != 0 || u.is_player || (u.soldier.pos - *p).length() >= LONE_APART
                    })
                })
                .collect();
            pts.sort_by(|a, b| (*a - want).length().total_cmp(&(*b - want).length()));
            let seen = pts
                .iter()
                .take(8)
                .find(|p| core::perception::los(sim, eye, **p + Vec3::Y));
            if let Some(p) = seen.or(pts.first()) {
                let to = eye - *p;
                *spot = Some((*p, (-to.x).atan2(-to.z)));
            }
        }
    }
    let Some((p, yaw)) = *spot else {
        return;
    };
    let u = &mut sim.units[i];
    u.soldier.pos = p;
    u.soldier.vel = Vec3::ZERO;
    u.soldier.yaw = yaw;
    u.health.cur = u.health.max;
}

/// The flat field: `Sim::empty` on 400 m of level ground, no posts, no graph,
/// 8 v 8 troopers on two lines 30 m apart.
/// Team 0 at `tiers[0]`, team 1 at `tiers[1]`: equal tiers are the round's
/// one `difficulty` and no override, so a `vs x:x` run is the `x` run.
/// Unequal ones leave the round's tier (the readers that are not a unit's:
/// the human team's bleed handicap) at the LOWER of the two, so a pair's two
/// orientations differ only in which side is which.
fn set_tiers(sim: &mut Sim, tiers: [Difficulty; 2]) {
    sim.difficulty = tiers[0];
    if tiers[0] != tiers[1] {
        sim.difficulty = tiers[0].min(tiers[1]);
        sim.team_difficulty = tiers.map(Some);
    }
}

fn flat_sim(seed: u32, tiers: [Difficulty; 2]) -> Sim {
    let mut sim = Sim::empty(seed);
    set_tiers(&mut sim, tiers);
    // No manifest here, so the BF2 value stands (RE §9.1: BF1 maps are the
    // ones with no omni).
    sim.omni_radius = 10.0;
    sim.heightfield = Some(core::heightfield::Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    // Team 0 at z = +15 looking -Z (yaw 0), team 1 at z = -15 looking +Z.
    for i in 0..8u32 {
        let x = (i as f32 - 3.5) * 2.0;
        for (team, z, yaw) in [(0u8, 15.0, 0.0), (1u8, -15.0, std::f32::consts::PI)] {
            sim.spawn(&UnitSpec {
                controller: Controller::Ai,
                yaw,
                ..UnitSpec::trooper(team, Vec3::new(x, 0.0, z))
            })
            .expect("the arena's 8 v 8 fits in the default ticket pool");
        }
    }
    sim
}

fn run_flat(ticks: u64, seed: u32, tiers: [Difficulty; 2]) -> Row {
    let started = std::time::Instant::now();
    let mut sim = flat_sim(seed, tiers);
    let mut proxy = make_proxy(&mut sim);
    let mut f = Fold::new();
    let idle = [PadState::default()];
    let mut t = 0u64;
    while t < ticks {
        sim.step(&idle);
        f.eat(&sim);
        if let Some((i, tally, _)) = proxy.as_mut() {
            tally.tick(&sim, Some((*i, "proxy")));
        }
        t += 1;
        // One life each: the round is over when a side is wiped out.
        let (a, b) = alive_counts(&sim);
        if a == 0 || b == 0 {
            break;
        }
    }
    if let Some(p) = &proxy {
        proxy_line("flat", seed, p);
    }
    let (a, b) = alive_counts(&sim);
    row(tiers, &sim, f, t, started, [a as f64, b as f64])
}

fn alive_counts(sim: &Sim) -> (usize, usize) {
    let mut c = (0, 0);
    for (_, u) in sim.alive_units() {
        if u.team == 0 {
            c.0 += 1;
        } else {
            c.1 += 1;
        }
    }
    c
}

fn row(
    tiers: [Difficulty; 2],
    sim: &Sim,
    f: Fold,
    ticks: u64,
    started: std::time::Instant,
    margin: [f64; 2],
) -> Row {
    let (a, b) = alive_counts(sim);
    Row {
        tiers,
        side: f.side,
        margin,
        kills: f.kills,
        ai_shots_events: f.shots,
        ai_hits: f.hits,
        ttfk: f.ttfk,
        player_hits: f.player_hits,
        player_deaths: f.player_deaths,
        nav_ai_shots: sim.nav_stats.ai_shots,
        unperceived: sim.nav_stats.unperceived_shots,
        hash: sim.hash(),
        ticks: ticks.max(1),
        wall_s: started.elapsed().as_secs_f32(),
        alive: [a, b],
    }
}

/// `missions/<key>_con.ron`, when the generator wrote one for this map.
fn mission_for(key: &str) -> Option<game::mission::MissionDef> {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let path = dir.join(format!("{key}_con.ron"));
    path.exists()
        .then(|| game::mission::MissionDef::load(&path).ok())
        .flatten()
}

/// The map field retains the arena's historical referee setup: mode sublevel,
/// shared props, mission tickets/factions/class mix, nav graph, model collision,
/// then `setup_conquest` at 16 v 16 with the idle player. Unlike the sweep it
/// omits mission death drains, arc blocks and barrier toggles; unlike runtime
/// it also omits mission unit counts and alive-AI caps. Changing these inputs
/// needs a separate review of the protected arena baselines.
fn run_map(key: &str, path: &Path, ticks: u64, seed: u32, tiers: [Difficulty; 2]) -> Option<Row> {
    let started = std::time::Instant::now();
    let mut scene = match game::assets::load_scene(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{key}: LOAD FAILED {e:#}");
            return None;
        }
    };
    scene.apply_mode_layer("con");
    let common_dir = path
        .parent()
        .map(|p| p.join(format!("common_{}", scene.manifest.source_game)));
    if let Some(cd) = common_dir.filter(|d| d.join("common.ron").exists()) {
        match game::assets::load_common(&cd) {
            Ok(c) => {
                scene.merge_common(c);
            }
            Err(e) => eprintln!("{key}: common assets unreadable: {e:#}"),
        }
    }

    let mission = mission_for(key);
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = mission.as_ref().map(|m| layout.apply_mission(m));

    let mut sim = Sim::empty(seed);
    set_tiers(&mut sim, tiers);
    sim.omni_radius = core::sim::omni_radius_for(&scene.manifest.source_game);
    scene.install_water(&mut sim);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.item_pool = game::roster::item_pool(&scene.manifest.source_game, &scene.manifest.map);
    if let Some(m) = &mission {
        sim.battle = core::battle::Battle::new(m.tickets());
    }
    let factions = mission
        .as_ref()
        .map(game::mission::MissionDef::factions)
        .unwrap_or_else(|| {
            game::roster::default_factions(&scene.manifest.source_game).map(str::to_owned)
        });
    let classes_dir = path
        .parent()
        .map(|p| p.join("classes"))
        .unwrap_or_else(|| std::path::PathBuf::from("classes"));
    let rosters = game::roster::FactionRoster::pair(
        &classes_dir,
        &scene.manifest.source_game,
        [factions[0].as_str(), factions[1].as_str()],
        mission.as_ref(),
    );
    if let Ok((_, warnings)) = &rosters {
        for w in warnings {
            eprintln!("{w}");
        }
    }
    let mut tables: [Vec<core::unit::UnitClass>; 2] = [Vec::new(), Vec::new()];
    let teams = match &rosters {
        Ok((rs, _)) => {
            // One weapon index per WEAPON NAME, as the sweep does.
            let mut names: Vec<(String, usize)> = Vec::new();
            let mut t = 0usize;
            rs.each_ref().map(|r| {
                for c in &r.classes {
                    let uc = c.register(&mut sim, &mut names);
                    tables[t].push(uc);
                }
                t += 1;
                game::walk::TeamTemplate::from_roster(
                    r,
                    &tables[t - 1][0],
                    r.classes
                        .iter()
                        .map(game::walk::ClassVisual::from_roster)
                        .collect(),
                )
            })
        }
        Err(e) => {
            eprintln!("{key}: rosters unavailable: {e:#} (built-in troopers)");
            let t = game::walk::TeamTemplate::default_trooper();
            factions.map(|faction| game::walk::TeamTemplate {
                faction,
                ..t.clone()
            })
        }
    };
    let nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    let mut w = WalkSim::with_teams(sim, scene.heightfield(), Vec3::ZERO, teams, nav);
    w.sim.classes = tables;
    scene.install_collision(&mut w.sim);
    if let Some(a) = &applied {
        w.sim.death_regions = a.death_regions.clone();
        w.sim.fly_ceiling = a.fly_ceiling;
        w.sim.flight_limits = a.flight_limits;
        w.sim.ground_flyer_map = mission.as_ref().is_some_and(|m| m.ground_flyer_map);
    }
    // Card DTEAM: the idle player in slot 0 (team 0) is no human of this
    // tier comparison; one counted on team 1 cancels him for the team rule
    // (it reads only the difference), so the rows are the tier's, as before.
    w.sim.extra_humans = [0, 1];
    w.setup_conquest(&layout, [16, 16], true);
    // The mission's AI settings (card MAIS), as the game applies them.
    if let Some(m) = &mission {
        m.apply_ai(&mut w.sim);
    }
    // Slot 0 as a bot, opt-in (see `slot0_is_bot`): the same un-spawn-and-refill
    // `round_to_outcome` does by default, so the two referees can be run on the
    // same field. OFF, the idle player stands there and its hits and deaths are
    // the row's `player_hits`/`player_deaths` columns as they always were.
    if slot0_is_bot() {
        if let Some((pos, yaw)) = w.sim.vacate_player_slot(0) {
            if w.sim.classes[0].is_empty() {
                let spec = w.teams[0].spec(0, pos, yaw, Controller::Ai, 0);
                w.sim.spawn(&spec);
            } else {
                w.sim.spawn_ai(0, pos, yaw);
            }
        }
        w.sim.extra_humans = [0, 0];
    }

    let mut proxy = make_proxy(&mut w.sim);
    let mut f = Fold::new();
    let idle = [PadState::default()];
    let mut t = 0u64;
    let (shadow, lone) = (shadow_human(), lone_human());
    let mut spot = None;
    while t < ticks && w.sim.outcome.is_none() {
        if shadow {
            shadow_step(&mut w.sim);
        } else if lone {
            lone_step(&mut w.sim, &mut spot);
        }
        w.step_tick(&idle);
        f.eat(&w.sim);
        if let Some((i, tally, _)) = proxy.as_mut() {
            tally.tick(&w.sim, Some((*i, "proxy")));
        }
        t += 1;
    }
    if let Some(p) = &proxy {
        proxy_line(key, seed, p);
    }
    let tickets = w
        .sim
        .battle
        .teams
        .each_ref()
        .map(|t| f64::from(t.reinforcements));
    // Card TOKN: the idle player's side, on stderr (stdout is the pinned row).
    let alive_s = f.player_alive as f64 / HZ;
    eprintln!(
        "human {key} {} seed {seed}: hits {} dmg {:.0} alive_s {alive_s:.0} dmg_per_alive_min {:.1} deaths {} shooters_in_3s [1,2,3,4+] {:?} tokn_aimed_off {} tokn_stray {} tokn_cross {}",
        tiers[1].label(),
        f.player_hits,
        f.player_dmg,
        f.player_dmg / (alive_s / 60.0).max(1e-9),
        f.player_deaths,
        f.shooters,
        w.sim.nav_stats.tokn_aimed_off,
        w.sim.nav_stats.tokn_stray,
        w.sim.nav_stats.tokn_cross,
    );
    Some(row(tiers, &w.sim, f, t, started, tickets))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ticks_arg: Option<u64> = args.first().and_then(|s| s.parse().ok());
    let seed: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(42);
    let target = args.get(2).cloned().unwrap_or_else(|| "flat".to_owned());
    let tier_arg = args.get(3).cloned().unwrap_or_else(|| "all".to_owned());
    let vs = (tier_arg == "vs").then(|| {
        let spec = args.get(4).cloned().unwrap_or_else(|| "all".to_owned());
        let seeds: u32 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(1);
        (spec, seeds.max(1))
    });
    let tiers: Vec<Difficulty> = if tier_arg == "all" || vs.is_some() {
        Difficulty::ALL.to_vec()
    } else {
        match Difficulty::parse(&tier_arg) {
            Some(d) => vec![d],
            None => {
                eprintln!("arena: unknown tier {tier_arg:?} (easy|normal|hard|elite|all)");
                return;
            }
        }
    };
    let flat = target == "flat";
    let ticks = ticks_arg.unwrap_or(if flat { 3_000 } else { 9_000 });

    let map: Option<(String, std::path::PathBuf)> = if flat {
        None
    } else {
        let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets_baked"));
        match game::assets::discover_maps(dir)
            .into_iter()
            .find(|(key, _, _)| *key == target)
        {
            Some((key, _, path)) => Some((key, path)),
            None => {
                eprintln!("arena: no baked map {target:?} under {}", dir.display());
                return;
            }
        }
    };

    if let Some((spec, seeds)) = vs {
        head_to_head(&map, ticks, seed, &spec, seeds);
        return;
    }
    println!(
        "arena: {} ticks {ticks} seed {seed}",
        map.as_ref().map_or("flat 8v8", |(k, _)| k.as_str())
    );
    println!(
        "{:<7} {:>9} {:>9} {:>8} {:>8} {:>8} {:>6} {:>16}",
        "tier", "kills/min", "hit_rate", "ttfk_s", "sh/kill", "ai_shots", "unperc", "hash"
    );
    let mut rows: Vec<Row> = Vec::new();
    for tier in tiers {
        let r = match &map {
            None => run_flat(ticks, seed, [tier; 2]),
            Some((key, path)) => match run_map(key, path, ticks, seed, [tier; 2]) {
                Some(r) => r,
                None => return,
            },
        };
        r.print();
        rows.push(r);
    }
    if rows.len() == 4 {
        let mono = |v: Vec<f64>| v.windows(2).all(|w| w[0] < w[1]);
        let kills = mono(rows.iter().map(Row::kills_per_min).collect());
        let hits = mono(rows.iter().map(Row::hit_rate).collect());
        println!(
            "monotonic: kills {}  hits {}",
            if kills { "yes" } else { "no" },
            if hits { "yes" } else { "no" }
        );
    }
    let bad: u64 = rows.iter().map(|r| r.unperceived).sum();
    if bad > 0 {
        println!("PERCEPTION ASSERTION FAILED: {bad} unperceived AI shots");
    }
}

/// One run of the field: the flat one, or the map.
fn run_any(
    map: &Option<(String, std::path::PathBuf)>,
    ticks: u64,
    seed: u32,
    tiers: [Difficulty; 2],
) -> Option<Row> {
    match map {
        None => Some(run_flat(ticks, seed, tiers)),
        Some((key, path)) => run_map(key, path, ticks, seed, tiers),
    }
}

/// `vs <a>:<b>|all [seeds]` (card ARVS): the runs in parallel, the rows in
/// seed order, then a `beats:` line per pair under `all`.
fn head_to_head(
    map: &Option<(String, std::path::PathBuf)>,
    ticks: u64,
    seed: u32,
    spec: &str,
    seeds: u32,
) {
    let pairs: Vec<[Difficulty; 2]> = if spec == "all" {
        Difficulty::ALL.windows(2).map(|w| [w[0], w[1]]).collect()
    } else {
        let parsed = spec
            .split_once(':')
            .and_then(|(a, b)| Some([Difficulty::parse(a)?, Difficulty::parse(b)?]));
        match parsed {
            Some(p) => vec![p],
            None => {
                eprintln!("arena: vs wants <tier>:<tier> or all, not {spec:?}");
                return;
            }
        }
    };
    // `all` runs each pair both ways round on a seed; `a:b` just as given.
    let both = spec == "all";
    let mut jobs: Vec<(u32, [Difficulty; 2])> = Vec::new();
    for p in &pairs {
        for s in seed..seed + seeds {
            jobs.push((s, *p));
            if both {
                jobs.push((s, [p[1], p[0]]));
            }
        }
    }
    let n_jobs: usize = std::env::var("ARENA_JOBS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8usize)
        .max(1);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: Vec<std::sync::Mutex<Option<Row>>> =
        jobs.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..n_jobs.min(jobs.len()) {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(&(s, t)) = jobs.get(k) else {
                    break;
                };
                *out[k].lock().unwrap() = run_any(map, ticks, s, t);
            });
        }
    });
    let rows: Vec<Option<Row>> = out.into_iter().map(|m| m.into_inner().unwrap()).collect();
    if rows.iter().any(Option::is_none) {
        eprintln!("arena: a run failed to load");
        return;
    }
    let rows: Vec<Row> = rows.into_iter().flatten().collect();

    println!(
        "arena vs: {} ticks {ticks} seeds {seed}..{} {spec}",
        map.as_ref().map_or("flat 8v8", |(k, _)| k.as_str()),
        seed + seeds - 1,
    );
    let margin = if map.is_some() { "tickets" } else { "alive" };
    println!(" seed  team 0 | team 1 | margin = {margin} team 0/team 1");
    for ((s, _), r) in jobs.iter().zip(&rows) {
        r.print_vs(*s);
    }
    if !both {
        return;
    }
    for p in &pairs {
        let (lo, hi) = (p[0], p[1]);
        let mut shares: Vec<f64> = Vec::new();
        let mut margins: Vec<f64> = Vec::new();
        let (mut k_hi, mut k_lo) = (0u64, 0u64);
        for s in seed..seed + seeds {
            let run = |t: [Difficulty; 2]| {
                jobs.iter()
                    .zip(&rows)
                    .find(|((js, jt), _)| *js == s && *jt == t)
                    .map(|(_, r)| r)
                    .expect("every pair ran both ways")
            };
            // Run 1: lo is team 0; run 2: hi is team 0.
            let (r1, r2) = (run([lo, hi]), run([hi, lo]));
            let hi_k = r1.side[1].kills + r2.side[0].kills;
            let lo_k = r1.side[0].kills + r2.side[1].kills;
            k_hi += hi_k;
            k_lo += lo_k;
            if hi_k + lo_k > 0 {
                shares.push(hi_k as f64 / (hi_k + lo_k) as f64);
            }
            margins.push(((r1.margin[1] - r1.margin[0]) + (r2.margin[0] - r2.margin[1])) / 2.0);
        }
        let (mean, t) = paired_t(&shares, 0.5);
        let won = shares.iter().filter(|&&x| x > 0.5).count();
        let verdict = if t >= 2.0 {
            "yes"
        } else if t <= -2.0 {
            "REVERSED"
        } else {
            "coin flip"
        };
        println!(
            "beats: {}>{} {verdict}  share {:.3}  t {:+.2}  won {} of {}  kills {}/{}  margin {:+.1}",
            hi.label(),
            lo.label(),
            mean,
            t,
            won,
            shares.len(),
            k_hi,
            k_lo,
            margins.iter().sum::<f64>() / margins.len().max(1) as f64,
        );
    }
    let bad: u64 = rows.iter().map(|r| r.unperceived).sum();
    if bad > 0 {
        println!("PERCEPTION ASSERTION FAILED: {bad} unperceived AI shots");
    }
}

/// The mean of `xs` and its one-sample t against `mu` (0 when it cannot be
/// taken: under two values, or no spread).
fn paired_t(xs: &[f64], mu: f64) -> (f64, f64) {
    let n = xs.len() as f64;
    if xs.is_empty() {
        return (f64::NAN, 0.0);
    }
    let mean = xs.iter().sum::<f64>() / n;
    if xs.len() < 2 {
        return (mean, 0.0);
    }
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let se = (var / n).sqrt();
    (mean, if se > 0.0 { (mean - mu) / se } else { 0.0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D14's acceptance: two in-process runs of the same arena are the same
    /// run, and the seed is what makes them differ. `game/Cargo.toml` sets
    /// `test = true` on this example so `cargo test` runs this against the
    /// example's OWN rig rather than a copy of it.
    #[test]
    fn the_flat_arena_is_deterministic_and_seeded() {
        let a = run_flat(600, 42, [Difficulty::Normal; 2]);
        let b = run_flat(600, 42, [Difficulty::Normal; 2]);
        assert_eq!(a.hash, b.hash, "same seed, same final state");
        assert_eq!(
            (a.kills, a.ai_hits, a.ai_shots_events),
            (b.kills, b.ai_hits, b.ai_shots_events)
        );
        assert_eq!((a.ticks, a.ttfk, a.alive), (b.ticks, b.ttfk, b.alive));
        assert_eq!(
            a.unperceived, 0,
            "the perception assertion holds in the arena"
        );

        let c = run_flat(600, 43, [Difficulty::Normal; 2]);
        assert_ne!(a.hash, c.hash, "a different seed is a different fight");

        // The tier reaches the bots: Elite's fight is not Normal's, nor
        // Easy's.
        let low = run_flat(600, 42, [Difficulty::Easy; 2]);
        assert_ne!(a.hash, low.hash);
        assert_eq!(low.unperceived, 0);
        let e = run_flat(600, 42, [Difficulty::Elite; 2]);
        assert_ne!(a.hash, e.hash);
        assert_eq!(e.unperceived, 0);
    }

    /// Card ARVS: `vs x:x` is the `x` run (no override), and an uneven pair
    /// is a different fight whose kills fold per side.
    #[test]
    fn a_head_to_head_of_equal_tiers_is_the_one_tier_run() {
        let n = run_flat(600, 42, [Difficulty::Normal; 2]);
        let mut sim = flat_sim(42, [Difficulty::Normal; 2]);
        assert_eq!(sim.team_difficulty, [None; 2]);
        set_tiers(&mut sim, [Difficulty::Easy, Difficulty::Elite]);
        assert_eq!(sim.difficulty, Difficulty::Easy, "the lower of the two");
        let v = run_flat(600, 42, [Difficulty::Easy, Difficulty::Elite]);
        assert_ne!(n.hash, v.hash);
        assert_eq!(v.side[0].kills + v.side[1].kills, v.kills);
        assert_eq!(v.side[0].shots + v.side[1].shots, v.ai_shots_events);
        assert_eq!(v.unperceived, 0);
        let (mean, t) = paired_t(&[0.6, 0.7, 0.8], 0.5);
        assert!(
            (mean - 0.7).abs() < 1e-12 && (t - 3.4641).abs() < 1e-3,
            "{mean} {t}"
        );
    }
}
