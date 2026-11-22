//! SESSION STATS (dev only, fourth playtest batch): one JSON file per round
//! played, for tuning difficulty from real play. `scripts/session_stats.py`
//! summarises a folder of them per map / era / tier.
//!
//! - **Off by default.** The game crate's cargo feature `session-stats`
//!   ([`ENABLED`]) turns it on; `scripts/pack.py --session-stats` builds a
//!   game with it. Nobody else's build writes anything.
//! - **LAN-neutral.** The feature changes neither the LAN build id
//!   (`netplay::content::build_id`: the commit plus a hash of uncommitted
//!   source, `build.rs`, which never reads a cargo feature) nor the lockstep
//!   stream: everything here takes the sim by shared reference, after its
//!   tick, and sends nothing. A flagged build plays with an unflagged build
//!   of the same source (`build_id_ignores_cargo_features` below).
//! - **Where.** `session_stats/` beside the profile
//!   ([`crate::profile::data_dir`]; `OPENBF_SESSION_STATS_DIR` overrides it), `<UTC stamp>_<map>_<mode>_<role>_p<slot>_s<seed>.json`.
//!   Files older than [`KEEP_SECS`] (3 days) are deleted at launch
//!   ([`prune_at_launch`]) and before each write.
//! - **When.** Every round, single player and LAN (host or guest): written
//!   once at the round's outcome, else when the battle is left (`quit`).
//!   The window's tick hook feeds it every stepped tick; the headless
//!   `--headless --seconds N` round does too.
//! - **What.** The round's header (build id, date/time, map, era, mode,
//!   tier, team sizes, XL, awards and cheats, humans and their sides), its
//!   result (duration, winner, how it was decided, starting and final
//!   tickets or points per side), a timeline every [`SAMPLE_SECS`] (score
//!   per side, alive per side, the adaptive-difficulty factor) and one row
//!   per soldier slot, humans and every bot (side, classes, hero turns,
//!   kills, deaths, score, captures, team kills, vehicle kills, damage dealt
//!   and taken, shots, direct hits, accuracy).
//!
//! Field notes:
//! - A row is a unit SLOT (`Sim::units` index), as the stats pages count
//!   it: a bot respawns in its slot.
//! - `kills`, `score`, `captures`, `team_kills`, `vehicle_kills` are the
//!   stats pages' own numbers (`round_stats::UnitRound`, the event feed's
//!   rules). `deaths` is the sim's `Unit::deaths`.
//! - `damage_dealt` is damage to enemy soldiers (`Event::Hit`, self and
//!   allies excluded); `vehicle_damage_dealt` to enemy vehicles and turrets
//!   (`Event::VehicleHit`, `Event::TurretHit`); `damage_taken` is every
//!   `Event::Hit` and `Event::FallDamage` on the slot.
//! - `accuracy` is `hits / shots`, capped at 1: `shots` counts
//!   `Event::Shot` (one per trigger shot, a shotgun's pellets are one),
//!   `hits` counts direct (bolt) hits on enemy soldiers and vehicles, so a
//!   shotgun can land several per shot. `null` with no shot.
//! - `adaptive` in the timeline is the bots' adaptive-difficulty factor
//!   (1.0 = the tier's base skill), read through the
//!   [`SessionStats::adaptive`] probe, core's [`Sim::adaptive_factor`];
//!   `null` while the rule is off for the round (bots only, humans on both
//!   sides).

use crate::walk::WalkSim;
use core::sim::{Event, Sim, OUTCOME_DRAW};
use core::TICK_HZ;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The cargo feature `session-stats` is on.
pub const ENABLED: bool = cfg!(feature = "session-stats");
/// The file format; bump when a field changes meaning.
pub const SCHEMA: u32 = 1;
/// Seconds between two timeline samples.
pub const SAMPLE_SECS: u64 = 10;
/// Files older than this are deleted (3 days).
pub const KEEP_SECS: u64 = 3 * 24 * 3600;

/// Reads the adaptive-difficulty factor from the sim (1.0 = base skill).
/// The default is core's [`Sim::adaptive_factor`]; a test may set
/// [`SessionStats::adaptive`] to its own reader.
pub type AdaptiveProbe = fn(&Sim) -> Option<f32>;

/// The folder the files go to: `session_stats/` beside the profile, or
/// `OPENBF_SESSION_STATS_DIR` when set.
pub fn dir() -> PathBuf {
    std::env::var_os("OPENBF_SESSION_STATS_DIR").map_or_else(
        || crate::profile::data_dir().join("session_stats"),
        PathBuf::from,
    )
}

/// Delete `*.json` files in `dir` last modified more than [`KEEP_SECS`]
/// before `now`. Returns how many went.
pub fn prune(dir: &Path, now: SystemTime) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let keep = Duration::from_secs(KEEP_SECS);
    let mut gone = 0;
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .is_some_and(|age| age > keep);
        if old && std::fs::remove_file(&p).is_ok() {
            gone += 1;
        }
    }
    gone
}

/// [`prune`] of [`dir`] now, when the feature is on (the game's launch).
pub fn prune_at_launch() {
    if ENABLED {
        let d = dir();
        let n = prune(&d, SystemTime::now());
        println!(
            "session stats: on, rounds write {} ({n} old files deleted)",
            d.display()
        );
    }
}

/// What the caller knows about the round that the sim does not hold.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RoundInfo {
    pub map: String,
    /// The shell's mode key (`con`, `ctf`, `hunt`, `xl`, ...).
    pub mode: String,
    pub mission: String,
    pub source_game: String,
    pub seed: u32,
    pub xl: bool,
    /// Instant Action's Team Size percentage, when the mode has one.
    pub team_size_pct: Option<u32>,
    /// `single`, `host`, `guest` or `headless`.
    pub role: String,
    /// The local player's slot.
    pub local_slot: u8,
}

#[derive(Clone, Debug, Default)]
struct Tally {
    seen: bool,
    team: u8,
    was_alive: bool,
    classes: BTreeSet<String>,
    hero_turns: u32,
    shots: u64,
    hits: u64,
    damage_dealt: f64,
    vehicle_damage_dealt: f64,
    damage_taken: f64,
    human: bool,
    slot: Option<u8>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Sample {
    /// Seconds of sim clock.
    pub t: u64,
    /// Tickets or points per side (as `score_kind`).
    pub score: [i64; 2],
    pub alive: [u32; 2],
    pub adaptive: Option<f32>,
}

/// One round's collector. Read-only on the sim: see the module header.
pub struct SessionStats {
    info: RoundInfo,
    factions: [String; 2],
    started_unix: u64,
    start_score: [i64; 2],
    score_kind: &'static str,
    timeline: Vec<Sample>,
    next_sample: u64,
    units: Vec<Tally>,
    /// Every cheat id ON at any tick, session-wide or for a human's slot.
    cheats_ever: BTreeSet<&'static str>,
    decided: Option<(u64, &'static str)>,
    dir: PathBuf,
    written: bool,
    /// The adaptive-difficulty reader ([`AdaptiveProbe`]).
    pub adaptive: AdaptiveProbe,
}

/// The score the round is decided on, per side: `points` (Hunt, Hero
/// Assault, XL by points, space assault), `flags` (CTF), else `tickets`.
pub fn side_score(sim: &Sim) -> (&'static str, [i64; 2]) {
    if let Some(sa) = &sim.space_assault {
        ("points", sa.points.map(i64::from))
    } else if let Some(ctf) = &sim.ctf {
        ("flags", ctf.team_points.map(i64::from))
    } else if let Some((p, _)) = sim.team_points() {
        ("points", p.map(i64::from))
    } else {
        (
            "tickets",
            [0, 1].map(|t| i64::from(sim.battle.teams[t].tickets())),
        )
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl SessionStats {
    /// A collector for `walk`'s round, writing into `dir`.
    pub fn start(info: RoundInfo, walk: &WalkSim, dir: PathBuf) -> SessionStats {
        let (score_kind, start_score) = side_score(&walk.sim);
        let mut s = SessionStats {
            info,
            factions: [0u8, 1].map(|t| walk.team_template(t).faction.clone()),
            started_unix: unix_now(),
            start_score,
            score_kind,
            timeline: Vec::with_capacity(256),
            next_sample: walk.sim.tick,
            units: Vec::new(),
            cheats_ever: BTreeSet::new(),
            decided: None,
            dir,
            written: false,
            adaptive: Sim::adaptive_factor,
        };
        s.observe_units(walk);
        s
    }

    pub fn written(&self) -> bool {
        self.written
    }

    pub fn info(&self) -> &RoundInfo {
        &self.info
    }

    fn tally(&mut self, i: usize) -> &mut Tally {
        if self.units.len() <= i {
            self.units.resize(i + 1, Tally::default());
        }
        &mut self.units[i]
    }

    fn observe_units(&mut self, walk: &WalkSim) {
        let sim = &walk.sim;
        for (i, u) in sim.units.iter().enumerate() {
            let alive = u.alive;
            let spawned = alive && !self.units.get(i).is_some_and(|t| t.was_alive);
            let hero = spawned && sim.hero_unit(u);
            let class =
                spawned.then(|| walk.team_template(u.team).class(u.spec.class).name.clone());
            let human = u.is_player && u.proxy_human.is_none();
            let t = self.tally(i);
            t.was_alive = alive;
            if alive || !t.seen {
                t.team = u.team;
            }
            t.seen = true;
            t.human |= human;
            if human {
                t.slot = u.spec.player_slot;
            }
            if let Some(c) = class {
                t.classes.insert(c);
            }
            if hero {
                t.hero_turns += 1;
            }
        }
    }

    fn enemy_unit(sim: &Sim, team: u8, i: usize) -> bool {
        sim.units.get(i).is_some_and(|v| !sim.allied(team, v.team))
    }

    fn observe_events(&mut self, sim: &Sim) {
        for e in &sim.events {
            match *e {
                Event::Shot { unit, .. } => self.tally(unit.index as usize).shots += 1,
                Event::Hit {
                    direct,
                    shooter,
                    victim,
                    damage,
                    ..
                } => {
                    let (s, v) = (shooter.index as usize, victim.index as usize);
                    self.tally(v).damage_taken += f64::from(damage);
                    let team = sim.units.get(s).map(|u| u.team);
                    if s != v && team.is_some_and(|t| Self::enemy_unit(sim, t, v)) {
                        let t = self.tally(s);
                        t.damage_dealt += f64::from(damage);
                        t.hits += u64::from(direct);
                    }
                }
                Event::FallDamage { victim, damage, .. } => {
                    self.tally(victim.index as usize).damage_taken += f64::from(damage);
                }
                Event::VehicleHit {
                    shooter: Some(shooter),
                    vehicle,
                    damage,
                    dir,
                } => {
                    let s = shooter.index as usize;
                    let team = sim.units.get(s).map(|u| u.team);
                    let vteam = sim
                        .vehicles
                        .get(vehicle.index as usize)
                        .and_then(|v| v.team);
                    let enemy = match (team, vteam) {
                        (Some(a), Some(b)) => !sim.allied(a, b),
                        (Some(_), None) => true,
                        _ => false,
                    };
                    if enemy {
                        let t = self.tally(s);
                        t.vehicle_damage_dealt += f64::from(damage);
                        // A bolt or blast; a direct one has a direction.
                        t.hits += u64::from(dir.is_some() && damage > 0.0);
                    }
                }
                Event::TurretHit {
                    shooter,
                    team,
                    damage,
                    ..
                } => {
                    let s = shooter.index as usize;
                    if sim.units.get(s).is_some_and(|u| !sim.allied(u.team, team)) {
                        self.tally(s).vehicle_damage_dealt += f64::from(damage);
                    }
                }
                _ => {}
            }
        }
    }

    fn observe_cheats(&mut self, sim: &Sim) {
        for (id, _, on) in sim.cheats.entries() {
            if on {
                self.cheats_ever.insert(id);
            }
        }
        for slot in self.units.iter().filter(|t| t.human).filter_map(|t| t.slot) {
            for (id, _, on) in sim.cheats_for_slot(usize::from(slot)).entries() {
                if on {
                    self.cheats_ever.insert(id);
                }
            }
        }
    }

    fn sample(&self, sim: &Sim) -> Sample {
        Sample {
            t: sim.tick / u64::from(TICK_HZ),
            score: side_score(sim).1,
            alive: [0, 1].map(|t| sim.battle.teams[t].alive()),
            adaptive: (self.adaptive)(sim),
        }
    }

    /// One stepped tick: after `Sim::step`, while its events are there.
    pub fn tick(&mut self, walk: &WalkSim) {
        let sim = &walk.sim;
        self.observe_units(walk);
        self.observe_events(sim);
        self.observe_cheats(sim);
        if self.decided.is_none() && sim.outcome.is_some() {
            self.decided = Some((sim.tick, how_decided(sim)));
        }
        if self.decided.is_none() && sim.tick >= self.next_sample {
            let s = self.sample(sim);
            self.timeline.push(s);
            let every = SAMPLE_SECS * u64::from(TICK_HZ);
            self.next_sample = (sim.tick / every + 1) * every;
        }
    }

    /// The file's JSON: `reason` is `outcome` or `quit`.
    pub fn json(
        &self,
        walk: &WalkSim,
        round: &crate::round_stats::RoundStats,
        names: Option<&crate::event_feed::EventFeed>,
        reason: &str,
    ) -> serde_json::Value {
        let sim = &walk.sim;
        let end_tick = self.decided.map_or(sim.tick, |(t, _)| t);
        let mut timeline = self.timeline.clone();
        let last = self.sample(sim);
        if timeline.last().is_none_or(|s| s.t != last.t) {
            timeline.push(last);
        }
        let (_, final_score) = side_score(sim);
        let side = |team: u8| -> serde_json::Value {
            match team {
                0 | 1 => serde_json::json!(self.factions[usize::from(team)]),
                t => serde_json::json!(format!("team{t}")),
            }
        };
        let mut sizes = [0u32; 2];
        let mut human_sides = BTreeSet::new();
        let mut players = Vec::new();
        let mut humans = Vec::new();
        for (i, t) in self.units.iter().enumerate().filter(|(_, t)| t.seen) {
            if t.team < 2 {
                sizes[usize::from(t.team)] += 1;
            }
            let u = sim.units.get(i);
            let r = round.units.get(i).cloned().unwrap_or_default();
            let name = names.map(|f| f.unit_name(sim, sim.handle(i)));
            if t.human {
                human_sides.insert(t.team);
                humans.push(serde_json::json!({
                    "unit": i,
                    "slot": t.slot,
                    "name": name,
                    "team": t.team,
                    "side": side(t.team),
                    "award_bits": u.map(|u| sim.award_bits(u)),
                }));
            }
            let accuracy = (t.shots > 0).then(|| t.hits.min(t.shots) as f64 / t.shots as f64);
            players.push(serde_json::json!({
                "unit": i,
                "human": t.human,
                "slot": t.slot,
                "name": name,
                "team": t.team,
                "side": side(t.team),
                "classes": t.classes,
                "hero_turns": t.hero_turns,
                "kills": r.kills,
                "deaths": u.map_or(0, |u| u.deaths),
                "score": r.points,
                "hero_score": r.hero_points,
                "captures": r.posts,
                "team_kills": r.team_kills,
                "vehicle_kills": r.vehicles,
                "damage_dealt": round1(t.damage_dealt),
                "vehicle_damage_dealt": round1(t.vehicle_damage_dealt),
                "damage_taken": round1(t.damage_taken),
                "shots": t.shots,
                "hits": t.hits,
                "accuracy": accuracy.map(|a| (a * 1000.0).round() / 1000.0),
            }));
        }
        let winner = sim.outcome.map(|w| match w {
            OUTCOME_DRAW => serde_json::json!("draw"),
            t => side(t),
        });
        let humans_team = (human_sides.len() == 1).then(|| *human_sides.first().unwrap());
        let humans_won = match (sim.outcome, humans_team) {
            (Some(OUTCOME_DRAW), _) | (None, _) | (_, None) => None,
            (Some(w), Some(h)) => Some(w == h),
        };
        let now = unix_now();
        serde_json::json!({
            "schema": SCHEMA,
            "reason": reason,
            "build_id": crate::netplay::content::build_id(),
            "target": crate::netplay::content::target(),
            "started_unix": self.started_unix,
            "started_utc": crate::matchlog::stamp(self.started_unix),
            "written_utc": crate::matchlog::stamp(now),
            "map": self.info.map,
            "mission": self.info.mission,
            "source_game": self.info.source_game,
            "era": era(&self.factions),
            "mode": self.info.mode,
            "tier": sim.difficulty.label(),
            "seed": self.info.seed,
            "role": self.info.role,
            "local_slot": self.info.local_slot,
            "xl": self.info.xl,
            "team_size_pct": self.info.team_size_pct,
            "team_combatants": sim.team_combatants,
            "team_sizes": sizes,
            "factions": self.factions,
            "awards_all": sim.cheats.all_awards,
            "cheats": self.cheats_ever,
            "humans": humans,
            "humans_team": humans_team,
            "duration_s": end_tick as f64 / f64::from(TICK_HZ),
            "tick_hz": TICK_HZ,
            "winner": winner,
            "winner_team": sim.outcome,
            "humans_won": humans_won,
            "decided_by": self.decided.map_or("unfinished", |(_, how)| how),
            "score_kind": self.score_kind,
            "start_score": self.start_score,
            "final_score": final_score,
            "time_limit_s": sim.time_limit,
            "timeline": timeline,
            "players": players,
        })
    }

    /// Write the file once (prunes the folder first). `None` when already
    /// written or the write failed (said on stderr).
    pub fn write(
        &mut self,
        walk: &WalkSim,
        round: &crate::round_stats::RoundStats,
        names: Option<&crate::event_feed::EventFeed>,
        reason: &str,
    ) -> Option<PathBuf> {
        if self.written {
            return None;
        }
        self.written = true;
        let value = self.json(walk, round, names, reason);
        if let Err(e) = std::fs::create_dir_all(&self.dir) {
            eprintln!("session stats: {}: {e}", self.dir.display());
            return None;
        }
        prune(&self.dir, SystemTime::now());
        let name = format!(
            "{}_{}_{}_{}_p{}_s{}.json",
            crate::matchlog::stamp(self.started_unix),
            safe(&self.info.map),
            safe(&self.info.mode),
            safe(&self.info.role),
            self.info.local_slot,
            self.info.seed
        );
        let path = self.dir.join(name);
        let text = serde_json::to_vec_pretty(&value).unwrap_or_default();
        match std::fs::write(&path, text) {
            Ok(()) => Some(path),
            Err(e) => {
                eprintln!("session stats: {}: {e}", path.display());
                None
            }
        }
    }
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// `cw` for a Clone Wars pair, else `gcw`.
pub fn era(factions: &[String; 2]) -> &'static str {
    if factions.iter().any(|f| f == "rep" || f == "cis") {
        "cw"
    } else {
        "gcw"
    }
}

/// How the round just decided was decided (read on the tick its outcome
/// appeared): the time limit, flags, points, tickets or domination.
pub fn how_decided(sim: &Sim) -> &'static str {
    if sim.events.iter().any(|e| matches!(e, Event::TimeUp { .. })) {
        return "time_limit";
    }
    if sim.space_assault.is_some() {
        return "points";
    }
    if sim.ctf.is_some() {
        return "flags";
    }
    if sim.team_points().is_some() {
        return "points";
    }
    match sim.outcome {
        Some(w) if w < 2 => {
            let loser = 1 - usize::from(w);
            if sim.battle.teams[loser].tickets() == 0 {
                "tickets"
            } else if sim.domination.is_some() {
                "domination"
            } else {
                "other"
            }
        }
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_deletes_only_old_json() {
        let dir = std::env::temp_dir().join(format!("openbf_session_stats_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["a.json", "b.json", "c.txt"] {
            std::fs::write(dir.join(n), b"{}").unwrap();
        }
        // Now: nothing is older than 3 days.
        assert_eq!(prune(&dir, SystemTime::now()), 0);
        // Four days on: both JSON files go, the text file stays.
        let later = SystemTime::now() + Duration::from_secs(4 * 24 * 3600);
        assert_eq!(prune(&dir, later), 2);
        assert!(dir.join("c.txt").exists());
        assert!(!dir.join("a.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The LAN build id (`build.rs`) is the commit and a hash of the
    /// uncommitted source: no cargo feature can change it, so a
    /// `session-stats` build plays LAN with a build without it.
    #[test]
    fn build_id_ignores_cargo_features() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        for f in ["build.rs", "build_rules.rs"] {
            let src = std::fs::read_to_string(Path::new(manifest).join(f)).unwrap();
            assert!(
                !src.contains("CARGO_FEATURE") && !src.contains("CARGO_CFG_FEATURE"),
                "{f} must not read cargo features: the LAN build id would split flagged builds"
            );
        }
        let id = crate::netplay::content::build_id();
        assert!(!id.contains("session"), "{id}");
    }

    /// Two identical rounds, one with the collector: the same sim hash every
    /// tick (it observes, never steps or writes the sim), and the file holds
    /// the round's header, a timeline and a row per slot.
    #[test]
    fn collector_leaves_the_sim_alone_and_writes_a_round() {
        let mut plain = WalkSim::new(7, glam::Vec3::ZERO, None);
        let mut watched = WalkSim::new(7, glam::Vec3::ZERO, None);
        let dir =
            std::env::temp_dir().join(format!("openbf_session_rounds_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let info = RoundInfo {
            map: "test".into(),
            mode: "con".into(),
            role: "single".into(),
            seed: 7,
            ..Default::default()
        };
        let mut stats = SessionStats::start(info, &watched, dir.clone());
        stats.adaptive = |_| Some(1.0);
        let idle = [core::sim::PadState::default()];
        let ticks = 25 * u64::from(TICK_HZ);
        for _ in 0..ticks {
            plain.step_tick(&idle);
            watched.step_tick(&idle);
            stats.tick(&watched);
            assert_eq!(
                plain.sim.hash(),
                watched.sim.hash(),
                "tick {}",
                plain.sim.tick
            );
        }
        let round = crate::round_stats::RoundStats::default();
        let path = stats
            .write(&watched, &round, None, "quit")
            .expect("written");
        assert!(
            stats.write(&watched, &round, None, "quit").is_none(),
            "once"
        );
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["schema"], SCHEMA);
        assert_eq!(v["map"], "test");
        assert_eq!(v["reason"], "quit");
        assert_eq!(v["decided_by"], "unfinished");
        assert_eq!(v["build_id"], crate::netplay::content::build_id());
        let timeline = v["timeline"].as_array().unwrap();
        // t = 0, 10, 20 and the closing sample at 25.
        let t: Vec<u64> = timeline.iter().map(|s| s["t"].as_u64().unwrap()).collect();
        assert_eq!(t, [0, 10, 20, 25]);
        assert_eq!(timeline[0]["adaptive"], 1.0);
        let players = v["players"].as_array().unwrap();
        assert_eq!(players.len(), watched.sim.units.len());
        assert!(players.iter().any(|p| p["human"] == true));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn era_and_safe_names() {
        assert_eq!(era(&["rep".into(), "cis".into()]), "cw");
        assert_eq!(era(&["all".into(), "imp".into()]), "gcw");
        assert_eq!(safe("tat2/con x"), "tat2_con_x");
    }
}
