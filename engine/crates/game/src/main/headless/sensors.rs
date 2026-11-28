//! The machine's clocks, power and temperatures alongside a bench run (`--bench-pipelined`),
//! sampled on their own thread about four times a second: each CPU's current frequency
//! (`cpufreq/scaling_cur_freq`), the AMD GPU's shader clock, power and load (its hwmon and
//! `gpu_metrics`), and the CPU's control temperature (k10temp `Tctl`). The thread that started the
//! sampler (the bench's main thread) is followed too: the CPU it last ran on and that CPU's clock,
//! since the busiest core's clock need not be its. Linux only; a source the machine does not have
//! reads `null`. Nothing here reaches the sim or the frames.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Where each reading comes from, found once.
#[derive(Default)]
struct Sources {
    /// (CPU number, its `scaling_cur_freq`).
    cpu_freq: Vec<(u32, PathBuf)>,
    /// The followed thread's `stat` (`/proc/self/task/<tid>/stat`).
    main_stat: Option<PathBuf>,
    /// The amdgpu hwmon directory (`freq1_input` sclk Hz, `power1_average` / `power1_input`
    /// µW, `temp1_input` m°C).
    gpu_hwmon: Option<PathBuf>,
    /// The card's `device` directory (`gpu_busy_percent`, `gpu_metrics`).
    gpu_device: Option<PathBuf>,
    /// k10temp's `temp1_input` (Tctl, m°C).
    tctl: Option<PathBuf>,
}

fn read_num(p: &Path) -> Option<f64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

impl Sources {
    fn find() -> Self {
        let mut s = Sources::default();
        if let Ok(dir) = std::fs::read_dir("/sys/devices/system/cpu") {
            let mut cpus: Vec<(u32, PathBuf)> = dir
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().into_string().ok()?;
                    let n: u32 = name.strip_prefix("cpu")?.parse().ok()?;
                    let f = e.path().join("cpufreq/scaling_cur_freq");
                    f.exists().then_some((n, f))
                })
                .collect();
            cpus.sort();
            s.cpu_freq = cpus;
        }
        if let Ok(dir) = std::fs::read_dir("/sys/class/hwmon") {
            for e in dir.flatten() {
                let name = std::fs::read_to_string(e.path().join("name")).unwrap_or_default();
                match name.trim() {
                    "amdgpu" if s.gpu_hwmon.is_none() => {
                        s.gpu_device = std::fs::canonicalize(e.path().join("device")).ok();
                        s.gpu_hwmon = Some(e.path());
                    }
                    "k10temp" if s.tctl.is_none() => s.tctl = Some(e.path().join("temp1_input")),
                    _ => {}
                }
            }
        }
        s
    }

    fn sample(&self, t_ms: f64) -> Value {
        let mhz: Vec<f64> = self
            .cpu_freq
            .iter()
            .filter_map(|(_, p)| read_num(p))
            .map(|khz| khz / 1000.0)
            .collect();
        let avg = (!mhz.is_empty()).then(|| mhz.iter().sum::<f64>() / mhz.len() as f64);
        let hw = self.gpu_hwmon.as_deref();
        let power_uw = hw.and_then(|h| {
            read_num(&h.join("power1_average")).or_else(|| read_num(&h.join("power1_input")))
        });
        let dev = self.gpu_device.as_deref();
        let metrics = dev.and_then(|d| std::fs::read(d.join("gpu_metrics")).ok());
        let main_cpu = self
            .main_stat
            .as_deref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|stat| last_cpu(&stat));
        let main_mhz = main_cpu.and_then(|c| {
            let (_, p) = self.cpu_freq.iter().find(|(n, _)| *n == c)?;
            read_num(p).map(|khz| khz / 1000.0)
        });
        json!({
            "t_ms": t_ms,
            "cpu_mhz_avg": avg,
            "cpu_mhz_min": mhz.iter().copied().reduce(f64::min),
            "cpu_mhz_max": mhz.iter().copied().reduce(f64::max),
            "main_cpu": main_cpu,
            "main_cpu_mhz": main_mhz,
            "gpu_sclk_mhz": hw.and_then(|h| read_num(&h.join("freq1_input"))).map(|hz| hz / 1e6),
            "gpu_power_w": power_uw.map(|uw| uw / 1e6),
            "gpu_temp_c": hw.and_then(|h| read_num(&h.join("temp1_input"))).map(|m| m / 1000.0),
            "gpu_busy_pct": dev.and_then(|d| read_num(&d.join("gpu_busy_percent"))),
            "tctl_c": self.tctl.as_deref().and_then(read_num).map(|m| m / 1000.0),
            "metrics": metrics.as_deref().and_then(gpu_metrics_v2),
        })
    }
}

/// The CPU a thread last ran on: field 39 (`processor`) of its `/proc/.../stat`, counted after
/// the command name's closing parenthesis (the name may hold spaces).
fn last_cpu(stat: &str) -> Option<u32> {
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(39 - 3)?.parse().ok()
}

/// The caller's own `/proc/self/task/<tid>/stat` (`/proc/thread-self` names the thread).
fn own_thread_stat() -> Option<PathBuf> {
    let link = std::fs::read_link("/proc/thread-self").ok()?;
    let tid = link.file_name()?.to_str()?.to_owned();
    Some(PathBuf::from("/proc/self/task").join(tid).join("stat"))
}

/// The fields this bench reads from an APU's `gpu_metrics` table, format 2 (`gpu_metrics_v2_*`
/// in the kernel's `kgd_pp_interface.h`): powers in W, clocks in MHz. Another format, or a
/// table too short, gives `None`; a field the firmware does not fill (0xffff) is `null`.
fn gpu_metrics_v2(b: &[u8]) -> Option<Value> {
    if b.len() < 112 || b[2] != 2 {
        return None;
    }
    let u16_at = |o: usize| {
        let v = u16::from_le_bytes([b[o], b[o + 1]]);
        (v != u16::MAX).then_some(f64::from(v))
    };
    let cores: Vec<Option<f64>> = (0..8).map(|k| u16_at(88 + 2 * k)).collect();
    Some(json!({
        "rev": [b[2], b[3]],
        "socket_w": u16_at(40).map(|mw| mw / 1000.0),
        "cpu_w": u16_at(42).map(|mw| mw / 1000.0),
        "soc_w": u16_at(44).map(|mw| mw / 1000.0),
        "gfx_w": u16_at(46).map(|mw| mw / 1000.0),
        "gfxclk_mhz": u16_at(64),
        "uclk_mhz": u16_at(68),
        "fclk_mhz": u16_at(70),
        "core_mhz": cores,
        "throttle": u32::from_le_bytes([b[108], b[109], b[110], b[111]]),
    }))
}

/// A sampling thread; [`Sampler::finish`] stops it and hands back its samples.
pub(super) struct Sampler {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Vec<Value>>>,
}

impl Sampler {
    /// Sample every `period` from now (the samples' `t_ms` count from `start`), following the
    /// calling thread.
    pub(super) fn start(start: Instant, period: Duration) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let main_stat = own_thread_stat();
        let thread = std::thread::Builder::new()
            .name("bench-sensors".into())
            .spawn(move || {
                let sources = Sources {
                    main_stat,
                    ..Sources::find()
                };
                let mut out = Vec::new();
                while !flag.load(Ordering::Relaxed) {
                    out.push(sources.sample(start.elapsed().as_secs_f64() * 1000.0));
                    std::thread::sleep(period);
                }
                out
            })
            .ok();
        Self { stop, thread }
    }

    pub(super) fn finish(mut self) -> Vec<Value> {
        self.stop.store(true, Ordering::Relaxed);
        self.thread
            .take()
            .and_then(|t| t.join().ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table as an APU's firmware writes it (format 2.1, read on a Radeon 780M): 6.98 W socket,
    /// 800 MHz gfx clock, 2800 MHz memory clock, a core at 4750 MHz.
    #[test]
    fn a_thread_stat_gives_its_last_cpu() {
        // A command name with spaces and a parenthesis; `processor` (field 39) is 5.
        let mut stat = String::from("4242 (bench (main) x) R");
        for f in 4..=52 {
            stat.push_str(&format!(" {}", if f == 39 { 5 } else { f }));
        }
        assert_eq!(last_cpu(&stat), Some(5));
        assert!(own_thread_stat()
            .is_none_or(|p| last_cpu(&std::fs::read_to_string(p).unwrap()).is_some()));
    }

    #[test]
    fn a_format_two_table_reads_its_power_and_clocks() {
        let mut b = vec![0u8; 120];
        b[0..4].copy_from_slice(&[0x78, 0, 2, 1]);
        b[40..42].copy_from_slice(&6981u16.to_le_bytes());
        b[42..44].copy_from_slice(&u16::MAX.to_le_bytes());
        b[64..66].copy_from_slice(&800u16.to_le_bytes());
        b[68..70].copy_from_slice(&2800u16.to_le_bytes());
        b[90..92].copy_from_slice(&4750u16.to_le_bytes());
        let m = gpu_metrics_v2(&b).unwrap();
        assert_eq!(m["socket_w"], json!(6.981));
        assert_eq!(m["cpu_w"], Value::Null);
        assert_eq!(m["gfxclk_mhz"], json!(800.0));
        assert_eq!(m["uclk_mhz"], json!(2800.0));
        assert_eq!(m["core_mhz"][1], json!(4750.0));
        b[2] = 3;
        assert!(gpu_metrics_v2(&b).is_none());
    }
}
