//! Last-resort solid drawing for fingerprinted donor plants, tunnels and cliffs.
//!
//! Plant/tunnel fits, donor vertex projection and three subdivision levels
//! leave measured contact holes. The cliff also resists nearest-surface,
//! nearest-corner and directional projections. A dense reference-art cage would
//! store artwork, so it is not a public recipe.
//! This fallback reads only the player's already-corrected collision, projects
//! the player's own material/UVs onto it and leaves all simulation inputs
//! and model headers unchanged. Pure PS2 never selects a recipe.
//! The selected models have identical imported soldier and ordnance surfaces;
//! a union of different query surfaces would not prove either one's residual.

use anyhow::{anyhow, bail, ensure, Result};
use glam::{Mat3, Vec3};
use mungefs::{model::Model, Chunk};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

struct Recipe {
    name: &'static str,
    bf2: bool,
    inputs: &'static [&'static str],
}

const RECIPES: &[Recipe] = &[
    // Two missing upper cliff corners expose the hangar through its wall.
    // Donor projection with three subdivision levels still leaves >22 cm
    // extra contact gaps. Reuse only the player's canonical collision mesh,
    // with the donor material/UVs, as the last resort for this cliff model.
    Recipe {
        name: "uta_bldg_cliff_hanger",
        bf2: true,
        inputs: &["8e72b2cf177e2b6509cc226ace0b0fe6b0605dae28590deadf634db3b24edc91"],
    },
    // The donor cliff's upper lip is recessed by metres. Nearest-surface,
    // nearest-corner and outward-normal projections leave contact gaps above
    // two metres. Reuse the player's canonical cliff mesh with donor UVs;
    // no collision or reference-art vertex stream is stored here.
    Recipe {
        name: "uta1_bldg_rep_ground",
        bf2: true,
        inputs: &["e52c4caf140cc9a54fa1aeaf001a313866dc37a22e6e1126aea71c5a6fdbd638"],
    },
    Recipe {
        name: "fel1_bldg_pitcher_l",
        bf2: true,
        inputs: &[
            "987b756723460e437034ae46d3594ec14cd14abf66fbd5203d1c407afa5f641d",
            "679e130ba32271d320b3b3a634ba096355189872eb931dcfe892125a2b0b45e4",
        ],
    },
    Recipe {
        name: "fel1_bldg_pitcher_clump",
        bf2: true,
        inputs: &[
            "a052a91600aa1d4626d9f46b87c4b09d79d75e15496c1c32d3b3ef0c327cd1cd",
            "8e8f716d4f413a183f71b228e8245d4d05a52a5346349363d208ed80f50888b1",
        ],
    },
    Recipe {
        name: "fel1_bldg_whisker_clump01",
        bf2: true,
        inputs: &[
            "00435dd72b4609e3e0532527c04024d95d32bdbe2dc676231e53095fe33dfcf1",
            "034fb1719541a60b03c95a16fb997c5809ce62cd99281ea811316a9664718f1d",
        ],
    },
    Recipe {
        name: "fel1_bldg_pitcher_c",
        bf2: true,
        inputs: &[
            "dc4b6ddcca8e72eeb37643a67d504b3553033a7ee4ef3f54573ef2699d6a20e3",
            "6b0ce32af753d4f53c856f8435e36f620ca6b56b7fb7d5eb215b2b6dbca3f772",
        ],
    },
    Recipe {
        name: "fel1_bldg_pitcher_m",
        bf2: true,
        inputs: &[
            "9650936ac2813921ef0199af1e7b3240095b83d37da34baed6997d13d156415c",
            "259a6153fc6bb5a297ede49be0b9c0c9853c47c719b5e09d91d85c2fefe39bbb",
        ],
    },
    Recipe {
        name: "fel1_bldg_pitcher_s",
        bf2: true,
        inputs: &[
            "faa1cb4c0c84a2037a1c5a40969ccd72f4dd03b049af8d84f8f9c65795228e91",
            "0f764128b400bd89d5eb0656883b71020ab3e8105bcb0adbf3ff43dd6b9f70dc",
        ],
    },
    // The donor special tunnel has a narrower, taller floor plus missing wall
    // corners. Model/group fits and canonical projection through three donor
    // subdivision levels still leave >14 cm contact witnesses. This last-resort
    // recipe reads only the player's canonical mesh and retains donor sheets.
    Recipe {
        name: "rhn1_prop_tunnel_special_01",
        bf2: false,
        inputs: &[
            "f2975ca1dcef985a75befd0625ccfd1d0c34ad29b37f5651872752280f4ae7d1",
            "12d98bfa1541004f42de539d440af4bce8402141a31bd84f5d1aafb1565fc0a6",
        ],
    },
    // These remaining tunnel types retain measured holes after donor-only
    // edge changes and nearest/vertical floor or wall projection. Even a
    // canonical floor plus donor-wall edge changes leaves a 15 mm reverse
    // straight_6 witness. Use the player's active mesh only as
    // the last resort, with unchanged donor textures and canonical headers.
    Recipe {
        name: "rhn1_prop_tunnel_height",
        bf2: false,
        inputs: &[
            "533710ca222b655d4fbc55d7d3adc2089a484a5ea8235f65dc6a38a793d3a376",
            "cb60cf474128f7ea369a94a86e728ac79929029f4f9e3be83754903e09ae9a04",
        ],
    },
    Recipe {
        name: "rhn1_prop_tunnel_t",
        bf2: false,
        inputs: &[
            "550a1c79ff441f5a59df80bb40b09d611acfa6ba5a61eed3d7f860e755a0cbc1",
            "98cc3435dc0a99a64acd72624ddedd7a9e0d234355081c29609b16c91d6835a8",
        ],
    },
    Recipe {
        name: "rhn1_prop_tunnel_straight_6",
        bf2: false,
        inputs: &[
            "6dd75bae0d4d758fdd9a10d597f325bd4f0df155a3eca0abf1629dcd48c20315",
            "f23aa7def44edb3d118a9a7e9e69313731a29797b1d4f105092c7b4d943354d0",
        ],
    },
];

// A new cliff wall can lie beyond the donor face. Clamped nearest-point UVs
// collapse that extension into stripes. Reuse an aligned donor plane's UV
// gradient so its texture continues at the donor's own scale.
fn plane_uv(point: Vec3, triangle: [Vec3; 3], uv: [[f32; 2]; 3]) -> Option<[f32; 2]> {
    let ab = triangle[1] - triangle[0];
    let ac = triangle[2] - triangle[0];
    let ap = point - triangle[0];
    let aa = ab.dot(ab);
    let cc = ac.dot(ac);
    let cross = ab.dot(ac);
    let denominator = aa * cc - cross * cross;
    if denominator.abs() < 1e-8 {
        return None;
    }
    let u = (cc * ap.dot(ab) - cross * ap.dot(ac)) / denominator;
    let v = (aa * ap.dot(ac) - cross * ap.dot(ab)) / denominator;
    Some(std::array::from_fn(|axis| {
        uv[0][axis] + u * (uv[1][axis] - uv[0][axis]) + v * (uv[2][axis] - uv[0][axis])
    }))
}

fn aligned_face(center: Vec3, normal: Vec3, model: &Model) -> Option<(usize, [u32; 3])> {
    let mut best = None;
    for (si, segment) in model.segments.iter().enumerate() {
        for &face in &segment.triangles {
            let points = face.map(|i| Vec3::from(segment.positions[i as usize]));
            let direction = (points[1] - points[0])
                .cross(points[2] - points[0])
                .normalize_or_zero();
            if direction.dot(normal).abs() < 0.75 {
                continue;
            }
            let distance = center.distance_squared((points[0] + points[1] + points[2]) / 3.);
            if best.is_none_or(|(old, _, _)| distance < old) {
                best = Some((distance, si, face));
            }
        }
    }
    best.map(|(_, si, face)| (si, face))
}

pub(super) struct Conformation(HashMap<String, bool>);

impl Conformation {
    /// The chamber recipe uses the existing guarded solid-surface producer,
    /// then keeps only the annex; its caller retains the untouched donor core.
    pub(super) fn chamber_surface(
        artwork: &Model,
        items: &[Chunk<'_>],
        alpha: &[crate::alpha::SegmentAlpha],
    ) -> Result<Model> {
        ensure!(
            artwork.name.eq_ignore_ascii_case("bes2_bldg_chamber")
                || artwork.name.eq_ignore_ascii_case("bes2_bldg_chamberLOWD"),
            "render chamber: unexpected model"
        );
        let mut result = artwork.clone();
        result.name = "bes2_bldg_chamber".into();
        let selected = Self(HashMap::from([(result.name.clone(), false)]));
        selected.apply(&mut result, items, alpha)?;
        result.name.clone_from(&artwork.name);
        Ok(result)
    }
    pub(super) fn of(original: Option<&[u8]>) -> Result<Self> {
        let mut selected = HashMap::new();
        if let Some(original) = original {
            for chunk in mungefs::content_chunks(mungefs::open(original)?)? {
                if chunk.tag != *b"modl" {
                    continue;
                }
                let Some(name) = chunk.child_string(b"NAME") else {
                    continue;
                };
                let Some(recipe) = RECIPES.iter().find(|r| r.name.eq_ignore_ascii_case(&name))
                else {
                    continue;
                };
                let hash = format!("{:x}", Sha256::digest(chunk.payload));
                if recipe.inputs.contains(&hash.as_str()) {
                    selected.insert(name.to_ascii_lowercase(), recipe.bf2);
                }
            }
        }
        Ok(Self(selected))
    }

    pub(super) fn apply(
        &self,
        artwork: &mut Model,
        items: &[Chunk<'_>],
        original_alpha: &[crate::alpha::SegmentAlpha],
    ) -> Result<bool> {
        let Some(&bf2) = self.0.get(&artwork.name.to_ascii_lowercase()) else {
            return Ok(false);
        };
        ensure!(
            !artwork.bone_space,
            "render conform: {} is not posed",
            artwork.name
        );
        ensure!(
            original_alpha.len() == artwork.segments.len()
                && (bf2
                    || original_alpha
                        .iter()
                        .all(|a| a.mode == crate::alpha::AlphaMode::Opaque)),
            "render conform: BF1 donor must already draw every segment opaque"
        );
        validate_classes(&artwork.name, items)?;
        let faces = shapes(&artwork.name, items)?;
        ensure!(
            !faces.is_empty(),
            "render conform: {} has no active collision",
            artwork.name
        );
        let planar_cliff = artwork.name.eq_ignore_ascii_case("uta_bldg_cliff_hanger");
        let mut result = artwork.clone();
        result.segments = artwork
            .segments
            .iter()
            .map(super::render_recovery::empty_segment)
            .collect();
        for segment in &mut result.segments {
            ensure!(
                segment.bf2_material == bf2,
                "render conform: changed donor material family"
            );
            // These are solid collision surfaces, not translucent leaf cards.
            // Keep source texture alpha bytes separately, but do not let them
            // punch contact holes through the generated drawing.
            segment.material_flags &= if bf2 { !0x206 } else { !0x46 };
            segment.colors = None;
        }
        for triangle in faces {
            let normal = (triangle[1] - triangle[0])
                .cross(triangle[2] - triangle[0])
                .normalize_or_zero();
            if normal == Vec3::ZERO {
                continue;
            }
            let center = (triangle[0] + triangle[1] + triangle[2]) / 3.;
            let (si, donor_face, _) = super::collision_standin::nearest(center, artwork, None)
                .ok_or_else(|| {
                    anyhow!("render conform: {} has no donor triangles", artwork.name)
                })?;
            let (si, donor_face) = if planar_cliff {
                aligned_face(center, normal, artwork).unwrap_or((si, donor_face))
            } else {
                (si, donor_face)
            };
            let donor = &artwork.segments[si];
            let donor_normal = donor_face
                .into_iter()
                .map(|v| Vec3::from(donor.normals[v as usize]))
                .sum::<Vec3>();
            let normal = if normal.dot(donor_normal) < 0. {
                -normal
            } else {
                normal
            };
            let output = &mut result.segments[si];
            let first = u32::try_from(output.positions.len())?;
            for point in triangle {
                let uv = if planar_cliff {
                    plane_uv(
                        point,
                        donor_face.map(|i| Vec3::from(donor.positions[i as usize])),
                        donor_face.map(|i| donor.uvs[i as usize]),
                    )
                    .ok_or_else(|| anyhow!("render conform: degenerate donor UV plane"))?
                } else {
                    let (_, face, weights) =
                        super::collision_standin::nearest(point, artwork, Some(si)).unwrap();
                    std::array::from_fn(|axis| {
                        (0..3)
                            .map(|i| donor.uvs[face[i] as usize][axis] * weights[i])
                            .sum()
                    })
                };
                output.positions.push(point.to_array());
                output.normals.push(normal.to_array());
                output.uvs.push(uv);
            }
            output.triangles.push([first, first + 1, first + 2]);
        }
        result
            .segments
            .retain(|segment| !segment.triangles.is_empty());
        ensure!(
            !result.segments.is_empty(),
            "render conform: {} has no nondegenerate surface",
            artwork.name
        );
        for segment in &mut result.segments {
            segment.vertex_count = u32::try_from(segment.positions.len())?;
            segment.index_count = u32::try_from(segment.triangles.len() * 3)?;
        }
        eprintln!(
            "render conform: {}: donor-textured active collision surface",
            artwork.name
        );
        *artwork = result;
        Ok(true)
    }
}

fn validate_classes(name: &str, items: &[Chunk<'_>]) -> Result<()> {
    let mut found = false;
    for item in items.iter().filter(|item| item.tag == *b"entc") {
        let class = mungefs::entity::EntityClass::parse(item).map_err(|e| anyhow!(e))?;
        if class
            .geometry_name()
            .is_some_and(|g| g.eq_ignore_ascii_case(name))
        {
            found = true;
            ensure!(
                class.base.eq_ignore_ascii_case("prop") && class.collision_layers().is_empty(),
                "render conform: {name} requires reviewed static prop/default body lists"
            );
        }
    }
    ensure!(found, "render conform: {name} has no owning class");
    Ok(())
}

fn shapes(name: &str, items: &[Chunk<'_>]) -> Result<Vec<[Vec3; 3]>> {
    let bind = crate::bind_pose::BindPose::of(items);
    let mut faces = Vec::new();
    let mut mesh_seen = false;
    let mut prims_seen = false;
    for item in items {
        if item.tag == *b"coll"
            && item
                .child_string(b"NAME")
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
            && !mesh_seen
        {
            mesh_seen = true;
            let mesh = mungefs::coll::CollisionMesh::parse(item).map_err(|e| anyhow!(e))?;
            faces.extend(
                mesh.triangles()
                    .into_iter()
                    .map(|t| t.map(|i| Vec3::from(mesh.vertices[i as usize]))),
            );
        } else if item.tag == *b"prim" {
            let prims = mungefs::coll::Primitives::parse(item).map_err(|e| anyhow!(e))?;
            if !prims.model.eq_ignore_ascii_case(name) || prims_seen {
                continue;
            }
            prims_seen = true;
            let (mut prims, unknown) = super::manifest_prims(&prims);
            ensure!(
                unknown == 0,
                "render conform: {name} has unsupported primitive kinds"
            );
            bind.rest_prims(name, &mut prims);
            // Empty class lists leave the MASK seeds in effect. A shared
            // solid surface must cover the same imported bodies for each
            // query, rather than silently certifying their combined union.
            for prim in prims {
                let soldier = prim.mask == 0 || prim.mask & 1 != 0;
                let ordnance = prim.mask == 0 || prim.mask & 16 != 0;
                ensure!(
                    soldier == ordnance,
                    "render conform: {name} has different soldier and ordnance surfaces"
                );
                if soldier {
                    faces.extend(primitive_faces(&prim)?);
                }
            }
        }
    }
    ensure!(
        faces.iter().flatten().all(|p| p.is_finite()),
        "render conform: nonfinite collision surface"
    );
    Ok(faces)
}

fn cylinder_sides(radius: f32) -> Result<u32> {
    ensure!(
        radius.is_finite() && radius >= 0.,
        "render conform: invalid cylinder radius"
    );
    let mut sides = 16;
    // A full circular arc and its chord differ by r*(1-cos(pi/n)). Caps
    // use complete fans of the same ring. The gate independently charges
    // this approximation plus frame/float error; this is not a sampled bound.
    while radius * (1. - (std::f32::consts::PI / sides as f32).cos()) > 0.001 {
        sides *= 2;
        ensure!(
            sides <= 1024,
            "render conform: excessive cylinder tessellation"
        );
    }
    Ok(sides)
}

fn primitive_faces(prim: &manifest::ManifestPrim) -> Result<Vec<[Vec3; 3]>> {
    ensure!(
        prim.dims.iter().all(|x| x.is_finite() && *x >= 0.),
        "render conform: invalid primitive dimensions"
    );
    let mut faces = Vec::new();
    match prim.kind.as_str() {
        "box" => {
            let points: Vec<_> = (0..8)
                .map(|i| {
                    let point: [f32; 3] = std::array::from_fn(|axis| {
                        if i & (1 << axis) == 0 {
                            -prim.dims[axis]
                        } else {
                            prim.dims[axis]
                        }
                    });
                    Vec3::from(point)
                })
                .collect();
            for q in [
                [0, 4, 6, 2],
                [1, 3, 7, 5],
                [0, 1, 5, 4],
                [2, 6, 7, 3],
                [0, 2, 3, 1],
                [4, 5, 7, 6],
            ] {
                faces.extend([
                    [points[q[0]], points[q[1]], points[q[2]]],
                    [points[q[0]], points[q[2]], points[q[3]]],
                ]);
            }
        }
        "cylinder" => {
            let sides = cylinder_sides(prim.dims[0])?;
            let half = prim.dims[1] * 0.5;
            let point = |i: u32, y| {
                let angle = (i % sides) as f32 * std::f32::consts::TAU / sides as f32;
                Vec3::new(prim.dims[0] * angle.cos(), y, prim.dims[0] * angle.sin())
            };
            for i in 0..sides {
                let [a, b, c, d] = [
                    point(i, -half),
                    point(i + 1, -half),
                    point(i, half),
                    point(i + 1, half),
                ];
                faces.extend([
                    [a, c, d],
                    [a, d, b],
                    [Vec3::new(0., -half, 0.), a, b],
                    [Vec3::new(0., half, 0.), d, c],
                ]);
            }
        }
        kind => bail!("render conform: unreviewed primitive kind {kind}"),
    }
    let rotation = Mat3::from_cols_array_2d(&prim.rotation);
    let translation = Vec3::from(prim.translation);
    Ok(faces
        .into_iter()
        .map(|triangle| triangle.map(|point| rotation * point + translation))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::model::{Aabb, RenderType, Segment};

    #[test]
    fn projected_cliff_uv_keeps_texture_density_beyond_the_donor_triangle() {
        let triangle = [Vec3::ZERO, Vec3::X, Vec3::Y];
        let uv = [[0., 0.], [1., 0.], [0., 1.]];
        assert_eq!(
            plane_uv(Vec3::new(3., 5., 2.), triangle, uv),
            Some([3., 5.])
        );
        assert_eq!(plane_uv(Vec3::ZERO, [Vec3::ZERO; 3], uv), None);
    }

    fn conform(selected: &Conformation, model: &mut Model, items: &[Chunk<'_>]) -> Result<bool> {
        let alpha = crate::alpha::AlphaTable::new(&[]).model(model);
        selected.apply(model, items, &alpha)
    }

    fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut bytes = tag.to_vec();
        bytes.extend((body.len() as u32).to_le_bytes());
        bytes.extend(body);
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        bytes
    }

    fn class(override_layer: bool) -> Vec<u8> {
        let mut geometry = 0x47c8_6b4au32.to_le_bytes().to_vec();
        geometry.extend(b"m\0");
        let mut body = [
            chunk(b"TYPE", b"m\0"),
            chunk(b"BASE", b"prop\0"),
            chunk(b"PROP", &geometry),
        ]
        .concat();
        if override_layer {
            let mut property = 0x5dfd_c07fu32.to_le_bytes().to_vec();
            property.extend(b"clear\0");
            body.extend(chunk(b"PROP", &property));
        }
        body
    }

    fn prims(active_mask: u8) -> Vec<u8> {
        let mut info = b"m\0".to_vec();
        info.extend(2u32.to_le_bytes());
        let mut body = chunk(b"INFO", &info);
        let mut data = 4u32.to_le_bytes().to_vec();
        data.extend([0.5f32; 3].iter().flat_map(|f| f.to_le_bytes()));
        for (name, mask) in [
            (b"active\0".as_slice(), active_mask),
            (b"unused\0".as_slice(), 2),
        ] {
            body.extend(chunk(b"NAME", name));
            body.extend(chunk(b"MASK", &[mask]));
            body.extend(chunk(b"DATA", &data));
        }
        body
    }

    fn artwork() -> Model {
        Model {
            name: "m".into(),
            bone_space: false,
            vertex_box: Aabb {
                min: [-7.; 3],
                max: [8.; 3],
            },
            vis_box: Aabb {
                min: [-9.; 3],
                max: [10.; 3],
            },
            face_count: 123,
            sphere: Some([8., 9., 10., 11.]),
            segments: vec![Segment {
                vertex_count: 3,
                index_count: 3,
                bf2_material: true,
                pc_geometry: false,
                material_flags: 0x203,
                render_type: RenderType::Bf2(0),
                spec_power: 0,
                spec_color: 0,
                uv_anim: [0; 2],
                textures: vec![(0, "donor_only_texture".into())],
                bone: None,
                group: None,
                bmap: vec![],
                bind: vec![],
                wght: vec![],
                positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
                normals: vec![[0., 0., 1.]; 3],
                uvs: vec![[0., 0.], [1., 0.], [0., 1.]],
                colors: Some(vec![[32, 64, 128, 255]; 3]),
                triangles: vec![[0, 1, 2]],
            }],
        }
    }

    #[test]
    fn reference_and_unrelated_inputs_do_not_select_recipes() {
        assert!(Conformation::of(None).unwrap().0.is_empty());
        let bytes = chunk(b"ucfb", &chunk(b"modl", &chunk(b"NAME", b"other\0")));
        assert!(Conformation::of(Some(&bytes)).unwrap().0.is_empty());
    }

    #[test]
    fn source_attributes_supply_uvs_while_headers_and_inputs_stay_unchanged() {
        let class = class(false);
        let prims = prims(17);
        let items = [
            Chunk {
                tag: *b"entc",
                payload: &class,
                offset: 0,
            },
            Chunk {
                tag: *b"prim",
                payload: &prims,
                offset: 0,
            },
        ];
        let source = artwork();
        let mut model = source.clone();
        let selected = Conformation(HashMap::from([("m".into(), true)]));
        assert!(conform(&selected, &mut model, &items).unwrap());
        assert_eq!(model.vertex_box.min, source.vertex_box.min);
        assert_eq!(model.vertex_box.max, source.vertex_box.max);
        assert_eq!(model.vis_box.min, source.vis_box.min);
        assert_eq!(model.vis_box.max, source.vis_box.max);
        assert_eq!(model.face_count, source.face_count);
        assert_eq!(model.sphere, source.sphere);
        assert_eq!(model.segments[0].textures, source.segments[0].textures);
        assert_eq!(model.segments[0].triangles.len(), 12); // MASK=2 contributed none.
        assert!(model.segments[0]
            .positions
            .iter()
            .all(|p| p.iter().all(|v| v.abs() == 0.5)));
        assert!(model.segments[0]
            .uvs
            .iter()
            .all(|uv| uv[0] >= 0. && uv[1] >= 0. && uv[0] + uv[1] <= 1.000001));
        assert!(model.segments[0].colors.is_none());
        assert_eq!(
            crate::alpha::AlphaTable::new(&[]).model(&model)[0].mode,
            crate::alpha::AlphaMode::Opaque
        );
        // Borrowed collision bytes were read, never rebuilt or changed.
        assert_eq!(prims, self::prims(17));
    }

    #[test]
    fn a_soldier_only_cage_cannot_certify_the_ordnance_surface() {
        let class = class(false);
        let prims = prims(1);
        let items = [
            Chunk {
                tag: *b"entc",
                payload: &class,
                offset: 0,
            },
            Chunk {
                tag: *b"prim",
                payload: &prims,
                offset: 0,
            },
        ];
        let mut model = artwork();
        let original = model.segments[0].positions.clone();
        assert!(conform(
            &Conformation(HashMap::from([("m".into(), true)])),
            &mut model,
            &items
        )
        .is_err());
        assert_eq!(model.segments[0].positions, original);
    }

    #[test]
    fn changed_class_body_lists_fail_before_any_art_mutation() {
        let body = class(true);
        let items = [Chunk {
            tag: *b"entc",
            payload: &body,
            offset: 0,
        }];
        let mut model = artwork();
        let original_positions = model.segments[0].positions.clone();
        assert!(conform(
            &Conformation(HashMap::from([("m".into(), true)])),
            &mut model,
            &items
        )
        .is_err());
        assert_eq!(model.segments[0].positions, original_positions);
    }

    #[test]
    fn bf1_material_family_is_explicit_and_wrong_family_fails_atomically() {
        let class = class(false);
        let prims = prims(17);
        let items = [
            Chunk {
                tag: *b"entc",
                payload: &class,
                offset: 0,
            },
            Chunk {
                tag: *b"prim",
                payload: &prims,
                offset: 0,
            },
        ];
        let selected = Conformation(HashMap::from([("m".into(), false)]));
        let mut wrong = artwork();
        let original = wrong.segments[0].positions.clone();
        assert!(conform(&selected, &mut wrong, &items).is_err());
        assert_eq!(wrong.segments[0].positions, original);
        let mut model = artwork();
        model.segments[0].bf2_material = false;
        model.segments[0].render_type = RenderType::Bf1("Normal".into());
        model.segments[0].material_flags = 0x41;
        assert!(conform(&selected, &mut model, &items).unwrap());
        assert!(!model.segments[0].bf2_material);
        assert_eq!(model.segments[0].material_flags & 0x46, 0);
        assert_eq!(model.segments[0].triangles.len(), 12);
        assert_eq!(
            crate::alpha::AlphaTable::new(&[]).model(&model)[0].mode,
            crate::alpha::AlphaMode::Opaque
        );
        let original_positions = model.segments[0].positions.clone();
        let mut alpha = crate::alpha::AlphaTable::new(&[]).model(&model);
        alpha[0].mode = crate::alpha::AlphaMode::Mask;
        assert!(selected.apply(&mut model, &items, &alpha).is_err());
        assert_eq!(model.segments[0].positions, original_positions);
    }

    #[test]
    fn ps2_selection_is_empty_even_for_a_supported_model_name() {
        let selected = Conformation::of(None).unwrap();
        for recipe in RECIPES {
            let mut model = artwork();
            model.name = recipe.name.into();
            model.bone_space = true;
            let original = model.segments[0].positions.clone();
            assert!(!conform(&selected, &mut model, &[]).unwrap());
            assert_eq!(model.segments[0].positions, original);
        }
    }

    #[test]
    fn cylinder_resolution_has_an_analytic_millimetre_budget() {
        for radius in [0., 0.1, 0.25, 0.65, 0.75, 2., 16.] {
            let sides = cylinder_sides(radius).unwrap();
            let bound = radius as f64 * (1. - (std::f64::consts::PI / sides as f64).cos());
            assert!(bound <= 0.0010001, "r={radius}, n={sides}, error={bound}");
        }
        assert!(cylinder_sides(f32::NAN).is_err());
        assert!(cylinder_sides(-1.).is_err());
    }
}
