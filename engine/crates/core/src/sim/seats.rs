//! WGMUL reconciles explicit BF1 claim budgets after think write-back, before entry.
//! WGCAL keeps non-ground entry calls here; ground calls follow wait start in step.
//! WGSEAT ground call eligibility is independent of gun seats; admission is unchanged.
//! GBOARD/WGORD store BF1 answerers' next-think targets over retained orders.
//! Entry, exit and follower write-back discard ground-claim maintenance (CWAIT).
//! Enter, drive, exit (card VSEAT, `analysis/re-vehicles.md` §5 [O],
//! `analysis/re-hover.md` §5.2 [O]): the use input's pick and seat choice,
//! the exit placement, and the driver's pad written into the hover's input.
//!
//! - **The use input** (B1 FUN_0020a4f0 / B2 FUN_00249870): on the ACTION
//!   edge a mounted soldier exits; an unmounted one gathers the vehicles
//!   whose sphere meets its own grown by 0.5 (FUN_00161c00, B1:119520),
//!   scores each with FUN_00241b98(R, 1.0, 5.0, eye, aim, centre)
//!   (B1:158844-158895): t = (centre − eye)·aim, accepted for 0 ≤ t ≤ 5 + R
//!   and perp² ≤ (R + t)², score t + 16·perp², lowest wins.
//! - **CanEnter** (B1 FUN_0020b1b0): a neutral vehicle takes anyone, else the
//!   entrant's team must be its live team (a player's friendly relation,
//!   FUN_00221890 [H] — one team per side here); the seat is the first in
//!   ring order whose PilotType boards a soldier and that is empty or held
//!   by an AI — a player takes the AI's seat and the AI is exited and placed
//!   (B1:120133). Enter and exit are instant (§5 [D]).
//! - **The exit** (B1 FUN_0020b918, decoded from its stack block at
//!   B1:120490-120540): 9 candidates 1 m up in the vehicle's yaw frame at
//!   ρ = R + (0.72 − 0.15)·1.2 = R + 0.684 — behind (0, −ρ), left (−ρ, 0), right (ρ, 0), the four
//!   diagonals at 0.7071·ρ (back-left, back-right, front-right, front-left),
//!   front (0, ρ), then the centre, whose path test is skipped. Each needs a
//!   clear line from the vehicle's centre, then a 5 m ray up (y += 5t − 0.1)
//!   and a ray down of `SeatSetup::exit_drop` that hits (y −= drop·t); the
//!   first that passes places the soldier standing, still, facing the
//!   vehicle's heading; when none does, at the frame's origin, the vehicle's
//!   position (B1:120583-120642). The PS2's frame is the body's own up row
//!   and a second row made perpendicular to it (FUN_001ac6f8, 0x20bba4-
//!   0x20bc28 — which entity's row is forward is unresolved [H]); the port's
//!   is world up and the body's heading, PROVISIONAL. When the exit is a
//!   seat change (its second argument), an AI in a type-2 seat is moved into the one
//!   left (B1:120689-120718) — VAI's. BF2's best-height pick among the
//!   candidates is [H] and not modelled. A building turret (card VTURB)
//!   skips the path test: its origin sits inside its own collision mesh,
//!   which every path from it hits within 0.2 m (BF1 HOT1's hoth turret,
//!   `turret_ride`) and the soldier would be placed there, trapped. The PS2
//!   starts the test at the seat entity's collision-sphere centre (FUN_001645a8,
//!   B1:120472-120483) against mask 0x4c and evidently gets out; how its
//!   building primitives answer a ray from inside is [H], so the skip is
//!   PROVISIONAL.
//! - **Card VMESH (ours):** a candidate whose point is shut inside a map
//!   body (`CollisionWorld::enclosing`, 1 m over the feet) does not pass:
//!   with a vehicle's centre inside a closed prop (BF1 Endor's cp2 boulder,
//!   the hollow trunks) every candidate inside it passed the path test and
//!   131 of 131 riders' stays inside began at an exit. When none passes and
//!   one was shut in, eight points just outside that body's box are tried,
//!   the side nearest the vehicle first, before the centre.
//! - **The driver's pad** (B2 FUN_002b0548, re-hover §5.2): left stick =
//!   strafe / throttle, right stick = turn / pitch [H: which physical stick];
//!   each clamped to ±1; the full-stick timer E+0x360 tests the RAW right
//!   stick (turn² + pitch² ≥ 0.9); then a nonzero move axis under 0.3 becomes
//!   ±0.3 and each pair is scaled into the unit circle. SPRINT is the boost
//!   button (E+0x2bc bit 0) and JUMP the bike's jump (E+0x2b4 bit 0) — which
//!   PS2 buttons those bits are is [H]; the port's pick is a feel call
//!   (`FEEL_NOTES.md`). With no human at the pilot seat the input is zero
//!   and the timer 0 (FUN_002f4a50).
//!
//! WAIT4 preserves the successful entry clock through AI local write-back.
//! SQVEH followers walk to their leader's vehicle and take a free boardable
//! seat in step 1c (`analysis/re-squad-orders.md` §4.3). This directed entry
//! bypasses autonomous seat search and its cooldown; the port's boarding
//! radius and per-tick service remain PROVISIONAL.
//!
//! PROVISIONAL (ours): the gather's sphere test, the soldier's sphere (the
//! capsule's centre, radius max(r, h/2)), the eye (the muzzle) and the
//! vehicle's sphere centred on its origin; the rays' surfaces (the terrain
//! sheet and the SOLDIER-layer bodies); a mounted soldier rides at the
//! vehicle's origin and neither moves, collides, aims nor fires. Only a seat's
//! `SeatSetup::body_active` gives him a body to be hit on, at the seat's frame.

use super::input::{buttons, PadState};
use super::state::Sim;
use crate::ai::{route, SHUT_PROBE_UP};
use crate::collision::{HullFilter, Layers};
use crate::fmath::FloatExt;
use crate::soldier::Stance;
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::flyer::FlightPhase;
use crate::vehicle::hover::{ramp_stick_timer, HoverInput};
use crate::vehicle::{SeatKind, VehicleHandle, VehicleKind};
use glam::Vec3;

/// FUN_00241b98's reach past the target's radius (0x40a00000, B1:119544).
pub const ENTER_REACH: f32 = 5.0;
/// The gather sphere's growth over the soldier's own (B1:119520).
pub const ENTER_GROW: f32 = 0.5;
/// FUN_0020b918's ring beyond the vehicle's radius, (DAT_00533090 − 0.15) ·
/// 1.2 (B1 0x20ba14-0x20ba6c; the decomp prints the folded `0.6840001`):
/// DAT_00533090 is f32 0.72 in .data with no text writer (`re_check`), the
/// first of a {0.72, 0.45, 0.80} table — a soldier radius per stance [H].
pub const EXIT_RING: f32 = (0.72 - 0.15) * 1.2;
/// The exit's ray up (0x40a00000) and its 0.1 m clearance.
pub const EXIT_RISE: f32 = 5.0;
/// The candidates' height in the yaw frame (1.0f at every `+4` slot).
pub const EXIT_LIFT: f32 = 1.0;
/// Card VMESH (ours): how many points round a body's box the exit tries
/// when every one of §5's points is shut inside the body.
pub const EXIT_OUT_DIRS: usize = 8;
/// §5.2: a nonzero move axis smaller than this is raised to it.
pub const MOVE_FLOOR: f32 = 0.3;

/// The diagonals' factor: f32 0x3f350481 = 0.7071 (lui+ori at B1 0x20bae8),
/// the PS2's own rounding of 1/√2, kept.
#[allow(clippy::approx_constant)]
const DIAG: f32 = 0.7071;

/// The 9 exit candidates, (x right, z forward) in units of ρ (B1:120490-120540).
const EXIT_CANDIDATES: [(f32, f32); 9] = [
    (0.0, -1.0),
    (-1.0, 0.0),
    (1.0, 0.0),
    (-DIAG, -DIAG),
    (DIAG, -DIAG),
    (DIAG, DIAG),
    (-DIAG, DIAG),
    (0.0, 1.0),
    (0.0, 0.0),
];

/// §5.2's shaping of the four driver axes into the hover's input. The timer
/// flag is the raw look stick at full deflection.
pub fn driver_input(pad: &PadState) -> (HoverInput, bool) {
    let strafe = PadState::axis(pad.lx);
    let throttle = PadState::axis(pad.ly);
    let turn = PadState::axis(pad.rx);
    let pitch = PadState::axis(pad.ry);
    let full = turn * turn + pitch * pitch >= 0.9;
    let floor = |a: f32| {
        if a != 0.0 && a.abs() < MOVE_FLOOR {
            MOVE_FLOOR.copysign(a)
        } else {
            a
        }
    };
    let unit = |a: f32, b: f32| {
        let l2 = a * a + b * b;
        if l2 > 1.0 {
            let k = 1.0 / l2.sqrt();
            (a * k, b * k)
        } else {
            (a, b)
        }
    };
    let (strafe, throttle) = unit(floor(strafe), floor(throttle));
    let (turn, pitch) = unit(turn, pitch);
    (
        HoverInput {
            throttle,
            strafe,
            turn,
            pitch,
            human: true,
            boost: pad.pressed(buttons::SPRINT),
            jump: pad.pressed(buttons::JUMP),
        },
        full,
    )
}

/// The vehicle's heading frame: (right, forward), horizontal and unit. The
/// PS2 builds it from the body's forward row and world up (FUN_001ac6f8).
fn yaw_frame(rot: glam::Quat) -> (Vec3, Vec3) {
    let f = rot * Vec3::Z;
    let f = Vec3::new(f.x, 0.0, f.z).try_normalize().unwrap_or(Vec3::Z);
    // The model's +x row with +y up and +z forward (`Vehicle::rot`).
    (Vec3::new(f.z, 0.0, -f.x), f)
}

/// The soldier yaw (render convention: yaw 0 looks down −Z) facing `f`.
fn yaw_facing(f: Vec3) -> f32 {
    crate::fmath::atan2(-f.x, -f.z)
}

impl Sim {
    /// Step 1c: every ACTION edge, in unit order — a mounted soldier exits,
    /// an unmounted one enters what it looks at. Only a pad presses ACTION;
    /// the AI's boarding (VAI) goes through [`Sim::enter_vehicle`] too.
    pub(super) fn use_pass(&mut self, unit_pads: &[PadState]) {
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if !u.alive {
                continue;
            }
            let Some(p) = unit_pads.get(i) else { continue };
            let edge =
                p.pressed(buttons::ACTION) && u.soldier.prev_buttons() & buttons::ACTION == 0;
            if !edge {
                continue;
            }
            let h = self.handle(i);
            if self.units[i].mount.is_some() {
                if !self.rider_held_in_flight(h) {
                    self.exit_vehicle(h);
                }
            } else if let Some((vh, seat)) = self.enter_target(h) {
                self.enter_vehicle(h, vh, seat);
            }
        }
    }

    /// Step 1c, for the bots (card VAI): the request each think left in
    /// `AiState::ride` — a seated bot gets out; one on foot enters its
    /// claim's pilot seat when the vehicle is still within
    /// `ai::in_reach` and CanEnter still gives it (the PS2's AI enters
    /// through the same Enter slot, re-vehicles §5). A bot that changed
    /// place re-plans on its next think; a failed entry blocks new claims
    /// for `ai::BOARD_BLOCK`.
    pub(crate) fn ai_use_pass(&mut self) {
        crate::ai::called_claim_pass(self);
        // VBAIL: collect the pilot's order before changing ANY seat, so a
        // gunner at a lower unit index exits on the same pass. Player seats
        // never receive an AI get-out order. Preserve the normal exit path.
        let mut bail = Vec::new();
        if crate::ai::VEHICLE_BAIL && !self.vehicles.is_empty() {
            for (vh, v) in self.alive_vehicles() {
                if !matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker)
                    || v.health.cur >= v.health.max * 0.2
                {
                    continue;
                }
                let Some(h) = v
                    .seats
                    .first()
                    .filter(|s| s.kind == SeatKind::Pilot)
                    .and_then(|s| s.occupant)
                else {
                    continue;
                };
                let Some(u) = self.unit(h).filter(|u| u.controller == Controller::Ai) else {
                    continue;
                };
                if u.ai.ride.is_some_and(|r| r.use_req) && crate::ai::vehicle_bail_allowed(self, vh)
                {
                    bail.push(vh);
                }
            }
            // Count each low-health driver ride once, including failing draws.
            for i in 0..self.units.len() {
                let u = &self.units[i];
                let exposed = u.alive
                    && u.controller == Controller::Ai
                    && u.mount.is_some_and(|m| {
                        m.seat == 0
                            && self.vehicle(m.vehicle).is_some_and(|v| {
                                matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker)
                                    && v.health.cur < v.health.max * 0.2
                            })
                    });
                if exposed
                    && u.ai
                        .ride
                        .is_some_and(|r| r.bail_draw.is_some() && !r.bail_seen)
                {
                    self.units[i].ai.ride.as_mut().unwrap().bail_seen = true;
                    self.nav_stats.ai_bail_exposed += 1;
                }
            }
        }
        for vh in &bail {
            self.nav_stats.ai_bail_drivers += 1;
            for u in &mut self.units {
                if u.alive
                    && u.controller == Controller::Ai
                    && u.mount.is_some_and(|m| m.vehicle == *vh)
                {
                    let r = u.ai.ride.get_or_insert_default();
                    r.use_req = true;
                    r.board = None;
                    r.called_board = None;
                    r.player_board = None;
                    r.maintenance = None;
                    self.nav_stats.ai_bail_occupants += 1;
                }
            }
        }
        let mut pilots = Vec::new();
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if !u.alive || u.controller != Controller::Ai || u.droideka.is_some() {
                continue;
            }
            // SQVEH: a follower boards the vehicle its leader occupies,
            // independently of autonomous claim/search eligibility. Re-read
            // CanEnter in unit order so two followers never take one seat.
            // PROVISIONAL: served every tick on reaching the port's boarding
            // radius, rather than the PS2 board agent's queued think.
            let follow_target = crate::squad::follow_vehicle(self, u).filter(|_| {
                !u.ai
                    .ride
                    .is_some_and(|r| r.ordered_call || r.player_board.is_some())
            });
            let following = follow_target.filter(|&vh| crate::ai::in_reach(self, u, vh));
            if follow_target.is_some() && following.is_none() {
                continue;
            }
            let mut r = if let Some(vh) = following {
                crate::ai::RideState {
                    board: Some(vh),
                    maintenance: None,
                    use_req: true,
                    ..u.ai.ride.unwrap_or_default()
                }
            } else if let Some(r) = u.ai.ride.filter(|r| r.use_req) {
                r
            } else {
                continue;
            };
            r.use_req = false;
            let h = self.handle(i);
            let moved = if let Some(m) = self.units[i].mount {
                // Card VMESH: a driver getting out of a vehicle its stuck
                // window has given up on (`DRIVE_STUCK_BAIL`) marks it.
                let stuck = self.units[i]
                    .ai
                    .route()
                    .is_some_and(|rt| rt.stuck_run >= crate::ai::DRIVE_STUCK_BAIL);
                if stuck {
                    if let Some(v) = self.vehicle_mut(m.vehicle) {
                        v.stuck_at = Some(v.pos);
                    }
                }
                self.exit_vehicle(h);
                // Unseating records endgame command departure on the unit.
                // Preserve it when this request's retained ride copy writes back.
                r.command_dismounted |= self.units[i]
                    .ai
                    .ride
                    .is_some_and(|ride| ride.command_dismounted);
                r.pickup = None;
                r.board = None;
                r.called_board = None;
                r.player_board = None;
                r.maintenance = None;
                r.bail_draw = None;
                r.bail_seen = false;
                self.nav_stats.ai_dismounts += 1;
                r.block_until = self.tick + crate::ai::BOARD_BLOCK;
                true
            } else if let Some(vh) = r.board.take() {
                r.ordered_call = false;
                r.maintenance = None;
                let near = crate::ai::in_reach(self, &self.units[i], vh);
                let seat = if following == Some(vh) {
                    self.enter_seat(h, vh)
                } else {
                    crate::ai::claimed_seat(
                        self,
                        h,
                        vh,
                        crate::ai::RideState {
                            board: Some(vh),
                            ..r
                        },
                    )
                }
                .filter(|_| near);
                let called = r.called_board.take().is_some();
                r.player_board = None;
                let entered = seat.is_some_and(|k| self.enter_vehicle(h, vh, k));
                if entered {
                    // Preserve seat_unit's clock, zero driver output and draw
                    // when writing this pass's local state back to the unit.
                    r.pickup = self.units[i].ai.ride.and_then(|r| r.pickup);
                    r.sticks = self.units[i].ai.ride.and_then(|r| r.sticks);
                    r.hover = self.units[i].ai.ride.and_then(|r| r.hover);
                    r.flyer = self.units[i].ai.ride.and_then(|r| r.flyer);
                    r.bail_draw = self.units[i].ai.ride.and_then(|r| r.bail_draw);
                    r.bail_seen = false;
                    if following.is_some() {
                        self.units[i].command = None;
                        self.units[i].ai.cmd_clear();
                    }
                    self.nav_stats.ai_boards += 1;
                    self.nav_stats.man_boards += u64::from(
                        self.vehicle(vh)
                            .is_some_and(|v| v.kind == VehicleKind::Turret),
                    );
                    self.nav_stats.ai_gunners += u64::from(seat != Some(0));
                    let walker = self
                        .vehicle(vh)
                        .is_some_and(|v| v.kind == VehicleKind::Walker);
                    self.nav_stats.ai_walker_rides += u64::from(walker && seat == Some(0));
                    if seat == Some(0) {
                        pilots.push((h, vh));
                    }
                } else {
                    r.call_blocked |= called;
                    let until = self.tick + crate::ai::BOARD_BLOCK;
                    r.block_until = if following.is_some() {
                        r.block_until.max(until)
                    } else {
                        until
                    };
                }
                entered
            } else {
                false
            };
            let ai = &mut self.units[i].ai;
            ai.ride = Some(r);
            if moved {
                route::clear(ai);
                ai.next_think = self.tick;
            }
        }
        for (h, vh) in pilots {
            // WGCAL: ground pilots call when their pickup wait starts in step,
            // rather than on entry. Retain other vehicles' entry behavior.
            if self
                .vehicle(vh)
                .is_some_and(|v| !matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker))
            {
                self.need_gunner_call(h, vh);
            }
        }
    }

    /// Card UTALK, BF1's need-gunner call (re-squad-orders.md 1, 5; a
    /// BF2-rule sim asks none): an AI
    /// pilot issues the vehicle order (type 4) with chance
    /// [`NEED_GUNNER_CHANCE`] (B1 `FUN_0024d020` 167039 -> `FUN_002613f8`):
    /// its line, and then every friendly AI soldier on foot within
    /// [`crate::squad::ORDER_REACH`] (3-D, no cap, no sight) answers "yes"
    /// ([`Call::Yes`]) - the order is delivered whether or not the issuer's
    /// line passes its sound gates (`FUN_00260f38` 181702-181793).
    /// Ground pilots attempt once at pickup-wait start and first routed departure
    /// (WGCAL/WGOUT, `re-driver-wait.md` §§1/2/4/8), after all AI write-backs. Other vehicle
    /// kinds retain entry timing and the existing free armed-gun gate. Ground
    /// calls do not test seats (WGSEAT, `re-driver-wait.md` §§4/7); receivers
    /// still pass ordinary boarding admission at their next think.
    /// The answers come on the issue tick, not at each receiver's
    /// next think. GBOARD/WGORD give ground-vehicle receivers goal 5
    /// at their next think (RE 4.4), retaining prior Hold/Follow (WGORD).
    ///
    /// [`NEED_GUNNER_CHANCE`]: super::state::NEED_GUNNER_CHANCE
    /// [`Call::Yes`]: super::state::Call::Yes
    pub(crate) fn need_gunner_call(&mut self, h: UnitHandle, vh: VehicleHandle) {
        use super::state::{Call, Event, NEED_GUNNER_CHANCE};
        // BF1 only: BF2's squad talk is its VOSound's (card UVO2).
        if self.ground_rules == crate::soldier::GroundRules::Bf2 {
            return;
        }
        let Some(v) = self.vehicle(vh) else { return };
        let ground = matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker);
        let guns = self
            .seat_setup
            .get(v.class as usize)
            .and_then(|s| s.as_ref())
            .map(|s| s.guns.as_slice())
            .unwrap_or_default();
        let free_gun =
            v.seats.iter().enumerate().skip(1).any(|(k, seat)| {
                seat.occupant.is_none() && guns.get(k).is_some_and(Option::is_some)
            });
        if !ground && !free_gun {
            return;
        }
        let draw = crate::ai::unit_unit(h.index, self.tick, 0, crate::ai::SALT_NEED_GUNNER);
        if draw >= NEED_GUNNER_CHANCE {
            return;
        }
        let Some(me) = self.unit(h) else { return };
        let (team, at) = (me.team, me.soldier.pos);
        self.events.push(Event::Call {
            unit: h,
            call: Call::NeedGunner,
        });
        let reach = crate::squad::ORDER_REACH;
        let yes: Vec<UnitHandle> = (0..self.units.len())
            .filter(|&j| {
                let o = &self.units[j];
                j != h.index as usize
                    && o.alive
                    && o.team == team
                    && o.controller == Controller::Ai
                    && o.mount.is_none()
                    && o.droideka.is_none()
                    && o.soldier.pos.distance_squared(at) <= reach * reach
            })
            .map(|j| self.handle(j))
            .collect();
        for unit in yes {
            let u = &mut self.units[unit.index as usize];
            // WGORD: every ground recipient receives the explicit goal;
            // Hold/Follow remains installed behind boarding.
            if ground {
                u.ai.pending_board = Some((unit, vh));
            }
            self.events.push(Event::Call {
                unit,
                call: Call::Yes,
            });
        }
    }

    /// The use input's pick (FUN_0020a4f0 + FUN_00241b98) and CanEnter's
    /// seat: `None` when nothing in reach can take this soldier.
    pub fn enter_target(&self, u: UnitHandle) -> Option<(VehicleHandle, u8)> {
        let unit = self.unit(u).filter(|x| x.mount.is_none())?;
        let cap = unit.capsule();
        let (centre, r_self) = (cap.center(), cap.radius.fmax(cap.height * 0.5));
        let (aim, _, _) = unit.aim_frame();
        let eye = unit.muzzle();
        let mut best: Option<(f32, VehicleHandle, u8)> = None;
        for (vh, v) in self.alive_vehicles() {
            if v.kind == VehicleKind::Flyer && !v.health.alive() {
                continue;
            }
            let Some(setup) = self
                .seat_setup
                .get(v.class as usize)
                .and_then(|s| s.as_ref())
            else {
                continue;
            };
            let r = setup.radius;
            let sphere = v.pos
                + v.rot
                    * self
                        .blast_setup
                        .get(v.class as usize)
                        .and_then(|b| b.as_ref())
                        .map_or(Vec3::ZERO, |b| b.sphere_centre);
            // FUN_00161c00's gather: the two spheres meet, the soldier's grown.
            let reach = r + r_self + ENTER_GROW;
            if sphere.distance_squared(centre) > reach * reach {
                continue;
            }
            let d = sphere - eye;
            let t = d.dot(aim);
            if !(0.0..=ENTER_REACH + r).contains(&t) {
                continue;
            }
            let perp2 = (d - aim * t).length_squared();
            if perp2 > (r + t) * (r + t) {
                continue;
            }
            let Some(seat) = self.enter_seat(u, vh) else {
                continue;
            };
            let score = t + 16.0 * perp2;
            if best.is_none_or(|(b, _, _)| score < b) {
                best = Some((score, vh, seat));
            }
        }
        best.map(|(_, vh, s)| (vh, s))
    }

    /// CanEnter (B1 FUN_0020b1b0): the seat `u` would take in `vh`.
    pub fn enter_seat(&self, u: UnitHandle, vh: VehicleHandle) -> Option<u8> {
        let unit = self.unit(u).filter(|unit| unit.droideka.is_none())?;
        let v = self.vehicle(vh)?;
        if v.team.is_some_and(|t| t != unit.team) || v.held_against(u, self.tick) {
            return None;
        }
        // BF2's CanEnter refuses a flyer in flight (B2:164851). BF1's has no
        // such test; its bots are held off by the claim's Landed gate.
        if v.kind == crate::vehicle::VehicleKind::Flyer
            && self.ground_rules == crate::soldier::GroundRules::Bf2
            && v.flyer.phase == crate::vehicle::flyer::FlightPhase::Flying
        {
            return None;
        }
        let setup = self.seat_setup.get(v.class as usize)?.as_ref()?;
        let player = unit.is_player;
        v.seats.iter().enumerate().find_map(|(i, s)| {
            if !setup.boardable.get(i).copied().unwrap_or(false) {
                return None;
            }
            let free = match s.occupant {
                None => true,
                Some(o) => player && self.unit(o).is_some_and(|x| !x.is_player),
            };
            free.then_some(i as u8)
        })
    }

    /// Put `u` into seat `seat` of `vh`, exiting (and placing) an AI that
    /// holds it. `false` when the seat stays taken or either side is gone.
    pub fn enter_vehicle(&mut self, u: UnitHandle, vh: VehicleHandle, seat: u8) -> bool {
        // Validate every refusal before moving the current rider outside.
        let Some(unit) = self
            .unit(u)
            .filter(|unit| unit.mount.is_none() && unit.droideka.is_none())
        else {
            return false;
        };
        let Some(v) = self.vehicle(vh) else {
            return false;
        };
        let Some(target) = v.seats.get(usize::from(seat)) else {
            return false;
        };
        if v.held_against(u, self.tick) {
            return false;
        }
        let held = target.occupant;
        if held.is_some_and(|occupant| {
            !unit.is_player || self.unit(occupant).is_none_or(|rider| rider.is_player)
        }) {
            return false;
        }
        if let Some(ai) = held {
            self.exit_vehicle(ai);
        }
        if !self.seat_unit(u, vh, seat) {
            return false;
        }
        let s = &mut self.units[u.index as usize].soldier;
        s.vel = Vec3::ZERO;
        true
    }

    /// A BF2 flyer in flight keeps its pilot and gunners aboard; passengers
    /// may jump out (re-space-assault §10). In space modes passengers are held
    /// too, a named deviation (no ground to land on);
    /// measure: a PS2 space transport passenger pressing ACTION in flight.
    fn rider_held_in_flight(&self, u: UnitHandle) -> bool {
        if self.ground_rules != crate::soldier::GroundRules::Bf2 {
            return false;
        }
        self.seat_of(u).is_some_and(|(h, seat)| {
            (!matches!(seat, SeatKind::Passenger(_)) || self.space_round())
                && self.vehicle(h).is_some_and(|v| {
                    v.kind == VehicleKind::Flyer
                        && v.flyer.phase != crate::vehicle::flyer::FlightPhase::Landed
                })
        })
    }

    /// Take `u` out of its seat and stand it at the first of §5's 9 points
    /// that passes (FUN_0020b918). A no-op for an unmounted unit.
    pub fn exit_vehicle(&mut self, u: UnitHandle) {
        let Some(m) = self.unit(u).and_then(|x| x.mount) else {
            return;
        };
        let ai = self.unit(u).is_some_and(|u| u.controller == Controller::Ai);
        let spot = self.vehicle(m.vehicle).map(|v| {
            let drop = self
                .seat_setup
                .get(v.class as usize)
                .and_then(|s| s.as_ref())
                .map_or(10.0, |s| s.exit_drop);
            let r = self
                .seat_setup
                .get(v.class as usize)
                .and_then(|s| s.as_ref())
                .map_or(0.0, |s| s.radius);
            let (right, fwd) = yaw_frame(v.rot);
            let paths = v.kind != VehicleKind::Turret;
            let past = HullFilter::except(Some(m.vehicle));
            let prefer_walkable = ai
                && v.kind == VehicleKind::Turret
                && self
                    .building_turrets
                    .iter()
                    .any(|t| t.handle == m.vehicle && t.prefer_walkable_exit);
            (
                self.exit_point(
                    v.pos,
                    right,
                    fwd,
                    r + EXIT_RING,
                    drop,
                    paths,
                    past,
                    ai && v.kind == VehicleKind::Walker,
                    prefer_walkable,
                ),
                fwd,
            )
        });
        self.unseat_unit(u);
        if let Some((pos, fwd)) = spot {
            let s = &mut self.units[u.index as usize].soldier;
            s.pos = pos;
            s.vel = Vec3::ZERO;
            s.yaw = yaw_facing(fwd);
            s.pitch = 0.0;
            s.stance = Stance::Stand;
            s.grounded = true;
            s.ground_n = Vec3::Y;
            s.air_time = 0.0;
        }
    }

    /// FUN_0020b918's candidate walk: the first point with a clear line from
    /// `centre` (tested only when `paths`), lifted under a 5 m ray and
    /// dropped onto the ground; `centre` itself when none passes. Card
    /// VMESH (ours, the PS2 has no such test): a point shut inside a map
    /// body (`CollisionWorld::enclosing`) does not pass, and when none does
    /// and one was shut in, [`EXIT_OUT_DIRS`] points just outside that
    /// body's box are tried before the centre. The rays
    /// see the hulls `hulls` admits — every one but the vehicle's own, whose
    /// centre is inside it (card VHULL; PROVISIONAL: the PS2's mask 0x4c
    /// against its own entity is untraced).
    #[allow(clippy::too_many_arguments)]
    fn exit_point(
        &self,
        centre: Vec3,
        right: Vec3,
        fwd: Vec3,
        rho: f32,
        drop: f32,
        paths: bool,
        hulls: HullFilter,
        ai_walker: bool,
        prefer_walkable: bool,
    ) -> Vec3 {
        let mut shut_in = None;
        let mut first_turret_spot = None;
        for (k, &(x, z)) in EXIT_CANDIDATES.iter().enumerate() {
            let p = centre + right * (x * rho) + fwd * (z * rho) + Vec3::Y * EXIT_LIFT;
            if paths && k != 8 && self.first_hit(centre, p, hulls).is_some() {
                continue;
            }
            let Some(p) = self.exit_drop(p, drop, hulls) else {
                continue;
            };
            // Card VMESH (ours): never a point shut inside a map body.
            match self.shut_in(p + Vec3::Y * SHUT_PROBE_UP) {
                Some(b) => shut_in = shut_in.or(Some(b)),
                None => {
                    if prefer_walkable && !self.turret_exit_reaches_floor(p) {
                        first_turret_spot.get_or_insert(p);
                        continue;
                    }
                    // OURS: an embedded walker centre can occlude every ring
                    // ray at its origin. If the centre fallback strands the
                    // bot, retry from the ring's raised height.
                    // measure: PS2 Hoth dismounts when the pivot is below the floor.
                    if ai_walker && k == 8 && p.y > centre.y && self.level_at(p).is_off() {
                        let raised = centre + Vec3::Y * EXIT_LIFT;
                        for &(x, z) in &EXIT_CANDIDATES[..k] {
                            let q = raised + right * (x * rho) + fwd * (z * rho);
                            if self.first_hit(raised, q, hulls).is_some() {
                                continue;
                            }
                            if let Some(q) = self.exit_drop(q, drop, hulls).filter(|q| {
                                self.level_at(*q) == super::Level::On
                                    && self.shut_in(*q + Vec3::Y * SHUT_PROBE_UP).is_none()
                                    && !self.death_regions.iter().any(|r| r.contains(*q))
                            }) {
                                return q;
                            }
                        }
                    }
                    return p;
                }
            }
        }
        if let Some(p) = first_turret_spot {
            return p;
        }
        // Card VMESH (ours): every candidate was inside a body - the
        // vehicle's centre is in it (a bike through Endor's cp2 boulder, a
        // walker's centre in its edge). Out past the body's box instead,
        // the nearest side first, before the centre.
        let shut_in = shut_in.or_else(|| self.shut_in(centre));
        if let Some(body) = shut_in.and_then(|b| self.collision.bodies().get(b)) {
            let (lo, hi) = (body.aabb.min, body.aabb.max);
            let mid = Vec3::new(lo.x + hi.x, 0.0, lo.z + hi.z) * 0.5;
            let reach = 0.5 * Vec3::new(hi.x - lo.x, 0.0, hi.z - lo.z).length() + EXIT_RING;
            let first = Vec3::new(centre.x - mid.x, 0.0, centre.z - mid.z)
                .try_normalize()
                .unwrap_or(fwd);
            for k in 0..EXIT_OUT_DIRS {
                let a = k as f32 * std::f32::consts::TAU / EXIT_OUT_DIRS as f32;
                let (sn, cs) = crate::fmath::sin_cos(a);
                let d = Vec3::new(
                    first.x * cs - first.z * sn,
                    0.0,
                    first.x * sn + first.z * cs,
                );
                let p = Vec3::new(mid.x, centre.y + EXIT_LIFT, mid.z) + d * reach;
                if let Some(p) = self
                    .exit_drop(p, drop, hulls)
                    .filter(|p| self.shut_in(*p + Vec3::Y * SHUT_PROBE_UP).is_none())
                {
                    return p;
                }
            }
        }
        centre
    }

    /// OURS: a turret can drop a bot onto an isolated rim at the same
    /// height. Try the other normal placements for a walk back to the graph.
    /// measure: Coruscant's upper plaza turret exit, issue #108.
    fn turret_exit_reaches_floor(&self, pos: Vec3) -> bool {
        let Some(g) = self.nav.as_ref() else {
            return true;
        };
        let (lift, radius) = super::soldier_blast_sphere(Stance::Stand);
        if !self.soldier_clear(pos)
            || self.death_regions.iter().any(|r| r.contains(pos))
            || self
                .damage_regions
                .iter()
                .any(|r| r.region.touches_sphere(pos + Vec3::Y * lift, radius))
        {
            return false;
        }
        let near = self.nearest_floor_hubs::<{ crate::ai::LEVEL_HUB_TRIES }>(g, pos, |i| {
            self.nav_floor(i as u16)
        });
        near.into_iter().any(|(_, h)| {
            let (Some(hub), Some(y)) = (g.hub(h), self.nav_floor(h)) else {
                return false;
            };
            let to = Vec3::new(hub.pos.x, y, hub.pos.z);
            pos.distance_squared(to) <= crate::fmath::square(crate::ai::RECYCLE_ROAM)
                && self.floor_line(pos, to)
        })
    }

    /// Card VMESH (ours): the map body a point is SHUT inside - enclosed by
    /// it (`CollisionWorld::enclosing`) where no planning hub of the map is
    /// enclosed by the same body. A room, corridor, cave or tunnel whose
    /// shell is one layer of faces encloses its inside just as a closed
    /// prop does (7,775 shut thinks in one Rhen Var tunnel, over 8 seeds);
    /// the bots' hubs inside it say it is walked in. A boulder or a hollow
    /// trunk has none. `None` = not shut.
    pub fn shut_in(&self, p: Vec3) -> Option<usize> {
        let b = self.collision.enclosing(p)?;
        let body = self.collision.bodies().get(b)?;
        let lived_in = self.nav.as_ref().is_some_and(|g| {
            g.hubs.iter().any(|h| {
                let q = h.pos + Vec3::Y * SHUT_PROBE_UP;
                body.aabb.min.cmple(q).all()
                    && q.cmple(body.aabb.max).all()
                    && self.collision.enclosing(q) == Some(b)
            })
        });
        (!lived_in).then_some(b)
    }

    /// Card VMESH (ours): [`Self::shut_in`] where the body is WATERTIGHT
    /// (`Body::closed`: every edge shared by two faces, such as BF1 Endor's
    /// cp2 boulder) - a solid nobody walks or drives into. The open shells
    /// (rooms, trenches, caves, the hollow trunks) are left to the exit.
    pub fn shut_in_solid(&self, p: Vec3) -> Option<usize> {
        self.shut_in(p)
            .filter(|&b| self.collision.bodies().get(b).is_some_and(|b| b.closed))
    }

    /// FUN_0020b918's placement of one candidate: lifted under a 5 m ray
    /// (y += 5t − 0.1) and dropped by `drop` onto the ground; `None` when the
    /// drop meets nothing.
    fn exit_drop(&self, mut p: Vec3, drop: f32, hulls: HullFilter) -> Option<Vec3> {
        let up = self
            .first_hit(p, p + Vec3::Y * EXIT_RISE, hulls)
            .unwrap_or(1.0);
        p.y += EXIT_RISE * up - 0.1;
        let t = self.first_hit(p, p - Vec3::Y * drop, hulls)?;
        p.y -= drop * t;
        Some(p)
    }

    /// The fraction along `from → to` of the first terrain or SOLDIER-layer
    /// surface, `None` when the segment is clear.
    fn first_hit(&self, from: Vec3, to: Vec3, hulls: HullFilter) -> Option<f32> {
        let d = to - from;
        let len = d.length();
        if len <= 1e-6 {
            return None;
        }
        let dir = d / len;
        let terrain = if dir.x == 0.0 && dir.z == 0.0 {
            // A vertical ray meets the sheet at its height (not where cut).
            self.heightfield.as_ref().and_then(|hf| {
                let h = hf.height_at(from.x, from.z);
                let t = (from.y - h) / (from.y - to.y);
                ((0.0..=1.0).contains(&t) && !self.collision.is_cut(Vec3::new(from.x, h, from.z)))
                    .then_some(t * len)
            })
        } else {
            self.terrain_ray(from, dir, len, 0.5)
        };
        let world = self
            .collision
            .ray_with(from, dir, len, Layers::SOLDIER, hulls)
            .map(|h| h.t);
        let t = match (terrain, world) {
            (Some(a), Some(b)) => a.fmin(b),
            (a, b) => a.or(b)?,
        };
        Some(t / len)
    }

    /// Step 3, for a mounted unit: it stays put and its pad drives the
    /// vehicle when it holds the pilot seat.
    pub(super) fn ride(&mut self, i: usize, pad: &PadState) {
        // The seat's zoom press, read before the buttons are kept as the
        // last tick's (card VTURZ).
        let zoom_press =
            pad.pressed(buttons::ZOOM) && self.units[i].soldier.prev_buttons() & buttons::ZOOM == 0;
        let (inp, full) = driver_input(pad);
        self.tick_seat_zoom(i, zoom_press, inp.throttle);
        let u = &mut self.units[i];
        u.soldier.set_prev_buttons(pad.buttons);
        u.soldier.vel = Vec3::ZERO;
        let human = matches!(u.controller, Controller::Player { .. });
        let ai = u.controller == Controller::Ai;
        let sticks = u.ai.ride.and_then(|r| r.sticks);
        let hover = u.ai.ride.and_then(|r| r.hover);
        let flyer = u.ai.ride.and_then(|r| r.flyer);
        let Some(m) = u.mount else { return };
        let walker = self
            .vehicle(m.vehicle)
            .filter(|_| human && pad.has_look())
            .and_then(|v| self.walker_params.get(v.class as usize)?.clone());
        let flyer = flyer.map(|mut f| {
            f.ai = crate::ai::flyer_drive::commit_plan(self, m.vehicle, f.ai);
            f
        });
        let Some(v) = self.vehicle_mut(m.vehicle) else {
            return;
        };
        if v.seats.get(usize::from(m.seat)).map(|s| s.kind) != Some(SeatKind::Pilot) {
            return;
        }
        if human {
            v.input = inp;
            if v.kind == VehicleKind::Flyer {
                v.flyer.pilot_maneuvers.observe(pad.buttons);
            }
            let full = super::look::mouse_drive(v, pad, full, walker.as_ref());
            ramp_stick_timer(&mut v.hover, full, crate::TICK_DT);
        } else if ai {
            // Card VAI: the bot's pad through the same shaping, as an AI
            // driver — no full-stick timer (re-hover §5.1 I7), no boost (the
            // AI branch of I5 is undecoded) and no jump.
            v.input = HoverInput {
                human: false,
                boost: false,
                jump: false,
                ..inp
            };
            v.hover.stick_timer = 0.0;
            // Card WAI: a walker's driver wrote its sticks raw (the PS2's
            // AI stores E+0x2dc / +0x2e0 unshaped: a walking spider holds
            // (0.997, 1.0), re-walker-ai §7); the pad carries only its aim.
            if let Some([throttle, strafe]) = sticks.filter(|_| v.kind == VehicleKind::Walker) {
                v.input = HoverInput {
                    throttle,
                    strafe,
                    turn: PadState::axis(pad.rx),
                    pitch: PadState::axis(pad.ry),
                    ..v.input
                };
            }
            // Card VDROP: so did a hover's (re-hover-ai §2 H10); the pad
            // carries its gun's pitch.
            if let Some(h) = hover.filter(|_| v.kind == VehicleKind::Hover) {
                let [throttle, strafe, turn] = h.sticks;
                v.input = HoverInput {
                    throttle,
                    strafe,
                    turn,
                    ..v.input
                };
                v.hover.ai = h.ai;
            }
            // A flyer's pilot writes all four, also raw; the pad keeps only
            // its trigger. The takeoff toggle is the AI's request
            // (BF1 FUN_001dd340), through the same edge as a human's button.
            if let Some(f) = flyer.filter(|_| v.kind == VehicleKind::Flyer) {
                let [throttle, strafe, turn, pitch] = f.sticks;
                v.input = HoverInput {
                    throttle,
                    strafe,
                    turn,
                    pitch,
                    jump: f.takeoff,
                    ..v.input
                };
                v.flyer.ai = f.ai;
                // Stuck in flight: destroyed in every mode (see `tick_flyers`).
                if v.flyer.ai.wedges >= crate::ai::flyer_drive::WEDGED_GIVEUP
                    && v.flyer.phase != FlightPhase::Landed
                {
                    self.destroy_vehicle(m.vehicle, None);
                }
            }
        }
    }

    /// Before the hovers tick: a vehicle with no driver — a human, or since
    /// card VAI a bot — at its pilot seat gets no input and a zero timer
    /// (FUN_002f4a50), so a rider who died or left leaves nothing held.
    pub(super) fn clear_driverless(&mut self) {
        for i in 0..self.vehicles.len() {
            let v = &self.vehicles[i];
            if !v.alive {
                continue;
            }
            let driven = v
                .seats
                .first()
                .filter(|s| s.kind == SeatKind::Pilot)
                .and_then(|s| s.occupant)
                .and_then(|o| self.unit(o))
                .is_some_and(|x| {
                    matches!(x.controller, Controller::Player { .. } | Controller::Ai)
                });
            if !driven {
                let v = &mut self.vehicles[i];
                v.input = HoverInput::default();
                v.hover.stick_timer = 0.0;
                v.hover.ai.unseat();
                if v.kind == VehicleKind::Flyer {
                    v.flyer.ai = crate::vehicle::flyer::FlyerAi::default();
                }
            }
        }
    }

    /// After the hovers tick: every rider follows its vehicle's origin, a
    /// building turret's drawn gunner his seat (`GunSetup::gunner`), facing
    /// the way his drawn body faces.
    pub(super) fn carry_riders(&mut self) {
        for i in 0..self.units.len() {
            let Some(m) = self.units[i].mount.filter(|_| self.units[i].alive) else {
                continue;
            };
            if let Some((p, yaw)) = self.vehicle(m.vehicle).map(|v| self.rider_pose(v, m.seat)) {
                self.units[i].soldier.pos = p;
                if let Some(yaw) = yaw {
                    self.units[i].soldier.yaw = yaw;
                }
            }
        }
    }

    /// Where a rider of seat `seat` of `v` stands: the origin, or a drawn
    /// building-turret gunner's seat at the gun's look with the yaw his
    /// drawn body faces (`None` = keep his own).
    pub(super) fn rider_pose(&self, v: &crate::vehicle::Vehicle, seat: u8) -> (Vec3, Option<f32>) {
        let s = usize::from(seat);
        let gun = self
            .seat_setup
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .and_then(|st| st.guns.get(s)?.as_ref());
        match gun.and_then(|g| Some((g, g.gunner?))) {
            Some((g, hang)) if v.kind == VehicleKind::Turret => {
                let look = v
                    .guns
                    .get(s)
                    .and_then(Option::as_ref)
                    .map_or(g.rest().0, |st| st.look);
                let f = hang.frame(look);
                let face = v.rot * f.z_axis.truncate();
                let yaw = (face.x != 0.0 || face.z != 0.0).then(|| yaw_facing(face));
                (v.pos + v.rot * f.w_axis.truncate(), yaw)
            }
            _ => (v.pos, None),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
