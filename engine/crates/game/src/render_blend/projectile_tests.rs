use super::*;
use core::collision::HullFilter;
use core::projectile::{Bolt, Shooter, VisualShot};

fn round(walk: &WalkSim, class: usize, event: usize) -> Bolt {
    let o = &walk.sim.weapon_classes[class].ordnance;
    let mut b = Bolt::fire(
        Vec3::Y * 40.0,
        Vec3::Z,
        o.velocity,
        o.life_span,
        Shooter {
            owner: walk.player,
            team: 0,
            weapon_class: class,
            is_player: true,
        },
        None,
        None,
        &walk.sim.collision,
        HullFilter::Static,
    );
    b.visual_shot = Some(VisualShot { tick: 0, event });
    if let Some(c) = &o.missile {
        b.as_missile(o, c, None, &mut core::lcg::GameLcg::new(42));
    }
    b
}

#[test]
fn modelled_projectiles_interpolate_by_identity_and_restore_every_bit() {
    // BF1 jet-trooper bullets and missile-family launchers share this draw path.
    for missile in [false, true] {
        let mut walk = WalkSim::new(42, Vec3::ZERO, None);
        let mut w = core::weapon::WeaponClass::bf1_rifle();
        w.ordnance.geometry = Some("com_weap_missile".into());
        w.ordnance.velocity = 100.0;
        if missile {
            w.ordnance.kind = core::weapon::OrdKind::Missile;
            w.ordnance.missile = Some(core::missile::MissileClass::default());
        }
        let class = walk.sim.add_weapon_class(w);
        walk.sim.bolts = vec![round(&walk, class, 1), round(&walk, class, 2)];
        walk.sim.bolts[1].pos += Vec3::X * 10.0;
        let prev = Pose::capture(&walk);
        for b in &mut walk.sim.bolts {
            b.pos += Vec3::Z * (100.0 * core::TICK_DT);
            b.age += core::TICK_DT;
            b.vel += Vec3::X * 10.0;
        }
        // A removed round cannot donate its pose to the shifted survivor.
        walk.sim.bolts.remove(0);
        let mut fresh = round(&walk, class, 3);
        fresh.pos += Vec3::X * 20.0;
        walk.sim.bolts.push(fresh);
        let cur = walk.sim.bolts.clone();
        let hash = walk.sim.hash();
        let bits = format!("{:?}", walk.sim.bolts);
        let mut blend = Blend {
            on: true,
            prev: Some(prev),
            ..Default::default()
        };
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert!(blend.apply(&mut walk, fraction));
            let start = Vec3::new(10.0, 40.0, 0.0);
            let expected = start.lerp(cur[0].pos, fraction);
            assert!(walk.sim.bolts[0].pos.abs_diff_eq(expected, 1e-5));
            assert_eq!(walk.sim.bolts[1].pos, cur[1].pos, "new round snaps");
            let draws = crate::bolt_vis::missiles_in_flight(&walk);
            assert!(draws[0].1.w_axis.truncate().abs_diff_eq(expected, 1e-5));
            assert!(draws[0]
                .1
                .z_axis
                .truncate()
                .abs_diff_eq(walk.sim.bolts[0].vel.normalize(), 1e-5));
            blend.restore(&mut walk);
            assert_eq!(walk.sim.hash(), hash);
            assert_eq!(format!("{:?}", walk.sim.bolts), bits);
        }
    }
}
