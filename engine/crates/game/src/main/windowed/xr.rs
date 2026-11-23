//! Route active display/spatial extensions and controller pointers through the game UI.

use super::App;
use extensions::runtime::{
    ActiveDisplayExtension, FrameContext, SimView, StereoView,
};

pub(super) fn live_view(walk: &game::walk::WalkSim) -> bool {
    walk.sim.outcome.is_none()
        && walk.sim.unit(walk.player).is_some_and(|unit| unit.alive)
        && walk
            .sim
            .spawn_stage
            .get(usize::from(walk.local_slot))
            .is_none_or(|stage| *stage == core::sim::SpawnStage::Accepted)
        && !walk.spectating()
}

pub(super) fn sim_view(walk: &game::walk::WalkSim) -> SimView<'_> {
    SimView {
        sim: &walk.sim,
        player: walk.player,
        local_slot: walk.local_slot,
        camera: walk.camera(),
        spectating: walk.spectating(),
        eye_clip: Some(walk),
    }
}

pub(super) fn listener(view: &StereoView) -> game::sounds::Listener {
    game::sounds::Listener {
        pos: view.listener.pos,
        yaw: view.listener.yaw,
    }
}

pub(super) fn prepare_soldiers(
    view: &StereoView,
    walk: &game::walk::WalkSim,
    visuals: &mut game::soldier_vis::SoldierVisuals,
) {
    let vp = view.eyes.map(|(camera, fov)| fov.view_projection(&camera));
    visuals.prepare_stereo_view(walk, &view.camera, view.aspect, vp);
}

/// Use the tracked gun or right wrist for the bolt and flash.
pub(super) fn place_tracked_muzzle(
    view: &StereoView,
    walk: &game::walk::WalkSim,
    visuals: &game::soldier_vis::SoldierVisuals,
    points: &mut [Option<glam::Vec3>],
) {
    if let Some((mount, slot)) =
        view.hand_mounts[1].zip(points.get_mut(walk.player.index as usize))
    {
        if let Some(point) = visuals.tracked_fire_point(walk, mount) {
            *slot = Some(point);
        }
    }
}

pub(super) fn stats_pointer(
    mut pad: ui::stats::Pad,
    walk: &game::walk::WalkSim,
    display_ext: Option<&dyn ActiveDisplayExtension>,
    renderer: Option<&render::Renderer>,
    hud: Option<&game::hud_feed::HudView>,
    pointer: &mut ui::cursor::Pointer,
) -> ui::stats::Pad {
    let Some((stats, (renderer, hud))) = display_ext
        .filter(|ext| ext.has_frame())
        .and_then(|_| walk.round_end.as_ref()?.stats.as_ref())
        .zip(renderer.zip(hud))
    else {
        return pad;
    };
    if let Some((point, true)) = pointer.take_frame() {
        let (w, h) = renderer.size();
        let hit = hud.stats_help_at(stats, point, [w as f32, h as f32]);
        pad.cross |= hit.cross;
        pad.triangle |= hit.triangle;
        pad.square |= hit.square;
    }
    pad
}

impl App {
    pub(super) fn renderer_for_window(
        &mut self,
        window: std::sync::Arc<winit::window::Window>,
    ) -> anyhow::Result<render::Renderer> {
        if let Some(early) = self.early_display.take() {
            let (renderer, active) = early.open_renderer(window)?;
            self.display_ext = Some(active);
            return Ok(renderer);
        }
        Ok(render::Renderer::new_windowed(window)?)
    }

    pub(super) fn begin_display_ext_frame(&mut self) -> anyhow::Result<()> {
        if self.display_ext.is_none() {
            return Ok(());
        }
        let battle = self.in_battle() && self.netplay.is_none();
        let walk = self.walk.as_ref().filter(|_| battle).map(|(walk, _)| walk);
        let stereo_allowed = !self.paused && walk.is_some_and(live_view);
        let tracked_guns = match (walk, self.soldier_vis.as_ref()) {
            (Some(walk), Some(vis)) if stereo_allowed => vis.tracked_guns(walk),
            _ => [None; 2],
        };
        let window_size = self.renderer.as_ref().map_or([1280, 720], |r| r.size().into());
        let sim_snap = walk.map(sim_view);
        let ctx = FrameContext {
            sim: sim_snap.as_ref(),
            stereo_allowed,
            tracked_guns,
            window_size,
        };
        let directives = self
            .display_ext
            .as_mut()
            .expect("checked display_ext")
            .begin_frame(&ctx)?;
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.capture_flat_frame(directives.capture_flat);
        }
        if let Some(cursor) = &directives.menu_cursor {
            if let Some(position) = cursor.position {
                self.shell_pointer.moved_to(position);
                self.shell_pointer.wheel(cursor.wheel);
                if cursor.click {
                    if matches!(self.screen, super::Screen::Loading) {
                        self.loading_press();
                    } else {
                        self.shell_pointer.click();
                    }
                }
            } else {
                self.shell_pointer.clear();
            }
        }
        if let Some(overlay) = self.overlay.as_mut() {
            overlay.set_controller_pointer(
                directives
                    .menu_cursor
                    .map(|cursor| (cursor.position, cursor.down, cursor.wheel)),
            );
        }
        Ok(())
    }

    pub(super) fn finish_redraw(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let (Some(ext), Some(renderer)) = (&mut self.display_ext, &self.renderer) {
            if let Err(error) = ext.finish_flat(renderer) {
                self.failure = Some(error);
                event_loop.exit();
                return;
            }
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

pub(super) fn render_battle(
    renderer: &mut render::Renderer,
    camera: &render::Camera,
    draws: &[render::DrawCmd],
    skinned: &[render::SkinnedDrawCmd],
    clear: [f64; 4],
    overlay: Option<render::OverlayFn<'_>>,
    stereo: Option<&mut (dyn ActiveDisplayExtension + 'static)>,
) -> anyhow::Result<()> {
    if let Some(ext) = stereo {
        return ext.finish_stereo(renderer, overlay);
    }
    Ok(renderer.render_frame(camera, draws, skinned, clear, overlay)?)
}
