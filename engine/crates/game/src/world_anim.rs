//! World animation groups, as the campaign scripts drive them
//! (`PlayAnimation`, `PauseAnimation`, `RewindAnimation`,
//! `PlayAnimationFromTo`, `SetAnimationStartPoint`). The data is the map's
//! `manifest::world_anim::FILE`.
//!
//! BF2, `SWBFRNT2.ELF-decomp.c`:
//! - **Tables.** Animations by name (0x691918, FUN_00272ef8), groups by name
//!   (0x6979c8, FUN_00274280). A name already taken keeps the first.
//! - **Entries.** 0x60 bytes at DAT_00691a20 (FUN_00274008): its time, its
//!   object, and a base matrix, identity until the object of that name is
//!   placed, which links it and copies its matrix (FUN_00274738).
//! - **Bindings.** `PlayAnimation` (FUN_00190dc8) sets the group playing,
//!   `PauseAnimation` (FUN_00190e28) clears it, `RewindAnimation`
//!   (FUN_00190e88) sets every entry's time to 0 and the end time to -1,
//!   `PlayAnimationFromTo` (FUN_00190f68) pauses, sets every time to `from`
//!   and the end to `to`, then plays (only for 0 <= from < to), and
//!   `SetAnimationStartPoint` (FUN_00190f08) copies each object's matrix into
//!   its entry's base (FUN_002741b8).
//! - **Each frame** (FUN_00274c18): a playing group advances each linked
//!   entry by dt; with an end time it advances only up to it, then stops.
//! - **Evaluation** (FUN_002736e0, an animation with its local byte clear):
//!   time += dt, clamped to the duration, or wrapped when it loops. The
//!   matrix is the base; rotation keys give angles (radians) in x, y, z,
//!   built as identity × Rx × Ry × Rz (FUN_0053ad40, FUN_0053adf0,
//!   FUN_0053ae90) and put before the base; position keys put the
//!   translation at the base's origin plus p along the base's own axes.
//!   Row-vector on the PS2, so here `base * Rz * Ry * Rx` and
//!   `base.transform_point3(p)`.
//! - **Keys** (FUN_00273298): the last key at or before t and the next one;
//!   past the last, the last and (looping) the first, or (not) itself; past
//!   the duration when not looping, the last twice; before the first, none,
//!   and that channel keeps the base.
//! - **Interpolation** (FUN_00273530), by the first key's byte: 0 holds it,
//!   1 is linear over the gap (the gap to the duration when the next key is
//!   not later, 1 s when neither), 2 is a cubic Hermite in the seconds since
//!   the first key, with both tangents stored on it (FUN_00539d78).
//!   PROVISIONAL: the cubic's span is the gap; the float FUN_00539d50
//!   passes on is not visible in the listing, and the gap is the one
//!   value that makes the curve reach the next key.
//!
//! The port moves only what it can place: an entry whose object is a sim
//! `Station` ([`core::sim::Sim::set_station_transform`]), which the viewer
//! draws and collides from its pose. trace: an entry on any other object
//! (Mygeeto's gunships and dropship `repveh4`..`6`) is left static.
//! trace: an animation with its local byte set (FUN_002736e0's other
//! branch, Mygeeto's `gunshipfly`) steps along its own turning axes and is
//! not ported. trace: the group flag that re-takes the base when the object
//! reports a state (group+0xe bit 0), and the hierarchy pass that carries
//! child objects (DAT_008252c0), are not ported. PROVISIONAL: the sign of
//! each axis rotation is glam's; FUN_0053ad40's lane order is not read.

use core::fmath::FloatExt;
use glam::{Mat4, Vec3};
use manifest::world_anim::{WorldAnim, WorldAnimKey, WorldAnims};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct Entry {
    anim: Option<usize>,
    station: Option<usize>,
    vehicle_pad: Option<usize>,
    /// The pad's vehicle this entry is linked to. The BF2 pad spawn step
    /// links the entry of the pad's name to each vehicle it creates and copies
    /// that vehicle's matrix into `base` after its constructor has added the
    /// flyer's LandedHeight (SWBFRNT2.ELF FUN_003725f0 lines 358123-358128,
    /// analysis/re-vehicle-rest.md §2).
    linked: Option<core::vehicle::VehicleHandle>,
    instance: Option<u32>,
    time: f32,
    base: Mat4,
    /// Accumulated world position for `local == true` animations
    /// (SWBFRNT2.ELF FUN_002736e0 lines 193587-193768), reset to `base`'s
    /// translation on rewind or loop wrap.
    cur_pos: Vec3,
    parts: Option<(bool, core::sim::StationParts)>,
}

#[derive(Clone, Debug)]
struct Group {
    name: String,
    playing: bool,
    end: f32,
    entries: Vec<Entry>,
}

#[derive(Clone, Debug, Default)]
pub struct WorldAnimPlayer {
    anims: Vec<WorldAnim>,
    groups: Vec<Group>,
    /// Posed world instances by manifest index, written each `step` for
    /// entries whose object is a placed prop (Coruscant's sliding library
    /// doors, Mustafar's lava bridge) rather than a sim Station. The viewer
    /// poses these draws.
    instance_poses: BTreeMap<u32, Mat4>,
    /// Each animated world instance's collision bodies in its rest-base space,
    /// captured once at load before any group moves it. Shared across groups
    /// that drive the same instance (e.g. `lava_bridge_drop` and
    /// `lava_bridge_raise` on Mustafar's `lavatrap`) so a second group does
    /// not bake in the first group's displacement.
    instance_parts: BTreeMap<u32, Vec<(usize, Mat4)>>,
}

fn bracket(a: &WorldAnim, keys: &[WorldAnimKey], t: f32) -> Option<(WorldAnimKey, WorldAnimKey)> {
    let last = *keys.last()?;
    if !a.looping && a.duration < t {
        return Some((last, last));
    }
    for (i, k) in keys.iter().enumerate() {
        if k.time > t {
            continue;
        }
        match keys.get(i + 1) {
            None => return Some((*k, if a.looping { keys[0] } else { *k })),
            Some(next) if t < next.time => return Some((*k, *next)),
            Some(_) => {}
        }
    }
    None
}

fn sample(a: &WorldAnim, keys: &[WorldAnimKey], t: f32) -> Option<Vec3> {
    let (k0, k1) = bracket(a, keys, t)?;
    let u = t - k0.time;
    let gap = if k0.time < k1.time {
        k1.time - k0.time
    } else if k0.time < a.duration {
        a.duration - k0.time
    } else {
        1.0
    };
    let (p0, p1) = (Vec3::from(k0.value), Vec3::from(k1.value));
    Some(match k0.interp {
        1 => p1 * (u / gap) + p0 * (1.0 - u / gap),
        2 => {
            let (m0, m1) = (Vec3::from(k0.tangent_out), Vec3::from(k0.tangent_in));
            let c3 = ((p0 - p1) * 2.0 + (m0 + m1) * gap) / (gap * gap * gap);
            let c2 = ((p1 - p0) * 3.0 - (m0 * 2.0 + m1) * gap) / (gap * gap);
            ((c3 * u + c2) * u + m0) * u + p0
        }
        _ => p0,
    })
}

/// Advance `time` by `dt` and give the matrix (FUN_002736e0).
/// Supports both global (`local == false`) and local-axis (`local == true`,
/// lines 193587-193768) world animations.
fn evaluate(a: &WorldAnim, time: &mut f32, cur_pos: &mut Vec3, dt: f32, base: Mat4) -> Mat4 {
    if !a.local {
        *time += dt;
        if !a.looping {
            *time = time.fmin(a.duration);
        } else if a.duration > 0.0 {
            while *time > a.duration {
                *time -= a.duration;
            }
        }
        let mut out = base;
        if let Some(r) = sample(a, &a.rot, *time) {
            out = base
                * Mat4::from_rotation_z(r.z)
                * Mat4::from_rotation_y(r.y)
                * Mat4::from_rotation_x(r.x);
        }
        if let Some(p) = sample(a, &a.pos, *time) {
            out.w_axis = base.transform_point3(p).extend(1.0);
        }
        *cur_pos = out.w_axis.truncate();
        return out;
    }

    // `local == true` branch (FUN_002736e0 lines 193587-193768):
    // integrate position delta along the animation's current rotated axes.
    let mut prev_pos = sample(a, &a.pos, *time).unwrap_or(Vec3::ZERO);
    let old_time = *time;
    *time += dt;
    if !a.looping {
        *time = time.fmin(a.duration);
    } else if a.duration > 0.0 && *time > a.duration {
        while *time > a.duration {
            *time -= a.duration;
        }
        if old_time <= a.duration {
            *cur_pos = base.w_axis.truncate();
            prev_pos = Vec3::ZERO;
        }
    }
    let mut out = base;
    if let Some(r) = sample(a, &a.rot, *time) {
        out = base
            * Mat4::from_rotation_z(r.z)
            * Mat4::from_rotation_y(r.y)
            * Mat4::from_rotation_x(r.x);
    }
    let new_pos = sample(a, &a.pos, *time).unwrap_or(Vec3::ZERO);
    let dp = new_pos - prev_pos;
    *cur_pos += out.x_axis.truncate() * dp.x
        + out.y_axis.truncate() * dp.y
        + out.z_axis.truncate() * dp.z;
    out.w_axis = cur_pos.extend(1.0);
    out
}

impl WorldAnimPlayer {
    /// The map's world animations in `dir`, without those of a world in a
    /// layer the round left unmerged (`unloaded`). Each entry links to the
    /// station of its object's name first (`station_names`, lowercase), else
    /// to a placed world instance (`instance_names` / `instance_mats`, also
    /// lowercase and index-aligned) so prop animations such as Coruscant's
    /// library doors move their draws.
    pub fn load(
        dir: &std::path::Path,
        unloaded: &[String],
        station_names: &[String],
        instance_names: &[String],
        instance_mats: &[Mat4],
        sim: &core::sim::Sim,
    ) -> Self {
        let path = dir.join(manifest::world_anim::FILE);
        let Some(all) = pak::read_to_string(&path)
            .ok()
            .and_then(|t| ron::from_str::<WorldAnims>(&t).ok())
        else {
            return Self::default();
        };
        let loaded = |w: &str| !unloaded.iter().any(|u| u.eq_ignore_ascii_case(w));
        let mut anims: Vec<WorldAnim> = Vec::new();
        for a in all.anims.into_iter().filter(|a| loaded(&a.world)) {
            if !anims.iter().any(|b| b.name.eq_ignore_ascii_case(&a.name)) {
                anims.push(a);
            }
        }
        let mut groups: Vec<Group> = Vec::new();
        let mut instance_parts: BTreeMap<u32, Vec<(usize, Mat4)>> = BTreeMap::new();
        for g in all.groups.into_iter().filter(|g| loaded(&g.world)) {
            if groups.iter().any(|h| h.name.eq_ignore_ascii_case(&g.name)) {
                continue;
            }
            let entries = g
                .entries
                .iter()
                .map(|(anim, object)| {
                    let object = object.to_ascii_lowercase();
                    let station = station_names.iter().position(|n| *n == object);
                    let vehicle_pad = if station.is_some() {
                        None
                    } else {
                        sim.vehicle_pads
                            .iter()
                            .position(|p| p.def.name.eq_ignore_ascii_case(&object))
                    };
                    let instance_idx = instance_names
                        .iter()
                        .position(|n| *n == object)
                        .map(|i| i as u32);
                    let instance = if station.is_some() || vehicle_pad.is_some() {
                        None
                    } else {
                        instance_idx
                    };
                    let base = station
                        .and_then(|s| sim.stations.get(s))
                        .map(|s| s.transform)
                        .or_else(|| {
                            vehicle_pad.map(|pi| {
                                instance_idx
                                    .and_then(|i| instance_mats.get(i as usize).copied())
                                    .unwrap_or_else(|| {
                                        let def = &sim.vehicle_pads[pi].def;
                                        Mat4::from_rotation_translation(
                                            glam::Quat::from_rotation_y(def.yaw),
                                            def.pos,
                                        )
                                    })
                            })
                        })
                        .or_else(|| instance.and_then(|i| instance_mats.get(i as usize).copied()))
                        .unwrap_or(Mat4::IDENTITY);
                    if let Some(ii) = instance {
                        instance_parts.entry(ii).or_insert_with(|| {
                            let inverse = base.inverse();
                            sim.collision
                                .instance_bodies(ii)
                                .into_iter()
                                .map(|(i, to_world)| (i, inverse * to_world))
                                .collect()
                        });
                    }
                    Entry {
                        anim: anims.iter().position(|a| a.name.eq_ignore_ascii_case(anim)),
                        base,
                        cur_pos: base.w_axis.truncate(),
                        station,
                        vehicle_pad,
                        linked: None,
                        instance,
                        time: 0.0,
                        parts: None,
                    }
                })
                .collect();
            groups.push(Group {
                name: g.name.to_ascii_lowercase(),
                playing: g.play,
                end: -1.0,
                entries,
            });
        }
        WorldAnimPlayer {
            anims,
            groups,
            instance_poses: BTreeMap::new(),
            instance_parts,
        }
    }

    fn group(&mut self, name: &str) -> Option<&mut Group> {
        let name = name.to_ascii_lowercase();
        self.groups.iter_mut().find(|g| g.name == name)
    }

    pub fn play(&mut self, name: &str) {
        if let Some(g) = self.group(name) {
            g.playing = true;
        }
    }

    pub fn pause(&mut self, name: &str) {
        if let Some(g) = self.group(name) {
            g.playing = false;
        }
    }

    pub fn rewind(&mut self, name: &str) {
        if let Some(g) = self.group(name) {
            for e in &mut g.entries {
                e.time = 0.0;
                e.cur_pos = e.base.w_axis.truncate();
            }
            g.end = -1.0;
        }
    }

    pub fn play_from_to(&mut self, name: &str, from: f32, to: f32) {
        if !(from < to && from >= 0.0 && to >= 0.0) {
            return;
        }
        if let Some(g) = self.group(name) {
            g.entries.iter_mut().for_each(|e| e.time = from);
            g.end = to;
            g.playing = true;
        }
    }

    pub fn set_start_point(&mut self, name: &str, sim: &core::sim::Sim) {
        if let Some(g) = self.group(name) {
            for e in &mut g.entries {
                if let Some(s) = e.station.and_then(|s| sim.stations.get(s)) {
                    e.base = s.transform;
                } else if let Some(v) = e.linked.and_then(|h| sim.vehicle(h)) {
                    e.base = Mat4::from_rotation_translation(v.rot, v.pos);
                }
            }
        }
    }

    /// One tick of every playing group (FUN_00274c18).
    pub fn step(&mut self, sim: &mut core::sim::Sim, dt: f32) {
        self.link_pad_vehicles(sim);
        let mut poses = BTreeMap::new();
        for g in &mut self.groups {
            let Group {
                playing,
                end,
                entries,
                ..
            } = g;
            if !*playing {
                continue;
            }
            for e in entries {
                let Some(ai) = e.anim else {
                    continue;
                };
                if e.station.is_none() && e.linked.is_none() && e.instance.is_none() {
                    continue;
                }
                let a = &self.anims[ai];
                let step = if *end <= 0.0 {
                    dt
                } else if *end <= e.time {
                    *playing = false;
                    continue;
                } else if e.time + dt <= *end {
                    dt
                } else {
                    *playing = false;
                    *end - e.time
                };
                let m = evaluate(a, &mut e.time, &mut e.cur_pos, step, e.base);
                if let Some(si) = e.station {
                    let destroyed = sim.stations[si].destroyed;
                    if e.parts.as_ref().is_none_or(|(d, _)| *d != destroyed) {
                        e.parts = Some((destroyed, sim.station_parts(si)));
                    }
                    if let Some((_, parts)) = &e.parts {
                        sim.set_station_transform(si, m, parts);
                    }
                } else if let Some(h) = e.linked {
                    sim.set_vehicle_world_pose(h, m, step);
                } else if let Some(ii) = e.instance {
                    if let Some(parts) = self.instance_parts.get(&ii) {
                        for &(i, local) in parts {
                            sim.collision.move_body(i, m * local);
                        }
                    }
                    poses.insert(ii, m);
                }
            }
        }
        self.instance_poses.extend(poses);
    }

    /// Link every pad entry, playing or not, to its pad's newest vehicle once
    /// that vehicle is not the one already linked, and take its matrix as the
    /// base (FUN_003725f0 lines 358123-358128 → FUN_00274738). The pad
    /// creates the vehicle at its spawn point plus the flyer's LandedHeight
    /// before the link, so a landing animation such as Death Star's
    /// `incoming03` sets the trooper transport down on its gear, not sunk by
    /// that height. trace: the PS2 names a pad's second live vehicle with a
    /// number suffix (lines 358046-358054), so only the first carries the
    /// pad's name; the port links the newest one once the linked one is gone.
    fn link_pad_vehicles(&mut self, sim: &core::sim::Sim) {
        for e in self.groups.iter_mut().flat_map(|g| g.entries.iter_mut()) {
            let Some(pi) = e.vehicle_pad else {
                continue;
            };
            if e.linked.is_some_and(|h| sim.vehicle(h).is_some()) {
                continue;
            }
            let Some(h) = sim
                .vehicle_pads
                .get(pi)
                .and_then(|pad| pad.vehicles.last())
                .map(|pv| pv.handle)
                .filter(|h| Some(*h) != e.linked)
            else {
                continue;
            };
            let Some(v) = sim.vehicle(h) else {
                continue;
            };
            e.base = Mat4::from_rotation_translation(v.rot, v.pos);
            e.cur_pos = v.pos;
            e.linked = Some(h);
        }
    }

    /// The current pose of each animated world instance (manifest index).
    pub fn instance_poses(&self) -> &BTreeMap<u32, Mat4> {
        &self.instance_poses
    }

    /// Manifest instance indices driven by any loaded non-local world animation.
    pub fn animated_instances(&self) -> impl Iterator<Item = u32> + '_ {
        self.groups.iter().flat_map(|g| {
            g.entries.iter().filter_map(|e| {
                let anim = e.anim.and_then(|i| self.anims.get(i))?;
                (!anim.local).then_some(e.instance).flatten()
            })
        })
    }

    /// The station each entry of `group` moves.
    #[cfg(test)]
    pub(crate) fn stations(&self, group: &str) -> Vec<Option<usize>> {
        let group = group.to_ascii_lowercase();
        self.groups
            .iter()
            .filter(|g| g.name == group)
            .flat_map(|g| g.entries.iter().map(|e| e.station))
            .collect()
    }
}
