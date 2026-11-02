//! The map's sky shells: [`SkyShell`] (a `DomeModel` or `SkyObject` redrawn
//! around the eye every frame), the GS state its own material asks for
//! ([`SkyMode`]) and where it sits relative to the eye ([`Placement`]), with
//! `Renderer::set_sky` and the per-frame model matrix (`sky_model`). The sky
//! PASS is drawn by `render_frame` (`frame.rs`); the crate header's sky
//! paragraph says what is not built. The placement and material-rule tests
//! live here.

use crate::{MeshHandle, Renderer, TextureHandle};
use glam::Mat4;

mod flight;
pub use flight::Flight as SkyFlight;

// Only for `SkyShell`'s docs, which link `Lighting`.
#[cfg(doc)]
use crate::Lighting;

/// A sky-dome shell, drawn before everything else. The renderer owns these
/// (like [`Lighting`], they are per-map state) because their model matrix
/// follows the camera and so has to be rebuilt every frame.
///
/// Each map's `sky_ DomeInfo` names one or more `DomeModel` shells, already
/// in that map's baked models: a dome plus, on some maps, parallax shells of
/// distant rocks or cloud. See `bake::lighting` for the reduction.
///
/// **No shell is ever fogged**, and there is no per-shell fog flag. Dome
/// draws use 0x20031, excluded by both fog gates (`analysis/re-fog.md` section 2.4).
/// `SkyObject` draws instead use `flags | 0x20001` with the composite/fog
/// switch cleared (BF2 0x3bf060); their material lighting and depth remain
/// enabled (`analysis/re-bf2-geonosis-atmosphere.md`).
///
/// **Removed deviation (session 5, deleted 2026-09-03):** the port used to
/// route a PARALLAX shell — a `DomeModel` with an authored `Offset` or
/// `MovementScale` — through a fogged sky shader, on the argument that such a
/// shell is authored LANDSCAPE and that leaving it sharp behind fogged
/// terrain left a seam at the horizon (the seam on Kamino). The ELF says
/// otherwise: the PS2 draws that shell sharp and the seam is what the disc
/// shows. The classifier (`SkyShell::is_parallax`) and the `fs_sky_fogged`
/// entry point are both gone.
#[derive(Debug, Clone, Copy)]
pub struct SkyShell {
    pub mesh: MeshHandle,
    pub texture: Option<TextureHandle>,
    /// Flat tint over the texture: white, except the dome shell the map's
    /// Lightning effect darkens (`game::viewer::scene::sky_shell_tint`) and a
    /// mirrored shell's `ReflectionColor`.
    pub color: [f32; 4],
    /// Uniform scale. The shipped dome meshes are ~7 units across; the scale
    /// is what puts them at map distance, and it only matters relative to
    /// `offset` (a shell centred on the eye looks the same at any scale).
    pub scale: f32,
    /// Where the shell sits relative to the eye — see [`Placement`]. A
    /// `DomeModel` is a [`Placement::Dome`] (the ELF's Y lerp); a `SkyObject`
    /// is a [`Placement::Object`] (the port's own authored offset).
    pub placement: Placement,
    /// Rotation about Y, radians. Zero for every `DomeModel`; a `SkyObject`
    /// uses it to face along its bearing.
    pub yaw: f32,
    /// The GS state this shell's own MATERIAL asks for — see [`SkyMode`].
    pub mode: SkyMode,
    /// Whether this shell is lit by the map's sun and ambient light.
    /// False for dome shells and mirrors. Sky objects follow each segment's
    /// material bit 0x01 (`analysis/re-bf2-geonosis-atmosphere.md`).
    pub lit: bool,
    pub emission: Option<crate::LightPath>,
    pub depth: bool,
}

/// How a sky shell's segment is composited — the ELF's per-MATERIAL GS state,
/// not a property of the sky pass.
///
/// `analysis/re-sky-dome-placement.md` §5 [V] (`FUN_0058ed70` 723411–723480),
/// **LEAD CORRECTION 2, session 12**: the sky tick always calls that routine
/// with the sky draw flag 0x20 set (0x20031), and under that flag a material
/// word carrying EITHER 0x04 or 0x02 takes the TRANSLUCENT path — GS ALPHA
/// 0x44 (`Cs·As + Cd·(1−As)`) with TEST 0x51001 and no Z write, and no AREF
/// test at all. The alpha-TEST word (TEST 0x5000b at the global AREF
/// `DAT_0060c64c` = 0x60) is reached only on the NON-sky path. So under the
/// sky flag every alpha-carrying material composites, and only the plain
/// words paint. On the discs that is `mus1_sky_volcano` (0x00010003) and
/// `myg1_sky_mountains_01` (0x00000003), both BF2 — BF1 ships no 0x02 sky
/// shell — while `mus1_sky_dome`'s 0x00010001 / 0x00010010 stay OPAQUE.
///
/// (This replaces the port's 2026-09-03 reading, which alpha-TESTED bit 0x02
/// on the sky at the shader's 0.5. There is no sky MASK mode any more, and no
/// sky pipeline selects one.)
///
/// Two bits of the ELF's rule are NOT built here because no sky shell needs
/// them yet and neither has a port-side meaning: `f & 0x40` (additive, GS
/// ALPHA 0x48) and `f & 8` (ALPHA 0x64 with FIX = alpha/2). Both fall through
/// to [`SkyMode::Opaque`]. The colour-alpha clause (`c.a != 0xff` also makes a
/// draw translucent) is not built either — every shell's instance colour is
/// the map's `DomeInfo.Ambient` at alpha 1.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkyMode {
    /// Neither bit: the shell PAINTS. Its texture's alpha is not a mask and
    /// not a weight, so the frame takes its RGB everywhere the mesh covers.
    Opaque,
    /// Bit 0x04 OR bit 0x02: standard `over` composite.
    Blend,
}

impl SkyMode {
    /// Bit 0x04 or bit 0x02 blends, else opaque — RE §5 [V] as corrected in
    /// session 12 (see [`SkyMode`]). The argument is the segment's RAW MTRL
    /// word, which the bake writes on the GLB material as `extras.flags`
    /// (`bake::glb`); the same two bits are
    /// `bake::detail::{TRANSPARENT, CUTOUT}`.
    pub fn from_flags(flags: u32) -> SkyMode {
        if flags & 0x06 != 0 {
            SkyMode::Blend
        } else {
            SkyMode::Opaque
        }
    }

    /// Index into the renderer's sky pipeline table.
    pub(super) fn index(self) -> usize {
        match self {
            SkyMode::Opaque => 0,
            SkyMode::Blend => 1,
        }
    }
}

/// How a sky shell is placed relative to the eye each frame.
///
/// The two cases are genuinely different rules, not two settings of one rule,
/// so they are spelled out rather than distinguished by testing
/// `movement_scale == 1.0`.
///
/// NOT BUILT (`analysis/re-sky-dome-placement.md` §8.2): `RotationSpeed`
/// (the yaw the ELF spins a shell by — the bake does not parse the key).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    Flight(SkyFlight),
    SpinningDome {
        offset_y: f32,
        movement_scale: f32,
        speed: f32,
        axis: glam::Vec3,
    },
    /// A `DomeModel` shell, placed the way both ELFs place it
    /// (`analysis/re-sky-dome-placement.md` §3 BF2 `FUN_003cb918` 425830–425837,
    /// §4 BF1 `FUN_002f2308` 280211–280221, both [V]): X and Z are the
    /// camera's EXACTLY — there is no `eye · MovementScale` horizontal lag —
    /// and Y is the lerp
    ///
    /// ```text
    /// y = cam.y + ((0 + Offset) − cam.y) · (1 − MovementScale) · S
    /// ```
    ///
    /// with the disc's camera scale `S` = 1.0 (§3 [D]). So the authored
    /// `Offset` reaches the shell ONLY through `(1 − MovementScale)`:
    /// MovementScale 1.0 pins the shell to the eye whatever the Offset says,
    /// and MovementScale 0 pins it at world height `Offset`.
    ///
    /// The port draws the mesh at `k` times its authored size instead of the
    /// disc's 1.0 (`SKY_SHELL_RADIUS`), so the lift is scaled by `k` too — a
    /// uniform scale about the eye is angle-invariant, which reproduces the
    /// ELF's elevation angles exactly (§8.1).
    Dome {
        /// The shell's authored `DomeModel.Offset`, world metres.
        offset_y: f32,
        /// The shell's authored `DomeModel.MovementScale` (default 1.0, §0 [V]).
        movement_scale: f32,
    },
    /// A `SkyObject` flight — a distant capital ship. The offset is the
    /// PORT'S OWN (the discs author a count, an altitude band and a distance
    /// but no azimuth; see `game::viewer::scene::upload_sky_objects`), so it is held
    /// at a fixed vector from the eye: camera-locked in XZ at its authored
    /// distance and bearing. PROVISIONAL, unchanged by this module's rule.
    Object {
        offset: glam::Vec3,
    },
    /// The mirrored copy of a `Reflect` dome shell, drawn just before the
    /// shell (BF1 `FUN_002f2308` 280190–280255, `analysis/re-sky-reflect.md`
    /// §2 [V][O]): the shell's own [`Placement::Dome`] position lifted by
    /// [`MIRROR_LIFT`], with Y flipped and squashed by [`mirror_squash`].
    /// Like the dome's Offset lift, the 0.1 m is scaled by `k` so the PS2's
    /// angles survive the port's uniform scale (§5). BF1 KAS1 / KAS2 only.
    Mirror {
        /// The shell's authored `DomeModel.Offset`, world metres.
        offset_y: f32,
        /// The shell's authored `DomeModel.MovementScale`.
        movement_scale: f32,
    },
}

/// How far a mirrored shell sits above its shell, in the disc's shell units
/// (0x3dcccccd, 280218; `analysis/re-sky-reflect.md` §2 [V]).
pub const MIRROR_LIFT: f32 = 0.1;

/// The mirrored shell's Y squash at renderer time `t` seconds:
/// `0.85 + 0.006·sin φ` with φ advancing 2.0/30 per 30 Hz tick, i.e.
/// 2.0 rad/s (`DAT_00518b20`/`DAT_00518b24`, 280194–280212; measured
/// 1.994 rad/s on a running PS2, `analysis/re-sky-reflect.md` §2). The PS2's
/// φ starts at 0, so a headless frame (t = 0) is squashed by exactly 0.85.
/// [`crate::TIME_WRAP`] restarts φ, a jump of under 0.012 in the squash
/// once every 2000 s.
pub fn mirror_squash(t: f32) -> f32 {
    0.85 + 0.006 * (2.0 * t).sin()
}

impl Placement {
    /// The shell's world translation this frame, for an eye at `eye` and a
    /// uniform mesh scale of `k`.
    pub fn translation(&self, eye: glam::Vec3, k: f32) -> glam::Vec3 {
        match *self {
            Placement::Flight(flight) => flight.model().w_axis.truncate(),
            Placement::Dome {
                offset_y,
                movement_scale,
            }
            | Placement::SpinningDome {
                offset_y,
                movement_scale,
                ..
            } => eye + glam::Vec3::Y * ((offset_y - eye.y) * (1.0 - movement_scale) * k),
            Placement::Object { offset } => eye + offset,
            Placement::Mirror {
                offset_y,
                movement_scale,
            } => {
                eye + glam::Vec3::Y
                    * (((offset_y - eye.y) * (1.0 - movement_scale) + MIRROR_LIFT) * k)
            }
        }
    }
}

impl Renderer {
    pub fn set_sky_seed(&mut self, seed: u32) {
        self.sky_seed = seed;
    }

    pub fn advance_sky(&mut self) {
        let time = self.atmosphere_time();
        advance_flights(&mut self.sky, time, &mut self.sky_seed);
    }

    pub(super) fn sky_color(shell: &SkyShell) -> [f32; 4] {
        let mut color = shell.color;
        if let Placement::Flight(flight) = shell.placement {
            color[3] *= flight.opacity();
        }
        color
    }

    pub(super) fn sky_mode(shell: &SkyShell) -> SkyMode {
        if Self::sky_color(shell)[3] < 1.0 {
            SkyMode::Blend
        } else {
            shell.mode
        }
    }
    /// Replace the map's sky shells. Domes paint in authored order;
    /// object hulls use their material's depth and blend state.
    pub fn set_sky(&mut self, shells: Vec<SkyShell>) {
        self.sky_seed = shells
            .iter()
            .find_map(|s| match s.placement {
                Placement::Flight(f) => Some(f.id()),
                _ => None,
            })
            .unwrap_or(0);
        self.sky = shells;
    }

    /// Read the map's sky shells.
    pub fn sky(&self) -> &[SkyShell] {
        &self.sky
    }

    /// Access the map's sky shells mutably.
    pub fn sky_mut(&mut self) -> &mut [SkyShell] {
        &mut self.sky
    }

    /// Where a shell sits this frame: `atmo_time` controls dome spin and
    /// freezes when paused; `water_time` controls mirror squash and continues.
    /// See [`Placement`] for the rule and its citation. A mirrored shell's Y
    /// scale is negative; the sky pipelines cull nothing, so its flipped
    /// winding still draws.
    pub(super) fn sky_model(
        shell: &SkyShell,
        eye: glam::Vec3,
        atmo_time: f64,
        water_time: f64,
    ) -> Mat4 {
        if let Placement::Flight(flight) = shell.placement {
            return flight.model() * Mat4::from_scale(glam::Vec3::splat(shell.scale));
        }
        let k = shell.scale;
        let scale = match shell.placement {
            Placement::Mirror { .. } => {
                glam::Vec3::new(k, -mirror_squash((water_time % 2000.0) as f32) * k, k)
            }
            _ => glam::Vec3::splat(k),
        };
        let rotation = match shell.placement {
            Placement::SpinningDome { speed, axis, .. } if axis.length_squared() > 0.0 => {
                Mat4::from_axis_angle(
                    axis.normalize(),
                    (f64::from(speed) * atmo_time * std::f64::consts::TAU)
                        .rem_euclid(std::f64::consts::TAU) as f32,
                )
            }
            _ => Mat4::from_rotation_y(shell.yaw),
        };
        Mat4::from_translation(shell.placement.translation(eye, k))
            * rotation
            * Mat4::from_scale(scale)
    }
}

fn advance_flights(shells: &mut [SkyShell], time: f64, seed: &mut u32) {
    let mut updated = std::collections::HashMap::new();
    for shell in shells {
        if let Placement::Flight(flight) = &mut shell.placement {
            *flight = *updated.entry(flight.id()).or_insert_with(|| {
                flight.update_shared(time, seed);
                *flight
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_spawn_stream_advances_once_per_ship_independent_of_mesh_parts() {
        let shell = |id| SkyShell {
            mesh: MeshHandle(0),
            texture: None,
            color: [1.0; 4],
            scale: 1.0,
            placement: Placement::Flight(SkyFlight::new(
                [200.0, 300.0],
                1500.0,
                [[0.0; 2], [0.0; 2], [20.0, 50.0]],
                glam::Vec3::ZERO,
                2.0,
                1000.0,
                id,
            )),
            yaw: 0.0,
            mode: SkyMode::Opaque,
            lit: true,
            depth: true,
            emission: None,
        };
        let mut single = [shell(3), shell(4)];
        let mut segmented = [shell(3), shell(3), shell(3), shell(4), shell(4)];
        let (mut a, mut b) = (3u32, 3u32);
        for time in [0.0, 0.5, 0.5, 20.0, 120.0, 124.0, 128.0] {
            advance_flights(&mut single, time, &mut a);
            advance_flights(&mut segmented, time, &mut b);
            assert_eq!(a, b);
            for index in [0, 1, 2] {
                assert_eq!(single[0].placement, segmented[index].placement);
            }
            for index in [3, 4] {
                assert_eq!(single[1].placement, segmented[index].placement);
            }
        }
        assert_ne!(single[0].placement, single[1].placement);
    }

    /// The ELF's dome rule as arithmetic (`analysis/re-sky-dome-placement.md`
    /// §3 [V]): X/Z are the eye's exactly and Y is the lerp
    /// `cam.y + (Offset − cam.y)·(1 − MovementScale)`, times the port's
    /// uniform mesh scale `k`.
    #[test]
    fn a_dome_shell_takes_offset_only_through_one_minus_movement_scale() {
        // MUS1's volcano at the lead's ground eye: Offset 100, MS 0.995,
        // k = 1000 / 19.9. The disc's own lift is (100 − 68)·0.005 = 0.16 m;
        // scaled by k it is 8.04 m, which is the SAME ANGLE.
        let k = 1000.0 / 19.9;
        let eye = glam::Vec3::new(-45.6, 68.0, 90.1);
        let p = Placement::Dome {
            offset_y: 100.0,
            movement_scale: 0.995,
        };
        let t = p.translation(eye, k);
        assert_eq!(t.x, eye.x, "a dome shell has no horizontal lag");
        assert_eq!(t.z, eye.z, "a dome shell has no horizontal lag");
        assert!(
            (t.y - eye.y - 8.04).abs() < 0.01,
            "lift {} m, expected 8.04",
            t.y - eye.y
        );

        // MovementScale 1.0 pins the shell to the eye whatever the Offset
        // says — the port used to add the full Offset here.
        for offset_y in [0.0, 100.0, -40.0, 190.0] {
            let p = Placement::Dome {
                offset_y,
                movement_scale: 1.0,
            };
            assert_eq!(
                p.translation(eye, k),
                eye,
                "Offset {offset_y} moved a MS=1 shell"
            );
        }
    }

    /// BF1's mirror matrix (`analysis/re-sky-reflect.md` §2 [V]): the
    /// shell's own lerped position + 0.1 m, then Y × −(0.85 + 0.006 sin φ)
    /// with φ = 2.0 rad/s — both lifts scaled by the port's `k`.
    #[test]
    fn a_mirrored_shell_flips_y_about_its_lifted_position() {
        let k = 10.0;
        let eye = glam::Vec3::new(3.0, -26.5, 7.0);
        let (offset_y, movement_scale) = (-40.0, 0.995); // KAS1's shells
        let shell = |placement| SkyShell {
            mesh: MeshHandle(0),
            texture: None,
            color: [1.0; 4],
            scale: k,
            placement,
            yaw: 0.0,
            mode: SkyMode::Opaque,
            lit: false,
            depth: false,
            emission: None,
        };
        let dome = shell(Placement::Dome {
            offset_y,
            movement_scale,
        });
        let mirror = shell(Placement::Mirror {
            offset_y,
            movement_scale,
        });
        let lift = mirror.placement.translation(eye, k) - dome.placement.translation(eye, k);
        assert!(
            (lift - glam::Vec3::Y * MIRROR_LIFT * k).length() < 1e-4,
            "{lift}"
        );

        // At t = 0 (every headless golden) the squash is exactly 0.85.
        assert_eq!(mirror_squash(0.0), 0.85);
        let m = Renderer::sky_model(&mirror, eye, 0.0, 0.0);
        let d = Renderer::sky_model(&dome, eye, 0.0, 0.0);
        let p = glam::Vec3::new(1.0, 2.0, -3.0);
        let (pm, pd) = (m.transform_point3(p), d.transform_point3(p));
        assert!((pm.x - pd.x).abs() < 1e-4 && (pm.z - pd.z).abs() < 1e-4);
        let base = mirror.placement.translation(eye, k).y;
        assert!((pm.y - (base - 0.85 * k * 2.0)).abs() < 1e-3, "{pm} {base}");

        // The wobble peaks at 0.856 a quarter period in (φ = π/2).
        let q = std::f32::consts::FRAC_PI_4;
        assert!((mirror_squash(q) - 0.856).abs() < 1e-6);
    }

    /// The ELF's material rule UNDER THE SKY FLAG (RE §5 [V], LEAD
    /// CORRECTION 2, session 12): 0x04 or 0x02 composites, everything else
    /// paints. There is no alpha-test state on the sky path.
    #[test]
    fn sky_mode_follows_the_material_word() {
        // mus1_sky_dome, both segments (RE §0/§5, the lead's correction).
        assert_eq!(SkyMode::from_flags(0x0001_0001), SkyMode::Opaque);
        assert_eq!(SkyMode::from_flags(0x0001_0010), SkyMode::Opaque);
        // The two 0x02 sky shells the discs ship — `mus1_sky_volcano`
        // (0x00010003) and BF2 MYG1's `myg1_sky_mountains_01` (0x00000003).
        // The sky flag 0x20 sends both down the TRANSLUCENT path.
        assert_eq!(SkyMode::from_flags(0x0001_0003), SkyMode::Blend);
        assert_eq!(SkyMode::from_flags(0x0000_0003), SkyMode::Blend);
        // 0x04 blends, with or without 0x02.
        assert_eq!(SkyMode::from_flags(0x0001_0004), SkyMode::Blend);
        assert_eq!(SkyMode::from_flags(0x0001_0007), SkyMode::Blend);
        // A GLB with no `extras.flags` reads 0 — OPAQUE, and the same two
        // slots the pipeline table is indexed by.
        assert_eq!(SkyMode::from_flags(0), SkyMode::Opaque);
        // Neither bit: the other alpha bits the port does not model
        // (0x40 additive, 0x08 ALPHA 0x64) still paint.
        assert_eq!(SkyMode::from_flags(0x0000_0048), SkyMode::Opaque);
        assert_eq!([SkyMode::Opaque.index(), SkyMode::Blend.index()], [0, 1]);
    }

    /// A `SkyObject` is placed by the port's own rule and this change does
    /// not touch it.
    #[test]
    fn a_sky_object_sits_at_its_authored_offset_from_the_eye() {
        let eye = glam::Vec3::new(10.0, 20.0, -30.0);
        let offset = glam::Vec3::new(2500.0, 420.0, 0.0);
        let p = Placement::Object { offset };
        // Independent of the dome scale, which is 1.0 for a sky object.
        assert_eq!(p.translation(eye, 1.0), eye + offset);
        assert_eq!(p.translation(eye, 50.25), eye + offset);
    }

    /// The seam, as arithmetic. `mus1_sky_volcano`'s upper band is a ring of
    /// radius 19.04 at mesh y −1.78 whose texture is opaque lava at v = 1.0
    /// (RE §7), so that ring IS a line across the sky at whatever elevation
    /// the placement rule puts it. The ELF's rule puts it BELOW the horizon
    /// (−4.9° at MUS1's walkable y ≈ 68); the port's old rule — the full
    /// 100 m Offset over a 1000 m shell — put it at +0.63°, in open sky.
    #[test]
    fn the_mus1_volcano_band_edge_lands_below_the_horizon() {
        let (ring_y, ring_r) = (-1.78f32, 19.04f32);
        let k = 1000.0 / 19.9;
        let eye = glam::Vec3::new(0.0, 68.0, 0.0);
        let shell = Placement::Dome {
            offset_y: 100.0,
            movement_scale: 0.995,
        };
        let lift = shell.translation(eye, k).y - eye.y;
        let deg = (ring_y * k + lift).atan2(ring_r * k).to_degrees();
        assert!(
            (deg + 4.9).abs() < 0.1,
            "band edge at {deg:.2}°, expected −4.9° (RE §7)"
        );

        // The rule this replaced — `eye * MS + Offset`, the full authored
        // Offset in metres over the normalised 1000 m shell — put the same
        // edge at +0.63°, above the horizon:
        // `atan2(ring_y * k + 100 - 0.005 * eye.y, ring_r * k)`.
    }
}
