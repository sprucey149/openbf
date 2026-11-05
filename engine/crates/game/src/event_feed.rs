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
        Some(t) if sim.allied(my_team, t) => FRIEND,
        Some(t) if sim.hostile(my_team, t) => ENEMY,
        _ => NONE,
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
    space_round: Option<std::sync::Arc<crate::space_assault::SpaceRound>>,
    space_hud: crate::space_assault::SpaceHud,
    space_tick: u64,
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
    native_classes: std::collections::BTreeMap<u8, Vec<String>>,
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
    damage_flash: Option<(UnitHandle, u64, crate::hit_flash::Flash)>,
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
            native_classes: Default::default(),
            vehicles: Vec::new(),
            stations: Vec::new(),
            order_labels: ORDER_LABELS.map(|l| texts.order_label(l)),
            driver_labels: DRIVER_LABELS.map(|l| texts.order_label(l)),
            texts,
            space_round: None,
            space_hud: Default::default(),
            space_tick: 0,
            out: Vec::new(),
            ambush: Vec::new(),
            damage_flash: None,
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
        feed.space_round = walk.space_round.clone();
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
        for (&team, template) in &walk.native_teams {
            feed.native_classes.insert(
                team,
                template
                    .classes
                    .iter()
                    .map(|c| {
                        feed.texts
                            .class_name(&c.name)
                            .unwrap_or_else(|| c.label.clone())
                    })
                    .collect(),
            );
        }
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
        self.classes
            .get(usize::from(team))
            .or_else(|| self.native_classes.get(&team))
            .map_or(&[], Vec::as_slice)
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
        if let Some(name) = self
            .native_classes
            .get(&u.team)
            .and_then(|classes| classes.get(usize::from(u.spec.class)))
        {
            return name.clone();
        }
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

    fn space_messages(&mut self, sim: &Sim, my_team: u8, tick: u64) {
        let Some(round) = self.space_round.clone() else {
            return;
        };
        for e in &sim.events {
            let keys: Vec<String> = match *e {
                Event::SpaceObjectiveDown { objective, owner } => round
                    .objectives
                    .get(usize::from(objective))
                    .map(|o| o.down_text(owner != my_team))
                    .into_iter()
                    .collect(),
                Event::SpaceLinkChanged { kind, index, up } => round
                    .scripted(kind, index, up)
                    .map(|s| {
                        s.texts
                            .iter()
                            .filter(|(_, team)| *team == my_team)
                            .map(|(key, _)| key.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
                _ => continue,
            };
            for key in keys {
                if let Some(text) = self.texts.bf2_text(&key).map(|t| t.trim().to_owned()) {
                    self.say(text, message_color::YELLOW, tick);
                }
            }
        }
    }

    pub fn space_status(
        &mut self,
        sim: &Sim,
        round: &crate::space_assault::SpaceRound,
        my_team: u8,
    ) -> Option<ui::hud_space::SpaceStatus> {
        let dt = sim.tick.saturating_sub(self.space_tick) as f32 / core::TICK_HZ as f32;
        self.space_tick = sim.tick;
        self.space_hud.status(sim, round, my_team, dt)
    }

    /// The Space Assault objective a team reads on the spawn screen, the
    /// first its script gives to `AddMissionObjective`.
    pub fn space_objective(&self, team: u8) -> Option<String> {
        let round = self.space_round.as_ref()?;
        let key = round.team_text.get(usize::from(team))?;
        self.texts.bf2_text(key).map(|t| t.trim().to_owned())
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
            if sim.allied(my_team, team) {
                message_color::OWN
            } else if sim.hostile(my_team, team) {
                message_color::ENEMY
            } else {
                message_color::NEUTRAL
            }
        };
        self.note_capture_regions(sim);
        self.round.tick(sim);

        self.space_messages(sim, my_team, tick);
        for e in &sim.events {
            if let Event::Hit {
                victim,
                damage,
                health_left,
                ..
            } = *e
            {
                if victim == viewer {
                    if let Some(flash) = crate::hit_flash::Flash::from_damage(damage, health_left) {
                        self.damage_flash = Some((viewer, tick, flash));
                    }
                }
            }
            if let Event::VehicleHit {
                vehicle, damage, ..
            } = *e
            {
                let mount = sim.unit(viewer).and_then(|u| u.mount);
                let hull = sim.vehicle(vehicle).filter(|v| {
                    mount.is_some_and(|m| {
                        m.vehicle == vehicle
                            && v.seats.get(usize::from(m.seat)).map(|s| s.kind)
                                == Some(SeatKind::Pilot)
                    })
                });
                if let Some(flash) =
                    hull.and_then(|v| crate::hit_flash::Flash::from_damage(damage, v.health.cur))
                {
                    self.damage_flash = Some((viewer, tick, flash));
                }
            }
            if let Some(edge) = ambush_edge(sim, viewer, e) {
                self.ambush.push(edge);
            }
            match *e {
                Event::Droideka {
                    unit,
                    cue: core::droideka::Cue::EnergyRefused,
                    ..
                } if unit == viewer => {
                    // Ours: a refused ball jump means the shared bar (the
                    // shield's charge) ran empty and is not yet back at the
                    // recover share.
                    self.say(
                        "Shield charge spent — wait for recharge".to_owned(),
                        message_color::OWN,
                        tick,
                    );
                }
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
                        .is_some_and(|u| u.gen == victim.gen && sim.hostile(my_team, u.team)) =>
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
                        } else if sim.allied(k.team, v.team) {
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
                        } else if sim.allied(k.team, v.team) {
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

    /// Local damage overlay, on the simulation clock and scoped to this life.
    pub fn damage_flash(&self, tick: u64, viewer: UnitHandle) -> f32 {
        self.damage_flash
            .filter(|(unit, _, _)| *unit == viewer)
            .map_or(0.0, |(_, start, flash)| {
                flash.alpha_at(tick.saturating_sub(start) as f32 / core::TICK_HZ as f32)
            })
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
    /// unowned building turret is (card HTGT). The classes' `HideHealthBar` drop (§2: two BF2 classes) is not ported.
    pub fn target(&self, sim: &Sim, slot: usize) -> Option<HudTarget> {
        let Some((t, relation)) = sim.reticle_pick.get(slot).copied().flatten() else {
            return self
                .wreck_target(sim, slot)
                .or_else(|| self.station_target(sim, slot));
        };
        self.aim_target(sim, t, relation)
    }

    pub fn lock_target(&self, sim: &Sim, target: core::sim::LockTarget) -> Option<HudTarget> {
        match target {
            core::sim::LockTarget::Aim(t) => self.aim_target(sim, t, -1),
            core::sim::LockTarget::Station(i) => {
                let s = sim.stations.get(usize::from(i))?;
                self.station_hud(s, i as usize, -1)
            }
        }
    }

    fn station_hud(&self, s: &core::sim::Station, i: usize, relation: i8) -> Option<HudTarget> {
        (s.health.max > 0.0).then(|| HudTarget {
            id: (1 << 62) | i as u64,
            name: self
                .stations
                .get(usize::from(s.class))
                .cloned()
                .unwrap_or_default(),
            health_fraction: (s.health.cur / s.health.max).clamp(0.0, 1.0),
            relation,
            team0: s.team.is_none(),
            shield: None,
        })
    }

    fn aim_target(&self, sim: &Sim, t: AimTarget, relation: i8) -> Option<HudTarget> {
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
                let name = self
                    .class_name(u.team, u.spec.class)
                    .unwrap_or_default()
                    .to_owned();
                Some(HudTarget {
                    id: (u64::from(h.gen) << 32) | u64::from(h.index),
                    name,
                    health_fraction: fraction(&u.health),
                    relation,
                    team0: false,
                    shield: u.droideka.as_ref().filter(|d| d.shield.on).and_then(|d| {
                        let max = u.droideka_class()?.shield.max;
                        (max > 0.0)
                            .then(|| ("Shield".to_owned(), (d.shield.points / max).clamp(0.0, 1.0)))
                    }),
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
        self.class_names(team)
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
mod tests;
