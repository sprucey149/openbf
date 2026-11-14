//! Per-panel statistics of a quarter-size walkabout, against a committed
//! reference: every map, every post, every bearing, plus the vantage shot.
//!
//! The map goldens (`golden_gallery.rs`) see two panels per map. A texture
//! that stops binding somewhere only the walkabout looks — a post on the far
//! side of the map, a bearing the golden does not face — waited for an eye.
//! This test gives each walkabout panel three numbers and fails the NAMED
//! panel whose numbers moved:
//!
//! * the mean R, G and B (tolerance ±[`MEAN_TOL`] per channel);
//! * the standard deviation of R, G and B (±[`STD_TOL`] relative, never
//!   tighter than ±[`STD_FLOOR`] — a panel of flat sky has a σ near zero);
//! * the percentage of "flat grey" pixels, |r−g| and |g−b| under 4 and
//!   60 < r < 200 — the untextured fallback's signature (+[`GREY_TOL`]
//!   points; only a RISE fails).
//!
//! The reference is `engine/goldens/walkabout_stats.ron`, rendered at the PS2
//! texture tier like every golden. The team bar across the top of each panel
//! is not counted.
//!
//! Ignored by default (needs a GPU adapter + baked assets); `scripts/gpu_tests.sh`
//! runs it. Rewrite the reference deliberately, after reading the table:
//!   UPDATE_GOLDENS=1 cargo test -p game --test walkabout_stats -- --ignored
//! The hook that proves it: `OPENBF_TEST_DROP_TEXTURE=<name>` makes the scene
//! upload skip that one texture, and the panels that see it fail.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod common;
use common::{force_ps2_tier, goldens_dir};

/// Every panel edge is divided by this (1280x720 → 320x180).
const SCALE: u32 = 4;
/// Mean-channel tolerance, 0..255 units.
const MEAN_TOL: f32 = 3.0;
/// Standard-deviation tolerance, relative.
const STD_TOL: f32 = 0.10;
/// The σ tolerance never drops below this, 0..255 units.
const STD_FLOOR: f32 = 1.0;
/// Flat-grey tolerance, percentage points; only an increase fails.
const GREY_TOL: f32 = 2.0;

/// One panel's numbers. `grey` is a percentage of the counted pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Panel {
    mean: [f32; 3],
    std: [f32; 3],
    grey: f32,
}

/// Two decimals: enough for the tolerances, and a rewrite of an unchanged
/// set rewrites the same bytes.
fn round2(v: f64) -> f32 {
    ((v * 100.0).round() / 100.0) as f32
}

/// Is this pixel the untextured fallback's flat grey?
fn flat_grey(r: u8, g: u8, b: u8) -> bool {
    r.abs_diff(g) < 4 && g.abs_diff(b) < 4 && 60 < r && r < 200
}

/// The numbers for the `pw`x`ph` panel at (`x0`, `y0`) in an RGBA image `w`
/// px wide, skipping its top `skip` rows (the team bar).
fn panel_stats(rgba: &[u8], w: u32, x0: u32, y0: u32, pw: u32, ph: u32, skip: u32) -> Panel {
    let (mut sum, mut sq, mut grey, mut n) = ([0f64; 3], [0f64; 3], 0u64, 0u64);
    for y in y0 + skip..y0 + ph {
        for x in x0..x0 + pw {
            let i = ((y * w + x) * 4) as usize;
            let (r, g, b) = (rgba[i], rgba[i + 1], rgba[i + 2]);
            for (c, v) in [r, g, b].into_iter().enumerate() {
                sum[c] += f64::from(v);
                sq[c] += f64::from(v) * f64::from(v);
            }
            grey += u64::from(flat_grey(r, g, b));
            n += 1;
        }
    }
    let n = n.max(1) as f64;
    let mean = sum.map(|s| s / n);
    let std = [0, 1, 2].map(|c| (sq[c] / n - mean[c] * mean[c]).max(0.0).sqrt());
    Panel {
        mean: mean.map(round2),
        std: std.map(round2),
        grey: round2(grey as f64 * 100.0 / n),
    }
}

/// What moved, or `None` when `got` is inside the tolerances of `want`.
fn compare(want: &Panel, got: &Panel) -> Option<String> {
    let mut out = Vec::new();
    for c in 0..3 {
        let ch = ["r", "g", "b"][c];
        if (got.mean[c] - want.mean[c]).abs() > MEAN_TOL {
            out.push(format!("mean {ch} {} -> {}", want.mean[c], got.mean[c]));
        }
        let tol = (want.std[c] * STD_TOL).max(STD_FLOOR);
        if (got.std[c] - want.std[c]).abs() > tol {
            out.push(format!("std {ch} {} -> {}", want.std[c], got.std[c]));
        }
    }
    if got.grey - want.grey > GREY_TOL {
        out.push(format!("flat grey {}% -> {}%", want.grey, got.grey));
    }
    (!out.is_empty()).then(|| out.join(", "))
}

/// Every panel of one map: `<map>/<shot>/<bearing>` for a post's four,
/// `<map>/vantage` for the vantage shot.
fn map_panels(key: &str, set: &game::gallery::WalkaboutSet, out: &mut BTreeMap<String, Panel>) {
    for shot in &set.shots {
        if shot.panels.len() == 1 {
            let p = panel_stats(&shot.rgba, shot.width, 0, 0, shot.width, shot.height, 0);
            out.insert(format!("{key}/{}", shot.name), p);
            continue;
        }
        let (pw, ph) = (shot.width / 2, shot.height / 2);
        for (i, st) in shot.panels.iter().enumerate() {
            let (x0, y0) = ((i as u32 % 2) * pw, (i as u32 / 2) * ph);
            let p = panel_stats(
                &shot.rgba,
                shot.width,
                x0,
                y0,
                pw,
                ph,
                game::gallery::WALK_BAR,
            );
            out.insert(format!("{key}/{}/{:03}", shot.name, st.bearing as u32), p);
        }
    }
}

fn reference_path() -> std::path::PathBuf {
    goldens_dir().join("walkabout_stats.ron")
}

#[test]
#[ignore = "requires a GPU adapter + baked assets_baked"]
fn every_walkabout_panel_matches_its_statistics() {
    force_ps2_tier();
    let assets = game::assets::baked_root();
    let maps = game::assets::discover_maps(&assets);
    assert!(!maps.is_empty(), "no baked maps under {}", assets.display());
    let mut got = BTreeMap::new();
    let mut failures = Vec::new();
    for (key, _game, dir) in &maps {
        match game::gallery::walkabout(dir, &assets, SCALE) {
            Ok(set) => map_panels(key, &set, &mut got),
            Err(e) => failures.push(format!("{key}: render failed: {e:#}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n  "));

    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        let text = ron::ser::to_string_pretty(&got, ron::ser::PrettyConfig::default())
            .expect("serialise the reference");
        std::fs::write(reference_path(), text + "\n").expect("write the reference");
        println!(
            "reference written: {} panels over {} maps",
            got.len(),
            maps.len()
        );
        return;
    }

    let text = std::fs::read_to_string(reference_path())
        .expect("engine/goldens/walkabout_stats.ron — write it with UPDATE_GOLDENS=1");
    let want: BTreeMap<String, Panel> = ron::from_str(&text).expect("parse the reference");
    for (name, w) in &want {
        match got.get(name) {
            None => failures.push(format!("{name}: in the reference, not rendered")),
            Some(g) => {
                if let Some(f) = compare(w, g) {
                    failures.push(format!("{name}: {f}"));
                }
            }
        }
    }
    for name in got.keys().filter(|n| !want.contains_key(*n)) {
        failures.push(format!("{name}: rendered, not in the reference"));
    }
    assert!(
        failures.is_empty(),
        "{} of {} walkabout panels moved:\n  {}",
        failures.len(),
        want.len(),
        failures.join("\n  ")
    );
    println!(
        "{} walkabout panels over {} maps match",
        got.len(),
        maps.len()
    );
}

/// The statistics themselves, on a hand-built image: the untextured grey
/// counts, the bar rows do not, and the tolerances cut where they say.
#[test]
fn panel_statistics_count_the_flat_grey_and_skip_the_bar() {
    // 4x4: the top row red (the "bar"), then 6 grey and 6 blue pixels.
    let mut px = Vec::new();
    for i in 0..16 {
        let c = match i {
            0..=3 => [255, 0, 0],
            4..=9 => [128, 129, 127],
            _ => [0, 0, 255],
        };
        px.extend_from_slice(&c);
        px.push(255);
    }
    let p = panel_stats(&px, 4, 0, 0, 4, 4, 1);
    assert_eq!(p.grey, 50.0);
    assert_eq!(p.mean, [64.0, 64.5, 191.0]);
    assert_eq!(p.std[0], 64.0);
    // The bar counted would be a red panel's worth of difference.
    assert_ne!(panel_stats(&px, 4, 0, 0, 4, 4, 0).mean, p.mean);

    let mut q = p;
    q.mean[1] += MEAN_TOL;
    q.std[2] += (p.std[2] * STD_TOL).max(STD_FLOOR) * 0.99;
    q.grey -= 10.0;
    assert_eq!(
        compare(&p, &q),
        None,
        "inside the limits, and a FALL in grey, pass"
    );
    q.grey = p.grey + GREY_TOL + 0.01;
    assert!(compare(&p, &q).unwrap().contains("flat grey"));
    q.grey = p.grey;
    q.mean[0] -= MEAN_TOL + 0.01;
    assert!(compare(&p, &q).unwrap().contains("mean r"));
}
