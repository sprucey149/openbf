//! Recover presentation resources whose PS2 object partition differs from the
//! donor's. Collision and world records stay unchanged. Exact named resources
//! from another LVL take precedence. A proven-absent piece can use a local
//! stand-in shaped by corrected collision and textured from placed donor art.

use anyhow::{anyhow, Context, Result};
use glam::{Mat3, Vec3};
use mungefs::{
    model::{Aabb, Model, Segment},
    world::{Transform, World},
    Chunk,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
};

fn chunks(bytes: &[u8]) -> Result<Vec<Chunk<'_>>> {
    mungefs::content_chunks(mungefs::open(bytes).map_err(|e| anyhow!("{e}"))?)
        .map_err(|e| anyhow!("{e}"))
}

fn named(c: &Chunk<'_>) -> Option<String> {
    c.child_string(b"NAME").map(|n| n.to_ascii_lowercase())
}

fn geometry_names(items: &[Chunk<'_>]) -> BTreeSet<String> {
    items
        .iter()
        .filter(|c| &c.tag == b"entc")
        .filter_map(|c| mungefs::entity::EntityClass::parse(c).ok())
        .flat_map(|e| {
            e.props
                .into_iter()
                .filter(|p| p.key_hash == 0x47c8_6b4a)
                .map(|p| p.value.to_ascii_lowercase())
        })
        .collect()
}

fn files(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            files(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("lvl"))
        {
            out.push(path);
        }
    }
    Ok(())
}

fn append_chunk(out: &mut Vec<u8>, c: &Chunk<'_>) {
    out.extend(c.tag);
    out.extend((c.payload.len() as u32).to_le_bytes());
    out.extend(c.payload);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn recovery_names() -> Result<BTreeSet<String>> {
    let bounds: BTreeMap<String, serde_json::Value> = serde_json::from_str(include_str!(
        "../../../manifest/data/gameplay/render-bounds-v1.json"
    ))?;
    let mut names: BTreeSet<_> = bounds.into_keys().collect();
    names.extend([
        "rhn1_bldg_brokencorner".to_owned(),
        "rhn_bldg_basebrick_1".to_owned(),
    ]);
    Ok(names)
}

/// Render-only resources, never classes, placements, collision or skeletons.
/// A full sorted LVL inventory handles shared, common and side directories as
/// well as map siblings. File/model names are compared without case.
pub(super) fn shared(lvl: &str, items: &[Chunk<'_>]) -> Result<Vec<u8>> {
    let allowed = recovery_names()?;
    let known: BTreeSet<_> = items
        .iter()
        .filter(|c| &c.tag == b"modl")
        .filter_map(named)
        .collect();
    let geometries: BTreeMap<_, _> = items
        .iter()
        .filter(|c| &c.tag == b"gmod")
        .filter_map(|c| {
            let g = mungefs::entity::GeomModel::parse(c).ok()?;
            Some((
                g.name.to_ascii_lowercase(),
                g.best_model()?.to_ascii_lowercase(),
            ))
        })
        .collect();
    let mut wanted: BTreeSet<_> = geometry_names(items)
        .into_iter()
        .filter(|name| allowed.contains(name))
        .filter_map(|geometry| {
            let model = geometries.get(&geometry).unwrap_or(&geometry);
            (!known.contains(model)).then(|| model.clone())
        })
        .collect();
    let known_textures: BTreeSet<_> = items
        .iter()
        .filter(|c| &c.tag == b"tex_")
        .filter_map(named)
        .collect();
    let wanted_textures: BTreeSet<_> = items
        .iter()
        .filter(|c| &c.tag == b"modl")
        .filter_map(|c| mungefs::model::Model::parse(c).ok())
        .flat_map(|m| m.segments)
        .flat_map(|s| s.textures)
        .map(|(_, n)| n.to_ascii_lowercase())
        // Jabba's placed Gamorrean axe uses the identical named sheet from
        // this install's SIDE/GAM. Do not import unrelated optional slots.
        .filter(|n| n == "gam_1st_weap_inf_axe" && !known_textures.contains(n))
        .collect();
    if wanted.is_empty() && wanted_textures.is_empty() {
        return Ok(Vec::new());
    }
    let Some(root) = Path::new(lvl).ancestors().skip(1).find(|p| {
        crate::join_ci(p, "CORE.LVL").is_file() && crate::join_ci(p, "MISSION.LVL").is_file()
    }) else {
        return Ok(Vec::new());
    };
    let mut paths = Vec::new();
    files(root, &mut paths)?;
    paths.sort();
    let mut body = Vec::new();
    let mut imported = BTreeSet::new();
    // A gmod may point to a differently named model occurring earlier in the
    // sorted inventory, so repeat only while a newly found dependency needs it.
    loop {
        let before = imported.len();
        for path in &paths {
            if path == Path::new(lvl) {
                continue;
            }
            let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
            let content = chunks(&bytes).with_context(|| path.display().to_string())?;
            let mut took = false;
            for c in &content {
                if &c.tag == b"tex_" {
                    if let Some(name) = named(c).filter(|n| wanted_textures.contains(n)) {
                        if imported.insert((c.tag, name.clone())) {
                            append_chunk(&mut body, c);
                            eprintln!("render recovery: exact tex_ {name} from {}", path.display());
                        }
                    }
                    continue;
                }
                if !matches!(&c.tag, b"modl" | b"gmod") {
                    continue;
                }
                let Some(name) = named(c) else {
                    continue;
                };
                if !wanted.contains(&name) || !imported.insert((c.tag, name.clone())) {
                    continue;
                }
                if &c.tag == b"gmod" {
                    let g = mungefs::entity::GeomModel::parse(c).map_err(|e| anyhow!("{e}"))?;
                    wanted.extend(g.lods.into_iter().map(|l| l.model.to_ascii_lowercase()));
                }
                append_chunk(&mut body, c);
                took = true;
                eprintln!(
                    "render recovery: exact {} {} from {}",
                    String::from_utf8_lossy(&c.tag),
                    name,
                    path.display()
                );
            }
            if took {
                // The normal map walk deduplicates these against local sheets.
                for c in content.iter().filter(|c| &c.tag == b"tex_") {
                    append_chunk(&mut body, c);
                }
            }
        }
        if before == imported.len() {
            break;
        }
    }
    Ok(body)
}

fn matrix(t: &Transform) -> Mat3 {
    Mat3::from_cols_array_2d(&t.rotation)
}
fn position(t: &Transform, p: Vec3) -> Vec3 {
    matrix(t) * p + Vec3::from(t.translation)
}

fn measured_bounds(name: &str) -> Result<Option<Aabb>> {
    #[derive(serde::Deserialize)]
    struct Bounds {
        min: [f32; 3],
        max: [f32; 3],
    }
    let measured: BTreeMap<String, Bounds> = serde_json::from_str(include_str!(
        "../../../manifest/data/gameplay/render-bounds-v1.json"
    ))?;
    if let Some(bounds) = measured.get(name) {
        return Ok(Some(Aabb {
            min: bounds.min,
            max: bounds.max,
        }));
    }
    Ok(None)
}

fn bounds_for(name: &str, items: &[Chunk<'_>]) -> Result<Option<Aabb>> {
    if let Some(bounds) = measured_bounds(name)? {
        return Ok(Some(bounds));
    }
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for c in items {
        if &c.tag == b"coll" && named(c).as_deref() == Some(name) {
            let mesh = mungefs::coll::CollisionMesh::parse(c).map_err(|e| anyhow!("{e}"))?;
            low = low.min(mesh.bbox.0.into());
            high = high.max(mesh.bbox.1.into());
        } else if &c.tag == b"prim" {
            let prims = mungefs::coll::Primitives::parse(c).map_err(|e| anyhow!("{e}"))?;
            if !prims.model.eq_ignore_ascii_case(name) {
                continue;
            }
            for prim in prims.prims {
                use mungefs::coll::PrimKind;
                let half = match prim.kind {
                    PrimKind::Box { half } => Vec3::from(half),
                    PrimKind::Sphere { radius } => Vec3::splat(radius),
                    PrimKind::Cylinder { radius, height } => {
                        Vec3::new(radius, height * 0.5, radius)
                    }
                    PrimKind::Unknown(_) => continue,
                };
                let rotation = Mat3::from_cols_array_2d(&prim.rotation);
                let half = rotation.x_axis.abs() * half.x
                    + rotation.y_axis.abs() * half.y
                    + rotation.z_axis.abs() * half.z;
                let center = Vec3::from(prim.translation);
                low = low.min(center - half);
                high = high.max(center + half);
            }
        }
    }
    Ok(low.is_finite().then_some(Aabb {
        min: low.to_array(),
        max: high.to_array(),
    }))
}

#[derive(Clone, Copy)]
pub(super) struct Vertex {
    pub(super) p: Vec3,
    pub(super) n: Vec3,
    pub(super) uv: [f32; 2],
    pub(super) color: [f32; 4],
}

impl Vertex {
    fn lerp(self, b: Self, t: f32) -> Self {
        Self {
            p: self.p.lerp(b.p, t),
            n: self.n.lerp(b.n, t),
            uv: std::array::from_fn(|i| self.uv[i] + (b.uv[i] - self.uv[i]) * t),
            color: std::array::from_fn(|i| self.color[i] + (b.color[i] - self.color[i]) * t),
        }
    }
}

pub(super) fn clip(mut polygon: Vec<Vertex>, min: Vec3, max: Vec3) -> Vec<Vertex> {
    for axis in 0..3 {
        for (edge, sign) in [(min[axis], 1.0), (max[axis], -1.0)] {
            if polygon.is_empty() {
                return polygon;
            }
            let mut next = Vec::new();
            let mut a = *polygon.last().unwrap();
            let mut da = (a.p[axis] - edge) * sign;
            for &b in &polygon {
                let db = (b.p[axis] - edge) * sign;
                if (da >= 0.0) != (db >= 0.0) {
                    next.push(a.lerp(b, da / (da - db)));
                }
                if db >= 0.0 {
                    next.push(b);
                }
                a = b;
                da = db;
            }
            polygon = next;
        }
    }
    polygon
}

pub(super) fn empty_segment(source: &Segment) -> Segment {
    let mut result = source.clone();
    result.positions.clear();
    result.normals.clear();
    result.uvs.clear();
    result.triangles.clear();
    result.colors = source.colors.as_ref().map(|_| Vec::new());
    result.bmap.clear();
    result.bind.clear();
    result.wght.clear();
    result.bone = None;
    result
}

pub(super) fn clipped(
    source: &Segment,
    transform: Mat3,
    translation: Vec3,
    bounds: Aabb,
) -> Option<Segment> {
    let mut result = empty_segment(source);
    let normal = transform.inverse().transpose();
    for triangle in &source.triangles {
        let vertices = triangle
            .iter()
            .map(|&i| {
                let i = i as usize;
                Vertex {
                    p: transform * Vec3::from(source.positions[i]) + translation,
                    n: normal * Vec3::from(source.normals[i]),
                    uv: source.uvs[i],
                    color: source
                        .colors
                        .as_ref()
                        .map_or([255.0; 4], |v| v[i].map(f32::from)),
                }
            })
            .collect();
        let polygon = clip(vertices, bounds.min.into(), bounds.max.into());
        if polygon.len() < 3 {
            continue;
        }
        let base = result.positions.len() as u32;
        for vertex in &polygon {
            result.positions.push(vertex.p.to_array());
            result.normals.push(vertex.n.normalize_or_zero().to_array());
            result.uvs.push(vertex.uv);
            if let Some(colors) = &mut result.colors {
                colors.push(vertex.color.map(|v| v.round().clamp(0.0, 255.0) as u8));
            }
        }
        for i in 1..polygon.len() - 1 {
            if (polygon[i].p - polygon[0].p)
                .cross(polygon[i + 1].p - polygon[0].p)
                .length_squared()
                > 1e-12
            {
                result
                    .triangles
                    .push([base, base + i as u32, base + i as u32 + 1]);
            }
        }
    }
    result.vertex_count = result.positions.len() as u32;
    result.index_count = (result.triangles.len() * 3) as u32;
    (!result.triangles.is_empty()).then_some(result)
}

/// A donor map can merge objects that the PS2 splits into many classes. Build
/// only the missing render pieces from the donor's original placed surfaces;
/// use corrected-collision stand-ins only for the measured absent-model list,
/// with materials/UVs from the donor. The collision resources stay unchanged.
pub(super) fn pieces(
    original: &[u8],
    items: &[Chunk<'_>],
    worlds: &[World],
    models: &HashMap<String, Model>,
) -> Result<Vec<Model>> {
    let donor = chunks(original)?;
    let pc_source = donor
        .iter()
        .any(|c| &c.tag == b"tex_" && c.child(b"FMT_").is_some());
    let donor_worlds: Vec<_> = donor
        .iter()
        .filter(|c| &c.tag == b"wrld")
        .map(World::parse)
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| anyhow!("{e}"))?;
    let class_geometry = |items: &[Chunk<'_>]| -> BTreeMap<String, String> {
        items
            .iter()
            .filter(|c| &c.tag == b"entc")
            .filter_map(|c| {
                let e = mungefs::entity::EntityClass::parse(c).ok()?;
                let g = e.props.iter().find(|p| p.key_hash == 0x47c8_6b4a)?;
                Some((e.name.to_ascii_lowercase(), g.value.to_ascii_lowercase()))
            })
            .collect()
    };
    let target_classes = class_geometry(items);
    let allowed = recovery_names()?;
    let donor_classes = class_geometry(&donor);
    let original_bounds: BTreeMap<_, _> = donor
        .iter()
        .filter(|c| &c.tag == b"modl")
        .filter_map(|c| {
            let model = Model::parse(c).ok()?;
            Some((model.name.to_ascii_lowercase(), model.vis_box))
        })
        .collect();
    let changed_bounds = |name: &str, model: &Model| {
        original_bounds.get(name).is_some_and(|bounds| {
            bounds.min != model.vis_box.min || bounds.max != model.vis_box.max
        })
    };
    let donor_geometries: BTreeMap<_, _> = donor
        .iter()
        .filter(|c| &c.tag == b"gmod")
        .filter_map(|c| {
            let g = mungefs::entity::GeomModel::parse(c).ok()?;
            Some((
                g.name.to_ascii_lowercase(),
                g.best_model()?.to_ascii_lowercase(),
            ))
        })
        .collect();
    let targets: Vec<_> = worlds
        .iter()
        .flat_map(|w| &w.instances)
        .filter_map(|i| {
            let geometry = target_classes.get(&i.class.to_ascii_lowercase())?;
            (allowed.contains(geometry) && !models.contains_key(geometry))
                .then_some((geometry.clone(), i.transform))
        })
        .collect();
    let mut recovered: BTreeMap<String, Model> = BTreeMap::new();
    let mut reused = BTreeSet::new();
    for (name, target) in targets {
        let Some(bounds) = bounds_for(&name, items)? else {
            eprintln!("render recovery: {name}: no canonical bounds; unresolved");
            continue;
        };
        let inverse = matrix(&target).inverse();
        let mut segments = Vec::new();
        let mut contributors = BTreeSet::new();
        let mut template = None;
        for instance in donor_worlds.iter().flat_map(|w| &w.instances) {
            let Some(geometry) = donor_classes.get(&instance.class.to_ascii_lowercase()) else {
                continue;
            };
            let source_name = donor_geometries.get(geometry).unwrap_or(geometry);
            let Some(source) = models.get(source_name) else {
                continue;
            };
            if source.bone_space {
                continue;
            }
            // Unchanged independent props already draw at their canonical
            // placements. They must not be duplicated into a recovered room.
            let unchanged = worlds.iter().flat_map(|w| &w.instances).any(|target| {
                target.transform == instance.transform
                    && target_classes.get(&target.class.to_ascii_lowercase()) == Some(geometry)
            });
            if unchanged && !changed_bounds(source_name, source) {
                continue;
            }
            let transform = inverse * matrix(&instance.transform);
            let translation = inverse
                * (position(&instance.transform, Vec3::ZERO) - Vec3::from(target.translation));
            // Broad phase before touching vertices of every placed building.
            let mut low = Vec3::splat(f32::INFINITY);
            let mut high = Vec3::splat(f32::NEG_INFINITY);
            for x in [source.vertex_box.min[0], source.vertex_box.max[0]] {
                for y in [source.vertex_box.min[1], source.vertex_box.max[1]] {
                    for z in [source.vertex_box.min[2], source.vertex_box.max[2]] {
                        let p = transform * Vec3::new(x, y, z) + translation;
                        low = low.min(p);
                        high = high.max(p);
                    }
                }
            }
            if (low.cmpgt(bounds.max.into()) | high.cmplt(bounds.min.into())).any() {
                continue;
            }
            for source_segment in &source.segments {
                if let Some(segment) = clipped(source_segment, transform, translation, bounds) {
                    contributors.insert(source.name.clone());
                    template.get_or_insert_with(|| source.clone());
                    segments.push(segment);
                }
            }
        }
        let Some(mut model) = template else {
            eprintln!("render recovery: {name}: no donor surface in canonical bounds; unresolved");
            continue;
        };
        model.name = name.clone();
        model.vertex_box = bounds;
        model.vis_box = bounds;
        model.sphere = None;
        model.face_count = segments.iter().map(|s| s.triangles.len() as u32).sum();
        model.segments = segments;
        eprintln!(
            "render recovery: {}: {} donor triangles from {}",
            model.name,
            model.face_count,
            contributors.iter().cloned().collect::<Vec<_>>().join(", ")
        );
        reused.extend(contributors);
        if recovered
            .get(&name)
            .is_none_or(|previous| previous.face_count < model.face_count)
        {
            recovered.insert(name, model);
        }
    }
    let mut result: Vec<_> = recovered.into_values().collect();
    for model in &mut result {
        // This measured allowlist contains only model types found absent in
        // the full source inventories. Other missing resources stay explicit.
        if let Some(bounds) = measured_bounds(&model.name)? {
            if let Some(standin) = super::collision_standin::fit(&model.name, items, model, bounds)?
            {
                *model = standin;
            }
        }
    }
    // The PS2 partitions a donor composite into independent pieces. Its
    // retained named piece must lose the extra source surfaces too. The
    // existing reviewed gameplay correction supplies its PS2 visibility box,
    // including authored art overhang beyond collision.
    for name in reused {
        // The PC ruin's PS2 visibility box excludes two live collision
        // contacts by 2.67 m. Keep that piece intact until a surface-level
        // correction replaces this coarse crop; Xbox's crop does not regress.
        if pc_source && name.eq_ignore_ascii_case("rhn_bldg_ruin_13") {
            eprintln!(
                "render recovery: {name}: retained uncropped PC surface after contact comparison"
            );
            continue;
        }
        let Some(source) = models.get(&name.to_ascii_lowercase()) else {
            continue;
        };
        if !changed_bounds(&name.to_ascii_lowercase(), source) {
            continue;
        }
        let mut model = source.clone();
        model.segments = source
            .segments
            .iter()
            .filter_map(|s| clipped(s, Mat3::IDENTITY, Vec3::ZERO, source.vis_box))
            .collect();
        model.face_count = model
            .segments
            .iter()
            .map(|s| s.triangles.len() as u32)
            .sum();
        model.vertex_box = source.vis_box;
        eprintln!(
            "render recovery: {}: retained canonical piece, {} triangles",
            model.name, model.face_count
        );
        result.push(model);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        append_chunk(
            &mut out,
            &Chunk {
                tag: *tag,
                payload: body,
                offset: 0,
            },
        );
        out
    }

    #[test]
    fn shared_lookup_finds_case_insensitive_nested_side_resources_without_sim_data() {
        let root =
            std::env::temp_dir().join(format!("openbf-render-shared-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Map")).unwrap();
        std::fs::create_dir_all(root.join("sIdE")).unwrap();
        std::fs::write(root.join("CORE.LVL"), resource(b"ucfb", &[])).unwrap();
        std::fs::write(root.join("MISSION.LVL"), resource(b"ucfb", &[])).unwrap();
        let mut prop = 0x47c8_6b4au32.to_le_bytes().to_vec();
        prop.extend(b"Rhn1_bldg_brokencorner\0");
        let entity = [
            resource(b"TYPE", b"class\0"),
            resource(b"BASE", b"prop\0"),
            resource(b"PROP", &prop),
        ]
        .concat();
        let local = resource(b"entc", &entity);
        let map = root.join("Map/map.lvl");
        std::fs::write(&map, resource(b"ucfb", &local)).unwrap();
        let donor = [
            resource(b"modl", &resource(b"NAME", b"RHN1_BLDG_BROKENCORNER\0")),
            resource(b"coll", b"unrelated collision"),
            resource(b"entc", &entity),
        ]
        .concat();
        let nested = resource(b"lvl_", &resource(&123u32.to_le_bytes(), &donor));
        std::fs::write(root.join("sIdE/shared.LvL"), resource(b"ucfb", &nested)).unwrap();
        let content = mungefs::content_chunks(&local).unwrap();
        let pulled = shared(map.to_str().unwrap(), &content).unwrap();
        let pulled = mungefs::content_chunks(&pulled).unwrap();
        assert_eq!(pulled.len(), 1);
        assert_eq!(pulled[0].tag, *b"modl");
        assert_eq!(named(&pulled[0]).as_deref(), Some("rhn1_bldg_brokencorner"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clips_a_crossing_surface_and_interpolates_its_artwork() {
        let vertex = |x: f32, y: f32| Vertex {
            p: Vec3::new(x, y, 0.0),
            n: Vec3::Z,
            uv: [x, y],
            color: [x * 100.0; 4],
        };
        let polygon = clip(
            vec![vertex(-1.0, 0.0), vertex(1.0, 0.0), vertex(1.0, 1.0)],
            Vec3::ZERO,
            Vec3::ONE,
        );
        assert_eq!(polygon.len(), 4);
        for v in polygon {
            assert!(v.p.x >= 0.0 && v.p.x <= 1.0);
            assert_eq!(v.uv, [v.p.x, v.p.y]);
            assert_eq!(v.color, [v.p.x * 100.0; 4]);
        }
    }
}
