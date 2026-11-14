//! Native eight-event input history, B2 0033f908/0033fb80/0033fe38.

use super::timing::add;
use crate::fmath::FloatExt;

#[derive(Debug, Clone)]
pub(super) struct ButtonEvent {
    pub time: f32,
    pub button: u8,
    pub flags: u8,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ButtonHistory {
    pub events: Vec<ButtonEvent>,
    pub held: u8,
}

impl ButtonHistory {
    pub fn update(&mut self, clock: f32, held: u8) {
        for button in 1..=7 {
            let bit = 1 << button;
            let last = self.events.iter().rev().find(|e| e.button == button);
            let flags = if held & bit != 0 {
                if self.held & bit == 0 {
                    self.held |= bit;
                    if last.is_some_and(|e| e.flags & 3 == 1 && clock <= add(e.time, 0.5)) {
                        Some(4)
                    } else {
                        Some(0)
                    }
                } else {
                    match last {
                        None => Some(2),
                        Some(e)
                            if e.flags & 1 == 0 && e.flags & 2 == 0 && add(e.time, 0.3) < clock =>
                        {
                            Some(e.flags | 2)
                        }
                        _ => None,
                    }
                }
            } else if self.held & bit != 0 {
                self.held &= !bit;
                last.and_then(|e| {
                    (e.flags & 1 == 0).then_some(if e.flags & 2 == 0 && clock <= add(e.time, 0.3) {
                        e.flags | 1
                    } else {
                        e.flags | 3
                    })
                })
            } else {
                None
            };
            if let Some(flags) = flags {
                if self.events.len() == 8 {
                    self.events.remove(0);
                }
                self.events.push(ButtonEvent {
                    time: clock,
                    button,
                    flags,
                });
            }
        }
    }

    pub fn prune(&mut self, cutoff: f32) {
        self.events.retain(|e| e.time > cutoff);
    }

    /// Return the native selected absolute event time, including Down/Up's
    /// continuously-held fallback at the requested window's start.
    pub fn query(&self, clock: f32, begin: f32, end: f32, button: u8, mode: u8) -> Option<f32> {
        if mode == 10 || mode == 8 {
            let down = mode == 10;
            let mut before = false;
            let mut crossing = false;
            for event in self.events.iter().filter(|e| e.button == button) {
                if event.time > end {
                    break;
                }
                let is_down = event.flags & 1 == 0;
                if event.time < begin {
                    before = is_down == down;
                } else if is_down == down {
                    return Some(event.time);
                } else {
                    // Native Down accepts a preceding down interval ending at
                    // this release; Up likewise accepts a short press interval.
                    if down || event.flags & 2 == 0 {
                        crossing = true;
                        break;
                    }
                }
            }
            if before
                || crossing
                || (begin <= clock && clock <= end && (self.held & (1 << button) != 0) == down)
            {
                Some(begin)
            } else {
                None
            }
        } else {
            let extra = if mode & 4 != 0 { 4 } else { 0 };
            let (mask, wanted) = if mode == 9 {
                (1, 1)
            } else {
                (
                    extra | if mode & 1 != 0 { 3 } else { 2 },
                    extra | (mode & 3),
                )
            };
            self.events
                .iter()
                .find(|e| {
                    e.button == button
                        && e.flags & mask == wanted
                        && begin <= e.time
                        && e.time <= end
                })
                .map(|e| e.time)
        }
    }

    pub fn lists(&self, pressed: u8, released: u8) -> bool {
        pressed | released == 0
            || self.events.iter().any(|e| {
                let bit = 1 << e.button;
                (pressed & bit != 0 && e.flags & 3 == 0)
                    || (released & bit != 0 && e.flags & 1 != 0)
            })
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&[self.held, self.events.len() as u8]);
        for event in &self.events {
            eat(&event.time.hash_bits().to_le_bytes());
            eat(&[event.button, event.flags]);
        }
    }
}

#[cfg(test)]
mod tests;
