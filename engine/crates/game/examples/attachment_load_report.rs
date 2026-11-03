//! CPU-only evidence from the real vehicle asset consumers.
//!
//! `cargo run -p game --example attachment_load_report -- <assets> <out.json> [map ...]`
//! Reports only names, counts and hashes. This is NOT a collision/pose/GPU
//! alignment certificate. Any required load or input-stability failure sets
//! load_complete:false and a failing exit status. The conservative attachment
//! identity gate remains in force; this report cannot replace it on its own.
use anyhow::{anyhow, bail, Context, Result};
use game::assets::{CpuMesh, CpuTexture, Scene, TextureTier};
use game::classdb::ClassDb;
use game::viewer::{find_model, find_texture, ModelSource, TexSource, Untextured, VehicleSides};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn encoded(value: &impl Serialize) -> String {
    digest(&serde_json::to_vec(value).unwrap())
}
#[derive(Clone, PartialEq, Serialize)]
struct Stamp {
    bytes: usize,
    sha256: String,
}
#[derive(Default, PartialEq)]
struct Inputs(BTreeMap<PathBuf, Stamp>);
impl Inputs {
    // Capture candidates before any consumer runs, including currently unused
    // sidecars. A newly created fallback/tier file changes route selection too.
    fn catalog(root: &Path) -> Result<Self> {
        fn visit(dir: &Path, inputs: &mut Inputs) -> Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let path = entry?.path();
                if path.is_dir() {
                    visit(&path, inputs)?;
                } else if path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| matches!(s, "glb" | "ron" | "ktx2" | "json"))
                {
                    inputs.receive(&path)?;
                }
            }
            Ok(())
        }
        let mut out = Self::default();
        visit(root, &mut out)?;
        Ok(out)
    }
    fn receive(&mut self, path: &Path) -> Result<()> {
        let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
        let stamp = Stamp {
            bytes: bytes.len(),
            sha256: digest(&bytes),
        };
        if self.0.get(path).is_some_and(|old| *old != stamp) {
            bail!("input changed during load: {}", path.display());
        }
        self.0.insert(path.to_path_buf(), stamp);
        Ok(())
    }
    fn verify(&self) -> Vec<String> {
        self.0
            .iter()
            .filter_map(|(p, s)| match std::fs::read(p) {
                Ok(b) if b.len() == s.bytes && digest(&b) == s.sha256 => None,
                _ => Some(format!(
                    "received input changed or disappeared: {}",
                    p.display()
                )),
            })
            .collect()
    }
    fn report(&self, root: &Path) -> BTreeMap<String, Stamp> {
        self.0
            .iter()
            .map(|(p, s)| {
                (
                    p.strip_prefix(root)
                        .unwrap_or(p)
                        .to_string_lossy()
                        .into_owned(),
                    s.clone(),
                )
            })
            .collect()
    }
}
fn mesh_geometry(mesh: &CpuMesh, joints: &[[u8; 4]]) -> Result<Value> {
    if mesh.positions.is_empty()
        || mesh.indices.is_empty()
        || mesh.positions.iter().flatten().any(|x| !x.is_finite())
        || mesh.normals.iter().flatten().any(|x| !x.is_finite())
        || mesh.uvs.iter().flatten().any(|x| !x.is_finite())
        || mesh
            .indices
            .iter()
            .any(|&i| i as usize >= mesh.positions.len())
    {
        bail!("invalid/nonfinite loaded mesh streams");
    }
    let position_bits: Vec<_> = mesh.positions.iter().map(|p| p.map(f32::to_bits)).collect();
    Ok(
        json!({"vertices":mesh.positions.len(),"triangles":mesh.indices.len()/3,
        "position_index_sha256":encoded(&(&position_bits,&mesh.indices)),
        "joint_sha256":encoded(&joints),"submeshes":mesh.submeshes.len(),
        "bounds_inputs_sha256":encoded(&position_bits)}),
    )
}
fn alpha_semantics(t: &CpuTexture) -> Result<Value> {
    if t.width == 0 || t.height == 0 {
        bail!("zero-size loaded texture");
    }
    let mut alphas = Vec::new();
    for (i, rgba) in std::iter::once(&t.rgba).chain(&t.extra_mips).enumerate() {
        let w = t.width.checked_shr(i as u32).unwrap_or(0).max(1);
        let h = t.height.checked_shr(i as u32).unwrap_or(0).max(1);
        if rgba.len() != w as usize * h as usize * 4 {
            bail!("loaded texture mip {i} has incomplete RGBA bytes");
        }
        alphas.push((
            w,
            h,
            rgba.iter().skip(3).step_by(4).copied().collect::<Vec<_>>(),
        ));
    }
    let detail = t.detail.as_deref().map(alpha_semantics).transpose()?;
    Ok(
        json!({"width":t.width,"height":t.height,"mips":alphas.len(),
        "alpha_sha256":encoded(&alphas),"detail":detail}),
    )
}
fn receive_texture(inputs: &mut Inputs, dir: &Path, name: &str) -> Result<()> {
    inputs.receive(&dir.join(format!("{name}.ktx2")))?;
    for suffix in ["tier2.ktx2", "dtl.ktx2"] {
        let p = dir.join(format!("{name}.{suffix}"));
        if p.is_file() {
            inputs.receive(&p)?;
        }
    }
    Ok(())
}
fn texture_route(
    scene: &Scene,
    sides: &VehicleSides,
    home: Option<usize>,
    name: &str,
    inputs: &mut Inputs,
) -> Result<Value> {
    let name = name.to_ascii_lowercase();
    let (loaded, where_from) = match find_texture(&scene.manifest, sides, home, &name) {
        Some(TexSource::Map) => (
            scene.textures.get(&name).map(alpha_semantics).transpose()?,
            Some("map/common loaded texture table".to_owned()),
        ),
        Some(TexSource::Side(i)) => {
            let dir = sides.dir(i).join("textures");
            if dir.join(format!("{name}.ktx2")).is_file() {
                receive_texture(inputs, &dir, &name)?;
                let tex = game::assets::load_texture(&dir, &name)?;
                (
                    Some(alpha_semantics(&tex)?),
                    Some(dir.to_string_lossy().into_owned()),
                )
            } else {
                (None, Some(dir.to_string_lossy().into_owned()))
            }
        }
        None => (None, None),
    };
    let route = if loaded.is_some() {
        "textured"
    } else if game::viewer::route_untextured((!name.is_empty()).then_some(name.as_str()))
        == Untextured::LookupFailed
    {
        "grey_fallback"
    } else {
        "skipped"
    };
    Ok(json!({"material":name,"route":route,"lookup":where_from,"loaded_alpha":loaded}))
}
fn source_stamp() -> BTreeMap<&'static str, String> {
    [
        (
            "example",
            include_bytes!("attachment_load_report.rs").as_slice(),
        ),
        ("assets", include_bytes!("../src/assets.rs").as_slice()),
        ("mesh", include_bytes!("../src/assets/mesh.rs").as_slice()),
        (
            "vehicle_routes",
            include_bytes!("../src/viewer/vehicles.rs").as_slice(),
        ),
        (
            "upload",
            include_bytes!("../src/viewer/upload.rs").as_slice(),
        ),
        ("classdb", include_bytes!("../src/classdb.rs").as_slice()),
        ("pads", include_bytes!("../src/vehicle_pads.rs").as_slice()),
        ("roster", include_bytes!("../src/roster.rs").as_slice()),
        ("skin", include_bytes!("../src/skin.rs").as_slice()),
        (
            "walker",
            include_bytes!("../src/viewer/walker_pose.rs").as_slice(),
        ),
        (
            "world_shader",
            include_bytes!("../../render/src/shader.wgsl").as_slice(),
        ),
    ]
    .into_iter()
    .map(|(n, b)| (n, digest(b)))
    .collect()
}
fn load_map(root: &Path, map: &Path, inputs: &mut Inputs) -> Result<Scene> {
    inputs.receive(&map.join("manifest.ron"))?;
    let scene = game::assets::load_scene(map)?;
    let common = root.join(format!(
        "common_{}",
        scene.manifest.source_game.to_ascii_lowercase()
    ));
    inputs.receive(&common.join("common.ron"))?;
    // Receive every registered texture whose real loader was just called.
    for (dir, names) in [
        (map.to_path_buf(), scene.manifest.textures.clone()),
        (
            common.clone(),
            ron::from_str::<manifest::CommonManifest>(&std::fs::read_to_string(
                common.join("common.ron"),
            )?)?
            .textures,
        ),
    ] {
        for name in names {
            receive_texture(inputs, &dir.join("textures"), &name)?;
        }
    }
    let mut scene = scene;
    scene.merge_common(game::assets::load_common(&common)?);
    Ok(scene)
}
fn route(
    root: &Path,
    scene: &Scene,
    sides: &VehicleSides,
    g: &str,
    inputs: &mut Inputs,
) -> Result<Value> {
    let (path, home) =
        match find_model(&scene.manifest, sides, g).context("no selected model resolves")? {
            ModelSource::Map(n) => (scene.dir.join("models").join(format!("{n}.glb")), None),
            ModelSource::Side(i, n) => (
                sides.dir(i).join("models").join(format!("{n}.glb")),
                Some(i),
            ),
        };
    inputs.receive(&path)?;
    let (mesh, joints) = game::assets::load_glb_joints(&path)?;
    let geometry = mesh_geometry(&mesh, &joints)?;
    let mut parts = Vec::new();
    for (start, count, mat, layer, _flags) in &mesh.submeshes {
        let texture = texture_route(scene, sides, home, mat, inputs)?;
        parts.push(json!({"start":start,"count":count,"layer":format!("{layer:?}"),"texture":texture,
            "coverage":"opaque only after successful load/route; MASK/BLEND needs separate coverage proof"}));
    }
    let mut pose = json!({"route":"rigid at body transform; walker fallback may use leg heading"});
    if let Some(i) = home.filter(|_| !joints.is_empty()) {
        let metadata = sides.dir(i).join("vehicles.ron");
        inputs.receive(&metadata)?;
        let vm: manifest::VehicleManifest = ron::from_str(&std::fs::read_to_string(&metadata)?)?;
        let model = path.file_stem().unwrap().to_string_lossy();
        if vm
            .vehicles
            .iter()
            .filter_map(|v| v.walker.as_ref())
            .any(|w| {
                w.skin
                    .as_ref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(&model))
            })
        {
            let skin = sides.dir(i).join("walkers").join(format!("{model}.glb"));
            inputs.receive(&skin)?;
            let a = game::assets::load_skinned_glb(&skin)?;
            if game::viewer::walker_rigs(&scene.manifest, root, &[Some(g.to_owned())])
                .first()
                .and_then(Option::as_ref)
                .is_none()
            {
                bail!("actual walker rig did not load");
            }
            let binds: Vec<_> = a
                .skeleton
                .local_bind
                .iter()
                .map(|m| m.to_cols_array().map(f32::to_bits))
                .collect();
            let clips:Vec<_>=a.clips.iter().map(|c|json!({"name":c.name,"fps":c.fps.to_bits(),"frames":c.frames,"tracks":c.tracks.iter().map(|t|t.as_ref().map(|t|t.iter().map(|k|(k.rotation.to_array().map(f32::to_bits),k.translation.to_array().map(f32::to_bits))).collect::<Vec<_>>())).collect::<Vec<_>>()})).collect();
            pose = json!({"route":"actual loaded walker rig","joints":a.skeleton.len(),"clips":a.clips.len(),"loaded_pose_sha256":encoded(&(&a.skeleton.names,&a.skeleton.parents,binds,clips)),"requires":"class/anim state, inverse-bind arithmetic, all reachable poses and GPU closure"});
        }
    }
    Ok(json!({"path":path.strip_prefix(root)?,"geometry":geometry,"parts":parts,"pose":pose}))
}
fn report(root: &Path, selected: &[String]) -> Value {
    let (mut inputs, catalog_error) = match Inputs::catalog(root) {
        Ok(i) => (i, None),
        Err(e) => (
            Inputs::default(),
            Some(format!("initial input catalog: {e:#}")),
        ),
    };
    let mut cases = Vec::new();
    let mut routes = Vec::new();
    let mut errors: Vec<String> = catalog_error.into_iter().collect();
    let mut maps = Vec::new();
    let discovered = game::assets::discover_maps(root);
    if discovered.is_empty() {
        errors.push("no maps discovered".to_owned());
    }
    for want in selected {
        if !discovered.iter().any(|(k, _, _)| k == want) {
            errors.push(format!("requested map absent: {want}"));
        }
    }
    for tier in [TextureTier::Ps2, TextureTier::Pc] {
        game::assets::set_texture_tier(tier);
        let tier_name = if tier == TextureTier::Ps2 {
            "ps2"
        } else {
            "pc"
        };
        for (key, game, map) in &discovered {
            if !selected.is_empty() && !selected.contains(key) {
                continue;
            }
            let mut scene = match load_map(root, map, &mut inputs) {
                Ok(s) => s,
                Err(e) => {
                    errors.push(format!("{tier_name}/{key}: {e:#}"));
                    continue;
                }
            };
            maps.push(json!({"map":key,"tier":tier_name,"loaded":true}));
            let sides = VehicleSides::load(root, game);
            for i in 0..sides.len() {
                if let Err(e) = inputs.receive(&sides.dir(i).join("vehicles.ron")) {
                    errors.push(format!("{e:#}"));
                }
            }
            let base = scene.manifest.clone();
            let mut layers = vec![("base".to_owned(), None)];
            layers.extend(
                base.layers
                    .iter()
                    .cloned()
                    .map(|l| (l.name.clone(), Some(l))),
            );
            let mut routed = BTreeMap::new();
            for (layer_name, layer) in layers {
                scene.manifest = base.clone();
                if let Some(l) = layer {
                    scene.manifest.instances.extend(l.instances);
                    scene.manifest.regions.extend(l.regions);
                }
                for a in ["all", "imp", "rep", "cis"] {
                    for b in ["all", "imp", "rep", "cis"] {
                        let attempt = (|| -> Result<Value> {
                            let mut db = ClassDb::default();
                            let local = map.join("classes.ron");
                            if local.is_file() {
                                inputs.receive(&local)?;
                            }
                            db.load_map(map)?;
                            for file in game::roster::class_files(game, &[a, b]).into_iter().chain(
                                game::vehicle_pads::mission_side_files(game, key)
                                    .iter()
                                    .copied(),
                            ) {
                                let p = root.join("classes").join(file);
                                inputs.receive(&p)?;
                                db.load(&p)?;
                            }
                            let build = game::vehicle_pads::build(
                                &scene.manifest,
                                &db,
                                &[a.into(), b.into()],
                                |_| None,
                            );
                            let mut classes = Vec::new();
                            for (c, g) in build.classes.iter().zip(&build.geometry) {
                                let id = if let Some(g) = g {
                                    let g = g.to_ascii_lowercase();
                                    if let Some(i) = routed.get(&g) {
                                        Some(*i)
                                    } else {
                                        let i = routes.len();
                                        let value =
                                            match route(root, &scene, &sides, &g, &mut inputs) {
                                                Ok(v) => v,
                                                Err(e) => {
                                                    errors.push(format!(
                                                        "{tier_name}/{key}/{g}: {e:#}"
                                                    ));
                                                    json!({"error":format!("{e:#}")})
                                                }
                                            };
                                        routes.push(json!({"map":key,"tier":tier_name,"geometry_name":g,"loaded":value}));
                                        routed.insert(g, i);
                                        Some(i)
                                    }
                                } else {
                                    None
                                };
                                classes.push(json!({"class":c,"kind":db.engine_base(c),"route":id,"world_turret_delegated":g.is_none()}));
                            }
                            Ok(
                                json!({"map":key,"tier":tier_name,"layer":layer_name,"factions":[a,b],"classes":classes,"pads":build.pads.len(),"absent":build.absent}),
                            )
                        })();
                        match attempt {
                            Ok(v) => cases.push(v),
                            Err(e) => errors
                                .push(format!("{tier_name}/{key}/{layer_name}/{a}/{b}: {e:#}")),
                        }
                    }
                }
            }
        }
    }
    errors.extend(inputs.verify());
    match Inputs::catalog(root) {
        Ok(after) if after == inputs => {}
        Ok(_) => errors.push("input file set or bytes changed during actual loads".into()),
        Err(e) => errors.push(format!("final input catalog: {e:#}")),
    }
    let received = inputs.report(root);
    let source = source_stamp();
    json!({"schema":1,"kind":"actual_vehicle_load_route_diagnostic","root":root,"runtime_complete":false,
        "load_complete":errors.is_empty(),"source_stamp":source,"input_catalog":received,"maps":maps,"cases":cases,"routes":routes,"errors":errors,
        "scope":"real CPU scene/common/class/model/texture/walker loaders; both tiers, each authored layer separately, all16 ordered faction pairs",
        "not_checked":["GPU upload/rasterization","all reachable poses/transitions","collision residuals","camera/fade/cull equality","mission layer combinations","soldier/door/turret/foliage consumers","transient changes restored between catalog reads"]})
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let root = Path::new(args.get(1).context("assets root")?);
    let out = Path::new(args.get(2).context("output JSON")?);
    let mut value = report(root, &args[3..]);
    value["payload_sha256"] = json!(encoded(&value));
    let complete = value["load_complete"] == true;
    let temporary = out.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(&value)?)?;
    std::fs::rename(temporary, out)?;
    println!(
        "{}",
        json!({"load_complete":complete,"runtime_complete":false,"maps":value["maps"].as_array().unwrap().len(),"routes":value["routes"].as_array().unwrap().len(),"errors":value["errors"].as_array().unwrap().len()})
    );
    if !complete {
        return Err(anyhow!(
            "required loads or input stability failed; diagnostic is incomplete"
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_rgb_does_not_change_loaded_alpha_semantics() {
        let texture = |rgb| CpuTexture {
            width: 1,
            height: 1,
            rgba: vec![rgb, 0, 0, 255],
            extra_mips: vec![],
            detail: None,
        };
        assert_eq!(
            alpha_semantics(&texture(0)).unwrap(),
            alpha_semantics(&texture(255)).unwrap()
        );
    }
    #[test]
    fn malformed_mip_is_not_a_load_coverage_proof() {
        let t = CpuTexture {
            width: 2,
            height: 2,
            rgba: vec![255; 15],
            extra_mips: vec![],
            detail: None,
        };
        assert!(alpha_semantics(&t).is_err());
    }
    #[test]
    fn no_maps_is_explicitly_incomplete() {
        let p = std::env::temp_dir().join(format!("attachment-no-maps-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        let r = report(&p, &[]);
        assert_eq!(r["load_complete"], false);
        assert_eq!(r["runtime_complete"], false);
        std::fs::remove_dir(p).unwrap();
    }
    #[test]
    fn new_tier_candidate_changes_catalog() {
        let p = std::env::temp_dir().join(format!("attachment-catalog-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        let before = Inputs::catalog(&p).unwrap();
        std::fs::write(p.join("hull.tier2.ktx2"), b"new fallback candidate").unwrap();
        assert!(before != Inputs::catalog(&p).unwrap());
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn received_artifact_bytes_cannot_change_silently() {
        let p = std::env::temp_dir().join(format!("attachment-stamp-{}", std::process::id()));
        std::fs::write(&p, b"before").unwrap();
        let mut i = Inputs::default();
        i.receive(&p).unwrap();
        std::fs::write(&p, b"after!").unwrap();
        assert!(i.receive(&p).is_err());
        assert_eq!(i.verify().len(), 1);
        std::fs::remove_file(p).unwrap();
    }
}
