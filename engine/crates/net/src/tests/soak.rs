//! Long lossy runs: bundle agreement, input conservation and pacing.

use super::harness::*;
use crate::*;
use core::sim::buttons::FIRE;

fn mean_after(d: &[u8], skip: usize) -> f64 {
    let s = &d[skip.min(d.len())..];
    s.iter().map(|&x| f64::from(x)).sum::<f64>() / s.len().max(1) as f64
}

/// Host + 4 clients, 40 ms ± 10 ms one way, 3% loss, 3,000 ticks.
fn soak(link: LinkParams, ticks: u32, seed: u64) -> Net {
    soak_with(link, ticks, seed, Net::host_config())
}

fn soak_with(link: LinkParams, ticks: u32, seed: u64, config: HostConfig) -> Net {
    let mut net = Net::new(seed, link, config);
    let frames = [(7.0, 1.0), (16.7, 1.0004), (11.0, 0.9996), (6.9, 1.001)];
    for (i, (f, d)) in frames.iter().enumerate() {
        let i = i as u16 + 1;
        net.join(
            i,
            &format!("P{i}"),
            100 + u64::from(i),
            Player::new(*f, *d, seed + u64::from(i)),
        );
    }
    net.connect_all();
    net.start_round();
    net.run_until(200_000, |n| n.host.next_tick() >= ticks);
    // Stop input and let every queue and stream drain.
    net.host_player.playing = false;
    for c in &mut net.clients {
        c.player.playing = false;
    }
    net.run_ms(3_000);
    net
}

fn check_agreement_and_conservation(net: &Net, ticks: usize) {
    let host = &net.host_bundles;
    assert!(host.len() >= ticks);
    for (i, b) in host.iter().enumerate() {
        assert_eq!(b.tick as usize, i);
    }
    for c in &net.clients {
        assert!(
            c.bundles.len() + 6 >= host.len(),
            "client lags: {} of {}",
            c.bundles.len(),
            host.len()
        );
        assert_eq!(
            c.bundles[..],
            host[..c.bundles.len()],
            "client stream differs"
        );
    }
    let mut players: Vec<(u8, &Vec<core::sim::PadState>)> = vec![(0, &net.host_player.submitted)];
    for c in &net.clients {
        players.push((c.core.slot().unwrap(), &c.player.submitted));
    }
    for (slot, sent) in players {
        let got = slot_pads(host, slot);
        assert!(sent.len() + 100 > ticks, "slot {slot} sent {}", sent.len());
        let sum = |v: &[core::sim::PadState]| -> (i64, i64) {
            v.iter().fold((0, 0), |a, p| {
                (a.0 + i64::from(p.mx), a.1 + i64::from(p.my))
            })
        };
        assert_eq!(sum(&got), sum(sent), "slot {slot} mouse deltas");
        assert_eq!(
            edges(got.iter(), FIRE),
            edges(sent.iter(), FIRE),
            "slot {slot} FIRE presses"
        );
    }
}

#[test]
fn soak_four_clients_lossy_jittery_link_agree_and_conserve_input() {
    let link = LinkParams {
        latency: Micros::from_millis(40),
        jitter: Micros::from_millis(10),
        loss: 0.03,
        duplicate: 0.0,
        reorder: 0.0,
    };
    let net = soak(link, 3_000, 11);
    check_agreement_and_conservation(&net, 3_000);
    for slot in 0..5u8 {
        let m = mean_after(&net.depths[usize::from(slot)][..3_000], 600);
        assert!((0.5..=2.5).contains(&m), "slot {slot} mean depth {m}");
    }
    let st = net.host.stats();
    assert_eq!(st.drops.malformed, 0);
}

#[test]
fn soak_pacing_converges_on_a_clean_lan() {
    let net = soak(LinkParams::ideal(Micros::from_millis(1)), 1_500, 5);
    check_agreement_and_conservation(&net, 1_500);
    for slot in 0..5u8 {
        let m = mean_after(&net.depths[usize::from(slot)][..1_500], 600);
        assert!((0.5..=2.5).contains(&m), "slot {slot} mean depth {m}");
    }
}

#[test]
fn soak_host_stamps_each_players_view_lag() {
    let link = LinkParams {
        latency: Micros::from_millis(40),
        jitter: Micros::from_millis(10),
        loss: 0.03,
        duplicate: 0.0,
        reorder: 0.0,
    };
    let net = soak(link, 900, 13);
    check_agreement_and_conservation(&net, 900);
    let lags = |slot: u8| -> Vec<u8> {
        let mut v: Vec<u8> = slot_pads(&net.host_bundles[..900], slot)
            .iter()
            .map(|p| p.view_lag)
            .collect();
        v.sort_unstable();
        v
    };
    let median = |v: &[u8]| v[v.len() / 2];
    // The host plays its bundles as they close: only its own queue delays
    // it (measured: median 0, max 4).
    let host = lags(0);
    assert!(median(&host) <= 1, "host median {}", median(&host));
    // A guest 80 ms round trip away views a few ticks back (measured: median
    // 3, max 7-8); never past the cap.
    for c in &net.clients {
        let v = lags(c.core.slot().unwrap());
        assert!((2..=5).contains(&median(&v)), "guest median {}", median(&v));
        assert!(v.iter().all(|&l| l <= DEFAULT_LAG_COMP_CAP));
    }
    // Cap 0 turns it off.
    let mut off = Net::host_config();
    off.lag_comp_cap = 0;
    let net = soak_with(link, 300, 13, off);
    assert!(net.host_bundles[..300]
        .iter()
        .all(|b| b.pads.iter().all(|(_, p)| p.view_lag == 0)));
}
