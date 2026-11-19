//! Owned-data regression for the donor Hailfire's complete vehicle bake.
use std::path::PathBuf;

fn check(key: &str, label: &str, segments: usize) {
    let source = std::env::var(key).expect(key);
    let out = PathBuf::from(std::env::var("OPENBF_HAILFIRE_OUT").expect("OPENBF_HAILFIRE_OUT"))
        .join(label);
    assert!(!out.exists(), "use a fresh owned output directory");
    bake::vehicle::bake_vehicles(&source, out.to_str().unwrap(), None).unwrap();
    let bytes = std::fs::read(out.join("models/cis_tread_hailfire.glb")).unwrap();
    let glb = gltf::Gltf::from_slice(&bytes).unwrap();
    let primitives: Vec<_> = glb.meshes().next().unwrap().primitives().collect();
    assert_eq!(primitives.len(), segments + 2);
    for (i, primitive) in primitives.iter().enumerate() {
        let reader = primitive.reader(|_| glb.blob.as_deref());
        let positions = reader.read_positions().unwrap().count();
        let joints = reader
            .read_joints(0)
            .unwrap()
            .into_u16()
            .collect::<Vec<_>>();
        assert_eq!(joints.len(), positions);
        assert_eq!(
            reader.read_weights(0).unwrap().into_f32().count(),
            positions
        );
        if i >= segments {
            assert_eq!(positions, 884);
            assert!(joints.iter().all(|b| *b == joints[0]));
        }
    }
}

#[test]
#[ignore = "requires OPENBF_HAILFIRE_PC and a fresh OPENBF_HAILFIRE_OUT"]
fn pc_hailfire_vehicle_bakes_with_complete_skin() {
    check("OPENBF_HAILFIRE_PC", "pc", 6);
}

#[test]
#[ignore = "requires OPENBF_HAILFIRE_XBOX and a fresh OPENBF_HAILFIRE_OUT"]
fn xbox_hailfire_vehicle_bakes_with_complete_skin() {
    check("OPENBF_HAILFIRE_XBOX", "xbox", 41);
}
