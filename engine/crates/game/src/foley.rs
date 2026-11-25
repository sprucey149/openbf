//! Soldier foley (card FOLY, `analysis/re-foley.md`, `[O]`): footsteps by
//! surface (walk / run, left / right) and the jump, land, roll, crouch, prone
//! and get-up sounds, from `<map>/sounds/foley.ron`. Presentation only: it
//! reads the sim after a stepped tick (beside `event_sounds::each`) and the
//! sim never reads it.
//!
//! - **Row** (§2): the unit class's `FoleyFXClass` picks the row of the
//!   current surface group; the battle's table is the sub-level `<map><era>`.
//! - **Surface** (§3): terrain -> `terrain_foley`, feet under the water ->
//!   `water_foley`; a group with no row for the class keeps the last row.
//!   PROVISIONAL: the prop under the feet (its class's `FoleyFXGroup`) is not
//!   read yet; a sub-level with no terrain group (Kamino, Bespin: every floor
//!   is a metal prop) starts on its first group instead.
//! - **Steps** (§4.1, §8.2): the PS2 starts a step when a foot of the DRAWN
//!   clip lifts off. The port's animator is GPU-side, so the same sounds come
//!   from the sim's state: the PS2's movement clip by its chooser's rules
//!   (§4.4), a distance phase, one step per half cycle of the clip's root
//!   travel, left and right alternating; a walk step when a run starts and
//!   when it stops. Only a drawn soldier steps ([`Who`]). The PS2's step
//!   also needs the foot to come down below the class's `MinFootHeight`
//!   first ([`MIN_FOOT_HEIGHT`]): with the drawn feet ([`Foley::feel`]), a
//!   step sounds only if a foot was that low within [`FOOT_WINDOW`] ticks, so
//!   a soldier whose clip floats - the Emperor's sprint and glide, Vader's
//!   sprint - makes none.
//! - **Events** (§4.5): Jump, Land, Roll, Squat, and Prone / Stand where the
//!   row has the slot (BF1's rows), for every soldier. `BodyFall` waits for a
//!   thrown-soldier state. Nothing for the weapons' `*_mvt_*` keys (§5).

mod droideka;

use crate::walk::WalkSim;
use core::soldier::{GroundRules, Stance};
use glam::{Mat4, Vec3};
use manifest::{FoleyGroup, FoleyRon, FoleyRow};
use std::path::Path;

/// The class key a soldier names its row with (`FoleyFXClass`, §1.3).
pub const FOLEY_FX_CLASS: u32 = 0x99D5_5922;

/// Which soldiers make footsteps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    /// The PS2's rule (§4.3): a soldier that is being drawn.
    InView,
    /// Every soldier in earshot - about ten times denser (§8.1).
    InReach,
}

/// FEEL (`FEEL_NOTES.md`): the PS2's rule.
pub const WHO: Who = Who::InView;

/// The foley template's max distance (§7): no start is heard past it.
pub const REACH: f32 = 50.0;

/// A step's foot must have been lower than this, model space, metres (§4.1:
/// B1's literal; B2's default for a class with no `MinFootHeight`, §1.3).
pub const MIN_FOOT_HEIGHT: f32 = 0.17;

/// How recently a drawn foot must have been below its class's
/// `MinFootHeight` for a step to sound, ticks. OURS: the PS2 starts each step
/// at a foot's lift-off; the port keeps its distance phase and asks only that
/// a foot was down lately. 0.5 s is over half a walk cycle (0.43 s at
/// 3 m/s), and a walking or running clip has a foot under 0.17 m in every
/// half cycle (lowest foot 0.13-0.14 m); the floating clips never come under
/// 0.33 m (the Emperor's sprint 0.54, his glide 0.43, Vader's sprint 0.34).
/// measure: the steps' count against the PS2's on a slow crouch-walk.
pub const FOOT_WINDOW: u32 = 15;

/// Ground covered per step by movement clip, metres. Forward is half the
/// clip's root travel (`re-anim-keys.md` §5: run 4.014 m, walk 2.581 m per
/// cycle); the others are PROVISIONAL, the middle of the PS2 player's
/// measured spacing (§8.2: backward 1.5-1.8, left 1.4-1.7, right 1.0 / 1.6).
const STRIDE_RUN_FORWARD: f32 = 4.014 / 2.0;
const STRIDE_WALK_FORWARD: f32 = 2.581 / 2.0;
const STRIDE_BACKWARD: f32 = 1.65;
const STRIDE_SIDE_RUN: f32 = 1.55;
const STRIDE_SIDE_WALK: f32 = 1.3;

/// Below this on both axes the clip is idle (BF1 `FUN_0022e990`; BF2's speed
/// mode 0 is 0.3 x `SkeletonRootScale`, 0.2 to stay).
const IDLE_SPEED: f32 = 0.3;
const IDLE_SPEED_STAY: f32 = 0.2;
/// BF1's chooser returns idle above this (§4.4).
const BF1_IDLE_ABOVE: f32 = 8.0;

/// The direction of the movement clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dir {
    #[default]
    Idle,
    Forward,
    Backward,
    Left,
    Right,
}

/// The movement clip: its direction and whether it is the run clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clip {
    pub dir: Dir,
    pub run: bool,
}

impl Clip {
    const IDLE: Clip = Clip {
        dir: Dir::Idle,
        run: false,
    };

    fn stride(self) -> f32 {
        match (self.dir, self.run) {
            (Dir::Forward, true) => STRIDE_RUN_FORWARD,
            (Dir::Forward, false) => STRIDE_WALK_FORWARD,
            (Dir::Backward, _) => STRIDE_BACKWARD,
            (_, true) => STRIDE_SIDE_RUN,
            (_, false) => STRIDE_SIDE_WALK,
        }
    }
}

fn speed(fwd: f32, side: f32) -> f32 {
    glam::Vec2::new(fwd, side).length()
}

/// The clip's direction from the body-space speeds (`fwd` ahead, `side` to
/// the right). PROVISIONAL: the larger axis wins.
fn direction(fwd: f32, side: f32, idle: f32) -> Dir {
    if fwd.abs() < idle && side.abs() < idle {
        Dir::Idle
    } else if fwd.abs() >= side.abs() {
        if fwd > 0.0 {
            Dir::Forward
        } else {
            Dir::Backward
        }
    } else if side > 0.0 {
        Dir::Right
    } else {
        Dir::Left
    }
}

/// BF1's movement clip (§4.4, `FUN_0022e990`): forward runs above 5.5 m/s
/// (5.0 to stay), right above 5.0 (5.5 when already walking right), and
/// backward and LEFT are always the run clip - the PS2's own tests there
/// compare a negative speed with a positive threshold.
pub fn clip_bf1(fwd: f32, side: f32, prev: Clip) -> Clip {
    if speed(fwd, side) > BF1_IDLE_ABOVE {
        return Clip::IDLE;
    }
    let dir = direction(fwd, side, IDLE_SPEED);
    let run = match dir {
        Dir::Idle => false,
        Dir::Forward => {
            let stay = prev.dir == Dir::Forward && prev.run;
            fwd > if stay { 5.0 } else { 5.5 }
        }
        Dir::Right => {
            let walking = prev.dir == Dir::Right && !prev.run;
            side > if walking { 5.5 } else { 5.0 }
        }
        Dir::Backward | Dir::Left => true,
    };
    Clip { dir, run }
}

/// BF2's movement clip (§4.4, `FUN_0024f350`): the run clip in any direction
/// above 0.78 x `MaxSpeed` (0.71 to stay).
pub fn clip_bf2(fwd: f32, side: f32, max_speed: f32, prev: Clip) -> Clip {
    let moving = prev.dir != Dir::Idle;
    let idle = if moving { IDLE_SPEED_STAY } else { IDLE_SPEED };
    let dir = direction(fwd, side, idle);
    let k = if prev.run { 0.71 } else { 0.78 };
    Clip {
        dir,
        run: dir != Dir::Idle && speed(fwd, side) > max_speed * k,
    }
}

/// The sound sub-levels of a battle on `map_dir`, by name hash, best first
/// (card SNDE): `named`, the ones the mission's script loads (the PS2 holds
/// those and no other); then the map's own for an army the mission does not
/// field or a battle without a mission - `<map><era>` for each team's
/// faction, a `cross` one (three BF2 maps), then whichever era the map has.
pub fn sublevels(map_dir: &Path, factions: [&str; 2], named: &[String]) -> Vec<u32> {
    let dir = map_dir.file_name().and_then(|d| d.to_str()).unwrap_or("");
    let map = dir
        .strip_prefix("bf1_")
        .or_else(|| dir.strip_prefix("bf2_"))
        .unwrap_or(dir);
    let era = |f: &str| match f {
        "rep" | "cis" => Some("cw"),
        "all" | "imp" => Some("gcw"),
        _ => None,
    };
    let h = crate::classdb::fnv1a_lowercase;
    let eras = factions.iter().filter_map(|f| era(f));
    let guesses = eras.chain(["cross", "cw", "gcw"]);
    let mut out: Vec<u32> = named.iter().map(|n| h(n)).collect();
    for l in guesses.map(|e| h(&format!("{map}{e}"))) {
        if !out.contains(&l) {
            out.push(l);
        }
    }
    out
}

/// The armies of a battle that its mission's sub-levels are not of the era
/// of (the port's era mix, card ERAM: the Republic in a `kas2gcw` battle),
/// each as its name prefix (`rep_`) with the map's sub-level of its own era
/// (`kas2cw`). A sub-level names the other era's commander lines too, as
/// aliases of its own era's takes (BF1 `kas2gcw`'s
/// `rep_off_com_report_captured_commandpost` is the Alliance's take), so
/// such an army's lines are looked up in its own era's sub-level first
/// (`SoundLib::for_battle`). Ours: the PS2 has no such battle.
pub fn foreign_armies(map_dir: &Path, factions: [&str; 2], named: &[String]) -> Vec<(String, u32)> {
    let dir = map_dir.file_name().and_then(|d| d.to_str()).unwrap_or("");
    let map = dir
        .strip_prefix("bf1_")
        .or_else(|| dir.strip_prefix("bf2_"))
        .unwrap_or(dir);
    // `gcw` ends in `cw`: test it first.
    let of_level = |n: &String| match n {
        n if n.ends_with("gcw") => Some("gcw"),
        n if n.ends_with("cw") => Some("cw"),
        _ => None,
    };
    let loaded: Vec<&str> = named.iter().filter_map(of_level).collect();
    let mut out = Vec::new();
    for f in factions {
        let era = match f {
            "rep" | "cis" => "cw",
            "all" | "imp" => "gcw",
            _ => continue,
        };
        if !loaded.is_empty() && !loaded.contains(&era) {
            let level = crate::classdb::fnv1a_lowercase(&format!("{map}{era}"));
            out.push((format!("{f}_"), level));
        }
    }
    out
}

/// [`sublevels`] for `walk`'s battle: its teams and its mission's sub-levels.
pub fn battle_levels(map_dir: &Path, walk: &WalkSim) -> Vec<u32> {
    let f = [0, 1].map(|t| walk.teams[t].faction.as_str());
    sublevels(map_dir, f, &walk.sound_levels)
}

/// A battle's foley: the sub-level's groups and its two special groups.
#[derive(Debug, Clone, Default)]
pub struct Table {
    pub groups: Vec<FoleyGroup>,
    pub terrain: Option<usize>,
    pub water: Option<usize>,
    /// Where a soldier starts (§2: the terrain group; see the module doc for
    /// a sub-level without one).
    pub start: Option<usize>,
}

impl Table {
    pub fn new(groups: Vec<FoleyGroup>) -> Table {
        let h = crate::classdb::fnv1a_lowercase;
        let find = |n: &str| groups.iter().position(|g| g.name == h(n));
        let terrain = find("terrain_foley");
        Table {
            water: find("water_foley"),
            start: terrain.or(if groups.is_empty() { None } else { Some(0) }),
            terrain,
            groups,
        }
    }

    /// The battle's sub-level ([`sublevels`]) of `<map dir>/sounds/foley.ron`.
    pub fn load(map_dir: &Path, levels: &[u32]) -> Option<Table> {
        let text = pak::read_to_string(map_dir.join("sounds").join("foley.ron")).ok()?;
        let ron: FoleyRon = ron::from_str(&text).ok()?;
        levels.iter().find_map(|name| {
            let l = ron.levels.iter().find(|l| l.name == *name)?;
            Some(Table::new(l.groups.clone()))
        })
    }

    /// The group's row for a soldier class: its own, else the first row with
    /// no class (B1 `FUN_00368238`).
    fn row_in(&self, group: usize, class: u32) -> Option<usize> {
        let rows = &self.groups.get(group)?.rows;
        rows.iter()
            .position(|r| r.class == class)
            .or_else(|| rows.iter().position(|r| r.class == 0))
    }

    fn row(&self, s: Surface) -> Option<&FoleyRow> {
        let (g, r) = s.row?;
        self.groups.get(g)?.rows.get(r)
    }
}

/// A soldier's collider (§2): the group last touched and the row in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Surface {
    pub group: Option<usize>,
    /// (group, row): not always of `group` - a group with no row for the
    /// class leaves the last row in place.
    pub row: Option<(usize, usize)>,
}

impl Surface {
    /// A new collider: on the start group.
    pub fn start(table: &Table, class: u32) -> Surface {
        Surface {
            group: table.start,
            row: table.start.and_then(|g| Some((g, table.row_in(g, class)?))),
        }
    }

    /// The soldier touched `group` (B1 `FUN_00367c90`): nothing for no
    /// group, the current group or a soldier with no foley class; else the
    /// group changes and the row only when the group has one for the class.
    pub fn touch(&mut self, table: &Table, class: u32, group: Option<usize>) {
        let Some(g) = group else { return };
        if self.group == Some(g) || class == 0 {
            return;
        }
        self.group = Some(g);
        if let Some(r) = table.row_in(g, class) {
            self.row = Some((g, r));
        }
    }
}

/// The group under a soldier (§3): the water group below the water's
/// surface, the terrain group on the ground, nothing new in the air.
pub fn group_under(
    table: &Table,
    feet_y: f32,
    water_level: Option<f32>,
    grounded: bool,
) -> Option<usize> {
    if water_level.is_some_and(|w| feet_y < w) {
        table.water
    } else if grounded {
        table.terrain
    } else {
        None
    }
}

/// The foley properties' names by hash, for a dev log (the discs hold
/// hashes only): `<class>_<action>_<surface>[_left|_right]` over the record
/// classes and surface words of both discs (§1.1).
pub fn names() -> std::collections::HashMap<u32, String> {
    const CLASSES: [&str; 9] = [
        "all_inf_soldier",
        "imp_inf_trooper",
        "imp_inf_soldier",
        "cis_inf_droid",
        "rep_inf_trooper",
        "rep_inf_soldier",
        "cis_inf_soldier",
        "wok_inf_soldier",
        "npc_inf_small",
    ];
    const SURFACES: [&str; 7] = ["dirt", "grass", "metal", "stone", "water", "wood", "snow"];
    const ACTIONS: [&str; 7] = ["jump", "land", "roll", "squat", "lie", "getup", "stop"];
    let mut out = std::collections::HashMap::new();
    let mut add = |n: String| {
        out.insert(crate::classdb::fnv1a_lowercase(&n), n);
    };
    for c in CLASSES {
        for s in SURFACES {
            for a in ACTIONS {
                add(format!("{c}_{a}_{s}"));
            }
            for a in ["run", "walk"] {
                for f in ["left", "right"] {
                    add(format!("{c}_{a}_{s}_{f}"));
                }
            }
        }
    }
    out
}

/// One started foley sound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sound {
    /// `Sim::units` index.
    pub unit: u32,
    /// The sound property's name hash.
    pub hash: u32,
    pub pos: Vec3,
}

/// What the camera draws: the PS2 poses - and so steps - only those.
/// Splitscreen (issue #194, `SPLITSCREEN.md` 18.1 item 6): the other awake
/// local players' views in `more`, and a soldier sounds when any view draws
/// him ([`View::any`]).
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub eye: Vec3,
    pub view_proj: Mat4,
    /// The further views' eyes and projections, in local order.
    pub more: [Option<(Vec3, Mat4)>; 3],
}

impl View {
    pub fn of(cam: &render::Camera, aspect: f32) -> View {
        View {
            eye: cam.position,
            view_proj: cam.view_proj(aspect),
            more: [None; 3],
        }
    }

    /// A view that draws no soldier, its eye at `eye` (splitscreen with every
    /// local player away: no cell draws the battle, so no foley sounds).
    pub fn blind(eye: Vec3) -> View {
        View {
            eye,
            view_proj: Mat4::ZERO,
            more: [None; 3],
        }
    }

    /// `self` with `views` added after the first, up to three (the rest are
    /// dropped; their own `more` is not read).
    pub fn with_more(mut self, views: impl IntoIterator<Item = View>) -> View {
        for (slot, v) in self.more.iter_mut().zip(views) {
            *slot = Some((v.eye, v.view_proj));
        }
        self
    }

    /// `test` passes for this view or one of `more`. With one view it is
    /// `test(self)`.
    pub(crate) fn any(&self, mut test: impl FnMut(&View) -> bool) -> bool {
        test(self)
            || self.more.iter().flatten().any(|&(eye, view_proj)| {
                test(&View {
                    eye,
                    view_proj,
                    more: [None; 3],
                })
            })
    }

    /// A soldier standing at `feet` is on screen (his 2 m, either end).
    fn sees(&self, feet: Vec3) -> bool {
        [feet, feet + Vec3::Y * 2.0].iter().any(|p| {
            let c = self.view_proj * p.extend(1.0);
            c.w > 0.0 && c.x.abs() <= c.w && c.y.abs() <= c.w
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Track {
    gen: u32,
    live: bool,
    surface: Surface,
    stance: Option<Stance>,
    clip: Clip,
    /// Ground covered since the last step.
    dist: f32,
    right_next: bool,
}

/// Every unit's foley state.
#[derive(Debug, Clone)]
pub struct Foley {
    pub table: Table,
    pub who: Who,
    tracks: Vec<Track>,
    /// Per unit: ticks since a drawn foot was below its class's
    /// `MinFootHeight`, `None` without a drawn pose ([`Foley::feel`]).
    feet: Vec<Option<u32>>,
    droidekas: droideka::Feet,
}

fn walking(s: Stance) -> bool {
    matches!(s, Stance::Stand | Stance::Crouch | Stance::Sprint)
}

impl Foley {
    pub fn new(table: Table) -> Foley {
        Foley {
            table,
            who: WHO,
            tracks: Vec::new(),
            feet: Vec::new(),
            droidekas: droideka::Feet::default(),
        }
    }

    /// The drawn feet of the tick about to be [`tick`](Foley::tick)ed: each
    /// unit's posed model-space foot heights, `None` for one not drawn. A unit
    /// never felt steps as it did before (the headless sound log).
    pub fn feel(&mut self, walk: &WalkSim, heights: &[Option<[f32; 2]>]) {
        self.feet.resize(walk.sim.units.len(), None);
        for (i, u) in walk.sim.units.iter().enumerate() {
            let Some(h) = heights.get(i).copied().flatten() else {
                self.feet[i] = None;
                continue;
            };
            let floor = walk
                .team_template(u.team)
                .class(u.spec.class)
                .min_foot_height
                .unwrap_or(MIN_FOOT_HEIGHT);
            self.feet[i] = if h[0] < floor || h[1] < floor {
                Some(0)
            } else {
                Some(self.feet[i].map_or(FOOT_WINDOW + 1, |ago| ago.saturating_add(1)))
            };
        }
    }

    /// The battle's foley, when the map has a table for it.
    pub fn load(map_dir: &Path, walk: &WalkSim, table: Option<Table>) -> Option<Foley> {
        let table = table.or_else(|| Table::load(map_dir, &battle_levels(map_dir, walk)));
        let droidekas = droideka::Feet::load(map_dir, walk);
        if table.is_none() && droidekas.is_empty() {
            return None;
        }
        let mut foley = Foley::new(table.unwrap_or_default());
        foley.droidekas = droidekas;
        Some(foley)
    }

    /// The tick the sim just stepped: `play` for each sound it starts.
    pub fn tick(&mut self, walk: &WalkSim, view: &View, mut play: impl FnMut(Sound)) {
        self.droidekas.tick(walk, view, self.who, &mut play);
        let sim = &walk.sim;
        self.tracks.resize(sim.units.len(), Track::default());
        let bf1 = sim.ground_rules == GroundRules::Bf1;
        for (i, u) in sim.units.iter().enumerate() {
            let t = &mut self.tracks[i];
            if !u.alive || u.mount.is_some() || u.droideka.is_some() {
                t.live = false;
                continue;
            }
            let s = &u.soldier;
            let class = walk.team_template(u.team).class(u.spec.class).foley_class;
            if !t.live || t.gen != u.gen {
                *t = Track {
                    gen: u.gen,
                    live: true,
                    surface: Surface::start(&self.table, class),
                    stance: Some(s.stance),
                    ..Track::default()
                };
            }
            let under = group_under(&self.table, s.pos.y, sim.water_under(s.pos), s.grounded);
            t.surface.touch(&self.table, class, under);
            let prev = t.stance.replace(s.stance).unwrap_or(s.stance);
            let Some(row) = self.table.row(t.surface) else {
                continue;
            };
            let mut start = |hash: u32| {
                if hash != 0 {
                    play(Sound {
                        unit: i as u32,
                        hash,
                        pos: s.pos,
                    });
                }
            };

            // The stance functions' slots (§4.5): every soldier's.
            if s.stance != prev {
                let air =
                    |x| matches!(x, Stance::Jump | Stance::Fall | Stance::Jet | Stance::Hover);
                match s.stance {
                    Stance::Jump => start(row.jump),
                    Stance::Roll => start(row.roll),
                    Stance::Prone => start(row.prone),
                    Stance::Crouch if walking(prev) => start(row.squat),
                    Stance::Stand | Stance::Sprint
                        if matches!(prev, Stance::Crouch | Stance::Prone) =>
                    {
                        start(row.stand)
                    }
                    _ => {}
                }
                if air(prev) && !air(s.stance) && s.stance != Stance::Roll {
                    start(row.land);
                }
            }

            // The steps (§4.1): on the ground, in a walking stance, drawn.
            if !(s.grounded && walking(s.stance)) {
                continue;
            }
            let who = self.who;
            let drawn = view.any(|v| {
                (s.pos - v.eye).length() <= REACH
                    && (who == Who::InReach || u.is_player || v.sees(s.pos))
            });
            if !drawn {
                t.clip = Clip::IDLE;
                t.dist = 0.0;
                continue;
            }
            let (sin, cos) = core::fmath::sin_cos(s.yaw);
            let fwd = -s.vel.x * sin - s.vel.z * cos;
            let side = s.vel.x * cos - s.vel.z * sin;
            let clip = if bf1 {
                clip_bf1(fwd, side, t.clip)
            } else {
                clip_bf2(fwd, side, u.consts.max_speed, t.clip)
            };
            let was = std::mem::replace(&mut t.clip, clip);
            // §4.1: no foot came down lately (a floating clip): no sound.
            let footed = self
                .feet
                .get(i)
                .copied()
                .flatten()
                .is_none_or(|ago| ago <= FOOT_WINDOW);
            let mut step = |t: &mut Track, run: bool| {
                let right = t.right_next;
                t.right_next = !right;
                if !footed {
                    return;
                }
                start(match (run, right) {
                    (true, true) => row.run_right,
                    (true, false) => row.run_left,
                    (false, true) => row.walk_right,
                    (false, false) => row.walk_left,
                });
            };
            match (was.dir == Dir::Idle, clip.dir == Dir::Idle) {
                // Standing.
                (true, true) => {}
                // A start and a stop each lift a foot in the walk clip (§8.2).
                (true, false) | (false, true) => {
                    t.dist = 0.0;
                    step(t, false);
                }
                (false, false) => {
                    t.dist += speed(fwd, side) * core::TICK_DT;
                    if t.dist >= clip.stride() {
                        t.dist -= clip.stride();
                        step(t, clip.run);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALK: Clip = Clip {
        dir: Dir::Right,
        run: false,
    };

    fn row(class: u32, run_left: u32) -> FoleyRow {
        FoleyRow {
            class,
            run_left,
            ..FoleyRow::default()
        }
    }

    fn table() -> Table {
        let h = crate::classdb::fnv1a_lowercase;
        Table::new(vec![
            FoleyGroup {
                name: h("terrain_foley"),
                rows: vec![row(1, 10), row(2, 20)],
            },
            FoleyGroup {
                name: h("water_foley"),
                rows: vec![row(1, 11)],
            },
            FoleyGroup {
                name: h("metal_foley"),
                rows: vec![row(2, 22), row(0, 99)],
            },
        ])
    }

    /// §3: terrain, water, a group with no row for the class keeps the last
    /// row, a class-less row serves any class, a soldier with no foley class
    /// has no row.
    #[test]
    fn the_surface_is_the_last_group_touched_and_keeps_a_row_it_cannot_replace() {
        let t = table();
        assert_eq!((t.terrain, t.water, t.start), (Some(0), Some(1), Some(0)));
        let run_left = |s: Surface| t.row(s).map(|r| r.run_left);

        let mut one = Surface::start(&t, 1);
        assert_eq!(run_left(one), Some(10));
        // On the ground above the water: terrain. Feet under it: water.
        assert_eq!(group_under(&t, 1.0, Some(0.5), true), Some(0));
        assert_eq!(group_under(&t, 0.2, Some(0.5), true), Some(1));
        assert_eq!(group_under(&t, 0.2, None, true), Some(0));
        assert_eq!(group_under(&t, 3.0, Some(0.5), false), None);
        one.touch(&t, 1, t.water);
        assert_eq!(run_left(one), Some(11));
        // In the air nothing is touched: the water row stays.
        one.touch(&t, 1, None);
        assert_eq!((one.group, run_left(one)), (Some(1), Some(11)));
        one.touch(&t, 1, t.terrain);
        assert_eq!(run_left(one), Some(10));

        // Class 2 has no water row: the group changes, the row does not.
        let mut two = Surface::start(&t, 2);
        two.touch(&t, 2, t.water);
        assert_eq!((two.group, run_left(two)), (Some(1), Some(20)));
        // Metal has its row; class 1 gets metal's class-less row.
        two.touch(&t, 2, Some(2));
        assert_eq!(run_left(two), Some(22));
        one.touch(&t, 1, Some(2));
        assert_eq!(run_left(one), Some(99));

        // No foley class: no row at the start, and no touch changes that.
        let mut none = Surface::start(&t, 0);
        assert_eq!(run_left(none), None);
        none.touch(&t, 0, Some(2));
        assert_eq!((none.group, none.row), (Some(0), None));

        // A sub-level with no terrain group starts on its first group.
        let kamino = Table::new(vec![FoleyGroup {
            name: crate::classdb::fnv1a_lowercase("metal_foley"),
            rows: vec![row(1, 7)],
        }]);
        assert_eq!((kamino.terrain, kamino.start), (None, Some(0)));
        let s = Surface::start(&kamino, 1);
        assert_eq!(kamino.row(s).map(|r| r.run_left), Some(7));
    }

    /// The tracker on a real sim: a soldier running on flat ground steps
    /// once per half cycle of the run clip, feet alternating, with a walk
    /// step at the start and at the stop; a jump starts Jump, then Land.
    #[test]
    fn a_run_steps_every_half_cycle_and_a_jump_lands() {
        use core::sim::{buttons, PadState};
        let flat = core::heightfield::Heightfield {
            size: 33,
            unit: 8.0,
            offset_x: 128.0,
            offset_z: 128.0,
            heights: vec![0.0; 33 * 33],
        };
        let mut walk = WalkSim::new(42, Vec3::ZERO, Some(flat));
        walk.teams[0].classes[0].foley_class = 1;
        let slots = FoleyRow {
            class: 1,
            walk_left: 1,
            walk_right: 2,
            run_left: 3,
            run_right: 4,
            jump: 5,
            land: 6,
            ..FoleyRow::default()
        };
        let mut foley = Foley::new(Table::new(vec![FoleyGroup {
            name: crate::classdb::fnv1a_lowercase("terrain_foley"),
            rows: vec![slots],
        }]));
        let mut drive = |walk: &mut WalkSim, pad: PadState, ticks: u32, from: u32| {
            let mut heard: Vec<(u32, u32)> = Vec::new();
            for i in 0..ticks {
                walk.step_tick(&[pad]);
                let view = View::of(&walk.camera(), 16.0 / 9.0);
                foley.tick(walk, &view, |s| heard.push((from + i, s.hash)));
            }
            heard
        };
        let idle = PadState::default();
        let forward = PadState { ly: 32767, ..idle };
        let mut heard = drive(&mut walk, idle, 30, 0);
        heard.extend(drive(&mut walk, forward, 120, 30));
        let speed = walk.soldier().vel.length();
        heard.extend(drive(&mut walk, idle, 60, 150));
        // Standing made no sound; the first step is a walk step, on the
        // tick the stick moved, and the last one is a walk step again.
        assert_eq!(heard[0], (30, 1), "{heard:?}");
        assert!(heard.last().unwrap().0 >= 150 && heard.last().unwrap().1 <= 2);
        let runs: Vec<(u32, u32)> = heard.iter().copied().filter(|h| h.1 >= 3).collect();
        assert!(runs.len() >= 8, "{heard:?}");
        assert!(runs.windows(2).all(|w| w[0].1 != w[1].1), "{runs:?}");
        // One step per 2.007 m at the soldier's speed.
        let per_step = (runs.last().unwrap().0 - runs[0].0) as f32 / (runs.len() - 1) as f32;
        let want = STRIDE_RUN_FORWARD / speed / core::TICK_DT;
        assert!(
            (per_step - want).abs() < 0.5,
            "{per_step} ticks, want {want}"
        );

        let jump = PadState {
            buttons: buttons::JUMP,
            ..idle
        };
        let mut heard = drive(&mut walk, jump, 2, 210);
        heard.extend(drive(&mut walk, idle, 90, 212));
        let kinds: Vec<u32> = heard.iter().map(|h| h.1).collect();
        assert_eq!(kinds, vec![5, 6], "{heard:?}");
    }

    /// §4.1: with drawn feet that never come under `MinFootHeight` (a
    /// floating clip: the Emperor's sprint) a run makes no step, the start's
    /// and stop's included; feet that touch down step as before; a jump still
    /// lands.
    #[test]
    fn a_floating_soldier_makes_no_steps() {
        use core::sim::{buttons, PadState};
        let flat = core::heightfield::Heightfield {
            size: 33,
            unit: 8.0,
            offset_x: 128.0,
            offset_z: 128.0,
            heights: vec![0.0; 33 * 33],
        };
        let mut walk = WalkSim::new(42, Vec3::ZERO, Some(flat));
        walk.teams[0].classes[0].foley_class = 1;
        let slots = FoleyRow {
            class: 1,
            walk_left: 1,
            walk_right: 2,
            run_left: 3,
            run_right: 4,
            jump: 5,
            land: 6,
            ..FoleyRow::default()
        };
        let mut foley = Foley::new(Table::new(vec![FoleyGroup {
            name: crate::classdb::fnv1a_lowercase("terrain_foley"),
            rows: vec![slots],
        }]));
        let player = walk.player.index as usize;
        // `feet(tick)`: the player's drawn foot heights that tick.
        let mut drive =
            |walk: &mut WalkSim, pad: PadState, ticks: u32, feet: &dyn Fn(u32) -> [f32; 2]| {
                let mut heard: Vec<u32> = Vec::new();
                for i in 0..ticks {
                    walk.step_tick(&[pad]);
                    let mut heights = vec![None; walk.sim.units.len()];
                    heights[player] = Some(feet(i));
                    foley.feel(walk, &heights);
                    let view = View::of(&walk.camera(), 16.0 / 9.0);
                    foley.tick(walk, &view, |s| heard.push(s.hash));
                }
                heard
            };
        let idle = PadState::default();
        let forward = PadState { ly: 32767, ..idle };
        let float = |_: u32| [0.54f32, 0.6];
        let mut heard = drive(&mut walk, idle, 30, &float);
        heard.extend(drive(&mut walk, forward, 120, &float));
        heard.extend(drive(&mut walk, idle, 60, &float));
        assert!(heard.is_empty(), "{heard:?}");

        // A running clip: one foot under 0.17 m in each half cycle.
        let touch = |i: u32| match i % 9 {
            0..=2 => [0.13f32, 0.4],
            3..=5 => [0.4, 0.13],
            _ => [0.4, 0.4],
        };
        let heard = drive(&mut walk, forward, 120, &touch);
        assert!(heard.iter().filter(|&&h| h >= 3).count() >= 8, "{heard:?}");

        // Floating again: the stop's walk step may still sound (a foot was
        // down within the window), then a jump is only Jump and Land.
        let mut heard = drive(&mut walk, idle, 60, &float);
        heard.retain(|&h| h > 4);
        let jump = PadState {
            buttons: buttons::JUMP,
            ..idle
        };
        heard.extend(drive(&mut walk, jump, 2, &float));
        heard.extend(drive(&mut walk, idle, 90, &float));
        assert_eq!(heard, vec![5, 6]);
    }

    /// The baked bodies (skips without them): the Emperor's sprint clip keeps
    /// both feet over `MinFootHeight` in every frame, and so does Vader's,
    /// while an Alliance soldier's run and sprint and a Jedi's sprint put a
    /// foot under it.
    #[test]
    fn the_emperors_sprint_floats_and_a_soldiers_run_touches_down() {
        let root = crate::assets::baked_root().join("side_bf2/soldiers");
        if !root.join("imp_inf_darthsidious.glb").is_file() {
            eprintln!("SKIP: baked BF2 soldiers required");
            return;
        }
        let floor = |body: &str, clip: &str| {
            let a = crate::assets::load_skinned_glb(&root.join(format!("{body}.glb"))).unwrap();
            let c = &a.clips[crate::animator::find_clip(&a.clips, clip).unwrap()];
            let feet = ["bone_l_foot", "bone_r_foot"].map(|b| a.skeleton.index_of(b).unwrap());
            let mut low = f32::MAX;
            for f in 0..c.frames {
                let pal = a.skeleton.palette(&c.sample_frame(&a.skeleton, f).locals());
                for b in feet {
                    let y = (pal[b] * a.skeleton.global_bind[b])
                        .transform_point3(Vec3::ZERO)
                        .y;
                    low = low.min(y);
                }
            }
            low
        };
        let sprint = floor("imp_inf_darthsidious", "sidious_sabre_sprint_full");
        assert!(sprint > 0.5, "{sprint}");
        let vader = floor("imp_inf_darthvader", "vader_sabre_sprint_full");
        assert!(vader > 0.3, "{vader}");
        for (body, clip) in [
            ("all_inf_soldier", "human_rifle_stand_runforward"),
            ("all_inf_soldier", "human_rifle_sprint_full"),
            ("rep_inf_ep3obiwan", "human_sabre_sprint_full"),
        ] {
            let low = floor(body, clip);
            assert!(low < MIN_FOOT_HEIGHT, "{body} {clip}: {low}");
        }
    }

    /// §4.4, BF1: forward 5.5 / 5.0, right 5.0 / 5.5, backward and left
    /// always the run clip (live: left at 4.4 m/s run, right at 4.5 walk).
    #[test]
    fn bf1_gait_has_two_always_run_directions() {
        let idle = Clip::IDLE;
        let run = |d| Clip { dir: d, run: true };
        let walk = |d| Clip { dir: d, run: false };
        assert_eq!(clip_bf1(0.2, -0.2, idle), idle);
        // Forward: over 5.5 to start, over 5.0 to stay.
        assert_eq!(clip_bf1(5.4, 0.0, idle), walk(Dir::Forward));
        assert_eq!(clip_bf1(5.6, 0.0, idle), run(Dir::Forward));
        assert_eq!(clip_bf1(5.4, 0.0, run(Dir::Forward)), run(Dir::Forward));
        assert_eq!(clip_bf1(4.9, 0.0, run(Dir::Forward)), walk(Dir::Forward));
        // Left and backward: the run clip at any speed.
        assert_eq!(clip_bf1(0.0, -4.4, idle), run(Dir::Left));
        assert_eq!(clip_bf1(0.0, -1.0, idle), run(Dir::Left));
        assert_eq!(clip_bf1(-1.0, 0.0, idle), run(Dir::Backward));
        // Right: a walk at 4.5; over 5.0 runs, unless already walking right.
        assert_eq!(clip_bf1(0.0, 4.5, idle), WALK);
        assert_eq!(clip_bf1(0.0, 5.2, idle), run(Dir::Right));
        assert_eq!(clip_bf1(0.0, 5.2, WALK), WALK);
        assert_eq!(clip_bf1(0.0, 5.6, WALK), run(Dir::Right));
        // Faster than 8 m/s the chooser returns idle.
        assert_eq!(clip_bf1(8.5, 0.0, run(Dir::Forward)), idle);
    }

    /// §4.4, BF2: run above 0.78 x MaxSpeed, 0.71 to stay, in any direction.
    #[test]
    fn bf2_gait_is_a_fraction_of_max_speed() {
        let idle = Clip::IDLE;
        let c = |fwd, side, prev| clip_bf2(fwd, side, 7.0, prev);
        assert_eq!(c(0.1, 0.1, idle), idle);
        let walk = c(5.2, 0.0, idle);
        assert_eq!((walk.dir, walk.run), (Dir::Forward, false));
        let run = c(5.6, 0.0, idle);
        assert!(run.run);
        // 5.2 = 0.743 x 7: stays a run, does not start one.
        assert!(c(5.2, 0.0, run).run);
        assert!(!c(4.9, 0.0, run).run);
        // Left at 4.4 walks here - BF1's always-run is not BF2's rule.
        let left = c(0.0, -4.4, idle);
        assert_eq!((left.dir, left.run), (Dir::Left, false));
        assert!(c(0.0, -5.6, idle).run && c(-5.6, 0.0, idle).run);
        // 0.25 m/s: idle from rest, still moving once moving.
        assert_eq!(c(0.25, 0.0, idle).dir, Dir::Idle);
        assert_eq!(c(0.25, 0.0, walk).dir, Dir::Forward);
    }
}
