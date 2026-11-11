//! Replay the other brain on the pre-writeback state.

use crate::ai::{AiState, NavTick};
use crate::sim::{PadState, Sim};

pub(crate) fn verify(sim: &Sim, index: usize, state: &AiState, pad: PadState, mut tick: NavTick) {
    let unit = &sim.units[index];
    let me = sim.handle(index);
    let mut shadow = unit.ai;
    let (shadow_pad, shadow_tick) = if super::uses_legacy(sim, me) {
        super::ground::think(sim, me, &mut shadow)
    } else {
        super::legacy::think(sim, me, &mut shadow)
    };
    tick.think_ns = 0;
    let at = || (sim.tick, me);
    assert_eq!(pad, shadow_pad, "brain pad at {:?}", at());
    assert_eq!(tick, shadow_tick, "brain diagnostics at {:?}", at());
    // Legacy has no scored owner. The ground unit hash writes it separately.
    shadow.step = state.step;
    assert_eq!(*state, shadow, "brain state at {:?}", at());

    let mut live_bytes = Vec::new();
    state.hash_into(&mut |part| live_bytes.extend_from_slice(part));
    let mut offset = 0;
    shadow.hash_into(&mut |part| {
        let end = offset + part.len();
        assert_eq!(
            live_bytes.get(offset..end),
            Some(part),
            "brain hash bytes at {:?}, offset {offset}",
            at()
        );
        offset = end;
    });
    assert_eq!(live_bytes.len(), offset, "brain hash length at {:?}", at());
}

#[cfg(test)]
mod tests {
    use crate::sim::Sim;
    use crate::unit::{Controller, UnitSpec};
    use glam::Vec3;

    #[test]
    fn both_brains_match_on_the_same_tick_inputs() {
        let mut sim = Sim::new(42);
        let mut bot = UnitSpec::player(1, Vec3::new(0.0, 0.0, 15.0));
        bot.controller = Controller::Ai;
        let handle = sim.spawn(&bot).expect("spawn shadow bot");
        for _ in 0..30 {
            sim.step(&[]);
        }
        assert!(sim.unit(handle).is_some_and(|unit| unit.alive));
    }
}
