//! Card COVR: a BF1 map's Cover hints from the BF2 version of the same map
//! (`lead_tmp/botre/xbox_pc.md` finding 1). BF2's Yavin Temple authors 973
//! Cover hints on the same planning graph as BF1's, which has 51: the hubs
//! match 85 of 85, so its hints fit BF1's ground as they stand.
//!
//! A step over two BAKED maps (`bake donor-hints <assets_baked>`, after
//! `bake map` has baked both): the donor's Cover hints, moved by the table's
//! offset, are added to the target's top-level nav block, named
//! `donor_<name>`. The graphs must still match (every target hub within
//! [`HUB_MATCH`] of a donor hub, moved), else nothing is added. A hint is
//! dropped when it lands deeper than [`PUSHABLE`] inside a soldier barrier of
//! the target, off its
//! graph (no hub within [`OFF_GRAPH`] in XZ and [`OFF_GRAPH_DY`] in height),
//! or within [`SAME`] of a hint the target authors itself. The position table
//! (`core::ai::tactic`) then drops what has no floor or head room, as for any
//! hint. Run again it replaces its own hints. A named deviation (D22,
//! `analysis/bot-design.md` 9).

use anyhow::{bail, Context, Result};
use manifest::{Manifest, ManifestBarrier, ManifestHint, ManifestNav};
use std::path::Path;

/// Which map takes whose hints, and the offset (m) that carries the donor's
/// positions onto the target's: (target, donor, offset). Only maps whose
/// donor is the user's own disc data; BF1 Bespin Platforms and Rhen Var
/// Harbor are not here (their BF2 versions come from a fan conversion).
pub const DONORS: &[(&str, &str, [f32; 3])] = &[("bf1_yav1", "bf2_yav1", [0.0, 0.0, 0.0])];

/// The donor's hint kind that is taken: Cover.
pub const COVER: u8 = 2;
/// A target hub is matched by a donor hub this near, moved (m)...
pub const HUB_MATCH: f32 = 0.5;
/// ...and this share of them must be.
pub const HUB_SHARE: f32 = 0.9;
/// A hint with no target hub within this in XZ (m)...
pub const OFF_GRAPH: f32 = 40.0;
/// ...and this in height (m) is off the graph.
pub const OFF_GRAPH_DY: f32 = 6.0;
/// A hint this deep inside a soldier barrier is dropped (m): the position
/// table moves one out of a barrier by at most 0.5 m to 0.35 m clear of it
/// (`core::ai::tactic`'s `HINT_PUSH`, `IN_BARRIER`), so a shallower one is
/// left for it to push (BF2's Cover hints are authored against the barriers
/// round the props they hide behind: 257 of Yavin's 973 lie within this).
pub const PUSHABLE: f32 = 0.15;
/// A hint this near one the target authors is the same one (m).
pub const SAME: f32 = 1.0;
/// The prefix of a donated hint's name.
pub const PREFIX: &str = "donor_";

/// What one donation did.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Donation {
    pub target: String,
    pub donor: String,
    pub hubs_matched: usize,
    pub hubs: usize,
    pub offered: usize,
    pub in_barrier: usize,
    pub off_graph: usize,
    pub same: usize,
    pub added: usize,
    /// The target's own Cover hints.
    pub own: usize,
}

/// Soldier barrier bit (`AISizeType` 0).
const SOLDIER: u32 = 1;

/// Is the hint's point (XZ) deeper than [`PUSHABLE`] inside the barrier?
/// The barrier's own test (`core::nav::Barrier::contains`): its rows scale
/// SIZE, a half extent.
fn inside(b: &ManifestBarrier, p: [f32; 3]) -> bool {
    let axis = |row: [f32; 3], fallback: [f32; 2]| {
        let len = (row[0] * row[0] + row[1] * row[1] + row[2] * row[2])
            .sqrt()
            .max(1e-6);
        let n = (row[0] * row[0] + row[2] * row[2]).sqrt();
        if n > 1e-6 {
            ([row[0] / n, row[2] / n], len)
        } else {
            (fallback, len)
        }
    };
    let (ax, lx) = axis(b.rows[0], [1.0, 0.0]);
    let (az, lz) = axis(b.rows[2], [0.0, 1.0]);
    let d = [p[0] - b.pos[0], p[2] - b.pos[2]];
    (d[0] * ax[0] + d[1] * ax[1]).abs() < b.size[0] * lx - PUSHABLE
        && (d[0] * az[0] + d[1] * az[1]).abs() < b.size[2] * lz - PUSHABLE
}

fn moved(p: [f32; 3], off: [f32; 3]) -> [f32; 3] {
    [p[0] + off[0], p[1] + off[1], p[2] + off[2]]
}

/// Add the donor's Cover hints to `target` (its own donated hints first
/// removed). `Err` when either has no nav block or the graphs do not match.
pub fn donate(target: &mut Manifest, donor: &Manifest, off: [f32; 3]) -> Result<Donation> {
    let (Some(tn), Some(dn)) = (target.nav.as_mut(), donor.nav.as_ref()) else {
        bail!("a map without a nav block");
    };
    let mut r = Donation {
        target: target.map.clone(),
        donor: donor.map.clone(),
        ..Donation::default()
    };
    let hub_pos = |n: &ManifestNav| -> Vec<[f32; 3]> {
        n.plan
            .as_ref()
            .map(|p| p.hubs.iter().map(|h| h.pos).collect())
            .unwrap_or_default()
    };
    let (th, dh) = (hub_pos(tn), hub_pos(dn));
    let dist = |a: [f32; 3], b: [f32; 3]| {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    };
    r.hubs = th.len();
    r.hubs_matched = th
        .iter()
        .filter(|t| dh.iter().any(|d| dist(**t, moved(*d, off)) <= HUB_MATCH))
        .count();
    if th.is_empty() || (r.hubs_matched as f32) < HUB_SHARE * th.len() as f32 {
        bail!(
            "the graphs do not match: {} of {} hubs within {HUB_MATCH} m",
            r.hubs_matched,
            r.hubs
        );
    }
    tn.hints.retain(|h| !h.name.starts_with(PREFIX));
    let own: Vec<[f32; 3]> = tn.hints.iter().map(|h| h.xform[3]).collect();
    r.own = tn.hints.iter().filter(|h| h.kind == COVER).count();
    let mut add: Vec<ManifestHint> = Vec::new();
    for h in dn.hints.iter().filter(|h| h.kind == COVER) {
        r.offered += 1;
        let p = moved(h.xform[3], off);
        let xz = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        if tn
            .barriers
            .iter()
            .any(|b| b.flags & SOLDIER != 0 && inside(b, p))
        {
            r.in_barrier += 1;
        } else if !th
            .iter()
            .any(|q| xz(*q, p) <= OFF_GRAPH && (q[1] - p[1]).abs() <= OFF_GRAPH_DY)
        {
            r.off_graph += 1;
        } else if own.iter().any(|q| dist(*q, p) <= SAME) {
            r.same += 1;
        } else {
            let mut h = h.clone();
            h.name = format!("{PREFIX}{}", h.name);
            h.xform[3] = p;
            add.push(h);
        }
    }
    r.added = add.len();
    tn.hints.extend(add);
    Ok(r)
}

/// Every [`DONORS`] row whose two maps are baked under `root`: read both
/// manifests, donate, write the target's back.
pub fn donate_all(root: &Path) -> Result<Vec<Donation>> {
    let read = |key: &str| -> Result<Option<Manifest>> {
        let p = root.join(key).join("manifest.ron");
        if !p.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&p).with_context(|| p.display().to_string())?;
        Ok(Some(
            ron::from_str(&text).with_context(|| p.display().to_string())?,
        ))
    };
    let mut out = Vec::new();
    for &(target, donor, off) in DONORS {
        let (Some(mut t), Some(d)) = (read(target)?, read(donor)?) else {
            continue;
        };
        let mut r = donate(&mut t, &d, off).with_context(|| format!("{target} from {donor}"))?;
        (r.target, r.donor) = (target.into(), donor.into());
        crate::write_ron(&root.join(target).join("manifest.ron"), &t)?;
        out.push(r);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use manifest::{ManifestNavHub, ManifestNavPlan};

    fn hint(name: &str, kind: u8, p: [f32; 3]) -> ManifestHint {
        ManifestHint {
            name: name.into(),
            kind,
            xform: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], p],
            primary_stance: None,
            secondary_stance: None,
            mode: None,
            radius: None,
            command_post: None,
            props: Vec::new(),
        }
    }

    fn map(
        name: &str,
        hubs: &[[f32; 3]],
        hints: Vec<ManifestHint>,
        barriers: Vec<ManifestBarrier>,
    ) -> Manifest {
        let mut m: Manifest = ron::from_str(&format!(
            "(map: \"{name}\", source_game: \"bf2\", terrain_name: None, sky_name: None, terrain_glb: None, models: [], \
             textures: [], classes: [], geometries: [], regions: [], instances: [])"
        ))
        .expect("a bare manifest");
        m.nav = Some(ManifestNav {
            plan: Some(ManifestNavPlan {
                hubs: hubs
                    .iter()
                    .map(|&pos| ManifestNavHub {
                        name: String::new(),
                        pos,
                        radius: 3.0,
                        arcs: Vec::new(),
                    })
                    .collect(),
                arcs: Vec::new(),
                routes: String::new(),
                plnp: None,
            }),
            hints,
            barriers,
        });
        m
    }

    fn wall(at: [f32; 3]) -> ManifestBarrier {
        ManifestBarrier {
            name: "w".into(),
            pos: at,
            rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            size: [2.0, 1.0, 2.0],
            flags: SOLDIER,
        }
    }

    const HUBS: [[f32; 3]; 2] = [[0.0, 0.0, 0.0], [30.0, 0.0, 0.0]];

    /// The donor's Cover hints are added, moved and renamed; one deep in a
    /// soldier barrier (not one at its edge), one off the graph (too far, too high), one on the
    /// target's own and a Snipe hint are not; run twice, the same.
    #[test]
    fn cover_hints_are_donated_where_they_fit() {
        let off = [1.0, 0.0, 0.0];
        let shifted: Vec<[f32; 3]> = HUBS.iter().map(|h| [h[0] - 1.0, h[1], h[2]]).collect();
        let donor = map(
            "d",
            &shifted,
            vec![
                hint("ok", COVER, [4.0, 0.0, 4.0]),
                hint("walled", COVER, [9.0, 0.0, 10.0]),
                // 0.1 m inside the wall's edge: left for the table to push.
                hint("edge", COVER, [10.9, 0.0, 10.0]),
                hint("far", COVER, [100.0, 0.0, 0.0]),
                hint("high", COVER, [5.0, 20.0, 0.0]),
                hint("twin", COVER, [-1.0, 0.0, 20.0]),
                hint("snipe", 0, [6.0, 0.0, 6.0]),
            ],
            Vec::new(),
        );
        let mut t = map(
            "t",
            &HUBS,
            vec![hint("own", COVER, [0.0, 0.0, 20.3])],
            vec![wall([10.0, 0.0, 10.0])],
        );
        let r = donate(&mut t, &donor, off).expect("matched");
        assert_eq!(
            (r.offered, r.in_barrier, r.off_graph, r.same, r.added, r.own),
            (6, 1, 2, 1, 2, 1)
        );
        let hints = &t.nav.as_ref().expect("nav").hints;
        let d = hints.iter().find(|h| h.name == "donor_ok").expect("added");
        assert_eq!(d.xform[3], [5.0, 0.0, 4.0], "moved by the offset");
        let once = t.clone();
        let again = donate(&mut t, &donor, off).expect("again");
        assert_eq!((t, again), (once, r), "a second run replaces its own");
    }

    /// Graphs that do not match give nothing.
    #[test]
    fn a_donor_on_another_graph_is_refused() {
        let donor = map(
            "d",
            &[[0.0, 0.0, 0.0], [50.0, 0.0, 0.0]],
            vec![hint("a", COVER, [1.0, 0.0, 1.0])],
            Vec::new(),
        );
        let mut t = map("t", &HUBS, Vec::new(), Vec::new());
        assert!(donate(&mut t, &donor, [0.0; 3]).is_err());
        assert!(t.nav.expect("nav").hints.is_empty());
    }
}
