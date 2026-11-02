//! The PS2's event messages (card EVMSG, `analysis/re-event-messages.md`):
//! the sim's events turned into the lines of BF2's `player1systemMessages`
//! and the numbers of its statistics pop-up, for ONE viewer — the local
//! player. `ui::hud::feed` sends them into the HUD tree.
//!
//! What makes a line (§1, §1.1 [V][O]) — neither game has a kill feed:
//!
//! - a soldier's death, only when the victim is a human: "%s killed %s",
//!   "%s killed %s - [TK]", "%s killed himself", and with no killer
//!   "%s died". An AI's death makes no line; the player's own kill shows as
//!   his statistics. A hero victim also makes a line when killed by a human
//!   or hero in a single-screen battle.
//! - `[ours]` a post captured or lost, on every map: "%s captured the %s" /
//!   "%s lost the %s" — BF1's lines in BF2's own unused strings (§7.3:
//!   The maintainer's ask; the BF2 PS2 shows none — [`POST_LINES`] reverses it).
//!
//! The local player's squad order prints no line (the PS2 prints none): the
//! feed keeps the press for BF1's D-pad cross — its arm, the PS2's 2 s timer
//! on the sim's clock and `[ours]` the count of units the order changed
//! (card SQHUD, [`EventFeed::squad`], drawn by `ui::hud_squad`).
//!
//! Colours are the viewer's (§1.2 [V][M]): a kill line takes the VICTIM's
//! team as the viewer sees it, a post line the capturing / losing team's, a
//! death with no killer nobody's grey.
//!
//! The target readout (card HTGT, `analysis/re-target-hud.md` §3-4): what
//! the reticle picks, by its class's name and health ([`EventFeed::target`]).
//!
//! The hit-direction chevrons (card HATK, `analysis/re-hud-attacker.md`):
//! the edge each hit on the viewer lights ([`ambush_edge`],
//! [`EventFeed::take_ambush`]), drawn by `ui::hud_ambush`.
//!
//! The critical-hit flash (hitmark, `analysis/re-target-hud.md` §12): the
//! viewer's sniper-rifle headshot, or his rocket launcher's hit on a
//! vehicle's weak spot, lights BF2's `hitCritical` reticle mark
//! ([`EventFeed::hit_critical`]). Every other hit he deals an enemy soldier
//! lights the plain `hit` flash ([`EventFeed::hit_body`], [ours]
//! 2026-10-29).
//!
//! The statistics (§5, §9): kills less two per team kill, and BF2's points
//! from its own table (PS2 RAM [M]) — see [`points`].
//!
//! Two rules carried over from the placeholder box (card SQMK):
//!
//! - **A tick's events are read once.** `Sim::events` holds the LAST stepped
//!   tick's events until the next step; a render frame that steps no tick
//!   must not read them again.
//! - **An identical line is one line** while the first is still up.

use core::projectile::Target;
use core::sim::{AimTarget, Event, Sim};
use core::unit::UnitHandle;
use core::vehicle::{SeatKind, VehicleKind};
use glam::Quat;
use manifest::hud::HudLocl;
use std::path::Path;
use ui::hud::{message_color, HudMessage, HudStats, HudTarget};
use ui::hud_ambush::Edge;
use ui::hud_squad::{self, SquadCross};

/// The PS2's hit timer: 0.5 s (B2 `FUN_00350558`), in sim ticks.
pub const HIT_TIMER_TICKS: u64 = core::TICK_HZ as u64 / 2;

/// Is weapon class `class` a sniper rifle - the AI sniper table's class
/// (`WeaponClass::ai_distance_table`: no spread, single shot, zoom 6 or
/// more)? Only these light the critical-hit flash on a head (hitmark: the maintainer
/// 2026-10-28 asked for sniper headshots; the PS2 flashes every head hit).
pub fn sniper_rifle(sim: &Sim, class: usize) -> bool {
    sim.weapon_classes
        .get(class)
        .is_some_and(|w| w.ai_distance_table())
}

/// Is weapon class `class` an infantry rocket launcher
/// (`WeaponClass::rocket_launcher`)? Only these light the critical-hit
/// flash on a vehicle's weak spot (hitmark, the maintainer 2026-10-28: the hits
/// BF2's demolition award counts).
pub fn rocket_launcher(sim: &Sim, class: usize) -> bool {
    sim.weapon_classes
        .get(class)
        .is_some_and(|w| w.rocket_launcher())
}

/// The mark's colour for the viewer's critical hit on `victim` with
/// `class`, `None` for no mark: a sniper rifle on a soldier's head or a
/// rocket launcher on a vehicle's weak spot. The PS2's `hitColor`
/// (B2 `FUN_003b3398`, `re-target-hud.md` §12): friend blue, enemy red, and
/// white for a target of no side (an empty vehicle). The slot is read dead
/// or alive (the hit may have killed it).
fn critical_mark(sim: &Sim, victim: Target, class: usize, my_team: u8) -> Option<[u8; 3]> {
    use ui::hud::target_color::{ENEMY, FRIEND, NONE};
    let team = match victim {
        Target::Unit(h) if sniper_rifle(sim, class) => {
            sim.units.get(h.index as usize).map(|u| u.team)
        }
        Target::Vehicle(h) if rocket_launcher(sim, class) => {
            sim.vehicles.get(h.index as usize)?.team
        }
        _ => return None,
    };
    Some(match team {
        None => NONE,
        Some(t) if t == my_team => FRIEND,
        Some(_) => ENEMY,
    })
}

/// Names made per team: more AI than this on one team reuse them in turn.
const BOT_NAMES: usize = 64;

/// `[ours]` Post lines on every map. The BF2 PS2 shows none (its events 4
/// and 5 only speak, §1 [V][O]); BF1 shows both. `false` is the BF2 disc.
pub const POST_LINES: bool = true;

/// How long an identical line is held back: a line's life on screen (0.5 s
/// in, 3 s held, 0.5 s out).
const SAME_LINE_TICKS: u64 = 4 * core::TICK_HZ as u64;

/// BF2's points per event (§9: the table at 0x5fedf0, filled at run time by
/// `ScriptCB_SetPlayerStatsPoints`, read from two HOT1 save states [M]; the
/// events by their call sites [V]). Not built: the 2-point kills beside a
/// post (events 0x17, 0x18, 0x1b, 0x1d, 0x1e).
pub mod points {
    /// Event 0: killing an AI soldier (`FUN_00362268`).
    pub const KILL_AI: i32 = 1;
    /// Event 1: killing a human.
    pub const KILL_HUMAN: i32 = 2;
    /// Event 0x14: killing a hero (`FUN_00362268`).
    pub const KILL_HERO: i32 = 5;
    pub fn kill(human: bool, hero: bool) -> i32 {
        if hero {
            KILL_HERO
        } else if human {
            KILL_HUMAN
        } else {
            KILL_AI
        }
    }
    /// Event 3: killing oneself (`FUN_00362c60`), or dying with no killer
    /// and no killing team (`FUN_0034f650`).
    pub const SUICIDE: i32 = -1;
    /// Event 4: killing a teammate.
    pub const TEAM_KILL: i32 = -2;
    /// Event 0x15: capturing a post as the unit longest in its capture
    /// region (`FUN_00357440` → `FUN_00363a30`).
    pub const CAPTURE: i32 = 5;
    /// Event 0x16: standing in the capture region when another captures it.
    pub const CAPTURE_ASSIST: i32 = 2;
}

/// The English lines the feed is built from, by BF2's own Locl hashes (§1's
/// table), with the disc's text as the fallback when no bake is at hand.
#[derive(Debug, Clone)]
pub struct Texts {
    /// BF2's table: the lines, the team names, the soldier names.
    bf2: Option<HudLocl>,
    /// The map's own game's table: its post names.
    map: Option<HudLocl>,
    /// The map is a BF1 map: its names are upper case on the disc.
    bf1_map: bool,
}

const KILLED: (u32, &str) = (0xacc1_eef7, "%s killed %s");
const TEAM_KILLED: (u32, &str) = (0xc57a_e533, "%s killed %s - [TK]");
const SUICIDE: (u32, &str) = (0x6163_33ea, "%s killed himself");
const DIED: (u32, &str) = (0x0a9f_4178, "%s died");
const DEFEATED: (u32, &str) = (0x083a_b1e9, "%s defeated %s");
const FLED: (u32, &str) = (0x07aa_d4c1, "%s has fled");
const HERO_SPAWNED: (u32, &str) = (0x0f66_9681, "%s is now playing as %s");
/// The killer's name a dispensed turret carries in BF2's death line (D:438921).
const AUTO_TURRET: (u32, &str) = (0x664f_fe37, "Auto Turret");
/// The recon droid's name in the discs' tables, for a droid weapon whose own
/// `weapons.<side>.<kind>.<rest>` key has none.
const RECON_DROID: (u32, &str) = (0x950b6800, "Recon Droid");
const CAPTURED: (u32, &str) = (0xdd27_9cfc, "%s captured the %s");
const LOST: (u32, &str) = (0xae1e_c790, "%s lost the %s");
const COMMAND_POST: (u32, &str) = (0x7e64_bc51, "Command Post");

pub(crate) fn read_locl(dir: &Path) -> Option<HudLocl> {
    let text = pak::read_to_string(dir.join("locl/english.ron")).ok()?;
    ron::from_str(&text).ok()
}

/// `%s` filled left to right.
fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut args = args.iter();
    let mut rest = template;
    while let Some(at) = rest.find("%s") {
        out.push_str(&rest[..at]);
        out.push_str(args.next().copied().unwrap_or(""));
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

/// "PIER ONE" → "Pier One": a BF1 name in BF2's case. Words of one or two
/// letters keep theirs ("LZ").
fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) if w.chars().count() > 2 => {
                    first.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()
                }
                _ => w.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_name(text: &str, bf1: bool) -> Option<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match text.is_empty() {
        true => None,
        false if bf1 => Some(title_case(&text)),
        false => Some(text),
    }
}

impl Texts {
    /// The tables `bake locl` wrote under `assets_dir`, for a map of
    /// `source_game`. Missing tables fall back to the built-in lines.
    pub fn load(assets_dir: &Path, source_game: &str) -> Self {
        let bf1_map = source_game.eq_ignore_ascii_case("bf1");
        let bf2 = read_locl(&assets_dir.join("common_bf2"));
        if bf2.is_none() {
            eprintln!("hud: no baked text table (bake common); names fall back");
        }
        let map = if bf1_map {
            read_locl(&assets_dir.join("common_bf1"))
        } else {
            bf2.clone()
        };
        Self { bf2, map, bf1_map }
    }

    /// Only the built-in lines (tests, a bake with no text).
    pub fn builtin() -> Self {
        Self {
            bf2: None,
            map: None,
            bf1_map: false,
        }
    }

    /// An order's name on the cross: BF1's own table on a BF1 map, else
    /// BF2's, else the disc's English.
    fn order_label(&self, (key, fallback): (&str, &str)) -> String {
        let hash = manifest::hud::locl_key(key);
        [&self.map, &self.bf2]
            .into_iter()
            .flatten()
            .find_map(|l| l.get(hash))
            .unwrap_or(fallback)
            .to_owned()
    }

    fn line(&self, (hash, fallback): (u32, &str)) -> String {
        self.bf2
            .as_ref()
            .and_then(|l| l.get(hash))
            .unwrap_or(fallback)
            .to_owned()
    }

    /// The BF2 table's text under the key path `key`, line ends and the
    /// trailing blank lines kept as the disc has them; `None` without one.
    pub fn bf2_text(&self, key: &str) -> Option<&str> {
        self.bf2.as_ref()?.text(key)
    }

    /// The map's game's table's text under `key` (BF1's own for a BF1
    /// map); `None` without one.
    pub fn map_text(&self, key: &str) -> Option<&str> {
        self.map.as_ref()?.text(key)
    }

    /// A team's name: `common.sides.<faction>.name` ("Rebels", "Empire").
    pub fn team_name(&self, faction: &str) -> String {
        self.bf2
            .as_ref()
            .and_then(|l| l.text(&format!("common.sides.{faction}.name")))
            .filter(|t| !t.trim().is_empty())
            .map_or_else(|| faction.to_uppercase(), str::to_owned)
    }

    /// A post's name by its key path (`level.<map>.<label>`: [D] the path
    /// that names 227 of the discs' 248 posts, and BF1 KAS2's CP5 "PIER ONE"
    /// as the PS2 printed it), else "Command Post" (the PS2's own fallback,
    /// §1 [V]).
    pub fn post_name(&self, key: &str) -> String {
        self.map
            .as_ref()
            .and_then(|l| l.text(key))
            .and_then(|t| display_name(t, self.bf1_map))
            .unwrap_or_else(|| self.line(COMMAND_POST))
    }

    /// A soldier class's name as the spawn screen prints it (card SPSEL):
    /// class `<side>_<rest>` is keyed `entity.<side>.<rest>` in both games'
    /// tables ("rep_inf_ep3_sniper" → "Clone Sharpshooter", BF1
    /// "all_inf_smuggler" → "WOOKIEE SMUGGLER", title-cased like the post
    /// names), the map's own game's table first. `None` when neither has it.
    pub fn class_name(&self, class: &str) -> Option<String> {
        let (side, rest) = class.split_once('_')?;
        let hash = manifest::hud::locl_key(&format!("entity.{side}.{rest}"));
        let (text, bf1) = [(&self.map, self.bf1_map), (&self.bf2, false)]
            .into_iter()
            .find_map(|(l, bf1)| Some((l.as_ref()?.get(hash)?, bf1)))?;
        display_name(text, bf1)
    }

    pub fn weapon_name(&self, class: &str) -> Option<String> {
        let (side, rest) = class.split_once('_')?;
        let (kind, rest) = rest.split_once('_')?;
        let key = format!("weapons.{side}.{kind}.{rest}");
        [(&self.map, self.bf1_map), (&self.bf2, false)]
            .into_iter()
            .find_map(|(table, bf1)| display_name(table.as_ref()?.text(&key)?, bf1))
    }

    /// The side's name lists, `<side>.firstnames.01` … and `.lastnames.01` …
    /// (§6: up to 64 each).
    fn name_lists(&self, faction: &str) -> (Vec<String>, Vec<String>) {
        let list = |kind: &str| -> Vec<String> {
            let Some(l) = &self.bf2 else {
                return Vec::new();
            };
            // The table keeps the source's escaped apostrophe ("Jal\\'daan");
            // the PS2 prints "JAL'DAAN".
            (1..=64)
                .map_while(|n| l.text(&format!("common.sides.{faction}.{kind}.{n:02}")))
                .map(|t| t.replace("\\'", "'"))
                .collect()
        };
        (list("firstnames"), list("lastnames"))
    }
}

/// The four field order names of BF1's D-pad cross by their Locl keys, with
/// the disc's English as the fallback: up, down, right, left
/// (`re-squad-orders.md` §2).
const ORDER_LABELS: [(&str, &str); 4] = [
    ("game.squadcommand.field.up", "MOVE OUT"),
    ("game.squadcommand.field.down", "HOLD POSITION"),
    ("game.squadcommand.field.right", "FOLLOW ME"),
    ("game.squadcommand.field.left", "AT EASE"),
];

const DRIVER_LABELS: [(&str, &str); 4] = [
    ("game.squadcommand.driver.up", ""),
    ("game.squadcommand.driver.down", ""),
    ("game.squadcommand.driver.right", "PILE IN"),
    ("game.squadcommand.driver.left", "EVERYONE OUT"),
];

/// The cross's arm for `Event::Command`'s `kind` byte (0 follow, 1 hold,
/// 2 at ease, 3 move out, 4 pile in, 5 everyone out).
pub fn order_arm(kind: u8) -> u8 {
    match kind {
        0 | 4 => hud_squad::RIGHT,
        1 => hud_squad::DOWN,
        2 | 5 => hud_squad::LEFT,
        _ => hud_squad::UP,
    }
}

/// The local player's last order, for the cross.
#[derive(Debug, Clone, Copy)]
struct Press {
    tick: u64,
    kind: u8,
    /// `[ours]` the units it changed so far (`Event::OrderTaken`'s `did`).
    took: u32,
}

/// One viewer's event lines and statistics.
#[derive(Debug)]
pub struct EventFeed {
    texts: Texts,
    /// Per team: its name and its AI soldiers' names in the order they are
    /// made (`crate::bot_names`).
    teams: [(String, Vec<String>); 2],
    /// Per team: its side ("rep", "cis", ...).
    sides: [String; 2],
    /// Per post (`Sim::cps` order): its name.
    posts: Vec<String>,
    /// Per team, its soldier classes' names in the class table's order
    /// ([`Texts::class_name`], else the roster's label). Empty for a feed
    /// built without a walk.
    classes: [Vec<String>; 2],
    /// The vehicle and building-turret classes' names, by `Vehicle::class`
    /// ([`Texts::class_name`], else the class's `Label`, else its ODF name).
    vehicles: Vec<String>,
    /// The station classes' Locl names (health and ammo droids), by
    /// `Sim::station_classes` index.
    stations: Vec<String>,
    /// The lines made since the last [`EventFeed::take`], oldest first.
    out: Vec<HudMessage>,
    /// The chevron edges the viewer's hits lit since the last
    /// [`EventFeed::take_ambush`] (card HATK).
    ambush: Vec<Edge>,
    /// The viewer's last marked critical hit: its tick and the
    /// victim's relation colour ([`EventFeed::hit_critical`]).
    critical: Option<(u64, [u8; 3])>,
    /// The tick of the viewer's last body hit on an enemy
    /// ([`EventFeed::hit_body`]).
    body: Option<u64>,
    /// What was said lately, and when (the one-line rule).
    recent: Vec<(String, u64)>,
    /// The tick whose events were already read.
    seen: Option<u64>,
    /// Per post: its owner when the last tick was read (who loses it).
    owners: Vec<Option<u8>>,
    /// Per unit: the post whose capture region it stands in, and since when.
    inside: Vec<Option<(u32, u64)>>,
    points: i32,
    kills: i32,
    battle: crate::profile::BattleStats,
    /// Every soldier's round, for the stats pages (issue #50).
    round: crate::round_stats::RoundStats,
    /// The four order names, up / down / right / left.
    order_labels: [String; 4],
    driver_labels: [String; 4],
    order: Option<Press>,
}

impl Default for EventFeed {
    fn default() -> Self {
        Self::new(Texts::builtin(), ["", ""], &[])
    }
}

impl EventFeed {
    /// A feed for a battle of `factions` over posts with the key paths
    /// `post_keys` (`Sim::cps` order).
    pub fn new(texts: Texts, factions: [&str; 2], post_keys: &[String]) -> Self {
        let team = |f: &str| {
            let (first, last) = texts.name_lists(f);
            let names = crate::bot_names::BotNames::list(f, &first, &last, BOT_NAMES);
            (texts.team_name(f), names)
        };
        Self {
            teams: [team(factions[0]), team(factions[1])],
            sides: factions.map(str::to_owned),
            posts: post_keys.iter().map(|k| texts.post_name(k)).collect(),
            classes: [Vec::new(), Vec::new()],
            vehicles: Vec::new(),
            stations: Vec::new(),
            order_labels: ORDER_LABELS.map(|l| texts.order_label(l)),
            driver_labels: DRIVER_LABELS.map(|l| texts.order_label(l)),
            texts,
            out: Vec::new(),
            ambush: Vec::new(),
            critical: None,
            body: None,
            recent: Vec::new(),
            seen: None,
            owners: Vec::new(),
            inside: Vec::new(),
            points: 0,
            kills: 0,
            round: Default::default(),
            battle: Default::default(),
            order: None,
        }
    }

    /// The feed of `walk`'s battle, with the text baked under `assets_dir`.
    pub fn for_walk(assets_dir: &Path, source_game: &str, walk: &crate::walk::WalkSim) -> Self {
        let mut feed = Self::new(
            Texts::load(assets_dir, source_game),
            [&walk.teams[0].faction, &walk.teams[1].faction],
            &walk.post_keys,
        );
        feed.classes = [0, 1].map(|t| {
            walk.teams[t]
                .classes
                .iter()
                .map(|c| {
                    feed.texts
                        .class_name(&c.name)
                        .unwrap_or_else(|| c.label.clone())
                })
                .collect()
        });
        feed.vehicles = walk
            .vehicle_classes
            .iter()
            .enumerate()
            .map(|(i, c)| {
                feed.texts.class_name(c).unwrap_or_else(|| {
                    walk.vehicle_labels
                        .get(i)
                        .cloned()
                        .flatten()
                        .unwrap_or_else(|| c.clone())
                })
            })
            .collect();
        feed.stations = walk
            .sim
            .station_classes
            .iter()
            .map(|c| {
                // BF2's unbuilt and hero droids have no text of their own:
                // the droid's ("Medical Droid").
                let base = c.name.trim_end_matches("_unbuilt").replace("_hero_", "_");
                feed.texts
                    .class_name(&c.name)
                    .or_else(|| feed.texts.class_name(&base))
                    .unwrap_or_else(|| c.name.clone())
            })
            .collect();
        feed
    }

    /// Team `team`'s soldier class names, in its class table's order (empty
    /// without a walk).
    pub fn class_names(&self, team: u8) -> &[String] {
        &self.classes[usize::from(team.min(1))]
    }

    pub fn extend_class_names(&mut self, walk: &crate::walk::WalkSim) {
        for team in 0..2 {
            for class in &walk.teams[team].classes[self.classes[team].len()..] {
                self.classes[team].push(
                    self.texts
                        .class_name(&class.name)
                        .unwrap_or_else(|| class.label.clone()),
                );
            }
        }
    }

    /// A shell or spawn-screen text from BF2's table by its key path
    /// (`ifs.sideselect.chooseteam`), else `fallback` - the disc's English.
    pub fn screen_text(&self, key: &str, fallback: &str) -> String {
        self.texts.bf2_text(key).unwrap_or(fallback).to_owned()
    }

    /// A text the ELF names by its hash (the class panel's, B2
    /// `FUN_00396430`), else `fallback`.
    pub fn text_by_hash(&self, hash: u32, fallback: &str) -> String {
        self.texts.line((hash, fallback))
    }

    pub fn weapon_name(&self, class: &str) -> Option<String> {
        self.texts.weapon_name(class)
    }

    /// The boundary's warning in the map's game's words (`re-boundary.md` T4).
    pub fn leaving_text(&self) -> String {
        const KEY: &str = "game.tooltips.leaving_battlefield";
        let text = self
            .texts
            .map_text(KEY)
            .or_else(|| self.texts.bf2_text(KEY));
        text.map_or_else(|| "LEAVING BATTLEFIELD".to_owned(), |t| t.trim().to_owned())
    }

    /// The two teams' names as the lines print them.
    pub fn team_names(&self) -> [&str; 2] {
        [&self.teams[0].0, &self.teams[1].0]
    }

    /// The two teams' sides ("rep", "cis", ...).
    pub fn sides(&self) -> [&str; 2] {
        [&self.sides[0], &self.sides[1]]
    }

    /// The posts' names as the lines print them, in `Sim::cps` order.
    pub fn post_names(&self) -> &[String] {
        &self.posts
    }

    /// The name a line gives unit `h`: "Player 1" for a human (the PS2's
    /// `Player %d`); an AI the team's next name in the order its AI units
    /// are made (unit order, `crate::bot_names`): the same unit keeps it
    /// through its respawns.
    pub fn unit_name(&self, sim: &Sim, h: UnitHandle) -> String {
        let i = h.index as usize;
        let u = &sim.units[i];
        match u.spec.player_slot {
            Some(slot) => format!("Player {}", u32::from(slot) + 1),
            None => {
                let k = sim.units[..i]
                    .iter()
                    .filter(|o| o.team == u.team && o.spec.player_slot.is_none())
                    .count();
                let names = &self.teams[usize::from(u.team.min(1))].1;
                names
                    .get(k % names.len().max(1))
                    .cloned()
                    .unwrap_or_else(|| format!("Unit {i}"))
            }
        }
    }

    fn post_name(&self, cp: u32, sim: &Sim) -> String {
        self.posts
            .get(cp as usize)
            .cloned()
            .unwrap_or_else(|| sim.cps[cp as usize].def.name.clone())
    }

    /// The squad order cross at sim tick `tick`: the last order's arm, the
    /// PS2's timer on the sim's clock and `[ours]` the count of units the
    /// order changed (none while that is 0); `None` once it has faded.
    pub fn squad(&self, tick: u64) -> Option<SquadCross> {
        let o = self.order?;
        let seconds = tick.saturating_sub(o.tick) as f32 / core::TICK_HZ as f32;
        Some(SquadCross {
            labels: if o.kind == core::squad::Order::PileIn as u8
                || o.kind == core::squad::Order::GetOut as u8
            {
                self.driver_labels.clone()
            } else {
                self.order_labels.clone()
            },
            arm: order_arm(o.kind),
            count: (o.took > 0).then_some(o.took),
            timer: SquadCross::timer_at(seconds)?,
        })
    }

    /// Queue `text` unless the same line is still up.
    pub fn say(&mut self, text: String, color: [u8; 3], tick: u64) {
        self.recent
            .retain(|(_, t)| tick.saturating_sub(*t) < SAME_LINE_TICKS);
        if self.recent.iter().any(|(l, _)| *l == text) {
            return;
        }
        self.recent.push((text.clone(), tick));
        self.out.push(HudMessage { text, color });
    }

    /// Read the sim's events if its tick is new. `viewer` is the local
    /// player: the lines are coloured for his team and the statistics are
    /// his. Call once per stepped tick (and freely more often).
    pub fn tick(&mut self, sim: &Sim, viewer: UnitHandle) {
        let tick = sim.tick;
        if self.seen == Some(tick) {
            return;
        }
        self.seen = Some(tick);
        let my_team = sim.units[viewer.index as usize].team;
        // A team as the viewer's team sees it (§1.2).
        let relation = |team: u8| {
            if team == my_team {
                message_color::OWN
            } else {
                message_color::ENEMY
            }
        };
        self.note_capture_regions(sim);
        self.round.tick(sim);

        for e in &sim.events {
            if let Some(edge) = ambush_edge(sim, viewer, e) {
                self.ambush.push(edge);
            }
            match *e {
                Event::Shot { unit, .. } if unit == viewer => {
                    self.battle.shots = self.battle.shots.saturating_add(1);
                }
                Event::Hit {
                    shooter,
                    victim,
                    damage,
                    ..
                } if shooter == viewer
                    && sim
                        .units
                        .get(victim.index as usize)
                        .is_some_and(|u| u.gen == victim.gen && u.team != my_team) =>
                {
                    // The plain hit flash: any enemy body hit ([ours]).
                    self.body = Some(tick);
                    if damage > 0.0 {
                        self.battle.hits = self.battle.hits.saturating_add(1);
                    }
                }
                Event::VehicleHit {
                    shooter: Some(shooter),
                    vehicle,
                    damage,
                    ..
                } if shooter == viewer
                    && damage > 0.0
                    && sim
                        .vehicles
                        .get(vehicle.index as usize)
                        .is_some_and(|v| v.gen == vehicle.gen && v.team != Some(my_team)) =>
                {
                    self.battle.hits = self.battle.hits.saturating_add(1);
                }
                Event::Kill { shooter, victim } => {
                    self.round.kill(sim, shooter, victim);
                    let (k, v) = (
                        &sim.units[shooter.index as usize],
                        &sim.units[victim.index as usize],
                    );
                    let hero = sim.hero_class(v.team) == Some(v.spec.class);
                    let killer_hero = sim.hero_class(k.team) == Some(k.spec.class);
                    // PS2's offline single-screen gate, §1.1.
                    if v.is_player
                        || (hero
                            && sim.players.iter().flatten().count() == 1
                            && (k.is_player || killer_hero))
                    {
                        let (kn, vn) = (self.unit_name(sim, shooter), self.unit_name(sim, victim));
                        let hero_name = self.classes[usize::from(v.team.min(1))]
                            .get(usize::from(v.spec.class))
                            .cloned()
                            .unwrap_or(vn.clone());
                        let text = if hero && shooter == victim {
                            fill(&self.texts.line(FLED), &[&hero_name])
                        } else if hero {
                            fill(&self.texts.line(DEFEATED), &[&kn, &hero_name])
                        } else if shooter == victim {
                            fill(&self.texts.line(SUICIDE), &[&vn])
                        } else if k.team == v.team {
                            fill(&self.texts.line(TEAM_KILLED), &[&kn, &vn])
                        } else {
                            fill(&self.texts.line(KILLED), &[&kn, &vn])
                        };
                        self.say(text, relation(v.team), tick);
                    }
                    if shooter == viewer {
                        if shooter == victim {
                            self.kills -= 1;
                            self.points += points::SUICIDE;
                        } else if k.team == v.team {
                            self.kills -= 1;
                            self.points += points::TEAM_KILL;
                        } else {
                            self.kills += 1;
                            self.points += points::kill(v.is_player, hero);
                        }
                    }
                }
                Event::CriticalHit {
                    shooter,
                    victim,
                    weapon_class,
                } if shooter == viewer => {
                    if let Some(color) = critical_mark(sim, victim, weapon_class, my_team) {
                        self.critical = Some((tick, color));
                    }
                }
                Event::EnvDeath { unit, .. } => {
                    if sim.units[unit.index as usize].is_player {
                        let u = &sim.units[unit.index as usize];
                        let hero = sim.hero_class(u.team) == Some(u.spec.class);
                        let name = if hero {
                            self.classes[usize::from(u.team.min(1))]
                                .get(usize::from(u.spec.class))
                                .cloned()
                                .unwrap_or_else(|| self.unit_name(sim, unit))
                        } else {
                            self.unit_name(sim, unit)
                        };
                        let text = fill(&self.texts.line(if hero { FLED } else { DIED }), &[&name]);
                        self.say(text, message_color::NEUTRAL, tick);
                    }
                    if unit == viewer {
                        self.points += points::SUICIDE;
                    }
                    self.round.env_death(sim, unit);
                }
                Event::Respawn { unit }
                    if sim.units.get(unit.index as usize).is_some_and(|u| {
                        u.gen == unit.gen && sim.hero_class(u.team) == Some(u.spec.class)
                    }) =>
                {
                    let u = &sim.units[unit.index as usize];
                    let name = self.classes[usize::from(u.team.min(1))]
                        .get(usize::from(u.spec.class))
                        .cloned()
                        .unwrap_or_default();
                    let text = fill(
                        &self.texts.line(HERO_SPAWNED),
                        &[&self.unit_name(sim, unit), &name],
                    );
                    self.say(text, relation(u.team), tick);
                }
                // The plain hit flash on an enemy's auto-turret too (the PS2's
                // hit call takes any victim of enemy relation, B2 `FUN_001e0580`).
                Event::TurretHit { shooter, team, .. } if shooter == viewer && team != my_team => {
                    self.body = Some(tick);
                }
                Event::VehicleDestroyed {
                    vehicle,
                    killer: Some(killer),
                    ..
                } => {
                    let team = sim
                        .vehicles
                        .get(vehicle.index as usize)
                        .and_then(|v| v.team);
                    self.round.vehicle(sim, killer, team);
                }
                Event::Captured { cp, team } => {
                    if POST_LINES {
                        let text = fill(
                            &self.texts.line(CAPTURED),
                            &[
                                &self.teams[usize::from(team.min(1))].0,
                                &self.post_name(cp, sim),
                            ],
                        );
                        self.say(text, relation(team), tick);
                    }
                    if let Some(first) = self.capture_credit(sim, viewer, cp, team) {
                        self.battle.captures = self.battle.captures.saturating_add(1);
                        self.points += if first {
                            points::CAPTURE
                        } else {
                            points::CAPTURE_ASSIST
                        };
                    }
                    // Every soldier's share, for the stats pages (issue #50).
                    for i in 0..sim.units.len() {
                        if let Some(first) = self.capture_credit(sim, sim.handle(i), cp, team) {
                            let u = &sim.units[i];
                            self.round.capture(
                                i,
                                first,
                                sim.hero_class(u.team) == Some(u.spec.class),
                            );
                        }
                    }
                }
                Event::Neutralized { cp, by } => {
                    if POST_LINES {
                        // The team that held it when the last tick was read.
                        let loser = self
                            .owners
                            .get(cp as usize)
                            .copied()
                            .flatten()
                            .unwrap_or(1 - by.min(1));
                        let text = fill(
                            &self.texts.line(LOST),
                            &[
                                &self.teams[usize::from(loser.min(1))].0,
                                &self.post_name(cp, sim),
                            ],
                        );
                        self.say(text, relation(loser), tick);
                    }
                }
                // The LOCAL player's own orders, and only theirs — slot 0 is
                // the local player everywhere in this binary. The press shows
                // the cross (card SQHUD); the PS2 prints no line.
                Event::Command { slot: 0, kind, .. } => {
                    self.order = Some(Press {
                        tick,
                        kind,
                        took: 0,
                    });
                }
                Event::OrderTaken {
                    slot: 0, did: true, ..
                } => {
                    if let Some(o) = &mut self.order {
                        o.took += 1;
                    }
                }
                _ => {}
            }
        }
        self.owners = sim.cps.iter().map(|c| c.owner).collect();
    }

    /// Keep, per unit, the post whose capture region it stands in and the
    /// tick it entered (the PS2 keeps the same pair on the player, and the
    /// capture's 5 points go to the one longest in, §9).
    fn note_capture_regions(&mut self, sim: &Sim) {
        self.inside.resize(sim.units.len(), None);
        for (i, u) in sim.units.iter().enumerate() {
            let at = (u.alive && u.capture_posts)
                .then(|| {
                    sim.cps
                        .iter()
                        .position(|c| c.def.capture.contains(u.soldier.pos))
                })
                .flatten()
                .map(|c| c as u32);
            self.inside[i] = match (at, self.inside[i]) {
                (Some(cp), Some((was, since))) if cp == was => Some((cp, since)),
                (Some(cp), _) => Some((cp, sim.tick)),
                (None, _) => None,
            };
        }
    }

    /// `None`: the viewer had no part in `team`'s capture of `cp`.
    /// `Some(true)`: positive residence and nobody of that team was there longer.
    fn capture_credit(&self, sim: &Sim, viewer: UnitHandle, cp: u32, team: u8) -> Option<bool> {
        let me = &sim.units[viewer.index as usize];
        let (_, since) = self
            .inside
            .get(viewer.index as usize)
            .copied()
            .flatten()
            .filter(|(at, _)| *at == cp && me.team == team)?;
        let longer = sim.units.iter().enumerate().any(|(i, u)| {
            u.team == team
                && i != viewer.index as usize
                && matches!(self.inside.get(i), Some(Some((at, s))) if *at == cp && *s < since)
        });
        Some(since < sim.tick && !longer)
    }

    /// The lines made since the last call, oldest first.
    pub fn take(&mut self) -> Vec<HudMessage> {
        std::mem::take(&mut self.out)
    }

    /// The critical-hit flash at `tick` (`HudModel::hit_critical`): the
    /// victim's relation colour while the PS2's 0.5 s hit timer of the
    /// viewer's last sniper-rifle headshot or rocket-launcher weak-spot hit
    /// runs (B2 `FUN_00350558` sets it,
    /// the player update `FUN_0034cea8` counts it down;
    /// `analysis/re-target-hud.md` §12), else `None`.
    pub fn hit_critical(&self, tick: u64) -> Option<[u8; 3]> {
        self.critical
            .filter(|&(at, _)| tick.saturating_sub(at) < HIT_TIMER_TICKS)
            .map(|(_, c)| c)
    }

    /// The plain hit flash at `tick` (`HudModel::hit_body`, [ours] the
    /// maintainer 2026-10-29): enemy red while the 0.5 s hit timer of the
    /// viewer's last hit on an enemy soldier runs, any weapon, blasts too;
    /// else `None`. The HUD sends the critical mark instead while one runs.
    pub fn hit_body(&self, tick: u64) -> Option<[u8; 3]> {
        self.body
            .filter(|&at| tick.saturating_sub(at) < HIT_TIMER_TICKS)
            .map(|_| ui::hud::target_color::ENEMY)
    }

    /// The chevron edges lit since the last call (`HudModel::ambush`).
    pub fn take_ambush(&mut self) -> Vec<Edge> {
        std::mem::take(&mut self.ambush)
    }

    /// What player slot `slot`'s reticle picks (`Sim::reticle_pick`) as the
    /// target readout shows it (`re-target-hud.md` §3-4 [V][O]): its class's
    /// name (`entity.<side>.<rest>`, as the spawn screen names it), health /
    /// max health, the relation the pick carries. The sim's sides are both
    /// armies, so no soldier is a team-0 target; an empty vehicle or an
    /// unowned building turret is (card HTGT). The sim has no shields, and
    /// the classes' `HideHealthBar` drop (§2: two BF2 classes) is not ported.
    pub fn target(&self, sim: &Sim, slot: usize) -> Option<HudTarget> {
        let Some((t, relation)) = sim.reticle_pick.get(slot).copied().flatten() else {
            return self
                .wreck_target(sim, slot)
                .or_else(|| self.station_target(sim, slot));
        };
        let fraction = |h: &core::weapon::Health| {
            if h.max > 0.0 {
                (h.cur / h.max).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        match t {
            AimTarget::Unit(h) => {
                let u = sim.unit(h)?;
                let name = self.classes[usize::from(u.team.min(1))]
                    .get(usize::from(u.spec.class))
                    .cloned()
                    .unwrap_or_default();
                Some(HudTarget {
                    id: (u64::from(h.gen) << 32) | u64::from(h.index),
                    name,
                    health_fraction: fraction(&u.health),
                    relation,
                    team0: false,
                    shield: None,
                })
            }
            AimTarget::Vehicle(h) => {
                let v = sim.vehicle(h)?;
                Some(HudTarget {
                    // Top bit: a vehicle's id never equals a unit's.
                    id: (1 << 63) | (u64::from(h.gen & 0x7fff_ffff) << 32) | u64::from(h.index),
                    name: self
                        .vehicles
                        .get(v.class as usize)
                        .cloned()
                        .unwrap_or_default(),
                    health_fraction: fraction(&v.health),
                    relation,
                    team0: v.team.is_none(),
                    shield: None,
                })
            }
            AimTarget::Droid(id) => {
                let d = sim.live_droid(id)?;
                let max = sim.droid_class(d)?.max_health;
                let name = sim
                    .weapon_classes
                    .get(d.weapon_class)
                    .and_then(|w| self.texts.weapon_name(&w.name))
                    .unwrap_or_else(|| self.texts.line(RECON_DROID));
                Some(HudTarget {
                    // Bit 59: a droid's id is no other kind's.
                    id: (1 << 59) | id,
                    name,
                    health_fraction: (d.health / max.max(1e-6)).clamp(0.0, 1.0),
                    relation,
                    team0: false,
                    shield: None,
                })
            }
            AimTarget::Turret(id) => {
                let t = sim.live_turret(id)?;
                let max = sim.turret_class(t)?.body.max_health;
                Some(HudTarget {
                    // Bit 60: a turret's id is no other kind's.
                    id: (1 << 60) | id,
                    name: self.texts.line(AUTO_TURRET),
                    health_fraction: (t.health / max.max(1e-6)).clamp(0.0, 1.0),
                    relation,
                    team0: false,
                    shield: None,
                })
            }
        }
    }

    /// A wreck the viewer's cutter is on - a fallen building turret or a
    /// destroyed droid (issue #35): its name and rebuild progress, shown as
    /// a live target's health (the originals show it the same way, the
    /// maintainer 2026-10-29). A fallen turret is neutral (team 0 on the
    /// PS2, `re-build-zones.md` §1).
    fn wreck_target(&self, sim: &Sim, slot: usize) -> Option<HudTarget> {
        let (i, me) = sim.units.iter().enumerate().find(|(_, u)| {
            u.alive
                && u.mount.is_none()
                && matches!(u.controller, core::unit::Controller::Player { slot: s } if usize::from(s) == slot)
        })?;
        match sim.cutter_target(i)? {
            Target::Station(si) => {
                let s = sim.stations.get(usize::from(si)).filter(|s| s.destroyed)?;
                Some(HudTarget {
                    id: (1 << 62) | u64::from(si),
                    name: self
                        .stations
                        .get(usize::from(s.class))
                        .cloned()
                        .unwrap_or_default(),
                    health_fraction: (s.health.cur / s.health.max.max(1e-6)).clamp(0.0, 1.0),
                    relation: match s.team {
                        None => 0,
                        Some(t) if t == me.team => 1,
                        Some(_) => -1,
                    },
                    team0: s.team.is_none(),
                    shield: None,
                })
            }
            Target::Building(bi) => {
                let t = sim.building_turrets.get(usize::from(bi))?;
                let h = t.down?;
                Some(HudTarget {
                    // Bit 61: a building's id is no unit's, vehicle's or station's.
                    id: (1 << 61) | u64::from(bi),
                    name: self
                        .vehicles
                        .get(t.spec.class as usize)
                        .cloned()
                        .unwrap_or_default(),
                    health_fraction: (h / t.spec.max_health.max(1e-6)).clamp(0.0, 1.0),
                    relation: 0,
                    team0: true,
                    shield: None,
                })
            }
            _ => None,
        }
    }

    /// A health or ammo droid under the reticle when the pick holds no unit
    /// or vehicle (the maintainer 2026-10-27: they showed no readout). Ours: the
    /// PS2's pick takes any object with health in the cone
    /// (`re-target-hud.md` §1), but the port's pick also drives the aim
    /// assist, so the stations join the readout only: the nearest live one
    /// whose sphere the reticle's ray passes within the weapon's
    /// ExtremeRange, in sight. Its relation is its team's (none: neutral,
    /// white).
    fn station_target(&self, sim: &Sim, slot: usize) -> Option<HudTarget> {
        let me = sim.units.iter().find(|u| {
            u.alive
                && u.mount.is_none()
                && matches!(u.controller, core::unit::Controller::Player { slot: s } if usize::from(s) == slot)
        })?;
        let (pos, dir) = me.aim_ray(sim.ground_rules, sim.views());
        let range = sim.weapon_classes.get(me.weapon_class)?.extreme_range;
        let mut best: Option<(f32, usize)> = None;
        for (i, s) in sim.stations.iter().enumerate() {
            if s.destroyed || s.health.max <= 0.0 {
                continue;
            }
            let (c, r) = (s.sphere_centre, s.sphere_radius.max(0.5));
            let along = (c - pos).dot(dir);
            if along <= 0.0 || along > range + r || best.is_some_and(|(b, _)| b <= along) {
                continue;
            }
            if (c - pos).length_squared() - along * along > r * r {
                continue;
            }
            let near = c - dir * r;
            if core::perception::los_seat(
                sim,
                pos,
                near,
                core::collision::HullFilter::All,
                [s.instance, None],
            ) {
                best = Some((along, i));
            }
        }
        let (_, i) = best?;
        let s = &sim.stations[i];
        Some(HudTarget {
            // Bit 62: a station's id is neither a unit's nor a vehicle's.
            id: (1 << 62) | i as u64,
            name: self
                .stations
                .get(usize::from(s.class))
                .cloned()
                .unwrap_or_default(),
            health_fraction: (s.health.cur / s.health.max).clamp(0.0, 1.0),
            relation: match s.team {
                None => 0,
                Some(t) if t == me.team => 1,
                Some(_) => -1,
            },
            team0: s.team.is_none(),
            shield: None,
        })
    }

    /// The viewer's statistics as BF2 publishes them (§5).
    pub fn stats(&self, sim: &Sim, viewer: UnitHandle) -> HudStats {
        HudStats {
            points: self.points,
            kills: self.kills,
            deaths: sim.units[viewer.index as usize].deaths as i32,
            hero_points: self
                .round
                .units
                .get(viewer.index as usize)
                .map_or(0, |u| u.hero_points),
            captures: self.battle.captures.min(i32::MAX as u32) as i32,
        }
    }

    /// Every soldier's round so far (`crate::round_stats`).
    pub fn round(&self) -> &crate::round_stats::RoundStats {
        &self.round
    }

    /// Team `team`'s soldier class `class`'s name, as the spawn screen's.
    pub fn class_name(&self, team: u8, class: u8) -> Option<&str> {
        self.classes[usize::from(team.min(1))]
            .get(usize::from(class))
            .map(String::as_str)
    }

    pub fn battle_stats(&self) -> crate::profile::BattleStats {
        crate::profile::BattleStats {
            points: self.points,
            ..self.battle
        }
    }
}

/// The chevron a hit on `viewer` lights (card HATK, `analysis/re-hud-attacker.md`
/// §3-4 [V][O]): the PS2 calls its trigger from the TakeDamage of the unit a
/// local player controls, alive at the call, before the damage is known — so
/// any hit, a killing one or one that takes nothing. The shot's way goes into
/// that unit's own (left, up, forward) rows: the soldier's facing, or the
/// vehicle he pilots (BF2's callers: the soldier, hover and walker; no
/// flyer [H], no building turret). A collision (no direction) lights none.
pub fn ambush_edge(sim: &Sim, viewer: UnitHandle, e: &Event) -> Option<Edge> {
    let me = sim.units.get(viewer.index as usize)?;
    let (dir, rot) = match *e {
        Event::Hit { victim, dir, .. } if victim == viewer => (
            dir,
            Quat::from_rotation_y(me.soldier.yaw + std::f32::consts::PI),
        ),
        Event::VehicleHit {
            vehicle,
            dir: Some(dir),
            ..
        } => {
            let m = me.mount.filter(|m| m.vehicle == vehicle)?;
            let v = sim.vehicle(vehicle)?;
            let piloted = v.seats.get(usize::from(m.seat)).map(|s| s.kind) == Some(SeatKind::Pilot);
            if !piloted || !matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker) {
                return None;
            }
            (dir, v.rot)
        }
        _ => return None,
    };
    // Both frames: left = +X, up = +Y, forward = +Z turned by `rot` (a
    // soldier's yaw 0 faces -Z, `render::Camera`).
    ui::hud_ambush::edge(rot.inverse() * dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cp::CpDef;
    use core::region::Region;
    use core::sim::{buttons, DeathCause, PadState};
    use core::unit::{Controller, UnitSpec};
    use glam::Vec3;

    /// BF2's own lines and names, as `bake locl` writes them.
    fn texts() -> Texts {
        let mut entries: Vec<(u32, String)> = [KILLED, TEAM_KILLED, SUICIDE, DIED, CAPTURED, LOST]
            .iter()
            .map(|(h, t)| (*h, t.to_string()))
            .collect();
        for (key, text) in [
            ("common.sides.all.name", "Rebels"),
            ("common.sides.imp.name", "Empire"),
            ("common.sides.all.firstnames.01", "Ben"),
            ("common.sides.all.lastnames.01", "Pulastra"),
            ("level.hoth.cp5", "Echo Base"),
            ("weapons.imp.weap.inf_rifle", "Blaster Rifle"),
            ("ifs.sideselect.chooseteam", "Select a team"),
            ("entity.all.inf_soldier", "Rebel Soldier"),
            ("entity.imp.inf_rifleman", "Stormtrooper"),
            ("entity.imp.hover_fightertank", "Fighter Tank"),
        ] {
            entries.push((manifest::hud::locl_key(key), text.to_string()));
        }
        entries.sort_by_key(|(h, _)| *h);
        let table = HudLocl {
            language: "english".into(),
            entries,
        };
        Texts {
            bf2: Some(table.clone()),
            map: Some(table),
            bf1_map: false,
        }
    }

    /// The Empire's player (team 1) with an AI beside him, two Rebel AI
    /// (team 0), and a post the Empire holds.
    struct Scene {
        sim: Sim,
        me: UnitHandle,
        mate: UnitHandle,
        rebels: [UnitHandle; 2],
        feed: EventFeed,
    }

    fn scene() -> Scene {
        let mut sim = Sim::empty(3);
        sim.heightfield = Some(core::heightfield::Heightfield {
            size: 2,
            unit: 400.0,
            offset_x: 200.0,
            offset_z: 200.0,
            heights: vec![0.0; 4],
        });
        let me = sim
            .spawn(&UnitSpec::player_in_slot(0, 1, Vec3::ZERO))
            .expect("player");
        let ai = |sim: &mut Sim, team: u8, x: f32| {
            sim.spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(team, Vec3::new(x, 0.0, 0.0))
            })
            .expect("bot")
        };
        let mate = ai(&mut sim, 1, 2.0);
        let rebels = [ai(&mut sim, 0, 40.0), ai(&mut sim, 0, 42.0)];
        let (capture_time, neutralize_time) = CpDef::class_times(true);
        sim.add_cp(
            CpDef {
                name: "cp5".into(),
                pos: Vec3::new(100.0, 0.0, 0.0),
                capture: Region::sphere(Vec3::new(100.0, 0.0, 0.0), 8.0),
                control: None,
                value_bleed: 10,
                value_atk: [10; 2],
                value_def: [10; 2],
                spawn_nodes: vec![(Vec3::new(100.0, 0.0, 0.0), 0.0)],
                capture_time,
                neutralize_time,
            },
            Some(1),
        );
        let feed = EventFeed::new(texts(), ["all", "imp"], &["level.hoth.cp5".to_string()]);
        Scene {
            sim,
            me,
            mate,
            rebels,
            feed,
        }
    }

    /// One tick whose events are exactly `events`.
    fn happen(s: &mut Scene, events: &[Event]) -> Vec<HudMessage> {
        s.sim.tick += 1;
        s.sim.events = events.to_vec();
        s.feed.tick(&s.sim, s.me);
        s.feed.take()
    }

    fn line(text: &str, color: [u8; 3]) -> HudMessage {
        HudMessage {
            text: text.into(),
            color,
        }
    }

    /// Card HTGT (`re-target-hud.md` §3-4): the reticle's pick by its
    /// class's Locl name and health — a Rebel at 300 then 150 of 300 reads
    /// 1.0 then 0.5, enemy; a team-mate a friend; no pick, no target.
    #[test]
    fn the_target_is_the_reticles_pick_by_class_name() {
        let mut s = scene();
        s.feed.classes = [
            vec![s
                .feed
                .texts
                .class_name("all_inf_soldier")
                .expect("entity key")],
            vec![s
                .feed
                .texts
                .class_name("imp_inf_rifleman")
                .expect("entity key")],
        ];
        let [r0, _] = s.rebels;
        s.sim.reticle_pick.resize(1, None);
        assert_eq!(s.feed.target(&s.sim, 0), None);
        let u = &mut s.sim.units[r0.index as usize];
        u.health.max = 300.0;
        u.health.cur = 300.0;
        s.sim.reticle_pick[0] = Some((AimTarget::Unit(r0), -1));
        let t = s.feed.target(&s.sim, 0).expect("picked");
        assert_eq!((t.name.as_str(), t.health_fraction), ("Rebel Soldier", 1.0));
        assert_eq!(t.color(), ui::hud::target_color::ENEMY);
        s.sim.units[r0.index as usize].health.cur = 150.0;
        assert_eq!(s.feed.target(&s.sim, 0).unwrap().health_fraction, 0.5);
        s.sim.reticle_pick[0] = Some((AimTarget::Unit(s.mate), 1));
        let t = s.feed.target(&s.sim, 0).unwrap();
        assert_eq!(t.name, "Stormtrooper");
        assert_eq!(t.color(), ui::hud::target_color::FRIEND);
        s.sim.reticle_pick[0] = Some((AimTarget::Unit(r0), -1));
        assert_ne!(
            t.id,
            s.feed.target(&s.sim, 0).unwrap().id,
            "one id per unit"
        );
    }

    /// A health droid under the reticle with no unit or vehicle picked reads
    /// its class's name and health, neutral white; a nearer pick wins, a
    /// destroyed droid or one out of the ray shows nothing.
    #[test]
    fn a_station_under_the_reticle_reads_its_health() {
        use core::sim::Station;
        let mut s = scene();
        s.sim.reticle_pick.resize(1, None);
        let me = s.sim.units.iter().find(|u| u.is_player).expect("player");
        let (eye, dir) = me.aim_ray(s.sim.ground_rules, s.sim.views());
        // Every station class has a name in the baked text, on both games'
        // maps ("Medical Droid", "Ammunition Droid", "Machinery Repair Droid").
        let root = crate::assets::baked_root();
        if root.join("common_bf2/locl/english.ron").exists() {
            for game in ["bf1", "bf2"] {
                let texts = Texts::load(&root, game);
                for c in [
                    "com_item_healthrecharge",
                    "com_item_weaponrecharge",
                    "com_item_vehiclerecharge",
                ] {
                    let name = texts.class_name(c);
                    println!("{game} {c}: {name:?}");
                    assert!(name.is_some(), "{game} {c}: no entity name");
                }
            }
        }
        s.feed.stations = vec!["Health Droid".into()];
        let mut st = Station::new(0, eye + dir * 10.0, 2.0);
        (st.sphere_centre, st.sphere_radius) = (eye + dir * 10.0, 0.8);
        st.health.cur = 40.0;
        s.sim.stations.push(st);
        let t = s.feed.target(&s.sim, 0).expect("the droid");
        assert_eq!(
            (t.name.as_str(), t.health_fraction, t.relation, t.team0),
            ("Health Droid", 0.5, 0, true)
        );
        assert_eq!(t.color(), ui::hud::target_color::NONE);
        // Off the ray: nothing.
        let side = dir.cross(glam::Vec3::Y).normalize() * 3.0;
        s.sim.stations[0].sphere_centre += side;
        assert_eq!(s.feed.target(&s.sim, 0), None);
        s.sim.stations[0].sphere_centre -= side;
        // A unit or vehicle pick comes first.
        let [r0, _] = s.rebels;
        s.sim.reticle_pick[0] = Some((AimTarget::Unit(r0), -1));
        assert_ne!(s.feed.target(&s.sim, 0).unwrap().id, t.id);
        s.sim.reticle_pick[0] = None;
        // A destroyed droid shows nothing with the rifle up; under a cutter it
        // reads its rebuild progress (`a_wreck_under_the_cutter_...`).
        s.sim.stations[0].destroyed = true;
        assert_eq!(s.feed.target(&s.sim, 0), None);
    }

    /// Issue #35: with the cutter up, a destroyed droid or a fallen turret
    /// it is on reads its name and rebuild progress like a live target's
    /// health; with the rifle up, or off the cutter, a wreck shows nothing.
    #[test]
    fn a_wreck_under_the_cutter_reads_its_rebuild_progress() {
        use core::sim::{BuildingTurret, Station, TurretFall};
        use core::vehicle::{VehicleHandle, VehicleSpec};
        use core::weapon::{HealthType, Repair, WeaponClass};
        let mut s = scene();
        s.sim.reticle_pick.resize(1, None);
        let mut health = [0.0; 8];
        (health[3], health[4], health[5], health[6]) = (200.0, 100.0, 50.0, 50.0);
        s.sim.weapon_classes.push(WeaponClass {
            name: "fusioncutter".into(),
            repair: Some(Repair { health }),
            target_team: 6,
            lock_on_range: 4.0,
            lock_on_angle: 90.0 * core::weapon::DEG,
            shot_delay: [0.5; 2],
            ..WeaponClass::default()
        });
        let cutter = s.sim.weapon_classes.len() - 1;
        let rifle = s.sim.unit(s.me).unwrap().weapon_class;
        let me = s.sim.unit(s.me).unwrap();
        let (eye, dir) = me.aim_ray(s.sim.ground_rules, s.sim.views());
        let at = eye + dir * 2.0;
        s.feed.stations = vec!["Health Droid".into()];
        let mut st = Station::new(0, at, 2.0);
        (st.sphere_centre, st.sphere_radius) = (at, 0.8);
        st.health.health_type = HealthType::Building;
        st.health.cur = st.health.max * 0.75;
        st.destroyed = true;
        s.sim.stations.push(st);
        // Rifle up: the wreck shows nothing (as before).
        assert_eq!(s.feed.target(&s.sim, 0), None);
        s.sim.unit_mut(s.me).unwrap().weapon_class = cutter;
        let t = s
            .feed
            .target(&s.sim, 0)
            .expect("the wreck under the cutter");
        assert_eq!(
            (t.name.as_str(), t.health_fraction, t.relation, t.team0),
            ("Health Droid", 0.75, 0, true)
        );
        // Off the cutter's cone: nothing.
        s.sim.stations[0].sphere_centre = eye - dir * 3.0;
        assert_eq!(s.feed.target(&s.sim, 0), None);
        s.sim.stations.clear();
        // A fallen turret on its pad: its class's name and health / MaxHealth.
        s.feed.vehicles = vec!["Laser Turret".into()];
        s.sim.building_turrets.push(BuildingTurret {
            spec: VehicleSpec {
                kind: VehicleKind::Turret,
                class: 0,
                home_team: None,
                neutral_when_empty: true,
                spawn_pad: None,
                pos: at,
                yaw: 0.0,
                max_health: 800.0,
                health_type: HealthType::Building,
                seats: vec![SeatKind::Gunner(0)],
            },
            rot: Quat::IDENTITY,
            aim_sphere: Some((glam::Vec3::ZERO, 0.8)),
            instance: 99,
            handle: VehicleHandle { index: 0, gen: 0 },
            fall: TurretFall::Pad,
            fallen_parts: Vec::new(),
            standing_parts: Vec::new(),
            down: Some(200.0),
            posed: None,
        });
        let t = s.feed.target(&s.sim, 0).expect("the fallen turret");
        assert_eq!(
            (t.name.as_str(), t.health_fraction, t.relation, t.team0),
            ("Laser Turret", 0.25, 0, true)
        );
        assert_eq!(t.color(), ui::hud::target_color::NONE);
        s.sim.unit_mut(s.me).unwrap().weapon_class = rifle;
        assert_eq!(s.feed.target(&s.sim, 0), None);
    }

    /// Card HTGT slice 3: a picked vehicle reads its class's Locl name and
    /// its hull's health; empty it is a team-0 target (white), crewed by the
    /// other team an enemy; its id is not a unit's.
    #[test]
    fn a_picked_vehicle_reads_its_class_name_and_hull() {
        use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
        let mut s = scene();
        s.feed.vehicles = vec![s
            .feed
            .texts
            .class_name("imp_hover_fightertank")
            .expect("entity key")];
        let v = s.sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(0.0, 0.0, -20.0),
            yaw: 0.0,
            max_health: 1000.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        s.sim.reticle_pick.resize(1, None);
        s.sim.reticle_pick[0] = Some((AimTarget::Vehicle(v), 0));
        let t = s.feed.target(&s.sim, 0).expect("picked");
        assert_eq!((t.name.as_str(), t.health_fraction), ("Fighter Tank", 1.0));
        assert!(t.team0);
        assert_eq!(t.color(), ui::hud::target_color::NONE);
        let veh = &mut s.sim.vehicles[v.index as usize];
        veh.health.cur = 250.0;
        veh.team = Some(1);
        s.sim.reticle_pick[0] = Some((AimTarget::Vehicle(v), -1));
        let t = s.feed.target(&s.sim, 0).unwrap();
        assert_eq!(t.health_fraction, 0.25);
        assert!(!t.team0);
        assert_eq!(t.color(), ui::hud::target_color::ENEMY);
        let unit = s.sim.handle(v.index as usize);
        s.sim.reticle_pick[0] = Some((AimTarget::Unit(unit), -1));
        if let Some(u) = s.feed.target(&s.sim, 0) {
            assert_ne!(u.id, t.id, "a vehicle's id is not its index's unit's");
        }
        s.sim.vehicles[v.index as usize].alive = false;
        s.sim.reticle_pick[0] = Some((AimTarget::Vehicle(v), -1));
        assert_eq!(s.feed.target(&s.sim, 0), None, "a wreck reads nothing");
    }

    /// Card HATK (`re-hud-attacker.md` §3): a hit on the player lights the
    /// edge toward the shooter in HIS frame - a Rebel at +X shooting him
    /// (the shot travelling -X, a little down) is on his right facing -Z,
    /// behind him facing -X, on his left facing +Z, and in front of him
    /// (nothing) facing +X; another unit's hit lights nothing.
    #[test]
    fn a_hit_on_the_player_lights_the_edge_toward_the_shooter() {
        let mut s = scene();
        let (me, mate, rebel) = (s.me, s.mate, s.rebels[0]);
        let from = s.sim.units[rebel.index as usize].soldier.pos + Vec3::Y * 1.6;
        let to = s.sim.units[me.index as usize].soldier.pos + Vec3::Y * 1.0;
        let hit = |victim| Event::Hit {
            shooter: rebel,
            victim,
            damage: 10.0,
            age: 0.5,
            health_left: 100.0,
            dir: (to - from).normalize(),
        };
        let edges = |s: &mut Scene, yaw: f32, victim| {
            s.sim.units[me.index as usize].soldier.yaw = yaw;
            happen(s, &[hit(victim)]);
            s.feed.take_ambush()
        };
        use std::f32::consts::{FRAC_PI_2, PI};
        // Yaw 0 faces -Z (positive yaw turns left).
        assert_eq!(edges(&mut s, 0.0, me), [Edge::Right]);
        assert_eq!(edges(&mut s, FRAC_PI_2, me), [Edge::Bottom]);
        assert_eq!(edges(&mut s, PI, me), [Edge::Left]);
        assert_eq!(edges(&mut s, -FRAC_PI_2, me), []);
        assert_eq!(edges(&mut s, 0.0, mate), []);
        assert!(s.feed.take_ambush().is_empty(), "taken once");
    }

    /// Hitmark (`re-target-hud.md` §12): the player's critical hit with a
    /// sniper rifle lights the mark for the PS2's 0.5 s hit timer, in the
    /// victim's relation colour; a rifle's critical hit, another unit's, or
    /// a plain body hit lights nothing.
    /// [ours] (the maintainer, 2026-10-29): every hit the viewer deals an
    /// enemy soldier, any weapon, blasts too, lights the plain hit flash
    /// for the 0.5 s hit timer; a team-mate, a bot's hit and a hit taken
    /// do not.
    #[test]
    fn an_enemy_body_hit_lights_the_plain_flash() {
        let mut s = scene();
        let (me, mate, rebel) = (s.me, s.mate, s.rebels[0]);
        let hit = |shooter, victim, age| Event::Hit {
            shooter,
            victim,
            damage: 50.0,
            age,
            health_left: 250.0,
            dir: Vec3::X,
        };
        happen(&mut s, &[hit(me, mate, 0.1)]);
        assert_eq!(s.feed.hit_body(s.sim.tick), None, "a team-mate");
        happen(&mut s, &[hit(rebel, me, 0.1)]);
        assert_eq!(s.feed.hit_body(s.sim.tick), None, "a hit taken");
        happen(&mut s, &[hit(rebel, mate, 0.1)]);
        assert_eq!(s.feed.hit_body(s.sim.tick), None, "a bot's hit");
        happen(&mut s, &[hit(me, rebel, 0.1)]);
        let at = s.sim.tick;
        assert_eq!(s.feed.hit_body(at), Some(ui::hud::target_color::ENEMY));
        assert!(s.feed.hit_body(at + HIT_TIMER_TICKS - 1).is_some());
        assert_eq!(s.feed.hit_body(at + HIT_TIMER_TICKS), None, "0.5 s");
        happen(&mut s, &[hit(me, rebel, core::sim::BLAST_AGE)]);
        assert!(s.feed.hit_body(s.sim.tick).is_some(), "a blast");
    }

    #[test]
    fn a_sniper_headshot_lights_the_critical_mark() {
        let mut s = scene();
        let (me, mate, rebel) = (s.me, s.mate, s.rebels[0]);
        let sniper = core::weapon::WeaponClass {
            yaw_spread: 0.0,
            trigger_single: true,
            zoom_max: 6.0,
            ..core::weapon::WeaponClass::bf2_rifle()
        };
        assert!(!core::weapon::WeaponClass::bf2_rifle().ai_distance_table());
        s.sim.weapon_classes = vec![core::weapon::WeaponClass::bf2_rifle(), sniper];
        let (rifle, sniper) = (0, 1);
        let crit = |shooter, victim, weapon_class| Event::CriticalHit {
            shooter,
            victim: Target::Unit(victim),
            weapon_class,
        };
        let body = Event::Hit {
            shooter: me,
            victim: rebel,
            damage: 50.0,
            age: 0.1,
            health_left: 250.0,
            dir: Vec3::X,
        };
        happen(&mut s, &[body]);
        assert_eq!(s.feed.hit_critical(s.sim.tick), None, "a body hit");
        happen(&mut s, &[body, crit(me, rebel, rifle)]);
        assert_eq!(s.feed.hit_critical(s.sim.tick), None, "a rifle headshot");
        happen(&mut s, &[crit(rebel, me, sniper)]);
        assert_eq!(s.feed.hit_critical(s.sim.tick), None, "a bot's headshot");
        happen(&mut s, &[body, crit(me, rebel, sniper)]);
        let at = s.sim.tick;
        assert_eq!(
            s.feed.hit_critical(at),
            Some(ui::hud::target_color::ENEMY),
            "a sniper headshot on an enemy"
        );
        assert!(s.feed.hit_critical(at + HIT_TIMER_TICKS - 1).is_some());
        assert_eq!(s.feed.hit_critical(at + HIT_TIMER_TICKS), None, "0.5 s");
        happen(&mut s, &[crit(me, mate, sniper)]);
        assert_eq!(
            s.feed.hit_critical(s.sim.tick),
            Some(ui::hud::target_color::FRIEND),
            "a team-mate's head: blue"
        );
    }

    /// Hitmark: the player's rocket launcher on a vehicle's weak spot lights
    /// the mark in the vehicle's relation colour (white for an empty one);
    /// a sniper rifle or a rifle on the weak spot, the EMP launcher, a
    /// rocket launcher on a soldier's head, or a bot's rocket on the weak
    /// spot lights nothing.
    #[test]
    fn a_rocket_on_a_weak_spot_lights_the_critical_mark() {
        use core::vehicle::{VehicleKind, VehicleSpec};
        use core::weapon::WeaponClass;
        let mut s = scene();
        let (me, rebel) = (s.me, s.rebels[0]);
        let my_team = s.sim.units[me.index as usize].team;
        let spec = |team| VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: Some(team),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: 1000.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![core::vehicle::SeatKind::Pilot],
        };
        let theirs = s.sim.spawn_vehicle(&spec(1 - my_team));
        let ours = s.sim.spawn_vehicle(&spec(my_team));
        let empty = s.sim.spawn_vehicle(&VehicleSpec {
            neutral_when_empty: true,
            ..spec(my_team)
        });
        let named = |name: &str, medals| WeaponClass {
            name: name.into(),
            medals_type: medals,
            ..WeaponClass::bf2_rifle()
        };
        let sniper = WeaponClass {
            yaw_spread: 0.0,
            trigger_single: true,
            zoom_max: 6.0,
            ..named("com_weap_inf_sniper_rifle", Some(4))
        };
        s.sim.weapon_classes = vec![
            named("com_weap_inf_rifle", Some(1)),
            sniper,
            named("rep_weap_inf_rocket_launcher", Some(2)),
            named("rep_weap_inf_emp_launcher", Some(-1)),
            named("all_weap_inf_torpedo_launcher", None),
        ];
        let (rifle, sniper, rocket, emp, bf1_rocket) = (0, 1, 2, 3, 4);
        let crit = |shooter, victim, weapon_class| Event::CriticalHit {
            shooter,
            victim,
            weapon_class,
        };
        let mark = |s: &mut Scene, e: Event| {
            s.sim.tick += HIT_TIMER_TICKS;
            happen(s, &[e]);
            s.feed.hit_critical(s.sim.tick)
        };
        let enemy = Some(ui::hud::target_color::ENEMY);
        let v = Target::Vehicle(theirs);
        assert_eq!(mark(&mut s, crit(me, v, rocket)), enemy, "a rocket");
        assert_eq!(mark(&mut s, crit(me, v, bf1_rocket)), enemy, "BF1's");
        assert_eq!(
            mark(&mut s, crit(me, Target::Vehicle(ours), rocket)),
            Some(ui::hud::target_color::FRIEND),
            "our own side's: blue"
        );
        assert_eq!(
            mark(&mut s, crit(me, Target::Vehicle(empty), rocket)),
            Some(ui::hud::target_color::NONE),
            "an empty one, of no side: white"
        );
        assert_eq!(mark(&mut s, crit(me, v, rifle)), None, "a rifle");
        assert_eq!(mark(&mut s, crit(me, v, sniper)), None, "a sniper rifle");
        assert_eq!(mark(&mut s, crit(me, v, emp)), None, "the EMP launcher");
        assert_eq!(mark(&mut s, crit(rebel, v, rocket)), None, "a bot's");
        assert_eq!(
            mark(&mut s, crit(me, Target::Unit(rebel), rocket)),
            None,
            "a rocket on a head"
        );
    }

    /// A hit on the hover he pilots lights the edge in the HULL's frame (its
    /// left +X, forward +Z); a gunner's seat, a collision, or a flyer light
    /// none (BF2's callers: soldier, hover, walker).
    #[test]
    fn a_hit_on_his_hover_lights_the_hulls_edge() {
        use core::vehicle::{Mount, SeatKind, VehicleKind, VehicleSpec};
        let mut s = scene();
        let me = s.me;
        let spec = |kind, yaw| VehicleSpec {
            kind,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw,
            max_health: 1000.0,
            health_type: core::weapon::HealthType::Vehicle,
            // A building turret has gunner seats only.
            seats: if kind == VehicleKind::Turret {
                vec![SeatKind::Gunner(0)]
            } else {
                vec![SeatKind::Pilot, SeatKind::Gunner(0)]
            },
        };
        let mut edges = |kind, yaw, seat, dir: Option<Vec3>| {
            let v = s.sim.spawn_vehicle(&spec(kind, yaw));
            s.sim.units[me.index as usize].mount = Some(Mount { vehicle: v, seat });
            // The soldier's own facing must not matter.
            s.sim.units[me.index as usize].soldier.yaw = 1.0;
            let rot = s.sim.vehicle(v).unwrap().rot;
            let ev = Event::VehicleHit {
                shooter: Some(s.rebels[0]),
                vehicle: v,
                damage: 10.0,
                dir: dir.map(|d| rot * d),
            };
            happen(&mut s, &[ev]);
            s.feed.take_ambush()
        };
        // Travelling toward the hull's right (-X): from its left.
        let from_left = Some(Vec3::new(-1.0, -0.1, 0.0).normalize());
        assert_eq!(edges(VehicleKind::Hover, 0.0, 0, from_left), [Edge::Left]);
        assert_eq!(edges(VehicleKind::Walker, 2.0, 0, from_left), [Edge::Left]);
        let from_behind = Some(Vec3::new(0.0, -0.1, 1.0).normalize());
        assert_eq!(
            edges(VehicleKind::Hover, 1.0, 0, from_behind),
            [Edge::Bottom]
        );
        assert_eq!(edges(VehicleKind::Hover, 0.0, 1, from_left), []);
        assert_eq!(edges(VehicleKind::Hover, 0.0, 0, None), []);
        assert_eq!(edges(VehicleKind::Flyer, 0.0, 0, from_left), []);
        assert_eq!(edges(VehicleKind::Turret, 0.0, 0, from_left), []);
    }

    #[test]
    fn hero_victim_uses_disc_points_and_hero_message_even_after_death() {
        let mut s = scene();
        let victim = s.rebels[0];
        s.sim.heroes = Some(core::sim::heroes::Heroes::new([Some(0), None]));
        s.feed.classes[0] = vec!["Luke Skywalker".into()];
        s.sim.units[victim.index as usize].alive = false;
        let me = s.me;
        assert_eq!(
            happen(
                &mut s,
                &[Event::Kill {
                    shooter: me,
                    victim
                }]
            ),
            [line("Player 1 defeated Luke Skywalker", [200, 100, 100])]
        );
        assert_eq!(s.feed.stats(&s.sim, me).points, points::KILL_HERO);
        assert_eq!(
            s.feed.round().units[me.index as usize].points,
            points::KILL_HERO
        );
    }

    #[test]
    fn hero_earned_points_are_displayed_separately_and_do_not_rank_future_offers() {
        let mut s = scene();
        s.sim.heroes = Some(core::sim::heroes::Heroes::new([None, Some(0)]));
        let me = s.me;
        let victim = s.rebels[0];
        happen(
            &mut s,
            &[Event::Kill {
                shooter: me,
                victim,
            }],
        );
        let stats = s.feed.stats(&s.sim, me);
        assert_eq!((stats.points, stats.hero_points), (1, 1));
        assert_eq!(s.feed.round().units[me.index as usize].ordinary_points(), 0);
        crate::heroes::scores(&mut s.sim, &s.feed);
        assert_eq!(s.sim.heroes.as_ref().unwrap().points[me.index as usize], 0);
    }

    /// Card EVMSG's gate (`re-event-messages.md` §1.1): only a human's death
    /// makes a line, coloured by the victim's team as the viewer sees it;
    /// the player's own kill is his statistics.
    #[test]
    fn only_a_humans_death_makes_a_line() {
        let mut s = scene();
        let [r0, r1] = s.rebels;
        let (me, mate) = (s.me, s.mate);
        // AI kills AI, either way round: nothing.
        let kill = |shooter, victim| Event::Kill { shooter, victim };
        assert!(happen(&mut s, &[kill(r0, mate), kill(mate, r1)]).is_empty());
        assert_eq!(s.feed.stats(&s.sim, me), HudStats::default());

        // An AI kills the human: the line, in the viewer's own-team colour.
        assert_eq!(
            happen(&mut s, &[kill(r0, me)]),
            [line("Ben Pulastra killed Player 1", [1, 86, 213])]
        );
        // The Empire has no name lists: two letters and a number.
        let name = s.feed.unit_name(&s.sim, mate);
        let b = name.as_bytes();
        assert!(
            name.len() == 6
                && b[..2].iter().all(u8::is_ascii_uppercase)
                && b[2] == b' '
                && b[3..].iter().all(u8::is_ascii_digit),
            "{name:?}"
        );

        // The human kills an ordinary AI: no line, kills +1 and a point.
        assert!(happen(&mut s, &[kill(me, r1)]).is_empty());
        let st = s.feed.stats(&s.sim, me);
        assert_eq!((st.kills, st.points), (1, points::KILL_AI));

        // A human suicide: the line, kills −1, a point lost.
        assert_eq!(
            happen(&mut s, &[kill(me, me)]),
            [line("Player 1 killed himself", [1, 86, 213])]
        );
        let st = s.feed.stats(&s.sim, me);
        assert_eq!((st.kills, st.points), (0, 0));

        // A teammate killed by the human: no line (the victim is an AI),
        // kills −1, two points lost.
        assert!(happen(&mut s, &[kill(me, mate)]).is_empty());
        let st = s.feed.stats(&s.sim, me);
        assert_eq!((st.kills, st.points), (-1, -2));

        // Dying with no killer: "died", nobody's grey, a point lost.
        let fell = Event::EnvDeath {
            unit: me,
            cause: DeathCause::Fall,
        };
        assert_eq!(
            happen(&mut s, &[fell]),
            [line("Player 1 died", [192, 192, 192])]
        );
        assert_eq!(s.feed.stats(&s.sim, me).points, -3);
        // An AI's fall: nothing.
        let fell = Event::EnvDeath {
            unit: r0,
            cause: DeathCause::Fall,
        };
        assert!(happen(&mut s, &[fell]).is_empty());
    }

    /// `[ours]` post lines in BF2's own strings, coloured by the team that
    /// captured or lost the post; standing in the region longest earns the
    /// capture's 5 points, standing in it at all its 2.
    #[test]
    fn post_lines_name_the_team_and_the_post() {
        let mut s = scene();
        let me = s.me;
        // The Rebels take the Empire's post: lost (the viewer's team, blue),
        // then captured (the enemy, red).
        assert_eq!(
            happen(&mut s, &[Event::Neutralized { cp: 0, by: 0 }]),
            [line("Empire lost the Echo Base", [1, 86, 213])]
        );
        s.sim.cps[0].owner = None;
        assert_eq!(
            happen(&mut s, &[Event::Captured { cp: 0, team: 0 }]),
            [line("Rebels captured the Echo Base", [200, 100, 100])]
        );
        assert_eq!(s.feed.stats(&s.sim, me).captures, 0);

        // The Empire takes it back with the player in the region first and
        // a teammate arriving later: his capture, 5 points.
        s.sim.cps[0].owner = Some(0);
        let at_post = Vec3::new(100.0, 0.0, 0.0);
        s.sim.units[me.index as usize].soldier.pos = at_post;
        assert!(happen(&mut s, &[]).is_empty());
        let mate = s.mate;
        s.sim.units[mate.index as usize].soldier.pos = at_post;
        assert_eq!(
            happen(&mut s, &[Event::Neutralized { cp: 0, by: 1 }]),
            [line("Rebels lost the Echo Base", [200, 100, 100])]
        );
        assert_eq!(
            happen(&mut s, &[Event::Captured { cp: 0, team: 1 }]),
            [line("Empire captured the Echo Base", [1, 86, 213])]
        );
        let st = s.feed.stats(&s.sim, me);
        assert_eq!((st.captures, st.points), (1, points::CAPTURE));

        // Arriving second is an assist.
        s.sim.units[me.index as usize].soldier.pos = Vec3::ZERO;
        assert!(happen(&mut s, &[]).is_empty());
        s.sim.units[me.index as usize].soldier.pos = at_post;
        for _ in 0..SAME_LINE_TICKS {
            assert!(happen(&mut s, &[]).is_empty());
        }
        happen(&mut s, &[Event::Captured { cp: 0, team: 1 }]);
        let st = s.feed.stats(&s.sim, me);
        assert_eq!(
            (st.captures, st.points),
            (2, points::CAPTURE + points::CAPTURE_ASSIST)
        );
    }

    /// CAPCR: being counted on the completing tick still earns an assist,
    /// but full credit requires positive residence (re-event-messages §9.1).
    #[test]
    fn capture_arrival_on_completing_tick_is_an_assist() {
        let mut s = scene();
        let me = s.me;
        s.sim.units[me.index as usize].soldier.pos = s.sim.cps[0].def.pos;
        happen(&mut s, &[Event::Captured { cp: 0, team: 1 }]);
        let stats = s.feed.stats(&s.sim, me);
        assert_eq!((stats.captures, stats.points), (1, points::CAPTURE_ASSIST));
        s.feed.tick(&s.sim, me);
        assert!(s.feed.take().is_empty());
        assert_eq!(s.feed.stats(&s.sim, me), stats);
    }

    #[test]
    fn capture_prior_tick_residence_retains_full_credit_and_ties() {
        // No teammate present, or a teammate entering on the same prior tick:
        // neither has longer residence than the viewer.
        for tied_teammate in [false, true] {
            let mut s = scene();
            let me = s.me;
            let at_post = s.sim.cps[0].def.pos;
            s.sim.units[me.index as usize].soldier.pos = at_post;
            if tied_teammate {
                s.sim.units[s.mate.index as usize].soldier.pos = at_post;
            }
            happen(&mut s, &[]);
            happen(&mut s, &[Event::Captured { cp: 0, team: 1 }]);
            let stats = s.feed.stats(&s.sim, me);
            assert_eq!((stats.captures, stats.points), (1, points::CAPTURE));
        }
    }

    #[test]
    fn capture_sim_step_arrival_on_completing_tick_is_an_assist() {
        let mut s = scene();
        let (me, mate) = (s.me, s.mate);
        for unit in &mut s.sim.units {
            unit.controller = Controller::Idle;
        }
        s.sim.cps[0].owner = None;
        let at_post = s.sim.cps[0].def.pos;
        s.sim.units[mate.index as usize].soldier.pos = at_post;
        let limit = s.sim.cps[0].def.capture_time;
        for _ in 0..1000 {
            if s.sim.cps[0].capture + core::TICK_DT > limit {
                break;
            }
            s.sim.step(&[]);
            s.feed.tick(&s.sim, me);
            s.feed.take();
            assert_eq!(s.sim.cps[0].owner, None);
        }
        assert!(s.sim.cps[0].capture + core::TICK_DT > limit);
        s.sim.units[mate.index as usize].soldier.pos = Vec3::ZERO;
        s.sim.units[me.index as usize].soldier.pos = at_post;
        s.sim.step(&[]);
        assert!(s
            .sim
            .events
            .iter()
            .any(|event| matches!(event, Event::Captured { cp: 0, team: 1 })));
        s.feed.tick(&s.sim, me);
        let stats = s.feed.stats(&s.sim, me);
        assert_eq!((stats.captures, stats.points), (1, points::CAPTURE_ASSIST));
        s.feed.tick(&s.sim, me);
        assert_eq!(s.feed.stats(&s.sim, me), stats);
    }

    /// A post with no name on the disc is the PS2's "Command Post", and a
    /// BF1 name is set in BF2's case.
    #[test]
    fn post_and_team_names_fall_back() {
        let t = texts();
        assert_eq!(t.post_name("level.hoth.cp5"), "Echo Base");
        assert_eq!(t.post_name("level.hoth.cp9"), "Command Post");
        assert_eq!(t.team_name("gun"), "GUN");
        assert_eq!(title_case("PIER ONE"), "Pier One");
        assert_eq!(title_case("WESTERN LZ"), "Western LZ");
        assert_eq!(
            fill("%s lost the %s", &["Empire", "Bunker"]),
            "Empire lost the Bunker"
        );
        // A side's lists from its keys; a side with none composes letters
        // and digits.
        let (first, last) = t.name_lists("all");
        assert_eq!((first, last), (vec!["Ben".into()], vec!["Pulastra".into()]));
        let feed = EventFeed::new(t.clone(), ["imp", "all"], &[]);
        assert_eq!(feed.teams[0].1[..2], ["DX 178", "KL 987"]);
        assert_eq!(feed.teams[1].1[0], "Ben Pulastra");
    }

    fn pad(buttons: u32) -> PadState {
        PadState {
            buttons,
            ..PadState::default()
        }
    }

    /// A battle of the player and `mates` bots of his team in a 5 m ring.
    fn squad_scene(mates: usize) -> (Sim, UnitHandle, EventFeed) {
        let mut s = scene();
        for h in s.rebels {
            s.sim.units[h.index as usize].alive = false;
        }
        s.sim.units[s.mate.index as usize].alive = false;
        for k in 0..mates {
            let a = k as f32 / mates as f32 * std::f32::consts::TAU;
            s.sim
                .spawn(&UnitSpec {
                    controller: Controller::Ai,
                    ..UnitSpec::trooper(1, Vec3::new(5.0 * a.cos(), 0.0, 5.0 * a.sin()))
                })
                .expect("bot");
        }
        (s.sim, s.me, s.feed)
    }

    /// Press `button`, run a second of ticks (every bot has thought), and
    /// answer the cross then.
    fn order(sim: &mut Sim, me: UnitHandle, feed: &mut EventFeed, button: u32) -> SquadCross {
        sim.step(&[pad(0)]);
        feed.tick(sim, me);
        sim.step(&[pad(button)]);
        feed.tick(sim, me);
        for _ in 0..core::TICK_HZ {
            sim.step(&[pad(0)]);
            feed.tick(sim, me);
        }
        assert!(feed.take().is_empty(), "an order prints no line");
        feed.squad(sim.tick).expect("the cross is up a second on")
    }

    /// Card SQHUD: the press shows the cross — its arm, the PS2's 2 s timer
    /// on the sim's clock — and prints no line, however many render frames
    /// look at the tick.
    #[test]
    fn an_order_shows_the_cross_and_no_line() {
        let (mut sim, me, mut feed) = squad_scene(0);
        feed.tick(&sim, me);
        assert!(feed.squad(sim.tick).is_none());
        sim.step(&[pad(buttons::SQUAD_FOLLOW)]);
        for _ in 0..3 {
            feed.tick(&sim, me);
        }
        assert!(feed.take().is_empty());
        let at = sim.tick;
        let hz = u64::from(core::TICK_HZ);
        let c = feed.squad(at).expect("shown at the press");
        assert_eq!((c.arm, c.timer, c.count), (hud_squad::RIGHT, 1.0, None));
        assert_eq!(
            c.labels,
            ["MOVE OUT", "HOLD POSITION", "FOLLOW ME", "AT EASE"]
        );
        assert_eq!(feed.squad(at + hz).map(|c| c.timer), Some(0.5));
        assert_eq!(feed.squad(at + 2 * hz).map(|c| c.timer), Some(0.0));
        assert!(feed.squad(at + 2 * hz + 1).is_none(), "gone after 2 s");
        // Each order has its own arm.
        for (button, arm) in [
            (buttons::SQUAD_HOLD, hud_squad::DOWN),
            (buttons::SQUAD_AT_EASE, hud_squad::LEFT),
            (buttons::SQUAD_MOVE_OUT, hud_squad::UP),
        ] {
            assert_eq!(order(&mut sim, me, &mut feed, button).arm, arm);
        }
    }

    /// Card SQHUD's count (the maintainer, INBOX 2026-09-30): the units the order
    /// changed. FOLLOW ME with five in reach shows 2, HOLD POSITION with
    /// three shows 3, a release the number released, nobody no number.
    #[test]
    fn the_cross_counts_the_units_that_took_the_order() {
        let (mut sim, me, mut feed) = squad_scene(5);
        let c = order(&mut sim, me, &mut feed, buttons::SQUAD_FOLLOW);
        assert_eq!(c.count, Some(2), "FOLLOW ME, five in reach");
        let c = order(&mut sim, me, &mut feed, buttons::SQUAD_MOVE_OUT);
        assert_eq!(c.count, Some(2), "MOVE OUT releases the two followers");
        let c = order(&mut sim, me, &mut feed, buttons::SQUAD_AT_EASE);
        assert_eq!(c.count, None, "nobody left to release");

        let (mut sim, me, mut feed) = squad_scene(3);
        let c = order(&mut sim, me, &mut feed, buttons::SQUAD_HOLD);
        assert_eq!(c.count, Some(3), "HOLD POSITION, three in reach");

        let (mut sim, me, mut feed) = squad_scene(0);
        for button in [buttons::SQUAD_FOLLOW, buttons::SQUAD_HOLD] {
            let c = order(&mut sim, me, &mut feed, button);
            assert_eq!(c.count, None, "nobody in reach");
        }
    }

    /// The spawn screens' texts (card SSELB): a Lua key path, a hash the
    /// ELF names, a weapon class by its `weapons.<side>.weap.<rest>` key,
    /// each falling back to the disc's English where the table lacks it.
    #[test]
    fn the_spawn_screens_read_the_table() {
        let feed = EventFeed::new(texts(), ["all", "imp"], &[]);
        assert_eq!(
            feed.screen_text("ifs.sideselect.chooseteam", "x"),
            "Select a team"
        );
        assert_eq!(
            feed.screen_text("ifs.sideselect.autoassign", "Auto Assign"),
            "Auto Assign"
        );
        assert_eq!(
            feed.weapon_name("imp_weap_inf_rifle").as_deref(),
            Some("Blaster Rifle")
        );
        assert_eq!(feed.weapon_name("imp_weap_inf_pistol"), None);
        assert_eq!(feed.weapon_name("norifle"), None);
        assert_eq!(
            feed.text_by_hash(manifest::hud::locl_key("level.hoth.cp5"), "x"),
            "Echo Base"
        );
        assert_eq!(
            feed.text_by_hash(0xecf8_496f, "Unit Count: %d"),
            "Unit Count: %d"
        );
    }
}
