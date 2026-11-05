//! Card TPOS — the POSITION TABLE (`analysis/bot-design.md` 4.1): every place
//! on a map a soldier might stand to fight, with what can be known about its
//! cover without looking again. Built once when the posts are in
//! ([`Sim::build_positions`]), a pure function of the map — its graph, its
//! hints, its barriers, its floors and bodies — so DERIVED and not hashed.
//!
//! Sources (OURS; the census that sized them is card BOTD's,
//! `lead_tmp/botd/tools/cover_census.patch`, `analysis/bot-design.md` M6):
//! the authored Cover and Snipe hints (the PS2's own, re-soldier-ai 3), the
//! points [`EDGE_OUT`] outside each soldier barrier's four edge middles and
//! four corners (the PS2's bots stand within a metre of a barrier a quarter of
//! their time, M3), and each hub's centre with 8 points at [`HUB_RING`] of its
//! radius. A point is kept when a soldier can stand there ([`Builder::stand`]):
//! a floor on its storey or the open terrain, dry, on the level, in no soldier
//! barrier, head room.
//!
//! Per position: the floor point, its source, the hint's facing and stances,
//! the nearest hub on its storey, and two masks over [`DIRS`] directions
//! (bit `k` = the direction `(cos a, 0, sin a)`, `a = k / 16` of a turn):
//! LOW cover (something shut at crouch-eye height within [`NEAR`], the lane
//! open at stand-eye height for [`LANE`]: crouch behind it, stand to fire) and
//! CORNER cover (shut at both heights, a [`LEAN`] step to a side has the
//! lane: lean to fire). The rays are the bolt's (terrain + the map's own
//! bodies, no vehicle hull), as perception's.
//!
//! The TACTICIAN (`analysis/bot-design.md` 4.6, [`fight`], [`choose`]) gives
//! a fighting bot one position of the table: within 40 m on its storey, the
//! 32 nearest with cover from the target inside the band and the 32 nearest
//! within 8 m; yes / no filters (reserved or within [`SPACING`] of a
//! reserved one, a body on it, a friend's line of fire, a grenade, a death
//! region, over [`BACKWARD`] farther from its objective, outside the
//! weapon's band grown [`BAND_GROW`]); three tries (cover from the target -
//! something shut at crouch height that way within [`NEAR`], the line to
//! fire along confirmed by the ray, `Position::shields_toward`, within
//! [`COVER_REACH`]; any within 8 m; none = the bot's own FGHT style), each first with the team's
//! positions and bodies [`SPREAD`] away (a grenade's blast, the maintainer's KAM1
//! bunching) and then without; a score of cover with a line (3),
//! nearness (2), cover from its other contacts (1) and its own bias (0.5);
//! one confirming ray from the best down. Kept while reserved, covered as it
//! was and in the band (re-scored every [`RESCORE`], the PS2's 5 s) and not
//! hit twice; from an open one a covered one is asked for every 2 s; at it the PS2's hide / fire cycle
//! (hidden 2-4 s, firing 2-6 s; re-soldier-ai 4.6) and another position
//! after 5-12 s - on every think, the target seen or remembered ([`Foe`],
//! card TPOSR). RESERVATIONS: a position is the bot's from the write-back
//! ([`grant`], unit-index order), renewed every tick it is held ([`keep`]),
//! to [`FREE_AFTER`] after it is left. The
//! weights and times are OURS and PROVISIONAL. Not yet: Capture / Guard /
//! Overwatch at positions, a turret seat as a position (card TPOS's next
//! slice).

mod cover;
pub use cover::{sector_bit, sector_pair};

use crate::fmath;
use crate::nav;
use crate::sim::{Level, Sim};
use glam::{Vec2, Vec3};

/// The sector masks' directions.
pub const DIRS: usize = 16;
/// LOW / CORNER cover: something shut within this of the position (m).
pub const NEAR: f32 = 3.0;
/// ...and a lane this long open to fire along (m).
pub const LANE: f32 = 30.0;
/// The eye heights over the floor: crouched and standing (m).
pub const CROUCH_EYE: f32 = 0.9;
pub const STAND_EYE: f32 = 1.6;
/// A corner's lean step (m).
pub const LEAN: f32 = 1.0;
/// A barrier's edge points lie this far outside it (m).
pub const EDGE_OUT: f32 = 0.9;
/// A hub's ring points lie at this share of its radius.
pub const HUB_RING: f32 = 0.7;
/// A point is in a barrier when within this of it (m): the census's test.
const IN_BARRIER: f32 = 0.3;
/// An authored hint within [`IN_BARRIER`] of a soldier barrier is moved out
/// of it by at most this (m) rather than dropped: Polis Massa's Cover hints
/// are authored against the barriers round its props (172 of 210 were
/// dropped, 112 of them outside the barrier proper), and the PS2's bots fight
/// from them 48.6 % of their attack time (re-soldier-ai 4.6).
const HINT_PUSH: f32 = 0.5;
/// A floor this far from the storey asked for is another storey (m).
const STOREY: f32 = 1.5;
/// Head room over the floor (m), asked from 0.3 m up.
const HEAD_ROOM: f32 = 1.5;
/// A barrier's edge point takes its storey from the nearest hub within this.
const EDGE_HUB: f32 = 40.0;
/// The grid cell of the table's index (m).
const CELL: f32 = 8.0;

/// Where a position came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Source {
    /// An authored Cover hint (kind 2).
    Cover = 0,
    /// An authored Snipe hint (kind 0).
    Snipe = 1,
    /// A hub's centre or one of its ring points.
    Hub = 2,
    /// A point outside a soldier barrier's edge or corner.
    Edge = 3,
}

impl Source {
    pub const ALL: [Source; 4] = [Source::Cover, Source::Snipe, Source::Hub, Source::Edge];
    pub fn name(self) -> &'static str {
        match self {
            Source::Cover => "cover hints",
            Source::Snipe => "snipe hints",
            Source::Hub => "hub points",
            Source::Edge => "barrier edges",
        }
    }
}

/// One standable place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    /// The floor point a soldier stands on.
    pub pos: Vec3,
    pub source: Source,
    /// A hint's authored facing (its `rows[2]`, XZ, unit); zero for the rest.
    pub facing: Vec2,
    /// A hint's primary and secondary stance masks (0 = none authored).
    pub stance: u8,
    pub stance2: u8,
    /// The nearest hub on this storey ([`u16::MAX`] = none).
    pub hub: u16,
    /// LOW cover per direction (module header).
    pub low: u16,
    /// CORNER cover per direction.
    pub corner: u16,
    /// Per direction, something shut at crouch-eye height within [`NEAR`]
    /// (what hides a crouched soldier, whatever lies beyond it), and of
    /// those the ones shut at stand-eye height too (fired round, not over).
    pub shut: u16,
    pub tall: u16,
}

/// What the build did, per source: points tried, standable, covered.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TableStats {
    pub tried: [u32; 4],
    pub standable: [u32; 4],
    pub covered: [u32; 4],
    pub rays: u64,
    /// Wall-clock seconds the build took (diagnostic; not deterministic).
    pub secs: f64,
}

/// The map's positions, indexed by an [`CELL`] m grid.
#[derive(Debug, Clone, Default)]
pub struct PositionTable {
    /// Sorted by grid cell (x, then z), then source order.
    pub positions: Vec<Position>,
    /// Per occupied cell, the run of `positions` in it.
    cells: std::collections::BTreeMap<(i32, i32), (u32, u32)>,
    pub stats: TableStats,
}

fn cell_of(x: f32, z: f32) -> (i32, i32) {
    ((x / CELL).floor() as i32, (z / CELL).floor() as i32)
}

/// The build's state: the map, and the rays it has cast.
struct Builder<'a> {
    sim: &'a Sim,
    g: &'a nav::NavGraph,
    rays: u64,
}

impl Builder<'_> {
    /// Is the segment open — no terrain, no map body (no vehicle hull)?
    fn clear(&mut self, from: Vec3, dir: Vec3, len: f32) -> bool {
        self.rays += 1;
        self.sim.terrain_ray(from, dir, len, 1.0).is_none()
            && !self.sim.collision.ray_blocked(
                from,
                dir,
                len,
                crate::collision::Layers::ORDNANCE,
                crate::collision::HullFilter::Static,
                [None; 2],
            )
    }

    /// The floor a soldier stands on at (x, z) on the storey of `y_ref`, when
    /// one can: else the open terrain's when nothing is built over it; dry, on
    /// the level, in no soldier barrier, with head room.
    fn stand(&mut self, x: f32, z: f32, y_ref: f32) -> Option<f32> {
        let sim = self.sim;
        let mut f = sim.ground_point_under(Vec3::new(x, y_ref + 1.0, z), false);
        if f <= crate::collision::NO_GROUND + 1.0 || (f - y_ref).abs() > STOREY {
            let t = sim.heightfield.as_ref().map(|h| h.height_at(x, z))?;
            f = sim.ground_point_under(Vec3::new(x, t + 0.2, z), false);
            if (f - t).abs() > 0.3 {
                return None;
            }
        }
        if sim
            .still_water_under(Vec3::new(x, f, z))
            .is_some_and(|w| f < w - 0.2)
        {
            return None;
        }
        if matches!(sim.level_at(Vec3::new(x, f, z)), Level::Off(_)) {
            return None;
        }
        let p = Vec2::new(x, z);
        if self
            .g
            .barriers
            .iter()
            .any(|b| b.enabled && b.stops(nav::SOLDIER) && b.contains_r(p, IN_BARRIER))
        {
            return None;
        }
        if !self.clear(Vec3::new(x, f + 0.3, z), Vec3::Y, HEAD_ROOM) {
            return None;
        }
        Some(f)
    }

    /// The (LOW, CORNER, shut, tall) masks at floor point `p`.
    fn sectors(&mut self, p: Vec3) -> (u16, u16, u16, u16) {
        let (mut low, mut corner, mut shut, mut tall) = (0u16, 0u16, 0u16, 0u16);
        let c = p + Vec3::Y * CROUCH_EYE;
        let s = p + Vec3::Y * STAND_EYE;
        for k in 0..DIRS {
            let (sa, ca) = fmath::sin_cos(k as f32 / DIRS as f32 * std::f32::consts::TAU);
            let d = Vec3::new(ca, 0.0, sa);
            if self.clear(c, d, NEAR) {
                continue;
            }
            shut |= 1 << k;
            if self.clear(s, d, NEAR) {
                if self.clear(s, d, LANE) {
                    low |= 1 << k;
                }
            } else {
                tall |= 1 << k;
                let side = Vec3::new(-d.z, 0.0, d.x);
                for sg in [-1.0f32, 1.0] {
                    if self.clear(s, side * sg, LEAN) && self.clear(s + side * (LEAN * sg), d, LANE)
                    {
                        corner |= 1 << k;
                        break;
                    }
                }
            }
        }
        (low, corner, shut, tall)
    }

    /// `p` (XZ) moved out of every soldier barrier grown [`IN_BARRIER`] it is
    /// in, along the nearer face, when that is at most [`HINT_PUSH`] in all.
    fn out_of_barriers(&self, p: Vec2) -> Option<Vec2> {
        let m = IN_BARRIER + 0.05;
        let mut q = p;
        for _ in 0..3 {
            let Some(b) = self
                .g
                .barriers
                .iter()
                .find(|b| b.enabled && b.stops(nav::SOLDIER) && b.contains_r(q, IN_BARRIER))
            else {
                return (q.distance(p) <= HINT_PUSH).then_some(q);
            };
            let d = q - b.centre;
            let l = [d.dot(b.axes[0]), d.dot(b.axes[1])];
            let over = [b.half.x + m - l[0].abs(), b.half.y + m - l[1].abs()];
            let k = usize::from(over[1] < over[0]);
            q += b.axes[k] * (over[k] * if l[k] < 0.0 { -1.0 } else { 1.0 });
        }
        None
    }

    /// The floor of the hub nearest (x, z) in XZ within [`EDGE_HUB`].
    fn hub_floor_near(&self, x: f32, z: f32) -> Option<f32> {
        let mut best: Option<(f32, f32)> = None;
        for (i, h) in self.g.hubs.iter().enumerate() {
            let Some(f) = self.sim.nav_floor(i as u16) else {
                continue;
            };
            let d = Vec2::new(h.pos.x - x, h.pos.z - z).length();
            if d <= EDGE_HUB && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, f));
            }
        }
        best.map(|b| b.1)
    }

    /// The hub nearest `p` in XZ whose floor is on `p`'s storey.
    fn hub_of(&self, p: Vec3) -> u16 {
        let mut best: Option<(f32, u16)> = None;
        for (i, h) in self.g.hubs.iter().enumerate() {
            let Some(f) = self.sim.nav_floor(i as u16) else {
                continue;
            };
            if (f - p.y).abs() > super::HUB_LEVEL_TOL {
                continue;
            }
            let d = nav::xz_dist(h.pos, p);
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i as u16));
            }
        }
        best.map_or(u16::MAX, |b| b.1)
    }
}

/// Build the table for `sim`'s map (empty with no graph). Pure: the same
/// map gives the same table, point for point.
pub fn build(sim: &Sim) -> PositionTable {
    let t0 = std::time::Instant::now();
    let Some(g) = sim.nav.as_ref() else {
        return PositionTable::default();
    };
    let mut b = Builder { sim, g, rays: 0 };
    let mut stats = TableStats::default();
    let mut out: Vec<Position> = Vec::new();
    let mut push = |b: &mut Builder, stats: &mut TableStats, mut q: Position| {
        let (low, corner, shut, tall) = b.sectors(q.pos);
        q.low = low;
        q.corner = corner;
        q.shut = shut;
        q.tall = tall;
        q.hub = b.hub_of(q.pos);
        let s = q.source as usize;
        stats.standable[s] += 1;
        stats.covered[s] += u32::from(q.covered());
        out.push(q);
    };
    let blank = |pos: Vec3, source: Source| Position {
        pos,
        source,
        facing: Vec2::ZERO,
        stance: 0,
        stance2: 0,
        hub: u16::MAX,
        low: 0,
        corner: 0,
        shut: 0,
        tall: 0,
    };
    for h in &g.hints {
        let source = match h.kind {
            2 => Source::Cover,
            0 => Source::Snipe,
            _ => continue,
        };
        stats.tried[source as usize] += 1;
        let at = |b: &mut Builder, p: Vec2| {
            b.stand(p.x, p.y, h.pos.y)
                .or_else(|| {
                    b.hub_floor_near(p.x, p.y)
                        .and_then(|y| b.stand(p.x, p.y, y))
                })
                .map(|f| Vec3::new(p.x, f, p.y))
        };
        let xz0 = Vec2::new(h.pos.x, h.pos.z);
        let p = match at(&mut b, xz0) {
            Some(p) => Some(p),
            None => b
                .out_of_barriers(xz0)
                .filter(|q| *q != xz0)
                .and_then(|q| at(&mut b, q)),
        };
        let Some(p) = p else {
            continue;
        };
        let ahead = Vec2::new(h.rows[2][0], h.rows[2][2]);
        let mut q = blank(p, source);
        q.facing = ahead.try_normalize().unwrap_or(Vec2::ZERO);
        q.stance = h.stance_mask;
        q.stance2 = h.secondary_stance_mask;
        push(&mut b, &mut stats, q);
    }
    for (i, h) in g.hubs.iter().enumerate() {
        let Some(f0) = sim.nav_floor(i as u16) else {
            continue;
        };
        for k in 0..9 {
            let (dx, dz) = if k == 8 {
                (0.0, 0.0)
            } else {
                let (sa, ca) = fmath::sin_cos(k as f32 / 8.0 * std::f32::consts::TAU);
                (ca * h.radius * HUB_RING, sa * h.radius * HUB_RING)
            };
            stats.tried[Source::Hub as usize] += 1;
            let (x, z) = (h.pos.x + dx, h.pos.z + dz);
            if let Some(f) = b.stand(x, z, f0) {
                push(&mut b, &mut stats, blank(Vec3::new(x, f, z), Source::Hub));
            }
        }
    }
    for br in g
        .barriers
        .iter()
        .filter(|br| br.enabled && br.stops(nav::SOLDIER))
    {
        for (sx, sz) in [
            (1.0f32, 0.0f32),
            (-1.0, 0.0),
            (0.0, 1.0),
            (0.0, -1.0),
            (1.0, 1.0),
            (1.0, -1.0),
            (-1.0, 1.0),
            (-1.0, -1.0),
        ] {
            let grow = |s: f32, half: f32| s * (half + if s != 0.0 { EDGE_OUT } else { 0.0 });
            let p = br.centre + br.axes[0] * grow(sx, br.half.x) + br.axes[1] * grow(sz, br.half.y);
            stats.tried[Source::Edge as usize] += 1;
            let Some(y) = b.hub_floor_near(p.x, p.y) else {
                continue;
            };
            if let Some(f) = b.stand(p.x, p.y, y) {
                push(
                    &mut b,
                    &mut stats,
                    blank(Vec3::new(p.x, f, p.y), Source::Edge),
                );
            }
        }
    }
    stats.rays = b.rays;
    stats.secs = t0.elapsed().as_secs_f64();
    PositionTable::index(out, stats)
}

// ── The Tactician (`analysis/bot-design.md` 4.6; OURS, the weights and
// times PROVISIONAL): asked with a bot and the target it fights, it answers
// one position of the table or none — the candidates near the bot, yes / no
// filters, three tries (cover from the target; any; none = the caller's own
// fight from where it stands), a score of non-negative terms, one confirming
// ray from the best down. The answer is kept while its conditions hold; at
// it the bot runs the PS2's hide / fire cycle (re-soldier-ai 4.6) and looks
// for another position after 5-12 s.

/// A position stays reserved this long after it is left (ticks): the PS2's
/// hint rule.
pub const FREE_AFTER: u64 = 2 * crate::TICK_HZ as u64;
/// No two bots hold positions closer than this (m).
pub const SPACING: f32 = 2.0;
/// ...and a team's fighters hold them this far apart where the place allows
/// (m; the maintainer's KAM1 bunching, 2026-10-04: a grenade's blast apart) - BF2's
/// thermal detonator explosion's DamageRadiusInner (`com_weap_inf_
/// thermaldetonator_exp`, key 3738439158 = 4.5; Outer 6.5).
pub const SPREAD: f32 = 4.5;
/// Candidates within this of the bot (m; and within the weapon's range)...
pub const SEARCH: f32 = 40.0;
/// ...the nearest this many.
pub const CANDIDATES: usize = 32;
/// A position no more than this farther from the bot's objective than the
/// bot is (m): the PS2's band move.
pub const BACKWARD: f32 = 15.0;
/// The weapon's band is grown by this (m).
pub const BAND_GROW: f32 = 5.0;
/// Another unit on foot within this of a position stands on it (m).
const CROWD: f32 = 1.5;
/// A position within this of a friend's line of fire is in it (m).
const LINE: f32 = 1.0;
/// A Cover hint's arrow points at the target within 60 degrees.
const FACING_COS: f32 = 0.5;
/// The cycle (ticks): hidden 2-4 s, firing 2-6 s; another position after
/// 5-12 s (re-soldier-ai 4.6).
const HIDE: (u64, u64) = (60, 120);
const FIRE: (u64, u64) = (60, 180);
const NEXT: (u64, u64) = (150, 360);
/// A position more than this above or below the bot is walked onto only
/// when its floor is its hub's (m).
const CLIMB: f32 = 1.2;
/// A walk to a position is given until its straight length at this speed
/// (m/s) and this slack (s): a soldier runs 6-7 m/s.
const TAKE_SPEED: f32 = 3.0;
const TAKE_SLACK: f32 = 3.0;
/// A query that found nothing is not asked again for this long (ticks).
const RETRY: u64 = 2 * crate::TICK_HZ as u64;
/// Confirming rays from the best candidates down, at most.
const CONFIRM: usize = 3;
/// A position with no cover from the target (the second try) is taken only
/// this near (m): spacing in the open, not a walk...
const OPEN_REACH: f32 = 8.0;
/// ...and only on the bot's own surface, within a step of its height (m):
/// an open point on a ledge's lip was hopped at (the maintainer's Kamino scene).
const STEP: f32 = 0.6;
/// A walk to a position ends this near it (m), where the bot is...
const ARRIVE_NEAR: f32 = 1.0;
/// ...and one cut short (its deadline, the mover's give-up) this near and
/// within [`STEP`] of its height is arrived where the bot stopped (m; B12's
/// own radius, `covered_from_target`); an open position this near is taken
/// where the bot stands, with no walk.
pub const NEAR_ENOUGH: f32 = LEAN + 0.5;
/// On the last [`OPEN_REACH`] of a walk to a position, the walk comes this
/// much nearer (m) within [`GAIN_TICKS`] or it is cut short (`cut_short`): a
/// position walked round is left (bots slid along a pod's flank for the
/// whole deadline at a point on its lip, the maintainer's Kamino scene).
pub const GAIN: f32 = 0.3;
const GAIN_TICKS: u64 = crate::TICK_HZ as u64;
/// Another position, when one is left, is at least this far from it (m): a
/// move, not a twitch - bots hopped 0.6-1.5 m between neighbouring points,
/// 7-9 reversals in 10 s (the maintainer's Kamino scene, card TPOS). A target gone
/// round the cover may send it to any.
const MOVE_MIN: f32 = 3.0;
/// A position held is re-scored (the target round its cover, out of the
/// band) this often (ticks): the PS2's 5 s (re-soldier-ai 4.6). Asked on
/// every think, a walk to a covered position was re-picked whenever the
/// target's bearing left its 22.5-degree sector (card TPOS).
const RESCORE: u64 = 5 * crate::TICK_HZ as u64;
/// A position with cover from the target is walked to only this far (m):
/// with the farthest of the box's 40 m the fight changed before the bot got
/// there (the maintainer's Kamino scene went to 1 bot over 6 reversals).
const COVER_REACH: f32 = 15.0;
/// Card HYST: the position held is re-scored beside the candidates of a
/// query that may replace it (the 5-12 s timer, the open position's ask for
/// cover), and another is taken only when it scores this share more; a tie
/// keeps it. OURS (`lead_tmp/botre/modern.md` B1: 10-20 %).
pub const SWITCH_MARGIN: f32 = 0.15;
/// Card HYST: a walk to a position that closes on the target by more than
/// this (m) and this share of the bot's distance to it, heading at it (the
/// closing over half the walk), is not taken while the bot is inside the
/// band already - it would run at the enemy in the open to reach its cover
/// (The Last of Us, `modern.md` B2). OURS: 4 m alone took 1 in 7 covered
/// picks and lowered the cover share (card HYST's sweep); with the share it
/// bites in the close fights only.
pub const CLOSING: f32 = 4.0;
pub const CLOSING_SHARE: f32 = 0.4;

/// The fight-time split's events (`NavTick::tpos_ev`, one bit each): the
/// query's answer...
pub const EV_PICK_COVERED: u16 = 1 << 0;
pub const EV_PICK_OPEN: u16 = 1 << 1;
pub const EV_PICK_NONE: u16 = 1 << 2;
/// ...why the position held is left (the target round its cover, hit twice,
/// out of the band, the 5-12 s timer)...
pub const EV_LEAVE_ROUND: u16 = 1 << 3;
pub const EV_LEAVE_HIT: u16 = 1 << 4;
pub const EV_LEAVE_BAND: u16 = 1 << 5;
pub const EV_LEAVE_TIMER: u16 = 1 << 6;
/// ...the timer's query found nothing and the position is kept...
pub const EV_KEEP_TIMER: u16 = 1 << 7;
/// ...and how a walk to one ends: there, short but near enough, failed by
/// its deadline, by no progress, by the mover's give-up, dropped (the fight
/// over, a vehicle for a target, the endgame); an open one taken in place.
pub const EV_WALK_ARRIVED: u16 = 1 << 8;
pub const EV_WALK_NEAR: u16 = 1 << 9;
pub const EV_WALK_DEADLINE: u16 = 1 << 10;
pub const EV_WALK_STALLED: u16 = 1 << 11;
pub const EV_WALK_GIVEN_UP: u16 = 1 << 12;
pub const EV_WALK_DROPPED: u16 = 1 << 13;
pub const EV_IN_PLACE: u16 = 1 << 14;
/// ...and a bot at an open position that found a covered one.
pub const EV_UPGRADE: u16 = 1 << 15;

/// Card HYST: what this think's Tactician did about the position held
/// (`NavTick::tpos_hyst`): another taken in its place (a flip), or kept
/// because no candidate beat it by [`SWITCH_MARGIN`].
pub const HY_FLIP: u8 = 1 << 0;
pub const HY_KEPT: u8 = 1 << 1;

/// The names, in bit order (the sweep's `tactician` line).
pub const EV_NAMES: [&str; 16] = [
    "pick-covered",
    "pick-open",
    "pick-none",
    "leave-round",
    "leave-hit",
    "leave-band",
    "leave-timer",
    "keep-timer",
    "walk-arrived",
    "walk-near",
    "walk-deadline",
    "walk-stalled",
    "walk-given-up",
    "walk-dropped",
    "in-place",
    "upgrade",
];

use super::behave::Tick;
use super::draw::{unit_unit, SALT_TPOS};
use super::state::{AiState, FightState, HoldState, NavTick};
use crate::unit::Unit;

/// The Tactician's answer: a table index, whether it has cover from the
/// target, and a CORNER position's lean side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Pick {
    pub idx: u32,
    pub covered: bool,
    pub lean: i8,
}

/// What the Tactician fights (card TPOSR): the target where it is seen, or
/// where it was last known - the PS2's cover state machine runs on every
/// update whether the target is seen or not, its walk and hidden states
/// keeping the target (`FUN_002ce4e8`: tracker+0x38 = FLT_MAX; AUDIT TPOS).
/// A remembered target is not peeked at: its last known place, standing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Foe {
    /// Its feet.
    pub at: Vec3,
    /// The point the confirming ray is cast at.
    pub chest: Vec3,
    /// In a vehicle (the Tactician's positions are for a soldier).
    pub mounted: bool,
    /// Seen on this think. The re-score (the target round the cover, out of
    /// the band) waits for a think it is seen: the PS2's tracker holds the
    /// target itself, the port's memory only a place it has left (re-scored
    /// against it, a bot whose target came round its cover waited out the
    /// next 5 s in view of it).
    pub seen: bool,
}

impl Foe {
    pub fn seen(e: &Unit) -> Self {
        Foe {
            at: e.soldier.pos,
            chest: e.muzzle(),
            mounted: e.mount.is_some(),
            seen: true,
        }
    }
    pub fn remembered(at: Vec3) -> Self {
        Foe {
            at,
            chest: at + Vec3::Y * crate::unit::muzzle_height(crate::soldier::Stance::Stand),
            mounted: false,
            seen: false,
        }
    }
}

/// A draw in `lo..hi` ticks.
fn span(me: crate::unit::UnitHandle, tick: u64, salt: u32, (lo, hi): (u64, u64)) -> u64 {
    lo + ((hi - lo) as f32 * unit_unit(me.index, tick, me.gen, SALT_TPOS ^ salt)) as u64
}

/// The weapon's band about its target, grown by [`BAND_GROW`] (m): inner =
/// min(optimal, min + 3), outer = min(max(optimal, range - 10), view - 10)
/// (re-soldier-ai 4.5). A scoped rifle's view is its scope's at the tier
/// (`perception::scope_reach`): a sniper fighting across a map is in its band.
fn band(sim: &Sim, tk: &Tick) -> (f32, f32) {
    let [lo, opt, max] = tk.wc.ai_range;
    let view = crate::perception::view_distance(crate::soldier::Stance::Stand, tk.tier.view_mult)
        .max(
            crate::perception::scope_reach(
                sim,
                tk.wc,
                crate::perception::scope_share(tk.u, &tk.tier),
            )
            .unwrap_or(0.0),
        );
    (
        opt.min(lo + 3.0) - BAND_GROW,
        opt.max(max - 10.0).min(view - 10.0) + BAND_GROW,
    )
}

fn xz(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

/// The point a bot standing at `at` fires from toward `aim`: its standing
/// eye, a CORNER position's a [`LEAN`] step to `lean`'s side.
fn firing_eye(at: Vec3, aim: Vec3, lean: i8) -> Vec3 {
    let d = xz(aim - at).normalize_or_zero();
    let side = Vec3::new(-d.y, 0.0, d.x) * (LEAN * f32::from(lean));
    at + side + Vec3::Y * crate::unit::muzzle_height(crate::soldier::Stance::Stand)
}

/// Is the line from `from` to `to` open (terrain and the map's bodies)?
fn line_open(sim: &Sim, from: Vec3, to: Vec3) -> bool {
    crate::perception::los_terrain(sim, from, to) && crate::perception::los_world(sim, from, to)
}

/// The Tactician: the position for bot `tk` fighting `foe`. `leaving` =
/// the one it leaves, left out, and with it every position within
/// [`MOVE_MIN`] of it unless `.1` (the target went round its cover).
/// `cover_only_tries` = only the tries with cover from the target (a bot at
/// an open position asking for better). `keep` = the position held, still
/// good (card HYST): re-scored as it stands and answered back unless the
/// winner beats it by [`SWITCH_MARGIN`]. `None` = nothing passes.
#[allow(clippy::too_many_arguments)]
pub(super) fn choose(
    sim: &Sim,
    tk: &Tick,
    st: &AiState,
    foe: &Foe,
    leaving: Option<(u32, bool)>,
    cover_only_tries: bool,
    keep: Option<Pick>,
    nt: &mut NavTick,
) -> Option<Pick> {
    nt.tpos_query = 1;
    let t = &sim.positions;
    if t.is_empty() {
        return None;
    }
    let Tick { me, u, wc, .. } = *tk;
    let pos = u.soldier.pos;
    let aim = foe.at;
    let chest = foe.chest;
    let (inner, outer) = band(sim, tk);
    let mut near = Vec::new();
    t.within(
        pos,
        SEARCH.min(wc.ai_range[2].max(inner + BAND_GROW)),
        &mut near,
    );
    // On the bot's storey, and walkable onto: a floor more than a step off
    // both the bot's and its hub's (a hint on top of a crate) is jumped at.
    let walkable = |q: &Position| {
        let dy = (q.pos.y - pos.y).abs();
        dy <= super::HUB_LEVEL_TOL
            && (dy <= CLIMB
                || sim
                    .nav_floor(q.hub)
                    .is_some_and(|f| (q.pos.y - f).abs() <= CLIMB))
    };
    // Cover from the target: something shut at crouch height that way, or a
    // Cover hint by the PS2's rule (re-soldier-ai 4.6) - facing the target
    // and on the bot's side of it, (H - T).(T - me) < 0.
    let covers = |q: &Position| {
        let d = xz(aim - q.pos);
        q.shut & sector_pair(d) != 0 || (q.hint_faces(d) && (-d).dot(xz(aim - pos)) < 0.0)
    };
    let funnel = |near: &[u32]| {
        near.iter()
            .filter(|&&i| covers(&t.positions[i as usize]))
            .count()
            .min(255) as u8
    };
    nt.tpos_funnel[0] = funnel(&near);
    let exclude = leaving.map(|l| l.0);
    let away = leaving
        .filter(|l| !l.1)
        .and_then(|l| t.positions.get(l.0 as usize))
        .map(|q| q.pos);
    near.retain(|&i| {
        let q = &t.positions[i as usize];
        Some(i) != exclude
            && !st.tpos.bad.contains(&Some(i))
            && walkable(q)
            && away.is_none_or(|a| nav::xz_dist(q.pos, a) >= MOVE_MIN)
    });
    let dist = |i: u32| nav::xz_dist(t.positions[i as usize].pos, pos);
    near.sort_by(|a, b| dist(*a).total_cmp(&dist(*b)).then(a.cmp(b)));
    nt.tpos_funnel[1] = funnel(&near);
    // The nearest CANDIDATES with cover from the target inside the band, and
    // the nearest CANDIDATES within the open try's reach - the PS2 takes 30
    // Cover hints of its box (re-soldier-ai 4.6). The 32 nearest of every
    // kind held 5.0 of Mygeeto's 42.6 covered candidates a query: the hub
    // points round the bot crowded them out (card TPOS).
    {
        let in_band = |i: u32| {
            let d = nav::xz_dist(t.positions[i as usize].pos, aim);
            d >= inner && d <= outer
        };
        let (mut cov, mut open) = (0, 0);
        near.retain(|&i| {
            let keep_cov = covers(&t.positions[i as usize]) && in_band(i) && cov < CANDIDATES;
            let keep_open = dist(i) <= OPEN_REACH && open < CANDIDATES;
            cov += usize::from(keep_cov);
            open += usize::from(keep_open && !keep_cov);
            keep_cov || keep_open
        });
    }
    nt.tpos_funnel[2] = funnel(&near);
    // Reserved points, the bodies standing about, the friends' lines.
    // (point, a teammate's)
    let mut taken: Vec<(Vec3, bool)> = Vec::new();
    let mut bodies: Vec<(Vec3, bool)> = Vec::new();
    let mut lines: Vec<(Vec3, Vec3)> = Vec::new();
    for (h, o) in sim.alive_units() {
        if h == me {
            continue;
        }
        let ts = &o.ai.tpos;
        for (i, until) in [(ts.held, ts.until), (ts.prev, ts.prev_until)] {
            if let Some(q) = i
                .filter(|_| until > sim.tick)
                .and_then(|i| t.positions.get(i as usize))
            {
                taken.push((q.pos, o.team == u.team));
            }
        }
        if o.mount.is_none() && nav::xz_dist(o.soldier.pos, pos) <= SEARCH + CROWD {
            bodies.push((o.soldier.pos, o.team == u.team));
            if o.team == u.team {
                if let Some(c) =
                    o.ai.target
                        .and_then(|th| crate::perception::contact_of(&o.ai, th))
                        .filter(|c| c.seen_now)
                {
                    lines.push((o.muzzle(), c.last_pos + Vec3::Y));
                }
            }
        }
    }
    let objective = st
        .target_cp
        .and_then(|c| sim.cps.get(c as usize))
        .map(|cp| cp.def.pos);
    let near_seg = |p: Vec3, (a, b): (Vec3, Vec3)| {
        let (a, b, p) = (xz(a), xz(b), xz(p));
        let ab = b - a;
        let k = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        (a + ab * k).distance(p) < LINE
    };
    // Card HYST (The Last of Us' rule): from inside the band, a walk that
    // closes on the target by over CLOSING (and CLOSING_SHARE of the way to
    // it) and mostly toward it runs at the
    // enemy in the open. (Outside the band the bot has to close.)
    let me_to_aim = nav::xz_dist(pos, aim);
    let closes = |p: Vec3| {
        let closing = me_to_aim - nav::xz_dist(p, aim);
        me_to_aim <= outer
            && closing > CLOSING.max(CLOSING_SHARE * me_to_aim)
            && closing > 0.5 * nav::xz_dist(pos, p)
    };
    // The first filter position `i` fails (1 ..= 8), else 0.
    let fails = |i: u32| -> usize {
        let q = &t.positions[i as usize];
        let p = q.pos;
        let d = nav::xz_dist(p, aim);
        if !(d >= inner && d <= outer) {
            1
        } else if taken.iter().any(|r| nav::xz_dist(r.0, p) < SPACING) {
            2
        } else if bodies.iter().any(|b| nav::xz_dist(b.0, p) < CROWD) {
            3
        } else if lines.iter().any(|l| near_seg(p, *l)) {
            4
        // (No position the tries can take fails this while [`COVER_REACH`]
        // and [`OPEN_REACH`] are within [`BACKWARD`] - the triangle
        // inequality; it counts in the funnel and binds a longer reach.)
        } else if !objective.is_none_or(|o| nav::xz_dist(p, o) <= nav::xz_dist(pos, o) + BACKWARD) {
            5
        } else if sim
            .hazards
            .iter()
            .any(|z| nav::xz_dist(z.pos, p) < z.radius)
            || sim.death_regions.iter().any(|r| r.contains(p + Vec3::Y))
        {
            6
        // The walk there crosses no soldier barrier (the design's
        // segment the Tactician validated, 4.7): a position behind one
        // was walked at and failed, again and again (the maintainer's Kamino
        // scene, card TPOS).
        } else if !sim
            .nav
            .as_ref()
            .is_none_or(|g| g.barrier_stop(xz(pos), xz(p), nav::SOLDIER) >= 1.0)
        {
            7
        } else if closes(p) {
            8
        } else {
            0
        }
    };
    for &i in &near {
        if covers(&t.positions[i as usize]) {
            let f = fails(i);
            if f != 0 {
                // (The closing filter counts after the funnel's old slots.)
                let k = if f == 8 { 14 } else { 2 + f };
                nt.tpos_funnel[k] = nt.tpos_funnel[k].saturating_add(1);
            }
        }
    }
    near.retain(|&i| fails(i) == 0);
    nt.tpos_funnel[10] = funnel(&near);
    let others: Vec<Vec3> = st
        .contacts
        .iter()
        .flatten()
        .filter(|c| Some(c.handle) != st.target)
        .map(|c| c.last_pos)
        .collect();
    if near.iter().any(|&i| covers(&t.positions[i as usize])) {
        nt.tpos_query = 2;
    }
    // Card DFOCUS: a fellow focuser's bearing from the human it focuses.
    let fellows = st
        .target
        .and_then(|t| t.unit())
        .map_or_else(Vec::new, |h| crate::ai::dfocus::fellows(sim, me, h, aim));
    // A teammate's position or body within SPREAD, or (card DFOCUS) a
    // position on a fellow focuser's line of approach.
    let crowded = |p: Vec3| {
        let d = xz(p - aim).normalize_or_zero();
        taken
            .iter()
            .chain(&bodies)
            .any(|(r, mate)| *mate && nav::xz_dist(*r, p) < SPREAD)
            || fellows
                .iter()
                .any(|f| d.dot(*f) > crate::ai::dfocus::SPREAD_COS)
    };
    // The score: cover with a line (3), nearness (2), cover from the other
    // contacts (1), the bot's own bias (0.5) - each term held to 0..1 (card
    // HYST: an authored arrow's +0.2 no longer lifts LOW cover over 1).
    let score = |i: u32, cov: bool| -> f32 {
        let q = &t.positions[i as usize];
        let d = xz(aim - q.pos);
        let bit = sector_pair(d);
        let mut cover = 0.0;
        if cov {
            // A lane the table found: LOW over it, CORNER round it; else
            // only the confirming ray will say (crouch-shut only).
            cover = if q.low & bit != 0 {
                1.0
            } else if q.corner & bit != 0 {
                0.8
            } else {
                0.6
            };
            if q.source == Source::Cover && q.facing.dot(d.normalize_or_zero()) >= FACING_COS {
                cover += 0.2;
            }
        }
        let cover = f32::min(cover, 1.0);
        let near_term = (1.0 - nav::xz_dist(pos, q.pos) / SEARCH).clamp(0.0, 1.0);
        let other = if others.is_empty() {
            0.0
        } else {
            others
                .iter()
                .filter(|o| q.shields_toward(xz(**o - q.pos)))
                .count() as f32
                / others.len() as f32
        };
        let bias = unit_unit(me.index, u64::from(i), me.gen, SALT_TPOS);
        3.0 * cover + 2.0 * near_term + other + 0.5 * bias
    };
    // Card HYST: the position held, scored as it stands (the bot on it).
    let bar = keep.map(|k| {
        let held = &t.positions[k.idx as usize];
        score(k.idx, covers(held)) * (1.0 + SWITCH_MARGIN)
    });
    let tries: &[(bool, bool)] = if cover_only_tries {
        &[(true, true), (true, false)]
    } else {
        &[(true, true), (true, false), (false, true), (false, false)]
    };
    for &(cover_only, wide) in tries {
        let mut scored: Vec<(f32, u32, bool)> = Vec::new();
        for &i in &near {
            let q = &t.positions[i as usize];
            if wide && crowded(q.pos) {
                continue;
            }
            let travel = nav::xz_dist(pos, q.pos);
            let cov = covers(q);
            // In the open only a step away: spacing, not a walk.
            let reach = if cover_only { COVER_REACH } else { OPEN_REACH };
            if (cover_only && !cov)
                || travel > reach
                || (!cov && (q.pos.y - pos.y).abs() > STEP)
                || (travel <= OPEN_REACH && !no_drop(sim, pos, q.pos))
            {
                if cover_only && cov && !wide {
                    nt.tpos_funnel[11] = nt.tpos_funnel[11].saturating_add(1);
                }
                continue;
            }
            scored.push((score(i, cov), i, cov));
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        for &(sc, i, cov) in scored.iter().take(CONFIRM) {
            let q = &t.positions[i as usize];
            // Fired over (shut low, open at stand height) or round (tall).
            let bit = sector_pair(xz(aim - q.pos));
            let corner = cov && q.shut & bit != 0 && q.shut & !q.tall & bit == 0;
            let sides: &[i8] = if corner { &[1, -1] } else { &[0] };
            if let Some(&lean) = sides
                .iter()
                .find(|&&s| line_open(sim, firing_eye(q.pos, aim, s), chest))
            {
                // Card HYST: not by the margin - the position held stays.
                if let (Some(k), Some(bar)) = (keep, bar) {
                    if sc <= bar {
                        nt.tpos_hyst |= HY_KEPT;
                        return Some(k);
                    }
                }
                return Some(Pick {
                    idx: i,
                    covered: cov,
                    lean,
                });
            }
            if cover_only && !wide {
                nt.tpos_funnel[12] = nt.tpos_funnel[12].saturating_add(1);
            }
        }
        if cover_only && !wide {
            nt.tpos_funnel[13] = scored.len().min(255) as u8;
        }
    }
    None
}

/// The straight way from `from` to `to` drops nowhere more than a [`STEP`]
/// below both ends (the floor sampled at its quarters): a point on a pod's
/// lip, a step up from the bot on its ramp, was walked at off the ramp's
/// edge and slid under (the maintainer's Kamino scene, kam_bldg_pod2).
fn no_drop(sim: &Sim, from: Vec3, to: Vec3) -> bool {
    let (top, low) = (from.y.max(to.y), from.y.min(to.y) - STEP);
    (1..4).all(|k| {
        let s = from.lerp(to, k as f32 / 4.0);
        sim.ground_point_under(Vec3::new(s.x, top, s.z), false) >= low
    })
}

/// Leave the position held (if any): it stays reserved [`FREE_AFTER`].
pub(super) fn release(st: &mut AiState, tick: u64) {
    if let Some(i) = st.tpos.held.take() {
        st.tpos.prev = Some(i);
        st.tpos.prev_until = tick + FREE_AFTER;
    }
    if st.fight.tactician() {
        st.fight.mode = 0;
    }
}

/// The position held is not asked for again by this bot (the last four).
fn shun(st: &mut AiState) {
    if st.tpos.held.is_some() {
        st.tpos.bad.rotate_right(1);
        st.tpos.bad[0] = st.tpos.held;
    }
}

/// The walk to the position held failed (its way is shut): it is left, not
/// asked for again by this bot, and nothing is asked for [`RETRY`].
pub(super) fn failed(st: &mut AiState, tick: u64) {
    shun(st);
    release(st, tick);
    st.tpos.retry_at = tick + RETRY;
}

/// Take position `p`: walk to it over the graph (as a ring point), or be
/// there (an open one within [`NEAR_ENOUGH`]: where the bot stands).
fn take(sim: &Sim, tk: &Tick, st: &mut AiState, h: &mut HoldState, p: Pick, nt: &mut NavTick) {
    let now = sim.tick;
    if st.tpos.held != Some(p.idx) {
        if st.tpos.held.is_some() {
            nt.tpos_hyst |= HY_FLIP;
        }
        release(st, now);
    }
    let q = sim.positions.positions[p.idx as usize];
    st.tpos.held = Some(p.idx);
    st.tpos.at = now;
    st.tpos.rescore_at = now + RESCORE;
    st.tpos.until = now + FREE_AFTER;
    st.tpos.covered = p.covered;
    st.tpos.lean = p.lean;
    st.tpos.hits = 0;
    st.tpos.firing = false;
    h.spot = q.pos;
    h.goal = q.pos;
    h.stance_mask = 0;
    h.commit_until = now + super::HOLD_COMMIT;
    h.blocked = 0;
    let here = tk.u.soldier.pos;
    let travel = nav::xz_dist(here, q.pos);
    st.tpos.stand = q.pos;
    if travel <= super::HOLD_ARRIVE || (!p.covered && travel <= NEAR_ENOUGH) {
        if travel > super::HOLD_ARRIVE {
            st.tpos.stand = here;
        }
        nt.tpos_ev |= EV_IN_PLACE;
        arrive(sim, tk, st, h);
    } else {
        st.fight.mode = FightState::TAKE;
        st.tpos.best = travel;
        st.tpos.best_at = now;
        // (On the way, `phase_until` is the walk's deadline.)
        st.tpos.phase_until =
            now + ((travel / TAKE_SPEED + TAKE_SLACK) * crate::TICK_HZ as f32) as u64;
        super::route::retarget_board(sim, tk.me, st, q.pos, nt);
    }
}

/// The walk to the position held ended short of it (its deadline, the
/// mover's give-up, or [`ARRIVE_NEAR`] on a think): within [`NEAR_ENOUGH`]
/// and a [`STEP`] of its height it is arrived where the bot stands (`true`),
/// else the walk failed ([`failed`]).
pub(super) fn cut_short(sim: &Sim, tk: &Tick, st: &mut AiState, h: &mut HoldState) -> bool {
    let near = walk_arrived(sim, tk, st, h);
    if !near {
        failed(st, sim.tick);
    }
    near
}

/// The mover's walk ended at its goal: there, when it is within
/// [`NEAR_ENOUGH`] and a [`STEP`] of the position held (`true`) - else the
/// outcome is the walk's to the position before (a covered one taken on the
/// think the bot arrived at an open one), and the walk goes on (a bot held
/// 2.4 m under a ledge's point in its cycle, 17 reversals in 10 s: the maintainer's
/// Kamino scene).
pub(super) fn walk_arrived(sim: &Sim, tk: &Tick, st: &mut AiState, h: &mut HoldState) -> bool {
    let here = tk.u.soldier.pos;
    let near = st
        .tpos
        .held
        .and_then(|i| sim.positions.positions.get(i as usize))
        .is_some_and(|q| {
            nav::xz_dist(here, q.pos) <= NEAR_ENOUGH && (here.y - q.pos.y).abs() <= STEP
        });
    if near {
        st.tpos.stand = here;
        arrive(sim, tk, st, h);
    }
    near
}

/// At the position: its cycle starts hidden (firing at once from one with no
/// cover), and another is looked for after 5-12 s.
pub(super) fn arrive(sim: &Sim, tk: &Tick, st: &mut AiState, h: &mut HoldState) {
    let now = sim.tick;
    st.fight.mode = FightState::COVER;
    st.tpos.firing = !st.tpos.covered;
    st.tpos.phase_until = now + span(tk.me, now, 1, if st.tpos.firing { FIRE } else { HIDE });
    st.tpos.next_at = now + span(tk.me, now, 2, NEXT);
    cycle(sim, tk, st, h, None);
}

/// The hide / fire cycle at the position: LOW cover crouched, then standing
/// to fire; CORNER cover standing behind it, then a lean step out; no cover,
/// firing in the bot's own stance (`crouches`, its fixed style).
fn cycle(sim: &Sim, tk: &Tick, st: &mut AiState, h: &mut HoldState, aim: Option<Vec3>) {
    use crate::commander::{MASK_CROUCH, MASK_STAND};
    let now = sim.tick;
    if st
        .tpos
        .held
        .and_then(|i| sim.positions.positions.get(i as usize))
        .is_none()
    {
        return;
    }
    if st.tpos.covered && now >= st.tpos.phase_until {
        st.tpos.firing = !st.tpos.firing;
        let w = if st.tpos.firing { FIRE } else { HIDE };
        st.tpos.phase_until = now + span(tk.me, now, 1, w);
    }
    let at = st.tpos.stand;
    h.spot = at;
    h.goal = at;
    h.stance_mask = match (st.tpos.covered, st.tpos.lean) {
        (false, _) => h.stance_mask,
        (true, 0) if st.tpos.firing => MASK_STAND,
        (true, 0) => MASK_CROUCH,
        (true, _) => MASK_STAND,
    };
    if let (true, Some(aim), true) = (st.tpos.covered && st.tpos.lean != 0, aim, st.tpos.firing) {
        let eye = firing_eye(at, aim, st.tpos.lean);
        h.goal = Vec3::new(eye.x, at.y, eye.z);
    }
}

/// Card TPOS: the fight's position from the Tactician, this tick (`true`)
/// — or `false` for the caller's own rule (FGHT's stand or circle: no table,
/// a vehicle for a target, or nothing passed). Asked on the think, the
/// target seen or only remembered (card TPOSR: its cycle and leave tests run
/// all the same); kept while covered from the target as it was, inside the
/// band and not compromised (hit twice); another looked for 5-12 s after
/// arriving.
pub(super) fn fight(
    sim: &Sim,
    tk: &Tick,
    st: &mut AiState,
    foe: &Foe,
    h: &mut HoldState,
    thinking: bool,
    nt: &mut NavTick,
) -> bool {
    // The endgame (card HOLD's rule): a side down to its last few
    // reinforcements ends the round only by kills, and nobody waits in a
    // position any more (with positions, BF1 Tatooine 2's rounds ran 522-702
    // s at 4 v 0 alive where main's end by 420).
    let endgame = sim
        .battle
        .teams
        .iter()
        .any(|t| t.tickets() <= super::HOLD_ENDGAME);
    if sim.positions.is_empty() || foe.mounted || endgame {
        if st.fight.mode == FightState::TAKE {
            nt.tpos_ev |= EV_WALK_DROPPED;
        }
        if st.fight.tactician() {
            release(st, sim.tick);
        }
        return false;
    }
    let now = sim.tick;
    let on = st.fight.tactician() && st.tpos.held.is_some();
    if !thinking {
        return on;
    }
    let aim = foe.at;
    let mut leaving = None;
    let mut timer_only = false;
    // On the way: this near, it is there where it stands; a walk not done by
    // its deadline (a ledge it hops at, a way round that does not end) is
    // there if near enough, else failed: left, and not asked again.
    if on && st.fight.mode == FightState::TAKE {
        let here = tk.u.soldier.pos;
        let q = sim.positions.positions[st.tpos.held.unwrap_or(0) as usize];
        let d = nav::xz_dist(here, q.pos);
        if d + GAIN <= st.tpos.best {
            st.tpos.best = d;
            st.tpos.best_at = now;
        }
        let stalled = d <= OPEN_REACH && now > st.tpos.best_at + GAIN_TICKS;
        let late = now > st.tpos.phase_until;
        if d <= ARRIVE_NEAR && (here.y - q.pos.y).abs() <= STEP {
            st.tpos.stand = here;
            nt.tpos_ev |= EV_WALK_ARRIVED;
            arrive(sim, tk, st, h);
        } else if (late || stalled) && cut_short(sim, tk, st, h) {
            nt.tpos_ev |= EV_WALK_NEAR;
        } else if late || stalled {
            nt.tpos_ev |= if late {
                EV_WALK_DEADLINE
            } else {
                EV_WALK_STALLED
            };
            // (The bot's own fight stands where it is, not at the position.)
            h.spot = here;
            h.goal = here;
            st.fight.mode = FightState::STAND;
            return false;
        }
    }
    if let Some(i) = st.tpos.held.filter(|_| on) {
        let q = sim.positions.positions[i as usize];
        let d = xz(aim - q.pos);
        let (inner, outer) = band(sim, tk);
        let rescore = foe.seen && now >= st.tpos.rescore_at;
        if rescore {
            st.tpos.rescore_at = now + RESCORE;
        }
        let round = rescore && st.tpos.covered && !q.hides_toward(d);
        let hit = st.tpos.hits >= 2;
        let off_band = rescore && (d.length() < inner || d.length() > outer);
        let timer = st.fight.mode == FightState::COVER && now >= st.tpos.next_at;
        // At an open position a covered one is looked for every RETRY (ours:
        // the open spot was kept 5-12 s while cover came within reach).
        if !(round || hit || off_band || timer || st.tpos.covered)
            && st.fight.mode == FightState::COVER
            && now >= st.tpos.retry_at
        {
            st.tpos.retry_at = now + RETRY;
            let here = Pick {
                idx: i,
                covered: false,
                lean: 0,
            };
            if let Some(p) = choose(sim, tk, st, foe, Some((i, false)), true, Some(here), nt)
                .filter(|p| p.idx != i)
            {
                nt.tpos_ev |= EV_UPGRADE | EV_PICK_COVERED;
                take(sim, tk, st, h, p, nt);
                return true;
            }
        }
        if !(round || hit || off_band || timer) {
            st.tpos.until = now + FREE_AFTER;
            if st.fight.mode == FightState::COVER {
                cycle(sim, tk, st, h, Some(aim));
            }
            return true;
        }
        leaving = Some((i, round));
        timer_only = !(round || hit || off_band);
        // Card TPOSR (OURS): a position hit twice is compromised - not asked
        // for again, as a failed walk's (left at once now, a bot with nothing
        // else in reach walked back to it at its next ask, 2 s on).
        if hit {
            shun(st);
        }
        nt.tpos_ev |= if round {
            EV_LEAVE_ROUND
        } else if hit {
            EV_LEAVE_HIT
        } else if off_band {
            EV_LEAVE_BAND
        } else {
            EV_LEAVE_TIMER
        };
    }
    if !on && now < st.tpos.retry_at {
        return false;
    }
    // The 5-12 s timer takes another QUALIFYING position (the PS2's
    // "another qualifying hint"): from cover, only another with cover.
    let cover_only = timer_only && st.tpos.covered;
    // Card HYST: the timer's query re-scores the position held beside its
    // candidates (it is still good: no other leave test fired).
    let keep = leaving.filter(|_| timer_only).map(|(i, _)| Pick {
        idx: i,
        covered: st.tpos.covered,
        lean: st.tpos.lean,
    });
    match choose(sim, tk, st, foe, leaving, cover_only, keep, nt) {
        Some(p) if Some(p.idx) == keep.map(|k| k.idx) => {
            nt.tpos_ev |= EV_KEEP_TIMER;
            st.tpos.until = now + FREE_AFTER;
            st.tpos.next_at = now + span(tk.me, now, 2, NEXT);
            cycle(sim, tk, st, h, Some(aim));
            true
        }
        Some(p) => {
            nt.tpos_ev |= if p.covered {
                EV_PICK_COVERED
            } else {
                EV_PICK_OPEN
            };
            take(sim, tk, st, h, p, nt);
            true
        }
        None if timer_only => {
            nt.tpos_ev |= EV_PICK_NONE | EV_KEEP_TIMER;
            st.tpos.until = now + FREE_AFTER;
            st.tpos.next_at = now + span(tk.me, now, 2, NEXT);
            cycle(sim, tk, st, h, Some(aim));
            true
        }
        None => {
            nt.tpos_ev |= EV_PICK_NONE;
            release(st, now);
            st.tpos.retry_at = now + RETRY;
            false
        }
    }
}

/// Card TPOSR, every tick after the fight's rules: the position held stays
/// reserved for as long as it is held, the target seen or not - the PS2's
/// cover state machine re-stamps its hint's reservation on every update of
/// its walk and cover states (`FUN_002d0c78` -> `FUN_002ded48`; AUDIT TPOS:
/// renewed only on thinks with the target seen, 20 % of held time went
/// unreserved and two bots shared a position). And a position hit twice is
/// left at the next think whatever else the bot does (a target seen beyond
/// the fight's reach, a friend's told record: neither runs [`fight`]).
pub(super) fn keep(sim: &Sim, st: &mut AiState, thinking: bool, nt: &mut NavTick) {
    if !st.fight.tactician() || st.tpos.held.is_none() {
        return;
    }
    if thinking && st.tpos.hits >= 2 {
        nt.tpos_ev |= EV_LEAVE_HIT;
        shun(st);
        release(st, sim.tick);
        return;
    }
    st.tpos.until = st.tpos.until.max(sim.tick + FREE_AFTER);
}

/// A hit on a bot at its position: two and it is compromised (left at the
/// next think); the tier's "move" share of hits leaves it at once (card
/// REACT's row). One draw per burst ([`super::REACT_COOLDOWN`]).
pub(super) fn hit(sim: &Sim, tk: &Tick, st: &mut AiState) {
    st.dodge_until = sim.tick + super::REACT_COOLDOWN;
    if st.fight.mode != FightState::COVER {
        return;
    }
    st.tpos.hits = st.tpos.hits.saturating_add(1);
    if unit_unit(tk.me.index, sim.tick, tk.me.gen, SALT_TPOS ^ 3) < tk.tier.react[2] {
        st.tpos.hits = st.tpos.hits.max(2);
    }
}

/// The write-back's grant (`Sim::step`, in unit-index order): a position
/// taken this tick within [`SPACING`] of one a lower index took this tick
/// is refused — the bot asks again at its next think.
pub fn grant(table: &PositionTable, tick: u64, st: &mut AiState, granted: &mut Vec<Vec3>) {
    let Some(q) = st
        .tpos
        .held
        .filter(|_| st.tpos.at == tick)
        .and_then(|i| table.positions.get(i as usize))
    else {
        return;
    };
    if granted.iter().any(|g| nav::xz_dist(*g, q.pos) < SPACING) {
        st.tpos.held = None;
        if st.fight.tactician() {
            st.fight.mode = 0;
        }
        st.hold = None;
    } else {
        granted.push(q.pos);
    }
}

/// B12 (`analysis/bot-design.md` 10): is unit `u` fighting at its position
/// with cover from its target?
pub fn covered_from_target(sim: &Sim, u: &Unit) -> bool {
    let st = &u.ai;
    if st.fight.mode != FightState::COVER {
        return false;
    }
    let Some(q) = st
        .tpos
        .held
        .and_then(|i| sim.positions.positions.get(i as usize))
    else {
        return false;
    };
    let Some(c) = st.target.and_then(|h| crate::perception::contact_of(st, h)) else {
        return false;
    };
    nav::xz_dist(u.soldier.pos, q.pos) <= LEAN + 0.5 && q.shields_toward(xz(c.last_pos - q.pos))
}

impl Sim {
    /// Card TPOS: (re)build the position table ([`build`]) — once the
    /// posts are in, since the floors it stands on are walked from them.
    pub fn build_positions(&mut self) {
        self.positions = build(self);
    }
}

#[cfg(test)]
mod tests;
