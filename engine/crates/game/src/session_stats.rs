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
//!
//! Lives and heroes (schema 2, additive):
//! - `lives`: one row per unit LIFE (a spawn: the unit's generation), humans
//!   and bots: the class played (`hero` when it is a hero class),
//!   `spawn_tick`, `end_tick`, how it ended and the `kills`, `team_kills`,
//!   damage, `shots` and `hits` of that life alone (the slot row's rules,
//!   credited by the shooter's handle, so a grenade landing after its
//!   thrower died counts for the life that threw it). A life's `kills` are
//!   enemy kills only; the slot row's `kills` is the stats page's net count
//!   (a team kill or suicide takes one off).
//! - `end`: `killed` (an enemy's `Event::Kill`), `team_killed`, `suicide`,
//!   `drained` (a hero's time drain: `Event::EnvDeath` with
//!   `HeroExhausted`, or a kill credited to the hero's last attacker on a
//!   tick nothing hit him, his health within two ticks' drain of 0),
//!   `forced` (a fall after a push, credited to its
//!   attacker), `env` (the world; `cause` says how), `despawned` (taken off
//!   with no death: accepting a hero), `removed` (gone without an event)
//!   or `round_end` (alive when the file was written). `killed_by` is the
//!   killer's unit, side and class in the life that killed.
//! - A life's or class total's `deaths` counts the ends that are deaths
//!   (all but `despawned`, `removed` and `round_end`).
//! - Each slot row gains `hero_kills`, `hero_deaths`, `hero_damage_dealt`
//!   and `by_class` (its lives summed per class); `class_totals` sums every
//!   life per team, human or bot, and class (the bots' per-team split).
//! - `heroes`: the hero class per side, each of the hero manager's
//!   `unlocks`, its `offers` to humans, and `lives`: every hero life with
//!   its `unlocked_tick`, `offered_tick` and `accepted_tick` (the manager
//!   making him the owner) when known. Hero Assault has no unlock: every
//!   class is a hero there.

use crate::walk::WalkSim;
use core::sim::{DeathCause, Event, Sim, OUTCOME_DRAW};
use core::unit::UnitHandle;
use core::TICK_HZ;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The cargo feature `session-stats` is on.
pub const ENABLED: bool = cfg!(feature = "session-stats");
/// The file format; bump when a field changes meaning.
pub const SCHEMA: u32 = 2;
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
    /// The index in [`SessionStats::lives`] of the slot's latest life.
    life: Option<usize>,
}

/// How a life ended.
#[derive(Clone, Debug)]
struct LifeEnd {
    tick: u64,
    how: &'static str,
    cause: Option<DeathCause>,
    /// The killer's handle and life ([`SessionStats::lives`] index).
    by: Option<(UnitHandle, Option<usize>)>,
}

impl LifeEnd {
    fn death(&self) -> bool {
        !matches!(self.how, "despawned" | "removed" | "round_end")
    }
}

/// One unit life: a spawn of a slot, until its death or removal.
#[derive(Clone, Debug)]
struct Life {
    unit: u32,
    gen: u32,
    human: bool,
    slot: Option<u8>,
    team: u8,
    class: String,
    hero: bool,
    spawn_tick: u64,
    end: Option<LifeEnd>,
    kills: u32,
    team_kills: u32,
    shots: u64,
    hits: u64,
    damage_dealt: f64,
    vehicle_damage_dealt: f64,
    damage_taken: f64,
    unlocked_tick: Option<u64>,
    offered_tick: Option<u64>,
    accepted_tick: Option<u64>,
    /// Health on the last tick seen alive (tells a drain's death).
    last_health: f32,
}

/// Lives summed per class.
#[derive(Clone, Debug, Default)]
struct ClassSum {
    hero: bool,
    lives: u32,
    kills: u32,
    team_kills: u32,
    deaths: u32,
    damage_dealt: f64,
    vehicle_damage_dealt: f64,
    damage_taken: f64,
    shots: u64,
    hits: u64,
    alive_s: f64,
}

impl ClassSum {
    fn add(&mut self, l: &Life, end_tick: u64) {
        self.hero |= l.hero;
        self.lives += 1;
        self.kills += l.kills;
        self.team_kills += l.team_kills;
        self.deaths += u32::from(l.end.as_ref().is_some_and(LifeEnd::death));
        self.damage_dealt += l.damage_dealt;
        self.vehicle_damage_dealt += l.vehicle_damage_dealt;
        self.damage_taken += l.damage_taken;
        self.shots += l.shots;
        self.hits += l.hits;
        let end = l.end.as_ref().map_or(end_tick, |e| e.tick);
        self.alive_s += end.saturating_sub(l.spawn_tick) as f64 / f64::from(TICK_HZ);
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "hero": self.hero,
            "lives": self.lives,
            "kills": self.kills,
            "team_kills": self.team_kills,
            "deaths": self.deaths,
            "damage_dealt": round1(self.damage_dealt),
            "vehicle_damage_dealt": round1(self.vehicle_damage_dealt),
            "damage_taken": round1(self.damage_taken),
            "shots": self.shots,
            "hits": self.hits,
            "alive_s": round1(self.alive_s),
        })
    }
}

/// The hero manager's moments, as seen tick by tick.
#[derive(Clone, Debug, Default)]
struct HeroLog {
    was_unlocked: [bool; 2],
    owner: [Option<u32>; 2],
    /// (team, tick) of each unlock.
    unlocks: Vec<(u8, u64)>,
    /// (tick, unit, team) of each offer.
    offers: Vec<(u64, UnitHandle, u8)>,
    /// (team, member, tick) of each new owner.
    accepts: Vec<(u8, u32, u64)>,
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
    /// Every unit life, in spawn order.
    lives: Vec<Life>,
    /// (unit index, generation) -> [`Self::lives`] index.
    life_of: HashMap<(u32, u32), usize>,
    hero_log: HeroLog,
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
            lives: Vec::new(),
            life_of: HashMap::new(),
            hero_log: HeroLog::default(),
            cheats_ever: BTreeSet::new(),
            decided: None,
            dir,
            written: false,
            adaptive: Sim::adaptive_factor,
        };
        s.observe_heroes(&walk.sim);
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
            let open = self.units.get(i).and_then(|t| t.life);
            if alive && open.is_none_or(|l| self.lives[l].gen != u.gen) {
                if let Some(l) = open {
                    self.end_life(l, sim.tick, "removed", None, None);
                }
                let l = self.new_life(walk, i);
                self.tally(i).life = Some(l);
            }
            if let Some(l) = self.units.get(i).and_then(|t| t.life).filter(|_| alive) {
                self.lives[l].last_health = u.health.cur;
            }
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

    /// Open a life for unit `i`'s current spawn.
    fn new_life(&mut self, walk: &WalkSim, i: usize) -> usize {
        let sim = &walk.sim;
        let u = &sim.units[i];
        let team = u.team;
        let hero = sim.hero_unit(u);
        let log = &self.hero_log;
        let unlocked_tick = log
            .unlocks
            .iter()
            .rev()
            .find(|(t, _)| *t == team)
            .map(|(_, k)| *k)
            .filter(|_| hero);
        let offered_tick = log
            .offers
            .iter()
            .rev()
            .find(|(k, h, _)| h.index as usize == i && unlocked_tick.is_none_or(|un| *k >= un))
            .map(|(k, _, _)| *k)
            .filter(|_| hero);
        let accepted_tick = log
            .accepts
            .iter()
            .rev()
            .find(|(t, m, _)| *t == team && *m as usize == i)
            .map(|(_, _, k)| *k)
            .filter(|_| hero);
        let l = self.lives.len();
        self.lives.push(Life {
            unit: i as u32,
            gen: u.gen,
            human: u.is_player && u.proxy_human.is_none(),
            slot: u.spec.player_slot,
            team,
            class: walk.team_template(team).class(u.spec.class).name.clone(),
            hero,
            spawn_tick: sim.tick,
            end: None,
            kills: 0,
            team_kills: 0,
            shots: 0,
            hits: 0,
            damage_dealt: 0.0,
            vehicle_damage_dealt: 0.0,
            damage_taken: 0.0,
            unlocked_tick,
            offered_tick,
            accepted_tick,
            last_health: u.health.cur,
        });
        self.life_of.insert((i as u32, u.gen), l);
        l
    }

    /// End life `l`, unless an event already ended it (a tentative
    /// `removed` gives way to the event that says why).
    fn end_life(
        &mut self,
        l: usize,
        tick: u64,
        how: &'static str,
        cause: Option<DeathCause>,
        by: Option<(UnitHandle, Option<usize>)>,
    ) {
        let life = &mut self.lives[l];
        if life.end.as_ref().is_none_or(|e| e.how == "removed") {
            life.end = Some(LifeEnd {
                tick,
                how,
                cause,
                by,
            });
        }
    }

    fn life(&mut self, h: UnitHandle) -> Option<&mut Life> {
        let l = *self.life_of.get(&(h.index, h.gen))?;
        Some(&mut self.lives[l])
    }

    /// The hero manager's unlocks, offers and new owners this tick.
    fn observe_heroes(&mut self, sim: &Sim) {
        let log = &mut self.hero_log;
        if let Some(h) = &sim.heroes {
            for (team, st) in h.teams.iter().enumerate() {
                if st.unlocked && !log.was_unlocked[team] {
                    log.unlocks.push((team as u8, sim.tick));
                }
                log.was_unlocked[team] = st.unlocked;
                if let Some(m) = st.owner.filter(|m| log.owner[team] != Some(*m)) {
                    log.accepts.push((team as u8, m, sim.tick));
                }
                log.owner[team] = st.owner;
            }
        }
        for e in &sim.events {
            if let Event::HeroOffered { unit, .. } = *e {
                let team = sim.units.get(unit.index as usize).map_or(0, |u| u.team);
                log.offers.push((sim.tick, unit, team));
            }
        }
    }

    /// The lives that ended this tick: the kill and death events, then any
    /// open life whose soldier is gone without one.
    fn observe_ends(&mut self, sim: &Sim) {
        // A tick's drain: a hero this close to 0 with nothing hitting him
        // died of it, though the kill goes to his last attacker.
        let drain_step = sim
            .heroes
            .as_ref()
            .map_or(0.0, |h| h.rules.health.time_drain_per_second)
            / TICK_HZ as f32;
        let hit_now: BTreeSet<(u32, u32)> = sim
            .events
            .iter()
            .filter_map(|e| match *e {
                Event::Hit { victim, .. } => Some((victim.index, victim.gen)),
                _ => None,
            })
            .collect();
        for e in &sim.events {
            match *e {
                Event::Kill { shooter, victim } => {
                    let Some(&v) = self.life_of.get(&(victim.index, victim.gen)) else {
                        continue;
                    };
                    let k = self.life_of.get(&(shooter.index, shooter.gen)).copied();
                    let vteam = self.lives[v].team;
                    let kteam = k
                        .map(|k| self.lives[k].team)
                        .or_else(|| sim.units.get(shooter.index as usize).map(|u| u.team));
                    let how = if shooter == victim {
                        "suicide"
                    } else if kteam.is_some_and(|t| sim.allied(t, vteam)) {
                        "team_killed"
                    } else if self.lives[v].hero
                        && drain_step > 0.0
                        && self.lives[v].last_health <= 2.0 * drain_step
                        && !hit_now.contains(&(victim.index, victim.gen))
                    {
                        "drained"
                    } else {
                        "killed"
                    };
                    if let Some(k) = k.filter(|_| shooter != victim) {
                        if how == "team_killed" {
                            self.lives[k].team_kills += 1;
                        } else {
                            self.lives[k].kills += 1;
                        }
                    }
                    self.end_life(v, sim.tick, how, None, Some((shooter, k)));
                }
                Event::ForcedDeath { unit, cause } => {
                    if let Some(&v) = self.life_of.get(&(unit.index, unit.gen)) {
                        if let Some(end) = self.lives[v].end.as_mut() {
                            if matches!(end.how, "killed" | "drained") {
                                end.how = "forced";
                            }
                            end.cause = Some(cause);
                        }
                    }
                }
                Event::EnvDeath { unit, cause } => {
                    if let Some(&v) = self.life_of.get(&(unit.index, unit.gen)) {
                        let how = if cause == DeathCause::HeroExhausted {
                            "drained"
                        } else {
                            "env"
                        };
                        self.end_life(v, sim.tick, how, Some(cause), None);
                    }
                }
                Event::Despawn { unit } => {
                    if let Some(&v) = self.life_of.get(&(unit.index, unit.gen)) {
                        self.end_life(v, sim.tick, "despawned", None, None);
                    }
                }
                _ => {}
            }
        }
        for t in &self.units {
            let Some(l) = t.life else { continue };
            let life = &mut self.lives[l];
            let gone = sim
                .units
                .get(life.unit as usize)
                .is_none_or(|u| !u.alive || u.gen != life.gen);
            if life.end.is_none() && gone {
                life.end = Some(LifeEnd {
                    tick: sim.tick,
                    how: "removed",
                    cause: None,
                    by: None,
                });
            }
        }
    }

    fn enemy_unit(sim: &Sim, team: u8, i: usize) -> bool {
        sim.units.get(i).is_some_and(|v| !sim.allied(team, v.team))
    }

    fn observe_events(&mut self, sim: &Sim) {
        for e in &sim.events {
            match *e {
                Event::Shot { unit, .. } => {
                    self.tally(unit.index as usize).shots += 1;
                    if let Some(l) = self.life(unit) {
                        l.shots += 1;
                    }
                }
                Event::Hit {
                    direct,
                    shooter,
                    victim,
                    damage,
                    ..
                } => {
                    let (s, v) = (shooter.index as usize, victim.index as usize);
                    self.tally(v).damage_taken += f64::from(damage);
                    if let Some(l) = self.life(victim) {
                        l.damage_taken += f64::from(damage);
                    }
                    let team = sim.units.get(s).map(|u| u.team);
                    if s != v && team.is_some_and(|t| Self::enemy_unit(sim, t, v)) {
                        let t = self.tally(s);
                        t.damage_dealt += f64::from(damage);
                        t.hits += u64::from(direct);
                        if let Some(l) = self.life(shooter) {
                            l.damage_dealt += f64::from(damage);
                            l.hits += u64::from(direct);
                        }
                    }
                }
                Event::FallDamage { victim, damage, .. } => {
                    self.tally(victim.index as usize).damage_taken += f64::from(damage);
                    if let Some(l) = self.life(victim) {
                        l.damage_taken += f64::from(damage);
                    }
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
                        let hit = u64::from(dir.is_some() && damage > 0.0);
                        t.hits += hit;
                        if let Some(l) = self.life(shooter) {
                            l.vehicle_damage_dealt += f64::from(damage);
                            l.hits += hit;
                        }
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
                        if let Some(l) = self.life(shooter) {
                            l.vehicle_damage_dealt += f64::from(damage);
                        }
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
        self.observe_heroes(sim);
        self.observe_units(walk);
        self.observe_events(sim);
        self.observe_ends(sim);
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
        let name_of = |i: usize| names.map(|f| f.unit_name(sim, sim.handle(i)));
        let life_row = |l: &Life| -> serde_json::Value {
            let end = l.end.as_ref();
            let killed_by = end.and_then(|e| e.by).map(|(h, k)| match k {
                Some(k) => {
                    let kl = &self.lives[k];
                    serde_json::json!({
                        "unit": kl.unit,
                        "human": kl.human,
                        "slot": kl.slot,
                        "team": kl.team,
                        "side": side(kl.team),
                        "class": kl.class,
                        "hero": kl.hero,
                    })
                }
                None => serde_json::json!({ "unit": h.index }),
            });
            serde_json::json!({
                "unit": l.unit,
                "human": l.human,
                "slot": l.slot,
                "team": l.team,
                "side": side(l.team),
                "class": l.class,
                "hero": l.hero,
                "spawn_tick": l.spawn_tick,
                "end_tick": end.map_or(end_tick.max(l.spawn_tick), |e| e.tick),
                "end": end.map_or("round_end", |e| e.how),
                "cause": end.and_then(|e| e.cause).map(|c| format!("{c:?}")),
                "killed_by": killed_by,
                "kills": l.kills,
                "team_kills": l.team_kills,
                "damage_dealt": round1(l.damage_dealt),
                "vehicle_damage_dealt": round1(l.vehicle_damage_dealt),
                "damage_taken": round1(l.damage_taken),
                "shots": l.shots,
                "hits": l.hits,
            })
        };
        let lives: Vec<_> = self.lives.iter().map(&life_row).collect();
        // Per slot and class, and per (team, human, class).
        let mut slot_classes: BTreeMap<u32, BTreeMap<&str, ClassSum>> = BTreeMap::new();
        let mut totals: BTreeMap<(u8, bool, &str), ClassSum> = BTreeMap::new();
        for l in &self.lives {
            slot_classes
                .entry(l.unit)
                .or_default()
                .entry(&l.class)
                .or_default()
                .add(l, end_tick);
            totals
                .entry((l.team, l.human, &l.class))
                .or_default()
                .add(l, end_tick);
        }
        let class_totals: Vec<_> = totals
            .iter()
            .map(|((team, human, class), sum)| {
                let mut v = sum.json();
                v["team"] = serde_json::json!(team);
                v["side"] = side(*team);
                v["human"] = serde_json::json!(human);
                v["class"] = serde_json::json!(class);
                v
            })
            .collect();
        let hero_lives: Vec<_> = self
            .lives
            .iter()
            .filter(|l| l.hero)
            .map(|l| {
                let mut v = life_row(l);
                v["name"] = serde_json::json!(name_of(l.unit as usize));
                v["unlocked_tick"] = serde_json::json!(l.unlocked_tick);
                v["offered_tick"] = serde_json::json!(l.offered_tick);
                v["accepted_tick"] = serde_json::json!(l.accepted_tick);
                v
            })
            .collect();
        let hero_classes = [0u8, 1].map(|t| {
            sim.hero_class(t)
                .map(|c| walk.team_template(t).class(c).name.clone())
        });
        let heroes = serde_json::json!({
            "hero_assault": sim.hero_assault(),
            "classes": hero_classes,
            "unlocks": self.hero_log.unlocks.iter().map(|(team, tick)| serde_json::json!({
                "team": team, "side": side(*team), "tick": tick,
            })).collect::<Vec<_>>(),
            "offers": self.hero_log.offers.iter().map(|(tick, h, team)| serde_json::json!({
                "tick": tick,
                "unit": h.index,
                "slot": sim.units.get(h.index as usize).and_then(|u| u.spec.player_slot),
                "team": team,
            })).collect::<Vec<_>>(),
            "lives": hero_lives,
        });
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
            let by_class = slot_classes.get(&(i as u32));
            let hero_sum = |f: fn(&ClassSum) -> f64| -> f64 {
                by_class.map_or(0.0, |m| m.values().filter(|c| c.hero).map(f).sum())
            };
            let by_class_json: serde_json::Map<String, serde_json::Value> = by_class
                .into_iter()
                .flatten()
                .map(|(c, sum)| ((*c).to_owned(), sum.json()))
                .collect();
            players.push(serde_json::json!({
                "hero_kills": hero_sum(|c| f64::from(c.kills)) as u32,
                "hero_deaths": hero_sum(|c| f64::from(c.deaths)) as u32,
                "hero_damage_dealt": round1(hero_sum(|c| c.damage_dealt)),
                "by_class": by_class_json,
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
            "class_totals": class_totals,
            "heroes": heroes,
            "lives": lives,
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

    /// A round where the human and a bot each play the hero: both get a
    /// hero life (accepted tick, class, end) beside their ordinary life,
    /// the human's kill of the bot hero is a hero kill, and the class
    /// totals split hero from ordinary classes.
    #[test]
    fn hero_lives_and_class_totals() {
        let mut walk = WalkSim::new(7, glam::Vec3::ZERO, None);
        let p = walk.player;
        let u = &walk.sim.units[p.index as usize];
        let class = core::unit::UnitClass {
            consts: u.spec.consts,
            max_health: u.health.max,
            health_type: u.spec.health_type,
            weapon_class: u.weapon_class,
            total_clips: 6,
            weapon_slot: u.weapon_slot,
            slots: u.spec.slots.clone(),
            jet: false,
            capture_posts: true,
            drops: Default::default(),
            chunk: None,
            droideka: None,
            min: 1,
            max: None,
            unlock_points: 0,
        };
        for team in 0..2 {
            walk.sim.classes[team] = vec![class.clone(), class.clone()];
            let mut hero = walk.teams[team].classes[0].clone();
            hero.name = format!("hero{team}");
            walk.teams[team].classes.truncate(1);
            walk.teams[team].classes.push(hero);
        }
        walk.sim.heroes = Some(core::sim::heroes::Heroes::new([Some(1), Some(1)]));
        // A post per side to respawn the heroes at.
        for (team, x) in [(0u8, 0.0f32), (1, 60.0)] {
            let pos = glam::Vec3::new(x, 0.0, 0.0);
            let (capture_time, neutralize_time) = core::cp::CpDef::class_times(true);
            walk.sim.add_cp(
                core::cp::CpDef {
                    name: format!("cp{team}"),
                    pos,
                    capture: core::region::Region::sphere(pos, 8.0),
                    control: None,
                    value_bleed: 10,
                    value_atk: [10; 2],
                    value_def: [10; 2],
                    spawn_nodes: vec![(pos, 0.0)],
                    capture_time,
                    neutralize_time,
                },
                Some(team),
            );
        }
        let spec = walk.teams[1].spec(
            1,
            glam::Vec3::new(30.0, 0.0, 0.0),
            0.0,
            core::unit::Controller::Ai,
            0,
        );
        let bot = walk.sim.spawn(&spec).expect("an enemy bot").index as usize;
        let dir =
            std::env::temp_dir().join(format!("openbf_session_heroes_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let info = RoundInfo {
            map: "test".into(),
            mode: "con".into(),
            role: "single".into(),
            ..Default::default()
        };
        let mut stats = SessionStats::start(info, &walk, dir.clone());
        let idle = [core::sim::PadState::default()];
        let run = |walk: &mut WalkSim, stats: &mut SessionStats, n: u64| {
            for _ in 0..n {
                walk.step_tick(&idle);
                stats.tick(walk);
            }
        };
        run(&mut walk, &mut stats, 2 * u64::from(TICK_HZ));
        let accepted = walk.sim.tick;
        walk.sim.events.clear();
        assert!(walk.sim.accept_scripted_hero(p.index));
        assert!(walk.sim.accept_scripted_hero(bot as u32));
        stats.tick(&walk);
        run(&mut walk, &mut stats, u64::from(TICK_HZ));
        let hero_p = walk.sim.handle(p.index as usize);
        let hero_b = walk.sim.handle(bot);
        assert!(walk.sim.is_hero(hero_p) && walk.sim.is_hero(hero_b));
        let killed = walk.sim.tick;
        walk.sim.events.clear();
        walk.sim.kill_unit_by(hero_b, Some(hero_p));
        stats.tick(&walk);
        run(&mut walk, &mut stats, u64::from(TICK_HZ));
        let round = crate::round_stats::RoundStats::default();
        let v = stats.json(&walk, &round, None, "quit");
        assert_eq!(v["schema"], 2);
        let heroes = &v["heroes"];
        assert_eq!(heroes["classes"], serde_json::json!(["hero0", "hero1"]));
        let hl = heroes["lives"].as_array().unwrap();
        assert_eq!(hl.len(), 2, "{hl:#?}");
        let human = hl.iter().find(|l| l["human"] == true).unwrap();
        let botl = hl.iter().find(|l| l["human"] == false).unwrap();
        assert_eq!(human["unit"], p.index);
        assert_eq!(human["class"], format!("hero{}", human["team"]));
        assert_eq!(human["accepted_tick"], accepted);
        assert_eq!(human["spawn_tick"], accepted);
        assert_eq!(human["kills"], 1);
        assert_eq!(human["end"], "round_end");
        assert_eq!(botl["unit"], bot);
        assert_eq!(botl["accepted_tick"], accepted);
        assert_eq!(botl["end"], "killed");
        assert_eq!(botl["end_tick"], killed);
        assert_eq!(botl["killed_by"]["unit"], p.index);
        assert_eq!(botl["killed_by"]["hero"], true);
        assert_eq!(botl["killed_by"]["human"], true);
        // Their ordinary lives before: ended by the accept, no death.
        let lives = v["lives"].as_array().unwrap();
        for unit in [u64::from(p.index), bot as u64] {
            let first = lives.iter().find(|l| l["unit"] == unit).unwrap();
            assert_eq!(first["hero"], false);
            assert_eq!(first["end"], "despawned");
            assert_eq!(first["end_tick"], accepted);
        }
        // The human's row: one hero kill, split by class.
        let row = v["players"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["unit"] == p.index)
            .unwrap();
        assert_eq!(row["hero_kills"], 1);
        assert_eq!(row["hero_deaths"], 0);
        let hero_name = human["class"].as_str().unwrap();
        assert_eq!(row["by_class"][hero_name]["hero"], true);
        assert_eq!(row["by_class"][hero_name]["kills"], 1);
        let totals = v["class_totals"].as_array().unwrap();
        let bot_hero = totals
            .iter()
            .find(|t| t["human"] == false && t["hero"] == true)
            .unwrap();
        assert_eq!(
            (bot_hero["lives"].clone(), bot_hero["deaths"].clone()),
            (1.into(), 1.into())
        );
        assert!(totals
            .iter()
            .any(|t| t["human"] == false && t["hero"] == false && t["team"] == botl["team"]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn era_and_safe_names() {
        assert_eq!(era(&["rep".into(), "cis".into()]), "cw");
        assert_eq!(era(&["all".into(), "imp".into()]), "gcw");
        assert_eq!(safe("tat2/con x"), "tat2_con_x");
    }
}
