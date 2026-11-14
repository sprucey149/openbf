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
}

/// Classic flight: the left stick pitches and turns, the right throttles
/// and rolls. Both games' default tables for a flyer swap the two sticks' functions
/// against the soldier's and the ground vehicle's (BF2 0x600ee0 words 16..19, 32 33 30
/// 31, against 30 31 32 33 at 0x600e40; BF1 file offset 0x3ba210; read back by BF2
/// `FUN_004333a0`, BF1 `FUN_0034a7e8`). The look stick's own [`LOOK_SCALE`] comes
/// off: a flyer's stick reaches 1, and BF2's rate blend runs over 0.7 to 1.
/// Only the controller's axes: the keyboard's walk keys are merged after this.
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
