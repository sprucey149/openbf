//! The sniper scope's overlay — the PS2's `ScopeDisplay` HUD element
//! (`analysis/re-zoom.md` §8 [V][O]; BF1 vtable 0x540880, update
//! `FUN_002d66a8`; BF2 vtable 0x62e9a0, built by `FUN_0038f050`, update
//! `FUN_0038f558`).
//!
//! - **When**: the viewport live, the unit alive, the zoom flag set on a
//!   weapon whose zoom is the scope view (`ZoomFirstPerson`) and the
//!   camera's zoom S above 1.0 (B2:388231-388245). The game decides it
//!   (`game::hud_feed::scope_view`); the HUD draws [`ScopeView`].
//! - **What**: four quads, each half the screen wide and high, all textured
//!   `weapon_scope` (one quarter of the picture: the dark surround, the
//!   window, the range ladders and a quarter of the centre ring) — the
//!   top-left one as stored, the top-right mirrored left/right, the
//!   bottom-left mirrored top/bottom, the bottom-right both
//!   (`FUN_0038f050`: sizes W/2 x H/2, UV rects (0,0,1,1), (1,0,0,1),
//!   (0,1,1,0), (1,1,0,0)). BF2 applies no `ScopeTexture`: the ELF's one
//!   set-texture call is the constant fnv("weapon_scope") (§8 [D]).
//! - **Where in the HUD**: BF2's driver `FUN_003b61e0` switches the weapon
//!   panel off while the display is up (it sends `player%d.weaponsDisable`,
//!   B2:411756-411767, and `weaponsEnable` again after) and skips the health,
//!   energy and jet fuel senders; the target readout and the reticle keep
//!   running (`FUN_003b3398` and the reticle placer are called either way)
//!   — `crate::hud::feed`.
//!
//! `[ours]`: the PS2 stretches the quads over its 4:3 frame; the port lays
//! them over the HUD's 4:3 box (the window's height, centred) and fills the
//! wider window's sides with the picture's own edge column, so the ring
//! stays round. The overlay is drawn under the HUD tree, so the readout at
//! the top and the reticle stay over it — the PS2's order between the
//! element and the layout was not read [H]. BF2 freezes the health and
//! energy bars while scoped (their senders are skipped); the port keeps
//! them current. The zoom sounds (`ScopeDisplayZoomIn` / `Out`) are not
//! ported here.

use crate::hud_draw::{color, Ctx, Quad, Screen};
use glam::Mat4;

/// The scope picture's sheet: BF2's (INGAME.LVL), on every map. BF1's
/// COMMON.LVL copy is not baked (`bake::map::content`).
pub const SHEET: &str = "weapon_scope";

/// The scope overlay is up this frame; it draws [`SHEET`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeView;

/// The quarter-screen quads in window pixels for a `size` window: (rect
/// x0, y0, x1, y1; uv u0, v0, u1, v1). The 4:3 box first (four quads),
/// then the side fills when the window is wider.
pub fn quads(size: [f32; 2]) -> Vec<([f32; 4], [f32; 4])> {
    let [w, h] = size;
    let bw = (4.0 / 3.0 * h).min(w);
    let x0 = (w - bw) * 0.5;
    let (cx, cy) = (x0 + bw * 0.5, h * 0.5);
    let mut out = vec![
        ([x0, 0.0, cx, cy], [0.0, 0.0, 1.0, 1.0]),
        ([cx, 0.0, x0 + bw, cy], [1.0, 0.0, 0.0, 1.0]),
        ([x0, cy, cx, h], [0.0, 1.0, 1.0, 0.0]),
        ([cx, cy, x0 + bw, h], [1.0, 1.0, 0.0, 0.0]),
    ];
    if x0 > 0.0 {
        // The picture's outer column, stretched sideways.
        for (rect_x, v) in [((0.0, x0), (0.0, 1.0)), ((x0 + bw, w), (0.0, 1.0))] {
            out.push(([rect_x.0, 0.0, rect_x.1, cy], [0.0, v.0, 0.0, v.1]));
            out.push(([rect_x.0, cy, rect_x.1, h], [0.0, v.1, 0.0, v.0]));
        }
    }
    out
}

impl Ctx<'_> {
    pub(crate) fn scope(&mut self, _: &ScopeView, size: [f32; 2]) {
        let s = Screen {
            origin: [0.0, 0.0],
            scale: [1.0, 1.0],
        };
        for (rect, uv) in quads(size) {
            self.quad(
                usize::MAX,
                &Mat4::IDENTITY,
                &s,
                &Quad {
                    rect,
                    uv,
                    color: color([255.0; 3], 1.0),
                    texture: SHEET,
                    additive: false,
                    shadow: false,
                    boxed: false,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4:3 window: four quads meeting at the centre, each mirrored
    /// toward it (the ring's quarter of the top-left quad sits at its
    /// bottom-right).
    #[test]
    fn four_mirrored_quarters_meet_at_the_centre() {
        let q = quads([640.0, 480.0]);
        assert_eq!(q.len(), 4);
        assert_eq!(q[0], ([0.0, 0.0, 320.0, 240.0], [0.0, 0.0, 1.0, 1.0]));
        assert_eq!(q[1], ([320.0, 0.0, 640.0, 240.0], [1.0, 0.0, 0.0, 1.0]));
        assert_eq!(q[2], ([0.0, 240.0, 320.0, 480.0], [0.0, 1.0, 1.0, 0.0]));
        assert_eq!(q[3], ([320.0, 240.0, 640.0, 480.0], [1.0, 1.0, 0.0, 0.0]));
    }

    /// A wide window: the picture keeps the 4:3 box of the height, centred,
    /// and the sides take its outer column, so nothing of the scene shows
    /// beside it.
    #[test]
    fn a_wide_window_fills_its_sides() {
        let q = quads([1280.0, 720.0]);
        assert_eq!(q.len(), 8);
        assert_eq!(q[0].0, [160.0, 0.0, 640.0, 360.0]);
        assert_eq!(q[3].0, [640.0, 360.0, 1120.0, 720.0]);
        let covered: f32 = q.iter().map(|(r, _)| (r[2] - r[0]) * (r[3] - r[1])).sum();
        assert_eq!(covered, 1280.0 * 720.0);
        assert!(q[4..].iter().all(|(_, uv)| uv[0] == 0.0 && uv[2] == 0.0));
    }
}
