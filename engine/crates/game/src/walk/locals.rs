//! Several people on one machine (issue #194, `analysis/plan/SPLITSCREEN.md` §5).
//!
//! The first local player is the walk's own `player` and `local_slot`, and everything that reads
//! them is unchanged. Each further local player is a [`LocalPlayer`]: a slot, its unit and its
//! own [`LocalView`], the camera, zoom, death and spawn-backdrop state that belongs to one
//! person's screen. To run the walk's per-player routines for one of them, the walk swaps that
//! person's view and handle in for the first player's ([`WalkSim::swap_local`]), runs the
//! routine and swaps back. With one local player nothing is swapped and nothing differs.
//!
//! The sim is not involved: a local player is a human slot (`Sim::join_human`) stepped from its
//! own `PadState`, as a LAN human is. Rule (§5): nothing here feeds the sim except through that
//! pad, so a respawn is the RESPAWN bit in the pad.
//!
//! In a LAN round (issue #194 B18) a further local player is adopted instead
//! ([`WalkSim::adopt_local`]): the bundles seat its slot (`AddHuman`), as
//! they do every guest, so every machine's sim is the same. Until then it is
//! waiting ([`WalkSim::is_waiting`]), which is not away. Its respawn is the
//! RESPAWN bit in the pad its machine submits: the walk adds none of its own
//! under `netplay.active`, which would change one machine's sim alone; the window takes the
//! press with [`WalkSim::take_respawn_of`] and puts it in that pad.

use super::*;

/// The camera, zoom, death and spawn-backdrop state of one local player's screen.
pub struct LocalView {
    pub(super) zoom_s: f32,
    pub(super) zoom_fov_mix: f32,
    pub(super) cam_place: Option<(f32, Vec3)>,
    pub(super) cam_jet_lag: Vec3,
    pub(super) cam_flyer: Option<FlyerCam>,
    pub(super) cam_gunner: Option<GunnerCam>,
    pub(super) cam_exit: Option<Vec3>,
    pub(super) cam_handover: Option<Handover>,
    pub(super) cam_owner: Option<CamOwner>,
    pub(super) cam_guided: Option<GuidedCam>,
    pub(super) seat_view: Option<Camera>,
    pub(super) seat_view_timer: f32,
    pub(super) cam_pull: f32,
    pub(super) cam_pull_track: Option<(CamKind, Vec3)>,
    pub(super) backdrop: Option<crate::spawn_backdrop::Backdrop>,
    pub(super) death_cam: Option<DeathCam>,
    pub(super) death_seen: Option<UnitHandle>,
    pub(super) spectate: Spectate,
    pub(super) blend: crate::render_blend::Blend,
    pub(super) respawn_requested: bool,
    /// This player's class kills not yet in their profile (`WalkSim::flush_class_kills`).
    pub(super) pending_class_kills: std::collections::BTreeMap<String, u32>,
    /// What this player's earlier soldiers scored before their controller dropped.
    pub(super) round_carry: RoundCarry,
    /// This player's own FOV multiplier (`WalkSim::fov_scale`): their profile's, set from the
    /// Game Menu they open (issue #194 B10).
    pub(super) fov_scale: f32,
}

/// What a local player scored this round before their controller last dropped
/// ([`WalkSim::suspend_local_player`]). The rejoin takes a fresh unit at some free index
/// ([`Sim::join_human`]), whose counters start at zero (or hold a bot's), so the round's record
/// and the class gate add these back. All zero, and `team` `None`, for a player who never dropped.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoundCarry {
    pub(super) kills: u32,
    pub(super) deaths: u32,
    /// The slot's class-gate points (`Sim::spawn_points`) at the last drop. The further players'
    /// gate is `netplay::gate_locals`, the first's [`WalkSim::first_gate_points`].
    pub(super) points: i32,
    /// The side at the last drop; `None`: never dropped this round.
    pub(super) team: Option<u8>,
}

impl Default for LocalView {
    /// A view as the walk starts one.
    fn default() -> Self {
        LocalView {
            zoom_s: 1.0,
            zoom_fov_mix: 0.0,
            cam_place: None,
            cam_jet_lag: Vec3::ZERO,
            cam_flyer: None,
            cam_gunner: None,
            cam_exit: None,
            cam_handover: None,
            cam_owner: None,
            cam_guided: None,
            seat_view: None,
            seat_view_timer: 0.0,
            cam_pull: 1.0,
            cam_pull_track: None,
            backdrop: None,
            death_cam: None,
            death_seen: None,
            spectate: Spectate::default(),
            blend: crate::render_blend::Blend::default(),
            respawn_requested: false,
            pending_class_kills: Default::default(),
            round_carry: RoundCarry::default(),
            fov_scale: 1.0,
        }
    }
}

/// One further person playing on this machine.
pub struct LocalPlayer {
    /// The human slot this player's pad drives.
    pub slot: u8,
    /// This player's unit (follows the slot through respawns).
    pub player: UnitHandle,
    /// The slot has held this player's unit this round: an adopted LAN local
    /// is not until the bundles seat it ([`WalkSim::is_waiting`]).
    pub(super) seated: bool,
    pub(super) view: LocalView,
}

/// What a player who lost their controller leaves behind (see [`WalkSim::suspend_local_player`]).
#[derive(Clone)]
pub struct Suspended {
    team: u8,
    keep: core::sim::SlotKeep,
}

/// What a local player's tick reads before the sim steps (the first player's `step_tick` locals).
pub(super) struct Before {
    pub(super) mount: Option<core::vehicle::Mount>,
    pub(super) camera: Camera,
    pub(super) killer: Option<UnitHandle>,
    /// The kills of this player's unit before the step, for their class kills.
    pub(super) kills: u32,
}

impl WalkSim {
    /// Seat another person on this machine in `slot`: a human slot as a LAN human gets
    /// ([`Sim::join_human`]), waiting on the spawn screens. `team` `None` picks the side with
    /// fewer humans. The caller opens the spawn screens for the slot
    /// ([`Sim::open_spawn_screens`]) as it does for the first player.
    pub fn add_local_player(
        &mut self,
        slot: u8,
        team: Option<u8>,
    ) -> Result<UnitHandle, core::sim::JoinError> {
        if slot == self.local_slot || self.locals.iter().any(|l| l.slot == slot) {
            return Err(core::sim::JoinError::SlotTaken);
        }
        let player = self.sim.join_human(usize::from(slot), team)?;
        // Until their own profile's FOV is installed, the first player's.
        let view = LocalView {
            fov_scale: self.fov_scale,
            ..LocalView::default()
        };
        self.locals.push(LocalPlayer {
            slot,
            player,
            seated: true,
            view,
        });
        Ok(player)
    }

    /// Issue #194 B18, a LAN round: another person on this machine plays
    /// `slot`, which the round's bundles seat (`AddHuman`, or the setup's
    /// humans), never this walk (module header). Its unit is followed from the
    /// slot each tick; until the slot holds one the player is waiting
    /// ([`WalkSim::is_waiting`]) and its per-player tick does not run. Adopting
    /// the walk's own slot, or a slot already adopted, does nothing.
    pub fn adopt_local(&mut self, slot: u8) {
        if slot == self.local_slot || self.locals.iter().any(|l| l.slot == slot) {
            return;
        }
        let held = self.sim.players.get(usize::from(slot)).copied().flatten();
        let view = LocalView {
            fov_scale: self.fov_scale,
            ..LocalView::default()
        };
        self.locals.push(LocalPlayer {
            slot,
            // Until seated, a handle the per-player reads can index (never read
            // as this player's while waiting).
            player: held.unwrap_or(self.player),
            seated: held.is_some(),
            view,
        });
    }

    /// Lets go of the adopted LAN local in `slot` (splitscreen B19): its slot
    /// left this machine (a kick, a host local leaving), so this walk no
    /// longer views or follows it. The bundles hand its unit to a bot, never
    /// this walk. The first player's slot, or a slot not adopted, does
    /// nothing. Call it outside [`WalkSim::with_local`].
    pub fn release_local(&mut self, slot: u8) {
        self.locals.retain(|l| l.slot != slot);
    }

    /// Follows every further local player's unit from its slot (a respawn, a
    /// bundle's `AddHuman`, a replayed round): a slot with no unit keeps the
    /// handle it had.
    pub(crate) fn follow_locals(&mut self) {
        for l in &mut self.locals {
            if let Some(Some(h)) = self.sim.players.get(usize::from(l.slot)) {
                l.player = *h;
                l.seated = true;
            }
        }
    }

    /// Is local player `i` waiting to be seated: an adopted LAN local whose slot
    /// the bundles have not seated yet this round (B18)? Such a player is also
    /// [`WalkSim::is_suspended`] (no unit), but is not away: nothing was taken
    /// from them. Never for the first player.
    pub fn is_waiting(&self, i: usize) -> bool {
        i > 0 && self.locals.get(i - 1).is_some_and(|l| !l.seated)
    }

    /// The number of people on this machine, the first player included.
    pub fn local_count(&self) -> usize {
        1 + self.locals.len()
    }

    /// Local player `i`'s slot: 0 is the first player.
    pub fn local_slot_of(&self, i: usize) -> u8 {
        if i == 0 {
            self.local_slot
        } else {
            self.locals[i - 1].slot
        }
    }

    /// Local player `i`'s unit handle: 0 is the first player.
    pub fn local_unit(&self, i: usize) -> UnitHandle {
        if i == 0 {
            self.player
        } else {
            self.locals[i - 1].player
        }
    }

    /// Ask for a respawn for local player `i` (0 is the first player): the RESPAWN bit in its
    /// pad on the next tick, as [`WalkSim::request_respawn`] does for the first.
    pub fn request_respawn_of(&mut self, i: usize) {
        if i == 0 {
            self.respawn_requested = true;
        } else {
            self.locals[i - 1].view.respawn_requested = true;
        }
    }

    /// Takes further local player `i`'s pending respawn press (`i` >= 1; always
    /// false for the first player, whose press `step_tick` applies). In a LAN
    /// round (`netplay.active`) `step_tick` leaves it set, since one
    /// machine's own press would part its sim from the others': the window
    /// takes it here and puts RESPAWN in the pad it submits for that player.
    pub fn take_respawn_of(&mut self, i: usize) -> bool {
        i > 0
            && self
                .locals
                .get_mut(i - 1)
                .is_some_and(|l| std::mem::take(&mut l.view.respawn_requested))
    }

    /// Run `f` with local player `i`'s view and unit as the walk's own (swapped back after), so
    /// every per-player routine and reader (`camera`, `first_person`, ...) answers for that
    /// person. `i == 0` is the walk as it stands.
    pub fn with_local<R>(&mut self, i: usize, f: impl FnOnce(&mut WalkSim) -> R) -> R {
        if i == 0 {
            return f(self);
        }
        self.swap_local(i - 1);
        let r = f(self);
        self.swap_local(i - 1);
        r
    }

    /// Is local player `i` away (their controller is gone and a bot plays their soldier)? Also
    /// true for an adopted LAN local not seated yet ([`WalkSim::is_waiting`] tells the two
    /// apart): either way the slot holds no unit of theirs, so per-player readers skip them.
    pub fn is_suspended(&self, i: usize) -> bool {
        self.sim
            .players
            .get(usize::from(self.local_slot_of(i)))
            .is_none_or(|p| p.is_none())
    }

    /// The side of the player this walk views now (the first player's, or local player `k`'s
    /// inside [`WalkSim::with_local`]): their soldier's, or while they are away the side they
    /// left on (their handle may name a bot's unit then; issue #194 B22).
    pub fn viewer_team(&self) -> u8 {
        let away = self
            .sim
            .players
            .get(usize::from(self.local_slot))
            .is_none_or(|p| p.is_none());
        match (away, self.round_carry.team) {
            (true, Some(team)) => team,
            _ => self.sim.units[self.player.index as usize].team,
        }
    }

    /// Local player `i`'s controller is gone: a bot of their team takes over the soldier where it
    /// stands ([`Sim::leave_human`]) and the others play on. Returns what to give back to
    /// [`WalkSim::resume_local_player`]: the team, and the slot's cheats, aim assist and medal
    /// career, which the leave clears. `None` when `i` is already away. The player's death
    /// camera, spawn backdrop, seat view and held guided-rocket view are dropped with them; a
    /// rocket or droid they were flying is let go by the sim on the next step.
    ///
    /// While away, a player's handle (`local_unit(i)`) may name a unit index a bot now holds:
    /// the window must skip every per-player reader of that player (camera, HUD, minimap, black
    /// box rows, pause data) while [`WalkSim::is_suspended`], and draw the notice instead.
    pub fn suspend_local_player(&mut self, i: usize) -> Option<Suspended> {
        if self.is_suspended(i) {
            return None;
        }
        let h = self.local_unit(i);
        let team = self
            .sim
            .unit(h)
            .map_or_else(|| self.sim.units[h.index as usize].team, |u| u.team);
        let slot = usize::from(self.local_slot_of(i));
        let keep = self.sim.keep_slot(slot);
        // The soldier's counters stay with the unit, which a bot may take over: bank them now.
        let (kills, deaths) = self
            .sim
            .units
            .get(h.index as usize)
            .filter(|u| u.gen == h.gen)
            .map_or((0, 0), |u| (u.kills, u.deaths));
        let points = self.sim.spawn_points.get(slot).copied().unwrap_or(0);
        self.sim.leave_human(slot);
        self.with_local(i, |w| {
            let c = &mut w.round_carry;
            c.kills = c.kills.saturating_add(kills);
            c.deaths = c.deaths.saturating_add(deaths);
            c.points = points;
            c.team = Some(team.min(1));
            w.death_cam = None;
            w.death_seen = None;
            w.backdrop = None;
            w.seat_view = None;
            w.seat_view_timer = 0.0;
            // A guided rocket or remote droid they flew is the old unit's: the sim lets it go on
            // the next step (its owner is gone), and their held guided view goes now, theirs
            // alone (issue #194 B21).
            w.cam_guided = None;
        });
        Some(Suspended {
            team: team.min(1),
            keep,
        })
    }

    /// Issue #194 B20, a LAN round: local player `i`'s slot left the
    /// round through the bundles (`RemoveHuman`, its pad away): bank its
    /// soldier's counters and side in its round carry, and drop its held
    /// views, as [`WalkSim::suspend_local_player`] does offline. The sim's
    /// side of the removal is the bundle's (`netplay::apply`); this is the
    /// viewer's alone.
    pub fn bank_removed_local(&mut self, i: usize, kills: u32, deaths: u32, team: u8) {
        if i >= self.local_count() {
            return;
        }
        self.with_local(i, |w| {
            let c = &mut w.round_carry;
            c.kills = c.kills.saturating_add(kills);
            c.deaths = c.deaths.saturating_add(deaths);
            c.team = Some(team.min(1));
            w.death_cam = None;
            w.death_seen = None;
            w.backdrop = None;
            w.seat_view = None;
            w.seat_view_timer = 0.0;
            w.cam_guided = None;
        });
    }

    /// Local player `i`'s controller is back: they join again on `team` behind the spawn screens
    /// at no ticket cost ([`Sim::join_human`]), and choose a post and a class as any joiner does.
    /// The new unit may sit at another index, whose round points are not theirs: an installed
    /// slot's points baseline is taken again on the next gate, and the gate adds back the points
    /// they had at the drop ([`RoundCarry`]).
    pub fn resume_local_player(
        &mut self,
        i: usize,
        away: Suspended,
    ) -> Result<UnitHandle, core::sim::JoinError> {
        let slot = self.local_slot_of(i);
        let h = self.sim.join_human(usize::from(slot), Some(away.team))?;
        self.sim.restore_slot(usize::from(slot), h, away.keep);
        if let Some(base) = self.netplay.baseline.get_mut(&slot) {
            *base = None;
        }
        let carried = self.with_local(i, |w| w.round_carry.points);
        self.sim.set_spawn_points(usize::from(slot), carried);
        if i == 0 {
            self.player = h;
        } else {
            self.locals[i - 1].player = h;
        }
        Ok(h)
    }

    /// The first local player who is not away (issue #194, `SPLITSCREEN.md` 15 and 18.1 item 6):
    /// whom the music and the commander's voice follow, and whose ear leads the mix
    /// (`event_sounds::listeners`). `Some(0)` with one local player. `None` when every local
    /// player is away: each handle may then name a bot's unit, so nothing per-player is read
    /// (the music and voice keep their last side, the mix its last ears).
    pub fn first_awake(&self) -> Option<usize> {
        if self.locals.is_empty() {
            return Some(0);
        }
        (0..self.local_count()).find(|&i| !self.is_suspended(i))
    }

    /// [`WalkSim::first_awake`]'s unit: the walk's own `player` with one local player.
    pub fn first_awake_unit(&self) -> Option<UnitHandle> {
        self.first_awake().map(|i| self.local_unit(i))
    }

    /// Local player `i`'s camera (0 is the first player).
    pub fn camera_of(&mut self, i: usize) -> Camera {
        self.with_local(i, |w| w.camera())
    }

    /// [`WalkSim::with_local`] in two halves, for a caller whose step borrows more than the
    /// walk (the window's spawn-screen pointer): swaps local player `i`'s view and unit in as the
    /// walk's own, and the same call again swaps them back. `i == 0` does nothing.
    pub fn swap_local_view(&mut self, i: usize) {
        if i != 0 {
            self.swap_local(i - 1);
        }
    }

    /// Each further local player's class kills and medal counts into the profile of their
    /// Players-screen seat (`profiles[seat_of(k)]`, the battle's one copy of it, `App.split_profiles`;
    /// issue #194, `SPLITSCREEN.md` 18.1 item 1), and that slot's class costs
    /// (`netplay::PlayerProfile::unlock_kills`) follow the new counts at once, as the first
    /// player's do. A player whose seat holds no profile (seat 0, the first player's, or the
    /// developer flag's unsaved seats) has nowhere to keep them: their class kills are dropped.
    /// Returns the seats whose profile changed, to save. Nothing with one local player.
    pub fn flush_local_careers(
        &mut self,
        profiles: &mut [Option<crate::profile::Profile>],
        seat_of: &dyn Fn(usize) -> usize,
    ) -> Vec<usize> {
        let mut changed = Vec::new();
        for k in 1..self.local_count() {
            let seat = seat_of(k);
            let Some(p) = profiles
                .get_mut(seat)
                .filter(|_| seat != 0)
                .and_then(Option::as_mut)
            else {
                self.with_local(k, |w| w.pending_class_kills.clear());
                continue;
            };
            if self.with_local(k, |w| w.flush_career_progress(p)) {
                let slot = self.local_slot_of(k);
                if let Some(installed) = self.netplay.profiles.get_mut(&slot) {
                    installed.unlock_kills = crate::netplay::PlayerProfile::unlock_kills_of(p);
                }
                changed.push(seat);
            }
        }
        changed
    }

    /// Local player `i`'s class-gate points carried over a dropped controller ([`RoundCarry`]).
    pub fn carried_points(&mut self, i: usize) -> i32 {
        self.with_local(i, |w| w.round_carry.points)
    }

    /// The first local player's class-gate points in a split round (issue #194), from `raw`, the
    /// round points of the unit they hold now (the feed's row for `self.player`). Never dropped
    /// this round: `raw`, as with one local player. Away: the points at the drop (the handle may
    /// name a bot's unit). Back: what the new unit scored since its first gate (its baseline,
    /// taken again on each rejoin by [`WalkSim::resume_local_player`]) on top of the carried
    /// points, as the further players' `netplay::gate_slot` does.
    pub fn first_gate_points(&mut self, raw: i32) -> i32 {
        let carry = self.round_carry;
        if carry.team.is_none() {
            return raw;
        }
        if self.is_suspended(0) {
            return carry.points;
        }
        let base = self.netplay.baseline.entry(self.local_slot).or_insert(None);
        let base = *base.get_or_insert(raw);
        raw - base + carry.points
    }

    /// Local player `k`'s round points for their profile when their controller dropped this
    /// round (the feed's row is their unit index's, which the rejoin may have changed): the class
    /// gate's (`Sim::spawn_points`), or the points at the drop while away. `None` for a player who
    /// never dropped: the feed's row is theirs.
    pub fn carried_round_points(&mut self, k: usize) -> Option<i32> {
        let carry = self.with_local(k, |w| w.round_carry);
        carry.team?;
        let slot = usize::from(self.local_slot_of(k));
        Some(match self.is_suspended(k) {
            true => carry.points,
            false => self.sim.spawn_points.get(slot).copied().unwrap_or(0),
        })
    }

    /// Each further local player's round into their seat's profile, as
    /// [`WalkSim::record_battle`] records the first player's: `stats(k)` is local player `k`'s
    /// own feed's battle stats. A player whose controller dropped this round has their points
    /// from the class gate instead (the feed's row is their unit index's, which the rejoin may
    /// have changed). Returns the seats recorded, to save.
    pub fn record_local_battles(
        &mut self,
        profiles: &mut [Option<crate::profile::Profile>],
        seat_of: &dyn Fn(usize) -> usize,
        stats: &dyn Fn(usize) -> crate::profile::BattleStats,
    ) -> Vec<usize> {
        let mut recorded = Vec::new();
        for k in 1..self.local_count() {
            let seat = seat_of(k);
            let Some(p) = profiles
                .get_mut(seat)
                .filter(|_| seat != 0)
                .and_then(Option::as_mut)
            else {
                continue;
            };
            let mut s = stats(k);
            if let Some(points) = self.carried_round_points(k) {
                s.points = points;
            }
            self.with_local(k, |w| w.record_battle(p, s));
            recorded.push(seat);
        }
        recorded
    }

    fn swap_local(&mut self, k: usize) {
        use std::mem::swap;
        let l = &mut self.locals[k];
        swap(&mut self.player, &mut l.player);
        let slot = &mut l.slot;
        swap(&mut self.local_slot, slot);
        let v = &mut l.view;
        swap(&mut self.zoom_s, &mut v.zoom_s);
        swap(&mut self.zoom_fov_mix, &mut v.zoom_fov_mix);
        swap(&mut self.cam_place, &mut v.cam_place);
        swap(&mut self.cam_jet_lag, &mut v.cam_jet_lag);
        swap(&mut self.cam_flyer, &mut v.cam_flyer);
        swap(&mut self.cam_gunner, &mut v.cam_gunner);
        swap(&mut self.cam_exit, &mut v.cam_exit);
        swap(&mut self.cam_handover, &mut v.cam_handover);
        swap(&mut self.cam_owner, &mut v.cam_owner);
        swap(&mut self.cam_guided, &mut v.cam_guided);
        swap(&mut self.seat_view, &mut v.seat_view);
        swap(&mut self.seat_view_timer, &mut v.seat_view_timer);
        swap(&mut self.cam_pull, &mut v.cam_pull);
        swap(&mut self.cam_pull_track, &mut v.cam_pull_track);
        swap(&mut self.backdrop, &mut v.backdrop);
        swap(&mut self.death_cam, &mut v.death_cam);
        swap(&mut self.death_seen, &mut v.death_seen);
        swap(&mut self.spectate, &mut v.spectate);
        swap(&mut self.blend, &mut v.blend);
        swap(&mut self.respawn_requested, &mut v.respawn_requested);
        swap(&mut self.pending_class_kills, &mut v.pending_class_kills);
        swap(&mut self.round_carry, &mut v.round_carry);
        swap(&mut self.fov_scale, &mut v.fov_scale);
    }

    /// Each further local player's reads before the sim steps.
    pub(super) fn locals_before(&mut self) -> Vec<(Before, bool)> {
        (1..self.local_count())
            .map(|i| {
                self.with_local(i, |w| {
                    // In a LAN round the press waits for the window
                    // ([`WalkSim::take_respawn_of`]) to put it in the pad.
                    let respawn = !w.netplay.active && std::mem::take(&mut w.respawn_requested);
                    (
                        Before {
                            mount: w.sim.unit(w.player).and_then(|u| u.mount),
                            camera: w.camera(),
                            killer: w.killer_of(w.player),
                            kills: w.sim.units[w.player.index as usize].kills,
                        },
                        respawn,
                    )
                })
            })
            .collect()
    }

    /// The pads of this tick with each further local player's pending respawn press set.
    pub(super) fn pads_with_local_respawns(
        &self,
        pads: &[PadState],
        respawns: &[bool],
    ) -> Vec<PadState> {
        let mut out = pads.to_vec();
        for (k, &asked) in respawns.iter().enumerate() {
            if asked {
                let slot = usize::from(self.locals[k].slot);
                if out.len() <= slot {
                    out.resize(slot + 1, PadState::default());
                }
                out[slot].buttons |= core::sim::buttons::RESPAWN;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sim::{buttons, DeathCause, SpawnStage};

    /// Two teams with a post each on flat ground, a first player and one further local player
    /// (the shared fixture, `walk::two_locals_walk`).
    fn two_locals() -> WalkSim {
        crate::walk::two_locals_walk()
    }

    fn pads(a: PadState, b: PadState) -> [PadState; 2] {
        [a, b]
    }

    fn tap(w: &mut WalkSim, slot: usize, bit: u32) {
        let mut p = [PadState::default(), PadState::default()];
        p[slot].buttons = bit;
        w.step_tick(&p);
        w.step_tick(&[PadState::default(), PadState::default()]);
    }

    /// Issue #194 phase 0 acceptance: two local humans on one walk both spawn, walk, fire, die and
    /// respawn from their own pads; the first player's own fields stay the first player's.
    #[test]
    fn two_local_humans_spawn_walk_fire_die_and_respawn() {
        let mut w = two_locals();
        assert_eq!(w.local_count(), 2);
        assert_eq!((w.local_slot_of(0), w.local_slot_of(1)), (0, 1));
        let first = w.player;
        // Each spawns through its own screens, from its own pad.
        for slot in 0..2 {
            if !w.sim.unit(w.local_unit(slot)).is_some_and(|u| u.alive) {
                w.sim.open_spawn_screens(slot, false);
                tap(&mut w, slot, buttons::SPAWN_ACCEPT);
                tap(&mut w, slot, buttons::SPAWN_ACCEPT);
            }
        }
        for i in 0..2 {
            assert!(
                w.sim.unit(w.local_unit(i)).is_some_and(|u| u.alive),
                "local player {i} spawned"
            );
        }
        assert_eq!(
            w.player, first,
            "the first player's handle is still the walk's own"
        );
        // Both walk and fire from their own pad.
        let at = |w: &WalkSim, i: usize| w.sim.unit(w.local_unit(i)).unwrap().soldier.pos;
        let start = [at(&w, 0), at(&w, 1)];
        let go = |ly: i16| PadState {
            ly,
            buttons: buttons::FIRE,
            ..Default::default()
        };
        for t in 0..60 {
            w.step_tick(&pads(go(20000), go(-20000)));
            for i in 0..2 {
                assert!(
                    w.sim.unit(w.local_unit(i)).is_some(),
                    "local player {i} is gone after tick {t}"
                );
            }
        }
        for i in 0..2 {
            assert!(
                (at(&w, i) - start[i]).length() > 1.0,
                "local player {i} walked"
            );
            assert!(
                w.sim.unit(w.local_unit(i)).unwrap().weapon.shots_fired > 0,
                "local player {i} fired"
            );
        }
        // Each camera follows its own unit.
        let (c0, c1) = (w.camera_of(0), w.camera_of(1));
        assert!((c0.position - c1.position).length() > 1.0);
        // Both die and each respawns by its own press.
        for i in 0..2 {
            let h = w.local_unit(i);
            w.sim.kill_environment(h, DeathCause::Water);
        }
        for _ in 0..3 {
            w.step_tick(&pads(PadState::default(), PadState::default()));
        }
        assert!(
            w.with_local(0, |w| w.death_cam.is_some()),
            "player 0's death camera"
        );
        assert!(
            w.with_local(1, |w| w.death_cam.is_some()),
            "player 1's death camera"
        );
        assert!(w.sim.unit(w.local_unit(0)).is_none() && w.sim.unit(w.local_unit(1)).is_none());
        // The respawn timer runs out and each slot's spawn screens come back; each accepts.
        for _ in 0..900 {
            if (0..2).all(|s| w.sim.spawn_stage[s] != SpawnStage::Accepted) {
                break;
            }
            w.step_tick(&pads(PadState::default(), PadState::default()));
        }
        for i in 0..2 {
            let slot = usize::from(w.local_slot_of(i));
            assert_ne!(
                w.sim.spawn_stage[slot],
                SpawnStage::Accepted,
                "slot {slot} waits"
            );
            tap(&mut w, slot, buttons::SPAWN_ACCEPT);
            tap(&mut w, slot, buttons::SPAWN_ACCEPT);
        }
        for i in 0..2 {
            assert!(
                w.sim.unit(w.local_unit(i)).is_some_and(|u| u.alive),
                "local player {i} respawned"
            );
        }
        assert!(w.with_local(0, |w| w.death_cam.is_none()));
        assert!(w.with_local(1, |w| w.death_cam.is_none()));
        // The pause menu's respawn press of one player kills that player and no other.
        w.request_respawn_of(1);
        w.step_tick(&pads(PadState::default(), PadState::default()));
        assert!(
            w.sim.unit(w.local_unit(0)).is_some_and(|u| u.alive),
            "player 0 unharmed"
        );
        assert!(
            w.sim.unit(w.local_unit(1)).is_none(),
            "player 1 asked to respawn"
        );
    }

    /// Issue #194 B18: in a LAN round a further local's respawn press stays
    /// for the window to take and put in that player's pad; the walk's tick
    /// never applies it.
    #[test]
    fn a_lan_round_keeps_a_local_respawn_for_the_pad() {
        let mut w = two_locals();
        for slot in 0..2 {
            w.sim.open_spawn_screens(slot, false);
            tap(&mut w, slot, buttons::SPAWN_ACCEPT);
            tap(&mut w, slot, buttons::SPAWN_ACCEPT);
        }
        w.netplay.active = true;
        w.request_respawn_of(1);
        w.step_tick(&pads(PadState::default(), PadState::default()));
        assert!(
            w.sim.unit(w.local_unit(1)).is_some_and(|u| u.alive),
            "the tick applied no press of its own"
        );
        assert!(!w.take_respawn_of(0), "the first player's is the walk's");
        assert!(w.take_respawn_of(1), "the press waits for the pad");
        assert!(!w.take_respawn_of(1), "taken once");
        assert!(!w.take_respawn_of(5), "no such player");
    }

    /// Issue #194: one controller disconnecting takes only that player off the field; the other
    /// plays on, and the returning player rejoins through the spawn screens.
    #[test]
    fn a_lost_controller_leaves_a_bot_and_the_others_play_on() {
        for gone in 0..2 {
            let other = 1 - gone;
            let mut w = two_locals();
            for slot in 0..2 {
                w.sim.open_spawn_screens(slot, false);
                tap(&mut w, slot, buttons::SPAWN_ACCEPT);
                tap(&mut w, slot, buttons::SPAWN_ACCEPT);
            }
            // A career, cheats and assist the slot holds before the controller goes.
            let slot_before = usize::from(w.local_slot_of(gone));
            let life = w.local_unit(gone);
            w.sim
                .install_slot_medal_career(slot_before, life, core::sim::MedalCounts::default());
            let cheat_bits = core::sim::Cheats {
                invincible: true,
                ..Default::default()
            };
            w.sim.set_slot_cheats(slot_before, Some(cheat_bits));
            let bots = |w: &WalkSim| {
                w.sim
                    .units
                    .iter()
                    .filter(|u| u.alive && !u.is_player)
                    .count()
            };
            let ai_before = bots(&w);
            assert!(!w.is_suspended(gone));
            let away = w.suspend_local_player(gone).expect("they were playing");
            assert!(w.is_suspended(gone) && !w.is_suspended(other));
            assert!(w.suspend_local_player(gone).is_none(), "already away");
            assert_eq!(bots(&w), ai_before + 1, "a bot took over the soldier");
            // The other player plays on through many ticks with the gone one's pad neutral.
            let start = w.sim.unit(w.local_unit(other)).unwrap().soldier.pos;
            let walk = PadState {
                ly: 20000,
                ..Default::default()
            };
            for _ in 0..90 {
                let mut p = [PadState::default(), PadState::default()];
                p[other] = walk;
                w.step_tick(&p);
            }
            let now = w.sim.unit(w.local_unit(other)).unwrap().soldier.pos;
            assert!((now - start).length() > 1.0, "player {other} kept walking");
            // The controller comes back: they rejoin behind the screens and play again.
            w.resume_local_player(gone, away)
                .expect("their slot is free");
            assert!(!w.is_suspended(gone));
            let slot = usize::from(w.local_slot_of(gone));
            let back = w.sim.keep_slot(slot);
            assert!(back.has_career(), "the medal career came back");
            assert_eq!(
                w.sim.slot_cheats.get(slot).copied().flatten(),
                Some(cheat_bits),
                "the slot's cheats came back"
            );
            assert_ne!(w.sim.spawn_stage[slot], SpawnStage::Accepted);
            tap(&mut w, slot, buttons::SPAWN_ACCEPT);
            tap(&mut w, slot, buttons::SPAWN_ACCEPT);
            assert!(
                w.sim.unit(w.local_unit(gone)).is_some_and(|u| u.alive),
                "player {gone} is back on the field"
            );
            let _ = w.camera_of(gone);
        }
    }

    /// A profile named `name` with `kills` career kills for `class`.
    fn profile(name: &str, class_kills: &[(&str, u32)]) -> crate::profile::Profile {
        let mut p = crate::profile::Profile::default();
        p.name = name.to_owned();
        for (c, k) in class_kills {
            p.career.class_kills.insert((*c).to_owned(), *k);
        }
        p
    }

    /// Seat `k`'s profile into its slot as the window does at the first seating
    /// (`App::install_seat_profiles`).
    fn install(w: &mut WalkSim, k: usize, p: &crate::profile::Profile) {
        let slot = w.local_slot_of(k);
        crate::netplay::install_local_profile(
            w,
            slot,
            &crate::netplay::PlayerProfile::from_profile(p),
        );
    }

    /// Both players on the field through their own spawn screens.
    fn spawn_both(w: &mut WalkSim) {
        for slot in 0..w.local_count() {
            w.sim.open_spawn_screens(slot, false);
            tap(w, slot, buttons::SPAWN_ACCEPT);
            tap(w, slot, buttons::SPAWN_ACCEPT);
        }
        for i in 0..w.local_count() {
            assert!(w.sim.unit(w.local_unit(i)).is_some_and(|u| u.alive));
        }
    }

    /// Local player `k` shoots down an enemy trooper stood 20 m before them.
    fn kill_one(w: &mut WalkSim, k: usize) {
        let me = w.local_unit(k);
        let (pos, team) = {
            let u = &mut w.sim.units[me.index as usize];
            u.soldier.pitch = 0.0;
            u.soldier.yaw = 0.0;
            u.health.max = 100_000.0;
            u.health.cur = 100_000.0;
            (u.soldier.pos, u.team)
        };
        let target = w
            .sim
            .spawn(&core::unit::UnitSpec::trooper(
                1 - team,
                pos + Vec3::new(0.4, 0.0, -20.0),
            ))
            .expect("a target");
        let slot = usize::from(w.local_slot_of(k));
        for _ in 0..240 {
            if w.sim.unit(target).is_none() {
                break;
            }
            let mut p = vec![PadState::default(); w.local_count().max(slot + 1)];
            p[slot].buttons = buttons::FIRE;
            w.step_tick(&p);
        }
        assert!(w.sim.unit(target).is_none(), "player {k} killed the target");
    }

    fn class_of(w: &WalkSim, k: usize) -> String {
        let u = &w.sim.units[w.local_unit(k).index as usize];
        w.teams[usize::from(u.team)].classes[usize::from(u.spec.class)]
            .name
            .clone()
    }

    /// Issue #194 B3: P2's kills, class kills and medals land in P2's own seat profile; P1's
    /// profile is untouched by them.
    #[test]
    fn p2s_kills_class_kills_and_medals_land_in_p2s_profile_only() {
        let mut w = two_locals();
        let mut p1 = profile("Luke", &[]);
        w.sim.install_medal_career(w.player, p1.career.medals);
        let mut seats: [Option<crate::profile::Profile>; 4] = Default::default();
        seats[1] = Some(profile("Han", &[]));
        install(&mut w, 1, seats[1].as_ref().unwrap());
        spawn_both(&mut w);
        let p1_before = p1.clone();
        kill_one(&mut w, 1);
        let class = class_of(&w, 1);
        // The split loop's medal points for P2 (`events.rs`: P2's feed's positive points).
        w.sim.score_medal_points(w.local_unit(1), 12);
        assert!(w.flush_career_progress(&mut p1) || p1 == p1_before);
        let changed = w.flush_local_careers(&mut seats, &|k| k);
        assert_eq!(changed, vec![1]);
        let p2 = seats[1].as_ref().unwrap();
        assert!(p2.career.class_kills.get(&class).copied().unwrap_or(0) >= 1);
        assert_eq!(p2.career.medals[6], 1, "P2 earned the 12-point medal");
        assert_eq!(p1.career.class_kills, p1_before.career.class_kills);
        assert_eq!(p1.career.medals, p1_before.career.medals);
        // The round: P2's kills in P2's profile, P1's (none) in P1's.
        let kills = w.sim.units[w.local_unit(1).index as usize].kills;
        w.record_battle(&mut p1, Default::default());
        let recorded = w.record_local_battles(&mut seats, &|k| k, &|_| Default::default());
        assert_eq!(recorded, vec![1]);
        let p2 = seats[1].as_ref().unwrap();
        assert_eq!((p2.career.rounds, p2.career.kills), (1, kills));
        assert_eq!((p1.career.rounds, p1.career.kills), (1, 0));
    }

    /// Issue #194 B3: P2's class costs come from P2's own career, P1's from P1's.
    #[test]
    fn p2_pays_its_own_class_costs() {
        let mut w = two_locals();
        let officer = core::unit::UnitClass {
            consts: core::soldier::SoldierConstants::default(),
            max_health: 300.0,
            health_type: core::weapon::HealthType::Person,
            weapon_class: 0,
            total_clips: 6,
            weapon_slot: 0,
            slots: std::sync::Arc::new([]),
            jet: false,
            capture_posts: true,
            drops: core::sim::DropTable::default(),
            chunk: None,
            min: 0,
            max: None,
            unlock_points: 8,
            droideka: None,
        };
        for t in 0..2 {
            w.teams[t].classes = vec![ClassVisual {
                name: "all_inf_officer".into(),
                ..ClassVisual::default()
            }];
            w.sim.classes[t] = vec![officer.clone()];
        }
        let p1 = profile("Luke", &[]);
        let p2 = profile("Han", &[("all_inf_officer", 1000)]);
        install(&mut w, 1, &p2);
        crate::unlocks::set_player_gate(&mut w, 0, &p1.career);
        crate::netplay::gate_locals(&mut w, None);
        for team in 0..2u8 {
            assert_eq!(w.sim.spawn_cost(0, team, 0), 8, "P1 pays the class's cost");
            assert_eq!(w.sim.spawn_cost(1, team, 0), 0, "P2 unlocked it for good");
        }
    }

    /// Issue #194 B3: with seats 0, 1 and 2 seated, each local's round lands in its own seat's
    /// profile, whatever the local order; seat 0 (the host's, `App.profile`) is never written here.
    #[test]
    fn each_locals_round_lands_in_its_own_seats_profile() {
        for order in [[0usize, 1, 2], [0, 2, 1]] {
            let mut w = two_locals();
            w.add_local_player(2, Some(0)).expect("slot 2 is free");
            let mut seats: [Option<crate::profile::Profile>; 4] = [
                Some(profile("Host", &[])),
                Some(profile("Two", &[])),
                Some(profile("Three", &[])),
                None,
            ];
            for (k, kills) in [(1usize, 5u32), (2, 7)] {
                let i = w.local_unit(k).index as usize;
                w.sim.units[i].kills = kills;
            }
            let recorded =
                w.record_local_battles(&mut seats, &|k| order[k], &|_| Default::default());
            assert_eq!(recorded, vec![order[1], order[2]]);
            let kills = |s: usize| seats[s].as_ref().unwrap().career.kills;
            assert_eq!(kills(order[1]), 5, "local 1's kills on seat {}", order[1]);
            assert_eq!(kills(order[2]), 7, "local 2's kills on seat {}", order[2]);
            assert_eq!(seats[0].as_ref().unwrap().career.rounds, 0);
        }
    }

    /// Issue #194 B3: two rounds in a row with P2 on one profile add up.
    #[test]
    fn p2s_counters_add_up_across_two_rounds() {
        let mut seats: [Option<crate::profile::Profile>; 4] = Default::default();
        seats[1] = Some(profile("Han", &[]));
        let mut total = 0;
        for round in 1..=2 {
            let mut w = two_locals();
            install(&mut w, 1, seats[1].as_ref().unwrap());
            spawn_both(&mut w);
            kill_one(&mut w, 1);
            let class = class_of(&w, 1);
            let before = seats[1]
                .as_ref()
                .unwrap()
                .career
                .class_kills
                .get(&class)
                .copied();
            w.flush_local_careers(&mut seats, &|k| k);
            let kills = w.sim.units[w.local_unit(1).index as usize].kills;
            total += kills;
            w.record_local_battles(&mut seats, &|k| k, &|_| Default::default());
            let p2 = seats[1].as_ref().unwrap();
            assert_eq!(p2.career.rounds, round);
            assert_eq!(p2.career.kills, total);
            assert!(p2.career.class_kills[&class] > before.unwrap_or(0));
        }
    }

    /// Issue #194 B3: P2 earns a medal, the pad drops and comes back: the medal, the progress
    /// toward the others and the active bonus are still the slot's (the resume gives them back;
    /// nothing installs the profile again), and they reach P2's profile.
    #[test]
    fn p2s_medal_progress_and_bonus_survive_a_lost_pad() {
        let mut w = two_locals();
        let mut seats: [Option<crate::profile::Profile>; 4] = Default::default();
        let mut han = profile("Han", &[]);
        han.career.medals = [4; 9];
        seats[1] = Some(han);
        install(&mut w, 1, seats[1].as_ref().unwrap());
        spawn_both(&mut w);
        let unit = w.local_unit(1);
        let slot = usize::from(w.local_slot_of(1));
        // 12 points: medal 6 earned (its fifth), 7 and 8 part of the way.
        w.sim
            .score_medal_points(unit, core::sim::medal_threshold(6, 4));
        assert_eq!(w.sim.slot_medal_counts(slot).unwrap()[6], 5);
        let bits = w.sim.award_bits(&w.sim.units[unit.index as usize]);
        assert_ne!(bits & (1 << 6), 0, "the medal's bonus is active");
        let kept = w.sim.keep_slot(slot);
        let away = w.suspend_local_player(1).expect("P2 was playing");
        let back = w.resume_local_player(1, away).expect("the slot is free");
        assert_eq!(
            w.sim.keep_slot(slot),
            kept.following(back),
            "medal, progress, earned bits and activations as they were"
        );
        assert!(w.sim.slot_career_award_bonuses(slot));
        let flushed = w.flush_local_careers(&mut seats, &|k| k);
        assert_eq!(flushed, vec![1]);
        assert_eq!(seats[1].as_ref().unwrap().career.medals[6], 5);
    }

    /// Issue #194 B3 review: a P2 whose pad is gone at the round's end is recorded from what
    /// they banked at the drop, never from the unit index a bot may now hold; one who came back
    /// keeps the kills and deaths from before the drop.
    #[test]
    fn a_lost_pad_keeps_p2s_round_counters_and_side() {
        for comes_back in [false, true] {
            let mut w = two_locals();
            let mut seats: [Option<crate::profile::Profile>; 4] = Default::default();
            seats[1] = Some(profile("Han", &[]));
            install(&mut w, 1, seats[1].as_ref().unwrap());
            spawn_both(&mut w);
            kill_one(&mut w, 1);
            let old = w.local_unit(1);
            let (kills, deaths, team) = {
                let u = &mut w.sim.units[old.index as usize];
                u.deaths = 2;
                (u.kills, u.deaths, u.team)
            };
            assert!(kills >= 1);
            let away = w.suspend_local_player(1).expect("P2 was playing");
            // Whoever holds the old index now scores there; none of it is P2's.
            w.sim.units[old.index as usize].kills += 50;
            w.sim.units[old.index as usize].deaths += 50;
            w.sim.units[old.index as usize].team = 1 - team;
            let mut extra = 0;
            if comes_back {
                w.resume_local_player(1, away).expect("the slot is free");
                w.sim.open_spawn_screens(1, false);
                tap(&mut w, 1, buttons::SPAWN_ACCEPT);
                tap(&mut w, 1, buttons::SPAWN_ACCEPT);
                kill_one(&mut w, 1);
                extra = w.sim.units[w.local_unit(1).index as usize].kills;
                assert!(extra >= 1);
            }
            w.sim.outcome = Some(team);
            let recorded = w.record_local_battles(&mut seats, &|k| k, &|_| Default::default());
            assert_eq!(recorded, vec![1]);
            let p2 = &seats[1].as_ref().unwrap().career;
            assert_eq!(p2.kills, kills + extra, "comes back: {comes_back}");
            assert_eq!(p2.deaths, deaths, "comes back: {comes_back}");
            assert_eq!(p2.wins, 1, "P2's own side won");
        }
    }

    /// Issue #194 B3 review: P2's class-gate points survive a lost pad even when the rejoin puts
    /// them on another unit index, whose round row holds someone else's points.
    #[test]
    fn p2s_spawn_points_survive_a_rejoin_on_another_index() {
        let mut w = two_locals();
        install(&mut w, 1, &profile("Han", &[]));
        spawn_both(&mut w);
        let slot = usize::from(w.local_slot_of(1));
        let mut round = crate::round_stats::RoundStats::default();
        let row = |r: &mut crate::round_stats::RoundStats, i: u32| {
            let i = i as usize;
            if r.units.len() <= i {
                r.units.resize(i + 1, Default::default());
            }
            i
        };
        let old = w.local_unit(1).index;
        let i = row(&mut round, old);
        round.units[i].points = 3; // a bot's, before P2 took the index
        crate::netplay::gate_locals(&mut w, Some(&round));
        round.units[i].points = 3 + 9;
        crate::netplay::gate_locals(&mut w, Some(&round));
        assert_eq!(w.sim.spawn_points[slot], 9);
        let away = w.suspend_local_player(1).expect("P2 was playing");
        // Keep P2's old index busy, so the rejoin takes another.
        let busy = &mut w.sim.units[old as usize];
        busy.alive = true;
        busy.gen += 1;
        w.resume_local_player(1, away).expect("the slot is free");
        let new = w.local_unit(1).index;
        assert_ne!(new, old, "the rejoin is on another index");
        let j = row(&mut round, new);
        round.units[j].points = 40; // the index's previous holder's
        crate::netplay::gate_locals(&mut w, Some(&round));
        assert_eq!(w.sim.spawn_points[slot], 9, "unchanged by the rejoin");
        round.units[j].points = 42;
        crate::netplay::gate_locals(&mut w, Some(&round));
        assert_eq!(w.sim.spawn_points[slot], 11, "and counting on");
    }

    /// Integration finding (issue #194): P1's lost pad carries their class-gate points and round
    /// points as P2's does. The gate reads P1's feed row (`raw`), whose index may change on the
    /// rejoin; the points at the drop stay, and only what the new unit scores is added.
    #[test]
    fn p1s_points_survive_a_rejoin_on_another_index() {
        let mut w = two_locals();
        spawn_both(&mut w);
        assert_eq!(w.first_gate_points(5), 5, "never dropped: the feed's row");
        assert_eq!(w.carried_round_points(0), None);
        crate::unlocks::set_player_gate(&mut w, 9, &Default::default());
        let old = w.local_unit(0).index;
        let away = w.suspend_local_player(0).expect("P1 was playing");
        assert_eq!(w.first_gate_points(77), 9, "away: the points at the drop");
        assert_eq!(w.carried_round_points(0), Some(9));
        let busy = &mut w.sim.units[old as usize];
        busy.alive = true;
        busy.gen += 1;
        w.resume_local_player(0, away).expect("the slot is free");
        assert_ne!(w.local_unit(0).index, old, "the rejoin is on another index");
        // The new index's row holds its previous holder's 40.
        let p = w.first_gate_points(40);
        assert_eq!(p, 9, "unchanged by the rejoin");
        crate::unlocks::set_player_gate(&mut w, p, &Default::default());
        let p = w.first_gate_points(42);
        assert_eq!(p, 11, "and counting on");
        crate::unlocks::set_player_gate(&mut w, p, &Default::default());
        assert_eq!(
            w.carried_round_points(0),
            Some(11),
            "the round's record reads the gate"
        );
    }

    /// Issue #194 B3 review: a class kill unlock reaches P2's class costs in the same round, as
    /// the first player's does; a local with no seat profile drops their class kills.
    #[test]
    fn p2s_class_costs_follow_their_flushed_kills() {
        let mut w = two_locals();
        let officer = core::unit::UnitClass {
            consts: core::soldier::SoldierConstants::default(),
            max_health: 300.0,
            health_type: core::weapon::HealthType::Person,
            weapon_class: 0,
            total_clips: 6,
            weapon_slot: 0,
            slots: std::sync::Arc::new([]),
            jet: false,
            capture_posts: true,
            drops: core::sim::DropTable::default(),
            chunk: None,
            min: 0,
            max: None,
            unlock_points: 8,
            droideka: None,
        };
        for t in 0..2 {
            w.teams[t].classes = vec![ClassVisual {
                name: "all_inf_officer".into(),
                ..ClassVisual::default()
            }];
            w.sim.classes[t] = vec![officer.clone()];
        }
        let mut seats: [Option<crate::profile::Profile>; 4] = Default::default();
        seats[1] = Some(profile("Han", &[]));
        install(&mut w, 1, seats[1].as_ref().unwrap());
        crate::netplay::gate_locals(&mut w, None);
        assert_eq!(w.sim.spawn_cost(1, 0, 0), 8);
        w.with_local(1, |w| {
            w.pending_class_kills.insert("all_inf_officer".into(), 1000);
        });
        assert_eq!(w.flush_local_careers(&mut seats, &|k| k), vec![1]);
        crate::netplay::gate_locals(&mut w, None);
        assert_eq!(w.sim.spawn_cost(1, 0, 0), 0, "unlocked mid-round");
        // No seat profile: nothing to keep them in, so they do not pile up.
        let mut none: [Option<crate::profile::Profile>; 4] = Default::default();
        w.with_local(1, |w| {
            w.pending_class_kills.insert("all_inf_officer".into(), 3);
        });
        assert!(w.flush_local_careers(&mut none, &|k| k).is_empty());
        assert!(w.with_local(1, |w| w.pending_class_kills.is_empty()));
    }

    /// The saved profile entry of a scripted one-player round (`SPLITSCREEN.md` 18.2 "Save
    /// identity"): a kill, a medal, class kills flushed as the window does each frame, the round
    /// recorded. The new per-local calls do nothing with one local player, so the bytes are the
    /// ones the same script saved before B3 (`SAVE_IDENTITY_PIN`, made on `fddea3b0b` with the two
    /// per-local calls left out).
    #[test]
    fn a_one_player_rounds_saved_profile_is_unchanged() {
        const SAVE_IDENTITY_PIN: u64 = 3_505_798_455_005_844_255;
        let mut w = WalkSim::new(1, Vec3::ZERO, None);
        let mut p = profile("Solo", &[("trooper", 2)]);
        p.career.medals = [3; 9];
        w.sim.install_medal_career(w.player, p.career.medals);
        let target = w
            .sim
            .spawn(&core::unit::UnitSpec::trooper(
                1,
                Vec3::new(0.4, 0.0, -20.0),
            ))
            .unwrap();
        let fire = PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        };
        let mut none: [Option<crate::profile::Profile>; 4] = Default::default();
        for t in 0..120 {
            w.step_tick(&[fire]);
            if t % 30 == 29 {
                w.flush_career_progress(&mut p);
                w.flush_local_careers(&mut none, &|k| k);
            }
        }
        assert!(w.sim.unit(target).is_none(), "the target fell");
        w.sim.score_medal_points(w.player, 12);
        w.flush_career_progress(&mut p);
        let stats = crate::profile::BattleStats {
            points: 10,
            shots: 7,
            hits: 5,
            captures: 1,
        };
        w.record_battle(&mut p, stats);
        w.record_local_battles(&mut none, &|k| k, &|_| Default::default());
        assert!(none.iter().all(Option::is_none));
        let text = ron::ser::to_string_pretty(&p, ron::ser::PrettyConfig::default()).unwrap();
        let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        });
        assert!(p.career.class_kills["trooper"] > 2 && p.career.rounds == 1);
        assert_eq!(hash, SAVE_IDENTITY_PIN, "saved profile:\n{text}");
    }

    struct NoProps;
    impl core::weapon::PropSource for NoProps {
        fn prop(&self, _: &str) -> Option<&str> {
            None
        }
        fn prop_hash(&self, _: u32) -> Option<&str> {
            None
        }
    }

    /// Local player `k` flies a guided rocket 30 m above their soldier (the shape of
    /// `walk::tests::a_guided_rocket_flies_under_its_chase_camera_and_backs_off_when_spent`).
    fn fly_rocket(w: &mut WalkSim, k: usize, id: u64) {
        let mut class = core::guided::GuidedClass::from_props(&NoProps);
        class.flight.min_speed = 30.0;
        class.flight.mid_speed = 30.0;
        class.flight.max_speed = 30.0;
        class.flight.energy_max = 100.0;
        class.view.track_center = Some([0.0, 1.0, -3.0]);
        class.view.track_offset = Some([0.0, 2.0, 6.0]);
        let flying = core::guided::GuidedRocket::flying(&class);
        let wc = w.sim.add_weapon_class(core::weapon::WeaponClass {
            guided: Some(Box::new(core::guided::GuidedWeapon {
                class,
                spawn_offset: Vec3::ZERO,
                requires_award: false,
            })),
            ..Default::default()
        });
        let owner = w.local_unit(k);
        let u = w.sim.unit_mut(owner).unwrap();
        u.weapon_class = wc;
        let (pos, team) = (u.soldier.pos + Vec3::new(0.0, 30.0, 0.0), u.team);
        let dir = Vec3::NEG_Z;
        w.sim.guided.push(core::guided::GuidedRocket {
            id,
            owner,
            team,
            weapon_class: wc,
            seat: None,
            pos,
            vel: dir * 30.0,
            rot: core::guided::GuidedRocket::body(dir),
            flyer: flying,
            health: 1.0,
            age: 0.0,
            prev_buttons: 0,
            pending_end: None,
            spent: None,
            blind: 0.0,
        });
    }

    /// Local player `k` flies a remote droid 2 m above their soldier.
    fn fly_droid(w: &mut WalkSim, k: usize, id: u64) {
        let droid = core::remote::DroidClass::from_props(&NoProps, false);
        let remote = core::remote::RemoteWeapon::new(
            &NoProps,
            droid,
            core::weapon::WeaponClass::bf1_rifle(),
        );
        let mut class = core::weapon::WeaponClass::bf1_rifle();
        class.remote = Some(Box::new(remote.clone()));
        let weapon = w.sim.add_weapon_class(class);
        let owner = w.local_unit(k);
        let u = w.sim.unit(owner).unwrap();
        let (pos, team) = (u.soldier.pos + Vec3::new(0.0, 2.0, 0.0), u.team);
        let mut d =
            core::remote::Droid::new(id, owner, team, weapon, &remote, pos, Vec3::ZERO, 0.0, true);
        d.controlled = true;
        w.sim.droids.push(d);
    }

    /// Issue #194 B21: P2's pad drops while they guide a rocket or fly a remote droid. The rocket
    /// or droid is let go on the next step and P2's held guided view is cleared; P1's own rocket
    /// flies on under P1's guided view, untouched.
    #[test]
    fn a_lost_pad_lets_go_of_p2s_rocket_or_droid_only() {
        for droid in [false, true] {
            let mut w = two_locals();
            spawn_both(&mut w);
            fly_rocket(&mut w, 0, 1);
            if droid {
                fly_droid(&mut w, 1, 2);
            } else {
                fly_rocket(&mut w, 1, 2);
            }
            let pads = [PadState::default(); 2];
            for _ in 0..3 {
                w.step_tick(&pads);
            }
            let p1 = w.local_unit(0);
            let p2 = w.local_unit(1);
            assert_eq!(
                w.cam_guided.map(|c| c.rocket),
                Some(1),
                "P1 guides rocket 1"
            );
            if droid {
                assert!(w.sim.controlled_droid(p2).is_some(), "P2 flies the droid");
            } else {
                assert_eq!(
                    w.with_local(1, |w| w.cam_guided.map(|c| c.rocket)),
                    Some(2),
                    "P2 guides rocket 2"
                );
            }
            let p1_view = w.cam_guided;
            let _away = w.suspend_local_player(1).expect("P2 was playing");
            assert!(
                w.with_local(1, |w| w.cam_guided.is_none()),
                "P2's guided view goes with the pad"
            );
            assert_eq!(w.cam_guided, p1_view, "P1's guided view is untouched");
            w.step_tick(&pads);
            if droid {
                assert!(w.sim.controlled_droid(p2).is_none(), "P2's droid let go");
            } else {
                assert!(w.sim.controlled_guided(p2).is_none(), "P2's rocket let go");
            }
            assert!(w.with_local(1, |w| w.cam_guided.is_none()));
            assert_eq!(
                w.sim.controlled_guided(p1).map(|r| r.id),
                Some(1),
                "P1 still flies their rocket"
            );
            assert_eq!(w.cam_guided.map(|c| c.rocket), Some(1));
        }
    }

    /// Issue #194 B12: the music and the voice follow the first local player who is not away.
    /// One local player: always the walk's own player.
    #[test]
    fn first_awake_is_p2_while_p1_is_away() {
        let mut one = crate::walk::posts_walk();
        crate::walk::spawn_locals(&mut one);
        assert_eq!(
            (one.first_awake(), one.first_awake_unit()),
            (Some(0), Some(one.player))
        );
        let mut w = two_locals();
        spawn_both(&mut w);
        assert_eq!(
            (w.first_awake(), w.first_awake_unit()),
            (Some(0), Some(w.player))
        );
        let away = w.suspend_local_player(0).expect("P1 was playing");
        assert_eq!(w.first_awake(), Some(1));
        assert_eq!(w.first_awake_unit(), Some(w.local_unit(1)));
        w.resume_local_player(0, away).expect("P1 comes back");
        assert_eq!(w.first_awake(), Some(0));
        // P2 away instead: P1 leads.
        let _away2 = w.suspend_local_player(1).expect("P2 was playing");
        assert_eq!(w.first_awake(), Some(0));
        // Everyone away: nobody's handle is read.
        let _away1 = w.suspend_local_player(0).expect("P1 was playing");
        assert_eq!((w.first_awake(), w.first_awake_unit()), (None, None));
    }
}
