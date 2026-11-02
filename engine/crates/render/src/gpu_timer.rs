//! GPU time per section of a frame (card ISPD): a measurement knob, off by
//! default. `OPENBF_GPU_TIMING=1` asks the device for timestamp queries; when
//! the adapter has them, `render_frame` writes a timestamp after each section
//! (the sky, each draw layer's runs, the soldiers, the grass, the blend pass,
//! the bolts and particles, BF2's terrain light pass, the frame filter, the
//! HUD) and the section's time is the gap from the timestamp before it.
//!
//! The readback is asynchronous: a ring of [`RING`] buffers, each mapped once
//! the GPU is done with it, read by [`GpuTimer::collect`] at the start of a
//! later frame. So [`GpuTimer::last`] is a frame or two old, and a frame whose
//! ring slot is still mapping is not timed at all. A section that recurs in a
//! frame (the terrain light pass splits the map pass in two) is summed under
//! its one label.
//!
//! Timestamps inside a render pass are what the GPU reports when the command
//! before them retires; on an immediate-mode GPU (desktop, Intel's iGPUs) the
//! split is good to a fraction of a millisecond, on a tiler it can blur. The
//! frame's total (first to last timestamp) is exact either way.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Timestamps one frame may write; marks past it are dropped (their time
/// falls into the next section that gets one).
const CAP: u32 = 256;
/// Readback buffers in flight.
const RING: usize = 3;

const IDLE: u8 = 0;
const MAPPING: u8 = 1;
const MAPPED: u8 = 2;

/// The features `OPENBF_GPU_TIMING=1` asks for, where the adapter has all of
/// them; else none (and the timer stays off).
pub(crate) fn wanted_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    let want = wgpu::Features::TIMESTAMP_QUERY
        | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
        | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES;
    if std::env::var("OPENBF_GPU_TIMING").as_deref() == Ok("1") {
        if adapter.features().contains(want) {
            return want;
        }
        eprintln!(
            "OPENBF_GPU_TIMING: adapter {:?} has no timestamp queries inside passes; GPU timing off",
            adapter.get_info().name
        );
    }
    wgpu::Features::empty()
}

struct Slot {
    buf: wgpu::Buffer,
    labels: Vec<&'static str>,
    state: Arc<AtomicU8>,
}

pub(crate) struct GpuTimer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    ring: Vec<Slot>,
    next: usize,
    /// The labels of this frame's timestamps so far; the first is the start.
    labels: Vec<&'static str>,
    /// The slot this frame resolves into, when one was free at `begin`.
    pending: Option<usize>,
    period_ns: f64,
    last: Vec<(&'static str, f64)>,
}

impl GpuTimer {
    /// A timer when the device was created with [`wanted_features`].
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES)
        {
            return None;
        }
        let bytes = u64::from(CAP) * 8;
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("gpu timer"),
            ty: wgpu::QueryType::Timestamp,
            count: CAP,
        });
        let resolve = crate::resources::buffer(
            device,
            "gpu timer resolve",
            bytes,
            wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        );
        let ring = (0..RING)
            .map(|_| Slot {
                buf: crate::resources::buffer(
                    device,
                    "gpu timer readback",
                    bytes,
                    wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                ),
                labels: Vec::new(),
                state: Arc::new(AtomicU8::new(IDLE)),
            })
            .collect();
        Some(Self {
            set,
            resolve,
            ring,
            next: 0,
            labels: Vec::new(),
            pending: None,
            period_ns: f64::from(queue.get_timestamp_period()),
            last: Vec::new(),
        })
    }

    /// Read every slot the GPU has finished, newest last, then start this
    /// frame: its first timestamp on `encoder`.
    pub(crate) fn begin(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
        self.collect(device);
        self.labels.clear();
        self.pending =
            (self.ring[self.next].state.load(Ordering::Acquire) == IDLE).then_some(self.next);
        if self.pending.is_some() {
            encoder.write_timestamp(&self.set, 0);
            self.labels.push("start");
        }
    }

    /// Read every slot the GPU has finished, newest last.
    pub(crate) fn collect(&mut self, device: &wgpu::Device) {
        let _ = device.poll(wgpu::PollType::Poll);
        for k in 0..RING {
            let i = (self.next + k) % RING;
            if self.ring[i].state.load(Ordering::Acquire) == MAPPED {
                self.read(i);
            }
        }
    }

    /// The end of section `label`, inside a render pass.
    pub(crate) fn mark_pass(&mut self, pass: &mut wgpu::RenderPass<'_>, label: &'static str) {
        if self.pending.is_some() && (self.labels.len() as u32) < CAP {
            pass.write_timestamp(&self.set, self.labels.len() as u32);
            self.labels.push(label);
        }
    }

    /// The end of section `label`, between passes.
    pub(crate) fn mark(&mut self, encoder: &mut wgpu::CommandEncoder, label: &'static str) {
        if self.pending.is_some() && (self.labels.len() as u32) < CAP {
            encoder.write_timestamp(&self.set, self.labels.len() as u32);
            self.labels.push(label);
        }
    }

    /// Resolve this frame's timestamps into its ring slot (before submit).
    pub(crate) fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let Some(i) = self.pending else { return };
        let n = self.labels.len() as u32;
        encoder.resolve_query_set(&self.set, 0..n, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.ring[i].buf, 0, u64::from(n) * 8);
        self.ring[i].labels = std::mem::take(&mut self.labels);
    }

    /// Map this frame's slot (after submit).
    pub(crate) fn submitted(&mut self) {
        let Some(i) = self.pending.take() else { return };
        let state = self.ring[i].state.clone();
        state.store(MAPPING, Ordering::Release);
        self.ring[i]
            .buf
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                state.store(if r.is_ok() { MAPPED } else { IDLE }, Ordering::Release);
            });
        self.next = (i + 1) % RING;
    }

    fn read(&mut self, i: usize) {
        let slot = &self.ring[i];
        let n = slot.labels.len();
        let ticks: Vec<u64> = {
            let view = slot.buf.slice(..(n as u64 * 8).max(8)).get_mapped_range();
            view.as_chunks::<8>()
                .0
                .iter()
                .take(n)
                .map(|b| u64::from_le_bytes(*b))
                .collect()
        };
        slot.buf.unmap();
        slot.state.store(IDLE, Ordering::Release);
        let mut out: Vec<(&'static str, f64)> = Vec::new();
        for k in 1..n {
            let ms = ticks[k].saturating_sub(ticks[k - 1]) as f64 * self.period_ns / 1.0e6;
            match out.iter_mut().find(|(l, _)| *l == slot.labels[k]) {
                Some((_, t)) => *t += ms,
                None => out.push((slot.labels[k], ms)),
            }
        }
        self.last = out;
    }

    /// The newest timed frame: (section, GPU ms), in drawing order.
    pub(crate) fn last(&self) -> &[(&'static str, f64)] {
        &self.last
    }
}
