use super::{Listener, LoopRequest, LoopSource, SoundLib};
use crate::walker_shots::Ramp;
impl SoundLib {
    /// The seated walker and flyer guns' fire loops and spin motors
    /// ([`crate::walker_shots::voices`]), each fading at its own rate.
    pub fn walker_fire_loops(
        &mut self,
        audio: &mut audio::Audio,
        walk: &crate::walk::WalkSim,
        listener: &Listener,
    ) {
        let dt = core::TICK_DT;
        let mut live = std::collections::HashSet::new();
        for v in crate::walker_shots::voices(walk) {
            let source = if v.spin {
                LoopSource::WalkerSpin(v.unit, v.class)
            } else {
                LoopSource::WalkerFire(v.unit, v.class)
            };
            let was = self.walker_gains.get(&source).copied();
            let gain = match (v.ramp, was) {
                (Ramp::Rise(t), g) => {
                    if t > 0.0 {
                        (g.unwrap_or(0.0) + dt / t).min(1.0)
                    } else {
                        1.0
                    }
                }
                (Ramp::Hold, Some(g)) => g,
                (Ramp::Fall(t), Some(g)) if t > 0.0 => (g - dt / t).max(0.0),
                _ => 0.0,
            };
            if gain <= 0.0 && !matches!(v.ramp, Ramp::Rise(_)) {
                continue;
            }
            self.walker_gains.insert(source, gain);
            let request = LoopRequest {
                source,
                sound: crate::classdb::fnv1a_lowercase(v.name),
                pos: v.pos,
                gain,
                pitch: v.pitch,
            };
            if self.play_loop_voice(audio, &request, listener) {
                live.insert(request.source);
            }
        }
        self.walker_gains.retain(|k, _| live.contains(k));
        self.loops.retain(|k, _| {
            !matches!(k, LoopSource::WalkerFire(..) | LoopSource::WalkerSpin(..))
                || live.contains(k)
        });
    }
}
