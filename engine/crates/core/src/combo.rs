//! Deterministic BF2 combo evaluator over the disc's authored config tree.
//! Frames are 30ths; PS2 parsers: 0x304048 conditions, 0x3058c0 attacks,
//! 0x306c48 deflect, 0x307a80 states. Named clip durations come from the bake.
//! Native eight-event FIFO and state clocks follow 0x33cda8/0x33f908.

use crate::fmath::FloatExt;
use manifest::combo::{ComboDef, ComboField as Field, ComboKeyword as K};

mod conditions;
mod input;
mod output;
pub(crate) mod thrust;
mod timing;
pub use output::{AttackWindow, ComboOutput, Deflect, Gravity, Movement};
use timing::{add as ee_add, mul as ee_mul, sub as ee_sub};

pub const FIRE: u16 = 1;
pub const FIRE_SECONDARY: u16 = 2;
pub const JUMP: u16 = 4;
pub const CROUCH: u16 = 8;
pub const SPRINT: u16 = 16;
pub const RELOAD: u16 = 32;
pub const THRUST: u16 = 64;
pub const STRAFE: u16 = 128;
pub const TURN: u16 = 256;
pub const ALL_CONTROLS: u16 = 511;

pub const STAND: u16 = 1;
pub const POSTURE_CROUCH: u16 = 2;
pub const PRONE: u16 = 4;
pub const POSTURE_SPRINT: u16 = 8;
pub const POSTURE_JUMP: u16 = 16;
pub const ROLL: u16 = 32;
pub const JET: u16 = 64;
pub const TUMBLE: u16 = 128;

#[derive(Debug, Clone, Copy, Default)]
pub struct ComboInput {
    pub buttons: u16,
    pub posture: u16,
    /// Actual energy units used by the ODF (not a normalized bar fraction).
    pub energy: f32,
    pub energy_depleted: bool,
    pub normal_restore_rate: f32,
    pub thrust: f32,
    /// Degrees: forward 0, left 90, back 180, right -90.
    pub thrust_angle: f32,
    pub forward: f32,
    pub strafe: f32,
    /// Raw native forward/right axes retained before input factors.
    pub retained_thrust: [f32; 2],
    pub velocity_y: f32,
    pub velocity_xz: f32,
    pub velocity_angle: f32,
    pub time_in_posture: f32,
    pub animation_velocity: [f32; 2],
}

#[derive(Debug, Clone)]
struct Retro {
    state: usize,
    duration: f32,
}

#[derive(Debug, Clone)]
pub struct ComboRuntime {
    pub state: usize,
    pub time: f32,
    pub state_serial: u32,
    pub chain_serial: u32,
    clock: f32,
    start: f32,
    history: input::ButtonHistory,
    retro: Option<Retro>,
    sound_played: bool,
    move_stopped: bool,
    /// A state entered from outside the program ([`ComboRuntime::force`]):
    /// the next step reports it entered.
    forced: bool,
}

impl Default for ComboRuntime {
    fn default() -> Self {
        Self {
            state: usize::MAX,
            time: 0.0,
            state_serial: 0,
            chain_serial: 0,
            clock: 0.0,
            start: 0.0,
            history: input::ButtonHistory::default(),
            retro: None,
            sound_played: false,
            move_stopped: false,
            forced: false,
        }
    }
}

fn animation(state: &Field) -> Option<&str> {
    state.find(K::Animation).and_then(|f| f.text(0))
}
fn same_animation(a: &Field, b: &Field) -> bool {
    match (animation(a), animation(b)) {
        (None, None) => true,
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

impl ComboRuntime {
    pub fn clock(&self) -> f32 {
        self.clock
    }

    /// Enter the program's state `name` from outside it, as the melee
    /// weapon does on a caught saber (FUN_00340738 + FUN_00340350, B2
    /// 333234-333249) or with its saber in flight (B2:333218-333226).
    /// `false` (nothing changes) when the program has no such state.
    pub fn force(&mut self, def: &ComboDef, name: &str) -> bool {
        let states: Vec<_> = def.states().collect();
        let (Some(state), Some(idle)) = (state_index(&states, name), state_index(&states, "IDLE"))
        else {
            return false;
        };
        self.enter(state, idle, 0.0);
        self.forced = true;
        true
    }

    /// Is the runtime in the program's state `name`?
    pub fn in_state(&self, def: &ComboDef, name: &str) -> bool {
        let states: Vec<_> = def.states().collect();
        state_index(&states, name) == Some(self.state)
    }

    fn enter(&mut self, state: usize, idle: usize, time: f32) {
        if self.state == idle || self.state == usize::MAX {
            self.chain_serial = self.chain_serial.wrapping_add(1);
        }
        self.state = state;
        self.state_serial = self.state_serial.wrapping_add(1);
        self.time = time;
        self.sound_played = false;
        self.move_stopped = false;
    }

    pub fn step<'a>(
        &mut self,
        def: &'a ComboDef,
        clip_seconds: &[f32],
        input: ComboInput,
        dt: f32,
    ) -> ComboOutput<'a> {
        let velocity = input.animation_velocity;
        self.step_with_animation(def, clip_seconds, input, dt, |_, _| velocity)
    }

    /// FIFO records use the old weapon clock. State elapsed advances before
    /// transition checks; the weapon clock advances after all checks/output.
    pub fn step_with_animation<'a>(
        &mut self,
        def: &'a ComboDef,
        clip_seconds: &[f32],
        mut input: ComboInput,
        dt: f32,
        mut sample: impl FnMut(Option<&str>, f32) -> [f32; 2],
    ) -> ComboOutput<'a> {
        let states: Vec<_> = def.states().collect();
        let Some(idle) = states
            .iter()
            .position(|s| s.text(0).is_some_and(|n| n.eq_ignore_ascii_case("IDLE")))
        else {
            return ComboOutput::default();
        };
        if let Some(remap) = def.find(K::RemapReloadButton).and_then(|f| f.text(0)) {
            if input.buttons & RELOAD != 0 {
                let target = conditions::control(remap);
                if target != 0 && target != RELOAD {
                    input.buttons = (input.buttons & !RELOAD) | target;
                }
            }
        }
        let dt = if dt.is_finite() { dt.fmax(0.0) } else { 0.0 };
        let held = ((input.buttons & 0x3f) << 1) as u8;
        let held = held | if held & 6 == 6 { 0x80 } else { 0 };
        self.history.update(self.clock, held);
        let initialized = self.state >= states.len();
        if initialized {
            self.enter(idle, idle, 0.0);
        }
        let before = self.time;
        let old_origin = ee_sub(self.clock, before);
        self.time = ee_add(self.time, dt);
        let mut entered = initialized || std::mem::take(&mut self.forced);
        let mut cost = 0.0;
        let mut exited_elapsed = 0.0;
        let mut transitions = 0;
        let mut clear_hits = false;
        // Valid disc programs terminate through their state update or the
        // MustShowOneFrame flag. Bound malformed cycles independently of dt.
        for _ in 0..states.len().saturating_mul(4).max(1) {
            let state = states[self.state];
            let clip = clip_seconds.get(self.state).copied().unwrap_or(0.0);
            let authored_duration = duration(state, clip);
            let infinite = authored_duration == 0.0;
            let limit = if infinite {
                self.time
            } else {
                authored_duration
            };
            let at_end = !infinite && self.time >= limit;
            let mut chosen = None;
            if !at_end {
                if let Some(old) = &self.retro {
                    let previous = states[old.state];
                    let old_clip = clip_seconds.get(old.state).copied().unwrap_or(0.0);
                    chosen = self
                        .pick(
                            previous,
                            ee_add(self.time, old.duration),
                            old.duration,
                            old_clip,
                            input,
                            conditions::Phase {
                                at_end: false,
                                previous: true,
                                infinite: false,
                            },
                        )
                        .map(|mut s| {
                            s.time = ee_sub(s.time, old.duration);
                            s
                        });
                }
            }
            if chosen.is_none() {
                chosen = self.pick(
                    state,
                    self.time,
                    limit,
                    clip,
                    input,
                    conditions::Phase {
                        at_end,
                        previous: false,
                        infinite,
                    },
                );
            }
            let Some(chosen) = chosen else {
                break;
            };
            let target = state_index(&states, chosen.target).unwrap_or(idle);
            if target == self.state {
                break;
            }
            let old_time = self.time;
            self.history
                .prune(ee_sub(ee_add(chosen.time, self.clock), old_time));
            let carry = same_animation(state, states[target]);
            let retain_previous = at_end
                && !chosen.conditional
                && conditions::last_window(state, clip, limit) > limit;
            self.retro = retain_previous.then_some(Retro {
                state: self.state,
                duration: limit,
            });
            // A late previous-state condition can replace a recovery state;
            // both ordinary and replacement exits rebase contact-cache time.
            exited_elapsed += old_time;
            self.enter(target, idle, if carry { old_time } else { dt });
            cost += chosen.cost;
            entered = true;
            transitions += 1;
            if states[target].find(K::MustShowOneFrame).is_some() {
                break;
            }
        }
        let state = states[self.state];
        let allowed = state
            .find(K::Posture)
            .map_or(0, |f| conditions::mask(f, 0, conditions::posture, 0));
        let interrupts = input.posture & (ROLL | TUMBLE) != 0
            || (input.posture & (POSTURE_JUMP | JET) != 0 && allowed & (POSTURE_JUMP | JET) == 0);
        if self.state != idle && allowed != 0 && allowed & input.posture == 0 && interrupts {
            exited_elapsed += self.time;
            self.enter(idle, idle, dt);
            self.retro = None;
            entered = true;
            transitions += 1;
            clear_hits = true;
        }
        let state = states[self.state];
        let clip = clip_seconds.get(self.state).copied().unwrap_or(0.0);
        self.start = ee_sub(self.clock, self.time);
        let movement_state = if state.find(K::AnimatedMove).is_some() {
            state
        } else {
            self.retro.as_ref().map_or(state, |old| states[old.state])
        };
        input.animation_velocity = sample(animation(movement_state), self.time);
        let after_clock = ee_add(self.clock, dt);
        let new_origin = ee_sub(after_clock, self.time);
        let mut out = self.output(
            state,
            self.retro.as_ref().map(|old| states[old.state]),
            output::OutputFrame {
                clip,
                input,
                before: if entered {
                    ee_sub(self.time, dt)
                } else {
                    before
                },
                entered,
                cache_origin: new_origin,
            },
        );
        out.energy_cost = cost;
        out.cache_shift = if transitions == 0 {
            0.0
        } else {
            new_origin - old_origin - exited_elapsed
        };
        out.clear_hits = clear_hits;
        self.clock = after_clock;
        self.start = new_origin;
        out
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&(self.state as u64).to_le_bytes());
        for value in [self.time, self.clock, self.start] {
            eat(&value.hash_bits().to_le_bytes());
        }
        for value in [self.state_serial, self.chain_serial] {
            eat(&value.to_le_bytes());
        }
        eat(&[
            self.sound_played as u8,
            self.move_stopped as u8,
            self.forced as u8,
        ]);
        self.history.hash_into(eat);
        eat(&[self.retro.is_some() as u8]);
        if let Some(r) = &self.retro {
            eat(&(r.state as u64).to_le_bytes());
            eat(&r.duration.hash_bits().to_le_bytes());
        }
    }
}

fn state_index(states: &[&Field], name: &str) -> Option<usize> {
    states
        .iter()
        .position(|s| s.text(0).is_some_and(|n| n.eq_ignore_ascii_case(name)))
}

fn number(field: &Field, index: usize) -> Option<f32> {
    // 0x535e78 copies raw argument words; numeric-looking strings do not
    // become numbers. In particular, the disc's TimeStart("-12") packs 0.
    field.number(index)
}

fn quantize(value: f32, scale: f32, lo: i32, hi: i32) -> f32 {
    ((value * scale) as i32).clamp(lo, hi) as f32 / scale
}

fn has_text(field: &Field, value: &str) -> bool {
    field
        .args
        .iter()
        .any(|a| matches!(a, manifest::combo::ComboValue::Text(s) if s.eq_ignore_ascii_case(value)))
}

fn time(field: &Field, index: usize, clip: f32, duration: f32) -> f32 {
    let mut value = number(field, index).unwrap_or(0.0);
    if has_text(field, "Frames") {
        value = ee_mul(value, 1.0 / 30.0);
    }
    let (lo, hi) = if matches!(field.keyword, K::TimeStart | K::TimeEnd) {
        (-512, 511)
    } else if field.keyword == K::Break {
        (0, 511)
    } else {
        (-1023, 1023)
    };
    value = ee_mul((ee_mul(value, 100.0) as i32).clamp(lo, hi) as f32, 0.01);
    if has_text(field, "FromAnim") {
        value *= clip;
    }
    if has_text(field, "FromEnd") {
        value = ee_add(value, duration);
    }
    value
}

fn duration(state: &Field, clip: f32) -> f32 {
    // 0030a480 resolves the default clip length into unsigned hundredths.
    state.find(K::Duration).map_or_else(
        || ee_mul((ee_mul(clip, 100.0) as i32).clamp(0, 1022) as f32, 0.01),
        |f| time(f, 0, clip, clip).fmax(0.0),
    )
}

#[cfg(test)]
mod tests;
