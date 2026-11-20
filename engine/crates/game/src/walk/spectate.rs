//! Watching teammates (issue #182, `[ours]`): while the player is dead and his
//! team cannot spawn him (no spawn post left, or no reinforcement for him),
//! the view follows a living teammate instead of holding on the spawn map.
//! Humans come first, by player slot, then bots, by unit; next and previous
//! step through them, and the view goes back to the spawn screens as soon as
//! a spawn is possible again.
//!
//! Presentation only: the sim never reads it, and next / previous read the
//! local pad's existing bits as they reach the tick (no new sim input), so a
//! LAN round's lockstep is untouched. They are bits a dead slot's sim
//! ignores ([`NEXT_BITS`]), so the hidden spawn screens keep their choice.
//! The view is the death camera's killer follow (`death_camera`): the same
//! look point (a soldier's head, a rider's vehicle) and the same clear-line
//! casts, from behind the teammate.

use super::*;
use core::fmath;
use core::sim::{buttons, SpawnStage};

/// `[ours]` Seconds a spawn must stay impossible before the view turns to a
/// teammate, so a post lost and retaken in a moment does not cut the spawn
/// screens away and back. measure: playtest feel on a post that flips.
pub const BLOCKED_HOLD: f32 = 1.0;
/// [`BLOCKED_HOLD`] in ticks, rounded up.
pub const SPECTATE_HOLD_TICKS: usize = (BLOCKED_HOLD * core::TICK_HZ as f32) as usize + 1;
/// `[ours]` How far behind a teammate on foot the view stands.
/// measure: playtest feel; the death camera stands 5 m off.
const FOOT_BACK: f32 = 4.5;
/// `[ours]` How far behind a teammate's vehicle the view stands.
/// measure: playtest feel across walkers, tanks and starfighters.
const VEHICLE_BACK: f32 = 11.0;
/// `[ours]` The view looks this much down per metre forward.
/// measure: playtest feel.
const TILT: f32 = 0.25;
/// `[ours]` The eye eases toward its place at this rate per second.
/// measure: playtest feel behind a fast starfighter.
const FOLLOW_RATE: f32 = 8.0;
/// The eye never comes closer to the look point than this.
const NEAREST: f32 = 0.5;

/// The pad bits that step to the next teammate: fire (R2 / RT; a click
/// with the free pointer comes through the window instead,
/// [`WalkSim::spectate_cycle`]).
///
/// Only bits the dead sim ignores: the spawn post and class bits (A / D,
/// Q, Tab, the wheel, the D-pad, R1) still move the hidden spawn screens'
/// choice in `Sim::spawn_select`, so stepping on them would change the
/// post or class the player picked behind the view.
///
/// The spawn accept and back bits (Space, Enter, E, Backspace; Cross,
/// Triangle) stay live too: behind the view they still walk the hidden
/// stage (Map to Class and back), so the view can end on the class screen.
/// No wrong spawn follows: the accept is refused while the spawn is blocked.
pub const NEXT_BITS: u32 = buttons::FIRE;
/// The pad bits that step to the previous one: secondary fire (L2 / LT)
/// and switch secondary (G / the mouse's forward button, L1 / LB).
pub const PREV_BITS: u32 = buttons::SWITCH_SECONDARY | buttons::SECONDARY_FIRE;

/// One tick's step through the teammates from the pad bits held now and at
/// the last tick: +1 next, -1 previous, 0 neither (or both).
pub fn steps(held: u32, prev: u32) -> i32 {
    let rose = held & !prev;
    let next = rose & NEXT_BITS != 0;
    let back = rose & PREV_BITS != 0;
    i32::from(next) - i32::from(back)
}

/// The teammate view's state (presentation only).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Spectate {
    /// The view is on: the spawn screens are hidden behind it.
    pub active: bool,
    /// The teammate watched; `None` with nobody left to watch.
    pub target: Option<UnitHandle>,
    pub eye: Vec3,
    pub rot: Quat,
    /// The eye is placed for `target` (a new one cuts to it).
    placed: bool,
    /// Seconds the spawn has been impossible, up to [`BLOCKED_HOLD`].
    blocked_for: f32,
    /// The local pad's bits at the last tick.
    prev_bits: u32,
}

impl WalkSim {
    /// Whether the teammate view is up (issue #182): the spawn screens wait
    /// behind it.
    pub fn spectating(&self) -> bool {
        self.spectate.active
    }

    /// The teammate watched, while the view is up.
    pub fn spectate_target(&self) -> Option<UnitHandle> {
        self.spectate.target.filter(|_| self.spectate.active)
    }

    /// Whether the local player's team cannot spawn him now: none of its
    /// spawn posts is left to him, or the reinforcements refuse a human
    /// (`core::battle::Battle::try_spawn`'s player rule).
    pub fn spawn_blocked(&self) -> bool {
        let Some(me) = self.sim.units.get(self.player.index as usize) else {
            return false;
        };
        let team = me.team.min(1);
        let t = &self.sim.battle.teams[usize::from(team)];
        let tickets = t.tickets();
        tickets == 0 || t.alive_players >= tickets || self.sim.player_spawn_posts(team).is_empty()
    }

    /// The player is dead in a live round, past the wait before his spawn,
    /// and cannot spawn.
    fn spectate_wanted(&self) -> bool {
        let Some(me) = self
            .sim
            .units
            .get(self.player.index as usize)
            .filter(|u| u.gen == self.player.gen && !u.alive && u.team < 2)
        else {
            return false;
        };
        if self.sim.outcome.is_some()
            || self.backdrop.is_some()
            || self.round_end.is_some()
            || self.held_seat_view().is_some()
        {
            return false;
        }
        let stage = self
            .sim
            .spawn_stage
            .get(usize::from(self.local_slot))
            .copied()
            .unwrap_or_default();
        // Not yet on a side, or a live class change: nothing to wait for.
        if matches!(stage, SpawnStage::Side | SpawnStage::Alive) {
            return false;
        }
        // The death camera's own time first: the wait before a spawn.
        if me.respawn_at.is_some_and(|t| t > self.sim.tick) {
            return false;
        }
        self.spawn_blocked()
    }

    /// The living teammates in watching order: humans by player slot, then
    /// bots by unit.
    pub fn spectate_order(&self) -> Vec<UnitHandle> {
        let Some(me) = self.sim.units.get(self.player.index as usize) else {
            return Vec::new();
        };
        let team = me.team;
        let mut list: Vec<((u8, u32), UnitHandle)> = self
            .sim
            .units
            .iter()
            .enumerate()
            .filter(|(i, u)| u.alive && u.team == team && *i != self.player.index as usize)
            .map(|(i, u)| {
                let h = UnitHandle {
                    index: i as u32,
                    gen: u.gen,
                };
                (self.spectate_key(h), h)
            })
            .collect();
        list.sort_by_key(|(k, _)| *k);
        list.into_iter().map(|(_, h)| h).collect()
    }

    fn spectate_key(&self, h: UnitHandle) -> (u8, u32) {
        match self
            .sim
            .units
            .get(h.index as usize)
            .and_then(|u| u.spec.player_slot)
        {
            Some(slot) => (0, u32::from(slot)),
            None => (1, h.index),
        }
    }

    /// Step the watched teammate `dir` places along [`WalkSim::spectate_order`]
    /// (wrapping); from a teammate no longer listed, the step goes from his
    /// place in the order. Presentation only.
    pub fn spectate_cycle(&mut self, dir: i32) {
        if !self.spectate.active || dir == 0 {
            return;
        }
        let order = self.spectate_order();
        let next = self.stepped(&order, self.spectate.target, dir);
        if next != self.spectate.target {
            self.spectate.target = next;
            self.spectate.placed = false;
        }
    }

    fn stepped(
        &self,
        order: &[UnitHandle],
        from: Option<UnitHandle>,
        dir: i32,
    ) -> Option<UnitHandle> {
        let first = if dir < 0 { order.last() } else { order.first() };
        let Some(from) = from else {
            return first.copied();
        };
        let key = self.spectate_key(from);
        let found = if dir < 0 {
            order.iter().rev().find(|&&h| self.spectate_key(h) < key)
        } else {
            order.iter().find(|&&h| self.spectate_key(h) > key)
        };
        found.or(first).copied()
    }

    /// One tick of the teammate view; `pad` is the local slot's pad this
    /// tick (its next / previous presses, [`steps`]).
    pub(super) fn step_spectate(&mut self, pad: Option<&PadState>) {
        let held = pad.map_or(0, |p| p.buttons);
        let dir = steps(held, self.spectate.prev_bits);
        self.spectate.prev_bits = held;
        if !self.spectate_wanted() {
            let prev_bits = self.spectate.prev_bits;
            self.spectate = Spectate {
                prev_bits,
                ..Spectate::default()
            };
            return;
        }
        if !self.spectate.active {
            self.spectate.blocked_for += core::TICK_DT;
            if self.spectate.blocked_for + 1e-4 < BLOCKED_HOLD {
                return;
            }
            self.spectate.active = true;
            self.spectate.target = None;
            self.spectate.placed = false;
        }
        // The watched teammate died or is gone: on to the next one.
        let order = self.spectate_order();
        if self.spectate.target.is_none_or(|t| !order.contains(&t)) {
            let next = self.stepped(&order, self.spectate.target, 1);
            self.spectate.target = next;
            self.spectate.placed = false;
        }
        self.spectate_cycle(dir);
        self.place_spectate_eye();
    }

    /// Ease the eye to its place behind the watched teammate, looking at his
    /// head (or his vehicle), a cut on a new teammate.
    fn place_spectate_eye(&mut self) {
        let Some(t) = self.spectate.target else {
            return;
        };
        let Some(look) = self.killer_point(t) else {
            return;
        };
        let Some(u) = self.sim.unit(t) else {
            return;
        };
        let foot = {
            let (s, c) = fmath::sin_cos(u.soldier.yaw);
            Vec3::new(-s, 0.0, -c)
        };
        let (forward, back) = match u.mount.and_then(|m| self.sim.vehicle(m.vehicle)) {
            Some(v) => {
                let f = (v.rot * Vec3::Z).with_y(0.0).normalize_or_zero();
                (if f == Vec3::ZERO { foot } else { f }, VEHICLE_BACK)
            }
            None => (foot, FOOT_BACK),
        };
        let dir = (forward - Vec3::Y * TILT).normalize();
        let clear = self.view_cast(look, -dir, back);
        let goal = look - dir * (back * clear - NEAREST).fmax(NEAREST);
        let s = &mut self.spectate;
        if s.placed {
            let w = 1.0 - fmath::exp(-FOLLOW_RATE * core::TICK_DT);
            s.eye += (goal - s.eye) * w;
        } else {
            s.eye = goal;
            s.placed = true;
        }
        let mut to = (look - s.eye).normalize_or_zero();
        if to == Vec3::ZERO {
            to = dir;
        }
        s.rot = super::death_camera::look_rotation(to);
    }

    /// The placed teammate view's watched teammate, eye and turn, for the
    /// render blend (`crate::render_blend::Pose::cam_spectate`).
    pub fn spectate_pose(&self) -> Option<(UnitHandle, Vec3, Quat)> {
        let s = &self.spectate;
        let t = s.target.filter(|_| s.active && s.placed)?;
        Some((t, s.eye, s.rot))
    }

    /// Draw the teammate view from a blended eye and turn (the render blend
    /// writes them in and puts the tick's own back).
    pub fn set_spectate_pose(&mut self, eye: Vec3, rot: Quat) {
        (self.spectate.eye, self.spectate.rot) = (eye, rot);
    }

    /// The teammate view's camera, while it watches someone.
    pub(super) fn spectate_camera(&self) -> Option<Camera> {
        let s = &self.spectate;
        (s.active && s.placed && s.target.is_some())
            .then(|| flyer_camera::camera_at(s.eye, s.rot, self.fov_y()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat() -> Heightfield {
        Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        }
    }

    fn ticks(w: &mut WalkSim, n: usize) {
        for _ in 0..n {
            w.step_tick(&[PadState::default()]);
        }
    }

    fn press(w: &mut WalkSim, bits: u32) {
        w.step_tick(&[PadState {
            buttons: bits,
            ..PadState::default()
        }]);
        ticks(w, 1);
    }

    /// A walk on flat ground whose player has two bot teammates and an
    /// enemy, then dies with no spawn post for his team.
    fn walk_with_mates() -> (WalkSim, [UnitHandle; 2]) {
        let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat()));
        ticks(&mut w, 5);
        let mate = |w: &mut WalkSim, x: f32| {
            w.sim
                .spawn(&UnitSpec::trooper(0, Vec3::new(x, 0.0, -10.0)))
                .unwrap()
        };
        let a = mate(&mut w, 10.0);
        let b = mate(&mut w, -10.0);
        w.sim
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -40.0)))
            .unwrap();
        (w, [a, b])
    }

    fn kill_player(w: &mut WalkSim) {
        let me = w.player;
        w.sim.units[me.index as usize].respawn_at = None;
        w.sim.kill_environment(me, core::sim::DeathCause::Suicide);
        w.sim.units[me.index as usize].respawn_at = None;
    }

    const HOLD_TICKS: usize = SPECTATE_HOLD_TICKS;

    #[test]
    fn next_and_previous_read_the_existing_bits() {
        use buttons as b;
        assert_eq!(steps(b::FIRE, 0), 1, "R2, a held pointer's fire");
        assert_eq!(steps(b::SWITCH_SECONDARY, 0), -1, "L1, G");
        assert_eq!(steps(b::SECONDARY_FIRE, 0), -1, "L2");
        assert_eq!(steps(b::FIRE, b::FIRE), 0, "held, not pressed");
        assert_eq!(steps(b::FIRE | b::SECONDARY_FIRE, 0), 0);
        // The spawn screens' own bits step nothing: the sim still reads
        // them for the hidden screens.
        for bits in [
            b::SPAWN_POST_LEFT,
            b::SPAWN_POST_RIGHT,
            b::SPAWN_POST_UP,
            b::SPAWN_POST_DOWN,
            b::SWITCH_WEAPON | b::SPAWN_CLASS_NEXT,
            b::SWITCH_WEAPON | b::SPAWN_CLASS_PREV,
            b::SPAWN_CLASS_PREV,
            b::SPAWN_ACCEPT,
            b::SPAWN_BACK,
        ] {
            assert_eq!(steps(bits, 0), 0, "{bits:#x}");
        }
    }

    /// The whole flow: blocked -> after the hold the view watches the first
    /// teammate (the spawn screens hidden); next / previous step and wrap; a
    /// watched teammate's death moves on; a post coming back ends the view.
    #[test]
    fn a_blocked_player_watches_his_teammates_until_he_can_spawn() {
        let (mut w, [a, b]) = walk_with_mates();
        // No post for anyone: the walk without a mission has none.
        assert!(w.sim.player_spawn_posts(0).is_empty());
        kill_player(&mut w);
        assert!(w.spawn_blocked());
        let sim_before = w.sim.tick;
        ticks(&mut w, 1);
        assert!(!w.spectating(), "the hold first");
        ticks(&mut w, HOLD_TICKS);
        assert!(w.spectating());
        assert_eq!(w.spectate_order(), vec![a, b]);
        assert_eq!(w.spectate_target(), Some(a), "the first teammate");
        // The camera is the teammate view: behind and above him, looking at
        // his head.
        let cam = w.camera();
        let head = w.sim.unit(a).unwrap().soldier.pos + Vec3::Y * 1.70;
        let fwd = w.spectate.rot * Vec3::NEG_Z;
        assert!(fwd.dot((head - cam.position).normalize()) > 0.999);
        let gap = (cam.position - head).length();
        assert!((3.0..6.0).contains(&gap), "{gap} m behind");
        // Next, next wraps, previous wraps back.
        press(&mut w, buttons::FIRE);
        assert_eq!(w.spectate_target(), Some(b));
        press(&mut w, buttons::FIRE);
        assert_eq!(w.spectate_target(), Some(a), "wraps");
        press(&mut w, buttons::SWITCH_SECONDARY);
        assert_eq!(w.spectate_target(), Some(b), "previous wraps");
        // The watched teammate dies: the view moves on.
        w.sim.kill_environment(b, core::sim::DeathCause::Suicide);
        w.sim.units[b.index as usize].respawn_at = None;
        ticks(&mut w, 1);
        assert_eq!(w.spectate_target(), Some(a));
        // The pointer's click (the window calls it directly).
        w.spectate_cycle(1);
        assert_eq!(w.spectate_target(), Some(a), "alone, he stays");
        // A spawn becomes possible (a post of his team's with a spawn
        // node): back to the spawn screens at once.
        assert!(w.sim.tick > sim_before);
        let at = Vec3::new(0.0, 0.0, 20.0);
        w.sim.add_cp(
            core::cp::CpDef {
                name: "cp".into(),
                pos: at,
                capture: core::region::Region::sphere(at, 8.0),
                control: None,
                value_bleed: 1,
                value_atk: [1, 1],
                value_def: [1, 1],
                spawn_nodes: vec![(at + Vec3::X * 4.0, 0.0)],
                capture_time: 10.0,
                neutralize_time: 10.0,
            },
            Some(0),
        );
        assert!(!w.spawn_blocked());
        ticks(&mut w, 1);
        assert!(!w.spectating(), "a post back ends the view");
        assert_eq!(w.spectate_target(), None);
    }
    /// The teammate view is drawn between ticks like the death camera: half
    /// a tick in, the camera stands halfway between the two ticks' eyes; a
    /// new teammate cuts (no blend from the last one's eye).
    #[test]
    fn the_teammate_view_is_drawn_between_its_ticks() {
        let (mut w, [a, b]) = walk_with_mates();
        kill_player(&mut w);
        ticks(&mut w, HOLD_TICKS + 1);
        assert_eq!(w.spectate_target(), Some(a));
        w.blend.on = true;
        let step = |w: &mut WalkSim| {
            w.set_frame_fraction(0.0);
            w.advance(core::TICK_DT, &[PadState::default()]);
        };
        step(&mut w);
        let eye0 = w.spectate.eye;
        w.sim.units[a.index as usize].soldier.pos += Vec3::X * 2.0;
        step(&mut w);
        let eye1 = w.spectate.eye;
        assert!(eye0.distance(eye1) > 0.1, "the eye followed him");
        w.set_frame_fraction(0.5);
        w.blend_in();
        let drawn = w.camera().position;
        w.blend_out();
        assert!(
            drawn.abs_diff_eq((eye0 + eye1) * 0.5, 1e-3),
            "{drawn} {eye0} {eye1}"
        );
        assert_eq!(w.spectate.eye, eye1, "the tick's own eye back");
        // A new teammate: the camera cuts to his eye.
        w.spectate_cycle(1);
        step(&mut w);
        assert_eq!(w.spectate_target(), Some(b));
        let cut = w.spectate.eye;
        w.set_frame_fraction(0.5);
        w.blend_in();
        let drawn = w.camera().position;
        w.blend_out();
        assert!(drawn.abs_diff_eq(cut, 1e-4), "{drawn} {cut}");
    }

    /// The reinforcement rule with a post up: as many of his team's humans
    /// alive as it has tickets (the LAN case) blocks him, with the stage
    /// still `Accepted` (a death whose spawn map never opened); a free
    /// ticket ends the view.
    #[test]
    fn living_players_holding_every_ticket_start_the_view() {
        let (mut w, [a, b]) = walk_with_mates();
        add_post(&mut w);
        w.sim.spawn_stage = vec![SpawnStage::Accepted];
        kill_player(&mut w);
        assert!(!w.spawn_blocked(), "a post and tickets");
        // Both teammates are humans of his team (slots 2 and 1: the slot
        // orders them), on the field and holding the last two tickets.
        w.sim.units[a.index as usize].spec.player_slot = Some(2);
        w.sim.units[b.index as usize].spec.player_slot = Some(1);
        let t = &mut w.sim.battle.teams[0];
        t.reinforcements = 2.0;
        t.alive_ai -= 2;
        t.alive_players += 2;
        assert!(w.spawn_blocked());
        ticks(&mut w, HOLD_TICKS + 1);
        assert_eq!(w.sim.outcome, None);
        assert!(w.spectating(), "stage {:?}", w.sim.spawn_stage[0]);
        assert_eq!(w.spectate_order(), vec![b, a], "by player slot");
        assert_eq!(w.spectate_target(), Some(b));
        w.sim.battle.teams[0].reinforcements = 3.0;
        ticks(&mut w, 1);
        assert!(!w.spectating(), "a free ticket ends the view");
    }

    /// No reinforcements left blocks the spawn too; in a round with posts
    /// that is the team's defeat, so the round's end takes over and the view
    /// never starts.
    #[test]
    fn no_reinforcements_left_blocks_the_spawn() {
        let (mut w, _) = walk_with_mates();
        add_post(&mut w);
        kill_player(&mut w);
        w.sim.battle.teams[0].reinforcements = 0.0;
        assert!(w.spawn_blocked());
        ticks(&mut w, HOLD_TICKS + 1);
        assert_eq!(w.sim.outcome, Some(1));
        assert!(!w.spectating(), "the round's end, not the view");
    }

    /// Review of #182: switching teammates never moves the class or post
    /// picked on the hidden spawn screens.
    #[test]
    fn switching_teammates_keeps_the_hidden_class_and_post() {
        let (mut w, [a, b]) = walk_with_mates();
        w.sim.spawn_screens = true;
        // Two classes, so the class screen has somewhere to move.
        let troop = &w.teams[0];
        let class = core::unit::UnitClass {
            consts: troop.consts,
            max_health: troop.max_health,
            health_type: troop.health_type,
            weapon_class: troop.weapon_class,
            total_clips: troop.total_clips,
            weapon_slot: troop.weapon_slot,
            slots: troop.slots.clone(),
            jet: false,
            capture_posts: true,
            drops: troop.drops,
            chunk: None,
            min: 0,
            max: None,
            unlock_points: 0,
            droideka: None,
        };
        w.sim.classes[0] = vec![class.clone(), class];
        kill_player(&mut w);
        for stage in [SpawnStage::Class, SpawnStage::Map] {
            w.sim.spawn_stage = vec![stage];
            ticks(&mut w, HOLD_TICKS + 1);
            assert!(w.spectating(), "{stage:?}");
            let choice = (
                w.sim.spawn_choice.clone(),
                w.sim.spawn_post_choice.clone(),
                w.sim.spawn_stage.clone(),
            );
            let before = w.spectate_target();
            press(&mut w, buttons::FIRE);
            press(&mut w, buttons::SWITCH_SECONDARY);
            press(&mut w, buttons::SECONDARY_FIRE);
            assert_ne!(w.spectate_target(), before, "{stage:?}: stepped");
            assert!(matches!(w.spectate_target(), Some(t) if t == a || t == b));
            assert_eq!(
                (
                    w.sim.spawn_choice.clone(),
                    w.sim.spawn_post_choice.clone(),
                    w.sim.spawn_stage.clone(),
                ),
                choice,
                "{stage:?}: the hidden screens keep their choice"
            );
        }
        // The class screen is live behind the view (the guard above has
        // teeth): its own next press moves the class, not the teammate.
        w.sim.spawn_stage = vec![SpawnStage::Class];
        let (class, mate) = (w.sim.spawn_choice.clone(), w.spectate_target());
        press(&mut w, buttons::SPAWN_CLASS_NEXT);
        assert_ne!(w.sim.spawn_choice, class);
        assert_eq!(w.spectate_target(), mate);
    }

    fn add_post(w: &mut WalkSim) {
        let at = Vec3::new(0.0, 0.0, 20.0);
        w.sim.add_cp(
            core::cp::CpDef {
                name: "cp".into(),
                pos: at,
                capture: core::region::Region::sphere(at, 8.0),
                control: None,
                value_bleed: 1,
                value_atk: [1, 1],
                value_def: [1, 1],
                spawn_nodes: vec![(at + Vec3::X * 4.0, 0.0)],
                capture_time: 10.0,
                neutralize_time: 10.0,
            },
            Some(0),
        );
    }
}
