//! The tread scroll on a hover's model (`analysis/re-hover.md` §6.18,
//! [`crate::vehicle_class::tread`]): per live hover, each tread's UV offset,
//! stepped once per sim tick from the hover's forward speed and yaw rate,
//! and the texture instance its segments draw with so the offset is that
//! vehicle's alone. Visual only: the sim never reads it.
//!
//! The drive keeps K's offsets at E+0x5a8 (u) / +0x5b8 (v), four words each
//! (B2:134070-134109), and the draw hands them to the segment whose MNAM
//! hash is K+0xfa0+4i as words 5 and 6 of its UV block, words 1-4 left at
//! identity (B2:131124-131151). The draw loads that block for the UV
//! program (B2:711303-711336); words 1-4 are the 2x2 a material's static
//! UV scale fills, 5 and 6 zero there (`FUN_0058e730`). Measured on the PS2
//! (BF2 Kashyyyk, the snail tank, a private PCSX2 on the maintainer's save
//! state 8, 2026-11-13): E+0x5a8 stays 0 and E+0x5b8 rises about 7 per
//! second while a bot reverses at vF = -8 m/s (K+0xfc0 = -0.8333 gives
//! 6.67), and falls while one drives on.
//!
//! The port draws between ticks (card RINT, `crate::render_blend`): the hull
//! is drawn at the frame's share of the way from the last tick's pose to
//! this one's, so the tread is too, at `offset - (1 - share) * step` with
//! `step` the last tick's unwrapped [`tread::delta`]. The PS2 drew once per
//! drive update and needed none; a repeating texture makes the offset's
//! wrap invisible either way.

use crate::vehicle_class::tread::{self, Tread};
use render::TextureHandle;
use std::collections::HashMap;

/// One live hover's treads.
struct Clock {
    gen: u32,
    class: u32,
    tick: u64,
    offsets: Vec<[f32; 2]>,
    steps: Vec<[f32; 2]>,
}

/// Per `Sim::vehicles` slot, its [`Clock`]; and per (slot, part, base
/// texture), the instance that part draws with.
#[derive(Default)]
pub(super) struct TreadClocks {
    clocks: HashMap<usize, Clock>,
    instances: HashMap<(usize, usize, TextureHandle), TextureHandle>,
}

impl TreadClocks {
    /// Bring every hover with treads to `sim`'s tick: one drive step of
    /// `TICK_DT` per tick since the last frame (at most 30), with the
    /// hover's velocity along its model +z (E+0x4c0 · E+0x140, the tilt
    /// included) and its body yaw rate (E+0x4d4).
    pub fn step(&mut self, sim: &core::sim::Sim, treads: &[Vec<Tread>]) {
        let tick = sim.tick;
        let mut live = Vec::new();
        for (slot, v) in sim.vehicles.iter().enumerate() {
            if !v.alive || v.kind != core::vehicle::VehicleKind::Hover {
                continue;
            }
            let Some(list) = treads.get(v.class as usize).filter(|l| !l.is_empty()) else {
                continue;
            };
            live.push(slot);
            let c = self.clocks.entry(slot).or_insert(Clock {
                gen: v.gen,
                class: v.class,
                tick,
                offsets: Vec::new(),
                steps: Vec::new(),
            });
            if c.gen != v.gen || c.class != v.class || c.offsets.len() != list.len() {
                *c = Clock {
                    gen: v.gen,
                    class: v.class,
                    tick,
                    offsets: vec![[0.0; 2]; list.len()],
                    steps: vec![[0.0; 2]; list.len()],
                };
            }
            let forward = v.vel.dot(v.rot * glam::Vec3::Z);
            let yaw = v.hover.omega.y;
            for _ in 0..tick.saturating_sub(c.tick).min(30) {
                for ((o, s), t) in c.offsets.iter_mut().zip(&mut c.steps).zip(list) {
                    *s = tread::delta(t, forward, yaw, core::TICK_DT);
                    *o = tread::step(*o, t, forward, yaw, core::TICK_DT);
                }
            }
            c.tick = tick;
        }
        self.clocks.retain(|slot, _| live.contains(slot));
    }

    /// Slot `slot`'s offset for the part whose MNAM is `group`, when its
    /// class scrolls that segment, drawn `share` of the way from the last
    /// tick's offset to this one's.
    pub fn offset(
        &self,
        slot: usize,
        treads: &[Tread],
        group: Option<&str>,
        share: f32,
    ) -> Option<[f32; 2]> {
        let group = group?;
        let i = treads
            .iter()
            .position(|t| t.segment.eq_ignore_ascii_case(group))?;
        let c = self.clocks.get(&slot)?;
        let (o, s) = (c.offsets.get(i)?, c.steps.get(i)?);
        Some([0, 1].map(|k| o[k] - (1.0 - share) * s[k]))
    }

    /// The texture slot `slot`'s part `part` draws with, `base` under
    /// params of its own, at `offset`.
    pub fn texture(
        &mut self,
        r: &mut render::Renderer,
        slot: usize,
        part: usize,
        base: TextureHandle,
        offset: [f32; 2],
    ) -> TextureHandle {
        let t = match self.instances.get(&(slot, part, base)) {
            Some(&t) => t,
            None => {
                let Some(t) = r.add_texture_instance(base) else {
                    return base;
                };
                self.instances.insert((slot, part, base), t);
                t
            }
        };
        r.set_texture_uv_offset(t, offset);
        t
    }
}
