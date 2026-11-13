//! BF2's tread scroll props (`analysis/re-hover.md` §1, §6.18): up to four
//! segments, each named by its MNAM, whose UVs slide with the hover's
//! forward speed and yaw rate. The hover setter `FUN_002223a0` writes them
//! into class K at the slot a select prop last chose (B2:136815-137250); the
//! constructor and the derive copy reset that slot to 0 (B2:135947,
//! B2:136122) and the derive copies the count and the four tables
//! (B2:136276-136281). BF1 has no such props.

use super::h;

/// K's four tables (+0xfa0 / +0xfb0 / +0xfc0 / +0xfd0 / +0xfe0, four words each).
pub const MAX_TREADS: usize = 4;

/// The select prop's hash; its name is not known. Its value is a
/// `strtol(v, 0, 10)` (`FUN_00520f50`) and the slot is that minus one
/// (B2:136514-136516). OURS: a slot outside 0..4 writes nothing here, where
/// the PS2 writes past the tables; no shipped class selects one (the
/// hailfire selects 1 and 2, the snail tank 1). trace: the LVL class scan.
const SELECT: u32 = 0x1a65_93db;

/// One tread segment's scroll rates, in UV units per metre of forward
/// speed (`veloc`) and per radian of yaw (`omega`), `[u, v]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tread {
    /// The segment's MNAM, as authored; the draw compares it without case
    /// (the PS2 compares the names' hashes, `FUN_0057dce0`).
    pub segment: String,
    pub veloc: [f32; 2],
    pub omega: [f32; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Treads {
    slots: [Tread; MAX_TREADS],
    count: usize,
    select: usize,
}

impl Treads {
    /// The derive copy: the tables and count stay, the select slot is 0.
    pub fn derive(&mut self) {
        self.select = 0;
    }

    /// The treads K+0xf9c counts.
    pub fn list(&self) -> &[Tread] {
        &self.slots[..self.count]
    }

    /// One prop; false when `k` is none of these.
    pub fn set(&mut self, k: u32, v: &str) -> bool {
        if k == SELECT {
            self.select = strtol(v)
                .checked_sub(1)
                .and_then(|i| usize::try_from(i).ok())
                .unwrap_or(usize::MAX);
            return true;
        }
        let slot = Some(self.select).filter(|&i| i < MAX_TREADS);
        let float = |v: &str| v.trim().parse::<f32>().ok();
        if k == h("WheelTexture") {
            if let Some(i) = slot {
                self.slots[i].segment = v.trim().to_owned();
                self.count = self.count.max(i + 1);
            }
        } else if k == h("WheelVelocToU") {
            if let (Some(i), Some(x)) = (slot, float(v)) {
                self.slots[i].veloc[0] = x;
            }
        } else if k == h("WheelVelocToV") {
            if let (Some(i), Some(x)) = (slot, float(v)) {
                self.slots[i].veloc[1] = x;
            }
        } else if k == h("WheelOmegaToU") {
            if let (Some(i), Some(x)) = (slot, float(v)) {
                self.slots[i].omega[0] = x;
            }
        } else if k == h("WheelOmegaToV") {
            if let (Some(i), Some(x)) = (slot, float(v)) {
                self.slots[i].omega[1] = x;
            }
        } else {
            return false;
        }
        true
    }
}

/// `strtol(v, 0, 10)`: leading blanks, a sign, then digits; 0 for none.
fn strtol(v: &str) -> i64 {
    let t = v.trim_start();
    let end = t
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+'))))
        .map_or(t.len(), |(i, _)| i);
    t[..end].parse().unwrap_or(0)
}

/// One tick of K's scroll for one tread (B2 `FUN_0021d8b8`,
/// B2:134057-134109): each offset moves by `dt (vF · veloc + ωy · omega)`
/// and a result below 0 or above 1 is brought back by one.
pub fn step(offset: [f32; 2], t: &Tread, forward_speed: f32, yaw_rate: f32, dt: f32) -> [f32; 2] {
    let d = delta(t, forward_speed, yaw_rate, dt);
    let mut out = offset;
    for (o, d) in out.iter_mut().zip(d) {
        let x = *o + d;
        *o = if x < 0.0 {
            x + 1.0
        } else if x > 1.0 {
            x - 1.0
        } else {
            x
        };
    }
    out
}

/// What one [`step`] adds before the wrap: `dt (vF · veloc + ωy · omega)`.
pub fn delta(t: &Tread, forward_speed: f32, yaw_rate: f32, dt: f32) -> [f32; 2] {
    [0, 1].map(|i| dt * (forward_speed * t.veloc[i] + yaw_rate * t.omega[i]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(list: &[(&str, &str)]) -> Treads {
        let mut t = Treads::default();
        for (k, v) in list {
            let key = if *k == "select" { SELECT } else { h(k) };
            assert!(t.set(key, v), "{k}");
        }
        t
    }

    #[test]
    fn the_snail_tank_and_the_hailfire_as_authored() {
        let snail = props(&[
            ("select", "1"),
            ("WheelTexture", "cis_tank_tread_tracks"),
            ("WheelVelocToV", "-0.8333333"),
            ("WheelOmegaToV", "0"),
        ]);
        assert_eq!(snail.list().len(), 1);
        assert_eq!(snail.list()[0].segment, "cis_tank_tread_tracks");
        assert_eq!(snail.list()[0].veloc, [0.0, -0.8333333]);
        let hailfire = props(&[
            ("select", "1"),
            ("WheelTexture", "wheel_left"),
            ("WheelVelocToU", "0.4"),
            ("WheelOmegaToU", "-1.1"),
            ("select", "2"),
            ("WheelTexture", "wheel_right"),
            ("WheelVelocToU", "0.4"),
            ("WheelOmegaToU", "1.1"),
        ]);
        let names: Vec<_> = hailfire.list().iter().map(|t| t.segment.as_str()).collect();
        assert_eq!(names, ["wheel_left", "wheel_right"]);
        assert_eq!(hailfire.list()[1].omega, [1.1, 0.0]);
    }

    #[test]
    fn a_rate_before_any_texture_still_lands_in_its_slot() {
        let t = props(&[("WheelVelocToV", "2"), ("WheelTexture", "a")]);
        assert_eq!(t.list()[0].veloc, [0.0, 2.0]);
    }

    #[test]
    fn reversing_raises_v_and_driving_on_lowers_it() {
        let t = Tread {
            segment: String::new(),
            veloc: [0.0, -0.8333333],
            omega: [0.0, 0.0],
        };
        let dt = 1.0 / 30.0;
        let back = step([0.0, 0.5], &t, -8.0, 0.0, dt);
        assert!((back[1] - (0.5 + 8.0 * 0.8333333 * dt)).abs() < 1e-6);
        let on = step([0.0, 0.05], &t, 9.0, 0.0, dt);
        assert!((on[1] - (0.05 - 9.0 * 0.8333333 * dt + 1.0)).abs() < 1e-6);
        assert_eq!(on[0], 0.0);
    }
}
