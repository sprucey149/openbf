use super::*;
use core::fmath::FloatExt;

fn visible(
    sim: &core::sim::Sim,
    viewer: core::unit::UnitHandle,
    target: Option<core::unit::UnitHandle>,
    from: glam::Vec3,
    to: glam::Vec3,
) -> bool {
    let target = target.unwrap_or(viewer);
    core::perception::los_seat(
        sim,
        from,
        to,
        core::perception::past_mounts(sim, viewer, target),
        core::perception::past_buildings(sim, viewer, target),
    )
}

pub(super) fn marker_screen(view_proj: glam::Mat4, pos: glam::Vec3) -> ([f32; 2], bool) {
    let clip = view_proj * pos.extend(1.0);
    // Divide by |w|, not w: a target behind the camera has w < 0, and the
    // signed divide would mirror it to the wrong edge. |w| keeps the sign of
    // the view-space x/y so the clamped marker points toward the target
    // (`re-hud-map.md` §5, `FUN_00179c10`).
    let w = clip.w.abs().fmax(1.0e-4);
    let (mut x, mut y) = (clip.x / w, -clip.y / w);
    let bound = x.abs().fmax(y.abs());
    const EDGE: f32 = 0.9;
    let on_screen = clip.w > 0.0 && bound <= EDGE;
    if !on_screen && bound > 0.0 {
        x *= EDGE / bound;
        y *= EDGE / bound;
    }
    ([x, y], on_screen)
}

pub(super) fn hud_hints(
    walk: &WalkSim,
    feed: &mut game::event_feed::EventFeed,
    slot: usize,
    aspect: f32,
) -> Vec<ui::hud_tree::HintMarker> {
    let locked = walk.sim.lock_on_target(slot).map(|(t, _)| t);
    let camera = walk.camera();
    let view_proj = camera.view_proj(aspect);
    let mut hints: Vec<ui::hud_tree::HintMarker> = feed
        .target_hints(&walk.sim, slot)
        .iter()
        .filter(|h| Some(h.target) != locked)
        .filter_map(|h| {
            let (ndc, on_screen) = marker_screen(view_proj, h.point);
            on_screen.then(|| ui::hud_tree::HintMarker {
                ndc,
                distance: (h.point - camera.position).length(),
                friendly: h.friendly,
                visible: h.visible,
                on_screen: true,
                flag: false,
                carrier: false,
                flyer_view: false,
                own_carrier: false,
                objective: false,
                pulse: None,
                tint: None,
            })
        })
        .collect();

    if let Some(ctf) = &walk.sim.ctf {
        let me_team = walk.sim.unit(walk.player).map_or(0, |u| u.team);
        let flyer_view = walk
            .sim
            .unit(walk.player)
            .and_then(|u| u.mount)
            .and_then(|m| walk.sim.vehicle(m.vehicle))
            .is_some_and(|v| v.kind == core::vehicle::VehicleKind::Flyer);
        let yellow = Some(ui::hud::message_color::YELLOW.map(f32::from));
        for flag in &ctf.flags {
            if let core::sim::FlagStatus::Carried { carrier } = flag.status {
                if let Some((pos, friendly)) =
                    game::minimap::carrier_marker(&walk.sim, carrier, walk.player)
                {
                    let (ndc, on_screen) = marker_screen(view_proj, pos);
                    hints.push(ui::hud_tree::HintMarker {
                        ndc,
                        distance: (pos - camera.position).length(),
                        friendly,
                        visible: visible(
                            &walk.sim,
                            walk.player,
                            Some(carrier),
                            camera.position,
                            pos,
                        ),
                        on_screen,
                        flag: false,
                        carrier: true,
                        flyer_view,
                        own_carrier: carrier == walk.player,
                        objective: false,
                        pulse: None,
                        tint: Some(if friendly {
                            [0.0, 0.0, 255.0]
                        } else {
                            [175.0, 0.0, 0.0]
                        }),
                    });
                }
            }
            let (pos, delivery, friendly) = if ctf.mode == core::sim::CtfMode::OneFlag {
                let Some((pos, delivery)) = game::minimap::ctf_marker(&walk.sim, flag, me_team)
                else {
                    continue;
                };
                (pos, delivery, true)
            } else {
                match flag.status {
                    core::sim::FlagStatus::Carried { carrier } if carrier == walk.player => (
                        ctf.capture_regions
                            .get(usize::from(me_team))
                            .map_or(flag.current_pos, |r| r.pos),
                        false,
                        true,
                    ),
                    core::sim::FlagStatus::Carried { carrier } => (
                        flag.current_pos,
                        false,
                        walk.sim.unit(carrier).is_some_and(|u| u.team == me_team),
                    ),
                    _ => (flag.current_pos, false, true),
                }
            };
            let pos = if delivery {
                pos + glam::Vec3::Y * 1.3
            } else {
                pos
            };
            let (ndc, on_screen) = marker_screen(view_proj, pos);
            hints.push(ui::hud_tree::HintMarker {
                ndc,
                distance: (pos - camera.position).length(),
                friendly,
                visible: visible(&walk.sim, walk.player, None, camera.position, pos),
                on_screen,
                flag: true,
                carrier: false,
                flyer_view,
                own_carrier: false,
                objective: delivery,
                pulse: (ctf.mode == core::sim::CtfMode::OneFlag)
                    .then_some((walk.sim.tick as f32 * core::TICK_DT) % 1.0),
                tint: yellow,
            });
        }
    }

    hints
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flag_marker_visibility_changes_when_a_wall_blocks_it() {
        let mut sim = core::sim::Sim::empty(1);
        let viewer = sim
            .spawn(&core::unit::UnitSpec::trooper(0, glam::Vec3::ZERO))
            .unwrap();
        let eye = glam::Vec3::new(0.0, 2.0, 0.0);
        let target = glam::Vec3::new(0.0, 2.0, -20.0);
        assert!(visible(&sim, viewer, None, eye, target));
        sim.collision.push_body(core::collision::Body::new(
            glam::Mat4::from_translation(glam::Vec3::new(0.0, 2.0, -10.0)),
            core::collision::Shape::Box {
                half: glam::Vec3::new(5.0, 5.0, 0.5),
            },
        ));
        assert!(!visible(&sim, viewer, None, eye, target));
        assert!(visible(
            &sim,
            viewer,
            None,
            eye,
            glam::Vec3::new(20.0, 2.0, 0.0)
        ));
    }

    #[test]
    fn offscreen_ally_keeps_both_carrier_and_delivery_markers() {
        let mut walk = WalkSim::new(1, glam::Vec3::ZERO, None);
        let camera = walk.camera();
        let behind = camera.position - camera.forward() * 100.0;
        let team = walk.sim.unit(walk.player).unwrap().team;
        let ally = walk
            .sim
            .spawn(&core::unit::UnitSpec::trooper(team, behind))
            .unwrap();
        walk.sim.unit_mut(ally).unwrap().soldier.pos = behind;
        let mut ctf = core::sim::CtfState::new_space_one_flag(5, glam::Vec3::ZERO, behind, behind);
        ctf.flags[0].status = core::sim::FlagStatus::Carried { carrier: ally };
        ctf.flags[0].current_pos = behind;
        walk.sim.ctf = Some(ctf);
        let mut feed = game::event_feed::EventFeed::default();
        let hints = hud_hints(&walk, &mut feed, 0, 16.0 / 9.0);
        assert!(hints
            .iter()
            .any(|h| h.carrier && h.friendly && !h.on_screen));
        assert!(hints.iter().any(|h| h.objective && !h.on_screen));
    }
}
