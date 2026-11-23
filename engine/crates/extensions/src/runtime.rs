//! Engine extension runtime traits and shared frame/input types.
//!
//! Enabled via `extensions = { features = ["runtime"] }` so `game` and bundled
//! engine extensions (`openbf_vr`) share clean interfaces without circular
//! crate dependencies.

use crate::ExtensionManifest;
use anyhow::Result;
use core::sim::{PadState, Sim};
use core::soldier::Stance;
use core::unit::UnitHandle;
use glam::{Mat4, Vec3};
use render::{Camera, DrawCmd, OverlayFn, Renderer, SkinnedDrawCmd, ViewFov};
use std::sync::Arc;
use winit::window::Window;

/// Weapon geometry measurements extracted from a baked soldier/weapon asset
/// for spatial weapon handling (muzzle point + optional two-handed foregrip).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackedGun {
    pub class: usize,
    pub slot: u8,
    pub muzzle: Vec3,
    pub foregrip: Option<Vec3>,
}

/// 3D audio listener pose supplied by an active spatial/VR view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListenerPose {
    pub pos: Vec3,
    pub yaw: f32,
}

/// Virtual cursor sample mapped from a spatial/motion pointer onto a flat UI surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MenuCursorFrame {
    pub position: Option<[f32; 2]>,
    pub down: bool,
    pub click: bool,
    pub wheel: f32,
}

/// Menu navigation edge flags emitted by an active extension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuButtons {
    pub confirm: bool,
    pub back: bool,
    pub pause: bool,
    pub east: bool,
}

/// Optional world-collision query provided by the game simulation host so
/// spatial extensions can clip tracked eye positions against map geometry.
pub trait EyeClipQuery {
    fn clip_tracked_eye(&self, anchor: Vec3, tracked: Vec3) -> Vec3;
}

/// Read-only simulation snapshot passed to active extensions during a frame.
pub struct SimView<'a> {
    pub sim: &'a Sim,
    pub player: UnitHandle,
    pub local_slot: u8,
    pub camera: Camera,
    pub spectating: bool,
    pub eye_clip: Option<&'a dyn EyeClipQuery>,
}

impl<'a> SimView<'a> {
    pub fn clip_tracked_eye(&self, anchor: Vec3, tracked: Vec3) -> Vec3 {
        self.eye_clip
            .map_or(tracked, |clip| clip.clip_tracked_eye(anchor, tracked))
    }

    pub fn player_on_foot(&self) -> bool {
        self.sim
            .unit(self.player)
            .is_some_and(|u| u.mount.is_none())
    }

    pub fn apply_sprint_latch(&self, latch: &mut input::SprintLatch, pad: PadState) -> PadState {
        self.apply_sprint_latch_with_buffer(latch, pad, false)
    }

    pub fn apply_sprint_latch_with_buffer(
        &self,
        latch: &mut input::SprintLatch,
        pad: PadState,
        buffer_start: bool,
    ) -> PadState {
        let Some(unit) = self.sim.unit(self.player) else {
            return pad;
        };
        let weapon = unit.weapon_class;
        latch.apply_with_start_buffer(
            pad,
            self.player_on_foot(),
            unit.soldier.stance == Stance::Sprint,
            self.sim.weapon_classes[weapon].saber.is_none(),
            buffer_start,
        )
    }
}

/// Per-frame context passed to [`ActiveDisplayExtension::begin_frame`].
pub struct FrameContext<'a> {
    pub sim: Option<&'a SimView<'a>>,
    pub stereo_allowed: bool,
    pub tracked_guns: [Option<TrackedGun>; 2],
    pub window_size: [u32; 2],
}

/// Per-frame presentation directives returned by [`ActiveDisplayExtension::begin_frame`].
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameDirectives {
    pub has_frame: bool,
    pub stereo_active: bool,
    pub capture_flat: bool,
    pub menu_cursor: Option<MenuCursorFrame>,
}

/// Per-frame stereo camera, hand/weapon mount transforms, and audio listener pose.
#[derive(Clone, Copy, Debug)]
pub struct StereoView {
    pub hand_mounts: [Option<Mat4>; 2],
    pub grenade: Option<(usize, Mat4)>,
    pub camera: Camera,
    pub eyes: [(Camera, ViewFov); 2],
    pub aspect: f32,
    pub listener: ListenerPose,
}

/// Top-level trait implemented by a bundled engine extension.
pub trait EngineExtension: Send + Sync {
    fn manifest(&self) -> ExtensionManifest;

    /// Early initialization hook executed before window/GPU device creation
    /// for extensions declaring [`crate::ExclusiveCapability::DisplayBackend`].
    fn early_init_display(&self) -> Result<Box<dyn EarlyDisplayContext>>;
}

/// Early-initialized display backend waiting for the desktop window handle to
/// create its compatible graphics device and [`Renderer`].
pub trait EarlyDisplayContext {
    fn extension_id(&self) -> &'static str;

    fn open_renderer(
        self: Box<Self>,
        window: Arc<Window>,
    ) -> Result<(Renderer, Box<dyn ActiveDisplayExtension>)>;
}

/// Live display/spatial-input extension bound to the active [`Renderer`].
pub trait ActiveDisplayExtension {
    fn extension_id(&self) -> &'static str;

    fn begin_frame(&mut self, ctx: &FrameContext<'_>) -> Result<FrameDirectives>;

    fn has_frame(&self) -> bool;

    fn has_menu_pointer(&self) -> bool;

    fn menu_buttons(&self) -> MenuButtons;

    fn recenter(&mut self);

    fn is_infantry(&self, sim: &SimView<'_>) -> bool;

    fn transform_pad(&self, sim: &SimView<'_>, pad: PadState) -> PadState;

    fn apply_sprint_latch(
        &self,
        sim: &SimView<'_>,
        latch: &mut input::SprintLatch,
        pad: PadState,
    ) -> PadState;

    fn on_sim_ticks_consumed(&mut self, ticks: u32);

    fn on_sim_events(&self, sim: &SimView<'_>, rumble_strength: f32);

    fn stereo_view(&self, sim: &SimView<'_>) -> Option<StereoView>;

    fn draw_stereo_eyes(
        &self,
        renderer: &mut Renderer,
        view: &StereoView,
        draws: &[DrawCmd],
        skinned: &[SkinnedDrawCmd],
        clear: [f64; 4],
    ) -> Result<()>;

    fn finish_stereo(
        &mut self,
        renderer: &mut Renderer,
        overlay: Option<OverlayFn<'_>>,
    ) -> Result<()>;

    fn finish_flat(&mut self, renderer: &Renderer) -> Result<()>;

    fn shutdown(&mut self);
}
