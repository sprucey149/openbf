//! A legacy mod's class changes as a patch over a stock `classes/*.ron`
//! (`<name>.patch.ron` beside it, served only by a mod's overlay).
//!
//! The stock classes are the PS2's gameplay values, while a legacy mod is
//! authored against the PC's. A mod's LVL therefore cannot simply replace the
//! stock file: every class it did not touch would switch from the PS2's
//! numbers to the PC's. `bake mod-convert` compares each of the mod's classes
//! with the stock PC copy it replaces and records only what the author
//! changed; the game applies those changes over the PS2-derived classes.

use serde::{Deserialize, Serialize};

use crate::ClassDef;

pub const CLASS_PATCH_SCHEMA: u32 = 1;

/// The file a class file's patch lives at: `bf2_imp.ron` -> `bf2_imp.patch.ron`.
pub fn patch_path(class_file: &std::path::Path) -> std::path::PathBuf {
    let stem = class_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("classes");
    class_file.with_file_name(format!("{stem}.patch.ron"))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassPatchFile {
    pub schema: u32,
    /// The mod LVL and the stock LVL the patch was computed between, for the
    /// player and for diagnostics.
    pub source: String,
    pub classes: Vec<ClassPatch>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClassPatch {
    /// A class only the mod has.
    Add(ClassDef),
    /// A stock class the mod's LVL no longer carries.
    Remove { name: String },
    /// A class both carry: the BASE when the mod changed it, every key whose
    /// value list the mod changed (all of the key's values, in the mod's
    /// order) and every key the mod dropped.
    Change {
        name: String,
        #[serde(default)]
        base: Option<String>,
        #[serde(default)]
        set: Vec<(u32, Vec<String>)>,
        #[serde(default)]
        remove: Vec<u32>,
    },
}

impl ClassPatch {
    pub fn name(&self) -> &str {
        match self {
            Self::Add(d) => &d.name,
            Self::Remove { name } | Self::Change { name, .. } => name,
        }
    }
}

/// Apply `patch` to `defs` in place. A changed key keeps the position of its
/// first stock occurrence (a class's property order is meaningful: collision
/// layers apply in PROP order); a key the stock class lacks is appended.
pub fn apply(defs: &mut Vec<ClassDef>, patch: &ClassPatchFile) {
    for p in &patch.classes {
        match p {
            ClassPatch::Add(d) => {
                if let Some(old) = defs.iter_mut().find(|c| c.name == d.name) {
                    *old = d.clone();
                } else {
                    defs.push(d.clone());
                }
            }
            ClassPatch::Remove { name } => defs.retain(|c| &c.name != name),
            ClassPatch::Change {
                name,
                base,
                set,
                remove,
            } => {
                let Some(c) = defs.iter_mut().find(|c| &c.name == name) else {
                    continue;
                };
                if let Some(b) = base {
                    c.base = b.clone();
                }
                for (key, values) in set {
                    let at = c.props.iter().position(|(k, _)| k == key);
                    c.props.retain(|(k, _)| k != key);
                    let rows = values.iter().map(|v| (*key, v.clone()));
                    match at {
                        Some(i) => {
                            let i = i.min(c.props.len());
                            let tail = c.props.split_off(i);
                            c.props.extend(rows);
                            c.props.extend(tail);
                        }
                        None => c.props.extend(rows),
                    }
                }
                c.props.retain(|(k, _)| !remove.contains(k));
            }
        }
    }
}

/// The patch that turns `stock` (the stock PC classes) into `modded` (the
/// mod's).
pub fn diff(stock: &[ClassDef], modded: &[ClassDef], source: &str) -> ClassPatchFile {
    let mut classes = Vec::new();
    for m in modded {
        let Some(s) = stock.iter().find(|s| s.name == m.name) else {
            classes.push(ClassPatch::Add(m.clone()));
            continue;
        };
        if s == m {
            continue;
        }
        let keys = |d: &ClassDef| {
            let mut ks: Vec<u32> = Vec::new();
            for (k, _) in &d.props {
                if !ks.contains(k) {
                    ks.push(*k);
                }
            }
            ks
        };
        let values = |d: &ClassDef, k: u32| -> Vec<String> {
            d.props
                .iter()
                .filter(|(h, _)| *h == k)
                .map(|(_, v)| v.clone())
                .collect()
        };
        let mut set = Vec::new();
        for k in keys(m) {
            let mv = values(m, k);
            if mv != values(s, k) {
                set.push((k, mv));
            }
        }
        let remove: Vec<u32> = keys(s)
            .into_iter()
            .filter(|k| !m.props.iter().any(|(h, _)| h == k))
            .collect();
        let base = (s.base != m.base).then(|| m.base.clone());
        if set.is_empty() && remove.is_empty() && base.is_none() {
            continue;
        }
        classes.push(ClassPatch::Change {
            name: m.name.clone(),
            base,
            set,
            remove,
        });
    }
    for s in stock {
        if !modded.iter().any(|m| m.name == s.name) {
            classes.push(ClassPatch::Remove {
                name: s.name.clone(),
            });
        }
    }
    ClassPatchFile {
        schema: CLASS_PATCH_SCHEMA,
        source: source.to_owned(),
        classes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(name: &str, props: &[(u32, &str)]) -> ClassDef {
        ClassDef {
            name: name.into(),
            kind: "entc".into(),
            base: "soldier".into(),
            props: props.iter().map(|(k, v)| (*k, (*v).to_owned())).collect(),
        }
    }

    #[test]
    fn a_patch_carries_only_authored_changes_onto_other_values() {
        let pc = vec![
            def("trooper", &[(1, "pc-speed"), (2, "a"), (2, "b"), (3, "tex")]),
            def("gone", &[]),
        ];
        let modded = vec![
            def("trooper", &[(1, "pc-speed"), (2, "a"), (2, "c"), (4, "new")]),
            def("added", &[(9, "x")]),
        ];
        let patch = diff(&pc, &modded, "test");
        let mut ps2 = vec![
            def("trooper", &[(1, "ps2-speed"), (2, "a"), (2, "b"), (3, "tex"), (5, "ps2")]),
            def("gone", &[]),
        ];
        apply(&mut ps2, &patch);
        let t = ps2.iter().find(|c| c.name == "trooper").unwrap();
        assert_eq!(
            t.props,
            vec![
                (1, "ps2-speed".to_owned()),
                (2, "a".to_owned()),
                (2, "c".to_owned()),
                (5, "ps2".to_owned()),
                (4, "new".to_owned()),
            ]
        );
        assert!(ps2.iter().any(|c| c.name == "added"));
        assert!(!ps2.iter().any(|c| c.name == "gone"));
        assert_eq!(
            patch_path(std::path::Path::new("classes/bf2_imp.ron")),
            std::path::Path::new("classes/bf2_imp.patch.ron")
        );
    }
}
