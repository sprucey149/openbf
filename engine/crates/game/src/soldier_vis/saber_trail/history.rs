//! Native eight-sample origin/direction history (B2 45c6f0/45c940),
//! Hermite evaluation (45dbb8/45dd08), and the 45c308 draw defaults.

use core::fmath::FloatExt;
use glam::Vec3;
use std::collections::VecDeque;

pub(super) const LIFE: f32 = 0.12;
const FULL_AGE: f32 = 0.02;
const FADE_DISTANCE: f32 = 0.35;
const TIME_STEP: f32 = 0.01;
const DISTANCE_STEP: f32 = 0.1;

#[derive(Clone, Copy, Debug)]
struct Sample {
    time: f32,
    origin: Vec3,
    direction: Vec3,
    velocity: Vec3,
    angular: Vec3,
    distance: f32,
}

#[derive(Default)]
pub(super) struct History {
    clock: f32,
    active: bool,
    samples: VecDeque<Sample>,
    previous: Option<(Vec3, Vec3)>,
    pub length: f32,
    pub width: f32,
    pub color: [u8; 4],
}

fn hermite(a: Vec3, b: Vec3, da: Vec3, db: Vec3, dt: f32, u: f32) -> Vec3 {
    let smooth = (3.0 - 2.0 * u) * u * u;
    let t = dt * u * (1.0 - u);
    a * (1.0 - smooth) + b * smooth + da * ((1.0 - u) * t) - db * (u * t)
}

fn speed(a: Vec3, b: Vec3) -> f32 {
    let (la, lb) = (a.length(), b.length());
    let mean = (la + lb) * 0.5;
    if la > 0.001 && lb > 0.001 {
        mean * (0.75 + a.dot(b) * 0.25 / (la * lb))
    } else {
        mean
    }
}

impl History {
    fn append(&mut self, time: f32, origin: Vec3, direction: Vec3, smooth: bool) {
        let mut next = Sample {
            time,
            origin,
            direction,
            velocity: Vec3::ZERO,
            angular: Vec3::ZERO,
            distance: 0.0,
        };
        let count = self.samples.len();
        if smooth {
            if let Some(prev) = self.samples.back_mut() {
                let dt = time - prev.time;
                if dt > 0.0 {
                    let velocity = (origin - prev.origin) / dt;
                    let dot = direction.dot(prev.direction).clamp(-1.0, 1.0);
                    let (at_prev, at_next) = if dot >= 0.999 {
                        let angular = (direction - prev.direction) / dt;
                        (angular, angular)
                    } else {
                        let bisector = (2.0 + dot * 2.0).sqrt();
                        if bisector > 0.0 {
                            let k = (2.0 - bisector) * 4.0 / (dt * bisector * (1.0 - dot));
                            (
                                (direction - prev.direction * dot) * k,
                                (direction * dot - prev.direction) * k,
                            )
                        } else {
                            (Vec3::ZERO, Vec3::ZERO)
                        }
                    };
                    if count == 1 || prev.distance <= 0.0 {
                        prev.velocity = velocity;
                        prev.angular = at_prev;
                    } else {
                        prev.velocity = (prev.velocity + velocity) * 0.5;
                        prev.angular = (prev.angular + at_prev) * 0.5;
                    }
                    next.velocity = velocity * 2.0 - prev.velocity;
                    next.angular = at_next;
                    next.distance = (speed(prev.velocity, next.velocity).fmax(speed(
                        prev.velocity + prev.angular,
                        next.velocity + next.angular,
                    )) * dt)
                        .fmax(0.001);
                }
            }
        }
        if self.samples.len() == 8 {
            self.samples.pop_front();
        }
        self.samples.push_back(next);
    }

    pub fn step(&mut self, dt: f32, frame: Option<(Vec3, Vec3, f32, [u8; 4])>, active: bool) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        if self.samples.is_empty() && !self.active {
            self.clock = 0.0;
        }
        self.clock += dt;
        while self.samples.len() > 1 && self.samples[1].time < self.clock - LIFE {
            self.samples.pop_front();
        }
        if self
            .samples
            .back()
            .is_some_and(|s| s.time < self.clock - LIFE)
        {
            self.samples.clear();
        }
        if let Some((origin, end, width, color)) = frame {
            let span = end - origin;
            let length = span.length();
            if origin.is_finite() && end.is_finite() && length > 0.0 {
                let direction = span / length;
                self.length = length;
                self.width = width;
                self.color = color;
                if active || self.active {
                    if self
                        .samples
                        .back()
                        .is_none_or(|s| self.clock - s.time > dt * 1.5)
                    {
                        let (p, d) = self.previous.unwrap_or((origin, direction));
                        self.append(self.clock - dt, p, d, false);
                    }
                    self.append(self.clock, origin, direction, true);
                }
                self.previous = Some((origin, direction));
            }
        }
        self.active = active;
    }

    pub fn sections(&self) -> Vec<(Vec3, Vec3, f32, [u8; 4])> {
        let mut out = Vec::new();
        let mut distance = 0.0;
        let rgb: [u8; 3] = std::array::from_fn(|k| {
            ((f32::from(self.color[k]) * f32::from(self.color[3]) + 0.5) / 255.0) as u8
        });
        for k in 1..self.samples.len() {
            let a = self.samples[k - 1];
            let b = self.samples[k];
            let dt = b.time - a.time;
            if dt <= 0.0 || b.distance <= 0.0 {
                continue;
            }
            let count = ((dt / TIME_STEP) as usize + 1)
                .max((b.distance / DISTANCE_STEP) as usize + 1)
                .min(255);
            for j in usize::from(k > 1)..=count {
                let u = j as f32 / count as f32;
                if j > 0 {
                    distance += b.distance / count as f32;
                }
                let time = a.time + dt * u;
                let age = self.clock - time;
                let fade = if age <= FULL_AGE {
                    1.0
                } else {
                    {
                        let alpha = ((LIFE - age) / (LIFE - FULL_AGE)).clamp(0.0, 1.0);
                        alpha * alpha
                    }
                } * (distance / FADE_DISTANCE).fmin(1.0);
                let origin = hermite(a.origin, b.origin, a.velocity, b.velocity, dt, u);
                let direction = hermite(a.direction, b.direction, a.angular, b.angular, dt, u);
                let color = [rgb[0], rgb[1], rgb[2], (fade * 255.0) as u8];
                out.push((origin, origin + direction * self.length, self.width, color));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
