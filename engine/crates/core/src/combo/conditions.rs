//! Transition conditions and buffered input events in authored priority order.

use super::*;

pub(super) fn control(name: &str) -> u16 {
    match name.to_ascii_lowercase().as_str() {
        "fire" => FIRE,
        "firesecondary" => FIRE_SECONDARY,
        "fireboth" => FIRE | FIRE_SECONDARY,
        "jump" => JUMP,
        "crouch" => CROUCH,
        "sprint" => SPRINT,
        "reload" => RELOAD,
        "thrust" => THRUST | STRAFE,
        "strafe" => STRAFE,
        "turn" => TURN,
        "all" | "any" => ALL_CONTROLS,
        _ => 0,
    }
}

pub(super) fn posture(name: &str) -> u16 {
    match name.to_ascii_lowercase().as_str() {
        "stand" => STAND,
        "crouch" => POSTURE_CROUCH,
        "prone" => PRONE,
        "sprint" => POSTURE_SPRINT,
        "jump" => POSTURE_JUMP,
        "jet" => JET,
        "roll" | "rollleft" | "rollright" => ROLL,
        "tumble" => TUMBLE,
        "all" | "any" => 127,
        _ => 0,
    }
}

pub(super) fn mask(field: &Field, first: usize, names: fn(&str) -> u16, default: u16) -> u16 {
    let mut out = 0;
    let mut any = false;
    for i in first..field.args.len() {
        let Some(name) = field.text(i) else {
            continue;
        };
        if name.eq_ignore_ascii_case("Frames") || name.eq_ignore_ascii_case("Seconds") {
            continue;
        }
        any = true;
        if let Some(name) = name.strip_prefix('!') {
            out &= !names(name);
        } else {
            out |= names(name);
        }
    }
    if any {
        out
    } else {
        default
    }
}

fn compare(field: &Field, value: f32, factor: f32) -> bool {
    let wanted = number(field, 1).unwrap_or(0.0) * factor;
    // PS2 0x304048 stores one comparison bit: '<' means <=, all others >=.
    if field.text(0).is_some_and(|s| s.starts_with('<')) {
        value <= wanted
    } else {
        value >= wanted
    }
}

pub(super) fn in_angle(angle: f32, lo: f32, hi: f32) -> bool {
    let span = hi - lo;
    if span.abs() >= 360.0 || span == 0.0 {
        return true;
    }
    (angle - lo).rem_euclid(360.0) <= span.rem_euclid(360.0)
}

pub(super) fn last_window(state: &Field, clip: f32, duration: f32) -> f32 {
    state
        .all(K::Transition)
        .flat_map(|t| t.children.iter())
        .filter(|g| matches!(g.keyword, K::If | K::Or))
        .filter_map(|g| g.find(K::TimeEnd))
        .map(|end| time(end, 0, clip, duration))
        .fold(duration, crate::fmath::max)
}

#[derive(Clone, Copy)]
pub(super) struct Selection<'a> {
    pub target: &'a str,
    pub cost: f32,
    pub time: f32,
    pub conditional: bool,
}

fn button(name: &str) -> u8 {
    match name.to_ascii_lowercase().as_str() {
        "fire" => 1,
        "firesecondary" => 2,
        "jump" => 3,
        "crouch" => 4,
        "sprint" => 5,
        "reload" => 6,
        "fireboth" => 7,
        _ => 0,
    }
}
fn button_mask(field: &Field) -> u8 {
    let mut bits = 0;
    for value in &field.args {
        if let manifest::combo::ComboValue::Text(s) = value {
            if s.eq_ignore_ascii_case("Any") || s.eq_ignore_ascii_case("All") {
                bits = 0x3f;
            } else if let Some(s) = s.strip_prefix('!') {
                bits &= !(1 << button(s));
            } else if button(s) != 0 {
                bits |= 1 << button(s);
            }
        }
    }
    bits
}
fn button_mode(field: &Field) -> u8 {
    // Exact native parser switch, including modes absent from hero data.
    match field.text(1).map(output::hash) {
        Some(0x01fcf9b4) => 0,
        Some(0xaaf74aa2) => 1,
        Some(0xadc649b8) => 2,
        Some(0x389c8f75) => 3,
        Some(0x46d056a1) => 4,
        Some(0x486aebad) => 5,
        Some(0x0c1f28db) => 6,
        Some(0x25bef6e8) => 7,
        Some(0x43430b20) => 8,
        Some(0x1036ae7e) => 9,
        Some(0x3db9b915) => 10,
        _ => 0,
    }
}

pub(super) struct Phase {
    pub at_end: bool,
    pub previous: bool,
    pub infinite: bool,
}

impl ComboRuntime {
    /// Native 0033ff90 scalar gates and 00340148 selected event time.
    pub(super) fn group(
        &self,
        group: &Field,
        elapsed: f32,
        duration: f32,
        clip: f32,
        input: ComboInput,
    ) -> Option<f32> {
        let begin = group
            .find(K::TimeStart)
            .map_or(0.0, |f| time(f, 0, clip, duration));
        let end = group
            .find(K::TimeEnd)
            .map_or(duration, |f| time(f, 0, clip, duration));
        let now = elapsed;
        let origin = ee_sub(self.clock, elapsed);
        if now < begin || now > end {
            return None;
        }
        let mut selected = begin;
        let mut pressed = 0;
        let mut released = 0;
        for field in &group.children {
            let ok = match field.keyword {
                K::Break | K::TimeStart | K::TimeEnd => true,
                K::Posture => {
                    let allowed = mask(field, 0, posture, 0);
                    allowed == 0 || input.posture & allowed != 0
                }
                K::Energy => compare(field, input.energy, 1.0),
                K::Thrust => compare(field, input.thrust, 1.0),
                K::VelocityY => compare(field, input.velocity_y, 1.0),
                K::VelocityYAbs => compare(field, input.velocity_y.abs(), 1.0),
                K::VelocityXZ => compare(field, input.velocity_xz, 1.0),
                K::TimeInPosture => compare(
                    field,
                    input.time_in_posture,
                    if has_text(field, "Frames") {
                        1.0 / 30.0
                    } else {
                        1.0
                    },
                ),
                K::ThrustAngle | K::VelocityXZAngle => in_angle(
                    if field.keyword == K::ThrustAngle {
                        input.thrust_angle
                    } else {
                        input.velocity_angle
                    },
                    number(field, 0).unwrap_or(0.0),
                    number(field, 1).unwrap_or(0.0),
                ),
                K::Button => {
                    let button = button(field.text(0).unwrap_or_default());
                    let at = self.history.query(
                        self.clock,
                        ee_add(origin, begin),
                        ee_add(origin, end),
                        button,
                        button_mode(field),
                    )?;
                    selected = ee_sub(at, origin);
                    button != 0
                }
                K::ButtonsPressed => {
                    pressed = button_mask(field);
                    true
                }
                K::ButtonsReleased => {
                    released = button_mask(field);
                    true
                }
                _ => false,
            };
            if !ok {
                return None;
            }
        }
        self.history.lists(pressed, released).then_some(selected)
    }

    /// All matching groups compete by selected event time; authored order wins
    /// ties. Nonbreaking groups are examined at duration, not latched early.
    pub(super) fn pick<'a>(
        &self,
        state: &'a Field,
        elapsed: f32,
        duration: f32,
        clip: f32,
        input: ComboInput,
        phase: Phase,
    ) -> Option<Selection<'a>> {
        let Phase {
            at_end,
            previous,
            infinite,
        } = phase;
        let now = elapsed;
        let mut best: Option<Selection<'a>> = None;
        for transition in state.all(K::Transition) {
            let cost = quantize(
                transition
                    .find(K::EnergyCost)
                    .and_then(|f| number(f, 0))
                    .unwrap_or(0.0),
                10.0,
                0,
                1023,
            );
            if cost > 0.0 && input.energy_depleted {
                continue;
            }
            let mut groups = transition
                .children
                .iter()
                .filter(|f| matches!(f.keyword, K::If | K::Or))
                .peekable();
            if groups.peek().is_none() && at_end && !previous {
                let candidate = Selection {
                    target: transition.text(0)?,
                    cost,
                    time: duration,
                    conditional: false,
                };
                if best.is_none_or(|b| candidate.time < b.time) {
                    best = Some(candidate);
                }
            }
            for group in groups {
                if !at_end && !previous {
                    let Some(br) = group.find(K::Break) else {
                        // A zero-duration state admits every group.
                        if !infinite {
                            continue;
                        }
                        if let Some(at) = self.group(group, elapsed, duration, clip, input) {
                            let candidate = Selection {
                                target: transition.text(0)?,
                                cost,
                                time: at,
                                conditional: true,
                            };
                            if best.is_none_or(|b| at < b.time) {
                                best = Some(candidate);
                            }
                        }
                        continue;
                    };
                    if now < time(br, 0, clip, duration) {
                        continue;
                    }
                }
                if let Some(at) = self.group(group, elapsed, duration, clip, input) {
                    let candidate = Selection {
                        target: transition.text(0)?,
                        cost,
                        time: at,
                        conditional: true,
                    };
                    if best.is_none_or(|b| at < b.time) {
                        best = Some(candidate);
                    }
                }
            }
        }
        if best.is_none() && at_end && !previous {
            // Native duration fallback is the last authored destination even
            // when its groups did not pass (0033cda8 line333394).
            best = Some(Selection {
                target: state
                    .all(K::Transition)
                    .last()
                    .and_then(|t| t.text(0))
                    .unwrap_or("IDLE"),
                cost: 0.0,
                time: duration,
                conditional: false,
            });
        }
        best
    }
}
