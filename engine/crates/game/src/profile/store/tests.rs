use super::*;
use crate::profile::Career;
use core::skill::Difficulty;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "openbf-profile-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn legacy_progress_and_settings_migrate_once() {
    let dir = Scratch::new();
    let path = dir.0.join("profile.ron");
    let mut old = Profile {
        name: "Original player".into(),
        ..Default::default()
    };
    old.settings.difficulty = Difficulty::Hard;
    old.settings.flight_controls = input::FlightControls::RcMode2;
    old.settings
        .controller_bindings
        .remap(0, gilrs::Button::West);
    old.settings
        .keyboard_bindings
        .remap(0, crate::kbm::Trigger::Key(winit::keyboard::KeyCode::KeyP));
    old.career.kills = 42;
    old.career.medals[0] = 12;
    old.career.class_kills.insert("all_inf_officer".into(), 7);
    old.save_to(&path).unwrap();
    let original = fs::read(&path).unwrap();

    let store = Store::new(dir.0.clone());
    let List { names, last } = store.list().unwrap();
    assert_eq!(names.len(), 1);
    let id = names[0].0;
    assert_eq!(last, Some(id));
    let migrated = store.select(id).unwrap();
    assert_eq!(
        (&migrated.name, &migrated.settings, &migrated.career),
        (&old.name, &old.settings, &old.career)
    );
    assert_eq!(fs::read(&path).unwrap(), original);

    store.delete(id).unwrap();
    assert!(store.list().unwrap().names.is_empty());
    let fresh = store.for_direct_launch().unwrap();
    assert_eq!(fresh.career, Career::default());
    assert_ne!(fresh.slot.unwrap().id, id);
    assert!(migrated.save().is_err());
}

#[test]
fn profiles_keep_separate_state_and_last_used_selection() {
    let dir = Scratch::new();
    let store = Store::new(dir.0.clone());
    let mut alice = store.create("Alice", Difficulty::Easy).unwrap();
    let mut bob = store.create("Bob", Difficulty::Elite).unwrap();
    let a = alice.slot.as_ref().unwrap().id;
    let b = bob.slot.as_ref().unwrap().id;
    assert!(store.create(" alice ", Difficulty::Normal).is_err());
    assert!(store.create(" ", Difficulty::Normal).is_err());
    assert!(store.create("elevenchars", Difficulty::Normal).is_err());
    alice.career.kills = 10;
    bob.career.kills = 20;
    alice.settings.music_volume = 0.25;
    bob.settings.music_volume = 0.75;
    std::thread::scope(|s| {
        s.spawn(|| alice.save().unwrap());
        s.spawn(|| bob.save().unwrap());
    });
    let reopened = Store::new(dir.0.clone());
    assert_eq!(reopened.list().unwrap().last, Some(b));
    assert_eq!(reopened.select(a).unwrap(), alice);
    assert_eq!(reopened.for_direct_launch().unwrap(), alice);
    assert_eq!(reopened.select(b).unwrap(), bob);
    reopened.delete(b).unwrap();
    assert_eq!(reopened.list().unwrap().last, Some(a));
}

#[test]
fn an_unreadable_catalog_is_preserved() {
    let dir = Scratch::new();
    let path = dir.0.join("profiles.ron");
    let store = Store::new(dir.0.clone());
    for raw in [
        "invalid profile catalog",
        "(profiles:{},last:None",
        r#"(profiles:{},last:"wrong type",next:0)"#,
        "(profiles:{},last:None,next:18446744073709551616)",
    ] {
        fs::write(&path, raw).unwrap();
        assert!(store.list().is_err());
        assert!(store.create("New", Difficulty::Normal).is_err());
        assert_eq!(fs::read(&path).unwrap(), raw.as_bytes());
    }
    let exhausted = "(profiles:{},last:None,next:18446744073709551615)";
    fs::write(&path, exhausted).unwrap();
    assert!(store.list().unwrap().names.is_empty());
    assert!(store.create("New", Difficulty::Normal).is_err());
    assert_eq!(fs::read(&path).unwrap(), exhausted.as_bytes());
}
