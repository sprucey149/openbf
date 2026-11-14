//! Narrow corrections to shipped gameplay data; raw parsing stays lossless.
use super::*;

pub(super) fn vader_terminal_target(combo: &mut ComboDef) {
    // PC and Xbox IMP.LVL both explicitly return Vader's second attack to
    // IDLE. PS2 names missing RECOVER2; its loader resolves that to IDLE too.
    // Correct the label, without inventing a recovery state or importing the
    // donors' four unrelated MustShowOneFrame flags (analysis/re-combo.md).
    if combo.name_hash != 0xe4c0_31da || combo.state("RECOVER2").is_some() {
        return;
    }
    let Some(state) = combo.fields.iter_mut().find(|field| {
        field.keyword == K::State
            && field
                .text(0)
                .is_some_and(|name| name.eq_ignore_ascii_case("ATTACK2"))
    }) else {
        return;
    };
    let Some(terminal) = state
        .children
        .iter_mut()
        .rfind(|field| field.keyword == K::Transition)
    else {
        return;
    };
    // Match this shipped label exactly, not a generalized typo correction.
    if terminal.args.len() == 1
        && terminal.text(0) == Some("RECOVER2")
        && !terminal.scoped
        && terminal.children.is_empty()
    {
        terminal.args[0] = ComboValue::Text("IDLE".into());
    }
}

/// The four deflect states of Vader's program on PC and Xbox.
const VADER_DEFLECTS: [&str; 4] = [
    "DEFLECT_JUMP",
    "DEFLECT_CROUCH",
    "DEFLECT_ROLL",
    "DEFLECT_SPRINT",
];

pub(super) fn vader_deflect_frame_flags(combo: &mut ComboDef) {
    // PC and Xbox IMP.LVL give Vader's four deflect states a MustShowOneFrame
    // flag the PS2 program lacks (the Emperor's program has it on every copy).
    // Entering a flagged state ends that tick's chain of transitions, so the
    // copies step Vader's saber differently and a mixed LAN round would
    // refuse or desynchronise (issue #168). The PS2 program is the rule: drop
    // exactly these four flags, and only when all four are present.
    if combo.name_hash != 0xe4c0_31da {
        return;
    }
    let is_flag = |f: &ComboField| {
        f.keyword == K::MustShowOneFrame && f.args.is_empty() && !f.scoped && f.children.is_empty()
    };
    let deflects = |f: &ComboField| {
        f.keyword == K::State
            && f.text(0)
                .is_some_and(|name| VADER_DEFLECTS.iter().any(|d| name.eq_ignore_ascii_case(d)))
    };
    let flagged = combo
        .fields
        .iter()
        .filter(|f| deflects(f) && f.children.iter().filter(|c| is_flag(c)).count() == 1)
        .count();
    if flagged != VADER_DEFLECTS.len() {
        return;
    }
    for state in combo.fields.iter_mut().filter(|f| deflects(f)) {
        state.children.retain(|c| !is_flag(c));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(keyword: K, name: &str, children: Vec<ComboField>) -> ComboField {
        ComboField {
            keyword,
            args: vec![ComboValue::Text(name.into())],
            scoped: keyword == K::State,
            children,
        }
    }

    fn program(hash: u32, target: &str, recovery: bool) -> ComboDef {
        let mut fields = vec![
            field(K::State, "IDLE", vec![]),
            field(
                K::State,
                "ATTACK2",
                vec![field(K::Transition, target, vec![])],
            ),
        ];
        if recovery {
            fields.push(field(K::State, "RECOVER2", vec![]));
        }
        ComboDef {
            name_hash: hash,
            fields,
        }
    }

    #[test]
    fn the_shipped_correction_is_idempotent() {
        let mut raw = program(0xe4c0_31da, "RECOVER2", false);
        let corrected = program(0xe4c0_31da, "IDLE", false);
        vader_terminal_target(&mut raw);
        assert_eq!(raw, corrected);
        vader_terminal_target(&mut raw);
        assert_eq!(raw, corrected);
    }

    fn deflects(hash: u32, flagged: usize) -> ComboDef {
        let flag = ComboField {
            keyword: K::MustShowOneFrame,
            args: Vec::new(),
            scoped: false,
            children: Vec::new(),
        };
        let mut fields = vec![field(K::State, "IDLE", vec![])];
        for (i, name) in VADER_DEFLECTS.iter().enumerate() {
            let mut children = vec![field(K::Posture, "Jump", vec![])];
            if i < flagged {
                children.push(flag.clone());
            }
            children.push(field(K::Transition, "IDLE", vec![]));
            fields.push(field(K::State, name, children));
        }
        ComboDef {
            name_hash: hash,
            fields,
        }
    }

    #[test]
    fn vaders_donor_deflect_flags_become_the_ps2_program() {
        let mut donor = deflects(0xe4c0_31da, 4);
        vader_deflect_frame_flags(&mut donor);
        assert_eq!(donor, deflects(0xe4c0_31da, 0));
        vader_deflect_frame_flags(&mut donor);
        assert_eq!(donor, deflects(0xe4c0_31da, 0));
        for mut other in [deflects(0xa11a_0085, 4), deflects(0xe4c0_31da, 3)] {
            let before = other.clone();
            vader_deflect_frame_flags(&mut other);
            assert_eq!(other, before);
        }
    }

    #[test]
    fn valid_recovery_other_programs_and_other_labels_are_untouched() {
        for mut source in [
            program(0xe4c0_31da, "RECOVER2", true),
            program(0xa11a_0085, "RECOVER2", false),
            program(0xe4c0_31da, "IDLE", false),
            program(0xe4c0_31da, "recover2", false),
        ] {
            let before = source.clone();
            vader_terminal_target(&mut source);
            assert_eq!(source, before);
        }
    }
}
