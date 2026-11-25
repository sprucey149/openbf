//! PC BF2's underwater tint, decided per frame (`analysis/re-water.md` §9.8
//! `[O]`; drawn by `render::Renderer::set_underwater`).
//!
//! PC `BattlefrontII.exe` 0x6cf320 runs on a flat-water map only (not the
//! ocean path, 0x6cf122 / 0x6cf34a) and lays `UnderwaterColor` over the part
//! of the near plane at or under the water's height, LAYR + the bob, when any
//! of the plane's four corners stands over a wet cell (0x6cffc0, called for
//! each corner in the clip loop at 0x6cf705). The height here is the drawn
//! surface's: the manifest's layer plus the renderer-clock bob the water
//! draws ride.

use super::Fx;
use core::sim::Sim;
use render::{Camera, Renderer, UnderwaterTint};

/// What a map authored for the tint: its layer height and `UnderwaterColor`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Underwater {
    pub height: f32,
    pub color: [f32; 4],
}

impl Underwater {
    pub fn from_manifest(m: &manifest::Manifest) -> Option<Underwater> {
        let w = m.water.as_ref()?;
        let c = w.config.as_ref()?;
        if c.ocean {
            return None;
        }
        Some(Underwater {
            height: w.height,
            color: c.underwater_color?,
        })
    }

    /// The frame's tint for `camera` (a view of `aspect`), with the surface
    /// lifted by `lift`: `None` unless a near-plane corner is over a wet cell.
    pub fn tint(
        &self,
        camera: &Camera,
        aspect: f32,
        lift: f32,
        wet: impl Fn(f32, f32) -> bool,
    ) -> Option<UnderwaterTint> {
        camera
            .near_corners(aspect)
            .iter()
            .any(|c| wet(c.x, c.z))
            .then_some(UnderwaterTint {
                level: self.height + lift,
                color: self.color,
            })
    }
}

impl Fx {
    pub(super) fn tint_underwater(&self, r: &mut Renderer, camera: &Camera, sim: &Sim) {
        let tint = self.underwater.and_then(|u| {
            let plane = sim.water_plane()?;
            let (w, h) = r.size();
            let lift = self
                .water
                .lift(crate::water_anim::clock(r.water_time) as f32);
            u.tint(camera, w as f32 / h.max(1) as f32, lift, |x, z| {
                plane.wet(x, z)
            })
        });
        r.set_underwater(tint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn abr() -> Underwater {
        Underwater {
            height: -1.3,
            color: [61.0, 124.0, 144.0, 128.0],
        }
    }

    #[test]
    fn a_wet_corner_gives_the_lifted_level_and_the_authored_colour() {
        let camera = Camera {
            position: Vec3::new(-36.92, -1.24, -44.37),
            ..Camera::default()
        };
        let tint = abr().tint(&camera, 16.0 / 9.0, 0.05, |_, _| true).unwrap();
        assert_eq!(tint.level, -1.3 + 0.05);
        assert_eq!(tint.color, [61.0, 124.0, 144.0, 128.0]);
    }

    #[test]
    fn a_dry_near_plane_draws_no_tint() {
        let camera = Camera::default();
        assert_eq!(abr().tint(&camera, 1.0, 0.0, |_, _| false), None);
    }

    #[test]
    fn one_wet_corner_is_enough() {
        let camera = Camera::default();
        let tint = abr().tint(&camera, 1.0, 0.0, |x, _| x > 0.0);
        assert!(tint.is_some());
    }
}
