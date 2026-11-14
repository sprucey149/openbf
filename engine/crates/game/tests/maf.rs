use glam::Mat4;

#[path = "../examples/support/walker_battle.rs"]
mod seat_fixture;

fn ps2(rows: [[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array_2d(&rows)
}

fn report(name: &str, port: &Mat4, ps2: &Mat4) -> f32 {
    let d = (0..4)
        .map(|c| (port.col(c) - ps2.col(c)).abs().max_element())
        .fold(0.0, f32::max);
    println!(
        "{name}: |d| {d:.4}\n  port {:?}\n  ps2  {:?}",
        port.to_cols_array_2d(),
        ps2.to_cols_array_2d()
    );
    d
}

/// BF1 BES1, the maintainer's MAF save states of 2026-11-14 (`analysis/re-vehicle-pose.md` 6):
/// the PS2's node-table rows of a parked MAF and of one whose rear gunner looks 4.98 deg left.
#[test]
fn bf1_maf_rear_cannon_nodes_match_the_ps2_table() {
    let root = game::assets::baked_root();
    if !root.join("bf1_bes1/manifest.ron").is_file() {
        assert!(std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none());
        return;
    }
    let (mut walk, _) = seat_fixture::battle_on(&root, "bf1_bes1", "bf1_bes1_con_cw").unwrap();
    let class = walk
        .vehicle_classes
        .iter()
        .position(|c| c == "cis_fly_maf")
        .unwrap();
    let slot = walk
        .sim
        .vehicles
        .iter()
        .position(|v| v.alive && v.class as usize == class)
        .unwrap();
    let handle = core::vehicle::VehicleHandle {
        index: slot as u32,
        gen: walk.sim.vehicles[slot].gen,
    };
    let rig = walk.flyer_rigs[class]
        .as_ref()
        .expect("rebake the BF1 CIS vehicles")
        .clone();
    let names = [
        "turret_rearcannon",
        "aimer_rearcannon",
        "barrel_r_rearcannon",
    ];
    let index: Vec<usize> = names
        .iter()
        .map(|n| rig.skeleton.index_of(n).unwrap())
        .collect();
    walk.sim.vehicle_mut(handle).unwrap().flyer.lift = 1.0;
    let rest = rig.flyer_aimed_nodes(walk.sim.vehicle(handle).unwrap(), None);
    assert!(
        report(
            "rest turret_rearcannon",
            &rest[index[0]],
            &ps2([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 6.4939, -6.0256, 1.0],
            ]),
        ) < 1e-3
    );
    assert!(walk.sim.seat_unit(walk.player, handle, 2));
    walk.sim.arm(slot);
    {
        let v = walk.sim.vehicle_mut(handle).unwrap();
        let g = v.guns[2].as_mut().unwrap();
        g.look = ((-4.98f32).to_radians(), 1.8f32.to_radians());
        g.aim = g.look;
    }
    let now = rig.flyer_aimed_nodes(walk.sim.vehicle(handle).unwrap(), None);
    assert!(
        report(
            "turret_rearcannon",
            &now[index[0]],
            &ps2([
                [0.9962, -0.0, 0.0868, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [-0.0868, 0.0, 0.9962, 0.0],
                [0.0, 6.4939, -6.0256, 1.0],
            ]),
        ) < 1e-3
    );
    assert!(
        report(
            "aimer_rearcannon",
            &now[index[1]],
            &ps2([
                [0.9962, -0.0, 0.0868, 0.0],
                [0.0027, 0.9995, -0.0309, 0.0],
                [-0.0868, 0.0311, 0.9957, 0.0],
                [-0.2969, 6.7228, -2.6181, 1.0],
            ]),
        ) < 0.01
    );
    assert!(
        report(
            "barrel_r_rearcannon",
            &now[index[2]],
            &ps2([
                [0.9962, -0.0, 0.0868, 0.0],
                [0.0027, 0.9995, -0.0309, 0.0],
                [-0.0868, 0.0311, 0.9957, 0.0],
                [-0.2969, 6.7228, -2.6181, 1.0],
            ]),
        ) < 0.01
    );
}
