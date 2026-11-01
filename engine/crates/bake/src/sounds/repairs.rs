//! Explicit PS2 data repair: TAN omits three female-voice aliases that TAT
//! and MYG author against byte-identical backing samples. No audio is replaced.

use super::*;

// (requested id, original backing id, rate, encoded bytes, decoded frames).
const FEMALE: [(u32, u32, u32, usize, u32); 3] = [
    (2_298_015_460, 2_911_796_988, 14_038, 3_760, 6_580),
    (2_314_793_079, 2_928_574_607, 14_047, 4_240, 7_420),
    (117_175_253, 2_945_352_226, 14_014, 10_320, 18_060),
];

pub(super) fn tantive_female_aliases(
    source: &Path,
    banks: &[mungefs::soundbank::SampleBank<'_>],
    props: &[SoundPropRon],
    samples: &mut Vec<(u32, String)>,
) -> Result<()> {
    if !source
        .file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("tan"))
    {
        return Ok(());
    }
    let Some(bank) = banks.iter().find(|b| b.name_hash == 0xd4cb_a316) else {
        return Ok(());
    };
    if bank.format == 2 {
        return Ok(());
    }
    let female = mungefs::fnv1a_lowercase("all_inf_com_chatter_wound_female");
    if !props.iter().any(|p| {
        p.name_hash == female
            && p.stream.is_none()
            && p.samples.iter().map(|s| s.0).eq(FEMALE.iter().map(|s| s.0))
    }) {
        return Ok(());
    }
    let mut added = Vec::new();
    for (alias, backing, rate, size, frames) in FEMALE {
        // A provided mapping remains authoritative, including a corrected disc.
        if samples.iter().any(|(id, _)| *id == alias) {
            continue;
        }
        let original = bank
            .samples
            .iter()
            .find(|s| s.id_hash == backing && s.alias_of.is_none())
            .ok_or_else(|| anyhow!("TAN alias repair: original backing {backing:08x} missing"))?;
        if (original.frequency, original.size, original.frames) != (rate, size, frames) {
            bail!("TAN alias repair: {backing:08x} does not match the original PS2 signature");
        }
        let path = samples
            .iter()
            .find(|(id, _)| *id == backing)
            .map(|(_, path)| path.clone())
            .ok_or_else(|| anyhow!("TAN alias repair: backing {backing:08x} was not baked"))?;
        added.push((alias, path));
    }
    if !added.is_empty() {
        eprintln!(
            "sounds: applied PS2 TAN female alias repair ({} mappings, original PCM retained)",
            added.len()
        );
        samples.extend(added);
    }
    Ok(())
}
