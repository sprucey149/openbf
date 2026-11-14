//! The port's FLOOR under everything the disc authors inside a map's water —
//! queue item WL. A measurement: it loads, it prints, it changes nothing.
//!
//! ```text
//! cargo run --release -p game --example water_floor            # the three maps the item names
//! cargo run --release -p game --example water_floor all        # every baked map with a water plane
//! cargo run --release -p game --example water_floor bf1_yav1   # named maps
//! ```
//!
//! `analysis/re-water.md` §2 [V] settles the PLANE: the surface height is the
//! first `LAYR`'s f32, `mungefs::water` reads exactly that field, and
//! YAV1's −72.5 / KAS2's −5.0 are the disc's own numbers. So the "crates sunk to
//! their lids" defect (`engine/artifacts/walkabout_defects/README.md` group 5)
//! is the OTHER side of the join: where the PORT's ground sits under the props
//! the disc authors in the wet cells. This prints that side.
//!
//! Per map:
//! * the plane's height and its WET footprint, read out of the baked
//!   `water.glb` itself — the bake writes one axis-aligned quad per wet cell in
//!   world space (`bake::water_glb::water_to_glb`), so "inside the water" is
//!   measured here, not assumed;
//! * every command post, and every instance whose XZ lands in a wet cell;
//! * for each such point: the AUTHORED y (the manifest translation the disc
//!   gives it), the raw heightfield height under it, whether a terrain cutter
//!   removes the sheet there, [`core::sim::Sim::ground_under`] — the floor a
//!   soldier standing at that authored point actually gets — and the highest
//!   WALKABLE model surface in the band around it
//!   ([`core::collision::CollisionWorld::ray_down`]).
//!
//! The last column is `floor − authored`. Negative means the port's ground is
//! BELOW the thing the disc put there; rows past the item's 0.3 m threshold are
//! flagged `!` and counted in the footer.
//!
//! The world is built exactly as the sweep builds it (`round_to_outcome`): the
//! `con` mode layer applied so the BF2 posts exist, `common_<game>` merged so
//! the post models' own collision is in the world, then
//! `Scene::collision_world`. No GPU and no sim stepping — nothing here can move
//! a replay hash or a golden.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use core::sim::Sim;
use glam::Vec3;
use std::path::{Path, PathBuf};

/// The maps the queue item names: YAV1's bazaar pool and both KAS2 beaches.
const DEFAULT_MAPS: [&str; 3] = ["bf1_yav1", "bf1_kas2", "bf2_kas2"];

/// The model probe starts this far ABOVE the authored point. A prop authored a
/// few centimetres under its own deck (KAS2's posts sit within 5 cm of the
/// plane) still finds the deck, and nothing more than a soldier's step above
/// the point can win.
const PROBE_UP: f32 = 2.0;
/// ...and reaches this far BELOW it: deep enough to find the pier deck or the
/// plaza floor under a marker authored at water level, short enough that the
/// answer is a surface at the point rather than the sea bed.
const PROBE_DOWN: f32 = 20.0;
/// The queue item's threshold: a port floor more than this far below the
/// authored y is the finding the lead decides on.
const FLOOR_TOLERANCE: f32 = 0.3;
/// How many of a wet post's nearest neighbours to probe (the item asks for
/// "three of its props"; five, because two of KAS2's nearest are the post's own
/// pair of dock sections).
const NEAR_PROPS: usize = 5;
/// ...within this XZ radius of the post.
const NEAR_RADIUS: f32 = 30.0;
/// A post counts as standing in the water when it is in a wet cell and its
/// authored y is within this much of the plane (KAS2's beach post is 5 cm
/// under it, YAV1's bazaar post 28 cm over it).
const WATER_POST_BAND: f32 = 3.0;
/// How many of the under-plane rows to print; the count and the median cover
/// the rest.
const UNDER_ROWS: usize = 25;

/// One authored point to probe.
struct Probe {
    /// "post" or "prop" — a command post marker, or any other instance.
    kind: &'static str,
    /// The instance's class (`yav_bldg_bazaar`), which is what the RE names.
    class: String,
    /// The instance's own name (`bazzar1`, `cp1`).
    name: String,
    pos: Vec3,
    /// Why this row is in the table.
    why: String,
}

/// What the port answers at a probe.
struct Row {
    probe: Probe,
    /// Raw heightfield height at the XZ; `None` on a map with no terrain.
    terrain: Option<f32>,
    /// A terrain cutter removes the sheet at the authored point.
    cut: bool,
    /// [`Sim::ground_under`] at the authored point.
    floor: f32,
    /// Highest walkable model surface in [auth+PROBE_UP, auth−PROBE_DOWN].
    model: Option<f32>,
    /// The XZ is inside a wet cell of the baked water plane.
    wet: bool,
    /// The authored y measured from the water plane (+ = above it).
    above_water: f32,
}

impl Row {
    /// `floor − authored`: negative = the port's ground is below the disc's prop.
    fn delta(&self) -> f32 {
        self.floor - self.probe.pos.y
    }
    /// The port has NO floor under the authored point: no model surface, and
    /// the terrain sheet does not apply (the point is inside a cutter, or more
    /// than `TERRAIN_BURY` = 2 m UNDER the sheet — a pier piling driven into
    /// the sea bed is exactly that). A different fact from a floor that is
    /// merely lower, so it is counted apart and never enters the deltas.
    fn no_floor(&self) -> bool {
        self.floor <= core::collision::NO_GROUND + 0.01
    }
    /// The item's flag: the port's floor is more than 0.3 m below the authored y.
    fn flagged(&self) -> bool {
        !self.no_floor() && self.probe.pos.y - self.floor > FLOOR_TOLERANCE
    }
}

/// The wet footprint of a baked water plane: one XZ box per drawn triangle.
///
/// The bake emits two triangles per wet cell over a shared axis-aligned
/// lattice. Each triangle's XZ bounding box covers the full wet square, so
/// each square contributes two equal footprints. A point is wet iff some
/// box contains it. Only INDEXED vertices
/// count — the lattice itself spans the whole map whether or not a cell is wet.
fn wet_boxes(glb: &Path) -> Vec<[f32; 4]> {
    let Ok(mesh) = game::assets::load_glb(glb) else {
        return Vec::new();
    };
    mesh.indices
        .as_chunks::<3>()
        .0
        .iter()
        .filter_map(|t| {
            let p: Vec<[f32; 3]> = t
                .iter()
                .map(|&i| *mesh.positions.get(i as usize).unwrap_or(&[0.0; 3]))
                .collect();
            let (mut x0, mut x1) = (f32::MAX, f32::MIN);
            let (mut z0, mut z1) = (f32::MAX, f32::MIN);
            for v in &p {
                x0 = x0.min(v[0]);
                x1 = x1.max(v[0]);
                z0 = z0.min(v[2]);
                z1 = z1.max(v[2]);
            }
            (x1 > x0 && z1 > z0).then_some([x0, z0, x1, z1])
        })
        .collect()
}

fn is_wet(boxes: &[[f32; 4]], x: f32, z: f32) -> bool {
    boxes
        .iter()
        .any(|b| x >= b[0] && x <= b[2] && z >= b[1] && z <= b[3])
}

/// Probe one authored point against the built world.
fn measure(sim: &Sim, probe: Probe, boxes: &[[f32; 4]], water_h: f32) -> Row {
    let p = probe.pos;
    let terrain = sim.heightfield.as_ref().map(|h| h.height_at(p.x, p.z));
    let cut = sim.collision.is_cut(p);
    let floor = sim.ground_under(p, false);
    let model = sim
        .collision
        .ray_down(Vec3::new(p.x, p.y + PROBE_UP, p.z), PROBE_UP + PROBE_DOWN)
        .map(|(y, _n)| y);
    let wet = is_wet(boxes, p.x, p.z);
    let above_water = p.y - water_h;
    Row {
        probe,
        terrain,
        cut,
        floor,
        model,
        wet,
        above_water,
    }
}

fn f2(v: Option<f32>) -> String {
    v.map_or_else(|| "     —".to_owned(), |y| format!("{y:6.2}"))
}

fn print_rows<R: std::borrow::Borrow<Row>>(rows: &[R]) {
    println!(
        "  {:5} {:30} {:>8} {:>8} {:>8} {:>8} {:>7} {:>4} {:>8} {:>7} {:>8}  why",
        "kind",
        "class/name",
        "x",
        "z",
        "auth_y",
        "y-water",
        "terrain",
        "cut",
        "floor",
        "model",
        "fl-auth",
    );
    for r in rows {
        let r: &Row = r.borrow();
        let p = &r.probe;
        println!(
            "  {:5} {:30} {:8.2} {:8.2} {:8.2} {:8.2} {:>7} {:>4} {:>8} {:>7} {:>8}{} {}{}",
            p.kind,
            format!("{}/{}", p.class, p.name)
                .chars()
                .take(30)
                .collect::<String>(),
            p.pos.x,
            p.pos.z,
            p.pos.y,
            r.above_water,
            f2(r.terrain),
            if r.cut { "yes" } else { "no" },
            if r.no_floor() {
                "   none".to_owned()
            } else {
                format!("{:7.2}", r.floor)
            },
            f2(r.model),
            if r.no_floor() {
                "      —".to_owned()
            } else {
                format!("{:7.2}", r.delta())
            },
            match (r.no_floor(), r.flagged()) {
                (true, _) => " ?",
                (_, true) => " !",
                _ => "  ",
            },
            p.why,
            if r.above_water < 0.0 {
                ", UNDER the plane"
            } else {
                ""
            },
        );
    }
}

/// One map's table. Returns (rows printed, rows flagged).
fn run_map(key: &str, dir: &Path) -> (usize, usize) {
    let (scene, load) = match game::assets::load_battle_scene(dir, "con") {
        Ok(s) => s,
        Err(e) => {
            println!("{key}: LOAD FAILED {e:#}");
            return (0, 0);
        }
    };
    if let Some((_, Err(e))) = load.common {
        println!("{key}: common assets unreadable: {e:#}");
    }
    let Some(water) = scene.manifest.water.clone() else {
        println!("{key}: no water plane");
        return (0, 0);
    };
    let boxes = wet_boxes(&scene.dir.join(&water.glb));

    let layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let mut sim = Sim::empty(0);
    sim.heightfield = scene.heightfield();
    scene.install_collision(&mut sim);
    let probe = |kind: &'static str, class: &str, name: &str, pos: Vec3, why: String| {
        measure(
            &sim,
            Probe {
                kind,
                class: class.to_owned(),
                name: name.to_owned(),
                pos,
                why,
            },
            &boxes,
            water.height,
        )
    };

    // The wet footprint comes from the water GLB's own quads, so its grain is
    // the disc's `WMAP` CELL: KAS2's are 128 m and hold dry land as well as
    // sea, YAV1's 32 m. "In a wet cell" is therefore a NEIGHBOURHOOD test;
    // `y-water < 0` — the authored point is under the plane — is the crisp one.
    let cell = boxes
        .first()
        .map_or(0.0, |b| (b[2] - b[0]).max(b[3] - b[1]));
    let wet_instances = scene
        .manifest
        .instances
        .iter()
        .filter(|i| is_wet(&boxes, i.translation[0], i.translation[2]))
        .count();
    // The footprint's own extent, so the table can be checked against the RE's
    // census of it (`re-water.md` §2 gives YAV1's as x −32..256, z −64..32).
    let ext = boxes
        .iter()
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |a, b| {
            [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ]
        });
    // Legacy stdout calls these "drawn quads"; boxes.len() counts triangle footprints.
    println!(
        "\n{key}: water {:+.2} m ({}), {} drawn quads of {cell:.0} m over x {:.0}..{:.0} z {:.0}..{:.0}, \
         {wet_instances} instances in a wet cell of {}; probe [auth+{PROBE_UP}, auth−{PROBE_DOWN}], \
         flag at {FLOOR_TOLERANCE} m",
        water.height,
        water.texture.as_deref().unwrap_or("no texture"),
        boxes.len(),
        ext[0],
        ext[2],
        ext[1],
        ext[3],
        scene.manifest.instances.len(),
    );

    // 0 — free points, when the caller names some: `WATER_FLOOR_POINTS="x,z;x,z"`.
    // A post or a prop reports the floor at ITS OWN xz, which on a plaza full
    // of columns is each column's own mesh; naming points is how a reader
    // checks the floor BETWEEN them. The probe starts at the plane, so the
    // band is [water+2, water−20].
    let points: Vec<Vec3> = std::env::var("WATER_FLOOR_POINTS")
        .unwrap_or_default()
        .split(';')
        .filter(|s| !s.trim().is_empty())
        .filter_map(|pair| {
            let (x, z) = pair.split_once(',')?;
            Some(Vec3::new(
                x.trim().parse().ok()?,
                water.height,
                z.trim().parse().ok()?,
            ))
        })
        .collect();
    if !points.is_empty() {
        let rows: Vec<Row> = points
            .iter()
            .map(|p| probe("point", "-", "-", *p, "WATER_FLOOR_POINTS".to_owned()))
            .collect();
        println!("\n  [0] the points named in WATER_FLOOR_POINTS (auth_y = the plane)");
        print_rows(&rows);
    }

    // 1 — every command post, in layout order. The walkabout sheets are named
    // after these, which is how the defect table points at a place.
    let posts: Vec<Row> = layout
        .cps
        .iter()
        .map(|cp| {
            probe(
                "post",
                "com_bldg_controlzone",
                &cp.name,
                cp.pos,
                "command post".to_owned(),
            )
        })
        .collect();
    println!("\n  [1] every command post");
    print_rows(&posts);

    // 2 — the props AT each water-side post: the nearest instances by XZ, so
    // the pick is the data's and not a hand-written list.
    let water_posts: Vec<&Row> = posts
        .iter()
        .filter(|r| r.wet && r.above_water.abs() <= WATER_POST_BAND)
        .collect();
    let mut near_rows: Vec<Row> = Vec::new();
    for post in &water_posts {
        let p = post.probe.pos;
        let mut near: Vec<(f32, &game::assets::ManifestInstance)> = scene
            .manifest
            .instances
            .iter()
            .map(|i| {
                let d = Vec3::new(i.translation[0], 0.0, i.translation[2])
                    .distance(Vec3::new(p.x, 0.0, p.z));
                (d, i)
            })
            .filter(|(d, _)| *d <= NEAR_RADIUS)
            .collect();
        // Distance, then class and name: a deterministic order even at a tie.
        near.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then_with(|| a.1.class.cmp(&b.1.class))
                .then_with(|| a.1.name.cmp(&b.1.name))
        });
        for (d, i) in near.into_iter().take(NEAR_PROPS) {
            near_rows.push(probe(
                "prop",
                &i.class,
                &i.name,
                Vec3::from_array(i.translation),
                format!("{d:.1} m from {}", post.probe.name),
            ));
        }
    }
    println!(
        "\n  [2] the {NEAR_PROPS} nearest instances of each post standing in the water \
         ({} of them, |y-water| <= {WATER_POST_BAND} m and in a wet cell)",
        water_posts.len()
    );
    print_rows(&near_rows);

    // 3 — everything the disc authors UNDER the plane, worst floor first.
    // Only instances the map DRAWS: half of KAS2's under-plane list is
    // `com_snd_amb_static` ambient emitters and vehicle spawn points, which
    // are invisible and cannot be the "crates sunk to their lids" the defect
    // table saw. The count left out is in the section header.
    let under_all = scene
        .manifest
        .instances
        .iter()
        .filter(|i| {
            i.translation[1] < water.height && is_wet(&boxes, i.translation[0], i.translation[2])
        })
        .count();
    let mut under: Vec<Row> = scene
        .manifest
        .instances
        .iter()
        .filter(|i| {
            i.translation[1] < water.height
                && is_wet(&boxes, i.translation[0], i.translation[2])
                && scene.resolve_model(&i.class).is_some()
        })
        .map(|i| {
            probe(
                "prop",
                &i.class,
                &i.name,
                Vec3::from_array(i.translation),
                format!("{:.2} m under the plane", water.height - i.translation[1]),
            )
        })
        .collect();
    // Worst floor first, and the rows the port gives no floor at all LAST:
    // they are a different finding and would otherwise fill the printed head.
    under.sort_by(|a, b| {
        a.no_floor()
            .cmp(&b.no_floor())
            .then_with(|| a.delta().total_cmp(&b.delta()))
            .then_with(|| a.probe.class.cmp(&b.probe.class))
            .then_with(|| a.probe.name.cmp(&b.probe.name))
    });
    let under_flagged = under.iter().filter(|r| r.flagged()).count();
    let under_nofloor = under.iter().filter(|r| r.no_floor()).count();
    let mut ds: Vec<f32> = under
        .iter()
        .filter(|r| !r.no_floor())
        .map(Row::delta)
        .collect();
    ds.sort_by(f32::total_cmp);
    let median = ds.get(ds.len() / 2).copied().unwrap_or(f32::NAN);
    println!(
        "\n  [3] DRAWN instances authored UNDER the plane ({} of {under_all}; the rest draw nothing): \
         {} of them, {under_flagged} flagged (!), \
         {under_nofloor} with no floor at all (?), floor-auth median {median:+.2} m over the rest; \
         worst {UNDER_ROWS} first",
        under.len(),
        under.len(),
    );
    print_rows(&under.iter().take(UNDER_ROWS).collect::<Vec<_>>());

    // 4 — the CONTROL. A prop's authored y is its PIVOT, and a pivot is not
    // promised to sit on the prop's own floor, so "the floor is 0.5 m under the
    // authored y" only means something against the same statistic over every
    // instance on the map — the dry ones included. The `NO_GROUND` rows are
    // counted separately: there the port has no floor at all under the prop
    // (a cut, or a prop the map hangs in the air), which is a different fact
    // from a floor that is merely lower.
    let floors: Vec<(f32, f32)> = scene
        .manifest
        .instances
        .iter()
        .map(|i| {
            let p = Vec3::from_array(i.translation);
            let f = sim.ground_under(p, false);
            (f, f - p.y)
        })
        .collect();
    let q = |v: &[f32], f: f32| -> f32 {
        if v.is_empty() {
            return f32::NAN;
        }
        v[((v.len() - 1) as f32 * f).round() as usize]
    };
    let stats = |name: &str, rows: &[(f32, f32)]| {
        let no_ground = rows
            .iter()
            .filter(|(f, _)| *f <= core::collision::NO_GROUND + 0.01)
            .count();
        let mut d: Vec<f32> = rows
            .iter()
            .filter(|(f, _)| *f > core::collision::NO_GROUND + 0.01)
            .map(|(_, d)| *d)
            .collect();
        d.sort_by(f32::total_cmp);
        let deep = d.iter().filter(|x| **x < -FLOOR_TOLERANCE).count();
        println!(
            "      {name:22} n {:4}  no floor {no_ground:4}  p05 {:+6.2}  median {:+6.2}  p95 {:+6.2}  \
             more than {FLOOR_TOLERANCE} m below: {deep:4} ({:.0} %)",
            rows.len(),
            q(&d, 0.05),
            q(&d, 0.5),
            q(&d, 0.95),
            100.0 * deep as f32 / d.len().max(1) as f32,
        );
    };
    println!("\n  [4] control — floor-auth over every instance, against the under-plane set");
    stats("every instance", &floors);
    stats(
        "under the plane",
        &under
            .iter()
            .map(|r| (r.floor, r.delta()))
            .collect::<Vec<_>>(),
    );

    let all: Vec<&Row> = posts
        .iter()
        .chain(near_rows.iter())
        .chain(under.iter())
        .collect();
    let flagged = all.iter().filter(|r| r.flagged()).count();
    let nofloor = all.iter().filter(|r| r.no_floor()).count();
    println!(
        "\n  {key}: {} rows ({} posts, {} at-post, {} under the plane), {flagged} flagged \
         (the port's floor more than {FLOOR_TOLERANCE} m below the authored y), {nofloor} with no floor",
        all.len(),
        posts.len(),
        near_rows.len(),
        under.len(),
    );
    (all.len(), flagged)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = game::assets::baked_root();
    let keys: Vec<String> = if args.first().is_some_and(|a| a == "all") {
        game::assets::discover_maps(&root)
            .into_iter()
            .filter(|(_, _, d)| d.join("water.glb").exists())
            .map(|(k, _, _)| k)
            .collect()
    } else if args.is_empty() {
        DEFAULT_MAPS.iter().map(|s| (*s).to_owned()).collect()
    } else {
        args
    };

    println!(
        "The port's floor under the disc's water-side props (queue item WL).\n\
         auth_y = the manifest translation; terrain = the raw heightfield; \
         floor = Sim::ground_under at the authored point;\n\
         model = the highest walkable model surface in the probe band; \
         fl-auth = floor − auth_y (negative: the port's ground is lower)."
    );
    let (mut total, mut flagged) = (0usize, 0usize);
    for key in &keys {
        let dir: PathBuf = root.join(key);
        if !dir.join("manifest.ron").exists() {
            println!("\n{key}: not baked");
            continue;
        }
        let (n, f) = run_map(key, &dir);
        total += n;
        flagged += f;
    }
    println!(
        "\ntotal: {total} rows over {} maps, {flagged} flagged",
        keys.len()
    );
}
