//! BF2 side `comb` chunks: checked names and lossless config scopes.
//! `NAME` is an FNV hash; DATA/SCOP is the shared compiled-config format.

use anyhow::{anyhow, bail, Context, Result};
use manifest::combo::{ComboDef, ComboField, ComboKeyword as K, ComboSet, ComboValue};
use mungefs::{config::ConfigNode, Chunk};
use std::path::Path;

mod compatibility;
mod loads;
mod scope;
mod validate;

macro_rules! keywords {
    ($($key:ident),* $(,)?) => {
        const KEYWORDS: &[(&str, K)] = &[$((stringify!($key), K::$key)),*];
    };
}

keywords![
    RemapReloadButton,
    Animation,
    State,
    ThrowAnimation,
    OffhandAnimation,
    Loop,
    AimType,
    SyncType,
    BlendInTime,
    BlendOutTime,
    BlendTimeIn,
    BlendTimeOut,
    BlendTimeTo,
    LowResPose,
    RestartAnimation,
    PlayExplosion,
    MustShowOneFrame,
    Duration,
    Posture,
    Sound,
    InputLock,
    AlignedToThrust,
    TurnOffLightsaber,
    TurnFactor,
    StrafeFactor,
    ThrustFactor,
    EnergyRestoreRate,
    AnimatedMove,
    VelocityX,
    VelocityY,
    VelocityZ,
    VelocityXZ,
    VelocityXZAngle,
    VelocityYAbs,
    VelocityFromThrust,
    VelocityFromStrafe,
    Gravity,
    GravityVelocityTarget,
    Until,
    Attack,
    DamageTime,
    DamageLength,
    DamageWidth,
    Damage,
    DamageDistance,
    Push,
    Edge,
    AttackId,
    Deflect,
    DeflectAngle,
    EnergyCost,
    DeflectAnimation,
    Transition,
    If,
    Or,
    Break,
    TimeStart,
    TimeEnd,
    Button,
    ButtonsPressed,
    ButtonsReleased,
    Energy,
    Thrust,
    ThrustAngle,
    TimeInPosture,
];

fn field(node: ConfigNode) -> Result<ComboField> {
    let keyword = KEYWORDS
        .iter()
        .find(|(name, _)| mungefs::fnv1a_lowercase(name) == node.hash)
        .map(|(_, key)| *key)
        // The community DLC spells two block-animation blend fields
        // "BendTimeTo". Normalize that specific spelling; all other unknown
        // hashes still fail rather than silently discarding combo fields.
        .or_else(|| (node.hash == mungefs::fnv1a_lowercase("BendTimeTo")).then_some(K::BlendTimeTo))
        .ok_or_else(|| anyhow!("unknown combo keyword 0x{:08x}", node.hash))?;
    let args = (0..node.values.len())
        .map(|i| match node.str_arg(i) {
            Some(s) => ComboValue::Text(s.into()),
            None => ComboValue::Number(f32::from_bits(node.values[i])),
        })
        .collect();
    let children = node
        .children
        .into_iter()
        .map(field)
        .collect::<Result<_>>()?;
    let field = ComboField {
        keyword,
        args,
        scoped: false,
        children,
    };
    validate::arguments(&field)?;
    Ok(field)
}

// The shared decoder tolerates short records for old config readers. Combos
// must fail a damaged record rather than silently lose a condition or attack.
fn check_records(container: &Chunk<'_>, depth: usize) -> Result<()> {
    if depth > 64 {
        bail!("combo scope nested deeper than 64");
    }
    let mut previous_data = false;
    for child in container.children() {
        let child = child?;
        match &child.tag {
            b"NAME" if depth == 0 && child.payload.len() == 4 => {
                previous_data = false;
            }
            b"DATA" => {
                let p = child.payload;
                if p.len() < 9 {
                    bail!("short combo DATA");
                }
                let count = p[4] as usize;
                let at = 5 + count * 4;
                if at + 4 > p.len() {
                    bail!("short combo arguments");
                }
                let blob = u32::from_le_bytes(p[at..at + 4].try_into().unwrap()) as usize;
                if blob != p.len() - at - 4 {
                    bail!("combo string blob length disagrees with DATA");
                }
                let strings = &p[at + 4..];
                if !strings.is_empty() {
                    if strings.last() != Some(&0) || std::str::from_utf8(strings).is_err() {
                        bail!("combo string blob is not terminated UTF-8");
                    }
                    let base = count * 4;
                    for slot in p[5..at].chunks_exact(4) {
                        let value = u32::from_le_bytes(slot.try_into().unwrap()) as usize;
                        if let Some(offset) = value.checked_sub(base).filter(|&n| n < blob) {
                            if offset != 0 && strings[offset - 1] != 0 {
                                bail!("combo string argument points inside a string");
                            }
                        }
                    }
                }
                previous_data = true;
            }
            b"SCOP" => {
                if !previous_data {
                    bail!("combo SCOP has no preceding DATA");
                }
                check_records(&child, depth + 1)?;
                previous_data = false;
            }
            _ => bail!("unexpected combo chunk {:?}", child.tag),
        }
    }
    Ok(())
}

/// One checked definition; its hash is deliberately kept side-local.
pub fn parse_combo(chunk: &Chunk<'_>) -> Result<ComboDef> {
    if &chunk.tag != b"comb" {
        bail!("expected comb, got {:?}", chunk.tag);
    }
    check_records(chunk, 0)?;
    let names: Vec<_> = chunk
        .children()
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|c| &c.tag == b"NAME")
        .collect();
    if names.len() != 1 {
        bail!("combo needs exactly one NAME");
    }
    let name_hash = u32::from_le_bytes(names[0].payload.try_into().unwrap());
    let mut fields: Vec<ComboField> = mungefs::config::parse_tree(chunk)?
        .into_iter()
        .map(field)
        .collect::<Result<_>>()
        .with_context(|| format!("combo 0x{name_hash:08x}"))?;
    scope::mark(chunk, &mut fields)?;
    let combo = ComboDef { name_hash, fields };
    if combo.state("IDLE").is_none() {
        bail!("combo 0x{name_hash:08x}: missing IDLE state");
    }
    let mut states = std::collections::BTreeSet::new();
    for state in combo.states() {
        let name = state.text(0).context("State needs a string name")?;
        if !states.insert(name.to_ascii_lowercase()) {
            bail!("combo 0x{name_hash:08x}: duplicate State {name}");
        }
    }
    Ok(combo)
}

/// All side definitions, first-seen order. Identical sublevel copies fold.
pub fn combos_from_lvl(buf: &[u8]) -> Result<ComboSet> {
    let mut set = ComboSet {
        scope_provenance: true,
        ..ComboSet::default()
    };
    for chunk in mungefs::content_chunks(mungefs::open(buf)?)? {
        if &chunk.tag != b"comb" {
            continue;
        }
        let combo = parse_combo(&chunk)?;
        if let Some(old) = set.combos.iter().find(|c| c.name_hash == combo.name_hash) {
            if *old != combo {
                bail!(
                    "different definitions of combo 0x{:08x} in one LVL",
                    combo.name_hash
                );
            }
        } else {
            set.combos.push(combo);
        }
    }
    set.loads = loads::index(buf)?;
    set.load_provenance = true;
    Ok(set)
}

/// Gameplay output, with only documented shipped-data corrections. The raw
/// readers above retain the disc's authored records for evidence and audits.
pub fn baked_combos_from_lvl(buf: &[u8]) -> Result<ComboSet> {
    let mut set = combos_from_lvl(buf)?;
    for combo in &mut set.combos {
        compatibility::vader_terminal_target(combo);
        compatibility::vader_deflect_frame_flags(combo);
    }
    Ok(set)
}

pub fn bake_combos(lvl_path: &str, out_ron: &str) -> Result<ComboSet> {
    let buf = crate::gameplay::read(lvl_path).with_context(|| lvl_path.to_owned())?;
    let combos = baked_combos_from_lvl(&buf).with_context(|| lvl_path.to_owned())?;
    crate::write_ron(Path::new(out_ron), &combos)?;
    Ok(combos)
}

/// Animation references, including blend targets and both offhand channels.
pub fn animation_names(combo: &ComboDef) -> Vec<String> {
    fn walk(fields: &[ComboField], out: &mut std::collections::BTreeSet<String>) {
        for f in fields {
            if matches!(
                f.keyword,
                K::Animation
                    | K::ThrowAnimation
                    | K::OffhandAnimation
                    | K::DeflectAnimation
                    | K::BlendTimeTo
            ) {
                if let Some(s) = f.text(0) {
                    out.insert(s.to_ascii_lowercase());
                }
            }
            walk(&f.children, out);
        }
    }
    let mut out = std::collections::BTreeSet::new();
    walk(&combo.fields, &mut out);
    out.into_iter().collect()
}

/// Targets absent from the authored state table. Retained as disc evidence.
pub fn dangling_transitions(combo: &ComboDef) -> Vec<(String, String)> {
    combo
        .states()
        .flat_map(|state| {
            state.all(K::Transition).filter_map(|transition| {
                let target = transition.text(0)?;
                combo.state(target).is_none().then(|| {
                    (
                        state.text(0).unwrap_or_default().to_owned(),
                        target.to_owned(),
                    )
                })
            })
        })
        .collect()
}

/// One soldier model's authored combo and animation-family binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboBinding {
    pub family: String,
    pub bank: String,
    pub name_hash: u32,
}

/// Resolve GeometryName, animation family and each weapon's inherited
/// ComboAnimationBank from the same class chains the soldier bake uses.
pub fn model_bindings(classes: &[manifest::ClassDef], model: &str) -> Vec<ComboBinding> {
    use std::collections::HashMap;
    fn chain<'a>(
        by_kind: &HashMap<(&str, String), &'a manifest::ClassDef>,
        kind: &str,
        first: &'a manifest::ClassDef,
    ) -> Vec<&'a manifest::ClassDef> {
        let mut out = vec![first];
        while out.len() < 16 {
            let Some(parent) = by_kind.get(&(kind, out.last().unwrap().base.to_ascii_lowercase()))
            else {
                break;
            };
            if out.iter().any(|p| std::ptr::eq(*p, *parent)) {
                break;
            }
            out.push(parent);
        }
        out
    }
    let mut by_kind = HashMap::new();
    for c in classes {
        by_kind
            .entry((c.kind.as_str(), c.name.to_ascii_lowercase()))
            .or_insert(c);
    }
    let prop = |rows: &[&manifest::ClassDef], names: &[&str]| {
        rows.iter()
            .find_map(|c| {
                c.props.iter().rev().find_map(|(hash, v)| {
                    names
                        .iter()
                        .any(|n| mungefs::fnv1a_lowercase(n) == *hash)
                        .then_some(v.as_str())
                })
            })
            .map(str::to_owned)
    };
    let mut out = Vec::new();
    for soldier in classes.iter().filter(|c| c.kind == "entc") {
        let rows = chain(&by_kind, "entc", soldier);
        if !prop(&rows, &["GeometryName"]).is_some_and(|s| s.eq_ignore_ascii_case(model)) {
            continue;
        }
        let family = prop(&rows, &["AnimationName", "SkeletonName"])
            .unwrap_or_else(|| "human".into())
            .split_whitespace()
            .next()
            .unwrap_or("human")
            .to_ascii_lowercase();
        for weapon in crate::anim_sets::bf2::weapon_names(&rows) {
            let Some(first) = by_kind.get(&("wpnc", weapon.to_ascii_lowercase())) else {
                continue;
            };
            let weapon_rows = chain(&by_kind, "wpnc", first);
            let Some(value) = prop(&weapon_rows, &["ComboAnimationBank"]) else {
                continue;
            };
            let words: Vec<_> = value.split_whitespace().collect();
            if words.len() < 3 {
                continue;
            }
            let binding = ComboBinding {
                family: family.clone(),
                bank: words[0]
                    .split_once('_')
                    .map_or(words[0], |(_, b)| b)
                    .to_ascii_lowercase(),
                name_hash: mungefs::fnv1a_lowercase(words[2]),
            };
            if !out.contains(&binding) {
                out.push(binding);
            }
        }
    }
    out
}

/// A hero's own clip first, then the human parent. Upper and lower scopes
/// resolve independently; a full clip supplies both. No unrelated hero bank.
pub fn resolve_animation_clips(
    inv: &crate::anim_sets::Inventory,
    binding: &ComboBinding,
    combo: &ComboDef,
) -> (Vec<crate::anim_sets::ClipRef>, Vec<String>) {
    let mut clips = Vec::new();
    let mut missing = Vec::new();
    for anim in animation_names(combo) {
        let scoped = ["_full", "_upper", "_lower"]
            .iter()
            .any(|s| anim.ends_with(s));
        let mut found = false;
        for layer in ["upper", "lower"] {
            let mut picked = None;
            for family in [binding.family.as_str(), "human"] {
                let base = format!("{family}_{}_{}", binding.bank, anim);
                let candidates = if scoped {
                    let stem = base
                        .strip_suffix("_upper")
                        .or_else(|| base.strip_suffix("_lower"));
                    match stem {
                        Some(stem) => vec![base.clone(), format!("{stem}_full")],
                        None => vec![base],
                    }
                } else {
                    vec![
                        base.clone(),
                        format!("{base}_{layer}"),
                        format!("{base}_full"),
                    ]
                };
                picked = candidates.iter().find_map(|name| inv.find(name));
                if picked.is_some() {
                    break;
                }
            }
            if let Some(clip) = picked {
                found = true;
                if !clips
                    .iter()
                    .any(|c: &crate::anim_sets::ClipRef| c.name == clip.name)
                {
                    clips.push(clip);
                }
            }
        }
        if !found {
            missing.push(anim);
        }
    }
    (clips, missing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlc_block_blend_typo_has_one_explicit_alias() {
        let mut node = ConfigNode {
            hash: mungefs::fnv1a_lowercase("BendTimeTo"),
            values: vec![8, 0],
            strings: vec![Some("stand_block_front1".into()), None],
            string: Some("stand_block_front1".into()),
            children: vec![],
        };
        let parsed = field(node.clone()).unwrap();
        assert_eq!(parsed.keyword, K::BlendTimeTo);
        assert_eq!(parsed.number(1), Some(0.0));
        node.hash = mungefs::fnv1a_lowercase("OtherTimeTo");
        assert!(field(node).is_err());
    }

    fn chunk(tag: &[u8; 4], p: &[u8]) -> Vec<u8> {
        let mut out = tag.to_vec();
        out.extend((p.len() as u32).to_le_bytes());
        out.extend(p);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        out
    }

    fn data(key: &str, text: &str) -> Vec<u8> {
        let mut p = mungefs::fnv1a_lowercase(key).to_le_bytes().to_vec();
        p.push(1);
        p.extend(4u32.to_le_bytes());
        p.extend((text.len() as u32 + 1).to_le_bytes());
        p.extend(text.as_bytes());
        p.push(0);
        chunk(b"DATA", &p)
    }

    fn fixture(key: &str) -> Vec<u8> {
        let mut p = chunk(b"NAME", &7u32.to_le_bytes());
        p.extend(data("State", "IDLE"));
        p.extend(chunk(b"SCOP", &data(key, "stand_idle")));
        chunk(b"comb", &p)
    }

    #[test]
    fn unknown_keyword_mutation_is_rejected_at_any_depth() {
        let good = fixture("Animation");
        assert!(parse_combo(&mungefs::chunks(&good).next().unwrap().unwrap()).is_ok());
        let bad = fixture("AnimationTypo");
        let err = parse_combo(&mungefs::chunks(&bad).next().unwrap().unwrap()).unwrap_err();
        assert!(
            format!("{err:#}").contains("unknown combo keyword"),
            "{err:#}"
        );
    }

    #[test]
    fn comb_directly_under_lvl_remains_a_content_chunk() {
        let bytes = chunk(b"ucfb", &chunk(b"lvl_", &fixture("Animation")));
        let parsed = combos_from_lvl(&bytes).unwrap();
        assert_eq!(parsed.combos.len(), 1);
        assert_eq!(
            parsed.combos[0]
                .state("IDLE")
                .unwrap()
                .find(K::Animation)
                .unwrap()
                .text(0),
            Some("stand_idle")
        );
    }

    #[test]
    fn truncating_a_data_argument_fails() {
        let mut p = chunk(b"NAME", &7u32.to_le_bytes());
        p.extend(chunk(b"DATA", &[0, 0, 0, 0, 9, 0, 0, 0, 0]));
        let bytes = chunk(b"comb", &p);
        assert!(parse_combo(&mungefs::chunks(&bytes).next().unwrap().unwrap()).is_err());
    }

    fn rejected(bytes: &[u8]) -> bool {
        match mungefs::chunks(bytes).next() {
            Some(Ok(c)) => parse_combo(&c).is_err(),
            Some(Err(_)) | None => true,
        }
    }

    #[test]
    fn malformed_record_shapes_and_every_payload_truncation_fail() {
        let good = fixture("Animation");
        let payload_end = 8 + u32::from_le_bytes(good[4..8].try_into().unwrap()) as usize;
        for end in 0..payload_end {
            assert!(rejected(&good[..end]), "accepted truncation at {end}");
        }
        assert!(!rejected(&good));
        let name = chunk(b"NAME", &7u32.to_le_bytes());
        let idle = data("State", "IDLE");
        let scope = chunk(b"SCOP", &data("Animation", "stand_idle"));
        for payload in [
            [name.clone(), name.clone(), idle.clone()].concat(),
            [chunk(b"NAME", &[7, 0, 0]), idle.clone()].concat(),
            [name.clone(), scope.clone(), idle.clone()].concat(),
            [name.clone(), idle.clone(), scope.clone(), scope].concat(),
            [name.clone(), idle.clone(), chunk(b"JUNK", &[])].concat(),
        ] {
            assert!(rejected(&chunk(b"comb", &payload)));
        }
        // Count and string-blob lengths cannot make the decoder read past DATA.
        let mut bad = data("Animation", "stand_idle");
        bad[8 + 4] = 255;
        assert!(rejected(&chunk(
            b"comb",
            &[name.clone(), idle.clone(), bad].concat()
        )));
        let mut bad = data("Animation", "stand_idle");
        bad[8 + 9..8 + 13].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(rejected(&chunk(b"comb", &[name, idle, bad].concat())));
    }

    #[test]
    fn nested_scope_limit_rejects_a_depth_bomb() {
        let mut inner = data("Animation", "stand_idle");
        for _ in 0..65 {
            inner = [data("Animation", "stand_idle"), chunk(b"SCOP", &inner)].concat();
        }
        let payload = [
            chunk(b"NAME", &7u32.to_le_bytes()),
            data("State", "IDLE"),
            chunk(b"SCOP", &inner),
        ]
        .concat();
        let bytes = chunk(b"comb", &payload);
        let err = parse_combo(&mungefs::chunks(&bytes).next().unwrap().unwrap()).unwrap_err();
        assert!(err.to_string().contains("nested deeper than 64"), "{err:#}");
    }
}

#[cfg(test)]
mod hostile;
