//! Retain SCOP presence after the shared value decoder. This is combo-local;
//! other compiled config consumers keep their existing representation.
use super::*;

pub(super) fn mark(container: &Chunk<'_>, fields: &mut [ComboField]) -> Result<()> {
    let mut count = 0usize;
    // A keyword the parse skipped has no field, and neither has its scope.
    let mut skipped = false;
    for chunk in container.children() {
        let chunk = chunk?;
        match &chunk.tag {
            b"DATA" => {
                let hash = chunk.payload.first_chunk().map(|b| u32::from_le_bytes(*b));
                skipped = hash.is_some_and(|hash| super::keyword(hash).is_none());
                count += usize::from(!skipped);
            }
            b"SCOP" if !skipped => {
                let field = count
                    .checked_sub(1)
                    .and_then(|i| fields.get_mut(i))
                    .context("checked combo scope disagrees with decoded fields")?;
                field.scoped = true;
                mark(&chunk, &mut field.children)?;
            }
            _ => {}
        }
    }
    if count != fields.len() {
        bail!("checked combo DATA count disagrees with decoded fields");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn build_chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = tag.to_vec();
        out.extend((payload.len() as u32).to_le_bytes());
        out.extend(payload);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        out
    }

    fn data(key: &str, text: &str) -> Vec<u8> {
        let mut payload = mungefs::fnv1a_lowercase(key).to_le_bytes().to_vec();
        payload.push(1);
        payload.extend(4u32.to_le_bytes());
        payload.extend((text.len() as u32 + 1).to_le_bytes());
        payload.extend(text.as_bytes());
        payload.push(0);
        build_chunk(b"DATA", &payload)
    }

    #[test]
    fn forward_animation_empty_scope_and_populated_scope_remain_distinct() {
        let animations = [
            data("Animation", "stand_attack1c"),
            data("Animation", "stand_attack1c"),
            build_chunk(b"SCOP", &[]),
            data("Animation", "stand_attack1c"),
            build_chunk(b"SCOP", &data("AimType", "Torso")),
        ]
        .concat();
        let payload = [
            build_chunk(b"NAME", &7u32.to_le_bytes()),
            data("State", "IDLE"),
            build_chunk(b"SCOP", &animations),
        ]
        .concat();
        let bytes = build_chunk(b"comb", &payload);
        let chunk = mungefs::chunks(&bytes).next().unwrap().unwrap();
        let combo = parse_combo(&chunk).unwrap();
        let state = combo.state("IDLE").unwrap();
        assert!(state.scoped);
        assert_eq!(state.children.len(), 3);
        assert!(!state.children[0].scoped);
        assert!(state.children[0].children.is_empty());
        assert!(state.children[1].scoped);
        assert!(state.children[1].children.is_empty());
        assert!(state.children[2].scoped);
        assert_eq!(
            state.children[2].find(K::AimType).unwrap().text(0),
            Some("Torso")
        );
        let serialized = ron::to_string(&combo).unwrap();
        assert_eq!(ron::from_str::<ComboDef>(&serialized).unwrap(), combo);
    }

    #[test]
    fn older_ron_loads_without_inventing_missing_scope_provenance() {
        let old = r#"(keyword:Animation,args:[Text("stand_attack1c")],children:[])"#;
        let field: ComboField = ron::from_str(old).unwrap();
        assert!(!field.scoped);
        assert_eq!(field.text(0), Some("stand_attack1c"));
        let legacy: ComboSet = ron::from_str("(combos:[])").unwrap();
        assert!(!legacy.scope_provenance);
        assert!(!legacy.load_provenance);
        assert!(legacy.loads.is_empty());
    }
}
