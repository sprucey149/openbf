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

/// Issue #194 (B1): a seat's pick or new profile never moves `last`, so the
/// title picker offers the host's profile next launch.
#[test]
fn bind_and_create_quiet_keep_last() {
    let dir = Scratch::new();
    let store = Store::new(dir.0.clone());
    let host = store.create("Host", Difficulty::Hard).unwrap();
    let host_id = host.id().unwrap();
    let other = store.create("Other", Difficulty::Normal).unwrap();
    let other_id = other.id().unwrap();
    store.select(host_id).unwrap();
    assert_eq!(store.list().unwrap().last, Some(host_id));

    let bound = store.bind(other_id).unwrap();
    assert_eq!(bound.id(), Some(other_id));
    assert_eq!(bound.name, "Other");
    assert!(bound.is_seat());
    assert_eq!(store.list().unwrap().last, Some(host_id));

    let quiet = store.create_quiet("Guest", Difficulty::Hard).unwrap();
    assert!(quiet.is_seat());
    assert_eq!(quiet.settings.difficulty, Difficulty::Hard);
    let quiet_id = quiet.id().unwrap();
    assert!(quiet_id != host_id && quiet_id != other_id);
    let list = store.list().unwrap();
    assert_eq!(list.last, Some(host_id));
    assert_eq!(list.names.len(), 3);
    // The name rules still hold.
    assert!(store.create_quiet("host", Difficulty::Normal).is_err());
    assert!(store.create_quiet("  ", Difficulty::Normal).is_err());
    assert!(store.bind(999).is_err());

    // A seat's profile saves to its own entry.
    let mut quiet = quiet;
    quiet.career.kills = 5;
    quiet.save().unwrap();
    assert_eq!(store.bind(quiet_id).unwrap().career.kills, 5);
    assert_eq!(store.select(host_id).unwrap().career.kills, 0);
    // The title picker's create still moves `last`.
    let next = store.create("Next", Difficulty::Normal).unwrap();
    assert!(!next.is_seat());
    assert_eq!(store.list().unwrap().last, next.id());
}

/// The catalog file as an older build wrote it, with each profile's
/// `settings` given as RON text.
fn catalog_text(entries: &[(u64, &str, &str)]) -> String {
    let profiles: Vec<String> = entries
        .iter()
        .map(|(id, name, settings)| {
            format!(
                "{id}: (name: \"{name}\", settings: {settings}, career: (rounds: 0, wins: 0, \
                 kills: {id}, deaths: 0, shots: 0, hits: 0, captures: 0))"
            )
        })
        .collect();
    format!(
        "(profiles: {{{}}}, last: Some(0), next: {})",
        profiles.join(", "),
        entries.len()
    )
}

/// Issue #194 (B1): an older file loads the Splitscreen defaults; a partly
/// missing entry loads; a bad `cells` (not a permutation, or the wrong
/// length, range or type) reads as the identity, a non-bool `name_tags` as
/// on; an unknown
/// `two_player` or `hud_size` loads as the default and the other profiles in
/// the file still load.
#[test]
fn splitscreen_settings_load_from_any_catalog() {
    use crate::profile::{HudSize, SplitscreenSettings, TwoPlayerSplit, IDENTITY_CELLS};
    let dir = Scratch::new();
    fs::write(
        dir.0.join("profiles.ron"),
        catalog_text(&[
            (0, "Old", "(difficulty: hard)"),
            (1, "Partial", "(splitscreen: (name_tags: false))"),
            (
                2,
                "BadCells",
                "(splitscreen: (cells: (0, 0, 7, 1), two_player: TopAndBottom))",
            ),
            (
                3,
                "Future",
                "(splitscreen: (two_player: Diagonal, hud_size: Tiny, cells: (3, 2, 1, 0)))",
            ),
            (
                4,
                "Short",
                "(splitscreen: (cells: (0, 1, 2), name_tags: false))",
            ),
            (
                5,
                "Wide",
                "(splitscreen: (cells: (300, 0, 1, 2), name_tags: 3))",
            ),
            (
                6,
                "Long",
                "(splitscreen: (cells: (3, 2, 1, 0, 4), name_tags: \"no\"))",
            ),
            (
                7,
                "Odd",
                "(splitscreen: (cells: \"left\", two_player: TopAndBottom))",
            ),
            (8, "Negative", "(splitscreen: (cells: (-1, 0, 1, 2)))"),
        ]),
    )
    .unwrap();
    let store = Store::new(dir.0.clone());
    let list = store.list().unwrap();
    assert_eq!(list.names.len(), 9);

    let old = store.select(0).unwrap();
    assert_eq!(old.settings.difficulty, Difficulty::Hard);
    assert_eq!(old.settings.splitscreen, SplitscreenSettings::default());
    assert!(old.settings.splitscreen.name_tags);
    assert_eq!(old.settings.splitscreen.cells, IDENTITY_CELLS);
    assert_eq!(
        old.settings.splitscreen.two_player,
        TwoPlayerSplit::SideBySide
    );
    assert_eq!(old.settings.splitscreen.hud_size, HudSize::Full);

    let partial = store.bind(1).unwrap().settings.splitscreen;
    assert!(!partial.name_tags);
    assert_eq!(partial.cells, IDENTITY_CELLS);
    assert_eq!(partial.two_player, TwoPlayerSplit::SideBySide);

    let bad = store.bind(2).unwrap().settings.splitscreen;
    assert_eq!(bad.cells, IDENTITY_CELLS);
    assert_eq!(bad.two_player, TwoPlayerSplit::TopAndBottom);

    let future = store.bind(3).unwrap();
    assert_eq!(future.career.kills, 3);
    let s = future.settings.splitscreen;
    assert_eq!(s.two_player, TwoPlayerSplit::SideBySide);
    assert_eq!(s.hud_size, HudSize::Full);
    assert_eq!(s.cells, [3, 2, 1, 0]);

    // The wrong length, range or type reads as the default, field by field.
    let short = store.bind(4).unwrap().settings.splitscreen;
    assert_eq!(short.cells, IDENTITY_CELLS);
    assert!(!short.name_tags);
    for id in [5, 6, 8] {
        let s = store.bind(id).unwrap().settings.splitscreen;
        assert_eq!(s.cells, IDENTITY_CELLS, "{id}");
        assert!(s.name_tags, "{id}");
    }
    let odd = store.bind(7).unwrap().settings.splitscreen;
    assert_eq!(odd.cells, IDENTITY_CELLS);
    assert_eq!(odd.two_player, TwoPlayerSplit::TopAndBottom);
}

/// The Splitscreen options round-trip through the catalog.
#[test]
fn splitscreen_settings_round_trip() {
    use crate::profile::{HudSize, SplitscreenSettings, TwoPlayerSplit};
    let dir = Scratch::new();
    let store = Store::new(dir.0.clone());
    let mut p = store.create("Host", Difficulty::Normal).unwrap();
    let id = p.id().unwrap();
    let chosen = SplitscreenSettings {
        two_player: TwoPlayerSplit::TopAndBottom,
        cells: [2, 0, 3, 1],
        name_tags: false,
        hud_size: HudSize::Smallest,
    };
    p.settings.splitscreen = chosen;
    p.save().unwrap();
    assert_eq!(store.select(id).unwrap().settings.splitscreen, chosen);
    assert_eq!(store.bind(id).unwrap().settings, p.settings);
}
