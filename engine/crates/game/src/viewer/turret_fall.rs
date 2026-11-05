//! A fallen building turret's look (card TBLD, `crate::turret_fall`): while
//! the sim has it down (`core::sim::Sim::building_down`) the view leaves its
//! intact draws out and draws its wreck or build pad, rigid at the instance
//! (`analysis/re-turret-draw.md` 5 [O]); a hidden one draws nothing. Health
//! above 0 on a standing turret is the intact look; a fallen one stays down
//! until it is built at full health (`analysis/re-build-zones.md` 1 [V]).

use render::DrawCmd;
use std::ops::Range;

/// One placed building turret's draws: the intact ones (model, LOWD, far
/// extension) and the fallen look's, kept out of [`super::SceneGpu::draws`].
pub struct FallenDraw {
    /// The manifest instance (the sim's `BuildingTurret::instance`).
    pub instance: usize,
    /// Its intact draws in [`super::SceneGpu::draws`].
    pub intact: Vec<Range<usize>>,
    /// The fallen look's draws (none when hidden or not baked).
    pub fallen: Vec<DrawCmd>,
}

impl FallenDraw {
    /// An ocean step spliced draws `old` into `new_len` (see
    /// `reindex_after_splice`): every index past it shifts.
    pub(super) fn shift(&mut self, old: &Range<usize>, new_len: usize) {
        let delta = new_len as isize - old.len() as isize;
        let shift = |k: usize| {
            if k >= old.end {
                (k as isize + delta) as usize
            } else {
                k
            }
        };
        for r in &mut self.intact {
            *r = shift(r.start)..shift(r.end);
        }
    }
}

impl super::SceneGpu {
    /// The intact draws of every building turret the sim has down, and the
    /// fallen looks drawn in their place (at the instance's see-through
    /// opacity, [`super::SceneGpu::see_through`]).
    pub(super) fn fallen_view(&self, sim: &core::sim::Sim) -> (Vec<Range<usize>>, Vec<DrawCmd>) {
        let mut hide = Vec::new();
        let mut add = Vec::new();
        for f in &self.fallen {
            if sim.building_down(f.instance as u32) {
                hide.extend(f.intact.iter().cloned());
                // The chase camera's see-through takes the fallen look too.
                let fade = self
                    .faded
                    .iter()
                    .find(|(i, _)| *i as usize == f.instance)
                    .map(|&(_, o)| render::Fade::see_through(o));
                if !sim.building_hidden(f.instance as u32) {
                    add.extend(f.fallen.iter().map(|d| DrawCmd { fade, ..*d }));
                }
            }
        }
        (hide, add)
    }
}
