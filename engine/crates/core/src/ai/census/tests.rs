use super::*;
use crate::perception::{Contact, Foe};
use crate::sim::Sim;
use crate::unit::{Controller, UnitSpec};
use glam::Vec3;

fn bot(sim: &mut Sim, team: u8, at: Vec3) -> UnitHandle {
    sim.spawn(&UnitSpec {
        controller: Controller::Ai,
        ..UnitSpec::trooper(team, at)
    })
    .expect("bot")
}

fn seen(sim: &Sim, h: UnitHandle) -> Contact {
    Contact {
        handle: Foe::Unit(h),
        first_seen: 0,
        last_seen: sim.tick,
        last_pos: sim.unit(h).unwrap().capsule().center(),
        seen_now: true,
        heard: false,
        told: false,
    }
}

/// A bot at the origin, an enemy target 30 m away and (with `near`) an
/// enemy bot 6 m away; `human`: the target is a proxy.
fn scene(human: bool, near: bool) -> (Sim, UnitHandle, AiState) {
    let (mut sim, _) = crate::ai::tests::l_sim();
    sim.units.clear();
    sim.players.clear();
    let me = bot(&mut sim, 0, Vec3::ZERO);
    let far = bot(&mut sim, 1, Vec3::new(0.0, 0.0, -30.0));
    if human {
        sim.set_proxy(far, Some(crate::ai::proxy::Strength::Good.profile()));
    }
    let mut st = AiState::default();
    st.contacts[0] = Some(seen(&sim, far));
    if near {
        let n = bot(&mut sim, 1, Vec3::new(6.0, 0.0, 0.0));
        st.contacts[1] = Some(seen(&sim, n));
    }
    st.target = Some(far.into());
    (sim, me, st)
}

/// D1: a far human chosen with a near engageable bot about is FAR; so is a
/// far bot (the comparison rate); with nobody nearer neither is.
#[test]
fn the_d1_bits_mark_a_target_chosen_across_a_nearer_bot() {
    let (sim, me, st) = scene(true, true);
    assert_eq!(
        d1(&sim, me, &st, true),
        D1_TARGET | D1_HUMAN | D1_FAR | D1_TAKEN
    );
    let (sim, me, st) = scene(false, true);
    assert_eq!(d1(&sim, me, &st, false), D1_TARGET | D1_FAR);
    let (sim, me, st) = scene(true, false);
    assert_eq!(d1(&sim, me, &st, false), D1_TARGET | D1_HUMAN);
    // A remembered target is not counted.
    let (sim, me, mut st) = scene(true, true);
    st.contacts[0].as_mut().unwrap().seen_now = false;
    assert_eq!(d1(&sim, me, &st, false), 0);
}

/// The census folds a tick's bits by group and keeps the D1 split.
#[test]
fn the_census_folds_its_groups_apart() {
    use crate::skill::Difficulty::{Hard, Normal};
    let mut c = BotCensus::default();
    let nt = NavTick {
        thought: 1,
        d1: D1_TARGET | D1_HUMAN | D1_FAR,
        levers: LV_REACT_WAIT | LV_FLINCH,
        ..NavTick::default()
    };
    c.add(Hard, true, false, true, &nt);
    c.add(Normal, false, false, true, &nt);
    c.add(Normal, false, true, true, &nt);
    c.add(Hard, true, false, false, &nt);
    let facing = c.cell(Hard, true, false);
    assert_eq!((facing.foot_ticks, facing.thinks), (1, 1));
    assert_eq!(facing.d1_split(), [1, 1, 0, 0, 0, 0, 0, 0]);
    assert_eq!(facing.levers[0], 1);
    assert_eq!(facing.levers[EVENT_LEVERS as usize], 1);
    assert_eq!(c.cell(Normal, false, false).foot_ticks, 1);
    assert_eq!(c.cell(Normal, false, true).foot_ticks, 1);
    assert_eq!(c.cell(Hard, false, false).foot_ticks, 0);
    assert_eq!(c.sum(|_, _, proxy| !proxy).foot_ticks, 2);
    let mut twice = c;
    twice.merge(&c);
    assert_eq!(twice.sum(|_, _, _| true).thinks, 6);
    assert_eq!(LEVER_NAMES[EVENT_LEVERS as usize], "flinch");
}

/// The histogram's quantiles land in the right bucket (25 % wide).
#[test]
fn the_think_histogram_reads_its_quantiles() {
    let mut h = Hist::default();
    for _ in 0..99 {
        h.add(10_000);
    }
    h.add(1_000_000);
    let p50 = h.quantile_us(0.5);
    assert!((10.0..=12.5).contains(&p50), "{p50}");
    assert!((10.0..=12.5).contains(&h.quantile_us(0.99)));
    let top = h.quantile_us(1.0);
    assert!((1000.0..=1250.0).contains(&top), "{top}");
    assert_eq!(Hist::default().quantile_us(0.5), 0.0);
}

/// Nothing the census holds is in the hash.
#[test]
fn the_census_never_moves_the_hash() {
    let (mut sim, _, _) = scene(true, true);
    let before = sim.hash();
    sim.bot_census.cells[2][1][0].foot_ticks = 7;
    sim.bot_census.cells[1][0][1].levers[3] = 11;
    sim.bot_census.cells[3][1][0].d1[5] = 13;
    sim.think_hist.all.add(123_456);
    sim.think_hist.proxy.add(42);
    assert_eq!(sim.hash(), before);
}

/// A stepped skirmish fills the census on foot and the think costs.
#[test]
fn a_stepped_round_counts_its_thinks() {
    let (mut sim, _, _) = scene(false, true);
    for _ in 0..60 {
        sim.step(&[]);
    }
    let c = sim.bot_census.sum(|_, _, proxy| !proxy);
    assert!(c.foot_ticks > 60 && c.foot_ticks <= 3 * 60, "{c:?}");
    assert!(c.thinks >= 6 && c.thinks < c.foot_ticks, "{c:?}");
    assert_eq!(sim.think_hist.all.n, c.foot_ticks, "nobody seated");
    assert_eq!(sim.bot_census.sum(|_, _, proxy| proxy).foot_ticks, 0);
}

/// In a proxy round the bots facing the proxy and his allies (a lower row,
/// card DTEAM) land in different groups, and the proxy in its own.
#[test]
fn a_proxy_round_keeps_the_proxys_foes_and_allies_apart() {
    let (mut sim, _, _) = scene(true, true);
    sim.difficulty = crate::skill::Difficulty::Hard;
    let (foe_row, ally_row) = (sim.difficulty_of(0), sim.difficulty_of(1));
    assert_eq!(foe_row, crate::skill::Difficulty::Hard);
    assert_ne!(ally_row, foe_row, "the proxy's team plays a lower row");
    for _ in 0..60 {
        sim.step(&[]);
    }
    let c = sim.bot_census;
    assert_eq!(c.cell(foe_row, true, false).foot_ticks, 60, "{c:?}");
    assert_eq!(c.cell(ally_row, false, false).foot_ticks, 60, "{c:?}");
    assert_eq!(c.cell(ally_row, false, true).foot_ticks, 60, "{c:?}");
    assert_eq!(c.sum(|_, _, _| true).foot_ticks, 180);
}
