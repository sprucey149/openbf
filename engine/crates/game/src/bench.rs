//! `--bench`: measure what a map costs, so optimisation is measured rather
//! than guessed and a regression is visible.
//!
//! Target hardware (the maintainer, 2026-08-30): 2018+ machines *including* laptop
//! integrated graphics, Intel Iris Xe (Gen12) or better, and it must stay that
//! way as lighting gets richer. So every number here is reported per ADAPTER —
//! pick one with `OPENBF_ADAPTER=igpu|dgpu|<name substring>` (see
//! `render::device::request_device`). A frame time without the adapter beside it says
//! nothing.
//!
//! What is measured, and what each number is good for:
//!
//! * **scene load** — CPU parse of the baked manifest/GLB/KTX2 into `Scene`.
//!   Bounds how long a map takes to appear.
//! * **gpu upload** — building the vertex/index/texture resources.
//!   Together with scene load this is the whole loading screen.
//! * **texture bytes** — the RGBA8 pixel payload the map hands the GPU. This
//!   is the number block compression would move, and the one most likely to
//!   hurt an iGPU sharing system memory.
//! * **sim tick** — one fixed 30 Hz `Sim::step`. The budget is 33.3 ms; the
//!   sim should be a rounding error inside it.
//! * **frame** — a full headless render of the map's draw list. Reported as
//!   mean/p50/p99 rather than an average, because a stutter lives in the tail.
//!
//! Timings use wall clock deliberately: this measures the machine, not the
//! sim, and nothing here feeds the deterministic sim state.

use std::time::Duration;

/// A set of timings in milliseconds, summarised by percentile.
#[derive(Default, Clone)]
pub struct Samples {
    ms: Vec<f64>,
}

impl Samples {
    pub fn push(&mut self, d: Duration) {
        self.ms.push(d.as_secs_f64() * 1000.0);
    }

    pub fn len(&self) -> usize {
        self.ms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ms.is_empty()
    }

    pub fn mean(&self) -> f64 {
        if self.ms.is_empty() {
            return 0.0;
        }
        self.ms.iter().sum::<f64>() / self.ms.len() as f64
    }

    /// Nearest-rank percentile (`q` in 0..=1) over a sorted copy. Nearest-rank
    /// rather than interpolated so p99 is always a sample that really happened.
    pub fn pct(&self, q: f64) -> f64 {
        if self.ms.is_empty() {
            return 0.0;
        }
        let mut v = self.ms.clone();
        v.sort_by(f64::total_cmp);
        let rank = (q * v.len() as f64).ceil().max(1.0) as usize;
        v[rank.min(v.len()) - 1]
    }

    pub fn max(&self) -> f64 {
        self.ms.iter().copied().fold(0.0, f64::max)
    }
}

/// What a loaded map is made of. Counted from the CPU-side scene, so it is the
/// same whichever adapter renders it.
#[derive(Default, Debug, Clone)]
pub struct SceneStats {
    pub meshes: usize,
    pub instances: usize,
    /// Instances that resolve to a model and are actually drawn.
    pub draws: usize,
    pub triangles: u64,
    pub textures: usize,
    /// RGBA8 bytes across mip 0 + every extra mip + detail companions.
    pub texture_bytes: u64,
    /// Of `texture_bytes`, what the `_dtl` detail companions contribute.
    pub detail_bytes: u64,
    pub terrain_triangles: u64,
    /// Draws in a real battle frame: the world list plus the sim's units.
    pub battle_draws: usize,
    /// Of those, skinned draws — each carries its own 96-joint palette.
    pub skinned_draws: usize,
}

/// One texture's RGBA8 bytes: mip 0, every extra mip, and its detail
/// companion (recursively). [`SceneStats::of`] sums this over a scene.
fn tex_bytes(t: &crate::assets::CpuTexture) -> u64 {
    let base: u64 = t.rgba.len() as u64 + t.extra_mips.iter().map(|m| m.len() as u64).sum::<u64>();
    base + t.detail.as_deref().map_or(0, tex_bytes)
}

impl SceneStats {
    pub fn of(scene: &crate::assets::Scene, draws: usize) -> SceneStats {
        let triangles: u64 = scene
            .meshes
            .values()
            .map(|m| m.indices.len() as u64 / 3)
            .sum();
        let terrain_triangles = scene
            .terrain
            .as_ref()
            .map_or(0, |t| t.indices.len() as u64 / 3);
        SceneStats {
            meshes: scene.meshes.len(),
            instances: scene.manifest.instances.len(),
            draws,
            triangles,
            textures: scene.textures.len(),
            texture_bytes: scene.textures.values().map(tex_bytes).sum(),
            detail_bytes: scene
                .textures
                .values()
                .filter_map(|t| t.detail.as_deref())
                .map(tex_bytes)
                .sum(),
            terrain_triangles,
            battle_draws: draws,
            skinned_draws: 0,
        }
    }
}

/// One map's measurements on one adapter.
pub struct Report {
    pub map: String,
    pub adapter: String,
    /// Units ALIVE at the end of the tick loop. The scaling curves are read
    /// against this, not against `--ai`: a member pool of 200 per team does
    /// not put 400 soldiers on the field at once (spawn waves and the
    /// alive-AI cap decide that), so plotting against the request would
    /// overstate what was measured.
    pub units_alive: usize,
    /// What a frame actually submits, from `Renderer::frame_stats` — the
    /// number that moves when culling or batching changes. The draw-LIST
    /// length (`stats.battle_draws`) does not.
    pub frame_stats: render::FrameStats,
    pub size: (u32, u32),
    pub scene_load: Duration,
    pub gpu_upload: Duration,
    pub stats: SceneStats,
    pub tick: Samples,
    pub frame: Samples,
    /// GPU ms per section of the frame (`OPENBF_GPU_TIMING=1`; empty off).
    pub gpu: Sections,
}

/// GPU ms per section of a frame (`Renderer::gpu_sections`), summed over the
/// timed frames: (section, total ms), in first-seen order, with one shared
/// timed-frame count.
#[derive(Default, Clone)]
pub struct Sections {
    rows: Vec<(&'static str, f64)>,
    frames: u32,
}

impl Sections {
    /// Add one timed frame's sections (an empty slice is no frame).
    pub fn add(&mut self, frame: &[(&'static str, f64)]) {
        if frame.is_empty() {
            return;
        }
        self.frames += 1;
        for &(label, ms) in frame {
            match self.rows.iter_mut().find(|r| r.0 == label) {
                Some(r) => {
                    r.1 += ms;
                }
                None => self.rows.push((label, ms)),
            }
        }
    }

    /// (section, mean ms per timed frame) - a section a frame did not draw
    /// counts 0 there - and the timed frame count.
    pub fn means(&self) -> (Vec<(&'static str, f64)>, u32) {
        let n = f64::from(self.frames.max(1));
        (
            self.rows.iter().map(|r| (r.0, r.1 / n)).collect(),
            self.frames,
        )
    }
}

/// `--bench-toggle`: one feature off and on in alternating blocks of frames in
/// the same process on the same frozen battle, so whatever else loads the machine
/// loads both sides alike. Only the feature differs between the two columns.
pub struct Toggle {
    pub name: &'static str,
    /// CPU building the frame's lists, then the whole frame, by [off, on].
    pub build: [Samples; 2],
    pub frame: [Samples; 2],
    pub gpu: [Sections; 2],
    pub draw_calls: [u32; 2],
    pub skinned: [usize; 2],
    pub shadow_triangles: [usize; 2],
}

impl Toggle {
    /// Frames per side before switching, and those dropped after each switch
    /// so one side's buffer growth is not billed to the other.
    pub const BLOCK: u32 = 20;
    pub const SETTLE: u32 = 2;

    pub fn new(name: &'static str) -> Self {
        Toggle {
            name,
            build: Default::default(),
            frame: Default::default(),
            gpu: Default::default(),
            draw_calls: [0; 2],
            skinned: [0; 2],
            shadow_triangles: [0; 2],
        }
    }

    pub fn is_on(frame: u32) -> bool {
        (frame / Self::BLOCK) % 2 == 1
    }

    pub fn settled(frame: u32) -> bool {
        frame % Self::BLOCK >= Self::SETTLE
    }

    pub fn print(&self) {
        println!("--- {} off/on, alternating ---", self.name);
        for (side, label) in ["off", "on "].iter().enumerate() {
            println!(
                "  {label}  frame p50 {:7.3} p95 {:7.3} mean {:7.3} ms ({} frames)   lists p50 {:6.3} ms   \
                 {} draw calls, {} skinned, {} shadow tris",
                self.frame[side].pct(0.50),
                self.frame[side].pct(0.95),
                self.frame[side].mean(),
                self.frame[side].len(),
                self.build[side].pct(0.50),
                self.draw_calls[side],
                self.skinned[side],
                self.shadow_triangles[side],
            );
            let (rows, frames) = self.gpu[side].means();
            if frames > 0 {
                let rows: Vec<String> = rows.iter().map(|(l, ms)| format!("{l} {ms:.3}")).collect();
                println!("       gpu ms: {}", rows.join(", "));
            }
        }
        println!(
            "  difference (on - off): frame p50 {:+.3} ms, lists p50 {:+.3} ms",
            self.frame[1].pct(0.50) - self.frame[0].pct(0.50),
            self.build[1].pct(0.50) - self.build[0].pct(0.50),
        );
    }
}

/// The fixed-tick budget one `Sim::step` has to fit inside (30 Hz, audit A5).
const TICK_BUDGET_MS: f64 = 1000.0 / 30.0;

impl Report {
    /// Human-readable block. Machine-readable is `csv_row`/`CSV_HEADER`.
    pub fn print(&self) {
        let mb = |b: u64| b as f64 / (1024.0 * 1024.0);
        println!("\n── bench: {} ──────────────────────────────", self.map);
        println!("adapter        {}", self.adapter);
        println!("target         {}x{}", self.size.0, self.size.1);
        println!(
            "scene load     {:7.1} ms   gpu upload {:7.1} ms   (= {:.1} ms to open the map)",
            self.scene_load.as_secs_f64() * 1000.0,
            self.gpu_upload.as_secs_f64() * 1000.0,
            (self.scene_load + self.gpu_upload).as_secs_f64() * 1000.0
        );
        println!(
            "geometry       {} meshes, {} draws of {} instances, {} tris (+{} terrain)",
            self.stats.meshes,
            self.stats.draws,
            self.stats.instances,
            self.stats.triangles,
            self.stats.terrain_triangles
        );
        println!(
            "battle frame   {} draws, {} of them skinned (96-joint palette each), \
             {} units alive",
            self.stats.battle_draws, self.stats.skinned_draws, self.units_alive
        );
        println!(
            "submitted      {} draw calls; frustum culled {} of {} static draws ({:.0}%)",
            self.frame_stats.draw_calls,
            self.frame_stats.culled,
            self.frame_stats.culled + self.frame_stats.submitted,
            100.0 * f64::from(self.frame_stats.culled)
                / f64::from((self.frame_stats.culled + self.frame_stats.submitted).max(1))
        );
        println!(
            "textures       {} textures, {:.1} MB RGBA8 ({:.1} MB of it detail companions)",
            self.stats.textures,
            mb(self.stats.texture_bytes),
            mb(self.stats.detail_bytes)
        );
        if !self.tick.is_empty() {
            println!(
                "sim tick       mean {:6.3} ms  p50 {:6.3}  p99 {:6.3}  max {:6.3}   ({} ticks, {:.1}% of the 33.3 ms budget)",
                self.tick.mean(),
                self.tick.pct(0.50),
                self.tick.pct(0.99),
                self.tick.max(),
                self.tick.len(),
                100.0 * self.tick.mean() / TICK_BUDGET_MS
            );
        }
        if !self.frame.is_empty() {
            println!(
                "frame          mean {:6.2} ms  p50 {:6.2}  p99 {:6.2}  max {:6.2}   ({} frames, mean {:.0} fps)",
                self.frame.mean(),
                self.frame.pct(0.50),
                self.frame.pct(0.99),
                self.frame.max(),
                self.frame.len(),
                1000.0 / self.frame.mean().max(1e-6)
            );
        }
        let (gpu, frames) = self.gpu.means();
        if frames > 0 {
            let total: f64 = gpu.iter().map(|g| g.1).sum();
            let parts: Vec<String> = gpu.iter().map(|(l, ms)| format!("{l} {ms:.2}")).collect();
            println!(
                "gpu ms         total {total:.2} = {}   ({frames} timed frames)",
                parts.join(", ")
            );
        }
    }

    pub const CSV_HEADER: &'static str =
        "map,adapter,width,height,scene_load_ms,gpu_upload_ms,meshes,draws,instances,\
         triangles,terrain_triangles,textures,texture_mb,battle_draws,skinned_draws,\
         units_alive,draw_calls,culled,tick_mean_ms,tick_p99_ms,frame_mean_ms,frame_p99_ms\n";

    pub fn csv_row(&self) -> String {
        format!(
            "{},{},{},{},{:.2},{:.2},{},{},{},{},{},{},{:.2},{},{},{},{},{},{:.4},{:.4},{:.3},{:.3}\n",
            self.map,
            self.adapter.replace(',', " "),
            self.size.0,
            self.size.1,
            self.scene_load.as_secs_f64() * 1000.0,
            self.gpu_upload.as_secs_f64() * 1000.0,
            self.stats.meshes,
            self.stats.draws,
            self.stats.instances,
            self.stats.triangles,
            self.stats.terrain_triangles,
            self.stats.textures,
            self.stats.texture_bytes as f64 / (1024.0 * 1024.0),
            self.stats.battle_draws,
            self.stats.skinned_draws,
            self.units_alive,
            self.frame_stats.draw_calls,
            self.frame_stats.culled,
            self.tick.mean(),
            self.tick.pct(0.99),
            self.frame.mean(),
            self.frame.pct(0.99),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_average_over_timed_frames() {
        let mut s = Sections::default();
        s.add(&[("sky", 1.0), ("models", 3.0)]);
        s.add(&[]);
        s.add(&[("sky", 2.0), ("hud", 4.0)]);
        let (m, n) = s.means();
        assert_eq!(n, 2);
        assert_eq!(m, vec![("sky", 1.5), ("models", 1.5), ("hud", 2.0)]);
    }

    #[test]
    fn percentiles_are_real_samples() {
        let mut s = Samples::default();
        for ms in [5.0, 1.0, 3.0, 2.0, 4.0] {
            s.push(Duration::from_secs_f64(ms / 1000.0));
        }
        assert_eq!(s.len(), 5);
        assert!((s.mean() - 3.0).abs() < 1e-9);
        // Nearest-rank: p50 of 5 samples is the 3rd, p99 is the 5th.
        assert!((s.pct(0.50) - 3.0).abs() < 1e-9);
        assert!((s.pct(0.99) - 5.0).abs() < 1e-9);
        assert!((s.max() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn empty_samples_do_not_divide_by_zero() {
        let s = Samples::default();
        assert!(s.is_empty());
        assert_eq!(s.mean(), 0.0);
        assert_eq!(s.pct(0.99), 0.0);
        assert_eq!(s.max(), 0.0);
    }

    /// A detail companion's pixels must be counted — they are real GPU bytes,
    /// and forgetting them under-reports exactly the BF1 maps that use them.
    #[test]
    fn texture_bytes_include_mips_and_detail() {
        use crate::assets::CpuTexture;
        let detail = CpuTexture {
            width: 4,
            height: 4,
            rgba: vec![0; 4 * 4 * 4],
            extra_mips: vec![],
            detail: None,
        };
        let t = CpuTexture {
            width: 2,
            height: 2,
            rgba: vec![0; 2 * 2 * 4],
            extra_mips: vec![vec![0; 4]],
            detail: Some(Box::new(detail)),
        };
        // The function `SceneStats::of` sums, not a copy of its arithmetic:
        // dropping the detail term there must fail here.
        assert_eq!(super::tex_bytes(&t), 16 + 4 + 64);
    }
}
