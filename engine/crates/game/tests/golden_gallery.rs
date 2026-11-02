//! Tier-2 visual goldens across the whole content set: one committed sheet per
//! baked map (overview + ground view with a soldier for scale) and one per
//! baked unit (four poses with its weapon).
//!
//! These exist so a change that only shows up on ONE map or ONE unit — a
//! texture that stops binding, a pose that breaks, terrain that goes flat —
//! fails a test instead of waiting to be noticed.
//!
//! Ignored by default (needs a GPU adapter + baked assets):
//!   cargo test -p game --test golden_gallery -- --ignored
//! Regenerate deliberately, after looking at the PNGs:
//!   UPDATE_GOLDENS=1 cargo test -p game --test golden_gallery -- --ignored
//! `scripts/goldens.sh` does the whole set in one go.

mod common;
use common::{check_golden, committed_goldens, force_ps2_tier, goldens_dir};

#[test]
#[ignore = "requires a GPU adapter + baked assets_baked"]
fn every_baked_map_matches_its_golden() {
    force_ps2_tier();
    let assets = game::assets::baked_root();
    let maps = game::assets::discover_maps(&assets);
    // Bug T1 for the 32 map goldens: an empty `maps` used to print SKIP and
    // pass, with `scripts/goldens.sh --check` exiting 0 and nothing rendered.
    // Every COMMITTED golden must be rendered — golden_hud.rs:42.
    let rendered: std::collections::BTreeSet<&str> =
        maps.iter().map(|(key, _, _)| key.as_str()).collect();
    let unrendered: Vec<String> = committed_goldens(&goldens_dir().join("maps"))
        .into_iter()
        .filter(|key| !rendered.contains(key.as_str()))
        .collect();
    assert!(
        unrendered.is_empty(),
        "{} committed map goldens have no baked map to render them (bake the maps first):\n  {}",
        unrendered.len(),
        unrendered.join("\n  ")
    );
    let mut failures = Vec::new();
    for (key, _game, dir) in &maps {
        let (rgba, w, h) = match game::gallery::map_sheet(dir, &assets) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("{key}: render failed: {e:#}"));
                continue;
            }
        };
        if let Some(f) = check_golden(
            &goldens_dir().join("maps").join(format!("{key}.png")),
            &rgba,
            w,
            h,
        ) {
            failures.push(f);
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} map goldens differ:\n  {}",
        failures.len(),
        maps.len(),
        failures.join("\n  ")
    );
    println!("{} map goldens match", maps.len());
}

/// The loading art, one grid per source game. No GPU needed — it is decoded
/// texture data — but it lives here because it is part of the same "look at
/// the pictures" set.
#[test]
#[ignore = "requires baked assets_baked"]
fn load_screens_match_their_goldens() {
    force_ps2_tier();
    let assets = game::assets::baked_root();
    let mut failures = Vec::new();
    // Both sheets are committed goldens, so both must be rendered: this used
    // to `continue` past an unbaked game and return early when neither built,
    // and `scripts/goldens.sh --check` still exited 0 (bug T1). A golden test
    // cannot skip silently — golden_hud.rs:42.
    for game in ["bf1", "bf2"] {
        let (rgba, w, h) = match game::gallery::loadscreen_sheet(&assets, game) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!(
                    "loadscreens_{game}.png has nothing to render: {e:#} (bake {game}'s load screens)"
                ));
                continue;
            }
        };
        if let Some(f) = check_golden(
            &goldens_dir().join(format!("loadscreens_{game}.png")),
            &rgba,
            w,
            h,
        ) {
            failures.push(f);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n  "));
}

#[test]
#[ignore = "requires a GPU adapter + baked assets_baked"]
fn every_baked_unit_matches_its_golden() {
    force_ps2_tier();
    let assets = game::assets::baked_root();
    let units = game::gallery::discover_units(&assets);
    // Bug T1: an absent `assets_baked/side_*/soldiers` used to return early
    // (and a half-baked one to check half the set) with `goldens.sh --check`
    // exiting 0 all the same. Every COMMITTED golden must be rendered — a
    // golden test cannot skip silently, golden_hud.rs:42.
    let rendered: std::collections::BTreeSet<&str> =
        units.iter().map(|(key, ..)| key.as_str()).collect();
    let unrendered: Vec<String> = committed_goldens(&goldens_dir().join("units"))
        .into_iter()
        .filter(|key| !rendered.contains(key.as_str()))
        .collect();
    assert!(
        unrendered.is_empty(),
        "{} committed unit goldens have no baked unit to render them (bake the sides first):\n  {}",
        unrendered.len(),
        unrendered.join("\n  ")
    );
    let mut failures = Vec::new();
    for (key, glb, weapon, set) in &units {
        let (rgba, w, h) = match game::gallery::unit_sheet(glb, weapon.as_deref(), set) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("{key}: render failed: {e:#}"));
                continue;
            }
        };
        if let Some(f) = check_golden(
            &goldens_dir().join("units").join(format!("{key}.png")),
            &rgba,
            w,
            h,
        ) {
            failures.push(f);
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} unit goldens differ:\n  {}",
        failures.len(),
        units.len(),
        failures.join("\n  ")
    );
    println!("{} unit goldens match", units.len());
}
