//! OpenXR session, frame pacing, and extension presentation hooks.

use crate::actions::{Actions, Input, Pose};
use crate::gesture::{GrenadeGesture, SaberSamples};
use crate::graphics::{self, Surface};
use crate::panel::{MenuCursor, Placement};
use crate::rig;
use anyhow::{bail, Context, Result};
use core::sim::{PadState, TrackedCombat, TrackedHand, TrackedShot};
use extensions::runtime::{
    ActiveDisplayExtension, EarlyDisplayContext, FrameContext, FrameDirectives, ListenerPose,
    MenuButtons, MenuCursorFrame, SimView, StereoView, TrackedGun,
};
use extensions::OPENBF_VR_ID;
use glam::Vec3;
use openxr as xr;
use render::{
    DrawCmd, FrameTarget, OverlayFn, Renderer, ReticlePass, SkinnedDrawCmd, ViewFov,
    ViewProjection, Visor, VisorEye, VisorPass, WorldReticle,
};
use std::sync::Arc;

const STEREO: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;
const OPAQUE: xr::EnvironmentBlendMode = xr::EnvironmentBlendMode::OPAQUE;
type Session = xr::Session<xr::Vulkan>;

#[derive(Default)]
struct Diag {
    since: Option<std::time::Instant>,
    idle: u32,
    menu: u32,
    stereo: u32,
    empty: u32,
    reason: &'static str,
    view: Option<(Pose, Placement)>,
}

impl Diag {
    fn empty(&mut self, reason: &'static str) {
        self.empty += 1;
        self.reason = reason;
    }

    fn report(&mut self, state: xr::SessionState, focused: bool) {
        let now = std::time::Instant::now();
        let since = *self.since.get_or_insert(now);
        if now.duration_since(since).as_secs_f32() < 2.0 {
            return;
        }
        if self.empty == 0 && self.idle == 0 {
            *self = Self {
                since: Some(now),
                ..Self::default()
            };
            return;
        }
        let panel = self.view.map_or_else(
            || "no panel".to_string(),
            |(head, placement)| {
                let offset = placement.position - head.position;
                let forward = head.orientation * Vec3::NEG_Z;
                let angle = offset
                    .normalize_or_zero()
                    .dot(forward)
                    .clamp(-1.0, 1.0)
                    .acos()
                    .to_degrees();
                format!(
                    "panel {:.2} m away, {angle:.0}° off view, facing {:.2}",
                    offset.length(),
                    (placement.orientation * Vec3::Z).dot(-offset.normalize_or_zero())
                )
            },
        );
        println!(
            "OpenXR: {state:?}, focused {focused}; {} idle, {} menu, {} stereo, {} empty ({}); {panel}",
            self.idle, self.menu, self.stereo, self.empty, self.reason
        );
        *self = Self {
            since: Some(now),
            ..Self::default()
        };
    }
}

/// Loading the runtime and finding the headset can fail before creating a window.
pub(crate) struct Runtime {
    instance: xr::Instance,
    system: xr::SystemId,
}

impl Runtime {
    pub fn new() -> Result<Self> {
        let entry = load_entry()?;
        let available = entry
            .enumerate_extensions()
            .context("find the active OpenXR runtime")?;
        if !available.khr_vulkan_enable2 {
            bail!("the active OpenXR runtime does not support XR_KHR_vulkan_enable2");
        }
        let mut extensions = xr::ExtensionSet::default();
        extensions.khr_vulkan_enable2 = true;
        let instance = entry.create_instance(
            &xr::ApplicationInfo {
                application_name: "openbf",
                engine_name: "openbf",
                ..Default::default()
            },
            &extensions,
            &[],
            &(),
        )?;
        let system = instance
            .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
            .context("find an OpenXR headset; start the runtime and connect the headset")?;
        if !instance
            .enumerate_environment_blend_modes(system, STEREO)?
            .contains(&OPAQUE)
        {
            bail!("the OpenXR headset does not offer opaque stereo presentation");
        }
        Ok(Self { instance, system })
    }

    pub fn open(self, window: Arc<winit::window::Window>) -> Result<(Renderer, Vr)> {
        let renderer = graphics::window_renderer(&self.instance, self.system, window)?;
        let vr = Vr::new(self, &renderer)?;
        Ok((renderer, vr))
    }
}

impl EarlyDisplayContext for Runtime {
    fn extension_id(&self) -> &'static str {
        OPENBF_VR_ID
    }

    fn open_renderer(
        self: Box<Self>,
        window: Arc<winit::window::Window>,
    ) -> Result<(Renderer, Box<dyn ActiveDisplayExtension>)> {
        let (renderer, vr) = (*self).open(window)?;
        Ok((renderer, Box::new(vr)))
    }
}

fn load_entry() -> Result<xr::Entry> {
    if let Some(path) = std::env::var_os("OPENBF_XR_LOADER") {
        let path = std::path::Path::new(&path);
        // OPENBF_XR_LOADER is an explicit override for a trusted loader library.
        return unsafe { xr::Entry::load_from(path, &()) }
            .with_context(|| format!("load OpenXR from OPENBF_XR_LOADER={}", path.display()));
    }
    #[cfg(windows)]
    return xr::Entry::linked(&()).context("initialize the bundled OpenXR loader");
    #[cfg(target_os = "linux")]
    unsafe {
        xr::Entry::load(&())
            .context("load the system OpenXR loader (or set OPENBF_XR_LOADER to its absolute path)")
    }
}

struct Eye {
    world: FrameTarget,
    surface: Surface,
}

struct Frame {
    time: xr::Time,
    views: [xr::View; 2],
    head: Pose,
    input: Input,
    guns: [Option<TrackedGun>; 2],
    stereo: bool,
    visor: Option<Visor>,
    reticle: Option<(WorldReticle, [(render::Camera, ViewFov); 2])>,
}

fn live_view(view: &SimView<'_>) -> bool {
    view.sim.outcome.is_none()
        && view.sim.unit(view.player).is_some_and(|unit| unit.alive)
        && view
            .sim
            .spawn_stage
            .get(usize::from(view.local_slot))
            .is_none_or(|stage| *stage == core::sim::SpawnStage::Accepted)
        && !view.spectating
}

pub(crate) struct Vr {
    instance: xr::Instance,
    session: Session,
    waiter: xr::FrameWaiter,
    stream: xr::FrameStream<xr::Vulkan>,
    local: xr::Space,
    view: xr::Space,
    actions: Actions,
    hands: [[xr::Space; 2]; 2],
    eyes: [Eye; 2],
    panel: Option<Surface>,
    format: graphics::Format,
    hud_frame: Option<FrameTarget>,
    visor_pass: VisorPass,
    reticle_frame: Option<FrameTarget>,
    reticle_pass: ReticlePass,
    menu_pose: Option<(Placement, i64)>,
    menu_cursor: MenuCursor,
    blit: wgpu::util::TextureBlitter,
    rig: rig::Rig,
    grenade: GrenadeGesture,
    saber: SaberSamples,
    running: bool,
    focused: bool,
    state: xr::SessionState,
    diag: Diag,
    recenter_at: Option<xr::Time>,
    frame: Option<Frame>,
    device: Arc<wgpu::Device>,
}

impl Vr {
    fn new(runtime: Runtime, renderer: &Renderer) -> Result<Self> {
        let Runtime { instance, system } = runtime;
        let device = renderer.device().clone();
        let config = instance.enumerate_view_configuration_views(system, STEREO)?;
        let [left, right] = config.as_slice() else {
            bail!("expected two OpenXR views, got {}", config.len());
        };
        let (session, waiter, stream) = graphics::create_session(&instance, system, &device)?;
        let actions = Actions::new(&instance, &session)?;
        let hands = actions.spaces(&session)?;
        let local =
            session.create_reference_space(xr::ReferenceSpaceType::LOCAL, xr::Posef::IDENTITY)?;
        let view =
            session.create_reference_space(xr::ReferenceSpaceType::VIEW, xr::Posef::IDENTITY)?;
        let format = graphics::Format::choose(&session)?;
        let eye = |desc: &xr::ViewConfigurationView| -> Result<Eye> {
            let size = [
                desc.recommended_image_rect_width,
                desc.recommended_image_rect_height,
            ];
            Ok(Eye {
                world: renderer.frame_target(size)?,
                surface: Surface::new(&session, &device, size, format)?,
            })
        };
        let eyes = [eye(left)?, eye(right)?];
        let blit = wgpu::util::TextureBlitter::new(&device, format.0);
        let visor_pass = VisorPass::new(&device, renderer.surface_format());
        let reticle_pass = ReticlePass::new(&device, renderer.surface_format());
        println!(
            "OpenXR: {} using {} ({}×{} / {}×{} per eye)",
            instance.properties()?.runtime_name,
            renderer.adapter_info().name,
            eyes[0].surface.size[0],
            eyes[0].surface.size[1],
            eyes[1].surface.size[0],
            eyes[1].surface.size[1]
        );
        Ok(Self {
            instance,
            session,
            waiter,
            stream,
            local,
            view,
            actions,
            hands,
            eyes,
            panel: None,
            format,
            hud_frame: None,
            visor_pass,
            reticle_frame: None,
            reticle_pass,
            menu_pose: None,
            menu_cursor: MenuCursor::default(),
            blit,
            rig: Default::default(),
            grenade: Default::default(),
            saber: Default::default(),
            running: false,
            focused: false,
            state: xr::SessionState::UNKNOWN,
            diag: Diag::default(),
            recenter_at: None,
            frame: None,
            device,
        })
    }

    fn events(&mut self) -> Result<()> {
        let mut buffer = xr::EventDataBuffer::new();
        while let Some(event) = self.instance.poll_event(&mut buffer)? {
            match event {
                xr::Event::SessionStateChanged(change) => {
                    self.focused = change.state() == xr::SessionState::FOCUSED;
                    self.state = change.state();
                    println!("OpenXR session state: {:?}", change.state());
                    match change.state() {
                        xr::SessionState::READY => {
                            self.session.begin(STEREO)?;
                            self.running = true;
                        }
                        xr::SessionState::STOPPING => {
                            self.running = false;
                            self.session.end()?;
                            self.recenter();
                        }
                        xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                            bail!("the OpenXR session ended");
                        }
                        _ => {}
                    }
                }
                xr::Event::ReferenceSpaceChangePending(change)
                    if change.reference_space_type() == xr::ReferenceSpaceType::LOCAL =>
                {
                    println!("OpenXR: the LOCAL space is recentering");
                    self.recenter_at = Some(change.change_time());
                }
                xr::Event::InstanceLossPending(_) => bail!("the OpenXR runtime disconnected"),
                _ => {}
            }
        }
        Ok(())
    }

    fn begin_internal(
        &mut self,
        view: Option<&SimView<'_>>,
        stereo: bool,
        tracked_guns: [Option<TrackedGun>; 2],
    ) -> Result<()> {
        // A screen transition may have bypassed the battle render.
        if let Some(frame) = self.frame.take() {
            self.diag.empty("frame not drawn");
            self.stream.end(frame.time, OPAQUE, &[])?;
        }
        self.events()?;
        self.diag.report(self.state, self.focused);
        if !self.running {
            self.diag.idle += 1;
            self.recenter();
            self.rig.track(view, None, Input::default(), false);
            self.rig.update_support(None, Input::default(), [None; 2]);
            return Ok(());
        }
        let state = self.waiter.wait()?;
        self.stream.begin()?;
        let time = state.predicted_display_time;
        if self
            .recenter_at
            .is_some_and(|change| time.as_nanos() >= change.as_nanos())
        {
            self.recenter_at = None;
            self.recenter();
        }
        if stereo {
            self.menu_pose = None;
            self.menu_cursor = MenuCursor::default();
        }
        let sample = if state.should_render {
            self.sample(time, stereo)
        } else {
            self.diag.empty("runtime asked not to render");
            Ok(None)
        };
        match sample {
            Ok(Some(mut frame)) => {
                if !stereo {
                    let now = time.as_nanos();
                    let placement = self
                        .menu_pose
                        .map(|(placement, at)| {
                            placement
                                .follow(frame.head, now.saturating_sub(at).max(0) as f32 * 1e-9)
                        })
                        .filter(|placement| placement.is_finite())
                        .unwrap_or_else(|| Placement::menu(frame.head));
                    self.menu_pose = Some((placement, now));
                    self.diag.view = Some((frame.head, placement));
                }
                self.rig
                    .track(view, Some(frame.head), frame.input, stereo && self.focused);
                if stereo {
                    frame.guns = tracked_guns;
                }
                let grip = Input {
                    support_grip: frame.input.support_grip && !self.grenade.holding(),
                    ..frame.input
                };
                self.rig.update_support(view, grip, frame.guns);
                let saber_hands = view
                    .filter(|_| stereo && self.focused)
                    .and_then(|view| {
                        let unit = rig::infantry(view)?;
                        view.sim.weapon_classes[unit.weapon_class].saber.as_ref()?;
                        let world = self.rig.world(Some(view), view.camera, frame.head);
                        let mounts = world.hand_mounts(frame.input.hands);
                        Some(std::array::from_fn(|side| {
                            frame.input.hands[side].grip?;
                            let (_, rotation, position) =
                                mounts[side]?.to_scale_rotation_translation();
                            Some(TrackedHand::new(position - unit.soldier.pos, rotation))
                        }))
                    })
                    .unwrap_or_default();
                self.saber.sample(&mut self.rig.motion_epoch, saber_hands);
                let left_free = stereo
                    && self.focused
                    && view.is_some_and(|view| {
                        self.rig.foregrip(view).is_none()
                            && rig::infantry(view).is_some_and(|unit| {
                                view.sim.weapon_classes[unit.weapon_class]
                                    .saber
                                    .as_ref()
                                    .is_none_or(|saber| {
                                        !saber
                                            .blades
                                            .iter()
                                            .any(|blade| blade.tracked_hand() == Some(0))
                                    })
                            })
                    });
                self.grenade
                    .sample(view, frame.input, time.as_nanos(), left_free);
                self.frame = Some(frame);
                Ok(())
            }
            result => {
                if state.should_render {
                    self.diag.empty("head not tracked");
                }
                self.menu_cursor = MenuCursor::default();
                self.rig.track(view, None, Input::default(), false);
                self.rig.update_support(None, Input::default(), [None; 2]);
                self.grenade.reset();
                self.saber = Default::default();
                self.stream.end(time, OPAQUE, &[])?;
                result.map(|_| ())
            }
        }
    }

    fn sample(&self, time: xr::Time, stereo: bool) -> Result<Option<Frame>> {
        let (flags, views) = self.session.locate_views(STEREO, time, &self.local)?;
        let valid = xr::ViewStateFlags::POSITION_VALID | xr::ViewStateFlags::ORIENTATION_VALID;
        if !flags.contains(valid) {
            return Ok(None);
        }
        let views: [xr::View; 2] = views
            .try_into()
            .map_err(|_| anyhow::anyhow!("expected two OpenXR eye poses"))?;
        let Some(head) =
            Pose::locate(&self.view, &self.local, time, xr::SpaceLocationFlags::EMPTY)?
        else {
            return Ok(None);
        };
        let input = if self.focused {
            self.actions
                .sample(&self.session, &self.hands, &self.local, time)?
        } else {
            Input::default()
        };
        Ok(Some(Frame {
            time,
            views,
            head,
            input,
            guns: [None; 2],
            stereo,
            reticle: None,
            visor: stereo.then(|| {
                let inverse = head.orientation.conjugate();
                Visor::new(views.map(|eye| {
                    let pose = Pose::from_xr(eye.pose);
                    VisorEye {
                        offset: inverse * (pose.position - head.position),
                        orientation: inverse * pose.orientation,
                        fov: eye_fov(eye.fov),
                    }
                }))
            }),
        }))
    }

    fn menu_cursor(&mut self, size: [u32; 2]) -> Option<MenuCursorFrame> {
        let frame = self.frame.as_ref().filter(|frame| !frame.stereo)?;
        if !self.focused {
            self.menu_cursor = MenuCursor::default();
            return None;
        }
        let screen = self
            .menu_pose
            .map_or_else(|| Placement::menu(frame.head), |(placement, _)| placement);
        let position = frame.input.hands[1].aim.and_then(|aim| {
            let uv = screen.hit(aim.position, aim.orientation * Vec3::NEG_Z, size)?;
            Some([uv[0] * size[0] as f32, uv[1] * size[1] as f32])
        });
        let trigger = frame.input.buttons & core::sim::buttons::FIRE != 0;
        Some(self.menu_cursor.sample(
            position,
            trigger,
            [
                frame.input.movement.unwrap_or_default(),
                frame.input.right_stick,
            ],
            frame.time.as_nanos(),
        ))
    }

    fn blit_surface(
        &self,
        renderer: &Renderer,
        surface: &Surface,
        source: &FrameTarget,
    ) -> Result<()> {
        surface.draw(|target| {
            let mut encoder = self.device.create_command_encoder(&Default::default());
            graphics::clear(&mut encoder, target);
            self.blit
                .copy(&self.device, &mut encoder, source.view(), target);
            // Submit to the shared Vulkan queue before releasing the XR image.
            renderer.queue().submit([encoder.finish()]);
        })
    }

    fn panel_for(&mut self, size: [u32; 2]) -> Result<()> {
        if self.panel.as_ref().is_none_or(|panel| panel.size != size) {
            self.panel = Some(Surface::new(
                &self.session,
                &self.device,
                size,
                self.format,
            )?);
        }
        Ok(())
    }

    fn finish_eyes(
        &mut self,
        renderer: &Renderer,
        frame: &Frame,
        overlay: Option<OverlayFn<'_>>,
    ) -> Result<()> {
        if let (Some((reticle, eyes)), Some(source)) = (&frame.reticle, &self.reticle_frame) {
            let mut encoder = self.device.create_command_encoder(&Default::default());
            for (index, (eye, view)) in self.eyes.iter().zip(eyes).enumerate() {
                self.reticle_pass.draw(
                    renderer,
                    &mut encoder,
                    &eye.world,
                    source,
                    reticle,
                    view,
                    index,
                );
            }
            renderer.queue().submit([encoder.finish()]);
        }
        if let Some(overlay) = overlay {
            let held = self.hud_frame.take();
            let hud = self.paint_frame(renderer, held, overlay)?;
            if let Some(visor) = &frame.visor {
                let mut encoder = self.device.create_command_encoder(&Default::default());
                for (index, eye) in self.eyes.iter().enumerate() {
                    self.visor_pass
                        .draw(renderer, &mut encoder, &eye.world, &hud, visor, index);
                }
                renderer.queue().submit([encoder.finish()]);
            }
            self.hud_frame = Some(hud);
        }
        for eye in &self.eyes {
            self.blit_surface(renderer, &eye.surface, &eye.world)?;
        }
        Ok(())
    }

    /// Paint `overlay` over a clear window-sized frame, reusing `held`.
    fn paint_frame(
        &self,
        renderer: &Renderer,
        held: Option<FrameTarget>,
        overlay: OverlayFn<'_>,
    ) -> Result<FrameTarget> {
        let size = renderer.size().into();
        let target = match held.filter(|target| target.size() == size) {
            Some(target) => target,
            None => renderer.frame_target(size)?,
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        graphics::clear(&mut encoder, target.view());
        overlay(
            &self.device,
            renderer.queue(),
            &mut encoder,
            target.view(),
            size,
        );
        renderer.queue().submit([encoder.finish()]);
        Ok(target)
    }

    fn end_after(&mut self, time: xr::Time, result: Result<()>) -> Result<()> {
        if result.is_err() {
            let _ = self.stream.end(time, OPAQUE, &[]);
        }
        result
    }

    fn present_eyes(&mut self, frame: &Frame) -> Result<()> {
        let chains = self
            .eyes
            .each_ref()
            .map(|eye| eye.surface.chain.lock().unwrap());
        let views: [_; 2] = std::array::from_fn(|i| {
            xr::CompositionLayerProjectionView::new()
                .pose(frame.views[i].pose)
                .fov(frame.views[i].fov)
                .sub_image(
                    xr::SwapchainSubImage::new()
                        .swapchain(&chains[i])
                        .image_rect(self.eyes[i].surface.rect()),
                )
        });
        let projection = xr::CompositionLayerProjection::new()
            .space(&self.local)
            .views(&views);
        self.stream.end(frame.time, OPAQUE, &[&projection])?;
        Ok(())
    }

    fn present_menu(&mut self, frame: &Frame, placement: Placement) -> Result<()> {
        let surface = self.panel.as_ref().expect("painted menu panel");
        let chain = surface.chain.lock().unwrap();
        let quad = xr::CompositionLayerQuad::new()
            .space(&self.local)
            .eye_visibility(xr::EyeVisibility::BOTH)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&chain)
                    .image_rect(surface.rect()),
            )
            .pose(placement.pose())
            .size(placement.extent(surface.size));
        self.stream.end(frame.time, OPAQUE, &[&quad])?;
        Ok(())
    }
}

impl ActiveDisplayExtension for Vr {
    fn extension_id(&self) -> &'static str {
        OPENBF_VR_ID
    }

    fn begin_frame(&mut self, ctx: &FrameContext<'_>) -> Result<FrameDirectives> {
        let stereo = ctx.stereo_allowed && ctx.sim.is_some_and(live_view);
        self.begin_internal(ctx.sim, stereo, ctx.tracked_guns)?;
        let has_frame = self.has_frame();
        let menu_cursor = if has_frame && !stereo {
            self.menu_cursor(ctx.window_size)
        } else {
            None
        };
        Ok(FrameDirectives {
            has_frame,
            stereo_active: stereo,
            capture_flat: has_frame && !stereo,
            menu_cursor,
        })
    }

    fn has_frame(&self) -> bool {
        self.frame.is_some()
    }

    fn has_menu_pointer(&self) -> bool {
        self.focused && self.frame.as_ref().is_some_and(|frame| !frame.stereo)
    }

    fn menu_buttons(&self) -> MenuButtons {
        let Some(frame) = &self.frame else {
            return MenuButtons::default();
        };
        menu_buttons_for_frame(frame.stereo, frame.input.buttons, frame.input.pause)
    }

    fn recenter(&mut self) {
        self.rig.recenter();
        self.grenade.reset();
        self.saber = Default::default();
        self.menu_pose = None;
        self.menu_cursor = MenuCursor::default();
    }

    fn is_infantry(&self, sim: &SimView<'_>) -> bool {
        rig::infantry(sim).is_some()
    }

    fn transform_pad(&self, sim: &SimView<'_>, mut pad: PadState) -> PadState {
        if let Some(unit) = rig::infantry(sim) {
            let mut combat = TrackedCombat {
                motion_epoch: self.rig.motion_epoch,
                ..Default::default()
            };
            if let Some(frame) = self.frame.as_ref().filter(|frame| frame.stereo) {
                let world = self.rig.world(Some(sim), sim.camera, frame.head);
                let active = usize::from(unit.active_channel == 1);
                let foregrip = self.rig.foregrip(sim);
                let hands = world.weapon_hand_mounts(frame.input.hands, foregrip);
                let aimed = hands[1].filter(|_| frame.input.hands[1].aim.is_some());
                if sim.sim.weapon_classes[unit.weapon_class].saber.is_some() {
                    combat.hands = self.saber.hands;
                    combat.waypoints = self.saber.waypoints;
                }
                combat.grenade = self.grenade.release(sim, world);
                let saber = sim.sim.weapon_classes[unit.weapon_class].saber.is_some();
                let facing = rig::facing(
                    saber,
                    world.pose(frame.head),
                    aimed.map(|m| m.transform_vector3(Vec3::Z)),
                );
                pad = self.rig.pad(unit, frame.head, frame.input, facing, pad);
                if let Some(aimed) = aimed {
                    let free = if foregrip.is_some() {
                        world.hand_mounts(frame.input.hands)[1].unwrap_or(aimed)
                    } else {
                        aimed
                    };
                    pad.tracked_shots = std::array::from_fn(|channel| {
                        let mount = if channel == active { aimed } else { free };
                        let muzzle = frame.guns[channel].map_or(Vec3::ZERO, |gun| gun.muzzle);
                        Some(TrackedShot::new(
                            mount.transform_point3(rig::REAR_GRIP) - unit.soldier.pos,
                            mount.transform_point3(muzzle) - unit.soldier.pos,
                            mount.transform_vector3(Vec3::Z),
                        ))
                    });
                }
            } else {
                use core::sim::buttons;
                pad.buttons = (pad.buttons | buttons::FIRST_PERSON)
                    & !(buttons::FIRE | buttons::SECONDARY_FIRE);
            }
            pad.tracked_combat = Some(combat);
        } else if self.frame.as_ref().is_some_and(|frame| frame.stereo) && rig::piloted(sim) {
            pad.buttons |= self.rig.seat_buttons(sim);
        }
        pad
    }

    fn seat_sticks(&self, sim: &SimView<'_>, pad: PadState, look: f32) -> PadState {
        self.frame
            .as_ref()
            .filter(|frame| frame.stereo && self.focused && rig::piloted(sim))
            .map_or(pad, |frame| rig::seat_sticks(frame.input, pad, look))
    }

    fn apply_sprint_latch(
        &self,
        sim: &SimView<'_>,
        latch: &mut input::SprintLatch,
        pad: PadState,
    ) -> PadState {
        if let Some((frame, stick)) = self.frame.as_ref().and_then(|frame| {
            (frame.stereo
                && self.focused
                && pad.tracked_combat.is_some()
                && self.is_infantry(sim))
            .then_some((frame, frame.input.movement?))
        }) {
            self.rig.sprint_pad(sim, frame.head, stick, latch, pad)
        } else {
            sim.apply_sprint_latch(latch, pad)
        }
    }

    fn on_sim_ticks_consumed(&mut self, ticks: u32) {
        if ticks > 0 {
            self.rig.consumed();
            self.grenade.consumed();
            self.saber.consumed();
        }
    }

    fn on_sim_events(&self, sim: &SimView<'_>, rumble_strength: f32) {
        if !self.focused || !self.frame.as_ref().is_some_and(|frame| frame.stereo) {
            return;
        }
        let mut pulses = [None; 2];
        for event in &sim.sim.events {
            if let core::sim::Event::SaberHandContact {
                attacker,
                hand,
                blocked,
                ..
            } = *event
            {
                if attacker == sim.player {
                    let pulse = &mut pulses[usize::from(hand)];
                    *pulse = Some(pulse.unwrap_or(true) && blocked);
                }
            }
        }
        for (hand, blocked) in pulses.into_iter().enumerate() {
            if let Some(blocked) = blocked {
                self.actions
                    .saber_pulse(&self.session, hand, blocked, rumble_strength);
            }
        }
    }

    fn stereo_view(&self, sim: &SimView<'_>) -> Option<StereoView> {
        let frame = self.frame.as_ref().filter(|frame| frame.stereo)?;
        let base = sim.camera;
        let world = self.rig.world(Some(sim), base, frame.head);
        let eyes = frame.views.map(|eye| {
            let camera = world.camera(Pose::from_xr(eye.pose), base);
            (camera, eye_fov(eye.fov))
        });
        let (x, y) = eyes.iter().fold((0.0f32, 0.0f32), |(x, y), (_, fov)| {
            (
                x.max(fov.right.abs()).max(fov.left.abs()),
                y.max(fov.up.abs()).max(fov.down.abs()),
            )
        });
        let mut camera = world.camera(frame.head, base);
        camera.fov_y = 2.0 * y.atan();
        let hand_mounts = if self.is_infantry(sim) {
            world.weapon_hand_mounts(frame.input.hands, self.rig.foregrip(sim))
        } else {
            [None; 2]
        };
        let grenade = self
            .grenade
            .held_class(sim)
            .zip(hand_mounts[0])
            .map(|(class, mut mount)| {
                mount.w_axis = mount.transform_point3(rig::REAR_GRIP).extend(1.0);
                (class, mount)
            });
        let listener = ListenerPose {
            pos: camera.position,
            yaw: camera.yaw,
        };
        Some(StereoView {
            hand_mounts,
            grenade,
            camera,
            eyes,
            aspect: x / y,
            listener,
        })
    }

    fn draw_stereo_eyes(
        &self,
        renderer: &mut Renderer,
        view: &StereoView,
        draws: &[DrawCmd],
        skinned: &[SkinnedDrawCmd],
        clear: [f64; 4],
    ) -> Result<()> {
        let mut frame = renderer.begin_frame();
        for (eye, (camera, fov)) in self.eyes.iter().zip(view.eyes) {
            frame.render_to(
                &eye.world,
                &camera,
                ViewProjection::Asymmetric(fov),
                draws,
                skinned,
                clear,
                None,
            )?;
        }
        frame.finish();
        Ok(())
    }

    fn set_reticle(
        &mut self,
        renderer: &Renderer,
        view: &StereoView,
        point: Vec3,
        paint: OverlayFn<'_>,
    ) -> Result<()> {
        let reticle = self
            .frame
            .as_ref()
            .and_then(|frame| frame.visor)
            .and_then(|visor| {
                WorldReticle::facing(point, &view.camera, visor.radians_per_height())
            });
        let Some(reticle) = reticle else {
            return Ok(());
        };
        let held = self.reticle_frame.take();
        self.reticle_frame = Some(self.paint_frame(renderer, held, paint)?);
        if let Some(frame) = self.frame.as_mut() {
            frame.reticle = Some((reticle, view.eyes));
        }
        Ok(())
    }

    fn finish_stereo(
        &mut self,
        renderer: &mut Renderer,
        overlay: Option<OverlayFn<'_>>,
    ) -> Result<()> {
        let Some(frame) = self.frame.take() else {
            return Ok(());
        };
        self.diag.stereo += 1;
        let result = self.finish_eyes(renderer, &frame, overlay);
        self.end_after(frame.time, result)?;
        self.present_eyes(&frame)?;
        renderer.mirror_frame(&self.eyes[0].world, None)?;
        Ok(())
    }

    fn finish_flat(&mut self, renderer: &Renderer) -> Result<()> {
        let Some(frame) = self.frame.take() else {
            return Ok(());
        };
        let Some(source) = renderer.flat_frame() else {
            self.diag.empty("no desktop frame captured");
            self.stream.end(frame.time, OPAQUE, &[])?;
            return Ok(());
        };
        self.diag.menu += 1;
        let placement = self
            .menu_pose
            .map_or_else(|| Placement::menu(frame.head), |(placement, _)| placement);
        let result = self
            .panel_for(source.size())
            .and_then(|()| self.blit_surface(renderer, self.panel.as_ref().unwrap(), source));
        self.end_after(frame.time, result)?;
        self.present_menu(&frame, placement)
    }

    fn shutdown(&mut self) {
        if let Some(frame) = self.frame.take() {
            let _ = self.stream.end(frame.time, OPAQUE, &[]);
        }
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        if self.running {
            let _ = self.session.end();
            self.running = false;
        }
    }
}

fn eye_fov(fov: xr::Fovf) -> ViewFov {
    ViewFov {
        left: fov.angle_left.tan(),
        right: fov.angle_right.tan(),
        down: fov.angle_down.tan(),
        up: fov.angle_up.tan(),
    }
}

fn menu_buttons_for_frame(stereo: bool, buttons: u32, pause: bool) -> MenuButtons {
    let back_east = !stereo && buttons & core::sim::buttons::SWITCH_WEAPON != 0;
    // Allow A / Jump to dismiss in-world popups (e.g. campaign objectives)
    // while staying in stereo view; Reload remains non-stereo-only.
    let confirm = if stereo {
        buttons & core::sim::buttons::JUMP != 0
    } else {
        buttons & (core::sim::buttons::JUMP | core::sim::buttons::RELOAD) != 0
    };
    MenuButtons {
        pause,
        back: back_east,
        east: back_east,
        confirm,
    }
}

impl Drop for Vr {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::menu_buttons_for_frame;
    use core::sim::buttons;

    #[test]
    fn jump_confirms_popups_in_stereo_without_enabling_reload_or_back() {
        let stereo_jump = menu_buttons_for_frame(true, buttons::JUMP, false);
        assert!(stereo_jump.confirm);
        assert!(!stereo_jump.back);
        assert!(!stereo_jump.east);

        let stereo_other =
            menu_buttons_for_frame(true, buttons::RELOAD | buttons::SWITCH_WEAPON, false);
        assert!(!stereo_other.confirm);
        assert!(!stereo_other.back);
        assert!(!stereo_other.east);

        let flat_reload = menu_buttons_for_frame(false, buttons::RELOAD | buttons::SWITCH_WEAPON, true);
        assert!(flat_reload.confirm);
        assert!(flat_reload.back);
        assert!(flat_reload.east);
        assert!(flat_reload.pause);
    }
}
