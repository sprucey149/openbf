//! P2 map viewer: baked Scene -> render resources + draw list (the fly-cam is
//! the binary's `App::drive_fly_camera`, `main/windowed.rs`).
//!
//! Transform convention (verified empirically on TAT1, see RE_LOG §Instance
//! transforms): XFRM's 3x3 rows are the object's basis vectors expressed in
//! world space (row-vector convention from the munge toolchain), world space
//! is Y-up. glam uses column vectors, so the stored rows become the columns
//! of the model matrix. Proven by the Tusken-camp barrier arc: barrier long
//! axes are tangent to the fence chain only under this convention (7/7
//! tangency 0.94-1.00 vs 0.21-0.97 transposed).
//!
//! Split by queue SPL1 (2026-09-23, a pure move) into four parts, each
//! re-exported here so every `crate::viewer::X` path is unchanged: `look` (a
//! map's look as render state), `scene` (the scene upload and its draw list,
//! sky included), `upload` (the GPU upload primitives) and `camera` (poses,
//! the instance transform and the auto-framing). `foliage` stands the terrain
//! foliage's model instances on the ground (queue FOLG); `grass` builds the
//! grass patches' templates and places the patches (queue FOLGG); `leaf`
//! builds and places the leaf patches (card LEAF); `vehicles` draws the
//! battle's vehicles at their sim transforms (card VDRAW), a skinned walker
//! through `walker_pose` (card WSKN).

mod camera;
pub mod droidekas;
mod dust;
pub mod foliage;
pub mod grass;
pub mod leaf;
mod look;
mod pickups;
mod scene;
pub mod stations;
pub mod turret_fall;
pub mod turret_pose;
mod upload;
mod vehicles;
mod walker_aim;
pub mod walker_pose;

pub(crate) use camera::robust_bounds;
pub use camera::{
    camera_from, instance_matrix, look_at, orbit_eye, overview_bounds, overview_camera, Overview,
    GOLDEN_CAM,
};
pub use droidekas::DroidekaVisuals;
pub use look::{
    apply_lighting, clear_color, far_cull, far_cull_enabled, far_cull_from, far_fade_from,
    lowres_enabled, lowres_range, render_lighting_from, water_look, FarCull, WaterLook, CLEAR,
    FOG_DENSITY_SCALE, MAP_CLEAR,
};
pub use scene::{
    install_spec_table, route_untextured, sky_wrap_v, upload_scene, DoorDraw, SceneGpu, Untextured,
};
pub use upload::{
    upload_billboard_quad, upload_cpu_texture, upload_cpu_texture_scrolling, upload_fx_sprite,
    upload_skinned, upload_submeshes, upload_texture_file, upload_texture_file_scrolling,
    upload_unit_cube, SkinnedPart, SubmeshPart, DETAIL_UV_SCALE,
};
pub use vehicles::{
    find_model, find_texture, flyer_rigs, hover_rigs, model_tops, walker_rigs, ModelSource,
    TexSource, VehicleSides, VehicleVisuals,
};
