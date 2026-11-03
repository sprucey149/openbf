//! Deterministic paths for EMITTER child particles (`analysis/re-fx.md` §13).
//! Sampling, sibling offsets and parent bounds reproduce the authored PS2
//! emitter traversal; these particles affect drawing, never simulation state.

use super::{Era, Frame, FxAssets, Lcg, LiveEffect};
use glam::{Mat3, Vec3};
use render::ParticleDraw;

const PARENTS: u32 = 31;
const STEP: f32 = 0.3;
const NODE_SPACING: f32 = 0.1;
const MERGED_SPAN_BIAS: f32 = 0.0001;
const SIBLING_OFFSET: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Node {
    pub time: f32,
    pub span: f32,
    pub pos: Vec3,
    pub vel: Vec3,
}

pub(super) fn path(age: f32, mut at: impl FnMut(f32) -> Option<Vec3>) -> Option<(Vec<Node>, f32)> {
    if age.is_nan() || age < 0.0 {
        return None;
    }
    let mut nodes = vec![Node {
        time: 0.0,
        span: 0.0,
        pos: at(0.0)?,
        vel: Vec3::ZERO,
    }];
    let (mut sample, mut step, mut now, mut timer) = (0.0f32, STEP, 0.0f32, 0.0f32);
    while sample != age {
        sample += STEP;
        if age < sample {
            step -= sample - age;
            sample = age;
        }
        let dt = step.min(sample);
        now += dt;
        let Some(pos) = at(sample) else { break };
        timer -= dt;
        let n = nodes.len();
        if n == 1 || timer < 0.0 {
            let last = &mut nodes[n - 1];
            let span = now - last.time;
            last.span = span;
            last.vel = if span > 0.0 {
                (pos - last.pos) / span
            } else {
                Vec3::ZERO
            };
            nodes.push(Node {
                time: now,
                span: 0.0,
                pos,
                vel: Vec3::ZERO,
            });
            timer = NODE_SPACING;
        } else {
            let moved = now - nodes[n - 1].time;
            nodes[n - 1].time = now;
            nodes[n - 1].pos = pos;
            let older = &mut nodes[n - 2];
            older.span += moved;
            older.vel = (pos - older.pos) / (older.span + MERGED_SPAN_BIAS);
        }
    }
    Some((nodes, now))
}

pub(super) fn frame_at(nodes: &[Node], time: f32) -> Frame {
    let mut local = time;
    let mut k = 0;
    while k + 1 < nodes.len() && nodes[k].span < local {
        local -= nodes[k].span;
        k += 1;
    }
    Frame {
        rot: Mat3::IDENTITY,
        pos: nodes[k].pos + nodes[k].vel * local,
        vel: nodes[k].vel,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    effect: &LiveEffect,
    index: usize,
    era: Era,
    eye: Vec3,
    forward: Vec3,
    zoom: f32,
    assets: &mut dyn FxAssets,
    out: &mut Vec<ParticleDraw>,
) {
    let Some(def) = &effect.def.children[index] else {
        return;
    };
    let inst = &effect.instances[index];
    let em = &effect.def.emitters[index];
    let cache = effect.def.cache(index);
    let parents = inst.particles(em, era, effect.t, effect.frame, cache, |_| true);
    for parent in parents.iter().filter(|p| p.idx < PARENTS) {
        let Some((nodes, now)) = path(parent.age, |age| {
            inst.particle_at(em, era, parent.idx, age, effect.frame, cache)
                .map(|p| p.pos)
        }) else {
            continue;
        };
        let offset = inst.child_offset(parent.idx);
        let mut child = LiveEffect::new(def.clone(), frame_at(&nodes, 0.0), &mut Lcg(offset));
        child.t = now;
        child.frame = frame_at(&nodes, now);
        for (k, (child_inst, child_em)) in child
            .instances
            .iter_mut()
            .zip(&child.def.emitters)
            .enumerate()
        {
            child_inst.set_offset(offset.wrapping_add(SIBLING_OFFSET * k as u32));
            child_inst.update_path(child_em, now, false, |time| frame_at(&nodes, time));
            child_inst.rgba = parent.rgba;
        }
        child.draws_with(eye, forward, zoom, assets, out);
    }
}
