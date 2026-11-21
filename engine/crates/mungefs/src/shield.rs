//! `Shield` effect records (`fx__`): the energy shell a droideka raises, and
//! the DLC Force ball's (`fisto_bubble`, its ordnance's `TrailEffect`). BF2
//! reads them in `FUN_0045f8f8` (B2): `Radius` +0x2c, `Height` +0x30, key
//! 0xb28af6f4 +0x34 (no reader found), `TurnOnTime` +0x38 (the grow, read by
//! `FUN_00460030` while the shell is on), `TurnOffTime` +0x3c (the shrink),
//! `ScrollRate` +0x40, `Texture`, `Color` and `SpecularColor` (bytes, each
//! component clamped to 0..255), `Type` a shape flag (a name starting with
//! `f` is the fambaa's dome; anything else the camera-facing ball).
use crate::config::{parse_tree, ConfigNode};
use crate::{Chunk, UcfbError};

pub const SHIELD: u32 = 0x3375_19b0;

mod key {
    pub const TYPE: u32 = 0x5127_f14d;
    pub const TEXTURE: u32 = 0x3c64_68f4;
    pub const COLOR: u32 = 0x3d7e_6258;
    pub const SPECULAR_COLOR: u32 = 0x01e4_aaf7;
    pub const RADIUS: u32 = 0x0dba_4cb3;
    pub const HEIGHT: u32 = 0xd5bd_bb42;
    pub const TURN_ON_TIME: u32 = 0x08aa_b3aa;
    pub const TURN_OFF_TIME: u32 = 0xc8a9_e3c6;
    pub const SCROLL_RATE: u32 = 0xd8bb_237e;
    /// Parsed to +0x34; no reader found (`analysis/droideka-65.md`).
    pub const UNREAD: u32 = 0xb28a_f6f4;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShieldConfig {
    pub name: String,
    /// `Type`: "droideka" (the ball) or "fambaa" (the dome).
    pub shape: String,
    pub texture: Option<String>,
    pub color: [u8; 4],
    pub specular_color: [u8; 4],
    pub radius: f32,
    pub height: f32,
    /// Seconds the shell takes to grow to full size.
    pub turn_on_time: f32,
    /// Seconds it takes to shrink away.
    pub turn_off_time: f32,
    pub scroll_rate: f32,
    /// Key 0xb28af6f4, kept as authored.
    pub unread_b28af6f4: f32,
}

impl ShieldConfig {
    fn new(name: String) -> Self {
        // The constructor's defaults (B2 `FUN_0045f8f8`'s record init).
        Self {
            name,
            shape: String::new(),
            texture: None,
            color: [255; 4],
            specular_color: [255; 4],
            radius: 1.0,
            height: 1.0,
            turn_on_time: 1.0,
            turn_off_time: 1.0,
            scroll_rate: 1.0,
            unread_b28af6f4: 0.0,
        }
    }

    /// The chunk's `Shield` record, `None` for any other effect.
    pub fn parse(fx: &Chunk<'_>) -> Result<Option<ShieldConfig>, UcfbError> {
        let Some(node) = parse_tree(fx)?
            .into_iter()
            .find(|n| n.hash == SHIELD && n.string.is_some())
        else {
            return Ok(None);
        };
        Ok(Some(Self::from_node(&node)))
    }

    fn from_node(node: &ConfigNode) -> Self {
        let mut s = Self::new(node.string.clone().unwrap_or_default());
        for c in &node.children {
            let v = |i: usize| c.f32(i).filter(|v| v.is_finite()).unwrap_or(0.0);
            let byte = |i: usize| v(i).clamp(0.0, 255.0) as u8;
            let bytes = |old: [u8; 4]| {
                let n = c.values.len();
                if n < 3 {
                    return old;
                }
                [
                    byte(0),
                    byte(1),
                    byte(2),
                    if n > 3 { byte(3) } else { old[3] },
                ]
            };
            match c.hash {
                key::TYPE => s.shape = c.string.clone().unwrap_or_default(),
                key::TEXTURE => s.texture = c.string.clone(),
                key::COLOR => s.color = bytes(s.color),
                key::SPECULAR_COLOR => s.specular_color = bytes(s.specular_color),
                key::RADIUS => s.radius = v(0),
                key::HEIGHT => s.height = v(0),
                key::TURN_ON_TIME => s.turn_on_time = v(0),
                key::TURN_OFF_TIME => s.turn_off_time = v(0),
                key::SCROLL_RATE => s.scroll_rate = v(0),
                key::UNREAD => s.unread_b28af6f4 = v(0),
                _ => {}
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_chunk, fnv1a_lowercase};

    fn data(hash: u32, nums: &[f32], strs: &[&str]) -> Vec<u8> {
        let argc = (nums.len() + strs.len()) as u8;
        let mut blob = Vec::new();
        let mut slots: Vec<u32> = Vec::new();
        for s in strs {
            slots.push(4 * argc as u32 + blob.len() as u32);
            blob.extend(s.as_bytes());
            blob.push(0);
        }
        slots.extend(nums.iter().map(|n| n.to_bits()));
        let mut p = hash.to_le_bytes().to_vec();
        p.push(argc);
        for s in &slots {
            p.extend(s.to_le_bytes());
        }
        p.extend((blob.len() as u32).to_le_bytes());
        p.extend(blob);
        build_chunk(b"DATA", &p)
    }

    fn parse(top: u32, name: &str, records: Vec<Vec<u8>>) -> Option<ShieldConfig> {
        let mut body = build_chunk(b"NAME", &fnv1a_lowercase(name).to_le_bytes());
        body.extend(data(top, &[], &[name]));
        body.extend(build_chunk(b"SCOP", &records.concat()));
        let buf = build_chunk(b"fx__", &body);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        ShieldConfig::parse(&c).unwrap()
    }

    #[test]
    fn hash_preimages_are_confirmed() {
        for (name, want) in [
            ("Shield", SHIELD),
            ("Type", key::TYPE),
            ("Texture", key::TEXTURE),
            ("Color", key::COLOR),
            ("SpecularColor", key::SPECULAR_COLOR),
            ("Radius", key::RADIUS),
            ("Height", key::HEIGHT),
            ("TurnOnTime", key::TURN_ON_TIME),
            ("TurnOffTime", key::TURN_OFF_TIME),
            ("ScrollRate", key::SCROLL_RATE),
        ] {
            assert_eq!(fnv1a_lowercase(name), want, "{name}");
        }
    }

    /// The DLC's `fisto_bubble`, built from its authored field values.
    #[test]
    fn the_force_balls_shell_reads_its_authored_fields() {
        let s = parse(
            SHIELD,
            "fisto_bubble",
            vec![
                data(key::TYPE, &[], &["droideka"]),
                data(key::TEXTURE, &[], &["cis_inf_droideka_shield"]),
                data(key::COLOR, &[0.0, 192.0, 220.0, 255.0], &[]),
                data(key::SPECULAR_COLOR, &[140.0, 150.0, 100.0], &[]),
                data(key::UNREAD, &[7.5], &[]),
                data(key::RADIUS, &[0.75], &[]),
                data(key::HEIGHT, &[0.75], &[]),
                data(key::TURN_ON_TIME, &[2.0], &[]),
                data(key::TURN_OFF_TIME, &[0.15], &[]),
                data(key::SCROLL_RATE, &[1.0], &[]),
            ],
        )
        .unwrap();
        assert_eq!(s.name, "fisto_bubble");
        assert_eq!(s.shape, "droideka");
        assert_eq!(s.texture.as_deref(), Some("cis_inf_droideka_shield"));
        assert_eq!(s.color, [0, 192, 220, 255]);
        assert_eq!(s.specular_color, [140, 150, 100, 255]);
        assert_eq!((s.radius, s.height), (0.75, 0.75));
        assert_eq!((s.turn_on_time, s.turn_off_time), (2.0, 0.15));
        assert_eq!((s.scroll_rate, s.unread_b28af6f4), (1.0, 7.5));
    }

    #[test]
    fn other_effects_are_not_shields_and_colours_clamp() {
        assert_eq!(
            parse(fnv1a_lowercase("ParticleEmitter"), "spark", vec![]),
            None
        );
        let s = parse(
            SHIELD,
            "x",
            vec![data(key::COLOR, &[300.0, -4.0, f32::NAN], &[])],
        )
        .unwrap();
        assert_eq!(s.color, [255, 0, 0, 255]);
        assert_eq!(s.turn_on_time, 1.0, "the constructor's default");
    }
}
