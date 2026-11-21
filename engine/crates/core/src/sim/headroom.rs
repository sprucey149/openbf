//! Issue #182 B1, HEADROOM: a soldier never grows into a stance his head has
//! no room for.
//!
//! The stance machine (`soldier/step.rs`) has no world: a roll, a prone, a
//! crouch, a tumble or a get-up ends in a stand (1.9 m) from 0.55 m or
//! 1.6 m with no look overhead, and the wall pass then pushes the head out
//! of a low deck, a stair or an eave SIDEWAYS (a ceiling pushes only by its
//! horizontal part), inside the solid. `ceiling_sweep` cannot answer it: it
//! sweeps a base that RISES, and a stance growing in place keeps its base.
//!
//! So after the soldier's own update and before the wall pass, a capsule
//! that grew this tick is tested from its old top to its new one: rays
//! straight up from inside the old top (the axis and two rings), up to the
//! taller capsule's surface. Blocked, he stays low: a crouch where a crouch fits, else prone
//! ([`Soldier::hold_low`]). Held low, he rises on his own as soon as there
//! is room, so a bot that rolled under a lip does not crawl on for ever.
//!
//! All OURS (engineering): the maintainer accepted staying crouched where
//! there is no room to stand (2026-11-19 overnight defaults).
//! trace: whether a PS2 BF2 roll ending under a low lip stands up into it.

use super::{Embed, Sim};
use crate::collision::{Capsule, HullFilter, Layers};
use crate::fmath::FloatExt;
use crate::soldier::{Soldier, Stance, StanceSnap};
use crate::unit::{capsule_height, SOLDIER_RADIUS};
use glam::Vec3;
use std::f32::consts::FRAC_1_SQRT_2;

/// OURS: how far under the old capsule's top the up rays start (m), so a
/// face the head already touches is seen. measure: the census's roll and
/// crouch sites under BF1 Kashyyyk Docks' hut decks.
const ROOM_RAY_BELOW_TOP: f32 = 0.1;
/// OURS: the lowest an up ray starts over the feet (m), over a step's floor.
const ROOM_RAY_MIN: f32 = 0.3;
/// OURS: the rings of up rays round the axis (m), inside the 0.5 m
/// capsule. measure: as [`ROOM_RAY_BELOW_TOP`].
const ROOM_RINGS: [f32; 2] = [0.25, 0.45];
/// OURS: how much deeper the taller capsule may sit in a body than the
/// shorter one (m) before the growth is refused: the embed oracle's own
/// overlap threshold, so a wall he only touches does not count.
const ROOM_OVERLAP: f32 = super::embed::EMBED_OVERLAP;
/// The rings' eight directions.
const ROOM_DIRS: [(f32, f32); 8] = [
    (1.0, 0.0),
    (FRAC_1_SQRT_2, FRAC_1_SQRT_2),
    (0.0, 1.0),
    (-FRAC_1_SQRT_2, FRAC_1_SQRT_2),
    (-1.0, 0.0),
    (-FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
    (0.0, -1.0),
    (FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
];

/// The stances B1 guards: the ones a grounded soldier grows into, and the
/// fall a prone soldier off the floor for 0.3 s turns into (1.9 m from
/// 0.55 m, in `Soldier::resolve_ground`, so [`Sim::keep_headroom_on_ground`]
/// checks it). A soldier held prone under a sloping overhang (Hoth's natural
/// corridors) lost the floor there, grew into the fall's capsule inside the
/// rock and the next tick's wall pass pushed him through it (the census's
/// push and tunnel sites at bf2_hot1 `corner3` / `corner16`).
fn guarded(s: Stance) -> bool {
    matches!(
        s,
        Stance::Stand | Stance::Sprint | Stance::Crouch | Stance::Prone | Stance::Fall
    )
}

impl Sim {
    /// Is there room over `feet` to grow a soldier's capsule from `from_h`
    /// to `to_h` (m)? Rays straight up through the growth: the axis and
    /// two rings of eight ([`ROOM_RINGS`]), each from inside the old top to
    /// the new capsule's surface over it. A ray that starts inside a solid
    /// (a roll ended against a post or in a prop's base: the wall pass's
    /// to push him out) does not count; nor does the capsule's side
    /// touching a wall, which no up ray meets. Always room on a world with
    /// no collision.
    pub fn stance_room(&self, feet: Vec3, from_h: f32, to_h: f32) -> bool {
        if to_h <= from_h || self.collision.is_empty() {
            return true;
        }
        let r = SOLDIER_RADIUS;
        // Already IN a solid at the short stance (a roll that ended with
        // its axis in a crate, a pillar or a watertight prop): that is the
        // wall pass's to push out, as before; held low, he stayed in it.
        // Only the oracle's solid classes: an open shell's parity reads a
        // soldier pressed on a building's outside wall as in it (A4), and
        // standing him up there is exactly the push through that wall.
        let short = Capsule {
            base: feet,
            height: from_h,
            radius: r,
        };
        if matches!(
            self.embed_capsule(&short),
            Embed::InPrim(_) | Embed::InClosedMesh(_)
        ) {
            return true;
        }
        let y0 = (from_h - ROOM_RAY_BELOW_TOP).fmax(ROOM_RAY_MIN).fmin(to_h);
        let rays = std::iter::once((0.0, 0.0, 0.0)).chain(ROOM_RINGS.iter().flat_map(|&rho| {
            ROOM_DIRS
                .iter()
                .map(move |&(dx, dz)| (dx * rho, dz * rho, rho))
        }));
        for (dx, dz, rho) in rays {
            // The new capsule's top surface over this ring.
            let top = to_h - r + (r * r - rho * rho).fmax(0.0).sqrt();
            let o = feet + Vec3::new(dx, y0, dz);
            if top <= y0 {
                continue;
            }
            let hit = self.collision.ray_blocked(
                o,
                Vec3::Y,
                top - y0,
                Layers::SOLDIER,
                HullFilter::All,
                [None; 2],
            );
            if hit
                && self.collision.inside_primitive(o).is_none()
                && self.collision.enclosing(o).is_none()
            {
                return false;
            }
        }
        // Pressed against a wall: the taller capsule may not sit deeper in
        // a body than the shorter one does (a roll's low capsule ends with
        // its axis closer to a wall than a stand's radius; standing there
        // gave the wall pass a deep contact to push him through).
        let depth = |h: f32| {
            let cap = Capsule {
                base: feet,
                height: h,
                radius: r,
            };
            self.collision
                .capsule_overlap(&cap.stepped())
                .map_or(0.0, |(d, _)| d)
        };
        depth(to_h) <= depth(from_h) + ROOM_OVERLAP
    }

    /// The stance B1 keeps a soldier in when the tick grew him from
    /// `before` into his stance with no room: `None` when it fits (or did
    /// not grow).
    fn held_stance(&self, s: &Soldier, before: Stance) -> Option<Stance> {
        let (from_h, to_h) = (capsule_height(before), capsule_height(s.stance));
        if !guarded(s.stance) || to_h <= from_h || self.stance_room(s.pos, from_h, to_h) {
            return None;
        }
        let crouch = capsule_height(Stance::Crouch);
        Some(
            if s.stance != Stance::Crouch && self.stance_room(s.pos, from_h, crouch) {
                Stance::Crouch
            } else {
                Stance::Prone
            },
        )
    }

    /// Issue #182 B1, after unit `i`'s own update and before the wall pass:
    /// rise from a held stance where there is room now, then refuse a
    /// growth with no room (the module header).
    pub(super) fn keep_headroom(&mut self, i: usize, before: &StanceSnap, dt: f32) {
        let s = &self.units[i].soldier;
        let held = s.held_low.filter(|h| *h == s.stance);
        if s.held_low.is_some() && held.is_none() {
            self.units[i].soldier.held_low = None;
        }
        if let Some(h) = held {
            // He changed nothing himself: stand where a stand fits, a
            // prone rises to a crouch where only that does.
            let s = &self.units[i].soldier;
            let h_now = capsule_height(h);
            let rise = if !s.grounded {
                None
            } else if self.stance_room(s.pos, h_now, capsule_height(Stance::Stand)) {
                Some(Stance::Stand)
            } else if h == Stance::Prone
                && self.stance_room(s.pos, h_now, capsule_height(Stance::Crouch))
            {
                Some(Stance::Crouch)
            } else {
                None
            };
            if let Some(to) = rise {
                self.units[i].soldier.rise_from_held(to);
            }
            return;
        }
        if let Some(low) = self.held_stance(&self.units[i].soldier, before.stance) {
            self.units[i].soldier.hold_low(low, before, dt);
        }
    }

    /// Issue #182 B1, after unit `i`'s ground query: the query's own
    /// stance changes off the floor (a prone into the fall, a crouch into
    /// the stand, after 0.3 s in the air) grow him too, after this tick's
    /// wall pass, so the NEXT tick's pass would push the grown capsule out
    /// of the overhang he was held under. Refused the same way, the timers
    /// put back as they were before the query (the tick's time is already
    /// in them).
    pub(super) fn keep_headroom_on_ground(&mut self, i: usize, before: &StanceSnap) {
        if let Some(low) = self.held_stance(&self.units[i].soldier, before.stance) {
            self.units[i].soldier.hold_low(low, before, 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::sim::{buttons, PadState};
    use crate::unit::UnitSpec;
    use glam::Mat4;

    /// A flat floor slab at y 0 and, when `deck_under` is set, a deck slab
    /// whose underside is that high over the floor, 6 m square at the origin.
    fn world(deck_under: Option<f32>) -> Sim {
        let mut s = Sim::empty(1);
        let slab = |y: f32| {
            Body::with_layers(
                Mat4::from_translation(Vec3::new(0.0, y, 0.0)),
                Shape::Box {
                    half: Vec3::new(30.0, 0.25, 30.0),
                },
                Layers::SOLDIER,
            )
        };
        let mut bodies = vec![slab(-0.25)];
        if let Some(h) = deck_under {
            bodies.push(Body::with_layers(
                Mat4::from_translation(Vec3::new(0.0, h + 0.25, 0.0)),
                Shape::Box {
                    half: Vec3::new(3.0, 0.25, 3.0),
                },
                Layers::SOLDIER,
            ));
        }
        s.collision = CollisionWorld::new(bodies, Vec::new());
        s
    }

    fn player(s: &mut Sim, at: Vec3) -> usize {
        let h = s.spawn(&UnitSpec::trooper(0, at)).expect("spawned");
        let i = h.index as usize;
        s.units[i].controller = crate::unit::Controller::Player { slot: 0 };
        s.units[i].is_player = true;
        s.units[i].soldier.pos = at;
        i
    }

    fn step(s: &mut Sim, buttons: u32) {
        s.step(&[PadState {
            buttons,
            ..PadState::default()
        }]);
    }

    /// The room test itself: under a deck 1.7 m up a crouch fits and a
    /// stand does not; in the open both do; under a 1.0 m deck only prone.
    #[test]
    fn stance_room_reads_the_deck_overhead() {
        let s = world(Some(1.7));
        let (p, c, st) = (0.55, 1.6, 1.9);
        assert!(s.stance_room(Vec3::ZERO, p, c));
        assert!(!s.stance_room(Vec3::ZERO, c, st));
        assert!(!s.stance_room(Vec3::ZERO, p, st));
        // Out from under it (the deck's edge is 3 m out, the capsule 0.5).
        assert!(s.stance_room(Vec3::new(4.0, 0.0, 0.0), c, st));
        let low = world(Some(1.0));
        assert!(!low.stance_room(Vec3::ZERO, p, c));
        // Shrinking always fits.
        assert!(low.stance_room(Vec3::ZERO, st, p));
    }

    /// The Theed-type roll under a low lip: the roll's end does not stand
    /// him up into the deck; he stays crouched, and stands on his own once
    /// he walks out from under it.
    #[test]
    fn roll_under_low_deck_stays_crouched() {
        let mut s = world(Some(1.7));
        let i = player(&mut s, Vec3::ZERO);
        for _ in 0..5 {
            step(&mut s, 0);
        }
        s.units[i].soldier.stance = Stance::Roll;
        s.units[i].soldier.time_in_stance = 10.0;
        s.units[i].soldier.vel = Vec3::ZERO;
        step(&mut s, 0);
        let sol = &s.units[i].soldier;
        assert_eq!(sol.stance, Stance::Crouch, "rolled out under the deck");
        assert_eq!(sol.held_low, Some(Stance::Crouch));
        assert!(s.embed_state(s.handle(i)).is_none(), "nothing embedded");
        // Still under it: still crouched, whatever the jump (stand) press.
        for k in 0..20 {
            step(&mut s, if k % 2 == 0 { buttons::JUMP } else { 0 });
            assert_eq!(s.units[i].soldier.stance, Stance::Crouch, "tick {k}");
        }
        // Out from under it: up on his own.
        s.units[i].soldier.pos = Vec3::new(5.0, 0.0, 0.0);
        step(&mut s, 0);
        assert_eq!(s.units[i].soldier.stance, Stance::Stand);
        assert_eq!(s.units[i].soldier.held_low, None);
    }

    /// A crouch under a lip cannot stand (the jump press stands a crouch
    /// up); in the open the same press does.
    #[test]
    fn crouch_under_lip_cannot_stand() {
        for (deck, want) in [(Some(1.7), Stance::Crouch), (None, Stance::Stand)] {
            let mut s = world(deck);
            let i = player(&mut s, Vec3::ZERO);
            for _ in 0..5 {
                step(&mut s, 0);
            }
            s.units[i].soldier.stance = Stance::Crouch;
            s.units[i].soldier.time_in_stance = 10.0;
            step(&mut s, buttons::JUMP);
            assert_eq!(s.units[i].soldier.stance, want, "deck {deck:?}");
            assert!(s.embed_state(s.handle(i)).is_none());
        }
    }

    /// A prone under a 1.0 m deck has no room even for a crouch: he stays
    /// prone, with the refused stand-up's lock undone.
    #[test]
    fn prone_under_a_crawlspace_stays_prone() {
        let mut s = world(Some(1.0));
        let i = player(&mut s, Vec3::ZERO);
        for _ in 0..5 {
            step(&mut s, 0);
        }
        s.units[i].soldier.stance = Stance::Prone;
        s.units[i].soldier.time_in_stance = 10.0;
        step(&mut s, buttons::JUMP);
        let sol = &s.units[i].soldier;
        assert_eq!(sol.stance, Stance::Prone);
        assert!(!sol.lock_hard, "the refused stand-up's lock is undone");
        assert!(sol.time_in_stance > 10.0);
    }

    /// A roll that ends with its axis inside a solid post (the stand-up is
    /// the wall pass's to push him out of it) is not held low inside it.
    #[test]
    fn roll_ending_inside_a_post_still_stands() {
        let mut s = world(None);
        let post = Body::with_layers(
            Mat4::from_translation(Vec3::new(0.3, 1.5, 0.0)),
            Shape::Box {
                half: Vec3::new(0.4, 1.5, 0.4),
            },
            Layers::SOLDIER,
        );
        let mut bodies = s.collision.bodies().to_vec();
        bodies.push(post);
        s.collision = CollisionWorld::new(bodies, Vec::new());
        let i = player(&mut s, Vec3::ZERO);
        s.units[i].soldier.stance = Stance::Roll;
        s.units[i].soldier.time_in_stance = 10.0;
        step(&mut s, 0);
        assert_eq!(s.units[i].soldier.stance, Stance::Stand);
        assert_eq!(s.units[i].soldier.held_low, None);
    }

    /// A roll that ends pressed on a wall (its low capsule's axis nearer
    /// the wall than a stand's radius) stays low: standing there gave the
    /// wall pass a deep contact that pushed him through thin walls (the
    /// census's `push` class, mostly rolls). Stepped away, he stands.
    #[test]
    fn roll_ending_pressed_on_a_wall_stays_low() {
        let mut s = world(None);
        let wall = Body::with_layers(
            Mat4::from_translation(Vec3::new(1.3, 2.0, 0.0)),
            Shape::Box {
                half: Vec3::new(1.0, 2.0, 4.0),
            },
            Layers::SOLDIER,
        );
        let mut bodies = s.collision.bodies().to_vec();
        bodies.push(wall);
        s.collision = CollisionWorld::new(bodies, Vec::new());
        let i = player(&mut s, Vec3::ZERO);
        s.units[i].soldier.stance = Stance::Roll;
        s.units[i].soldier.time_in_stance = 10.0;
        step(&mut s, 0);
        assert_ne!(s.units[i].soldier.stance, Stance::Stand);
        assert!(s.units[i].soldier.held_low.is_some());
        s.units[i].soldier.pos = Vec3::new(-1.0, 0.0, 0.0);
        step(&mut s, 0);
        assert_eq!(s.units[i].soldier.stance, Stance::Stand);
    }

    /// A prone soldier off the floor turns into the fall (1.9 m) after
    /// 0.3 s, in the ground query after the wall pass. Under a deck that
    /// fall's capsule would reach into, he stays prone until it fits (the
    /// held prone under Hoth's sloping overhangs grew into the rock that
    /// way and the next wall pass pushed him through). In the open the
    /// same drop falls as before.
    #[test]
    fn prone_off_the_floor_under_a_deck_does_not_grow_into_it() {
        for deck in [true, false] {
            let mut s = world(None);
            let mut bodies = s.collision.bodies().to_vec();
            if deck {
                bodies.push(Body::with_layers(
                    Mat4::from_translation(Vec3::new(0.0, 3.95, 0.0)),
                    Shape::Box {
                        half: Vec3::new(3.0, 0.25, 3.0),
                    },
                    Layers::SOLDIER,
                ));
            }
            s.collision = CollisionWorld::new(bodies, Vec::new());
            let i = player(&mut s, Vec3::new(0.0, 3.0, 0.0));
            s.units[i].soldier.stance = Stance::Prone;
            s.units[i].soldier.time_in_stance = 10.0;
            let mut fell = false;
            for k in 0..120 {
                step(&mut s, 0);
                let sol = &s.units[i].soldier;
                let h = crate::unit::capsule_height(sol.stance);
                if deck {
                    assert!(
                        sol.pos.y + h <= 3.7 + 0.05,
                        "tick {k}: {:?} at y {} reaches into the deck",
                        sol.stance,
                        sol.pos.y
                    );
                }
                fell |= sol.stance == Stance::Fall;
            }
            let sol = &s.units[i].soldier;
            // Under the deck he comes down held low (a crouch once that
            // fits) and stands where it all fits; in the open he falls.
            assert_eq!(fell, !deck, "deck {deck}");
            assert_eq!(sol.held_low, None, "deck {deck}: up once it fits");
            assert!(
                sol.grounded && sol.pos.y.abs() < 0.05,
                "deck {deck}: landed"
            );
            assert!(s.embed_state(s.handle(i)).is_none(), "deck {deck}");
        }
    }

    /// Nothing changes where nothing is overhead: an ordinary world's
    /// stance changes never set the held stance, so it never hashes.
    #[test]
    fn open_ground_never_holds_low() {
        let mut s = world(None);
        let i = player(&mut s, Vec3::ZERO);
        for st in [Stance::Roll, Stance::Prone, Stance::Crouch] {
            s.units[i].soldier.stance = st;
            s.units[i].soldier.time_in_stance = 10.0;
            for _ in 0..3 {
                step(&mut s, buttons::JUMP);
                step(&mut s, 0);
            }
            assert_eq!(s.units[i].soldier.held_low, None, "{st:?}");
        }
    }
}
