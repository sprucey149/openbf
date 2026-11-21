//! Issue #182 B4, the RESCUE: a unit found inside a solid or under the
//! ground is put back on the last spot he stood clear; with no such spot he
//! is killed, one ticket, like a fall.
//!
//! The leak census and the general fixes (B1 headroom, B3 terrain burial,
//! B5/B6 placement) close the ways in that are known. This is the net under
//! the ones that are not: a soldier who still ends up in a crate, a closed
//! rock or under the terrain sheet is not left there for the round.
//!
//! - **What counts.** Only the embed oracle's classes that the A4 sweep
//!   found no false positive for in normal play ([`Sim::solid_embed`]):
//!   inside a primitive, shut in a watertight mesh no nav hub is in, and
//!   under the uncut terrain with nothing overhead while the sim's own
//!   ground under him is that sheet ([`Sim::stuck_in`]). Never an open shell (a
//!   room reads as one), never an overlap (a wall graze), never off-level
//!   (a player may stand there; the bots have their own recovery).
//! - **When.** Each live soldier not in a vehicle is checked every
//!   [`RESCUE_EVERY`] ticks, staggered by unit index. He is rescued when
//!   [`RESCUE_PATIENCE`] checks in a row find him embedded, so at least
//!   `(RESCUE_PATIENCE - 1) * RESCUE_EVERY + 1` ticks inside. The A4 sweep's
//!   only false runs (BF2 Polis Massa's cut edge, under the terrain) last
//!   6-7 ticks; the real ones last seconds.
//! - **The safe spot.** A check that finds him clear, on the ground (not a
//!   vehicle's hull or a flyer's deck) and with his stepped capsule in no
//!   map body and no vehicle hull deeper than [`super::EMBED_OVERLAP`]
//!   records his feet. The spot is asked again before he is put there. A
//!   spot a map body now holds counts as none (he dies). A spot a vehicle
//!   hull now holds (one parked or respawned there since) is waited out:
//!   his embedded run goes on and each later check asks again, up to
//!   [`RESCUE_HULL_WAITS`] checks, then he dies as with no spot. Death or a
//!   vehicle forgets it.
//! - **The spot does not expire.** It is the last check that found him
//!   standing clear, however long ago and however far: after a long jet
//!   flight, or a ride on a hull or a deck, a put-back can move him back to
//!   where he took off or got on, across walls. That is intended: the spot
//!   was clear and grounded when recorded and is checked again before use,
//!   and the other way out (an age or distance cap) would kill him instead.
//! - **Credit.** The kill is [`DeathCause::Embedded`], one ticket from his
//!   own team, like a fall; inside an attacker's forced-move window (a
//!   Force push or blast that drove him in) it is that attacker's kill, as
//!   a fall is (`sim/fall_credit.rs`).
//! - **Hash.** Only the run of embedded checks reaches [`Sim::hash`], and
//!   only while it is running, so a round nobody gets stuck in keeps its
//!   hash. The safe spot is derived from hashed positions, read only by a
//!   rescue, and a rescue moves hashed state.
//!
//! - **Wedged in the air.** A soldier found falling (the fall stance, off
//!   the ground) whose feet moved less than [`WEDGE_MOVE`] since the last
//!   check, [`WEDGE_PATIENCE`] checks in a row, is wedged: held between two steep
//!   faces, or on a slope against a wall, where the ground never stands
//!   him (the leak census's `float` class: a ravine's bridge pier, an
//!   obelisk's groove). He goes back to his safe spot the same way, or
//!   dies with none. Its run of checks reaches [`Sim::hash`] only while it
//!   runs, like the embedded run's.
//! - **Wedged on a slope, standing.** The same stillness counts for a
//!   soldier who stays in a ground stance (stand, crouch, prone, sprint) but
//!   is off the ground on at least [`WEDGE_AIR_TICKS`] of the ticks since the
//!   last check: the sloped-face clamp holding him against a face he can't
//!   climb, his feet touching down one tick in three or four, so his stance
//!   never falls (BF2 Felucia's slope at (93, -136)). A check after that
//!   many ticks off records no safe spot, so the put-back takes him to where
//!   he last stood clear; with no such spot (or only the one he is held
//!   at) he is left as he is, never killed. A roll or a throw is no such
//!   state (their own stances). A jump counts only when he was off the ground on every tick
//!   since the last check: a jump held still in the air (the same slope,
//!   whose jump stance never turns into a fall). A hop in place lands
//!   between checks.
//!
//! All of it is OURS: the PS2 has no such rule (its soldiers rarely leave
//! the collision; ours do through the routes the census lists).
//! PROVISIONAL: [`RESCUE_EVERY`], [`RESCUE_PATIENCE`]; measure: the
//! `SWEEP_EMBED` sweep's run lengths per class and the census's rescue
//! counts.

use super::{DeathCause, Event, Sim};
use crate::collision::Capsule;
use glam::Vec3;

/// PROVISIONAL (ours): ticks between two checks of one unit.
pub const RESCUE_EVERY: u64 = 8;
/// PROVISIONAL (ours): embedded checks in a row before the rescue.
pub const RESCUE_PATIENCE: u8 = 3;
/// PROVISIONAL (ours): a check that finds a soldier off the ground within
/// this far (m) of where the last check found him counts toward a wedge.
/// A fall, a jump or a slide moves more in [`RESCUE_EVERY`] ticks.
pub const WEDGE_MOVE: f32 = 0.25;
/// PROVISIONAL (ours): wedged checks in a row before the rescue (about
/// 2 s at [`RESCUE_EVERY`]; the longest jump hangs under 1 s).
pub const WEDGE_PATIENCE: u8 = 8;
/// PROVISIONAL (ours): ticks off the ground, out of the [`RESCUE_EVERY`]
/// since the last check, that make a still soldier in a ground stance count
/// toward a wedge. A soldier walking into a corner flickers off the ground
/// for a tick at a time; the slope wedge is off three ticks in four.
pub const WEDGE_AIR_TICKS: u8 = 4;
/// PROVISIONAL (ours): checks the rescue waits for a vehicle hull to leave
/// his safe spot before it kills him (about 1 s at [`RESCUE_EVERY`]).
pub const RESCUE_HULL_WAITS: u8 = 4;

/// One unit's rescue state ([`crate::unit::Unit::rescue`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rescue {
    /// His feet at the last check that found him clear and grounded.
    pub safe: Option<Vec3>,
    /// Checks in a row that found him embedded.
    pub stuck: u8,
    /// His feet at the last check (the wedge's stillness).
    pub air_at: Option<Vec3>,
    /// Checks in a row that found him off the ground and still (a wedge).
    pub wedged: u8,
    /// Ticks since the last check he spent off the ground in a ground
    /// stance ([`WEDGE_AIR_TICKS`]).
    pub air_ticks: u8,
    /// Ticks since the last check he spent off the ground in the jump
    /// stance (a jump held still in the air, module header).
    pub jump_ticks: u8,
}

impl Rescue {
    /// Into [`Sim::hash`]: the embedded run, only while it runs.
    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if self.stuck != 0 {
            eat(b"RESCUE_STUCK");
            eat(&[self.stuck]);
        }
        if self.wedged != 0 {
            eat(b"RESCUE_WEDGED");
            eat(&[self.wedged]);
        }
    }
}

impl Sim {
    /// The tick's rescue pass (module header), after the death regions.
    pub(super) fn rescue_embedded(&mut self) {
        if self.rescue_off {
            return;
        }
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if !u.alive || u.mount.is_some() {
                if u.rescue != Rescue::default() {
                    self.units[i].rescue = Rescue::default();
                }
                continue;
            }
            // Each tick off the ground counts toward the slope wedge (a
            // walking stance) or the hung jump (module header).
            if !u.soldier.grounded && u.deck.is_none() && u.droideka.is_none() {
                use crate::soldier::Stance;
                let stance = u.soldier.stance;
                let r = &mut self.units[i].rescue;
                match stance {
                    Stance::Stand | Stance::Crouch | Stance::Prone | Stance::Sprint => {
                        r.air_ticks = r.air_ticks.saturating_add(1);
                    }
                    Stance::Jump => r.jump_ticks = r.jump_ticks.saturating_add(1),
                    _ => {}
                }
            }
            let u = &self.units[i];
            if !(self.tick + i as u64).is_multiple_of(RESCUE_EVERY) {
                continue;
            }
            let cap = self.unit_capsule(u);
            if self.stuck_in(&cap).is_some() {
                let r = &mut self.units[i].rescue;
                r.stuck = r.stuck.saturating_add(1);
                if r.stuck >= RESCUE_PATIENCE {
                    self.rescue_unit(i, &cap);
                }
                continue;
            }
            let standing = u.soldier.grounded
                && u.stood_on.is_none()
                && u.deck.is_none()
                && self.clear_of_bodies(&cap)
                && self.clear_of_hulls(&cap);
            // Falling, off the ground past the stance's air time (the
            // module header's wedge). A soldier walking into a corner
            // flickers off the ground for a tick at a time and stays in his
            // stance; a jet's hover is no fall.
            let aloft = !u.soldier.grounded
                && u.deck.is_none()
                && u.droideka.is_none()
                && u.soldier.stance == crate::soldier::Stance::Fall;
            let feet = cap.base;
            let r = &mut self.units[i].rescue;
            r.stuck = 0;
            // Falling, or held on a slope in a ground stance (module
            // header), and still since the last check counts; anything else
            // ends the run.
            let flicker = r.air_ticks >= WEDGE_AIR_TICKS;
            let hung = u64::from(r.jump_ticks) >= RESCUE_EVERY;
            r.air_ticks = 0;
            r.jump_ticks = 0;
            let still = r.air_at.is_some_and(|a| a.distance(feet) < WEDGE_MOVE);
            r.air_at = Some(feet);
            if standing && !flicker {
                r.safe = Some(cap.base);
            }
            r.wedged = if still && (aloft || flicker || hung) {
                r.wedged.saturating_add(1)
            } else {
                0
            };
            // A slope wedge in a walking stance is never a death, and a
            // put-back onto the spot he is held at does nothing: with no
            // spot at least a wedge's stillness away, the run just ends.
            if r.wedged >= WEDGE_PATIENCE
                && !(aloft || hung)
                && r.safe.is_none_or(|p| p.distance(feet) < WEDGE_MOVE)
            {
                r.wedged = 0;
            }
            if r.wedged >= WEDGE_PATIENCE {
                // As an embedded run at its patience: the hull waits count
                // from here.
                r.stuck = RESCUE_PATIENCE;
                r.wedged = 0;
                r.air_at = None;
                self.rescue_unit(i, &cap);
                if let Some(r) = self.units.get_mut(i).map(|u| &mut u.rescue) {
                    if r.stuck == RESCUE_PATIENCE {
                        // Put back (or waiting on a hull): the wedge's run
                        // starts again; a hull wait goes on as a wedge.
                        r.stuck = 0;
                    }
                }
            }
        }
    }

    /// What the rescue acts on: [`Sim::solid_embed`], where under the
    /// terrain counts only while the sim's own ground under him is the
    /// sheet (it would lift him: no floor of a model is under his feet).
    /// A soldier on a model's floor under an uncut sheet stands where the
    /// sim puts him: BF1 Yavin's arena floor lies 4.8 m under its sheet,
    /// and the oracle's single up ray reads its sloped edges as open sky
    /// (the bot sweep put one bot back 1,971 times there).
    pub(crate) fn stuck_in(&self, cap: &Capsule) -> Option<super::Embed> {
        match self.solid_embed(cap)? {
            super::Embed::UnderTerrain(d) => {
                let sheet = self
                    .heightfield
                    .as_ref()
                    .map_or(f32::NEG_INFINITY, |h| h.height_at(cap.base.x, cap.base.z));
                (self.ground_under(cap.base, false) >= sheet - 0.05)
                    .then_some(super::Embed::UnderTerrain(d))
            }
            e => Some(e),
        }
    }

    /// No body holds the stepped capsule deeper than the oracle's overlap.
    pub(super) fn clear_of_bodies(&self, cap: &Capsule) -> bool {
        self.collision
            .capsule_overlap(&cap.stepped())
            .is_none_or(|(d, _)| d <= super::EMBED_OVERLAP)
    }

    /// No live vehicle hull holds the stepped capsule deeper than the
    /// oracle's overlap (the rescue's spot only: the oracle skips hulls).
    fn clear_of_hulls(&self, cap: &Capsule) -> bool {
        self.collision
            .capsule_hull_overlap(&cap.stepped())
            .is_none_or(|d| d <= super::EMBED_OVERLAP)
    }

    /// Put unit `i` (embedded in capsule `cap`) back on his safe spot, or
    /// kill him when there is none. A spot a vehicle hull holds is waited
    /// out (module header): his run goes on and the next check asks again.
    fn rescue_unit(&mut self, i: usize, cap: &Capsule) {
        let h = self.handle(i);
        let back = self.units[i].rescue.safe.filter(|&p| {
            let at = Capsule { base: p, ..*cap };
            self.stuck_in(&at).is_none() && self.clear_of_bodies(&at)
        });
        if let Some(p) = back {
            let at = Capsule { base: p, ..*cap };
            if !self.clear_of_hulls(&at) {
                if self.units[i].rescue.stuck < RESCUE_PATIENCE + RESCUE_HULL_WAITS {
                    return;
                }
                self.units[i].rescue.safe = None;
            }
        }
        let back = self.units[i].rescue.safe.and(back);
        let Some(to) = back else {
            self.units[i].rescue = Rescue::default();
            self.rescue_counts[1] = self.rescue_counts[1].saturating_add(1);
            self.kill_environment(h, DeathCause::Embedded);
            return;
        };
        let u = &mut self.units[i];
        let from = u.soldier.pos;
        u.soldier.pos = to;
        u.soldier.vel = Vec3::ZERO;
        u.soldier.air_time = 0.0;
        u.wall_push = Vec3::ZERO;
        u.rescue.stuck = 0;
        self.rescue_counts[0] = self.rescue_counts[0].saturating_add(1);
        self.events.push(Event::Rescued { unit: h, from, to });
    }
}

#[cfg(test)]
impl Sim {
    /// Run unit `i`'s check `n` times in a row, as `n` check ticks would.
    fn rescue_embedded_for_test(&mut self, i: usize, n: u8) {
        for _ in 0..n {
            while !(self.tick + i as u64).is_multiple_of(RESCUE_EVERY) {
                self.tick += 1;
            }
            self.rescue_embedded();
            self.tick += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{Body, CollisionWorld, Layers, Shape};
    use crate::heightfield::Heightfield;
    use crate::unit::{UnitHandle, UnitSpec};
    use glam::Mat4;

    fn sim_with(bodies: Vec<Body>, terrain: f32) -> Sim {
        let mut s = Sim::empty(1);
        s.collision = CollisionWorld::new(bodies, Vec::new());
        s.heightfield = Some(Heightfield {
            size: 65,
            unit: 2.0,
            offset_x: 64.0,
            offset_z: 64.0,
            heights: vec![terrain; 65 * 65],
        });
        s
    }

    fn crate_at(centre: Vec3) -> Body {
        Body::with_layers(
            Mat4::from_translation(centre),
            Shape::Box {
                half: Vec3::new(2.0, 1.0, 2.0),
            },
            Layers::SOLDIER,
        )
    }

    /// Step `s` with no input until `n` ticks have passed.
    fn idle(s: &mut Sim, n: u64) {
        for _ in 0..n {
            s.step(&[]);
        }
    }

    fn checks(n: u8) -> u64 {
        u64::from(n) * RESCUE_EVERY
    }

    /// A soldier who stood clear and is then found inside a crate is put
    /// back where he stood, with a Rescued event and no death.
    #[test]
    fn embedded_soldier_goes_back_to_his_last_clear_spot() {
        let mut s = sim_with(vec![crate_at(Vec3::new(10.0, 1.0, 0.0))], 0.0);
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
        idle(&mut s, checks(2));
        let i = h.index as usize;
        let safe = s.units[i].rescue.safe.expect("a clear spot was recorded");
        assert!(safe.distance(Vec3::ZERO) < 0.5, "{safe}");
        s.units[i].soldier.pos = Vec3::new(10.0, 0.0, 0.0);
        s.units[i].soldier.vel = Vec3::ZERO;
        let mut rescued = None;
        for _ in 0..checks(RESCUE_PATIENCE + 1) {
            s.step(&[]);
            if let Some(e) = s.events.iter().find(|e| matches!(e, Event::Rescued { .. })) {
                rescued = Some(*e);
            }
        }
        match rescued {
            Some(Event::Rescued { unit, to, .. }) => {
                assert_eq!(unit, h);
                assert!(to.distance(safe) < 1e-4);
            }
            e => panic!("want a Rescued event, got {e:?}"),
        }
        assert!(s.units[i].alive);
        assert_eq!(s.units[i].deaths, 0);
        assert!(s.units[i].soldier.pos.distance(safe) < 0.5);
        assert_eq!(s.rescue_counts, [1, 0]);
    }

    /// A soldier held off the ground and still (a wedge: the census's
    /// `float`) is put back on his safe spot after the wedge's patience;
    /// one still for less, or a jet hovering, is left alone.
    #[test]
    fn a_soldier_wedged_in_the_air_goes_back_to_his_last_clear_spot() {
        let mut s = sim_with(Vec::new(), 0.0);
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
        idle(&mut s, checks(2));
        let i = h.index as usize;
        let safe = s.units[i].rescue.safe.expect("a clear spot was recorded");
        let hang = |s: &mut Sim, stance: crate::soldier::Stance, n: u8| {
            for _ in 0..n {
                let u = &mut s.units[i];
                u.soldier.pos = Vec3::new(5.0, 3.0, 0.0);
                u.soldier.grounded = false;
                u.soldier.stance = stance;
                s.rescue_embedded_for_test(i, 1);
            }
        };
        hang(&mut s, crate::soldier::Stance::Hover, WEDGE_PATIENCE + 2);
        assert_eq!(s.units[i].rescue.wedged, 0, "a hover is no wedge");
        hang(&mut s, crate::soldier::Stance::Fall, WEDGE_PATIENCE - 1);
        assert!(s.units[i].soldier.pos.distance(safe) > 1.0, "not yet");
        hang(&mut s, crate::soldier::Stance::Fall, 1);
        assert!(s.units[i].soldier.pos.distance(safe) < 1e-4, "put back");
        assert!(s.units[i].alive);
        assert_eq!(s.rescue_counts, [1, 0]);
        assert_eq!(s.units[i].rescue.wedged, 0);
    }

    /// A soldier held still on a slope in his walking stance, off the
    /// ground three ticks in four (BF2 Felucia's slope wedge), is put back
    /// after the wedge's patience; one who flickers off a tick at a time
    /// (a corner), or hops in place (the jump stance), is left alone.
    #[test]
    fn a_soldier_wedged_standing_on_a_slope_goes_back_to_his_last_clear_spot() {
        let mut s = sim_with(Vec::new(), 0.0);
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
        idle(&mut s, checks(2));
        let i = h.index as usize;
        let safe = s.units[i].rescue.safe.expect("a clear spot was recorded");
        // `n` checks' worth of ticks at `at`, in `stance`, off the ground
        // on `off` of every 4 ticks.
        let hold = |s: &mut Sim, at: Vec3, stance: crate::soldier::Stance, off: u64, n: u8| {
            for _ in 0..n {
                loop {
                    let u = &mut s.units[i];
                    u.soldier.pos = at;
                    u.soldier.stance = stance;
                    // Grounded on the check ticks, off `off` of each 4 (4:
                    // off every tick).
                    u.soldier.grounded = off < 4 && !(1..=off).contains(&((s.tick + i as u64) % 4));
                    let check = (s.tick + i as u64).is_multiple_of(RESCUE_EVERY);
                    s.rescue_embedded();
                    s.tick += 1;
                    if check {
                        break;
                    }
                }
            }
        };
        let corner = Vec3::new(5.0, 3.0, 0.0);
        hold(
            &mut s,
            corner,
            crate::soldier::Stance::Stand,
            1,
            WEDGE_PATIENCE + 2,
        );
        assert_eq!(
            s.units[i].rescue.wedged, 0,
            "a corner's flicker is no wedge"
        );
        hold(
            &mut s,
            corner,
            crate::soldier::Stance::Jump,
            3,
            WEDGE_PATIENCE + 2,
        );
        assert_eq!(s.units[i].rescue.wedged, 0, "a hop in place is no wedge");
        assert_eq!(s.rescue_counts, [0, 0]);
        // The corner's checks found him standing clear there.
        assert!(s.units[i]
            .rescue
            .safe
            .is_some_and(|p| p.distance(corner) < 1e-4));
        assert!(safe.distance(corner) > 1.0);
        let slope = Vec3::new(5.0, 3.0, 4.0);
        hold(
            &mut s,
            slope,
            crate::soldier::Stance::Stand,
            3,
            WEDGE_PATIENCE,
        );
        assert_eq!(s.rescue_counts, [0, 0], "not yet");
        hold(&mut s, slope, crate::soldier::Stance::Stand, 3, 1);
        assert_eq!(s.rescue_counts, [1, 0], "put back");
        assert!(
            s.units[i].soldier.pos.distance(corner) < 1e-4,
            "to the corner, not the slope"
        );
        assert!(s.units[i].alive);
        // A jump held still in the air, off the ground every tick.
        let hang = Vec3::new(5.0, 4.0, 8.0);
        hold(
            &mut s,
            hang,
            crate::soldier::Stance::Jump,
            4,
            WEDGE_PATIENCE,
        );
        assert_eq!(s.rescue_counts, [1, 0], "not yet");
        hold(&mut s, hang, crate::soldier::Stance::Jump, 4, 1);
        assert_eq!(s.rescue_counts, [2, 0], "put back");
        assert!(s.units[i].soldier.pos.distance(corner) < 1e-4);
    }

    /// A slope wedge in a walking stance with no safe spot leaves him be:
    /// no death (a soldier grinding into a wall on a slope where he spawned).
    #[test]
    fn a_slope_wedge_with_no_safe_spot_is_no_death() {
        let mut s = sim_with(Vec::new(), 0.0);
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
        let i = h.index as usize;
        for t in 0..u64::from(WEDGE_PATIENCE + 4) * RESCUE_EVERY {
            let u = &mut s.units[i];
            u.soldier.pos = Vec3::new(5.0, 3.0, 0.0);
            u.soldier.stance = crate::soldier::Stance::Stand;
            u.soldier.grounded = t % 4 == 0;
            s.units[i].rescue.safe = None;
            s.rescue_embedded();
            s.tick += 1;
        }
        assert!(s.units[i].alive);
        assert_eq!(s.rescue_counts, [0, 0]);
    }

    /// No clear spot ever recorded: the embedded soldier dies, one death,
    /// with the Embedded cause.
    #[test]
    fn embedded_soldier_with_no_clear_spot_dies() {
        let mut s = sim_with(vec![crate_at(Vec3::new(0.0, 1.0, 0.0))], 0.0);
        s.battle.unlimited_reinforcements = false;
        s.battle.teams[0].reinforcements = 50.0;
        s.battle.teams[1].reinforcements = 50.0;
        let h = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(0.5, 0.0, 0.0)))
            .expect("spawned");
        let i = h.index as usize;
        s.units[i].soldier.pos = Vec3::new(0.5, 0.0, 0.0);
        let mut cause = None;
        for _ in 0..checks(RESCUE_PATIENCE + 1) {
            s.step(&[]);
            for e in &s.events {
                if let Event::EnvDeath { unit, cause: c } = e {
                    if *unit == h {
                        cause = Some(*c);
                    }
                }
            }
        }
        assert_eq!(cause, Some(DeathCause::Embedded));
        assert!(!s.units[i].alive);
        assert_eq!(s.units[i].deaths, 1);
        // Exactly one ticket, from his own team only.
        assert_eq!(s.battle.teams[0].reinforcements, 49.0);
        assert_eq!(s.battle.teams[1].reinforcements, 50.0);
        assert_eq!(s.rescue_counts, [0, 1]);
        assert_eq!(s.units[i].rescue, Rescue::default());
    }

    /// A stay shorter than the patience is left alone (the A4 sweep's 6-7
    /// tick runs at a cut edge), and its run is forgotten once clear.
    #[test]
    fn a_short_stay_inside_is_not_rescued() {
        let mut s = sim_with(vec![crate_at(Vec3::new(10.0, 1.0, 0.0))], 0.0);
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
        let i = h.index as usize;
        idle(&mut s, checks(2));
        s.units[i].soldier.pos = Vec3::new(10.0, 0.0, 0.0);
        idle(&mut s, checks(RESCUE_PATIENCE - 1));
        assert!(s.units[i].rescue.stuck > 0);
        s.units[i].soldier.pos = Vec3::new(0.0, 0.0, 3.0);
        idle(&mut s, checks(1));
        assert_eq!(s.units[i].rescue.stuck, 0);
        assert_eq!(s.rescue_counts, [0, 0]);
        assert!(s.units[i].alive);
    }

    /// Under the terrain sheet with the sky overhead counts; the switch
    /// turns the whole rule off.
    #[test]
    fn under_the_terrain_counts_and_the_switch_turns_it_off() {
        for off in [false, true] {
            let mut s = sim_with(Vec::new(), 0.0);
            s.rescue_off = off;
            let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
            let i = h.index as usize;
            // Put under the sheet, the sky over him.
            s.units[i].soldier.pos = Vec3::new(0.0, -3.0, 0.0);
            let e = s.solid_embed(&s.unit_capsule(&s.units[i]));
            assert!(
                matches!(e, Some(crate::sim::Embed::UnderTerrain(_))),
                "{e:?}"
            );
            s.rescue_embedded_for_test(i, RESCUE_PATIENCE);
            assert_eq!(s.units[i].alive, off, "off {off}");
        }
    }

    /// Under the sheet on a model's floor (the sim's ground there is the
    /// floor, not the sheet) is not stuck; the same feet with no floor are.
    #[test]
    fn a_floor_under_the_sheet_is_not_stuck() {
        use crate::collision::Cutter;
        use glam::Vec4;
        let floor = Body::with_layers(
            Mat4::from_translation(Vec3::new(0.0, -5.1, 0.0)),
            Shape::Box {
                half: Vec3::new(6.0, 0.1, 6.0),
            },
            Layers::SOLDIER,
        );
        // A cutter round the sheet over the floor, not down to the feet:
        // the oracle (cut at the feet?) still reads under the terrain, the
        // ground query (cut at the sheet?) stands him on the floor.
        let cut = Cutter {
            sphere: [0.0; 4],
            planes: vec![
                Vec4::new(1.0, 0.0, 0.0, -6.0),
                Vec4::new(-1.0, 0.0, 0.0, -6.0),
                Vec4::new(0.0, 0.0, 1.0, -6.0),
                Vec4::new(0.0, 0.0, -1.0, -6.0),
                Vec4::new(0.0, 1.0, 0.0, -1.0),
                Vec4::new(0.0, -1.0, 0.0, -1.0),
            ],
        };
        let mut s = sim_with(vec![floor], 0.0);
        s.collision = CollisionWorld::new(s.collision.bodies().to_vec(), vec![cut]);
        let cap = |p: Vec3| crate::collision::Capsule {
            base: p,
            height: crate::unit::capsule_height(crate::soldier::Stance::Stand),
            radius: crate::unit::SOLDIER_RADIUS,
        };
        let on_floor = cap(Vec3::new(0.0, -5.0, 0.0));
        assert!(matches!(
            s.solid_embed(&on_floor),
            Some(crate::sim::Embed::UnderTerrain(_))
        ));
        assert!((s.ground_under(on_floor.base, false) + 5.0).abs() < 0.01);
        assert_eq!(s.stuck_in(&on_floor), None);
        // Out of the cutter, with no floor: the sheet is his ground, stuck.
        let no_floor = cap(Vec3::new(20.0, -5.0, 0.0));
        assert!(s.stuck_in(&no_floor).is_some());
    }

    /// Shut in a watertight mesh counts; the same box open below (a room
    /// the parity reads as shut in) never does, nor a closed box with an
    /// open shell round the soldier inside it (the oracle's first body).
    #[test]
    fn only_a_watertight_first_body_counts() {
        use crate::collision::box_mesh;
        let half = Vec3::new(3.0, 3.0, 3.0);
        let s = sim_with(
            vec![
                box_mesh(Vec3::new(0.0, 3.0, 0.0), half, false),
                box_mesh(Vec3::new(20.0, 3.0, 0.0), half, true),
            ],
            0.0,
        );
        let at = |x: f32| crate::collision::Capsule {
            base: Vec3::new(x, 0.0, 0.0),
            height: crate::unit::capsule_height(crate::soldier::Stance::Stand),
            radius: crate::unit::SOLDIER_RADIUS,
        };
        assert_eq!(
            s.solid_embed(&at(0.0)),
            Some(crate::sim::Embed::InClosedMesh(0))
        );
        assert_eq!(s.solid_embed(&at(20.0)), None);
        // A room (open shell, first) inside a watertight hull (second).
        let s = sim_with(
            vec![
                box_mesh(Vec3::new(0.0, 3.0, 0.0), half, true),
                box_mesh(Vec3::new(0.0, 3.0, 0.0), half * 2.0, false),
            ],
            0.0,
        );
        assert_eq!(
            s.embed_at(Vec3::ZERO, crate::soldier::Stance::Stand).name(),
            "inshell"
        );
        assert_eq!(s.solid_embed(&at(0.0)), None);
    }

    /// A sim whose one vehicle class has a 4x2x4 SOLDIER box hull, a
    /// soldier who stood clear at the origin, then is found in a crate, and
    /// a hover parked on his safe spot after it was recorded.
    fn parked_on_the_safe_spot() -> (Sim, UnitHandle, crate::vehicle::VehicleHandle, Vec3) {
        use crate::collision::HullPart;
        use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
        let mut s = sim_with(vec![crate_at(Vec3::new(10.0, 1.0, 0.0))], 0.0);
        s.hull_parts = vec![Some(
            vec![HullPart::new(
                Mat4::IDENTITY,
                Shape::Box {
                    half: Vec3::new(2.0, 1.0, 2.0),
                },
                Layers::SOLDIER,
            )]
            .into(),
        )];
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
        let i = h.index as usize;
        idle(&mut s, checks(3));
        let safe = s.units[i].rescue.safe.expect("a clear spot was recorded");
        s.units[i].soldier.pos = Vec3::new(10.0, 0.0, 0.0);
        s.units[i].soldier.vel = Vec3::ZERO;
        let v = s.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: Some(1),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::new(safe.x, 1.0, safe.z),
            yaw: 0.0,
            max_health: 300.0,
            health_type: crate::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        s.sync_hulls();
        let at = crate::collision::Capsule {
            base: safe,
            ..s.unit_capsule(&s.units[i])
        };
        assert!(!s.clear_of_hulls(&at), "the hull holds the safe spot");
        assert!(s.clear_of_bodies(&at), "no map body does");
        (s, h, v, safe)
    }

    /// Review SF1: a vehicle parked on the safe spot is never a put-back
    /// target. The rescue waits for it, then, with it still there, kills
    /// him as with no spot.
    #[test]
    fn a_vehicle_on_the_safe_spot_is_waited_out_then_he_dies() {
        let (mut s, h, _v, _) = parked_on_the_safe_spot();
        let i = h.index as usize;
        let mut cause = None;
        for _ in 0..checks(RESCUE_PATIENCE + RESCUE_HULL_WAITS + 2) {
            s.step(&[]);
            assert!(
                !s.events.iter().any(|e| matches!(e, Event::Rescued { .. })),
                "put back into the hull"
            );
            for e in &s.events {
                if let Event::EnvDeath { unit, cause: c } = e {
                    if *unit == h {
                        cause = Some(*c);
                    }
                }
            }
        }
        assert_eq!(cause, Some(DeathCause::Embedded));
        assert!(!s.units[i].alive);
        assert_eq!(s.rescue_counts, [0, 1]);
    }

    /// The same, but the vehicle leaves while he waits: he is put back on
    /// his spot, alive.
    #[test]
    fn a_vehicle_that_leaves_the_safe_spot_lets_the_rescue_go_on() {
        let (mut s, h, v, safe) = parked_on_the_safe_spot();
        let i = h.index as usize;
        idle(&mut s, checks(RESCUE_PATIENCE + 1));
        assert!(s.units[i].alive);
        assert!(s.units[i].rescue.stuck >= RESCUE_PATIENCE, "still waiting");
        s.remove_vehicle(v);
        s.sync_hulls();
        let mut rescued = None;
        for _ in 0..checks(RESCUE_HULL_WAITS) {
            s.step(&[]);
            if let Some(Event::Rescued { to, .. }) =
                s.events.iter().find(|e| matches!(e, Event::Rescued { .. }))
            {
                rescued = Some(*to);
            }
        }
        let to = rescued.expect("put back once the hull left");
        assert!(to.distance(safe) < 1e-4);
        assert!(s.units[i].alive);
        assert_eq!(s.rescue_counts, [1, 0]);
    }

    /// Review minor 1: like a fall, a forced move's attacker is credited
    /// with the rescue's kill inside his window, and the ticket is still
    /// exactly one from the victim's team.
    #[test]
    fn a_forced_embed_is_the_attackers_kill() {
        let mut s = sim_with(Vec::new(), 0.0);
        s.battle.unlimited_reinforcements = false;
        s.battle.teams[0].reinforcements = 50.0;
        s.battle.teams[1].reinforcements = 50.0;
        let by = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(30.0, 0.0, 30.0)))
            .expect("spawned");
        let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).expect("spawned");
        s.units[h.index as usize].forced_by = Some(crate::sim::ForcedMove::new(by, s.tick));
        let k0 = s.units[by.index as usize].kills;
        s.kill_environment(h, DeathCause::Embedded);
        assert!(!s.units[h.index as usize].alive);
        assert_eq!(s.units[by.index as usize].kills, k0 + 1);
        assert_eq!(s.battle.teams[0].reinforcements, 49.0);
        assert_eq!(s.battle.teams[1].reinforcements, 50.0);
    }

    /// A round where nobody is embedded hashes as it did before the rule:
    /// only a running embedded count reaches the hash.
    #[test]
    fn the_safe_spot_is_not_hashed() {
        let mut r = Rescue::default();
        let mut bytes = Vec::new();
        r.safe = Some(Vec3::new(1.0, 2.0, 3.0));
        r.hash_into(&mut |b: &[u8]| bytes.extend_from_slice(b));
        assert!(bytes.is_empty());
        r.stuck = 1;
        r.hash_into(&mut |b: &[u8]| bytes.extend_from_slice(b));
        assert!(!bytes.is_empty());
    }
}
