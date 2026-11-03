//! Which paths form each map's boundary. On the disc the list is the map LVL's
//! `bnd_` chunk (`analysis/re-boundary.md` 1), which the bake does not read; this
//! table holds what each chunk names, read off the LVLs of both discs, so the
//! existing bakes work without a rebake. Card BNDY can bake it in place of this.

use core::boundary::Boundary;
use manifest::ManifestPath;

/// Map directory -> the paths its boundary names. A map not listed has none: BF1
/// YAV1, YAV2 and BF2 TAT3 have no `bnd_` chunk; BF2 COR1, DAG1, DEA1, MUS1, MYG1,
/// NAB2, TAN1 and UTA1 have an empty one, although some carry a path called
/// `boundary`, which the PS2 never reads.
const SHAPES: [(&str, &[&str]); 23] = [
    ("bf1_bes1", &["Boundary"]),
    ("bf1_bes2", &["boundary1"]),
    ("bf1_end1", &["boundary1"]),
    ("bf1_geo1", &["boundary1"]),
    ("bf1_hot1", &["boundary1"]),
    ("bf1_kam1", &["boundary1"]),
    ("bf1_kas1", &["boundary1"]),
    ("bf1_kas2", &["Boundary1"]),
    ("bf1_nab1", &["boundary1"]),
    ("bf1_nab2", &["Boundary"]),
    ("bf1_rhn1", &["Boundary"]),
    ("bf1_rhn2", &["Boundary"]),
    ("bf1_tat2", &["boundary1"]),
    ("tat1", &["boundary"]),
    ("bf2_end1", &["boundary1"]),
    (
        "bf2_fel1",
        &["warning1", "Warning2", "Warning3", "Warning4"],
    ),
    ("bf2_geo1", &["boundary1"]),
    ("bf2_hot1", &["boundary1", "boundary2", "boundary3"]),
    ("bf2_kam1", &["1134563362"]),
    ("bf2_kas2", &["boundary1", "boundary2"]),
    ("bf2_pol1", &["pol1"]),
    ("bf2_yav1", &["boundary1"]),
    ("tat2", &["boundary1"]),
];

/// The shape names listed for the map directory `key`.
pub fn shape_names(key: &str) -> &'static [&'static str] {
    SHAPES
        .iter()
        .find(|(map, _)| map.eq_ignore_ascii_case(key))
        .map_or(&[], |(_, names)| names)
}

/// The map's boundary from its baked paths. A name takes the first path of that
/// name, as the PS2's lookup does (`re-boundary.md` C4); a name with no path adds
/// nothing (C3).
pub fn boundary(key: &str, paths: &[ManifestPath]) -> Boundary {
    let mut out = Boundary::default();
    for name in shape_names(key) {
        if let Some(p) = paths.iter().find(|p| p.name.eq_ignore_ascii_case(name)) {
            let nodes: Vec<[f32; 2]> = p.nodes.iter().map(|(n, _)| [n[0], n[2]]).collect();
            out.add_shape(&nodes);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str, x: f32) -> ManifestPath {
        ManifestPath {
            flight_props: Vec::new(),
            name: name.into(),
            nodes: [[0.0, 0.0], [0.0, 10.0], [10.0, 10.0], [10.0, 0.0]]
                .map(|[a, z]| ([a + x, 0.0, z], [1.0, 0.0, 0.0, 0.0]))
                .to_vec(),
            props: Vec::new(),
        }
    }

    #[test]
    fn a_name_takes_its_first_path_whatever_its_case() {
        let paths = [path("BOUNDARY1", 0.0), path("boundary1", 100.0)];
        let b = boundary("bf1_geo1", &paths);
        assert!(!b.outside(5.0, 5.0));
        assert!(b.outside(105.0, 5.0));
    }

    #[test]
    fn a_map_with_no_bnd_entry_ignores_a_path_called_boundary() {
        let b = boundary("bf2_myg1", &[path("boundary", 0.0)]);
        assert!(!b.outside(500.0, 500.0));
    }
}
