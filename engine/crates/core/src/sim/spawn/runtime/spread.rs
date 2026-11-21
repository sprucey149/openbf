//! Hunt spawn spread (OURS, the maintainer 2026-11-20): Hunt may field more
//! soldiers than its posts have spawn nodes, so a soldier spawning on a node
//! another soldier still stands on steps out onto a free spot around it
//! instead of stacking. No other mode moves a spawn.
//! measure: the big-sides Hunt fit's spawn census (soldiers within
//! CROWD_RADIUS of each other a second after spawning) before changing the
//! radius or the rings.
//!
//! Where both Hunt sides share one spawn path (BF2 Endor and BF2 Mos Eisley),
//! a soldier would open, or come back, right beside an enemy. So a Hunt
//! soldier whose node has a live enemy within [`ENEMY_CLEAR`] takes another
//! node of its post, clear of enemies where one is.
//! measure: the spawn census's nearest-enemy distance at spawn on the shared
//! path maps before changing [`ENEMY_CLEAR`].

use crate::sim::state::Sim;
use crate::unit::{SpawnPost, Unit};
use glam::{Vec2, Vec3};

/// A live soldier this close (XZ, m) to a spot, within [`CROWD_RISE`]
/// vertically, crowds it.
pub const CROWD_RADIUS: f32 = 1.2;
const CROWD_RISE: f32 = 2.0;
/// A Hunt spawn node with a live enemy closer than this (m) gives way to the
/// post's node farthest from live enemies.
pub const ENEMY_CLEAR: f32 = 20.0;
/// Friends within [`NODE_SHARE_RADIUS`] (m) that make a node full for the
/// enemy-clear pick: about what the crowd spread's rings seat.
pub const NODE_SHARE: usize = 8;
pub const NODE_SHARE_RADIUS: f32 = 5.0;
/// The rings tried around a crowded node, m, nearest first.
pub const SPREAD_RINGS: [f32; 3] = [1.5, 3.0, 4.5];
/// The most a spread spot's floor may differ from the node's, m: a step,
/// not a ledge.
const SPREAD_RISE: f32 = 0.75;
/// How close the walk back to the node must get, m.
const SPREAD_REACH: f32 = 0.5;

/// The enemy-clear pick's order: rank, friends around the node, steps along
/// the path (lowest first).
type PickKey = (u8, usize, usize);

/// Eight spokes as exact unit vectors (no trigonometry: the same spots on
/// every machine, so a LAN round's spawns agree).
const SPOKES: [(f32, f32); 8] = {
    const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
    [
        (1.0, 0.0),
        (D, D),
        (0.0, 1.0),
        (-D, D),
        (-1.0, 0.0),
        (-D, -D),
        (0.0, -1.0),
        (D, -D),
    ]
};

impl Sim {
    /// Is a live soldier standing within [`CROWD_RADIUS`] of `feet`?
    pub fn spawn_crowded(&self, feet: Vec3) -> bool {
        self.units.iter().any(|u| {
            u.alive
                && (u.soldier.pos.y - feet.y).abs() < CROWD_RISE
                && Vec2::new(u.soldier.pos.x - feet.x, u.soldier.pos.z - feet.z).length_squared()
                    < CROWD_RADIUS * CROWD_RADIUS
        })
    }

    /// Squared distance (m^2) from `at` to the nearest live soldier hostile
    /// to `team`; infinite with none.
    fn nearest_enemy_sq(&self, team: u8, at: Vec3) -> f32 {
        self.units
            .iter()
            .filter(|u| u.alive && self.hostile(team, u.team))
            .map(|u| (u.soldier.pos - at).length_squared())
            .fold(f32::INFINITY, crate::fmath::min)
    }

    /// Live soldiers of `team`'s side within [`NODE_SHARE_RADIUS`] of `at`,
    /// on the XZ plane only: an authored node can sit well above or below the
    /// floor its soldiers settle on.
    fn friends_near(&self, team: u8, at: Vec3) -> usize {
        self.units
            .iter()
            .filter(|u| {
                u.alive
                    && self.allied(team, u.team)
                    && Vec2::new(u.soldier.pos.x - at.x, u.soldier.pos.z - at.z).length_squared()
                        < NODE_SHARE_RADIUS * NODE_SHARE_RADIUS
            })
            .count()
    }

    /// In a Hunt, a soldier about to spawn on a node with a live enemy
    /// within [`ENEMY_CLEAR`] moves to another node of its post (`post`, or
    /// every node of its side when none is known). Best first: a node clear
    /// of enemies that is not full ([`NODE_SHARE`] friends around it); one
    /// at least half that far that is not full; a clear but full one; then
    /// the node farthest from live enemies. Among equals, the fewest friends
    /// around it, then the next along the path. It keeps its node when the
    /// pick is no farther from enemies. Call before the spawn's ground
    /// settle. Nothing changes outside a Hunt.
    pub(in crate::sim) fn hunt_clear_spawn(&self, unit: &mut Unit, post: Option<SpawnPost>) {
        if !self.is_hunt() || unit.team >= 2 {
            return;
        }
        let team = unit.team;
        let clear = ENEMY_CLEAR * ENEMY_CLEAR;
        let half = clear * 0.25;
        let here = self.nearest_enemy_sq(team, unit.spec.pos);
        if here >= clear {
            return;
        }
        let nodes = match post {
            Some(p) => self.spawn_post_nodes(team, p),
            None => self.spawn_points(team),
        };
        if nodes.is_empty() {
            return;
        }
        let start = nodes
            .iter()
            .position(|&(p, _)| p == unit.spec.pos)
            .unwrap_or(0);
        let mut best: Option<(PickKey, (Vec3, f32), f32)> = None;
        let mut farthest: Option<((Vec3, f32), f32)> = None;
        for k in 0..nodes.len() {
            let (p, y) = nodes[(start + k) % nodes.len()];
            let d = self.nearest_enemy_sq(team, p);
            if farthest.is_none_or(|(_, b)| d > b) {
                farthest = Some(((p, y), d));
            }
            let friends = self.friends_near(team, p);
            let full = friends >= NODE_SHARE;
            let rank = match (d >= clear, d >= half, full) {
                (true, _, false) => 0,
                (false, true, false) => 1,
                (true, _, true) => 2,
                _ => continue,
            };
            let key = (rank, friends, k);
            if best.is_none_or(|(b, _, _)| key < b) {
                best = Some((key, (p, y), d));
            }
        }
        let pick = best.map(|(_, n, d)| (n, d)).or(farthest);
        let Some(((pos, yaw), d)) = pick else {
            return;
        };
        if d <= here {
            return;
        }
        unit.spec.pos = pos;
        unit.spec.yaw = yaw;
        unit.soldier.pos = pos;
        unit.soldier.yaw = yaw;
    }

    /// In a Hunt, move a soldier about to spawn on a crowded node to the
    /// nearest free spot on [`SPREAD_RINGS`] around it: ground within a step
    /// of the node's, clear of bodies and death regions, above the kill
    /// floor, and walkable back to the node. Every spot crowded or unfit: it
    /// keeps the node. Nothing changes outside a Hunt.
    pub(in crate::sim) fn hunt_spread(&self, unit: &mut Unit) {
        if !self.is_hunt() || unit.team >= 2 {
            return;
        }
        let feet = unit.soldier.pos;
        if !self.spawn_crowded(feet) {
            return;
        }
        let node = unit.spec.pos;
        for (ring, r) in SPREAD_RINGS.into_iter().enumerate() {
            // Even rings try the four axes first, odd rings the diagonals,
            // so two rings' spots do not line up.
            let order = if ring % 2 == 0 {
                [0, 2, 4, 6, 1, 3, 5, 7]
            } else {
                [1, 3, 5, 7, 0, 2, 4, 6]
            };
            for (x, z) in order.map(|k| SPOKES[k]) {
                let probe = Vec3::new(node.x + r * x, node.y, node.z + r * z);
                let Some(floor) = self.spawn_ground_hit(probe) else {
                    continue;
                };
                if (floor - feet.y).abs() > SPREAD_RISE {
                    continue;
                }
                let at = Vec3::new(probe.x, floor, probe.z);
                if self.kill_floor.is_some_and(|minimum| floor < minimum)
                    || self.death_regions.iter().any(|region| region.contains(at))
                    || !self.soldier_clear(at)
                    || self.spawn_crowded(at)
                    || self.floor_path(
                        at,
                        Vec2::new(feet.x, feet.z),
                        feet.y,
                        SPREAD_REACH,
                        crate::collision::STEP_HEIGHT,
                        crate::sim::bounds::WALL_BAND,
                        crate::sim::bounds::WALL_CELLS,
                    ) != Some(true)
                {
                    continue;
                }
                unit.spec.pos = Vec3::new(at.x, node.y, at.z);
                unit.soldier.pos = at;
                return;
            }
        }
    }
}
