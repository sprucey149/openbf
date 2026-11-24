//! LightBeam config trees for prop halos (`analysis/re-halo.md`, unaudited).
use crate::config::{parse_tree, ConfigNode};
use crate::{Chunk, UcfbError};

pub const LIGHT_BEAM: u32 = 0xe865_1328;

mod key {
    pub const COLOR: u32 = 0x3d7e_6258;
    pub const FLARE_INTENSITY: u32 = 0x633e_d9d4;
    pub const BEAM_INTENSITY: u32 = 0x7509_6fab;
    pub const INITIAL_WIDTH: u32 = 0x1414_bf95;
    pub const FINAL_WIDTH: u32 = 0xaa67_1039;
    pub const FADE_LENGTH: u32 = 0x20c3_51fb;
    pub const FADE_FACTOR: u32 = 0xa939_a4bc;
    pub const DRAW_DISTANCE: u32 = 0x1612_c34c;
    pub const LENGTH: u32 = 0x83d0_3615;
    pub const POSITION: u32 = 0x934f_4e0a;
    pub const DIRECTION: u32 = 0xdf6d_c76a;
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightBeamConfig {
    pub name: String,
    pub color: Option<[u8; 4]>,
    pub flare_intensity: f32,
    pub beam_intensity: f32,
    pub length: f32,
    pub initial_radius: [f32; 2],
    pub final_radius: [f32; 2],
    pub fade_length: f32,
    pub fade_factor: f32,
    pub draw_distance: f32,
    pub position: [f32; 3],
    pub direction: [f32; 3],
}

impl LightBeamConfig {
    fn new(name: String) -> Self {
        Self {
            name,
            color: None,
            flare_intensity: 1.0,
            beam_intensity: 1.0,
            length: 10.0,
            initial_radius: [1.0, 1.0],
            final_radius: [1.0, 1.0],
            fade_length: 1.0,
            fade_factor: 0.2,
            draw_distance: 0.0,
            position: [0.0; 3],
            direction: [0.0, 1.0, 0.0],
        }
    }

    pub fn parse(fx: &Chunk<'_>) -> Result<Option<LightBeamConfig>, UcfbError> {
        let Some(node) = parse_tree(fx)?
            .into_iter()
            .find(|n| n.hash == LIGHT_BEAM && n.string.is_some())
        else {
            return Ok(None);
        };
        Ok(Some(Self::from_node(&node)))
    }

    fn from_node(node: &ConfigNode) -> Self {
        let mut b = Self::new(node.string.clone().unwrap_or_default());
        let mut placed = false;
        for c in &node.children {
            let v = |i: usize| c.f32(i).filter(|v| v.is_finite());
            let n = c.values.len();
            match c.hash {
                key::COLOR => {
                    let byte = |i: usize| {
                        let x = v(i).unwrap_or(0.0);
                        if x < 0.0 {
                            0
                        } else if x >= 255.0 {
                            255
                        } else {
                            x as u8
                        }
                    };
                    let alpha = if n < 4 { 255 } else { byte(3) };
                    b.color = Some([byte(0), byte(1), byte(2), alpha]);
                }
                key::FLARE_INTENSITY => b.flare_intensity = v(0).unwrap_or(0.0),
                key::BEAM_INTENSITY => b.beam_intensity = v(0).unwrap_or(0.0).max(0.0),
                key::INITIAL_WIDTH => {
                    let a = v(0).unwrap_or(0.0);
                    b.initial_radius = if n < 2 {
                        [a * 0.5; 2]
                    } else {
                        [a * 0.5, v(1).unwrap_or(0.0) * 0.5]
                    };
                }
                key::FINAL_WIDTH => {
                    let a = v(0).unwrap_or(0.0);
                    b.final_radius = if n < 2 {
                        if a > 0.0 {
                            [a * 0.5; 2]
                        } else {
                            [0.0; 2]
                        }
                    } else {
                        let z = v(1).unwrap_or(0.0);
                        if a > 0.0 && z > 0.0 {
                            [a * 0.5, z * 0.5]
                        } else {
                            [0.0; 2]
                        }
                    };
                }
                key::FADE_LENGTH => b.fade_length = v(0).unwrap_or(0.0),
                key::FADE_FACTOR => b.fade_factor = v(0).unwrap_or(0.0),
                key::DRAW_DISTANCE => b.draw_distance = v(0).unwrap_or(0.0).abs(),
                key::LENGTH => {
                    let l = v(0).unwrap_or(0.0);
                    b.length = l.max(0.0);
                    placed |= l > 0.0;
                }
                key::POSITION => b.position = std::array::from_fn(|i| v(i).unwrap_or(0.0)),
                key::DIRECTION => {
                    b.direction = std::array::from_fn(|i| v(i).unwrap_or(0.0));
                    placed = true;
                }
                _ => {}
            }
        }
        if !placed {
            b.length = 0.0;
            b.direction = [0.0, 1.0, 0.0];
        }
        b
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

    fn parse(name: &str, records: Vec<Vec<u8>>) -> Option<LightBeamConfig> {
        let mut body = data(LIGHT_BEAM, &[], &[name]);
        body.extend(build_chunk(b"SCOP", &records.concat()));
        let buf = build_chunk(b"fx__", &body);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        LightBeamConfig::parse(&c).unwrap()
    }

    #[test]
    fn hash_preimages_are_confirmed() {
        for (name, want) in [
            ("LightBeam", LIGHT_BEAM),
            ("Color", key::COLOR),
            ("FlareIntensity", key::FLARE_INTENSITY),
            ("BeamIntensity", key::BEAM_INTENSITY),
            ("InitialWidth", key::INITIAL_WIDTH),
            ("FinalWidth", key::FINAL_WIDTH),
            ("FadeLength", key::FADE_LENGTH),
            ("FadeFactor", key::FADE_FACTOR),
            ("DrawDistance", key::DRAW_DISTANCE),
            ("Length", key::LENGTH),
            ("Position", key::POSITION),
            ("Direction", key::DIRECTION),
        ] {
            assert_eq!(fnv1a_lowercase(name), want, "{name}");
        }
    }

    #[test]
    fn mustafars_blue_halo_is_an_elliptic_beam() {
        let b = parse(
            "Blue_halo",
            vec![
                data(key::COLOR, &[50.0, 177.0, 219.0], &[]),
                data(key::LENGTH, &[1.0], &[]),
                data(key::INITIAL_WIDTH, &[0.8, 0.2], &[]),
                data(key::FINAL_WIDTH, &[1.3, 0.8], &[]),
                data(key::FADE_FACTOR, &[0.3], &[]),
                data(key::FADE_LENGTH, &[0.8], &[]),
                data(key::FLARE_INTENSITY, &[0.0], &[]),
                data(fnv1a_lowercase("BeamIntensity"), &[0.5], &[]),
                data(key::DRAW_DISTANCE, &[300.0], &[]),
            ],
        )
        .unwrap();
        assert_eq!(b.name, "Blue_halo");
        assert_eq!(b.color, Some([50, 177, 219, 255]));
        assert_eq!(b.length, 1.0);
        assert_eq!(b.initial_radius, [0.4, 0.1]);
        assert_eq!(b.final_radius, [0.65, 0.4]);
        assert_eq!((b.fade_factor, b.fade_length), (0.3, 0.8));
        assert_eq!(
            (b.flare_intensity, b.beam_intensity, b.draw_distance),
            (0.0, 0.5, 300.0)
        );
        assert_eq!(b.direction, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn a_zero_length_halo_keeps_the_defaults_it_does_not_author() {
        let b = parse(
            "dea1_halo_red",
            vec![
                data(key::COLOR, &[255.0, 90.0, 90.0, 300.0], &[]),
                data(key::LENGTH, &[0.0], &[]),
                data(key::INITIAL_WIDTH, &[1.0], &[]),
                data(key::FINAL_WIDTH, &[0.2], &[]),
                data(key::FLARE_INTENSITY, &[0.5], &[]),
            ],
        )
        .unwrap();
        assert_eq!(b.color, Some([255, 90, 90, 255]));
        assert_eq!(b.length, 0.0);
        assert_eq!(b.initial_radius, [0.5, 0.5]);
        assert_eq!(b.final_radius, [0.1, 0.1]);
        assert_eq!(b.beam_intensity, 1.0);
        assert_eq!(
            (b.fade_factor, b.fade_length, b.draw_distance),
            (0.2, 1.0, 0.0)
        );
        let clamped = parse(
            "clamped_halo",
            vec![
                data(key::COLOR, &[255.0, 255.0, 255.0, 255.0], &[]),
                data(key::BEAM_INTENSITY, &[-0.25], &[]),
            ],
        )
        .unwrap();
        assert_eq!(clamped.beam_intensity, 0.0);
    }

    #[test]
    fn an_unplaced_beam_collapses_to_a_glow_and_a_direction_keeps_the_default_length() {
        let none = parse("x", vec![data(key::COLOR, &[1.0, 2.0, 3.0], &[])]).unwrap();
        assert_eq!(none.length, 0.0);
        let aimed = parse("x", vec![data(key::DIRECTION, &[1.0, 0.0, 0.0], &[])]).unwrap();
        assert_eq!((aimed.length, aimed.direction), (10.0, [1.0, 0.0, 0.0]));
        let bad = parse("x", vec![data(key::FINAL_WIDTH, &[2.0, -1.0], &[])]).unwrap();
        assert_eq!(bad.final_radius, [0.0, 0.0]);
    }

    #[test]
    fn nonfinite_vector_components_use_the_numeric_fallback() {
        let b = parse(
            "fixture",
            vec![
                data(key::POSITION, &[f32::NAN, f32::INFINITY, 2.0], &[]),
                data(key::DIRECTION, &[1.0, f32::NEG_INFINITY, 0.0], &[]),
            ],
        )
        .unwrap();
        assert_eq!(b.position, [0.0, 0.0, 2.0]);
        assert_eq!(b.direction, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_particle_effect_is_not_a_beam() {
        let mut body = data(fnv1a_lowercase("ParticleEmitter"), &[], &["spark"]);
        body.extend(build_chunk(b"SCOP", &data(key::COLOR, &[1.0], &[])));
        let buf = build_chunk(b"fx__", &body);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        assert_eq!(LightBeamConfig::parse(&c).unwrap(), None);
    }
}
