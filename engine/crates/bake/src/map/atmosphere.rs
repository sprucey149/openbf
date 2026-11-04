use super::Content;
use anyhow::Result;
use mungefs::Chunk;

pub(super) fn sun_flare(
    chunks: &[Chunk<'_>],
    kind: Content,
) -> Result<Option<manifest::ManifestSunFlare>> {
    Ok(if kind == Content::Map {
        chunks
            .iter()
            .filter(|c| &c.tag == b"fx__")
            .map(mungefs::sun_flare::SunFlare::parse)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .next()
            .map(|s| manifest::ManifestSunFlare {
                angle: s.angle,
                color: s.color,
                size: s.size,
                flare_out_size: s.flare_out_size,
                num_flare_outs: s.num_flare_outs,
                initial_alpha: s.initial_alpha,
                halo_rings: s.halo_rings,
                spike_color: s.spike_color,
                spike_size: s.spike_size,
            })
    } else {
        None
    })
}

pub(super) fn dust_textures(chunks: &[Chunk<'_>]) -> std::collections::HashSet<String> {
    chunks
        .iter()
        .filter(|c| &c.tag == b"entc")
        .filter_map(|c| mungefs::entity::EntityClass::parse(c).ok())
        .filter(|c| c.base.eq_ignore_ascii_case("dusteffect"))
        .flat_map(|c| {
            c.props
                .into_iter()
                .filter(|p| p.key_hash == mungefs::fnv1a_lowercase("Texture"))
        })
        .map(|p| p.value.to_ascii_lowercase())
        .collect()
}
