//! PS2 soldier budgets (BF1 0x4790e0/0x4c1ef0, BF2 0x563bb0/0x5ace40).
//! No model loads, skin remapping, or allocations after the view scratch warms up.
use super::{SoldierVisuals, CORPSE_SECS};
use crate::walk::WalkSim;
use core::fmath::FloatExt;
use glam::{Vec3, Vec4};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Choice {
    Hidden,
    Low,
    #[default]
    High,
}

#[derive(Clone, Copy)]
struct Candidate {
    index: usize,
    depth: f32,
    radius: f32,
    key: f32,
    high_count: u32,
    low: bool,
    player: bool,
}

pub(super) struct View {
    pub near: Option<[f32; 3]>,
    pub far: Option<f32>,
    pub enabled: bool,
    choices: Vec<Choice>,
    queue: Vec<Candidate>,
}
impl Default for View {
    fn default() -> Self {
        Self {
            near: None,
            far: None,
            enabled: true,
            choices: Vec::new(),
            queue: Vec::new(),
        }
    }
}
impl View {
    pub fn choice(&self, i: usize) -> Choice {
        self.choices.get(i).copied().unwrap_or(Choice::High)
    }

    fn choose(&mut self, bf2: bool, zoom: f32) {
        let near = self.near.map_or(f32::INFINITY, |r| r[2] * zoom);
        let far_start = self.near.map_or(f32::INFINITY, |r| r[1] * zoom);
        let far = self.far.unwrap_or(0.0);
        self.queue.sort_unstable_by(|a, b| {
            (a.depth - 3.0 * a.radius)
                .total_cmp(&(b.depth - 3.0 * b.radius))
                .then(a.index.cmp(&b.index))
        });
        let mut far_count = 0;
        for c in &self.queue {
            if c.low && far > far_start && c.depth + c.radius >= far_start {
                let visible_size =
                    !bf2 || c.radius.powi(2) / (c.depth + c.radius).powi(2) >= 25.0 / 208000.0;
                if far_count < 10 && visible_size && c.depth < far {
                    self.choices[c.index] = Choice::Low;
                }
                far_count += 1;
            }
        }
        // BF1's heap is nearest first (with an off-axis bias); BF2's is
        // largest projected sphere first. Unit index breaks equal keys stably.
        self.queue
            .sort_unstable_by(|a, b| b.key.total_cmp(&a.key).then(a.index.cmp(&b.index)));
        let mut budget = if bf2 { 8000u32 } else { 10000u32 };
        // The viewport's own soldier always keeps its high model; account
        // for it first so the familiar four-body budget includes the player.
        for c in &self.queue {
            if c.player && c.depth - c.radius <= near {
                self.choices[c.index] = Choice::High;
                budget = budget.saturating_sub(c.high_count);
            }
        }
        for c in &self.queue {
            if c.player || c.depth - c.radius > near {
                continue;
            }
            if bf2 && c.key < 25.0 / 208000.0 {
                continue;
            }
            let fits = c.high_count <= budget
                && (!bf2 || !c.low || c.key * 208000.0 >= c.high_count as f32);
            self.choices[c.index] = if fits || !c.low {
                budget = budget.saturating_sub(c.high_count);
                Choice::High
            } else {
                Choice::Low
            };
        }
    }
}

/// A soldier's draw sphere reaches this far past its model's authored one. Read
/// out of PS2 save states: exactly 1.0 on all 15 BF1 classes matched by strip
/// count, and on 9 of 11 BF2 radii. At BF2's rules it is the difference between a
/// full-detail body to 12 m or 23 m, and the size cull at 114 m or 205 m.
const SPHERE_MARGIN: f32 = 1.0;

/// A body's place in the near scene's heap, largest first, from its sphere's
/// centre in view space. BF1 (0x4c5808): depth, less half the off-axis distance,
/// less the radius, so nearer, more central and bigger bodies come first. BF2
/// (0x5af6b0): the projected sphere, attenuated where the frustum clips it.
fn heap_key(bf2: bool, p: Vec3, radius: f32, zoom: f32, frustum: (f32, f32, f32)) -> f32 {
    let depth = -p.z;
    if !bf2 {
        return -depth - (p.x.abs() + p.y.abs()) * 0.5 + radius;
    }
    let (near, tan_x, tan_y) = frustum;
    let mut key = (radius * zoom).powi(2) / p.length_squared().fmax(1.0);
    let scaled = (radius * zoom).fmax(f32::EPSILON);
    for cut in [
        near - depth,
        (p.x.abs() - depth * tan_x) / (1.0 + tan_x * tan_x).sqrt(),
        (p.y.abs() - depth * tan_y) / (1.0 + tan_y * tan_y).sqrt(),
    ] {
        if cut > 0.0 {
            key *= (1.0 - cut / scaled).fmax(0.0).powi(2);
        }
    }
    key.fmin(100.0)
}

impl SoldierVisuals {
    pub(super) fn load_low(
        r: Option<&mut render::Renderer>,
        root: &std::path::Path,
        game: &str,
        geometry: &str,
        asset: &crate::assets::SkinnedAsset,
        cache: &mut std::collections::HashMap<String, Option<render::TextureHandle>>,
    ) -> (
        Option<manifest::SoldierLod>,
        Option<Box<super::SoldierAsset>>,
    ) {
        if geometry.ends_with(".low") {
            return (None, None);
        }
        let path = root.join(format!("side_{game}/soldiers/{geometry}.lod.ron"));
        let lod = pak::read_to_string(&path)
            .ok()
            .and_then(|text| ron::from_str(&text).ok());
        let low = lod.as_ref()
            .and_then(|_: &manifest::SoldierLod| Self::load_body(r, root, game, &format!("{geometry}.low"), cache))
            .filter(|low| {
                let matches = low.asset.skeleton.names == asset.skeleton.names
                    && low.asset.skeleton.global_bind == asset.skeleton.global_bind;
                if !matches { eprintln!("soldier {geometry}: low companion uses a different bind rig; rebake this soldier"); }
                matches
            }).map(Box::new);
        (lod, low)
    }

    /// Presentation diagnostic: disabled reproduces the previous unlimited
    /// full-detail submission. The default uses the PS2 budget on both tiers.
    pub fn set_soldier_lod(&mut self, enabled: bool) {
        self.lod_view.enabled = enabled;
        self.lod_view.choices.clear();
    }

    /// Select once for this camera, before body/weapon/cloth draw lists.
    /// Audio, attachment queries and simulation retain all soldiers.
    pub fn prepare_view(&mut self, walk: &WalkSim, camera: &render::Camera, aspect: f32) {
        if !self.lod_view.enabled {
            self.lod_view.choices.clear();
            return;
        }
        self.lod_view.choices.clear();
        self.lod_view
            .choices
            .resize(walk.sim.units.len(), Choice::Hidden);
        self.lod_view.queue.clear();
        let vp = camera.view_proj(aspect);
        let r = [vp.row(0), vp.row(1), vp.row(2), vp.row(3)];
        let planes = [
            r[3] + r[0],
            r[3] - r[0],
            r[3] + r[1],
            r[3] - r[1],
            r[2],
            r[3] - r[2],
        ]
        .map(|p| p / p.truncate().length());
        let view = camera.view();
        let zoom = walk.zoom_s;
        let tan_y = (camera.fov_y * 0.5).tan();
        let tan_x = tan_y * aspect;
        for (i, u) in walk.sim.units.iter().enumerate() {
            let Some(v) = self.units.get(i) else { continue };
            if u.droideka.is_some()
                || !u.cloak.visible()
                || v.gen != u.gen
                || (!u.alive
                    && (u.burst || v.gone || v.dead_for > CORPSE_SECS || !v.animator.posed()))
                || (u.mount.is_some() && v.seat_model.is_none())
            {
                continue;
            }
            let Some(sa) = self.asset_for(u.team, u.spec.class) else {
                continue;
            };
            // Missing companion preserves the previous full-detail draw at
            // every distance, including old bakes. Saber bodies keep their
            // existing draw alongside their separately submitted blade/trail
            // passes; their LOD is outside this ordinary-soldier change.
            if sa.low.is_none() || self.saber_visual(u.team, u.spec.class).is_some() {
                self.lod_view.choices[i] = Choice::High;
                continue;
            }
            // Never substitute another class's low mesh.
            let sphere = sa.lod.as_ref().map_or([0.0, 1.0, 0.0, 1.5], |l| l.sphere);
            let model = v.seat_model.unwrap_or_else(|| super::model_of(&u.soldier));
            let center = model.transform_point3(Vec3::from_slice(&sphere[..3]));
            let radius = sphere[3] + SPHERE_MARGIN;
            if planes
                .iter()
                .any(|p: &Vec4| p.dot(center.extend(1.0)) < -radius)
            {
                continue;
            }
            let p = view.transform_point3(center);
            let depth = -p.z;
            let low = sa.low.is_some();
            let key = heap_key(self.bf2, p, radius, zoom, (camera.near, tan_x, tan_y));
            let high_count = if self.bf2 {
                2000
            } else {
                sa.lod.as_ref().map_or(0, |l| l.high_count)
            };
            self.lod_view.queue.push(Candidate {
                index: i,
                depth,
                radius,
                key,
                high_count,
                low,
                player: i == walk.player.index as usize,
            });
        }
        self.lod_view.choose(self.bf2, zoom);
    }

    pub fn soldier_lod_choices(&self) -> &[Choice] {
        &self.lod_view.choices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn crowd(depths: &[f32], bf2: bool) -> View {
        View {
            near: Some([1.0, 100.0, 150.0]),
            far: Some(350.0),
            choices: vec![Choice::Hidden; depths.len()],
            queue: depths
                .iter()
                .enumerate()
                .map(|(index, &depth)| Candidate {
                    index,
                    depth,
                    radius: 1.0,
                    key: if bf2 { 1.0 / (depth * depth) } else { -depth },
                    high_count: if bf2 { 2000 } else { 2370 },
                    low: true,
                    player: index == 0,
                })
                .collect(),
            ..View::default()
        }
    }
    /// BF1's heap takes the nearer and the more central body first, and a bigger
    /// sphere ahead of a smaller one: `depth - off_axis/2 - radius` smallest first.
    #[test]
    fn bf1_heap_prefers_the_central_and_the_large() {
        let key =
            |x: f32, r: f32| heap_key(false, Vec3::new(x, 0.0, -50.0), r, 1.0, (0.0, 0.0, 0.0));
        assert!(key(0.0, 1.0) > key(10.0, 1.0));
        assert!(key(0.0, 2.0) > key(0.0, 1.0));
        assert_eq!(key(4.0, 1.5), -50.0 - 2.0 + 1.5);
    }

    /// Issue #69: the queue a PS2 save state holds, through this chooser. The
    /// rows come from RAM (`OPENBF_PS2_SOLDIER_QUEUES`, one CSV per game, game
    /// data, never committed) and each row's pick goes to `<csv>.port`.
    #[test]
    #[ignore = "requires soldier queues read out of local PS2 save states"]
    fn ps2_state_queues_through_the_chooser() {
        let list = std::env::var("OPENBF_PS2_SOLDIER_QUEUES").unwrap();
        for path in list.split(',') {
            let text = std::fs::read_to_string(path).unwrap();
            let bf2 = path.contains("bf2");
            let mut states: std::collections::BTreeMap<String, Vec<Vec<String>>> =
                Default::default();
            for line in text.lines().skip(1) {
                let row: Vec<String> = line.split(',').map(str::to_owned).collect();
                states.entry(row[0].clone()).or_default().push(row);
            }
            let mut out = String::from("state,addr,pick\n");
            for (state, rows) in &states {
                let num = |r: &Vec<String>, i: usize| r[i].parse::<f32>().unwrap();
                let (b, c, s) = (num(&rows[0], 11), num(&rows[0], 12), num(&rows[0], 13));
                let (tan_x, tan_y) = (num(&rows[0], 14), num(&rows[0], 15));
                let player = rows
                    .iter()
                    .enumerate()
                    .min_by(|a, b| num(a.1, 3).total_cmp(&num(b.1, 3)))
                    .unwrap()
                    .0;
                let mut view = View {
                    near: Some([0.0, b, c]),
                    far: Some(if rows[0].len() > 17 {
                        num(&rows[0], 17)
                    } else {
                        0.0
                    }),
                    choices: vec![Choice::Hidden; rows.len()],
                    queue: rows
                        .iter()
                        .enumerate()
                        .map(|(index, r)| {
                            let p = Vec3::new(num(r, 1), num(r, 2), -num(r, 3));
                            let radius = num(r, 4);
                            Candidate {
                                index,
                                depth: -p.z,
                                radius,
                                key: heap_key(bf2, p, radius, s, (0.39, tan_x, tan_y)),
                                high_count: if bf2 { 2000 } else { num(r, 5) as u32 },
                                low: true,
                                player: index == player,
                            }
                        })
                        .collect(),
                    ..View::default()
                };
                view.choose(bf2, s);
                let (mut high, mut low, mut hidden) = (0, 0, 0);
                for (i, r) in rows.iter().enumerate() {
                    out += &format!("{state},{},{:?}\n", r[16], view.choices[i]);
                    match view.choices[i] {
                        Choice::High => high += 1,
                        Choice::Low => low += 1,
                        Choice::Hidden => hidden += 1,
                    }
                }
                eprintln!("{state}: {high} high, {low} low, {hidden} hidden");
            }
            std::fs::write(format!("{path}.port"), out).unwrap();
        }
    }

    #[test]
    fn four_high_bodies_including_player_and_low_overflow() {
        for bf2 in [false, true] {
            let mut view = crowd(&[3.0, 8.0, 7.0, 6.0, 5.0, 4.0], bf2);
            view.choose(bf2, 1.0);
            assert_eq!(
                view.choices,
                [
                    Choice::High,
                    Choice::Low,
                    Choice::Low,
                    Choice::High,
                    Choice::High,
                    Choice::High
                ]
            );
        }
    }
    #[test]
    fn bf2_density_and_zoom() {
        let mut view = crowd(&[3.0, 20.0], true);
        view.choose(true, 1.0);
        assert_eq!(view.choices[1], Choice::Low);
        view.queue[1].key *= 4.0;
        view.choose(true, 2.0);
        assert_eq!(view.choices[1], Choice::High);
    }
    #[test]
    fn far_scene_is_ten_nearest_and_zero_disables_it() {
        let depths: Vec<_> = std::iter::once(3.0)
            .chain((0..14).rev().map(|n| 160.0 + n as f32))
            .chain([350.0])
            .collect();
        let mut view = crowd(&depths, false);
        view.choose(false, 1.0);
        assert_eq!(
            view.choices.iter().filter(|&&c| c == Choice::Low).count(),
            10
        );
        for (d, c) in depths.iter().zip(&view.choices).skip(1) {
            assert_eq!(
                *c,
                if *d < 170.0 {
                    Choice::Low
                } else {
                    Choice::Hidden
                }
            );
        }
        view.choices.fill(Choice::Hidden);
        view.far = Some(0.0);
        view.choose(false, 1.0);
        assert_eq!(
            view.choices
                .iter()
                .filter(|&&c| c != Choice::Hidden)
                .count(),
            1
        );
    }
    #[test]
    fn far_heap_includes_the_native_radius_bias() {
        let depths: Vec<_> = std::iter::once(3.0)
            .chain((160..=170).map(|d| d as f32))
            .collect();
        let mut view = crowd(&depths, false);
        view.queue.last_mut().unwrap().radius = 5.0;
        view.choose(false, 1.0);
        assert_eq!(
            view.choices[11],
            Choice::Low,
            "170 - 3*5 sorts before 160 - 3*1"
        );
        assert_eq!(
            view.choices[10],
            Choice::Hidden,
            "169m body loses the tenth slot"
        );
    }

    #[test]
    fn bf2_far_scene_keeps_the_native_size_threshold() {
        let mut view = crowd(&[3.0, 40.0, 100.0], true);
        view.near = Some([1.0, 20.0, 30.0]);
        view.choose(true, 1.0);
        assert_eq!(view.choices, [Choice::High, Choice::Low, Choice::Hidden]);
    }

    #[test]
    fn transition_band_consumes_far_slots_and_absent_low_never_fills_them() {
        let mut view = crowd(&[3.0, 110.0, 160.0], false);
        view.queue[2].low = false;
        view.choose(false, 1.0);
        assert_eq!(view.choices, [Choice::High, Choice::High, Choice::Hidden]);
    }

    mod real_content {
        use super::*;

        /// Bake the two troopers into OPENBF_SOLDIER_LOD_ASSETS before running.
        /// Fails for missing content; never substitutes fixtures for the real mesh.
        #[test]
        #[ignore = "requires staged PS2 low companions and a headless GPU"]
        fn real_soldier_lod_submission_and_frame_cost() {
            let root = std::path::PathBuf::from(
                std::env::var("OPENBF_SOLDIER_LOD_ASSETS").expect("staged asset root"),
            );
            let mut r = render::Renderer::new_headless(1280, 720).expect("headless GPU");
            let cam = render::Camera {
                position: Vec3::new(0.0, 2.0, 0.0),
                ..Default::default()
            };
            for (game, geometry) in [("bf1", "rep_inf_trooper"), ("bf2", "rep_inf_ep3trooper")] {
                let mut team = crate::walk::TeamTemplate::default_trooper();
                team.classes[0].geometry = Some(geometry.into());
                team.classes[0].weapon_geometry = None;
                let mut vis =
                    SoldierVisuals::load(&mut r, &root, game, &[team.clone(), team.clone()]);
                let asset = &vis.assets[0];
                let low = asset.low.as_ref().expect("real low companion must load");
                assert_eq!(asset.asset.skeleton.names, low.asset.skeleton.names);
                assert!(low.asset.mesh.positions.len() < asset.asset.mesh.positions.len());
                eprintln!(
                    "{game} high={} low={} vertices",
                    asset.asset.mesh.positions.len(),
                    low.asset.mesh.positions.len()
                );
                for count in [16usize, 64] {
                    let ground = core::heightfield::Heightfield {
                        size: 2,
                        unit: 4000.0,
                        offset_x: 2000.0,
                        offset_z: 2000.0,
                        heights: vec![0.0; 4],
                    };
                    let mut walk = WalkSim::new(69, Vec3::new(0.0, 0.0, -4.0), Some(ground));
                    walk.teams = [team.clone(), team.clone()];
                    for i in 1..count {
                        walk.sim
                            .spawn(&core::unit::UnitSpec::trooper(
                                1,
                                Vec3::new(
                                    (i % 8) as f32 * 2.0 - 7.0,
                                    0.0,
                                    -12.0 - (i / 8) as f32 * 4.0,
                                ),
                            ))
                            .expect("spawn");
                    }
                    assert!(
                        walk.sim.units.iter().all(|u| u.soldier.pos.y == 0.0),
                        "benchmark actors must be on their ground plane"
                    );
                    let hash = walk.sim.hash();
                    vis.update(&walk, core::TICK_DT);
                    r.render_frame(&cam, &[], &[], [0.02, 0.02, 0.02, 1.0], None)
                        .unwrap();
                    let background = r.capture_rgba().unwrap().0;
                    let mut elapsed = [Vec::new(), Vec::new()];
                    let mut selected = Vec::new();
                    // Alternate order to reduce thermal/run-queue bias. Warm each
                    // draw list before timing; no readback in the measured frames.
                    for round in 0..6 {
                        for mode in [round % 2, 1 - round % 2] {
                            vis.set_soldier_lod(mode == 1);
                            vis.prepare_view(&walk, &cam, 1280.0 / 720.0);
                            if mode == 1 {
                                selected = vis.soldier_lod_choices().to_vec();
                            }
                            let draws = vis.draws(&walk);
                            assert!(!draws.is_empty());
                            r.render_frame(&cam, &[], &draws, [0.02, 0.02, 0.02, 1.0], None)
                                .unwrap();
                            r.wait_idle().unwrap();
                            for _ in 0..10 {
                                let start = std::time::Instant::now();
                                r.render_frame(&cam, &[], &draws, [0.02, 0.02, 0.02, 1.0], None)
                                    .unwrap();
                                r.wait_idle().unwrap();
                                elapsed[mode].push(start.elapsed().as_secs_f64() * 1000.0);
                            }
                            if round == 0 {
                                let (pixels, w, h) = r.capture_rgba().unwrap();
                                std::fs::write(
                                    root.join(format!("{game}_{count}_lod{mode}.png")),
                                    render::encode_png_rgb(&pixels, w, h).unwrap(),
                                )
                                .unwrap();
                                let changed = pixels
                                    .chunks_exact(4)
                                    .zip(background.chunks_exact(4))
                                    .filter(|(a, b)| a[..3] != b[..3])
                                    .count();
                                eprintln!("{game} {count} lod{mode}: {changed} non-background pixels, peak {}", pixels.chunks_exact(4).flat_map(|p|p[..3].iter()).max().unwrap());
                                assert!(changed > 128, "visible soldier silhouette");
                            }
                        }
                    }
                    for samples in &mut elapsed {
                        samples.sort_by(f64::total_cmp);
                    }
                    let median = |mode: usize| elapsed[mode][elapsed[mode].len() / 2];
                    eprintln!("{game} {count} bodies: full {:.3} ms, PS2 LOD {:.3} ms, ratio {:.3}; high {} low {}", median(0), median(1), median(1)/median(0), selected.iter().filter(|&&c|c==Choice::High).count(), selected.iter().filter(|&&c|c==Choice::Low).count());
                    let low = vis.assets[0].low.take();
                    vis.set_soldier_lod(true);
                    vis.prepare_view(&walk, &cam, 1280.0 / 720.0);
                    assert!(
                        vis.soldier_lod_choices().iter().all(|c| *c == Choice::High),
                        "old bake preserves full-detail submission"
                    );
                    vis.assets[0].low = low;
                    let start = std::time::Instant::now();
                    for _ in 0..1000 {
                        vis.prepare_view(&walk, &cam, 1280.0 / 720.0);
                    }
                    eprintln!(
                        "{game} {count} selection {:.2} us/frame (debug)",
                        start.elapsed().as_secs_f64() * 1000.0
                    );
                    assert_eq!(walk.sim.hash(), hash, "presentation changed simulation");
                }
            }
        }
    }
}
