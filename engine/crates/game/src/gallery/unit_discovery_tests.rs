use super::*;
use crate::classdb::{fnv1a_lowercase, ClassDef};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture_root() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "openbf-gallery-units-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn class(name: &str, base: &str, props: &[(&str, &str)]) -> ClassDef {
    ClassDef {
        name: name.into(),
        kind: String::new(),
        base: base.into(),
        props: props
            .iter()
            .map(|(k, v)| (fnv1a_lowercase(k), (*v).into()))
            .collect(),
    }
}

fn dlc_gallery_fixture() -> PathBuf {
    let root = fixture_root();
    for dir in ["classes", "side_bf2/soldiers", "side_bf2/weapons"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(root.join("classes/bf2_all.ron"), "[]").unwrap();
    std::fs::write(
        root.join("side_bf2/dlc_heroes.ron"),
        "(heroes:[(class:\"dlc_hero_fisto\",side:\"rep\"),(class:\"dlc_hero_ventress\",side:\"cis\")])",
    )
    .unwrap();
    std::fs::write(root.join("side_bf2/soldiers/rep_inf_kitfisto.glb"), []).unwrap();
    std::fs::write(root.join("side_bf2/weapons/temp_saber.glb"), []).unwrap();
    let classes = vec![
        class(
            "dlc_hero_fisto",
            "soldier",
            &[
                ("GeometryName", "rep_inf_kitfisto"),
                ("AnimationName", "fisto"),
                ("WeaponName", "dlch_dlc_weap_fisto_saber"),
            ],
        ),
        class(
            "dlch_dlc_weap_fisto_saber",
            "saber",
            &[
                ("GeometryName", "temp_saber"),
                ("ComboAnimationBank", "human_sabre melee dlc_hero_fisto"),
            ],
        ),
    ];
    std::fs::write(
        root.join("classes/bf2_dlc.ron"),
        ron::to_string(&classes).unwrap(),
    )
    .unwrap();
    root
}

#[test]
fn dlc_actor_gallery_uses_its_weapon_and_animation_family() {
    let root = dlc_gallery_fixture();
    let units = discover_units(&root).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    let (_, _, weapon, set) = units
        .iter()
        .find(|(key, ..)| key == "bf2_rep_inf_kitfisto")
        .unwrap();
    assert_eq!(
        weapon.as_ref(),
        Some(&root.join("side_bf2/weapons/temp_saber.glb"))
    );
    assert_eq!(set.family, "fisto");
    assert_eq!(set.bank, "human_sabre");
}

#[test]
fn installed_dlc_gallery_rejects_missing_corrupt_and_unmapped_classes() {
    for contents in [None, Some("not RON"), Some("[]")] {
        let root = dlc_gallery_fixture();
        let path = root.join("classes/bf2_dlc.ron");
        match contents {
            Some(s) => std::fs::write(path, s).unwrap(),
            None => std::fs::remove_file(path).unwrap(),
        }
        let result = discover_units(&root);
        std::fs::remove_dir_all(root).unwrap();
        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("gallery"), "{message}");
        assert!(
            message.contains("bf2_dlc.ron") || message.contains("rep_inf_kitfisto"),
            "{message}"
        );
    }
}

#[test]
fn unit_discovery_keeps_the_actor_and_skips_its_low_companion() {
    let root = fixture_root();
    let soldiers = root.join("side_bf1/soldiers");
    std::fs::create_dir_all(&soldiers).unwrap();
    std::fs::create_dir_all(root.join("classes")).unwrap();
    std::fs::write(soldiers.join("trooper.glb"), []).unwrap();
    std::fs::write(soldiers.join("trooper.low.glb"), []).unwrap();
    std::fs::write(root.join("classes/bf2_ingame.ron"), "[]").unwrap();
    std::fs::write(
        root.join("classes/bf1_all.ron"),
        ron::to_string(&vec![class(
            "all_inf_soldier",
            "soldier",
            &[("GeometryName", "trooper")],
        )])
        .unwrap(),
    )
    .unwrap();
    let units = discover_units(&root).unwrap();
    std::fs::remove_dir_all(root).unwrap();
    let keys: Vec<_> = units.iter().map(|(key, ..)| key.as_str()).collect();
    assert_eq!(keys, ["bf1_trooper"]);
}
