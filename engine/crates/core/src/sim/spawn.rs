//! Who is on the field: [`Sim::spawn`] through the battle's reinforcement
//! gate and the no-overspawn shedding; the class tables ([`Sim::ai_class`],
//! [`Sim::spawn_ai`]); the spawn screen's class choice (`spawn_select`, step
//! 0a); the respawn at a friendly post (`respawn_slot`, step 0), paced by the
//! [`SpawnWaves`]; where a team may spawn ([`Sim::spawn_points`]: its posts'
//! nodes and its live command walkers' points, card CWSP) and which of them a
//! bot takes — one post drawn by `AISpawnWeight`, then one of that post's
//! nodes ([`Sim::spawn_posts`], `keep_or_draw_spawn_post`, card BSPN); the floor a spawn
//! stands on ([`Sim::spawn_ground`] — a post's node inside a solid body on
//! top of it, [`Sim::resolve_spawn_seats`], card LLEG); and the headless
//! referees' [`Sim::vacate_player_slot`]; the human's spawn screens (card
//! SSELA: [`SpawnStage`], [`SideChoice`], [`Sim::open_spawn_screens`]); the
//! live class change at a friendly post (issue #28, `spawn/class_change.rs`).
//! Runtime methods live in `spawn/runtime.rs`; spawn and class regressions
//! live in `spawn/tests.rs`, with screen regressions in `spawn/tests/screens.rs`.

use super::input::buttons;
#[cfg(any(test, doc))]
use super::state::Sim;
use glam::Vec3;

/// Which spawn screen a human slot has up (card SSELA; the PS2's spawn
/// display state `SD+0x18`, re-spawn-flow §1: 0 side, 2 spawn map, 3 class,
/// 5 accepted). A slot is on the field only in `Accepted`; a dead slot sits
/// in `Accepted` until its respawn timer has run out, then the spawn map
/// opens (never the side screen again, `B2:392616`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum SpawnStage {
    Side = 0,
    Map = 2,
    Class = 3,
    /// Issue #28: the class screen opened by a LIVE soldier at a friendly
    /// post (the PS2's state 4, `FUN_00397f00`, `analysis/re-class-change.md`).
    /// The soldier stays on the field; its pad drives the screen.
    Alive = 4,
    #[default]
    Accepted = 5,
}

/// Issue #28: a class change opens only while the soldier faces the post
/// this much: the cosine of the angle between his facing and the line to
/// the post must exceed it (BF2 `FUN_001f8548`, `B2:109226`;
/// `analysis/re-class-change.md` §2).
pub const CLASS_CHANGE_FACING: f32 = 0.15;

/// Issue #28: what one class a slot played this life had in its weapons when
/// the slot swapped away from it (the maintainer's rule: a class played again this
/// life comes back with exactly this ammo; a death forgets it). Each entry
/// is (class weapon slot, rounds in the clip, spare rounds).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassAmmo {
    pub class: u8,
    pub rounds: Vec<(u8, u32, u32)>,
}

/// The side screen's rows in the PS2's order (`ifs_sideselect`,
/// re-spawn-flow §2; Spectate is hidden offline).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum SideChoice {
    #[default]
    Team1 = 0,
    Team2 = 1,
    Auto = 2,
}

/// The class screen's carousel input for one slot (card SSELB,
/// `FUN_00399dd8`, `analysis/re-spawn-flow.md` §4.1): `armed` = a step has
/// fired and the push is still held (`SD+0x25`, cleared once it is let go),
/// `slide` = ticks left of the line-up's slide (`SD+0x33` / `+0x3c`). A
/// push fires only while not armed and not sliding, so a step during a
/// slide is dropped, and a push HELD through a slide steps again when it
/// ends (the arm is set only by a step that fired, B2:393674-393678).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Carousel {
    pub armed: bool,
    pub slide: u8,
}

/// A slide's length in ticks: `+0x3c` starts at 0, gains dt x 3 in the
/// step's own update (the step runs before the slide's advance,
/// `FUN_00399dd8` -> `FUN_0039a980`, B2:393714) and the slide ends in the
/// update it reaches 1 (B2:394131-394136). BF2's step is 0x3d088888, so
/// dt x 3 = 0x3dcccccc, and the EE's add rounds toward zero: ten adds read
/// 0x3f7ffffd, under 1, and the eleventh ends it [D] - PROVISIONAL until a
/// PS2 row pins the boundary update.
pub const CLASS_SLIDE_TICKS: u8 = 11;

/// The bits that push the class carousel (and the map's post, sideways).
const CAROUSEL: u32 = buttons::SPAWN_CLASS_NEXT
    | buttons::SPAWN_CLASS_PREV
    | buttons::SPAWN_POST_LEFT
    | buttons::SPAWN_POST_RIGHT;

/// Why the class screen refuses a human a class (card SSELB, the PS2's
/// `FUN_00364290`, `analysis/re-spawn-flow.md` §4.3): the panel's first line
/// and the black line-up soldier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassRefusal {
    /// "Get %d more points to unlock!": this many more points.
    Points(u32),
    /// The member has not accepted this team's current hero offer.
    HeroUnavailable,
}

impl SideChoice {
    /// One row down (`step` +1) or up (-1), wrapping over the three rows.
    pub fn moved(self, step: i32) -> SideChoice {
        match (self as i32 + step).rem_euclid(3) {
            0 => SideChoice::Team1,
            1 => SideChoice::Team2,
            _ => SideChoice::Auto,
        }
    }
}

/// PROVISIONAL (ours): the weight EVERY class keeps once its share of the team
/// is alive, as a fraction of an even share (`Sim::ai_class_weights`).
pub const CLASS_FLOOR: f32 = 0.05;
/// PROVISIONAL (ours; the maintainer, 2026-10-01): how the team's alive-AI cap is shared
/// out — the DEFAULT class (the mission's first: the rifleman, the battle
/// droid; it carries the highest authored minimum on all 114 team tables)
/// takes [`CLASS_SHARE_DEFAULT`] parts, a STANDARD class
/// [`CLASS_SHARE_STANDARD`] and a SPECIAL one [`CLASS_SHARE_SPECIAL`].
pub const CLASS_SHARE_DEFAULT: f32 = 4.0;
pub const CLASS_SHARE_STANDARD: f32 = 2.0;
pub const CLASS_SHARE_SPECIAL: f32 = 1.0;
/// The special classes are the table's entries from this index on — by the
/// discs' own class ORDER: BF2 `SetupTeams` lists soldier, assault, engineer,
/// sniper, then `officer` and `special` (its two unlock classes); BF1's fifth
/// `AddUnitClass` is the wookiee, the dark trooper, the jet trooper or the
/// droideka. Over the 57 missions that is exactly the officers, wookiees,
/// smugglers, droidekas, dark troopers and jet troopers.
pub const CLASS_SPECIAL_FROM: usize = 4;

/// A class's parts of the team by its place in the table (see
/// [`CLASS_SHARE_DEFAULT`]).
pub fn class_share(index: usize) -> f32 {
    match index {
        0 => CLASS_SHARE_DEFAULT,
        i if i >= CLASS_SPECIAL_FROM => CLASS_SHARE_SPECIAL,
        _ => CLASS_SHARE_STANDARD,
    }
}

/// Spawn waves (re-tickets-spawn.md §1.5/§2.5: BF1 member update
/// `FUN_00295c40`, BF2 `FUN_0034cea8` + manager `FUN_0036c750`, same scheme).
/// A wave opens every `delay` s and releases one spawn slot every `interval`
/// s (`slots = delay / interval`); a member killed during wave `w` is assigned
/// wave `w + 1` and the team's next slot number, and may spawn — once its own
/// respawn timer has expired — when
/// `(w_m − w) * slots + (s_m − s) < 0`, i.e. its absolute slot is behind the
/// manager's. Missions call `SetSpawnDelay(10, 0.25)` (engine defaults
/// 15.0 / 0.5). PROVISIONAL detail: slot numbers restart at 0 on each new
/// wave and are handed out in death order (the doc pins the comparison and
/// the wave+1 assignment, not the slot bookkeeping).
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnWaves {
    pub delay: f32,
    pub interval: f32,
    pub timer: f32,
    pub wave: u32,
    pub next_slot: u32,
}

impl SpawnWaves {
    /// `SetSpawnDelay(10, 0.25)` as every retail conquest mission sets it.
    pub fn mission_default() -> Self {
        Self {
            delay: 10.0,
            interval: 0.25,
            timer: 0.0,
            wave: 0,
            next_slot: 0,
        }
    }

    pub fn slots(&self) -> u32 {
        (self.delay / self.interval).round().max(1.0) as u32
    }

    pub fn cur_slot(&self) -> u32 {
        (self.timer / self.interval) as u32
    }

    pub fn tick(&mut self, dt: f32) {
        self.timer += dt;
        if self.timer >= self.delay {
            self.timer -= self.delay;
            self.wave += 1;
            self.next_slot = 0;
        }
    }

    /// Slot for a member that just died: next wave, next slot.
    pub fn assign(&mut self) -> (u32, u32) {
        let slot = (self.wave + 1, self.next_slot);
        self.next_slot += 1;
        slot
    }

    /// Absolute slots the member is ahead of the manager (< 0 = may spawn).
    fn ahead(&self, (w, s): (u32, u32)) -> i64 {
        (i64::from(w) - i64::from(self.wave)) * i64::from(self.slots())
            + (i64::from(s) - i64::from(self.cur_slot()))
    }

    pub fn ready(&self, slot: (u32, u32)) -> bool {
        self.ahead(slot) < 0
    }

    /// Seconds until the member's slot passes (HUD "respawn in").
    pub fn countdown(&self, slot: (u32, u32)) -> f32 {
        let ahead = self.ahead(slot) + 1;
        let into_slot = self.timer - self.cur_slot() as f32 * self.interval;
        (ahead as f32 * self.interval - into_slot).max(0.0)
    }
}

/// A spawn node authored UNDER its floor is looked for from this far over the
/// node (m). PROVISIONAL (ours).
pub const SPAWN_LIFT: f32 = 3.0;

/// A floor a soldier walks to fewer than this many cells from is a PEN
/// ([`Sim::floor_reach`]; about 110 m2). PROVISIONAL (ours).
pub const PEN_CELLS: usize = 200;
/// The pen test's cell (m). PROVISIONAL (ours).
pub const PEN_CELL: f32 = 0.75;
/// The pen test walks off a ledge no higher than this (m): the AI's own
/// ledge rule (`ai::LEDGE_DROP`).
pub const PEN_DROP: f32 = 4.0;
/// A capsule the wall pass moves by more than this is in a body (m).
const PEN_CLEAR: f32 = 0.02;

/// A flood cell: its grid step and its storey (1.5 m bands), so a floor over
/// a floor is a cell of its own.
fn flood_key(ix: i32, iz: i32, y: f32) -> (i32, i32, i32) {
    (ix, iz, (y / 1.5).floor() as i32)
}
/// A penned spawn node's floor is looked for up to this far over it (whole
/// m). PROVISIONAL (ours): BF1 Naboo Plains' column is 4.26 m over its node.
pub const SEAT_REACH: u32 = 6;

/// One class spawn point of a command walker at `pos` with entity rotation
/// `rot`, as a spawn node (re-command-walker-post §4 [O], B2 FUN_00356998 /
/// B1 FUN_0029df70): the authored position through the walker's matrix
/// (`pos + x·right + y·up + z·forward`), facing the walker's heading turned
/// by the authored angle, yaw only. RAM vector: `the_ram_walker_gives_its_six_points`.
///
/// The live respawn path casts from the walker's bounding-sphere centre
/// toward this node. The turn's sense is the stored quaternion's: BF2
/// Hoth's RAM holds (0, 0.0436, 0, ±0.9990)
/// for 5° / 355° (`lead_tmp/cwsp/cp_ram2.py`), the form of
/// [`quat_y`](crate::vehicle::hover::quat_y) — the heading plus the angle,
/// so the AT-AT's two sides face 5° outward. That a spawned unit shows the
/// 5° is not measured (§11).
pub fn walker_point(pos: Vec3, rot: glam::Quat, (local, angle): (Vec3, f32)) -> (Vec3, f32) {
    let fwd = rot * Vec3::Z;
    (pos + rot * local, crate::fmath::atan2(fwd.x, fwd.z) + angle)
}

mod class_change;
mod runtime;

#[cfg(test)]
mod tests;
