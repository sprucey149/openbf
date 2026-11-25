//! The version of each group of baked output, and of the disc extraction.
//!
//! `scripts/pack.py` keys every `scripts/rebake.sh` group on its version here, the inputs it reads and the tools that
//! encode it, never on the bake's source or binary. A group bakes again on a player's machine only when one of those
//! changes. So: when a change makes a group's output different, raise that group's number here, in the same commit
//! (`python3 scripts/bake_versions.py bump <group>`, or `bump --card <ID>` from the card's `rebake=`). A change that
//! leaves every byte the same keeps the number (`bake_versions.py neutral <group>`; the bake gate proves it).
//! `engine/crates/bake/bake_versions.lock` records which code each number was checked against, and land.py's
//! `bake_versions.py check` refuses a code change that did neither.
//!
//! `extract-iso` covers what `bake extract-iso` and `bake extract-xiso` write: raising it makes every player extract
//! the discs again.

/// Raised when the meaning of a group's number changes (a new field in `format-versions`, say).
pub const SCHEMA: u32 = 1;

/// One entry per `REBAKE_GROUPS` group in scripts/rebake.sh, plus `extract-iso`.
pub const GROUP_VERSIONS: &[(&str, u32)] = &[
    ("tat1", 5),
    ("tat2", 6),
    ("maps", 8),
    ("common", 7),
    ("side", 5),
    ("dlc", 5),
    ("classes", 3),
    ("sounds", 2),
    ("global-sound", 2),
    ("load", 2),
    ("beep", 2),
    ("extract-iso", 2),
];

/// `bake format-versions`: the table above and the platform the binary was built for, as one JSON object.
pub fn format_versions_json() -> String {
    let groups: serde_json::Map<String, serde_json::Value> = GROUP_VERSIONS
        .iter()
        .map(|(name, version)| ((*name).to_owned(), (*version).into()))
        .collect();
    serde_json::json!({
        "schema": SCHEMA,
        "target": format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        "groups": groups,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::{format_versions_json, GROUP_VERSIONS};

    /// The groups rebake.sh knows, read from its own `REBAKE_GROUPS=` line.
    fn rebake_groups() -> Vec<&'static str> {
        let script = include_str!("../../../../scripts/rebake.sh");
        let line = script
            .lines()
            .find_map(|line| line.strip_prefix("REBAKE_GROUPS=\""))
            .expect("rebake.sh has a REBAKE_GROUPS line");
        line.trim_end_matches('"').split_whitespace().collect()
    }

    #[test]
    fn versions_name_every_rebake_group_once_and_the_extraction() {
        let mut names: Vec<&str> = GROUP_VERSIONS.iter().map(|(name, _)| *name).collect();
        let mut want = rebake_groups();
        want.push("extract-iso");
        names.sort_unstable();
        want.sort_unstable();
        assert_eq!(names, want);
        assert!(GROUP_VERSIONS.iter().all(|(_, version)| *version >= 1));
    }

    #[test]
    fn versions_format_is_the_json_pack_reads() {
        let value: serde_json::Value = serde_json::from_str(&format_versions_json()).unwrap();
        assert_eq!(value["schema"], 1);
        assert_eq!(value["groups"]["sounds"], GROUP_VERSIONS.iter().find(|(n, _)| *n == "sounds").unwrap().1);
        assert_eq!(value["groups"].as_object().unwrap().len(), GROUP_VERSIONS.len());
        assert!(value["target"].as_str().unwrap().contains('-'));
    }
}
