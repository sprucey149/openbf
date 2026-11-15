use super::*;

/// Step every cloth of one baked wearer through idle, a run and a second
/// stance while its model frame moves; the pins must follow their bones and
/// the free vertices stay finite. Returns (cloths, pose updates, moved).
fn exercise(
    side: &std::path::Path,
    geometry: &str,
    set: impl Fn(&crate::animator::Rig) -> usize,
) -> (usize, usize, usize) {
    let asset =
        crate::assets::load_skinned_glb(&side.join("soldiers").join(format!("{geometry}.glb")))
            .unwrap();
    let raw: Vec<Cloth> =
        ron::from_str(&pak::read_to_string(manifest::cloth::path(side, geometry)).unwrap())
            .unwrap();
    let rig = crate::soldier_vis::SoldierVisuals::load_rig(side, geometry, &asset).unwrap();
    let set = set(&rig);
    let (mut count, mut frames, mut moved) = (0, 0, 0);
    for cloth in raw {
        assert!(cloth.part < asset.mesh.submeshes.len());
        let def = Definition::new(cloth, &asset.skeleton).unwrap();
        let mut state = State::new(&def, Mat4::IDENTITY, &asset.skeleton.global_bind, 7);
        let mut displacement = 0.0f32;
        for i in 0..180 {
            let body = match i / 60 {
                0 => crate::animator::Body::Feet(0, crate::animator::Move::Idle),
                1 => crate::animator::Body::Feet(0, crate::animator::Move::RunForward),
                _ => crate::animator::Body::Feet(1, crate::animator::Move::Idle),
            };
            let pose = rig.still((&asset.clips, &asset.skeleton), set, body, None, i as f32);
            let palette = asset.skeleton.palette(&pose.locals());
            let globals: Vec<_> = palette
                .iter()
                .zip(&asset.skeleton.global_bind)
                .map(|(p, b)| *p * *b)
                .collect();
            let t = i as f32 / 30.0;
            let model =
                Mat4::from_translation(Vec3::new(t * 0.3, 0., 0.)) * Mat4::from_rotation_y(t * 0.4);
            state.step(&def, model, &globals, 1. / 30.);
            for (p, expected) in state.positions.iter().zip(def.pin_positions(&globals)) {
                assert!(p.distance(expected) < 1e-4, "{}: pin drift", def.raw.name);
            }
            assert!(state
                .vertices(&def, &globals)
                .iter()
                .all(|v| Vec3::from(v.position).is_finite()));
            for (p, rest) in state
                .positions
                .iter()
                .zip(&def.raw.positions)
                .skip(def.raw.pinned)
            {
                displacement = displacement.max(p.distance(Vec3::from(*rest)));
            }
            frames += 1;
        }
        assert!(
            displacement < 1000.,
            "{}: runaway {displacement}",
            def.raw.name
        );
        moved += usize::from(displacement > 0.01);
        count += 1;
        eprintln!(
            "{geometry} / {}: {} vertices, {} pins, max displacement {displacement:.3}m",
            def.raw.name,
            def.raw.positions.len(),
            def.raw.pinned
        );
    }
    (count, frames, moved)
}

#[test]
fn every_hero_cloth_keeps_live_pins_and_finite_moving_geometry() {
    #[derive(serde::Deserialize)]
    struct Heroes {
        heroes: Vec<Hero>,
    }
    #[derive(serde::Deserialize)]
    struct Hero {
        class: String,
        geometry: String,
    }
    let root = crate::assets::baked_root();
    let side = root.join("side_bf2");
    let path = side.join("heroes.ron");
    if !pak::exists(&path) {
        eprintln!("SKIP: BF2 hero closure is not baked");
        return;
    }
    let heroes: Heroes = ron::from_str(&pak::read_to_string(path).unwrap()).unwrap();
    let db =
        crate::roster::load_classdb(&root.join("classes"), "bf2", &crate::roster::FACTION_ORDER)
            .unwrap();
    let mut seen = std::collections::HashSet::new();
    let (mut count, mut frames, mut moved) = (0, 0, 0);
    for hero in heroes.heroes {
        if !seen.insert(hero.geometry.clone())
            || !pak::exists(manifest::cloth::path(&side, &hero.geometry))
        {
            continue;
        }
        let anim = crate::roster::class_anim(&db, &hero.class, &db.weapon_slots(&hero.class));
        let (c, f, m) = exercise(&side, &hero.geometry, |rig| {
            rig.weapon_set(
                &anim.family,
                anim.banks.first().map_or("rifle", |(_, b)| b.as_str()),
            )
        });
        count += c;
        frames += f;
        moved += m;
    }
    assert!(count > 0 && moved > 0);
    eprintln!("hero cloths: {count}, pose updates {frames}, moved {moved}");
}

/// The troops' cloth (the Hoth snowtroopers' skirt, the clone capes) is live,
/// not only the heroes': the soldier bake writes a companion for every
/// wearer, and every cloth in every companion moves.
#[test]
fn every_baked_cloth_companion_steps_live() {
    let side = crate::assets::baked_root().join("side_bf2");
    if !pak::exists(manifest::cloth::path(&side, "imp_inf_snowtrooper")) {
        eprintln!("SKIP: the BF2 snowtrooper's cloth is not baked");
        return;
    }
    let (mut count, mut frames, mut moved) = (0, 0, 0);
    for path in pak::read_dir(side.join("soldiers")).unwrap() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some(geometry) = name.strip_suffix(".cloths.ron") else {
            continue;
        };
        let (c, f, m) = exercise(&side, geometry, |rig| rig.weapon_set("human", "rifle"));
        count += c;
        frames += f;
        moved += m;
    }
    assert!(count > 0 && moved == count, "{moved} of {count} moved");
    eprintln!("baked cloths: {count}, pose updates {frames}, moved {moved}");
}

fn sheet() -> Cloth {
    Cloth {
        name: "sheet".into(),
        part: 0,
        parent: "root".into(),
        frame: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0., 0., 0.]],
        positions: vec![[0., 0., 0.], [1., 0., 0.], [1., -1., 0.]],
        uvs: vec![[0.; 2]; 3],
        pinned: 1,
        pin_bones: vec!["root".into()],
        triangles: vec![[0, 1, 2]],
        constraints: Default::default(),
        enabled: [true; 3],
        shapes: vec![],
        wind_direction: [1., 0., 0.],
        wind_speed: 0.,
        dampening: 0.,
        drag: 0.,
        mass: 1.,
        max_acceleration: 20.,
    }
}

#[test]
fn free_vertices_fall_while_a_live_bone_moves_the_pin() {
    let skel = crate::skin::Skeleton::new(vec!["root".into()], vec![None], vec![Mat4::IDENTITY]);
    let raw = sheet();
    let def = Definition::new(raw, &skel).unwrap();
    let mut state = State::new(&def, Mat4::IDENTITY, &[Mat4::IDENTITY], 0);
    let moved = [Mat4::from_translation(Vec3::new(0., 0.5, 0.))];
    state.step(&def, Mat4::IDENTITY, &moved, 1.0 / 30.0);
    let vertices = state.vertices(&def, &moved);
    assert_eq!(vertices[0].position, [0., 0.5, 0.]);
    assert!((state.positions[1].y + 9.8 / 900.0).abs() < 1e-6);
    assert_eq!(state.positions[1].x, 1.);
}

#[test]
fn rejects_nonfinite_force_data_and_singular_frames() {
    let skel = crate::skin::Skeleton::new(vec!["root".into()], vec![None], vec![Mat4::IDENTITY]);
    for case in 0..4 {
        let mut raw = sheet();
        match case {
            0 => raw.wind_speed = f32::NAN,
            1 => raw.frame[0] = [0.; 3],
            2 => raw.frame[3][1] = f32::INFINITY,
            _ => raw.shapes.push(manifest::cloth::Shape {
                bone: "root".into(),
                kind: 0,
                params: [f32::NAN, 0., 0.],
                frame: raw.frame,
            }),
        }
        assert!(
            Definition::new(raw, &skel).is_err(),
            "bad case {case} accepted"
        );
    }
}

#[test]
fn constraints_and_authored_collision_shapes_project_as_on_ps2() {
    let mut p = [Vec3::ZERO, Vec3::new(0., -2., 0.)];
    constrain(&mut p, 1, 0, 1, 1.);
    assert_eq!(p, [Vec3::ZERO, Vec3::NEG_Y]);
    let m = Mat4::from_translation(Vec3::new(3., 2., 1.));
    let mut p = m.transform_point3(Vec3::new(0.25, 0., 0.));
    collide(&mut p, 0, Vec3::splat(1.), m);
    assert_eq!(p, m.transform_point3(Vec3::X));
    let mut p = Vec3::new(0.8, 0., 0.);
    collide(&mut p, 1, Vec3::new(1., 1., 0.), Mat4::IDENTITY);
    assert_eq!(p, Vec3::X);
    let mut p = Vec3::new(-0.8, 0., 0.);
    collide(&mut p, 2, Vec3::ONE, Mat4::IDENTITY);
    assert_eq!(p, Vec3::NEG_X);
    let rotated = m * Mat4::from_rotation_y(0.7) * Mat4::from_rotation_z(-0.4);
    let mut p = rotated.transform_point3(Vec3::new(-0.8, 0., 0.));
    collide(&mut p, 2, Vec3::ONE, rotated);
    assert!(p.distance(rotated.transform_point3(Vec3::NEG_X)) < 1e-5);
}

#[test]
fn pins_use_noncommuting_live_bone_and_cloth_frames() {
    let root = Mat4::from_translation(Vec3::new(2., 1., -1.)) * Mat4::from_rotation_y(0.6);
    let child = Mat4::from_translation(Vec3::new(0.2, 0.4, -0.1)) * Mat4::from_rotation_z(0.8);
    let skel = crate::skin::Skeleton::new(
        vec!["root".into(), "pin".into()],
        vec![None, Some(0)],
        vec![root, child],
    );
    let frame = Mat4::from_translation(Vec3::new(0.1, 0.2, 0.3)) * Mat4::from_rotation_x(-0.4);
    let cols = frame.to_cols_array_2d();
    let raw = Cloth {
        name: "rotated".into(),
        part: 0,
        parent: "root".into(),
        frame: std::array::from_fn(|i| [cols[i][0], cols[i][1], cols[i][2]]),
        positions: vec![[0.3, 0.2, 0.1]],
        uvs: vec![[0.; 2]],
        pinned: 1,
        pin_bones: vec!["pin".into()],
        triangles: vec![],
        constraints: Default::default(),
        enabled: [true; 3],
        shapes: vec![],
        wind_direction: [1., 0., 0.],
        wind_speed: 10.,
        dampening: 0.5,
        drag: -2.,
        mass: 1.,
        max_acceleration: 20.,
    };
    let def = Definition::new(raw, &skel).unwrap();
    let local_pin = (root * child)
        .inverse()
        .transform_point3((root * frame).transform_point3(Vec3::new(0.3, 0.2, 0.1)));
    let live_root = Mat4::from_translation(Vec3::new(-1., 2., 3.)) * Mat4::from_rotation_x(0.2);
    let live_pin = live_root * Mat4::from_rotation_y(-0.5) * child;
    let live = [live_root, live_pin];
    let mut state = State::new(&def, Mat4::IDENTITY, &skel.global_bind, 9);
    state.step(&def, Mat4::IDENTITY, &live, 1. / 30.);
    let drawn = Vec3::from(state.vertices(&def, &live)[0].position);
    assert!(
        drawn.distance(live_pin.transform_point3(local_pin)) < 1e-5,
        "{drawn:?}"
    );
}
