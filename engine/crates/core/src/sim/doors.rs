//! Door simulation queries, body movement and sound-edge events.

use super::{Event, Sim};
use glam::Vec3;

impl Sim {
    /// Card MOUTH2: is `body` a leaf of a door that is OPENING — a living
    /// unit stands in one of its triggers, so its next tick slides it on, and
    /// its clip is not at its end yet (`crate::ai::steer`'s `stop_ahead`
    /// asks, of the body its sweep met)? A door that is fully open, or one
    /// with nobody in its triggers, is whatever its bodies are. The triggers
    /// are asked of the units as they stand now, [`Self::tick_doors`]' own
    /// test: the answer is a function of hashed state (the sound latch is
    /// not).
    pub fn door_opening(&self, body: &crate::collision::Body) -> bool {
        let bodies = self.collision.bodies();
        self.doors.iter().any(|d| {
            d.bodies
                .iter()
                .any(|k| bodies.get(k.body).is_some_and(|b| std::ptr::eq(b, body)))
                && self.opening(d)
        })
    }

    /// [`Self::door_opening`]'s test of one door: its clip not at its end
    /// and a living unit in one of its triggers.
    fn opening(&self, d: &crate::door::Door) -> bool {
        !d.locked
            && d.open < 1.0
            && d.occupied(self.units.iter().filter(|u| u.alive).map(|u| u.soldier.pos))
    }

    /// Card DOORS: does the vertical line through `p` pass through the box
    /// of a leaf of a door that is OPENING ([`Self::door_opening`]'s test)?
    /// `crate::ai::steer`'s `stop_ahead` asks it of the point its
    /// steep-floor probe comes down at: a face found there is the leaf's,
    /// and the leaf is leaving. The box is the leaf's as it stands this tick
    /// (a rolled leaf's box is wider than the leaf; the line through it still
    /// meets the leaf wherever the box is as tall as the leaf's own span).
    pub fn opening_leaf_over(&self, p: Vec3) -> bool {
        let bodies = self.collision.bodies();
        self.doors.iter().any(|d| {
            d.bodies.iter().any(|k| {
                bodies.get(k.body).is_some_and(|b| {
                    (b.aabb.min.x..=b.aabb.max.x).contains(&p.x)
                        && (b.aabb.min.z..=b.aabb.max.z).contains(&p.z)
                })
            }) && self.opening(d)
        })
    }

    /// One tick of every door (`crate::door`): occupied by any LIVE unit's
    /// feet, the parameter moves, a door that moved moves its bodies, and a
    /// trigger edge pushes its sound event (not hashed, like every event).
    pub(crate) fn tick_doors(&mut self, dt: f32) {
        if self.doors.is_empty() {
            return;
        }
        let feet: Vec<Vec3> = self
            .units
            .iter()
            .filter(|u| u.alive)
            .map(|u| u.soldier.pos)
            .collect();
        for (i, d) in self.doors.iter_mut().enumerate() {
            let occupied = d.occupied(feet.iter().copied());
            if d.advance(occupied, dt) {
                for k in 0..d.bodies.len() {
                    self.collision.move_body(d.bodies[k].body, d.pose(k));
                }
            }
            // The sound latch (DRV): an output, like every event.
            let door = i as u32;
            match d.edge(occupied, dt) {
                Some(crate::door::Edge::Opened) => self.events.push(Event::DoorOpened { door }),
                Some(crate::door::Edge::Closed) => self.events.push(Event::DoorClosed { door }),
                Some(crate::door::Edge::Locked) => self.events.push(Event::DoorLocked { door }),
                None => {}
            }
        }
    }
}
