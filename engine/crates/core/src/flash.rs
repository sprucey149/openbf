//! The muzzle flash (card FXMF, `analysis/re-muzzle-flash.md` [O]): what a
//! weapon class flashes with — its keys after the setter's rules — and the
//! two pure rules the draw applies, the fade (§4.3) and the matrix (§4.2).
//! Render-only: [`crate::weapon::WeaponClass::flash`] carries it and the sim
//! never reads it.
//!
//! Each round fired restarts the weapon's timer, end = now + `FlashLength`
//! (§3); while t = (end − now) / `FlashLength` > 0 the weapon's render draws
//! the flash MODEL (`muzzleflash` BF1, `com_sfx_muzzleflash` BF2) at the fire
//! point, +Z along the aim, scaled by `FlashRadius`, tinted `FlashColor` with
//! alpha min(A, 2A·t), additive, depth tested, no depth write, unlit.
//!
//! Never read by either ELF, so not held here (§6): `FlashWidth`,
//! `FlashLightColor`, `FlashLightRadius`, `FlashLightDuration` — the PS2's
//! muzzle flash makes no light (PCSX2 knock-ins agree), and
//! `MuzzleFlashEffect` is an `fx__` effect (card FXP3's).

use crate::fmath::FloatExt;
use glam::{Mat4, Vec3};

/// Flash keys (fnv over byte|0x20, re-muzzle-flash §1 [O]).
pub const HASH_MUZZLE_FLASH: u32 = 0xcfcc_cbce;
pub const HASH_FLASH_RADIUS: u32 = 0x86fd_c077;
pub const HASH_FLASH_LENGTH: u32 = 0xb7b2_d011;
pub const HASH_FLASH_COLOR: u32 = 0xeb42_30ac;
pub const HASH_MUZZLE_FLASH_MODEL: u32 = 0x1f63_537b;
pub const HASH_FIRE_POINT_NAME: u32 = 0x4274_fc96;

/// The `MuzzleFlash` presets (B1 210377–210408, B2 320841–320880):
/// (value hash, FlashRadius, FlashLength). Each also sets FlashColor
/// (255, 255, 50, 255); FlashWidth and the light keys it writes are unread.
/// Any other value does nothing.
const PRESETS: [(u32, f32, f32); 3] = [
    (0xae94_ded9, 1.8, 0.3),  // large_muzzle_flash
    (0x8467_4572, 1.0, 0.15), // med_muzzle_flash
    (0xb92c_f7a1, 0.6, 0.1),  // small_muzzle_flash
];
const PRESET_COLOR: [u8; 4] = [255, 255, 50, 255];

/// The model every weapon class's copy ctor attaches (re-muzzle-flash §1:
/// B1 `FUN_00285d70` 209648, B2 `FUN_003283d8` 319750).
pub const BF1_MODEL: &str = "muzzleflash";
pub const BF2_MODEL: &str = "com_sfx_muzzleflash";

/// The fire point's default node name (B1 CRC 0xb3ea0ee8, §4.1).
pub const DEFAULT_FIRE_POINT: &str = "hp_fire";

/// What a weapon class flashes with (re-muzzle-flash §1).
#[derive(Clone, Debug, PartialEq)]
pub struct MuzzleFlashLook {
    /// `FlashRadius`: the model's uniform scale. The label's −1 = no flash.
    pub radius: f32,
    /// `FlashLength`, seconds of game clock.
    pub length: f32,
    /// `FlashColor` bytes R, G, B, A.
    pub color: [u8; 4],
    /// The model drawn: the copy ctor's per game, or `MuzzleFlashModel`.
    /// `None` only at the label (no disc class stays there).
    pub model: Option<String>,
    /// `FirePointName`: the node of the weapon's `GeometryName` model the
    /// flash sits at (§4.1).
    pub fire_point: String,
}

impl Default for MuzzleFlashLook {
    /// The label ctor's defaults (B1 `FUN_00285a98` 209492–209522).
    fn default() -> Self {
        Self {
            radius: -1.0,
            length: 0.1,
            color: PRESET_COLOR,
            model: None,
            fire_point: DEFAULT_FIRE_POINT.to_owned(),
        }
    }
}

/// `"%i %i %i %i"` into the colour bytes, a missing value 0xff (the
/// setter's locals start at 0xff, B1 210451–210462). A token the scan stops
/// inside still counts its integer prefix and ends the scan.
fn colour_bytes(v: &str) -> [u8; 4] {
    let mut out = [0xff; 4];
    for (k, t) in v.split_whitespace().take(4).enumerate() {
        let digits = t
            .char_indices()
            .take_while(|&(i, c)| c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+')))
            .count();
        let Ok(n) = t[..digits].parse::<i64>() else {
            break;
        };
        out[k] = n as u8;
        if digits < t.len() {
            break;
        }
    }
    out
}

impl MuzzleFlashLook {
    /// The model a game's copy ctor attaches.
    pub fn default_model(bf2: bool) -> &'static str {
        if bf2 {
            BF2_MODEL
        } else {
            BF1_MODEL
        }
    }

    /// One PROP through the setter (B1 `FUN_002864f0`, B2 `FUN_00328fb0`);
    /// `false` for a key the look does not hold.
    pub fn set(&mut self, hash: u32, value: &str) -> bool {
        let f = || value.split_whitespace().next()?.parse::<f32>().ok();
        match hash {
            HASH_MUZZLE_FLASH => {
                let h = fnv_lower(value.trim());
                if let Some(&(_, r, l)) = PRESETS.iter().find(|p| p.0 == h) {
                    self.radius = r;
                    self.length = l;
                    self.color = PRESET_COLOR;
                }
            }
            HASH_FLASH_RADIUS => self.radius = f().unwrap_or(self.radius),
            HASH_FLASH_LENGTH => self.length = f().unwrap_or(self.length),
            HASH_FLASH_COLOR => self.color = colour_bytes(value),
            HASH_MUZZLE_FLASH_MODEL => self.model = Some(value.trim().to_ascii_lowercase()),
            HASH_FIRE_POINT_NAME => self.fire_point = value.trim().to_owned(),
            _ => return false,
        }
        true
    }

    /// Replay a class chain, PARENT FIRST, each class's PROPs in file order:
    /// every class the chain defines is a copy of its parent that attaches
    /// the game's model afresh (the copy ctor), then runs its own PROPs.
    pub fn replay<'a, C, P>(chain: C, bf2: bool) -> Self
    where
        C: IntoIterator<Item = P>,
        P: IntoIterator<Item = (u32, &'a str)>,
    {
        let mut l = Self::default();
        for class in chain {
            l.model = Some(Self::default_model(bf2).to_owned());
            for (h, v) in class {
                l.set(h, v);
            }
        }
        l
    }

    /// The weapon draws a flash at all: a radius and a model (§2).
    pub fn draws(&self) -> bool {
        self.radius > 0.0 && self.model.is_some()
    }

    /// t = (end − now) / FlashLength (§3), for a shot at `shot` seconds of
    /// game clock seen at `now`.
    pub fn t(&self, shot: f64, now: f64) -> f32 {
        if self.length <= 0.0 {
            return 0.0;
        }
        ((shot + f64::from(self.length) - now) / f64::from(self.length)) as f32
    }

    /// The colour the draw passes at `t` (§4.3): FlashColor's RGB with alpha
    /// (int)min(A, 2A·t) — full for the first half, then a linear fade.
    /// `None` when nothing is drawn: t ≤ 0, or an alpha of 0 (RedModel::Draw
    /// skips it, B1 557270).
    pub fn color_at(&self, t: f32) -> Option<[u8; 4]> {
        if t <= 0.0 {
            return None;
        }
        let a = f32::from(self.color[3]);
        let alpha = a.fmin(2.0 * a * t) as u8;
        (alpha > 0).then_some([self.color[0], self.color[1], self.color[2], alpha])
    }
}

/// The flash's world matrix (§4.2, B1 `FUN_00287ba8`, B2 `FUN_0032b0e0`):
/// with d the aim and ŷ = (0, 1, 0) the model's axes are
/// X = (d×ŷ)·R, Y = (d×(d×ŷ))·R, Z = d·R at `origin`. The cross products are
/// NOT normalised — the PS2's cross-section shrinks by cos(pitch) of the
/// aim, and a flash aimed straight up vanishes.
pub fn matrix(d: Vec3, radius: f32, origin: Vec3) -> Mat4 {
    let x = d.cross(Vec3::Y);
    let y = d.cross(x);
    Mat4::from_cols(
        (x * radius).extend(0.0),
        (y * radius).extend(0.0),
        (d * radius).extend(0.0),
        origin.extend(1.0),
    )
}

/// FNV-1a over byte|0x20 (the PS2's class/key hash).
pub(crate) fn fnv_lower(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5, |h, b| {
        (h ^ u32::from(b | 0x20)).wrapping_mul(0x0100_0193)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_hashes_are_the_names() {
        for (k, h) in [
            ("MuzzleFlash", HASH_MUZZLE_FLASH),
            ("FlashRadius", HASH_FLASH_RADIUS),
            ("FlashLength", HASH_FLASH_LENGTH),
            ("FlashColor", HASH_FLASH_COLOR),
            ("MuzzleFlashModel", HASH_MUZZLE_FLASH_MODEL),
            ("FirePointName", HASH_FIRE_POINT_NAME),
            ("large_muzzle_flash", PRESETS[0].0),
            ("med_muzzle_flash", PRESETS[1].0),
            ("small_muzzle_flash", PRESETS[2].0),
        ] {
            assert_eq!(fnv_lower(k), h, "{k}");
        }
    }

    /// The label has no flash; a disc class is a copy that attaches the
    /// game's model; the preset sets radius, length and colour; later keys
    /// override it (BF1 `rep_weap_inf_rifle`'s flattened ODF, §2).
    #[test]
    fn replay_reproduces_the_census_rows() {
        assert!(!MuzzleFlashLook::default().draws());
        let rep = MuzzleFlashLook::replay(
            [vec![
                (HASH_MUZZLE_FLASH, "small_muzzle_flash"),
                (HASH_FLASH_COLOR, "80 80 255 255"),
                (HASH_FLASH_LENGTH, "0.025"),
            ]],
            false,
        );
        assert_eq!(
            (rep.radius, rep.length, rep.color, rep.model.as_deref()),
            (0.6, 0.025, [80, 80, 255, 255], Some(BF1_MODEL))
        );
        assert!(rep.draws());
        // BF2: the child inherits its parent's keys through the copy.
        let com: Vec<(u32, &str)> = vec![
            (HASH_MUZZLE_FLASH, "small_muzzle_flash"),
            (HASH_FLASH_COLOR, "255 100 100"),
            (HASH_FLASH_LENGTH, "0.025"),
        ];
        let child = MuzzleFlashLook::replay([com, vec![]], true);
        assert_eq!(
            (child.radius, child.color, child.model.as_deref()),
            (0.6, [255, 100, 100, 255], Some(BF2_MODEL))
        );
        // An unknown preset (`com_sfx_muzzleflash_lg`) is a no-op.
        let lg = MuzzleFlashLook::replay([[(HASH_MUZZLE_FLASH, "com_sfx_muzzleflash_lg")]], true);
        assert_eq!(lg.radius, -1.0);
        assert!(!lg.draws());
        let med = MuzzleFlashLook::replay([[(HASH_MUZZLE_FLASH, "med_muzzle_flash")]], false);
        assert_eq!((med.radius, med.length), (1.0, 0.15));
    }

    /// §4.3: alpha = (int)min(A, 2A·t); t 1 → A, 0.5 → A, 0.25 → A/2.
    #[test]
    fn the_fade_is_full_for_half_the_length() {
        let l = MuzzleFlashLook {
            color: [80, 80, 255, 255],
            length: 0.025,
            radius: 0.6,
            ..MuzzleFlashLook::default()
        };
        assert_eq!(l.color_at(1.0), Some([80, 80, 255, 255]));
        assert_eq!(l.color_at(0.5).unwrap()[3], 255);
        assert_eq!(l.color_at(0.25).unwrap()[3], 127);
        assert_eq!(l.color_at(0.0), None);
        assert_eq!(l.color_at(-0.33), None);
        // The shot's own tick: t = 1; the next 30 Hz tick is past the end.
        assert_eq!(l.t(10.0, 10.0), 1.0);
        assert!(l.t(10.0, 10.0 + f64::from(crate::TICK_DT)) < 0.0);
    }

    /// §4.2: d = (0, 0, 1) → rows (−R, 0, 0), (0, −R, 0), (0, 0, R).
    #[test]
    fn the_matrix_runs_z_along_the_aim_unnormalised() {
        let m = matrix(Vec3::Z, 2.0, Vec3::new(1.0, 2.0, 3.0));
        assert!(m
            .x_axis
            .truncate()
            .abs_diff_eq(Vec3::new(-2.0, 0.0, 0.0), 1e-6));
        assert!(m
            .y_axis
            .truncate()
            .abs_diff_eq(Vec3::new(0.0, -2.0, 0.0), 1e-6));
        assert!(m
            .z_axis
            .truncate()
            .abs_diff_eq(Vec3::new(0.0, 0.0, 2.0), 1e-6));
        assert_eq!(m.w_axis.truncate(), Vec3::new(1.0, 2.0, 3.0));
        // 45° of pitch: the cross-section is cos(45°) as wide.
        let d = Vec3::new(0.0, 1.0, 1.0).normalize();
        let m = matrix(d, 1.0, Vec3::ZERO);
        assert!((m.x_axis.truncate().length() - 0.5f32.sqrt()).abs() < 1e-6);
        assert!((m.y_axis.truncate().length() - 0.5f32.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn colours_scan_like_the_setter() {
        assert_eq!(colour_bytes("255 100 100"), [255, 100, 100, 255]);
        assert_eq!(colour_bytes("80 80 255 255"), [80, 80, 255, 255]);
        assert_eq!(colour_bytes("255.0 7 7"), [255, 255, 255, 255]);
        assert_eq!(colour_bytes(""), [255; 4]);
    }
}
