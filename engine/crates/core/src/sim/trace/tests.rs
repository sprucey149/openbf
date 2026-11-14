use super::*;
use crate::sim::{buttons, InputLog, PadState};
use crate::unit::UnitSpec;
use glam::Vec3;

fn fixture() -> Sim {
    let mut sim = Sim::new(42);
    sim.heightfield = Some(crate::sim::tests::flat_ground());
    sim.spawn(&UnitSpec::trooper(1, Vec3::new(40.0, 0.0, 0.0)));
    sim.add_cp(
        crate::sim::tests::sphere_cp("post", Vec3::ZERO, 10.0, false),
        Some(0),
    );
    sim
}

fn replay(log: &InputLog, full: bool) -> Vec<u8> {
    let mut sim = fixture();
    let mut trace = TraceWriter::new(Vec::new(), "flat-fixture", log.seed, full).unwrap();
    let mut untraced = fixture();
    for pads in log.frames() {
        sim.step(pads);
        trace.write_tick(&sim).unwrap();
        untraced.step(pads);
        assert_eq!(
            sim.hash(),
            untraced.hash(),
            "observational at tick {}",
            sim.tick
        );
    }
    trace.finish().unwrap()
}

#[test]
fn replay_600_ticks_and_one_pad_byte_first_divergence() {
    let mut log = InputLog {
        seed: 42,
        slots: 1,
        ticks: vec![PadState::default(); 600],
    };
    for full in [false, true] {
        let a = replay(&log, full);
        assert_eq!(a, replay(&log, full));
        // Only the low button byte changes, in the 300th input frame.
        log.ticks[299].buttons = buttons::JUMP;
        let b = replay(&log, full);
        let a = String::from_utf8(a).unwrap();
        let b = String::from_utf8(b).unwrap();
        let (left, right) = a.lines().zip(b.lines()).find(|(a, b)| a != b).unwrap();
        assert!(
            left.starts_with("300 ") || left.starts_with("301 "),
            "{left}"
        );
        eprintln!("first difference (full={full}):\n{left}\n{right}");
        log.ticks[299].buttons = 0;
    }
}

#[test]
fn selected_fields_change_only_their_group() {
    let baseline = fixture();
    let hashes = |s: &Sim| groups(s, false, false).unwrap().map(|g| g.hash);
    let before = hashes(&baseline);
    let check = |s: Sim, group: usize| {
        let after = hashes(&s);
        for i in 0..5 {
            assert_eq!(before[i] != after[i], i == group, "group {i}");
        }
    };
    let mut s = fixture();
    s.units[0].health.cur -= 1.0;
    check(s, 0);
    let mut s = fixture();
    s.units[0].soldier.pos.x += 0.125;
    check(s, 0);
    let mut s = fixture();
    s.units[0].alive = false;
    check(s, 0);
    let mut s = fixture();
    s.units[0].gen += 1;
    let h = hashes(&s);
    assert_ne!(h[0], before[0]);
    assert_ne!(h[4], before[4]);
    let mut s = fixture();
    s.lcg = crate::lcg::GameLcg::new(43);
    check(s, 1);
    let mut s = fixture();
    s.battle.teams[1].reinforcements -= 1.0;
    check(s, 2);
    let mut s = fixture();
    s.units[0].kills += 1;
    check(s, 2);
    let mut s = fixture();
    s.units[1].deaths += 1;
    check(s, 2);
    let mut s = fixture();
    s.cps[0].owner = Some(1);
    check(s, 3);
    let mut s = fixture();
    s.cps[0].capture += 0.125;
    check(s, 3);
    let mut s = fixture();
    s.cps[0].neutral += 0.125;
    check(s, 3);
    let mut s = fixture();
    s.units[0].respawn_at = Some(300);
    check(s, 4);
    let mut s = fixture();
    s.time_limit = Some(100);
    check(s, 4);
    let mut s = fixture();
    s.domination = Some((0, 2.0));
    check(s, 4);
    let mut s = fixture();
    s.waves[0].timer += 0.125;
    check(s, 4);
    let mut s = fixture();
    s.tick += 1;
    check(s, 4);
    let mut s = fixture();
    s.units[0].soldier.pos.x += POSITION_QUANTUM * 0.25;
    assert_eq!(hashes(&s), before, "sub-quantum control");
}

#[test]
fn quantization_and_wire_hash_have_independent_known_answers() {
    assert_eq!(quantize(-0.03125, POSITION_QUANTUM).unwrap(), -1);
    assert_eq!(quantize(0.03125, POSITION_QUANTUM).unwrap(), 1);
    assert_eq!(quantize(-0.125, HEALTH_QUANTUM).unwrap(), -1);
    assert_eq!(quantize(-0.0, HEALTH_QUANTUM).unwrap(), 0);
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
        assert!(quantize(bad, POSITION_QUANTUM).is_err());
    }
    // Standard FNV-1a reference vector: bytes "hello".
    let mut g = Group::new(false, false);
    for byte in b"hello" {
        g.u8(*byte);
    }
    assert_eq!(g.hash, 0xa430d84680aabd0b);
    // u32 0x6c6c6568 followed by u8 'o' must be the same bytes.
    let mut g = Group::new(false, false);
    g.u32(0x6c6c6568);
    g.u8(b'o');
    assert_eq!(g.hash, 0xa430d84680aabd0b);
}

#[test]
fn limits_header_injection_and_io_errors_are_not_silent() {
    assert!(TraceWriter::new(Vec::new(), "bad\n# seed 0", 42, false).is_err());
    let mut w = TraceWriter::new(Vec::new(), "fixture", 42, true).unwrap();
    w.bytes = FULL_BYTE_LIMIT;
    assert!(w.write_tick(&fixture()).is_err());
    assert!(TraceWriter::new(
        std::io::Cursor::new(&mut [0u8; 1][..]),
        "fixture",
        42,
        false
    )
    .is_err());
}

#[test]
fn original_hash_pins_without_tracing() {
    // Card DTEAM (2026-10-28) moved both: one human against none, so the
    // hash eats the per-team bot rows (`Sim::team_rows`). 2026-10-29 moved
    // both again: every human's spawn tick is kept (`focus_born`).
    // 2026-11-13 (#168) moved both: glam's `scalar-math`, one math path on
    // every CPU (LAN_MULTIPLAYER.md §7); 5573f2b603ed95f4 -> 71dabcfadf689bff,
    // 5c5458c8927f122b -> abcec91efdcf4c93.
    let mut sim = Sim::new(42);
    for _ in 0..100 {
        sim.step(&[PadState::default()]);
    }
    assert_eq!(sim.hash(), 0x71da_bcfa_df68_9bff);
    for _ in 100..600 {
        sim.step(&[PadState::default()]);
    }
    assert_eq!(sim.hash(), 0xabce_c91e_fdcf_4c93);
}

/// The desync rows' exact groups (issue #168): a nudge under the position
/// quantum moves the exact `units` group alone, while the quantized SHASH
/// row does not see it.
#[test]
fn exact_groups_see_a_sub_quantum_nudge_in_units_only() {
    let mut sim = fixture();
    sim.step(&[PadState::default()]);
    let row = |s: &Sim| {
        let mut t = TraceWriter::new(Vec::new(), "m", 42, false).unwrap();
        t.write_tick(s).unwrap();
        String::from_utf8(t.finish().unwrap()).unwrap()
    };
    // On a grid line, then nudged by 1 cm: the same quantized integer
    // either way (a quantum is 6.25 cm).
    let i = sim.units.iter().position(|u| u.alive).unwrap();
    let x = sim.units[i].soldier.pos.x;
    sim.units[i].soldier.pos.x = (x / POSITION_QUANTUM).round() * POSITION_QUANTUM;
    let base = (exact_group_hashes(&sim), row(&sim));
    sim.units[i].soldier.pos.x += 0.01;
    let after = exact_group_hashes(&sim);
    assert_eq!(row(&sim), base.1, "the quantized row cannot see 1 cm");
    assert_ne!(after[0], base.0[0], "the exact units group does");
    assert_eq!(after[1..], base.0[1..], "and nothing else moves");
    assert_eq!(GROUPS[0], "units");
    assert_eq!(
        exact_group_hashes(&sim),
        after,
        "the same state, the same row"
    );
}
