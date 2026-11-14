use super::seat_camera::{map, missions_of};
use core::sim::{buttons, PadState};
use core::vehicle::VehicleSpec;

#[test]
fn hailfire_gatling_has_an_audible_loop_from_the_players_seat() {
    let root = game::assets::baked_root();
    let key = "bf2_geo1";
    let mission = missions_of(key)
        .into_iter()
        .next()
        .expect("Geonosis mission");
    let mut m = map(&root.join(key), &mission.factions()).expect("Geonosis bake");
    m.walk.walker_shot_styles = m.build.walker_shot_styles.clone();
    let spec = m
        .build
        .pads
        .iter()
        .flat_map(|p| {
            p.classes.iter().flatten().map(|c| VehicleSpec {
                pos: p.pos,
                yaw: p.yaw,
                ..c.spec.clone()
            })
        })
        .find(|s| m.build.classes[s.class as usize] == "cis_tread_hailfire")
        .expect("Hailfire pad");
    let h = m.walk.sim.spawn_vehicle(&spec);
    let player = m.walk.player;
    assert!(m.walk.sim.seat_unit(player, h, 0));
    let mut heard = 0;
    let lib = game::sounds::SoundLib::load(&root.join(key)).expect("Geonosis sound bank");
    for _ in 0..60 {
        m.walk.sim.step(&[PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        }]);
        for (unit, _, name, pos) in game::walker_shots::loops(&m.walk) {
            if unit != player {
                continue;
            }
            let (_, _, samples, _) = lib.audio_of_name(name).expect("gatling loop definition");
            assert!(!samples.is_empty(), "gatling loop must name a bank sample");
            let take = lib
                .take_path_by_hash(game::classdb::fnv1a_lowercase(name), 0)
                .expect("baked gatling loop take");
            assert!(take.is_file(), "{}", take.display());
            if heard == 0 {
                let decoded = audio::decode(&take).expect("decodable gatling loop");
                assert!(decoded.duration_secs() > 0.0 && decoded.frames() > 0);
            }
            let listener = game::sounds::Listener {
                pos: m.walk.camera().position,
                yaw: m.walk.camera().yaw,
            };
            let voice = game::engine_sound::Voice {
                vehicle: h.index,
                soldier: false,
                layer: 0,
                record: 0,
                sound: game::classdb::fnv1a_lowercase(name),
                pos,
                gain_factor: 1.0,
                pitch_factor: 1.0,
                speed: 0.0,
            };
            let ((left, right), rate) = lib
                .engine_level(&voice, &listener)
                .expect("audible from seat");
            assert!(left.max(right) > 0.0 && rate > 0.0);
            heard += 1;
        }
    }
    assert!(heard >= 10, "only {heard} firing ticks have a loop");
    m.walk.sim.step(&[PadState::default()]);
    assert!(game::walker_shots::loops(&m.walk)
        .iter()
        .all(|(u, ..)| *u != player));
}
