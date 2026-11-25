//! Actual posed blade queries against soldiers and destructible hurt primitives.

use super::*;
use crate::fmath::FloatExt;
use crate::{
    collision::Layers,
    hero_weapon::BladeSegment,
    projectile::{building_hit, Hit, Target},
    unit::UNIT_SNAP_M,
};
use glam::{Mat4, Quat};

pub(super) const TRACKED_RECORD: u64 = 1 << 63;
/// OURS: minimum blade endpoint speed and the settling time between swings.
/// measure: slow taps and full controller swings in a headset.
const SWING_SPEED: f32 = 1.2;
const QUIET_TICKS: u8 = 3;
const SWING_TRAVEL: f32 = SWING_SPEED * TICK_DT;
/// OURS: separation between samples and required travel before a reversal
/// counts as a fresh cut. measure: grazes, backswings and hand jitter in VR.
const TRACKED_STEP: f32 = 0.08;
const REVERSAL_TRAVEL: f32 = 0.16;
/// OURS: 400 ms between tracked hits from the same hand on the same target.
/// measure: repeated thrusts and alternating cuts in a headset.
const TRACKED_HIT_COOLDOWN: u64 = 2 * crate::TICK_HZ as u64 / 5;

type HandPose = (Vec3, Quat);

#[derive(Clone, Copy)]
struct TrackedArc {
    previous: HandPose,
    current: HandPose,
    local: BladeSegment,
    feet: [Vec3; 2],
    tick_range: [f32; 2],
}

impl TrackedArc {
    fn at(self, t: f32) -> BladeSegment {
        let (from, a) = self.previous;
        let (to, b) = self.current;
        let position = self.feet[0].lerp(self.feet[1], t) + from.lerp(to, t);
        let rotation = a.lerp(b, t);
        BladeSegment {
            start: position + rotation * self.local.start,
            end: position + rotation * self.local.end,
        }
    }

    fn steps(self) -> usize {
        let (from, a) = self.previous;
        let (to, b) = self.current;
        let translation = (to - from + self.feet[1] - self.feet[0]).length();
        // Overestimate angular displacement from the shortest quaternion
        // chord, since endpoints alone can undercount travel around a pivot.
        let chord = (if a.dot(b) < 0.0 { a + b } else { a - b }).length();
        let angle_bound = chord * (std::f32::consts::PI * std::f32::consts::FRAC_1_SQRT_2);
        let reach = self.local.start.length().fmax(self.local.end.length());
        ((translation + angle_bound * reach) / TRACKED_STEP)
            .ceil()
            .clamp(1.0, 64.0) as usize
    }

    fn tick_fraction(self, t: f32) -> f32 {
        self.tick_range[0] + (self.tick_range[1] - self.tick_range[0]) * t
    }
}

fn tracked_record(hand: usize, stroke: u32) -> u64 {
    TRACKED_RECORD | (u64::from(stroke) << 1) | hand as u64
}

struct Sweep {
    attack: crate::combo::AttackWindow,
    current: BladeSegment,
    previous: BladeSegment,
    length: f32,
    width: f32,
    incoming: Vec3,
    stroke_direction: Vec3,
    root_inset: f32,
    arc: Option<TrackedArc>,
}

struct SaberHit {
    target: Target,
    attack: crate::combo::AttackWindow,
    damage: f32,
    pos: Vec3,
    direction: Vec3,
    stroke_direction: Vec3,
    guard_direction: Vec3,
}

impl Sim {
    /// A tracked blade's substeps meet moving foot targets at matching times.
    pub(in crate::sim) fn saber_target_starts(
        &self,
        pads: &[PadState],
    ) -> Vec<Option<(u32, Vec3)>> {
        if !pads.iter().any(|pad| {
            pad.tracked_combat
                .is_some_and(|combat| combat.hands.iter().any(Option::is_some))
        }) {
            return Vec::new();
        }
        self.units
            .iter()
            .map(|unit| {
                (unit.alive && unit.mount.is_none()).then_some((unit.gen, unit.soldier.pos))
            })
            .collect()
    }

    pub(in crate::sim) fn saber_hits_with(
        &mut self,
        pads: &[PadState],
        starts: &[Option<(u32, Vec3)>],
    ) {
        for i in 0..self.units.len() {
            self.saber_hits_for(i, pads.get(i).and_then(|pad| pad.tracked_combat), starts);
            if let Some(state) = self.units[i].weapon.melee.as_mut() {
                if let Some(tracked) = state.tracked {
                    state.hit.retain(|hit| {
                        hit.record < TRACKED_RECORD
                            || tracked.hands.iter().enumerate().any(|(hand, state)| {
                                state.pose.is_some()
                                    && hit.record == tracked_record(hand, state.stroke)
                            })
                    });
                }
                state.expire_hits();
            }
        }
    }

    fn saber_hits_for(
        &mut self,
        i: usize,
        tracked: Option<crate::sim::TrackedCombat>,
        starts: &[Option<(u32, Vec3)>],
    ) {
        let u = &self.units[i];
        let free = u.alive && u.mount.is_none() && u.choked.is_none();
        let Some(class) = self.weapon_classes[u.weapon_class].saber.as_ref() else {
            return;
        };
        let Some(state) = u.weapon.melee.as_ref() else {
            return;
        };
        let tracked = tracked.filter(|_| u.is_human() && class.has_tracked_blade());
        let sweeps = if let Some(tracked) = tracked {
            let can_swing = free && u.soldier.can_swing_tracked();
            Self::tracked_sweeps(class, &mut self.units[i], tracked, can_swing, self.tick)
        } else {
            if state.tracked.is_some() {
                let state = self.units[i].weapon.melee.as_mut().unwrap();
                state.tracked = None;
                state.hit.retain(|hit| hit.record < TRACKED_RECORD);
            }
            if !free || self.units[i].soldier.no_fire() {
                return;
            }
            self.combo_sweeps(i)
        };
        let u = &self.units[i];
        let state = u.weapon.melee.as_ref().unwrap();
        let default_damage = if tracked.is_some() && !sweeps.is_empty() {
            crate::combo::tracked_swing_damage(&class.combo).unwrap_or(class.damage)
        } else {
            class.damage
        };
        let mut hits = Vec::new();
        for sweep in sweeps {
            let Sweep {
                attack,
                current,
                previous,
                length,
                width,
                incoming,
                stroke_direction,
                root_inset,
                arc,
            } = sweep;
            let BladeSegment { start, end } = current;
            let BladeSegment {
                start: old_start,
                end: old_end,
            } = previous;
            let travel = (start - old_start).length().fmax((end - old_end).length());
            let steps = arc.map_or_else(
                || (travel / 0.5).ceil().fmax(1.0) as usize,
                TrackedArc::steps,
            );
            let ray = |a: Vec3, b: Vec3| {
                let direction = (b - a).normalize_or_zero();
                (
                    a - direction * root_inset,
                    b - direction * root_inset,
                    direction,
                )
            };
            let arc_rays = arc.map(|arc| {
                (0..=steps)
                    .map(|step| {
                        let segment = arc.at(step as f32 / steps as f32);
                        ray(segment.start, segment.end)
                    })
                    .collect::<Vec<_>>()
            });
            let sample = |step: usize| {
                if let Some(rays) = &arc_rays {
                    rays[step]
                } else {
                    let fraction = step as f32 / steps as f32;
                    ray(old_start.lerp(start, fraction), old_end.lerp(end, fraction))
                }
            };
            // The body query backs the ray up by its width. In VR the ray
            // still needs to reach the glowing tip after that offset.
            let body_length = if arc.is_some() {
                length + width
            } else {
                length
            };
            let contact = |target, pos, guard_direction| SaberHit {
                target,
                attack,
                damage: attack.damage.unwrap_or(default_damage),
                pos,
                direction: incoming,
                stroke_direction,
                guard_direction,
            };
            for t in 0..self.units.len() {
                let handle = self.handle(t);
                // [ours] (the maintainer, 2026-10-30: "So the shield has to be
                // worn down before doing damage to the droideka"): a raised
                // bubble meets the blade first.
                let bubble = self.units[t].droideka_shield();
                let target = if bubble.is_some() {
                    Target::Shield(handle)
                } else {
                    Target::Unit(handle)
                };
                if t == i || !self.hero_body_active(t) || state.already_hit(&attack, target) {
                    continue;
                }
                let prims = self.hero_body_prims(t);
                if bubble.is_none() && prims.is_none() {
                    continue;
                }
                let unit = &self.units[t];
                let shift = if arc.is_some() && unit.mount.is_none() {
                    starts
                        .get(t)
                        .copied()
                        .flatten()
                        .and_then(|(gen, pos)| {
                            let delta = pos - unit.soldier.pos;
                            (gen == unit.gen && delta.length_squared() <= UNIT_SNAP_M * UNIT_SNAP_M)
                                .then_some(delta)
                        })
                        .unwrap_or_default()
                } else {
                    Vec3::ZERO
                };
                let touched = (0..=steps).find_map(|step| {
                    let (root, _, direction) = sample(step);
                    let offset = arc.map_or(Vec3::ZERO, |arc| {
                        shift * (1.0 - arc.tick_fraction(step as f32 / steps as f32))
                    });
                    match (&bubble, &prims) {
                        (Some(b), _) => crate::unit::ray_sphere(
                            root,
                            direction,
                            b.center + offset,
                            b.radius + width,
                        )
                        .filter(|&t| t < length)
                        .map(|t| (t, incoming)),
                        (None, Some(p)) => p
                            .melee_contact(root - offset, direction, body_length, width)
                            .map(|t| {
                                let guard_direction = if arc.is_some() {
                                    // A slash's tip can travel sideways even when
                                    // the hilt is behind the defender's guard.
                                    let from_hilt = p.pos + offset - root;
                                    Vec3::new(from_hilt.x, 0.0, from_hilt.z).normalize_or(incoming)
                                } else {
                                    incoming
                                };
                                (t, guard_direction)
                            }),
                        (None, None) => None,
                    }
                    .map(|(t, guard_direction)| (root + direction * t, guard_direction))
                });
                if let Some((pos, guard_direction)) = touched {
                    hits.push(contact(target, pos, guard_direction));
                }
            }
            // [ours] (the maintainer, 2026-10-30: "Make sure lightsabers and
            // heroes can destroy them too"): the sniper's dropped turret and a
            // remote droid are spheres to the blade, as to a bolt. The native
            // gather's mask does not name these; the wielder's own are spared.
            let items = self
                .droids
                .iter()
                .filter(|d| d.owner.index as usize != i)
                .map(|d| (Target::Droid(d.id), d.pos, crate::remote::RADIUS))
                .chain(
                    self.turrets
                        .iter()
                        .filter(|t| t.owner.index as usize != i)
                        .map(|t| (Target::Turret(t.id), t.centre(), crate::autoturret::RADIUS)),
                );
            for (target, at, radius) in items {
                if state.already_hit(&attack, target) {
                    continue;
                }
                let touched = (0..=steps).find_map(|step| {
                    let (root, _, direction) = sample(step);
                    crate::unit::ray_sphere(root, direction, at, radius + width)
                        .filter(|&t| t < length)
                        .map(|t| root + direction * t)
                });
                if let Some(pos) = touched {
                    hits.push(contact(target, pos, incoming));
                }
            }
            for body in self.collision.bodies_and_hulls() {
                if !body.layers.contains(Layers::ORDNANCE) {
                    continue;
                }
                let target = body.owner.map_or_else(
                    || building_hit(&self.collision, body.instance),
                    |v| Hit::Target(Target::Vehicle(v)),
                );
                let Hit::Target(target) = target else {
                    continue;
                };
                if state.already_hit(&attack, target) {
                    continue;
                }
                let touched = (0..=steps).find_map(|step| {
                    let (root, end, direction) = sample(step);
                    body.blade_contact_at(root, end, width)
                        .map(|t| root + direction * t)
                });
                if let Some(pos) = touched {
                    hits.push(contact(target, pos, incoming));
                }
            }
        }
        let mut struck = false;
        let wc = u.weapon_class;
        for SaberHit {
            target,
            attack,
            damage,
            pos,
            direction,
            stroke_direction,
            guard_direction,
        } in hits
        {
            let owner = self.handle(i);
            let hand = (attack.record >= TRACKED_RECORD).then_some((attack.record & 1) as u8);
            let state = self.units[i].weapon.melee.as_mut().unwrap();
            if let Some(hand) = hand {
                let memory = &mut state.tracked.as_mut().unwrap().hands[usize::from(hand)];
                if attack.record == tracked_record(usize::from(hand), memory.stroke)
                    && !state.hit.iter().any(|hit| hit.record == attack.record)
                {
                    // A wide wind-up can start well off the contact line.
                    // Compare the return with the first cut in this stroke.
                    memory.direction = stroke_direction;
                }
            }
            if !state.record_hit(&attack, target) {
                continue;
            }
            let cooldown_target = match target {
                Target::Shield(unit) => Target::Unit(unit),
                _ => target,
            };
            if hand.is_some_and(|hand| {
                state.tracked_hit_cooldowns[usize::from(hand)]
                    .iter()
                    .any(|&(victim, _)| victim == cooldown_target)
            }) {
                continue;
            }
            if let Target::Unit(h) = target {
                if self.saber_deflect(h, guard_direction, pos) {
                    if let Some(hand) = hand {
                        self.events.push(crate::sim::Event::SaberHandContact {
                            attacker: owner,
                            hand,
                            pos,
                            blocked: true,
                        });
                    }
                    continue;
                }
            }
            let mut desc = self.weapon_classes[wc].ordnance.damage_desc(0.0, false);
            desc.damage = damage;
            desc.scales = [1.0; 7];
            desc.shield_scale = 1.0;
            let team = self.units[i].team;
            let confirmed = match target {
                Target::Unit(h) => self.hero_damage(i, h, desc, direction) > 0.0,
                Target::Shield(h) => {
                    self.saber_on_shield(i, h, desc);
                    true
                }
                Target::Vehicle(h) => {
                    self.damage_vehicle_by(h, &desc, 1.0, Some(owner), Some(team), Some(direction))
                        > 0.0
                }
                Target::Station(n) => {
                    self.damage_station(n, &desc, 1.0, Some(owner), Some(team));
                    true
                }
                Target::Droid(_) | Target::Turret(_) => {
                    self.damage_item(target, desc.damage, &desc.scales, Some(team), Some(owner));
                    true
                }
                _ => false,
            };
            if let Some(hand) = hand.filter(|_| confirmed) {
                self.units[i]
                    .weapon
                    .melee
                    .as_mut()
                    .unwrap()
                    .tracked_hit_cooldowns[usize::from(hand)]
                .push((cooldown_target, self.tick + TRACKED_HIT_COOLDOWN));
                self.events.push(crate::sim::Event::SaberHandContact {
                    attacker: owner,
                    hand,
                    pos,
                    blocked: false,
                });
            }
            // HitEffect at every victim (B2:336296-336333).
            self.events.push(crate::sim::Event::SaberContact {
                pos,
                weapon_class: wc as u32,
                deflect: false,
            });
            struck = true;
            if let Target::Unit(h) = target {
                if attack.push != 0.0 {
                    self.hero_knock(h, direction, attack.push, owner);
                }
            }
        }
        // One HitSound an update, whatever it struck (B2:336346-336356).
        if struck {
            if let Some(sound_hash) = self.weapon_classes[wc]
                .saber
                .as_ref()
                .and_then(|c| c.hit_sound.as_deref())
                .map(crate::hero_weapon::sound_hash)
            {
                self.events.push(crate::sim::Event::ComboSound {
                    unit: self.handle(i),
                    weapon_class: wc as u32,
                    sound_hash,
                });
            }
        }
    }
    fn tracked_sweeps(
        class: &crate::hero_weapon::SaberClass,
        unit: &mut crate::unit::Unit,
        input: crate::sim::TrackedCombat,
        free: bool,
        tick: u64,
    ) -> Vec<Sweep> {
        let state = unit.weapon.melee.as_mut().unwrap();
        for hand in &mut state.tracked_hit_cooldowns {
            hand.retain(|&(_, next_tick)| tick < next_tick);
        }
        let flying = unit.weapon.saber_flight.is_some()
            || unit
                .held
                .iter()
                .any(|held| held.state.saber_flight.is_some());
        let mut memory = state.tracked.unwrap_or_default();
        if state.tracked.is_none()
            || memory.motion_epoch != input.motion_epoch
            || memory.tick.checked_add(1) != Some(tick)
        {
            memory.hands = Default::default();
            state.hit.retain(|hit| hit.record < TRACKED_RECORD);
        }
        memory.motion_epoch = input.motion_epoch;
        memory.tick = tick;
        let mut sweeps = Vec::new();
        for (hand, sample) in input.hands.into_iter().enumerate() {
            let memory = &mut memory.hands[hand];
            let sample = sample.filter(|_| free && !(hand == 1 && flying));
            let Some(now) = sample.and_then(crate::sim::TrackedHand::pose) else {
                *memory = Default::default();
                continue;
            };
            let previous = memory.pose.and_then(crate::sim::TrackedHand::pose);
            memory.pose = sample;
            let waypoint = input.waypoints[hand].and_then(crate::sim::TrackedHand::pose);
            let min_travel = SWING_TRAVEL / if waypoint.is_some() { 2.0 } else { 1.0 };
            let edges = || {
                class
                    .blades
                    .iter()
                    .zip(&class.hand_edges)
                    .enumerate()
                    .filter(|(_, (blade, _))| {
                        blade.tracked_hand() == Some(hand) && blade.length > 0.0
                    })
            };
            let before = sweeps.len();
            let path = previous.into_iter().chain(waypoint).chain([now]);
            let steps = previous.is_some() as usize + waypoint.is_some() as usize;
            for (step, (previous, current)) in path.clone().zip(path.skip(1)).enumerate() {
                let tick_range = [step, step + 1].map(|p| p as f32 / steps as f32);
                let feet = tick_range.map(|t| state.previous_position.lerp(unit.soldier.pos, t));
                let arc_for = |local| TrackedArc {
                    previous,
                    current,
                    local,
                    feet,
                    tick_range,
                };
                let motion = |arc: TrackedArc| {
                    let from = arc.at(0.0);
                    let to = arc.at(1.0);
                    let locomotion = feet[1] - feet[0];
                    let tip = to.end - from.end - locomotion;
                    let root = to.start - from.start - locomotion;
                    if tip.length_squared() >= root.length_squared() {
                        tip
                    } else {
                        root
                    }
                };
                let mut swing_edges = edges()
                    .filter_map(|(index, (blade, &local))| {
                        let arc = arc_for(local);
                        let motion = motion(arc);
                        (motion.length_squared() >= min_travel * min_travel)
                            .then_some((index, blade, arc, motion))
                    })
                    .peekable();
                // Both ends of a double saber use the first moving edge to
                // classify the stroke and keep one shared hit cache.
                let Some(&(_, _, _, stroke_motion)) = swing_edges.peek() else {
                    continue;
                };
                let stroke_direction = stroke_motion.normalize();
                if memory.travel >= REVERSAL_TRAVEL && memory.direction.dot(stroke_direction) < 0.0
                {
                    memory.stroke = memory.stroke.wrapping_add(1);
                    memory.travel = 0.0;
                }
                // A rounded return may reverse the stroke without any two
                // consecutive samples pointing in opposite directions.
                if memory.travel == 0.0 {
                    memory.direction = stroke_direction;
                }
                memory.travel += stroke_motion.length();

                for (edge, blade, arc, motion) in swing_edges {
                    let current = arc.at(1.0);
                    sweeps.push(Sweep {
                        attack: crate::combo::AttackWindow {
                            index: edge,
                            id: 0,
                            record: tracked_record(hand, memory.stroke),
                            cache_end: f32::INFINITY,
                            edge: edge as u8,
                            damage: None,
                            length: None,
                            length_from_edge: false,
                            width: None,
                            width_from_edge: false,
                            push: 0.0,
                            play_explosion: false,
                        },
                        current,
                        previous: arc.at(0.0),
                        length: (current.end - current.start).length(),
                        width: blade.width,
                        incoming: motion.normalize(),
                        stroke_direction,
                        root_inset: 0.0,
                        arc: Some(arc),
                    });
                }
            }
            if sweeps.len() != before {
                memory.quiet = 0;
            } else if memory.quiet < QUIET_TICKS {
                memory.quiet += 1;
                if memory.quiet == QUIET_TICKS {
                    memory.stroke = memory.stroke.wrapping_add(1);
                    memory.direction = Vec3::ZERO;
                    memory.travel = 0.0;
                }
            }
        }
        state.tracked = Some(memory);
        sweeps
    }

    fn combo_sweeps(&self, i: usize) -> Vec<Sweep> {
        let mut sweeps = Vec::new();
        let u = &self.units[i];
        let class = self.weapon_classes[u.weapon_class].saber.as_ref().unwrap();
        let state = u.weapon.melee.as_ref().unwrap();
        let Some(clip) = state
            .animation
            .as_deref()
            .and_then(|name| class.clips.iter().find(|c| c.name == name))
        else {
            return sweeps;
        };
        let basis = state.blade_frame(u.soldier.pos, u.soldier.yaw, u.soldier.pitch);
        let previous_basis = state.blade_frame(
            state.previous_position,
            state.previous_yaw,
            state.previous_pitch,
        );
        let incoming = basis.z_axis.truncate();
        for attack in &state.attacks {
            let edge = usize::from(attack.edge);
            let Some(blade) = class.blades.get(edge) else {
                continue;
            };
            if blade.bone.eq_ignore_ascii_case("hp_weapons")
                && (u.weapon.saber_flight.is_some()
                    || u.held.iter().any(|w| w.state.saber_flight.is_some()))
            {
                continue;
            }

            let Some(current) = clip.sample(state.runtime.time, edge) else {
                continue;
            };
            let previous = clip
                .sample((state.runtime.time - TICK_DT).fmax(0.0), edge)
                .unwrap_or(current);
            let length = attack.length.map_or(blade.length, |n| {
                if attack.length_from_edge {
                    n * blade.length
                } else {
                    n
                }
            });
            let width = attack.width.map_or(blade.width, |n| {
                if attack.width_from_edge {
                    n * blade.width
                } else {
                    n
                }
            });
            let place = |segment: BladeSegment, basis: Mat4| {
                let direction = (segment.end - segment.start).normalize_or_zero();
                let start = basis.transform_point3(segment.start);
                let end = basis.transform_point3(segment.start + direction * length);
                (start, end)
            };
            let (start, end) = place(current, basis);
            let (old_start, old_end) = place(previous, previous_basis);
            sweeps.push(Sweep {
                attack: *attack,
                current: BladeSegment { start, end },
                previous: BladeSegment {
                    start: old_start,
                    end: old_end,
                },
                length,
                width,
                incoming,
                stroke_direction: Vec3::ZERO,
                root_inset: 0.2,
                arc: None,
            });
        }
        sweeps
    }
}
