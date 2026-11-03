//! Repair fingerprinted donor face diagonals without moving donor vertices.
//!
//! The selected building/rock vertices already match PS2 within quantization
//! error, but nonplanar quads have different diagonals. Their measured extra
//! contact gaps range from 0.104 to 0.542 m. Placement, whole-model and group
//! transforms cannot repair a diagonal while keeping its matching corners.
//! Flip only donor edges; the Felucia rock also closes one existing-vertex face.
//! Positions, UVs, normals, colours, headers and collision are preserved.

use anyhow::{ensure, Result};
use mungefs::model::Model;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

struct SegmentRecipe {
    vertices: usize,
    faces: usize,
    edges: &'static [[u32; 2]],
    added: &'static [[u32; 3]],
}

struct Recipe {
    model: &'static str,
    inputs: &'static [&'static str],
    segments: &'static [SegmentRecipe],
}

const RECIPES: &[Recipe] = &[
    Recipe {
        model: "fel1_prop_rock_group01",
        inputs: &[
            "5d10b9b84f4f433d26a879d60f008fa41397e23bc41ab28881c56f4662d10146",
            "ee4074ca03293f4907e4431d42e6ee6f942323897dbb0c38e05c6a416f46a97b",
        ],
        segments: &[SegmentRecipe {
            vertices: 54,
            faces: 78,
            edges: &[
                [41, 53],
                [30, 43],
                [20, 32],
                [28, 34],
                [42, 47],
                [38, 46],
                [24, 38],
                [15, 24],
                [18, 25],
                [17, 30],
                [14, 26],
                [12, 23],
                [22, 33],
            ],
            added: &[[0, 2, 4]],
        }],
    },
    Recipe {
        model: "rhn_bldg_lego_1",
        inputs: &[
            "f677203f9f229c561b4480ecbbc4816c3ce4cd0b7d0924225a36a3dd3ec59721",
            "8a94f47a2984c49bd708476b9678ed8ff3d2b6c3442c886c9d091112a3fbcb47",
        ],
        segments: &[
            SegmentRecipe {
                vertices: 56,
                faces: 42,
                edges: &[[35, 53]],
                added: &[],
            },
            SegmentRecipe {
                vertices: 8,
                faces: 6,
                edges: &[],
                added: &[],
            },
        ],
    },
    Recipe {
        model: "tat1_bldg_pillar2",
        inputs: &[
            "5b5a37a1671f59842b4ef4b573e8eb5f2ecc6203e79b1f0c4afd7b5cae7565ca",
            "41e94a391e01c36d767975bd9a82260e4ec6bade225b1a863df7b0daab541a22",
        ],
        segments: &[
            SegmentRecipe {
                vertices: 149,
                faces: 207,
                edges: &[
                    [119, 129],
                    [127, 144],
                    [107, 134],
                    [108, 123],
                    [106, 126],
                    [105, 124],
                    [123, 137],
                    [134, 139],
                    [129, 147],
                    [135, 146],
                    [103, 108],
                    [91, 106],
                    [96, 105],
                    [100, 113],
                    [117, 128],
                    [65, 70],
                    [83, 90],
                    [70, 100],
                    [73, 96],
                    [81, 91],
                    [0, 39],
                    [2, 41],
                    [6, 42],
                    [10, 77],
                    [46, 63],
                    [48, 62],
                    [43, 64],
                    [5, 15],
                    [66, 73],
                    [131, 142],
                    [133, 142],
                ],
                added: &[],
            },
            SegmentRecipe {
                vertices: 46,
                faces: 42,
                edges: &[],
                added: &[],
            },
        ],
    },
    Recipe {
        model: "geo_bldg_greatspire_hq",
        inputs: &["5642319cab20e347b068d03a18c9ed0d2b2b70f7f079a89dc747aa56e0d78c69"],
        segments: &[
            SegmentRecipe {
                vertices: 499,
                faces: 726,
                edges: &[
                    [471, 491],
                    [462, 479],
                    [469, 475],
                    [447, 477],
                    [451, 473],
                    [440, 446],
                    [415, 446],
                    [380, 410],
                    [287, 331],
                    [438, 440],
                    [449, 468],
                    [286, 289],
                    [260, 273],
                    [249, 261],
                    [329, 340],
                    [372, 385],
                    [220, 240],
                    [213, 236],
                    [208, 233],
                    [244, 252],
                    [182, 210],
                    [72, 109],
                    [67, 93],
                    [54, 72],
                    [68, 70],
                    [68, 90],
                    [209, 212],
                    [205, 220],
                    [203, 255],
                    [193, 221],
                    [176, 193],
                    [194, 204],
                    [255, 314],
                    [350, 414],
                    [414, 443],
                    [398, 424],
                    [459, 483],
                    [476, 492],
                    [388, 450],
                    [169, 188],
                    [180, 196],
                    [189, 207],
                    [343, 372],
                    [477, 486],
                    [458, 473],
                    [448, 470],
                    [423, 466],
                    [403, 455],
                    [270, 305],
                    [455, 485],
                    [466, 480],
                    [371, 423],
                    [357, 403],
                    [342, 383],
                    [418, 441],
                    [467, 489],
                    [392, 467],
                    [386, 425],
                    [344, 384],
                    [288, 324],
                    [197, 227],
                    [206, 230],
                    [298, 319],
                    [349, 392],
                    [425, 476],
                    [354, 386],
                    [155, 163],
                    [117, 149],
                    [47, 74],
                    [82, 95],
                    [131, 150],
                    [141, 146],
                    [127, 142],
                    [112, 127],
                    [107, 126],
                    [84, 112],
                    [24, 33],
                    [36, 39],
                    [36, 64],
                    [19, 38],
                    [30, 38],
                    [37, 54],
                    [7, 8],
                    [13, 18],
                    [28, 35],
                    [32, 49],
                    [21, 22],
                    [102, 117],
                    [313, 343],
                    [301, 313],
                ],
                added: &[],
            },
            SegmentRecipe {
                vertices: 202,
                faces: 192,
                edges: &[[32, 46], [21, 42]],
                added: &[],
            },
            SegmentRecipe {
                vertices: 776,
                faces: 599,
                edges: &[[643, 647], [137, 165]],
                added: &[],
            },
            SegmentRecipe {
                vertices: 573,
                faces: 409,
                edges: &[[50, 123], [49, 121], [112, 148], [37, 111], [20, 29]],
                added: &[],
            },
        ],
    },
    Recipe {
        model: "tat1_bldg_rock_rubble_pillar",
        inputs: &["ef6b69b1da83aaab67eeca4688e0b307193e2c88806fe96758ae8d3680485156"],
        segments: &[
            SegmentRecipe {
                vertices: 186,
                faces: 253,
                edges: &[
                    [68, 90],
                    [45, 89],
                    [88, 123],
                    [119, 135],
                    [122, 137],
                    [94, 119],
                    [89, 120],
                    [132, 142],
                    [140, 170],
                    [142, 169],
                    [110, 133],
                    [118, 134],
                    [133, 141],
                    [130, 153],
                ],
                added: &[],
            },
            SegmentRecipe {
                vertices: 73,
                faces: 92,
                edges: &[],
                added: &[],
            },
        ],
    },
];

pub(super) struct Faces(HashSet<String>);

impl Faces {
    /// The corrected four-metre block must stay visible at camera fade edges.
    /// Selection still requires the measured raw donor hash.
    pub(super) fn contact_surface(&self, name: &str) -> bool {
        name.eq_ignore_ascii_case("rhn_bldg_lego_1") && self.0.contains(&name.to_ascii_lowercase())
    }

    pub(super) fn of(original: Option<&[u8]>) -> Result<Self> {
        let mut selected = HashSet::new();
        if let Some(original) = original {
            for chunk in mungefs::content_chunks(mungefs::open(original)?)? {
                if chunk.tag != *b"modl" {
                    continue;
                }
                let Some(name) = chunk.child_string(b"NAME") else {
                    continue;
                };
                if let Some(recipe) = RECIPES.iter().find(|r| r.model.eq_ignore_ascii_case(&name)) {
                    let hash = format!("{:x}", Sha256::digest(chunk.payload));
                    if recipe.inputs.contains(&hash.as_str()) {
                        selected.insert(name.to_ascii_lowercase());
                    }
                }
            }
        }
        Ok(Self(selected))
    }

    pub(super) fn apply(&self, model: &mut Model) -> Result<()> {
        if !self.0.contains(&model.name.to_ascii_lowercase()) {
            return Ok(());
        }
        let recipe = RECIPES
            .iter()
            .find(|r| r.model.eq_ignore_ascii_case(&model.name))
            .unwrap();
        ensure!(
            !model.bone_space && model.segments.len() == recipe.segments.len(),
            "render faces: {} posed segmentation changed",
            model.name
        );
        // Validate and edit copies first. A rejected later segment must not
        // leave earlier segments with a partially applied topology recipe.
        let mut result = Vec::new();
        for (segment, profile) in model.segments.iter().zip(recipe.segments) {
            ensure!(
                segment.positions.len() == profile.vertices
                    && segment.normals.len() == profile.vertices
                    && segment.uvs.len() == profile.vertices
                    && segment.triangles.len() == profile.faces,
                "render faces: {} donor streams changed",
                model.name
            );
            let mut triangles = segment.triangles.clone();
            for &edge in profile.edges {
                flip(&mut triangles, edge)?;
            }
            for &face in profile.added {
                ensure!(
                    face.iter().all(|&v| (v as usize) < profile.vertices)
                        && !triangles.iter().any(|t| face.iter().all(|v| t.contains(v))),
                    "render faces: invalid or existing closing face"
                );
                triangles.push(face);
            }
            result.push(triangles);
        }
        for (segment, triangles) in model.segments.iter_mut().zip(result) {
            segment.index_count = u32::try_from(triangles.len() * 3)?;
            segment.triangles = triangles;
        }
        eprintln!("render faces: {} donor-edge recipe", model.name);
        Ok(())
    }
}

pub(super) fn flip(triangles: &mut [[u32; 3]], edge: [u32; 2]) -> Result<()> {
    let selected: Vec<_> = triangles
        .iter()
        .enumerate()
        .filter(|(_, face)| edge.iter().all(|v| face.contains(v)))
        .map(|(index, _)| index)
        .collect();
    ensure!(
        selected.len() == 2,
        "render faces: edge {edge:?} needs exactly two faces"
    );
    let first = triangles[selected[0]];
    let second = triangles[selected[1]];
    let start = (0..3)
        .find(|&i| edge.contains(&first[i]) && edge.contains(&first[(i + 1) % 3]))
        .unwrap();
    let [a, b, c] = [first[start], first[(start + 1) % 3], first[(start + 2) % 3]];
    let opposite = (0..3).find(|&i| second[i] == b && second[(i + 1) % 3] == a);
    ensure!(
        opposite.is_some(),
        "render faces: inconsistent edge winding"
    );
    let d = second[(opposite.unwrap() + 2) % 3];
    ensure!(
        !edge.contains(&c) && !edge.contains(&d) && c != d,
        "render faces: edge does not bound four distinct corners"
    );
    triangles[selected[0]] = [c, d, b];
    triangles[selected[1]] = [d, c, a];
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_flip_keeps_oriented_boundary_and_uses_existing_corners() {
        let mut faces = [[0, 1, 2], [1, 0, 3]];
        flip(&mut faces, [0, 1]).unwrap();
        assert_eq!(faces, [[2, 3, 1], [3, 2, 0]]);
        let mut reversed_edge = [[0, 1, 2], [1, 0, 3]];
        flip(&mut reversed_edge, [1, 0]).unwrap();
        assert_eq!(faces, reversed_edge);
    }

    #[test]
    fn changed_topology_is_rejected_without_mutation() {
        for mut faces in [
            vec![[0, 1, 2]],
            vec![[0, 1, 2], [0, 1, 3]],
            vec![[0, 1, 2], [1, 0, 2]],
            vec![[0, 1, 2], [1, 0, 3], [0, 1, 4]],
        ] {
            let before = faces.clone();
            assert!(flip(&mut faces, [0, 1]).is_err());
            assert_eq!(faces, before);
        }
    }

    #[test]
    fn pure_reference_never_selects_donor_face_edits() {
        assert!(Faces::of(None).unwrap().0.is_empty());
    }
}
