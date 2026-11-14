//! The laser look (card FXB, `analysis/re-bolt.md` §1–§4 [O]): what an
//! OrdnanceLaser class (`bolt` / `laser`) draws with — its keys after the
//! setters' rules, and the per-bolt colour and width rules the draw applies.
//! Render-only: [`crate::weapon::OrdnanceClass::laser`] carries it and the
//! sim never reads it. It also holds the bullet base's light keys
//! (`LightColor`, `LightRadius`, BF2 `EndLightColor`): BF2 gives every such
//! bolt an omni light (re-bolt §5.2 [O], card FXBL); BF1 parses them and
//! never reads them (§5.1).

use crate::fmath::FloatExt;
use crate::weapon::{HASH_FALLOFF_DURATION, HASH_FALLOFF_START};

/// Laser keys (fnv over byte|0x20, `analysis/re-bolt.md` §1 [O]).
pub const HASH_LASER_WIDTH: u32 = 0x6f17_9ab8;
pub const HASH_LASER_LENGTH: u32 = 0xba85_8240;
pub const HASH_LASER_TEXTURE: u32 = 0xb709_b5e7;
pub const HASH_LASER_GLOW_COLOR: u32 = 0x3058_0110;
pub const HASH_END_LASER_GLOW_COLOR: u32 = 0x266f_54f7;
pub const HASH_GLOW_LENGTH: u32 = 0xc3a8_b510;
pub const HASH_BLUR_LENGTH: u32 = 0x9d0f_b7a4;
pub const HASH_FADE_OUT_TIME: u32 = 0x798d_c484;
pub const HASH_FADE_LIGHT_COLOR: u32 = 0xa58a_cbd0;
/// The bullet base's light keys (re-bolt §1 [O]).
pub const HASH_LIGHT_COLOR: u32 = 0x746c_e73e;
pub const HASH_LIGHT_RADIUS: u32 = 0x2776_564d;
pub const HASH_END_LIGHT_COLOR: u32 = 0x96e0_ccb1;

/// The BF2 beam setter's own keys (B2 `FUN_00312838`, B2:291437-291560;
/// `analysis/re-hover-camera.md` 6).
pub const HASH_BEAM_COLOR: u32 = 0xa309_3acd;
pub const HASH_BEAM_GLOW_WIDTH: u32 = 0x398b_9ad9;
pub const HASH_BEAM_GLOW_TEXTURE: u32 = 0xc6bf_c336;

/// A beam's glow half-width over its `LaserWidth` (B1 `FUN_002758e0`
/// B1:196928, B2 setter B2:291470).
pub const BEAM_GLOW_WIDTH: f32 = 2.5;

/// The setters' length clamp (B1 `lui 0x4300` at 0x27cbb0).
pub const LASER_MAX_LENGTH: f32 = 128.0;

/// What an OrdnanceLaser class (`bolt` / `laser`) draws with: re-bolt §1's
/// keys after its setter rules. Built by replaying the class chain's PROPs
/// in disc order, parent first ([`LaserLook::set`]), because `LaserLength`
/// also resets `GlowLength` and `BlurLength`. Charge levels are not
/// modelled (the class's own values; no bolt the port fires has a chain).
#[derive(Clone, Debug, PartialEq)]
pub struct LaserLook {
    /// `LaserTexture` as authored (the texture table key is its fnv).
    pub texture: Option<String>,
    /// `LaserWidth`: the core's half-width at the head, metres.
    pub width: f32,
    /// `LaserLength`: the core ribbon's longest.
    pub length: f32,
    /// `GlowLength`: the glow ribbon's longest.
    pub glow_length: f32,
    /// `BlurLength`: only bounds the drawn length's growth (re-bolt §4).
    pub blur_length: f32,
    /// `LaserGlowColor`, bytes as authored (ctor: the static white).
    pub glow_color: [u8; 4],
    /// BF2 `EndLaserGlowColor` (ctor white).
    pub end_glow_color: [u8; 4],
    /// BF2 `FadeLightColor`: the glow fades toward the End colour.
    pub fade_light: bool,
    /// BF2 class +0x34 / +0x38 (the damage-falloff start and length, keys
    /// uncracked): the End fade's window in seconds of flight. Ctor 0 [H].
    pub fade_start: f32,
    pub fade_duration: f32,
    /// 1 / `FadeOutTime` (0 = none): the fade after the hit, per second.
    pub fade_out_rate: f32,
    /// `LightColor` after the setter (RGB × A/255, alpha byte 0xff once
    /// set; ctor 0 — [`LaserLook::light`]'s gate).
    pub light_color: [u8; 4],
    /// BF2 `EndLightColor`, the same rule: the light's End colour.
    pub end_light_color: [u8; 4],
    /// `LightRadius`, metres (ctor 0).
    pub light_radius: f32,
    /// An OrdnanceBeam class's own draw, `None` for a bolt.
    pub beam: Option<BeamLook>,
}

/// What an OrdnanceBeam class (`beam`) draws with (B1 `FUN_002758e0`
/// B1:196842-196931, B2 `FUN_00312438` B2:291272-291366;
/// `analysis/re-hover-camera.md` 6): two ribbons from the muzzle to the
/// beam's end at one width each, no taper and no fade. The core is
/// [`LaserLook::texture`] at half-width [`LaserLook::width`], NORMAL blend;
/// the glow is `glow_texture` at `glow_width` in [`LaserLook::glow_color`].
#[derive(Clone, Debug, PartialEq)]
pub struct BeamLook {
    /// The core's vertex colour: BF2 `BeamColor` (ctor the static white),
    /// BF1 the static white.
    pub color: [u8; 4],
    /// The glow's half-width: 2.5 x `LaserWidth`, BF2 `BeamGlowWidth`.
    pub glow_width: f32,
    /// BF2 `BeamGlowTexture` as authored; `None` = `lightglow` (both ctors).
    pub glow_texture: Option<String>,
    /// The glow blends ADDITIVE on BF1 (`FUN_004afac0(lightglow, 1)`,
    /// B1:196927), NORMAL on BF2 (`FUN_00594270(C+0xd0, 0)`, B2:291360).
    pub glow_additive: bool,
}

impl Default for LaserLook {
    /// The ctors' defaults (B1 `FUN_0027c9d0`, B2 `FUN_0031ba10`): zeros and
    /// the static white.
    fn default() -> Self {
        Self {
            texture: None,
            width: 0.0,
            length: 0.0,
            glow_length: 0.0,
            blur_length: 0.0,
            glow_color: [255; 4],
            end_glow_color: [255; 4],
            fade_light: false,
            fade_start: 0.0,
            fade_duration: 0.0,
            fade_out_rate: 0.0,
            light_color: [0; 4],
            end_light_color: [0; 4],
            light_radius: 0.0,
            beam: None,
        }
    }
}

fn colour_bytes(v: &str, into: &mut [u8; 4]) {
    for (k, t) in v.split_whitespace().take(4).enumerate() {
        if let Ok(f) = t.parse::<f32>() {
            into[k] = f.clamp(0.0, 255.0) as u8;
        }
    }
}

/// The light colour setter (B2 `FUN_00314b98` 305324–305340, B1 198401–
/// 198416): `%d` values; four → (R·A/255, G·A/255, B·A/255, 0xff) in
/// integer division, three → (R, G, B, 0xff), fewer → unchanged. A token
/// the scan stops inside (`253.5`) still counts its integer prefix and ends
/// the scan, as `sscanf` would.
fn light_bytes(v: &str, into: &mut [u8; 4]) {
    let mut vals = Vec::with_capacity(4);
    for t in v.split_whitespace().take(4) {
        let digits = t
            .char_indices()
            .take_while(|&(i, c)| c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+')))
            .count();
        let Ok(n) = t[..digits].parse::<i32>() else {
            break;
        };
        vals.push(n);
        if digits < t.len() {
            break;
        }
    }
    match vals[..] {
        [r, g, b, a, ..] => *into = [r * a / 255, g * a / 255, b * a / 255, 0xff].map(|x| x as u8),
        [r, g, b] => *into = [r as u8, g as u8, b as u8, 0xff],
        _ => {}
    }
}

impl LaserLook {
    /// A rifle's sizes (re-bolt §2: width 0.1, LaserLength 1 → 3 / 5 after
    /// the authored GlowLength 3, BlurLength 5); the built-in troopers'.
    pub fn rifle() -> Self {
        Self {
            width: 0.1,
            length: 1.0,
            glow_length: 3.0,
            blur_length: 5.0,
            ..Self::default()
        }
    }

    /// One PROP through the setters (B1 `FUN_0027cae0` + the bullet base
    /// `FUN_002774b0`, B2 `FUN_0031bb60` + `FUN_00314b98`); `false` for a key
    /// the look does not hold.
    pub fn set(&mut self, hash: u32, value: &str) -> bool {
        let f = || value.split_whitespace().next()?.parse::<f32>().ok();
        match hash {
            HASH_LASER_WIDTH => self.width = f().unwrap_or(0.0),
            HASH_LASER_LENGTH => {
                let v = f().unwrap_or(0.0);
                self.length = v.fmin(LASER_MAX_LENGTH);
                self.glow_length = (3.0 * v).fmin(LASER_MAX_LENGTH);
                self.blur_length = (15.0 * v).fmin(LASER_MAX_LENGTH);
            }
            HASH_GLOW_LENGTH => self.glow_length = f().unwrap_or(0.0).fmin(LASER_MAX_LENGTH),
            HASH_BLUR_LENGTH => self.blur_length = f().unwrap_or(0.0).fmin(LASER_MAX_LENGTH),
            HASH_LASER_TEXTURE => self.texture = Some(value.trim().to_owned()),
            HASH_LASER_GLOW_COLOR => colour_bytes(value, &mut self.glow_color),
            HASH_END_LASER_GLOW_COLOR => colour_bytes(value, &mut self.end_glow_color),
            HASH_FADE_OUT_TIME => {
                let v = f().unwrap_or(0.0);
                self.fade_out_rate = if v > 0.0 { 1.0 / v } else { 0.0 };
            }
            HASH_FADE_LIGHT_COLOR => self.fade_light = f().unwrap_or(0.0) != 0.0,
            HASH_LIGHT_COLOR => light_bytes(value, &mut self.light_color),
            HASH_END_LIGHT_COLOR => light_bytes(value, &mut self.end_light_color),
            HASH_LIGHT_RADIUS => self.light_radius = f().unwrap_or(0.0),
            HASH_FALLOFF_START => self.fade_start = f().unwrap_or(0.0),
            HASH_FALLOFF_DURATION => self.fade_duration = f().unwrap_or(0.0),
            _ => return false,
        }
        true
    }

    /// Replay PROPs in order (the class chain parent first, each class's
    /// PROPs in file order).
    pub fn replay<'a>(props: impl IntoIterator<Item = (u32, &'a str)>) -> Self {
        let mut l = Self::default();
        for (h, v) in props {
            l.set(h, v);
        }
        l
    }

    /// An OrdnanceBeam class's look: PROPs replayed through the beam
    /// setter (B1 `FUN_00275bd0` B1:196986-197090, B2 `FUN_00312838`
    /// B2:291437-291560). `LaserWidth` also sets the glow width to 2.5 x
    /// (BF2); `LaserTexture`, `LaserGlowColor`, BF2 `BeamColor`,
    /// `BeamGlowWidth` (which, as the decomp reads, also names the glow
    /// texture after its own value) and `BeamGlowTexture`. Every other key
    /// goes to the ordnance base: no light, lengths or fades.
    pub fn replay_beam<'a>(props: impl IntoIterator<Item = (u32, &'a str)>, bf2: bool) -> Self {
        let mut l = Self::default();
        let mut b = BeamLook {
            color: [255; 4],
            glow_width: 0.0,
            glow_texture: None,
            glow_additive: !bf2,
        };
        for (h, v) in props {
            let f = || v.split_whitespace().next()?.parse::<f32>().ok();
            match h {
                HASH_LASER_WIDTH => {
                    l.width = f().unwrap_or(0.0);
                    b.glow_width = l.width * BEAM_GLOW_WIDTH;
                }
                HASH_LASER_TEXTURE => l.texture = Some(v.trim().to_owned()),
                HASH_LASER_GLOW_COLOR => colour_bytes(v, &mut l.glow_color),
                HASH_BEAM_COLOR if bf2 => colour_bytes(v, &mut b.color),
                HASH_BEAM_GLOW_WIDTH if bf2 => {
                    b.glow_width = f().unwrap_or(0.0);
                    b.glow_texture = Some(v.trim().to_owned());
                }
                HASH_BEAM_GLOW_TEXTURE if bf2 => b.glow_texture = Some(v.trim().to_owned()),
                _ => {}
            }
        }
        if !bf2 {
            b.glow_width = l.width * BEAM_GLOW_WIDTH;
        }
        l.beam = Some(b);
        l
    }

    /// How long the drawn length may grow: max(LaserLength, GlowLength,
    /// BlurLength) (the spawn's cap, re-bolt §4).
    pub fn cap(&self) -> f32 {
        self.length.fmax(self.glow_length).fmax(self.blur_length)
    }

    /// BF2's End fade of `from` toward `to` at `age` seconds of flight (the
    /// glow §3.5 and the light §5.2 share it: B2 304347–304385), rounded per
    /// byte.
    fn end_fade(&self, from: [u8; 4], to: [u8; 4], age: f32) -> [u8; 4] {
        let mut g = from;
        if self.fade_light && age > self.fade_start {
            let f = if self.fade_duration > 0.0 {
                ((age - self.fade_start) / self.fade_duration).clamp(0.0, 1.0)
            } else {
                1.0
            };
            for (b, e) in g.iter_mut().zip(to) {
                *b = (f32::from(*b) * (1.0 - f) + f32::from(e) * f + 0.5) as u8;
            }
        }
        g
    }

    /// The glow's colour bytes at `age` seconds of flight and `fade`
    /// (re-bolt §3.1 step 2, §3.5): BF2's End fade, then × fade when < 1.
    pub fn glow_bytes(&self, age: f32, fade: f32) -> [u8; 4] {
        let mut g = self.end_fade(self.glow_color, self.end_glow_color, age);
        if fade < 1.0 {
            g = g.map(|b| (f32::from(b) * fade.fmax(0.0)) as u8);
        }
        g
    }

    /// The BF2 bolt light at `age` seconds of flight: its RGB bytes and
    /// radius, or `None` for a class that makes none — the ctor's gate,
    /// `LightRadius` > 0 and the `LightColor` alpha byte set (B2
    /// `FUN_003130d8` 304080–304106; a vehicle cannon authors a radius and
    /// no colour). The colour fades to `EndLightColor` as the glow does; the
    /// hit's `FadeOutTime` does not dim it (re-bolt §5.2 [O]).
    pub fn light(&self, age: f32) -> Option<([u8; 3], f32)> {
        if self.light_radius <= 0.0 || self.light_color[3] == 0 {
            return None;
        }
        let c = self.end_fade(self.light_color, self.end_light_color, age);
        Some(([c[0], c[1], c[2]], self.light_radius))
    }

    /// The core's half-width at the head for `fade`: LaserWidth·(0.9·fade +
    /// 0.1) (B1:202316).
    pub fn half_width(&self, fade: f32) -> f32 {
        self.width * (fade * 0.9 + 0.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The beam setters (B1 `FUN_00275bd0`, B2 `FUN_00312838`): `LaserWidth`
    /// sets the glow to 2.5 x; BF2 reads `BeamColor`, `BeamGlowTexture`, BF1
    /// neither, and its glow adds. The bullet keys are not a beam's.
    #[test]
    fn a_beam_replays_its_own_setter() {
        let props = [
            (HASH_LASER_WIDTH, "0.4"),
            (HASH_LASER_TEXTURE, "com_sfx_laser_blue"),
            (HASH_LASER_GLOW_COLOR, "92 136 250 100"),
            (HASH_BEAM_COLOR, "200 210 220 230"),
            (HASH_BEAM_GLOW_TEXTURE, "glowy"),
            (HASH_LIGHT_COLOR, "253 43 43 150"),
            (HASH_LIGHT_RADIUS, "4"),
        ];
        let bf2 = LaserLook::replay_beam(props, true);
        let b = bf2.beam.clone().unwrap();
        assert_eq!(bf2.width, 0.4);
        assert_eq!(bf2.texture.as_deref(), Some("com_sfx_laser_blue"));
        assert_eq!(bf2.glow_color, [92, 136, 250, 100]);
        assert_eq!(b.color, [200, 210, 220, 230]);
        assert!((b.glow_width - 1.0).abs() < 1e-6);
        assert_eq!(b.glow_texture.as_deref(), Some("glowy"));
        assert!(!b.glow_additive);
        assert_eq!(bf2.light(0.0), None);
        let bf1 = LaserLook::replay_beam(props, false).beam.unwrap();
        assert_eq!((bf1.color, bf1.glow_texture), ([255; 4], None));
        assert!(bf1.glow_additive);
        assert!((bf1.glow_width - 1.0).abs() < 1e-6);
    }

    /// re-bolt §1's order rule: LaserLength also resets GlowLength = 3L and
    /// BlurLength = 15L (clamped to 128), so a later GlowLength wins and an
    /// earlier one is overwritten. `pistol_ord` authors 0.75 ×3 then Glow 3,
    /// Blur 5; the BF2 remote droid only LaserLength 0.75 (RAM: 2.25 / 11.25).
    #[test]
    fn laser_length_resets_glow_and_blur_in_prop_order() {
        let pistol = LaserLook::replay([
            (HASH_LASER_LENGTH, "0.75"),
            (HASH_GLOW_LENGTH, "0.75"),
            (HASH_BLUR_LENGTH, "0.75"),
            (HASH_GLOW_LENGTH, "3"),
            (HASH_BLUR_LENGTH, "5"),
        ]);
        assert_eq!(
            (pistol.length, pistol.glow_length, pistol.blur_length),
            (0.75, 3.0, 5.0)
        );
        let droid = LaserLook::replay([(HASH_GLOW_LENGTH, "9"), (HASH_LASER_LENGTH, "0.75")]);
        assert_eq!(
            (droid.length, droid.glow_length, droid.blur_length),
            (0.75, 2.25, 11.25)
        );
        let long = LaserLook::replay([(HASH_LASER_LENGTH, "20")]);
        assert_eq!(
            (long.length, long.glow_length, long.blur_length),
            (20.0, 60.0, 128.0)
        );
        assert_eq!(long.cap(), 128.0);
        let big = LaserLook::replay([(HASH_LASER_LENGTH, "200"), (HASH_GLOW_LENGTH, "300")]);
        assert_eq!((big.length, big.glow_length), (128.0, 128.0));
        let snipe = LaserLook::replay([(HASH_FADE_OUT_TIME, "1.0"), (0x1234_5678, "x")]);
        assert_eq!(snipe.fade_out_rate, 1.0);
        assert_eq!(LaserLook::default().glow_color, [255; 4]);
    }

    /// re-bolt §3.5: a BF2 Republic rifle glow keeps its blue until 0.4 s,
    /// is halfway to the red End at 0.5 s (rounded per byte) and red past
    /// 0.6 s; × fade (truncated) when fading; the width rule 0.9·fade + 0.1.
    #[test]
    fn glow_colour_fades_to_the_end_colour_and_with_the_fade() {
        let rep = crate::weapon::WeaponClass::bf2_rifle()
            .ordnance
            .laser
            .unwrap();
        assert_eq!(rep.glow_bytes(0.3, 1.0), [92, 136, 250, 100]);
        assert_eq!(rep.glow_bytes(0.5, 1.0), [126, 90, 147, 100]);
        assert_eq!(rep.glow_bytes(0.7, 1.0), [160, 43, 43, 100]);
        assert_eq!(rep.glow_bytes(0.3, 0.5), [46, 68, 125, 50]);
        let bf1 = crate::weapon::WeaponClass::bf1_rifle()
            .ordnance
            .laser
            .unwrap();
        assert_eq!(bf1.glow_bytes(5.0, 1.0), [253, 43, 43, 100], "no End fade");
        assert!((bf1.half_width(1.0) - 0.1).abs() < 1e-7);
        assert!((bf1.half_width(0.0) - 0.01).abs() < 1e-7);
    }

    /// re-bolt §1/§2: "253 43 43 150" → (148, 25, 25) (RAM); three values
    /// are taken as is; two leave it unset; no radius or no colour, no
    /// light. The BF2 rifle's light turns to its End colour (94, 25, 25)
    /// with the glow.
    #[test]
    fn light_colour_setter_and_gate() {
        let mut l = LaserLook::replay([(HASH_LIGHT_COLOR, "253 43 43 150")]);
        assert_eq!(l.light_color, [148, 25, 25, 255]);
        assert_eq!(l.light(0.0), None, "no radius");
        l.set(HASH_LIGHT_RADIUS, "4.0");
        assert_eq!(l.light(0.0), Some(([148, 25, 25], 4.0)));
        l.set(HASH_LIGHT_COLOR, "10 20");
        assert_eq!(l.light_color, [148, 25, 25, 255], "two values: unchanged");
        l.set(HASH_LIGHT_COLOR, "10 20 30");
        assert_eq!(l.light_color, [10, 20, 30, 255]);
        l.set(HASH_LIGHT_COLOR, "10 20 30.5 99");
        assert_eq!(l.light_color, [10, 20, 30, 255], "the scan stops in 30.5");
        let cannon = LaserLook::replay([(HASH_LIGHT_RADIUS, "5")]);
        assert_eq!(cannon.light(0.0), None, "a radius and no colour");
        let rep = crate::weapon::WeaponClass::bf2_rifle()
            .ordnance
            .laser
            .unwrap();
        assert_eq!(rep.light(0.3), Some(([36, 53, 98], 4.0)));
        assert_eq!(rep.light(0.7), Some(([94, 25, 25], 4.0)));
    }
}
