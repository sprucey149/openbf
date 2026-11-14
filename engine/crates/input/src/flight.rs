use core::sim::{PadState, LOOK_SCALE};

/// The controller's flight-stick layout, before merging keyboard and mouse.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FlightControls {
    #[default]
    Classic,
    RcMode2,
}

impl FlightControls {
    pub fn toggled(self) -> Self {
        match self {
            Self::Classic => Self::RcMode2,
            Self::RcMode2 => Self::Classic,
        }
    }

    /// Remap the sampled controller axes to the sim's roll, throttle, yaw
    /// and pitch channels. The right stick's look scaling comes off here.
    pub fn map_sticks(self, pad: PadState) -> PadState {
        let classic = flyer_sticks(pad);
        match self {
            Self::Classic => classic,
            Self::RcMode2 => PadState {
                ly: classic.ry,
                ry: -classic.ly,
                ..classic
            },
        }
    }

    /// Hover-capable aircraft translate on the left stick and aim on the right.
    /// RC Mode 2 remains an explicit pilot preference for either flight model.
    pub fn map_hover_sticks(self, pad: PadState) -> PadState {
        if self == Self::RcMode2 {
            return self.map_sticks(pad);
        }
        let unscaled =
            |a: i16| PadState::quantize((PadState::axis(a) / LOOK_SCALE).clamp(-1.0, 1.0));
        PadState {
            rx: unscaled(pad.rx),
            ry: unscaled(pad.ry),
            ..pad
        }
    }
}

/// Classic for forward-flying aircraft: left pitches/turns, right throttles/rolls.
/// Hovering aircraft instead use [`FlightControls::map_hover_sticks`].
/// BF2's default table for a flyer pilot swaps the two sticks' functions
/// against the soldier's and the ground vehicle's (BF2 0x600ee0 words 16..19, 32 33 30
/// 31, against 30 31 32 33 at 0x600e40, read back by `FUN_004333a0`). The look
/// stick's own [`LOOK_SCALE`] comes off: a flyer's stick reaches 1, and BF2's rate
/// blend runs over 0.7 to 1.
/// This layout applies to every source map (maintainer, 2026-11-14), a named
/// difference from the measured BF1 MAF table for forward-flying aircraft.
/// Only controller axes are mapped;
/// keyboard walk keys are merged after this.
pub fn flyer_sticks(pad: PadState) -> PadState {
    let unscaled = |a: i16| PadState::quantize((PadState::axis(a) / LOOK_SCALE).clamp(-1.0, 1.0));
    PadState {
        lx: unscaled(pad.rx),
        ly: unscaled(pad.ry),
        rx: pad.lx,
        ry: pad.ly,
        ..pad
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sim::buttons;

    #[test]
    fn hovering_classic_moves_on_left_and_aims_on_right() {
        let pad = PadState {
            lx: 17000,
            ly: -23000,
            rx: PadState::quantize(LOOK_SCALE),
            ry: PadState::quantize(-LOOK_SCALE),
            buttons: buttons::FIRE,
            ..Default::default()
        };
        let mapped = FlightControls::Classic.map_hover_sticks(pad);
        assert_eq!(
            (mapped.lx, mapped.ly, mapped.buttons),
            (pad.lx, pad.ly, pad.buttons)
        );
        assert!(PadState::axis(mapped.rx) > 0.999);
        assert!(PadState::axis(mapped.ry) < -0.999);
        assert_eq!(
            FlightControls::RcMode2.map_hover_sticks(pad),
            FlightControls::RcMode2.map_sticks(pad)
        );
    }

    #[test]
    fn default_layout_reserves_the_left_stick_for_steering() {
        let left = FlightControls::default().map_sticks(PadState {
            lx: PadState::quantize(0.75),
            ly: PadState::quantize(-0.5),
            ..Default::default()
        });
        assert_eq!((left.lx, left.ly), (0, 0), "no throttle or roll");
        assert!((PadState::axis(left.rx) - 0.75).abs() < 1e-4);
        assert!((PadState::axis(left.ry) + 0.5).abs() < 1e-4);
        let right = FlightControls::default().map_sticks(PadState {
            rx: PadState::quantize(LOOK_SCALE),
            ry: PadState::quantize(-LOOK_SCALE),
            ..Default::default()
        });
        assert_eq!((right.rx, right.ry), (0, 0), "no steering");
        assert!(PadState::axis(right.lx) > 0.999);
        assert!(PadState::axis(right.ly) < -0.999);
    }

    #[test]
    fn a_flyer_pilots_sticks_swap_and_reach_full() {
        let pad = PadState {
            lx: PadState::quantize(1.0),
            ly: PadState::quantize(-1.0),
            rx: PadState::quantize(LOOK_SCALE),
            ry: PadState::quantize(-LOOK_SCALE / 2.0),
            buttons: buttons::FIRE,
            ..PadState::default()
        };
        let f = flyer_sticks(pad);
        assert_eq!((f.rx, f.ry), (pad.lx, pad.ly));
        assert!((PadState::axis(f.lx) - 1.0).abs() < 1e-3);
        assert!((PadState::axis(f.ly) + 0.5).abs() < 1e-3);
        assert_eq!(f.buttons, pad.buttons);
    }

    #[test]
    fn rc_flight_controls_assign_left_throttle_and_right_pitch() {
        let pad = PadState {
            lx: PadState::quantize(-0.25),
            ly: PadState::quantize(0.5),
            rx: PadState::quantize(-LOOK_SCALE),
            ry: PadState::quantize(LOOK_SCALE),
            buttons: buttons::FIRE | buttons::SPRINT,
            ..PadState::default()
        };
        let classic = FlightControls::default();
        assert_eq!(classic, FlightControls::Classic);
        assert_eq!(classic.map_sticks(pad), flyer_sticks(pad));
        let rc = classic.toggled();
        assert_eq!(rc.toggled(), classic);
        let f = rc.map_sticks(pad);
        let c = flyer_sticks(pad);
        assert_eq!((f.lx, f.ly, f.rx), (c.lx, pad.ly, pad.lx));
        assert!((PadState::axis(f.lx) + 1.0).abs() < 1e-4);
        assert!(PadState::axis(f.ry) < -0.9999);
        let pulled_back = rc.map_sticks(PadState { ry: -pad.ry, ..pad });
        assert!(PadState::axis(pulled_back.ry) > 0.9999);
        assert_eq!(f.buttons, pad.buttons);
        assert_eq!(rc.map_sticks(PadState::default()), PadState::default());
    }
}
