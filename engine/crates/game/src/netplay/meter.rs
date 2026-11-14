//! The net meter (LAN §9): one frame's network figures, as the in-battle
//! overlay draws them (F7 or Options → Network meter) and the black box
//! records them with every snapshot. Presentation only: nothing here is
//! read by the sim or enters `Sim::hash`.

use serde::{Deserialize, Serialize};

/// Another player's link, as the host sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeterPeer {
    pub slot: u8,
    pub name: String,
    pub ping_ms: f32,
    /// Its input queue at the last close and its starvations.
    pub depth: u8,
    pub starved: u32,
    /// Silent for 1.5 s or more.
    pub trouble: bool,
}

/// One frame's figures.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NetMeter {
    pub host: bool,
    pub slot: u8,
    /// Round trip to the host (0 on the host).
    pub ping_ms: f32,
    pub ping_var_ms: f32,
    /// Local inputs sent and not yet stepped: the input delay.
    pub delay_ticks: u32,
    pub delay_ms: f32,
    /// The host's queue for this player at its last close, starvations, and
    /// how early the latest input arrived (ms).
    pub queue_depth: u8,
    pub starved: u32,
    pub slack_ms: Option<f32>,
    /// The input clock's dilation (fraction).
    pub dilation: f32,
    /// Bundles waiting, the jitter buffer's target and level (ticks), the
    /// measured arrival jitter (ticks).
    pub buffered: u32,
    pub buffer_target: f32,
    pub buffer_level: f32,
    pub jitter_ticks: f32,
    /// Packet loss (%), resends, bytes per second each way.
    pub loss_pct: f32,
    pub resends: u64,
    pub bytes_in_s: f32,
    pub bytes_out_s: f32,
    /// The last hash check: the tick reported, seconds since, and whether a
    /// desync was reported since the round began.
    pub hash_tick: Option<u32>,
    pub hash_age_s: f32,
    pub desyncs: u32,
    /// Prediction: mode, ticks predicted, its CPU ms, corrections.
    pub predict_mode: crate::netplay::PredictMode,
    pub predict_depth: u32,
    pub predict_ms: f32,
    pub corrections: u32,
    /// The confirmed tick and the lockstep mode.
    pub tick: u32,
    pub mode: String,
    /// The host's view of every guest.
    pub peers: Vec<MeterPeer>,
}

impl NetMeter {
    /// The overlay's lines.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.push(format!(
            "{} slot {}  tick {}  {}",
            if self.host { "HOST" } else { "GUEST" },
            self.slot,
            self.tick,
            self.mode
        ));
        if !self.host {
            out.push(format!(
                "ping {:.0} ms (±{:.0})",
                self.ping_ms, self.ping_var_ms
            ));
        }
        out.push(format!(
            "input delay {} ticks ({:.0} ms)  queue {} starved {}{}",
            self.delay_ticks,
            self.delay_ms,
            self.queue_depth,
            self.starved,
            self.slack_ms
                .map_or(String::new(), |s| format!("  slack {s:.1} ms")),
        ));
        if !self.host {
            out.push(format!(
                "buffer {} (target {:.1}, level {:.1})  jitter {:.2} t",
                self.buffered, self.buffer_target, self.buffer_level, self.jitter_ticks
            ));
            out.push(format!(
                "loss {:.1}%  resends {}  in {:.1} KB/s  out {:.1} KB/s",
                self.loss_pct,
                self.resends,
                self.bytes_in_s / 1024.0,
                self.bytes_out_s / 1024.0
            ));
        } else {
            out.push(format!(
                "in {:.1} KB/s  out {:.1} KB/s",
                self.bytes_in_s / 1024.0,
                self.bytes_out_s / 1024.0
            ));
        }
        out.push(match self.hash_tick {
            Some(t) if self.desyncs == 0 => {
                format!("hash check OK at tick {t} ({:.1} s ago)", self.hash_age_s)
            }
            Some(t) => format!(
                "hash check: {} DESYNC(S), last report tick {t}",
                self.desyncs
            ),
            None => "hash check: none yet".into(),
        });
        out.push(format!(
            "prediction {:?} depth {}  {:.2} ms  corrections {}",
            self.predict_mode, self.predict_depth, self.predict_ms, self.corrections
        ));
        for p in &self.peers {
            out.push(format!(
                "  {:>2} {:<16} {:>4.0} ms  q {} starved {}{}",
                p.slot,
                p.name,
                p.ping_ms,
                p.depth,
                p.starved,
                if p.trouble { "  (trouble)" } else { "" }
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The meter survives the black box's JSON and lists every guest.
    #[test]
    fn net_meter_lines_and_round_trip() {
        let m = NetMeter {
            host: true,
            hash_tick: Some(90),
            peers: vec![MeterPeer {
                slot: 3,
                name: "Guest".into(),
                ping_ms: 41.0,
                ..MeterPeer::default()
            }],
            ..NetMeter::default()
        };
        let lines = m.lines();
        assert!(lines
            .iter()
            .any(|l| l.contains("Guest") && l.contains("41 ms")));
        assert!(lines.iter().any(|l| l.contains("hash check OK at tick 90")));
        let back: NetMeter = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back, m);
    }
}
