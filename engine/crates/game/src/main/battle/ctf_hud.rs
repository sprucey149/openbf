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

fn ctf_flag_target(
    walk: &WalkSim,
    ctf: &core::sim::CtfState,
    flag: &core::sim::Flag,
    me_team: u8,
) -> Option<(glam::Vec3, bool, bool)> {
    let (pos, delivery, friendly) = if ctf.mode == core::sim::CtfMode::OneFlag {
        let (pos, delivery) = game::minimap::ctf_marker(&walk.sim, flag, me_team)?;
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
                walk.sim
                    .unit(carrier)
                    .is_some_and(|u| walk.sim.allied(me_team, u.team)),
            ),
            _ => (flag.current_pos, false, true),
        }
    };
    let pos = if delivery {
        pos + glam::Vec3::Y * 1.3
    } else {
        pos
    };
    Some((pos, delivery, friendly))
}

pub(super) fn hud_hints(
    walk: &WalkSim,
    feed: &mut game::event_feed::EventFeed,
    slot: usize,
    aspect: f32,
    stereo_view: Option<(&render::Camera, f32)>,
) -> Vec<ui::hud_tree::HintMarker> {
    let locked = walk.sim.lock_on_target(slot).map(|(t, _)| t);
    let (camera, view_aspect) = stereo_view.map_or_else(
        || (walk.camera(), aspect),
        |(cam, stereo_aspect)| (*cam, stereo_aspect),
    );
    let stereo_world_objectives = stereo_view.is_some();
    let view_proj = camera.view_proj(view_aspect);
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

    if let Some(ctf) = walk.sim.ctf.as_ref().filter(|_| walk.campaign.is_none()) {
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
            let Some((pos, delivery, friendly)) = ctf_flag_target(walk, ctf, flag, me_team) else {
                continue;
            };
            let (ndc, on_screen) = marker_screen(view_proj, pos);
            if stereo_world_objectives && on_screen {
                continue;
            }
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

    if let Some(c) = walk.campaign.as_ref() {
        let obj_flyer = walk
            .sim
            .unit(walk.player)
            .and_then(|u| u.mount)
            .and_then(|m| walk.sim.vehicles.get(m.vehicle.index as usize))
            .is_some_and(|v| v.kind == core::vehicle::VehicleKind::Flyer);
        for m in c.objective_markers(&walk.sim) {
            let pos = m.pos;
            let (ndc, on_screen) = marker_screen(view_proj, pos);
            if stereo_world_objectives && on_screen {
                continue;
            }
            hints.push(ui::hud_tree::HintMarker {
                ndc,
                distance: (pos - camera.position).length(),
                friendly: true,
                visible: visible(&walk.sim, walk.player, None, camera.position, pos),
                on_screen,
                flag: false,
                carrier: false,
                flyer_view: obj_flyer,
                own_carrier: false,
                objective: true,
                pulse: m
                    .pulse
                    .then_some((walk.sim.tick as f32 * core::TICK_DT) % 1.0),
                tint: Some([220.0, 175.0, 20.0]),
            });
        }
    }

    hints
}

fn signed_distance_polygon(p: glam::Vec2, poly: &[glam::Vec2]) -> f32 {
    let mut d = (p - poly[0]).length_squared();
    let mut sign = 1.0f32;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + n - 1) % n];
        let e = b - a;
        let w = p - a;
        let q = w - e * (w.dot(e) / e.length_squared().max(1e-8)).clamp(0.0, 1.0);
        d = d.min(q.length_squared());
        let c = [p.y >= a.y, p.y < b.y, e.x * w.y > e.y * w.x];
        if c.iter().all(|&b| b) || c.iter().all(|&b| !b) {
            sign = -sign;
        }
    }
    sign * d.sqrt()
}

fn srgb_to_linear(channel_u8: f32) -> f32 {
    let c = (channel_u8 / 255.0).clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn objective_marker_rgba() -> Vec<u8> {
    const SIZE: usize = 64;
    // Fallback when no baked HUD art is loaded: 3 stacked downward wedges matching BF2's silhouette.
    let tris: [[glam::Vec2; 3]; 3] = [-0.78, -0.26, 0.26].map(|top| {
        [
            glam::Vec2::new(-0.68, top),
            glam::Vec2::new(0.68, top),
            glam::Vec2::new(0.0, top + 0.38),
        ]
    });
    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let p = glam::Vec2::new(
                (x as f32 + 0.5) / (SIZE as f32 * 0.5) - 1.0,
                (y as f32 + 0.5) / (SIZE as f32 * 0.5) - 1.0,
            );
            let sd = tris
                .iter()
                .map(|tri| signed_distance_polygon(p, tri))
                .fold(f32::INFINITY, f32::min);
            let idx = (y * SIZE + x) * 4;
            let aa = 0.045;
            if sd <= 0.0 {
                rgba[idx..idx + 4].copy_from_slice(&[255, 255, 255, 255]);
            } else if sd < 0.11 {
                let edge_t = (sd / aa).clamp(0.0, 1.0);
                let luma = (255.0 * (1.0 - edge_t) + 24.0 * edge_t).round() as u8;
                let outer_a = 1.0 - ((sd - 0.065) / aa).clamp(0.0, 1.0);
                let alpha = (235.0 * outer_a).round().clamp(0.0, 255.0) as u8;
                rgba[idx..idx + 4].copy_from_slice(&[luma, luma, luma, alpha]);
            }
        }
    }
    rgba
}

pub(crate) fn objective_marker_texture(r: &mut render::Renderer) -> Option<render::TextureHandle> {
    let rgba = objective_marker_rgba();
    r.add_texture(&render::TextureDesc {
        width: 64,
        height: 64,
        mips: &[&rgba],
        sampler: render::SamplerMode::Linear,
        wrap_v: render::WrapV::ClampToEdge,
        srgb: true,
        detail: None,
        scroll: [0.0, 0.0],
        uv_scale: [1.0, 1.0],
        label: Some("objective marker fallback"),
    })
    .ok()
}

#[derive(Clone, Copy)]
pub(crate) struct UploadedMarkerStyle {
    pub mesh: render::MeshHandle,
    pub texture: render::TextureHandle,
    pub color: [f32; 3],
    pub friendly: Option<[f32; 3]>,
    pub alpha: f32,
    pub min_scale: f32,
    pub max_scale: f32,
}

#[derive(Default)]
pub(crate) struct WorldMarkerAssets {
    pub objective: [Option<UploadedMarkerStyle>; 2],
    pub flag: [Option<UploadedMarkerStyle>; 2],
    pub fallback: Option<render::TextureHandle>,
}

pub(crate) fn upload_world_marker_assets(
    r: &mut render::Renderer,
    hud_view: Option<&game::hud_feed::HudView>,
) -> WorldMarkerAssets {
    use std::collections::HashMap;
    use std::path::PathBuf;

    let mut tex_cache: HashMap<(PathBuf, String), Option<render::TextureHandle>> = HashMap::new();
    let mut upload_style = |style: Option<game::hud_feed::WorldMarkerStyle>| -> Option<UploadedMarkerStyle> {
        let s = style?;
        let key = (s.texture_dir.clone(), s.texture_name.clone());
        let texture = *tex_cache.entry(key).or_insert_with(|| {
            game::viewer::upload_texture_file(r, &s.texture_dir, &s.texture_name)
                .ok()
                .flatten()
        });
        let texture = texture?;
        let [x0, y0, x1, y1] = s.rect;
        let [u0, v0, u1, v1] = s.uv;
        let mesh = r
            .add_mesh(&render::MeshData {
                positions: &[
                    [x0, y0, 0.0],
                    [x1, y0, 0.0],
                    [x1, y1, 0.0],
                    [x0, y1, 0.0],
                ],
                normals: &[[0.0, 0.0, 1.0]; 4],
                uvs: &[[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
                colors: None,
                indices: &[0, 1, 2, 0, 2, 3],
            })
            .ok()?;
        Some(UploadedMarkerStyle {
            mesh,
            texture,
            color: s.color,
            friendly: s.friendly,
            alpha: s.alpha,
            min_scale: s.min_scale,
            max_scale: s.max_scale,
        })
    };

    let (objective, flag) = match hud_view {
        Some(hv) => {
            let (obj_styles, flag_styles) = hv.world_marker_styles();
            (
                [
                    upload_style(obj_styles[0].clone()),
                    upload_style(obj_styles[1].clone()),
                ],
                [
                    upload_style(flag_styles[0].clone()),
                    upload_style(flag_styles[1].clone()),
                ],
            )
        }
        None => ([None, None], [None, None]),
    };

    let fallback = objective_marker_texture(r);
    WorldMarkerAssets {
        objective,
        flag,
        fallback,
    }
}

struct WorldMarkerTarget {
    pos: glam::Vec3,
    friendly: bool,
    objective: bool,
    pulse: Option<f32>,
    tint: Option<[f32; 3]>,
    fallback_scale: f32,
}

fn collect_world_marker_targets(walk: &WalkSim) -> Vec<WorldMarkerTarget> {
    let mut out = Vec::new();
    if let Some(c) = walk.campaign.as_ref() {
        let pulse = (walk.sim.tick as f32 * core::TICK_DT) % 1.0;
        for m in c.objective_markers(&walk.sim) {
            out.push(WorldMarkerTarget {
                pos: m.pos,
                friendly: true,
                objective: true,
                pulse: m.pulse.then_some(pulse),
                tint: Some([220.0, 175.0, 20.0]),
                fallback_scale: m.scale.clamp(0.75, 2.0),
            });
        }
    } else if let Some(ctf) = walk.sim.ctf.as_ref() {
        let me_team = walk.sim.unit(walk.player).map_or(0, |u| u.team);
        let yellow = Some(ui::hud::message_color::YELLOW.map(f32::from));
        let pulse = (ctf.mode == core::sim::CtfMode::OneFlag)
            .then_some((walk.sim.tick as f32 * core::TICK_DT) % 1.0);
        for flag in &ctf.flags {
            if let Some((pos, delivery, friendly)) = ctf_flag_target(walk, ctf, flag, me_team) {
                out.push(WorldMarkerTarget {
                    pos,
                    friendly,
                    objective: delivery,
                    pulse,
                    tint: yellow,
                    fallback_scale: 1.0,
                });
            }
        }
    }
    out
}

pub(crate) fn world_objective_draws(
    walk: &WalkSim,
    camera: &render::Camera,
    quad: render::MeshHandle,
    assets: &WorldMarkerAssets,
) -> Vec<render::DrawCmd> {
    world_objective_draws_with_fallback(walk, camera, quad, assets, false)
}

fn baked_marker_model_and_rgba(
    pos: glam::Vec3,
    camera: &render::Camera,
    dist: f32,
    min_scale: f32,
    max_scale: f32,
    alpha: f32,
    base_color: [f32; 3],
    friendly_color: Option<[f32; 3]>,
    marker_tint: Option<[f32; 3]>,
    friendly: bool,
    pulse: Option<f32>,
) -> (glam::Mat4, [f32; 4]) {
    const LOCK_RAMP_NEAR: f32 = 200.0;
    const LOCK_RAMP_FAR: f32 = 1700.0;
    const DESKTOP_FOV_Y: f32 = 0.95;

    let right = camera.right();
    let up = right.cross(camera.forward()).normalize_or_zero();
    let forward = camera.forward();
    let t = ((dist - LOCK_RAMP_NEAR) / (LOCK_RAMP_FAR - LOCK_RAMP_NEAR)).clamp(0.0, 1.0);
    let mut s = t * max_scale + (1.0 - t) * min_scale;
    if let Some(pulse) = pulse {
        s = s * (1.0 - pulse) + 0.9;
    }
    let world_scale = 2.0 * dist.clamp(2.5, 400.0) * (DESKTOP_FOV_Y * 0.5).tan() * s;
    let tint = marker_tint.unwrap_or_else(|| friendly_color.filter(|_| friendly).unwrap_or(base_color));
    let rgba = [
        srgb_to_linear(tint[0]),
        srgb_to_linear(tint[1]),
        srgb_to_linear(tint[2]),
        alpha.clamp(0.0, 1.0),
    ];
    let basis = glam::Mat3::from_cols(
        right * world_scale,
        up * world_scale,
        -forward * world_scale,
    );
    let model = glam::Mat4::from_translation(pos) * glam::Mat4::from_mat3(basis);
    (model, rgba)
}

fn world_objective_draws_with_fallback(
    walk: &WalkSim,
    camera: &render::Camera,
    quad: render::MeshHandle,
    assets: &WorldMarkerAssets,
    allow_untextured_fallback: bool,
) -> Vec<render::DrawCmd> {
    let targets = collect_world_marker_targets(walk);
    if targets.is_empty() {
        return Vec::new();
    }

    let right = camera.right();
    let up = right.cross(camera.forward()).normalize_or_zero();
    let forward = camera.forward();
    let time = walk.sim.tick as f32 * core::TICK_DT;
    let fallback_pulse_mul = 1.0 + 0.14 * (time * std::f32::consts::TAU).sin();

    targets
        .into_iter()
        .filter_map(|m| {
            let is_visible = visible(&walk.sim, walk.player, None, camera.position, m.pos);
            let slot = usize::from(!is_visible);
            let dist = (m.pos - camera.position).length().max(1.0);
            let style = if m.objective {
                assets.objective[slot].as_ref().or(assets.objective[0].as_ref())
            } else {
                assets.flag[slot].as_ref().or(assets.flag[0].as_ref())
            };

            if let Some(style) = style {
                let (model, rgba) = baked_marker_model_and_rgba(
                    m.pos,
                    camera,
                    dist,
                    style.min_scale,
                    style.max_scale,
                    style.alpha,
                    style.color,
                    style.friendly,
                    m.tint,
                    m.friendly,
                    m.pulse,
                );
                return Some(render::DrawCmd::overlay(style.mesh, style.texture, rgba, model));
            }

            let tex = match assets.fallback {
                Some(tex) => Some(tex),
                None if allow_untextured_fallback => None,
                None => return None,
            };
            let base_size = (dist * 0.055).clamp(0.45, 9.0) * m.fallback_scale;
            let size = if m.pulse.is_some() {
                base_size * fallback_pulse_mul
            } else {
                base_size
            };
            let anchor = m.pos + glam::Vec3::Y * (base_size * 0.55);
            let basis = glam::Mat3::from_cols(right * size, up * size, -forward * size);
            let model = glam::Mat4::from_translation(anchor) * glam::Mat4::from_mat3(basis);
            let tint = m.tint.unwrap_or([220.0, 175.0, 20.0]);
            let alpha = if is_visible { 0.95 } else { 0.62 };
            Some(render::DrawCmd {
                texture: tex,
                layer: render::DrawLayer::Overlay,
                light: render::LightPath::Unlit,
                ..render::DrawCmd::flat(
                    quad,
                    [
                        srgb_to_linear(tint[0]),
                        srgb_to_linear(tint[1]),
                        srgb_to_linear(tint[2]),
                        alpha,
                    ],
                    model,
                )
            })
        })
        .collect()
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
        let hints = hud_hints(&walk, &mut feed, 0, 16.0 / 9.0, None);
        assert!(hints
            .iter()
            .any(|h| h.carrier && h.friendly && !h.on_screen));
        assert!(hints.iter().any(|h| h.objective && !h.on_screen));
    }

    #[test]
    fn stereo_view_hides_on_screen_flag_hints_and_emits_world_overlay_draw() {
        let mut walk = WalkSim::new(1, glam::Vec3::ZERO, None);
        let camera = walk.camera();
        let ahead = camera.position + camera.forward() * 40.0;
        let behind = camera.position - camera.forward() * 40.0;
        let mut ctf = core::sim::CtfState::new_space_one_flag(5, ahead, ahead, ahead);
        ctf.flags[0].current_pos = ahead;
        walk.sim.ctf = Some(ctf);

        let mut feed = game::event_feed::EventFeed::default();
        let desktop_hints = hud_hints(&walk, &mut feed, 0, 16.0 / 9.0, None);
        assert!(desktop_hints.iter().any(|h| h.flag && h.on_screen));

        // On-screen in stereo camera -> hidden from 2D visor HUD, shown in 3D overlay draws.
        let stereo_on = hud_hints(&walk, &mut feed, 0, 1.0, Some((&camera, 1.0)));
        assert!(!stereo_on.iter().any(|h| h.flag));

        let mut turned = camera;
        turned.yaw += std::f32::consts::PI;
        let stereo_off = hud_hints(&walk, &mut feed, 0, 1.0, Some((&turned, 1.0)));
        assert!(stereo_off.iter().any(|h| h.flag && !h.on_screen));

        // Behind marker check to ensure world draws track actual positions.
        walk.sim.ctf.as_mut().unwrap().flags[0].current_pos = behind;
        let quad = render::MeshHandle::default();
        let assets = WorldMarkerAssets::default();
        let draws = world_objective_draws_with_fallback(&walk, &camera, quad, &assets, true);
        assert_eq!(draws.len(), 1);
        assert_eq!(draws[0].layer, render::DrawLayer::Overlay);
        assert_eq!(draws[0].light, render::LightPath::Unlit);
        assert!(draws[0].model.w_axis.y > behind.y);
    }

    #[test]
    fn objective_marker_icon_has_three_stacked_chevrons_and_transparent_corners() {
        let rgba = objective_marker_rgba();
        assert_eq!(rgba.len(), 64 * 64 * 4);
        let px = |x: usize, y: usize| -> [u8; 4] {
            let idx = (y * 64 + x) * 4;
            rgba[idx..idx + 4].try_into().unwrap()
        };
        assert_eq!(px(0, 0), [0, 0, 0, 0]);
        // Three stacked downward chevrons along the vertical centerline.
        assert_eq!(px(32, 12), [255, 255, 255, 255]);
        assert_eq!(px(32, 29), [255, 255, 255, 255]);
        assert_eq!(px(32, 45), [255, 255, 255, 255]);
    }

    #[test]
    fn srgb_to_linear_matches_standard_transfer() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(255.0) - 1.0).abs() < 1e-6);
        assert!(srgb_to_linear(128.0) < 0.25);
    }

    #[test]
    fn baked_marker_scale_and_pulse_match_desktop_hud_ramp() {
        let cam = render::Camera::default();
        let pos = glam::Vec3::new(0.0, 2.0, -200.0);
        let (m0, rgba0) = baked_marker_model_and_rgba(
            pos,
            &cam,
            200.0,
            2.0,
            1.0,
            0.85,
            [255.0, 255.0, 255.0],
            None,
            Some([220.0, 175.0, 20.0]),
            true,
            Some(0.0),
        );
        let (m1, _) = baked_marker_model_and_rgba(
            pos,
            &cam,
            200.0,
            2.0,
            1.0,
            0.85,
            [255.0, 255.0, 255.0],
            None,
            Some([220.0, 175.0, 20.0]),
            true,
            Some(0.5),
        );
        assert!(m0.x_axis.length() > m1.x_axis.length());
        assert!((m0.w_axis.truncate() - pos).length() < 1e-5);
        assert!((rgba0[3] - 0.85).abs() < 1e-6);
        assert!((rgba0[0] - srgb_to_linear(220.0)).abs() < 1e-6);
    }
}
