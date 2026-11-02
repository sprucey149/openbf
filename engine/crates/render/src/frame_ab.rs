//! Per-frame interleaved pricing: what a frame leaves out or does differently,
//! so one run can draw the same world in several states and compare them.
//! Separate runs cannot do this here: another job on the machine moved the
//! Intel GPU's timestamps by a factor of four between runs, while frames drawn
//! a few milliseconds apart see the same machine. `game --bench` drives it
//! (`OPENBF_FRAMEAB`, `main/headless/detailed_bench.rs`); a game never sets this.

use crate::DrawOrder;

/// The sections of a frame that can be left out, named as the GPU timer names
/// them (`frame::run_section`).
const SECTIONS: [&str; 13] = [
    "sky",
    "terrain",
    "terrain layers",
    "far sheet",
    "models",
    "specular",
    "soldiers",
    "water",
    "grass",
    "blend",
    "bolts+particles",
    "bloom",
    "filter",
];

/// How the next frame differs from the game's own. The default is the game's.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FrameAb {
    skip: u32,
    pub(crate) order: DrawOrder,
    /// Every draw through the pipelines that can discard.
    pub(crate) discard: bool,
    /// The world at this fraction of the window (`world_scale`).
    pub(crate) world_scale: Option<f32>,
    /// Foliage drawn as it was before the distance and frustum filters.
    pub(crate) foliage_unfiltered: bool,
}

impl FrameAb {
    /// A state by name: `base`, `no:<section>`, `order:<order>`, `discard:on`,
    /// `scale:<fraction>`, `foliage:unfiltered`, or several joined by `~`.
    pub fn parse(name: &str) -> Result<Self, String> {
        let mut ab = Self::default();
        for part in name.split('~') {
            ab.apply(part)?;
        }
        Ok(ab)
    }

    fn apply(&mut self, name: &str) -> Result<(), String> {
        let ab = self;
        if name == "base" {
            return Ok(());
        }
        match name.split_once(':') {
            Some(("no", section)) => {
                let bit = SECTIONS
                    .iter()
                    .position(|s| *s == section)
                    .ok_or_else(|| format!("no frame section {section:?}"))?;
                ab.skip |= 1 << bit;
            }
            Some(("order", order)) => {
                ab.order = DrawOrder::parse(order).ok_or_else(|| format!("no order {order:?}"))?;
            }
            Some(("discard", "on")) => ab.discard = true,
            Some(("foliage", "unfiltered")) => ab.foliage_unfiltered = true,
            Some(("scale", fraction)) => {
                let f = fraction
                    .parse::<f32>()
                    .map_err(|_| format!("no scale {fraction:?}"))?;
                ab.world_scale = Some(f.clamp(0.25, 1.0));
            }
            _ => return Err(format!("no frame state {name:?}")),
        }
        Ok(())
    }

    /// Whether this frame leaves out `section`.
    pub(crate) fn skips(&self, section: &str) -> bool {
        self.skip != 0
            && SECTIONS
                .iter()
                .position(|s| *s == section)
                .is_some_and(|bit| self.skip & (1 << bit) != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_names_one_change_and_base_changes_nothing() {
        assert_eq!(FrameAb::parse("base"), Ok(FrameAb::default()));
        let no_models = FrameAb::parse("no:models").unwrap();
        assert!(no_models.skips("models") && !no_models.skips("terrain"));
        assert_eq!(no_models.order, DrawOrder::default());
        assert_eq!(FrameAb::parse("order:mesh").unwrap().order, DrawOrder::Mesh);
        assert!(FrameAb::parse("discard:on").unwrap().discard);
        assert_eq!(
            FrameAb::parse("scale:0.75").unwrap().world_scale,
            Some(0.75)
        );
        assert!(!FrameAb::default().skips("sky"));
        let both = FrameAb::parse("order:mesh~no:sky~no:water").unwrap();
        assert_eq!(both.order, DrawOrder::Mesh);
        assert!(both.skips("sky") && both.skips("water") && !both.skips("terrain"));
        assert!(
            FrameAb::parse("foliage:unfiltered")
                .unwrap()
                .foliage_unfiltered
        );
        for bad in [
            "no:nothing",
            "order:sideways",
            "scale:big",
            "foliage:far",
            "fast",
        ] {
            assert!(FrameAb::parse(bad).is_err(), "{bad}");
        }
    }

    /// Each section in the table has a bit of its own. The frame's labels are
    /// held to this table by `frame::tests`.
    #[test]
    fn every_section_a_frame_can_skip_has_a_bit() {
        for section in SECTIONS {
            let ab = FrameAb::parse(&format!("no:{section}")).unwrap();
            assert!(ab.skips(section), "{section}");
        }
    }
}
