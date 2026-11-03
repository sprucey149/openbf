use crate::{config::parse_tree, fnv1a_lowercase, Chunk, UcfbError};

#[derive(Debug, Clone, PartialEq)]
pub struct SunFlare {
    pub angle: [f32; 2],
    pub color: [f32; 4],
    pub size: f32,
    pub flare_out_size: f32,
    pub num_flare_outs: u32,
    pub initial_alpha: f32,
    pub halo_rings: [[f32; 5]; 3],
    pub spike_color: [f32; 4],
    pub spike_size: f32,
}

impl SunFlare {
    pub fn parse(chunk: &Chunk<'_>) -> Result<Option<Self>, UcfbError> {
        let nodes = parse_tree(chunk)?;
        let Some(node) = nodes.iter().find(|n| n.hash == fnv1a_lowercase("SunFlare")) else {
            return Ok(None);
        };
        let mut out = Self {
            angle: [0.0; 2],
            color: [255.0; 4],
            size: 5.0,
            flare_out_size: 20.0,
            num_flare_outs: 40,
            initial_alpha: 70.0,
            halo_rings: [
                [0.0, 255.0, 255.0, 0.0, 255.0],
                [5.0, 255.0, 255.0, 0.0, 128.0],
                [40.0, 255.0, 255.0, 0.0, 0.0],
            ],
            spike_color: [255.0, 255.0, 0.0, 128.0],
            spike_size: 20.0,
        };
        for c in &node.children {
            let key = c.hash;
            if key == fnv1a_lowercase("Angle") {
                out.angle = c.floats_or_zero();
            } else if key == fnv1a_lowercase("Color") {
                for (i, v) in out.color.iter_mut().enumerate() {
                    *v = c.f32(i).unwrap_or(255.0);
                }
            } else if key == fnv1a_lowercase("Size") {
                out.size = c.f32(0).unwrap_or(out.size);
            } else if key == fnv1a_lowercase("FlareOutSize") {
                out.flare_out_size = c.f32(0).unwrap_or(out.flare_out_size);
            } else if key == fnv1a_lowercase("NumFlareOuts") {
                out.num_flare_outs = c.f32(0).unwrap_or(0.0).clamp(0.0, 256.0) as u32;
            } else if key == fnv1a_lowercase("InitialFlareOutAlpha") {
                out.initial_alpha = c.f32(0).unwrap_or(out.initial_alpha);
            } else if key == fnv1a_lowercase("SpikeColor") {
                out.spike_color = c.floats_or_zero();
            } else if key == fnv1a_lowercase("SpikeSize") {
                out.spike_size = c.f32(0).unwrap_or(out.spike_size);
            } else {
                for (i, name) in ["HaloInnerRing", "HaloMiddleRing", "HaloOutterRing"]
                    .iter()
                    .enumerate()
                {
                    if key == fnv1a_lowercase(name) {
                        out.halo_rings[i] = c.floats_or_zero();
                    }
                }
            }
        }
        Ok(Some(out))
    }
}
