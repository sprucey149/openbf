use super::*;

/// BF2's English map names (§6), the rest from the tables' own fallbacks.
fn text(game: Game, key: &str) -> Option<String> {
    let names = [
        ("cor1", "Coruscant"),
        ("dag1", "Dagobah"),
        ("dea1", "Death Star"),
        ("end1", "Endor"),
        ("fel1", "Felucia"),
        ("geo1", "Geonosis"),
        ("hot1", "Hoth"),
        ("tat3", "Jabba's Palace"),
        ("kam1", "Kamino"),
        ("kas2", "Kashyyyk"),
        ("tat2", "Mos Eisley"),
        ("mus1", "Mustafar"),
        ("myg1", "Mygeeto"),
        ("nab2", "Naboo"),
        ("pol1", "Polis Massa"),
        ("spa7", "Space Felucia"),
        ("spa8", "Space Hoth"),
        ("spa3", "Space Kashyyyk"),
        ("spa6", "Space Mygeeto"),
        ("spa9", "Space Tatooine"),
        ("spa1", "Space Yavin"),
        ("tan1", "Tantive IV"),
        ("uta1", "Utapau"),
        ("yav1", "Yavin 4"),
    ];
    let code = key.strip_prefix("mapname.name.")?;
    (game == Game::Bf2)
        .then(|| names.iter().find(|(c, _)| *c == code))
        .flatten()
        .map(|(_, n)| (*n).to_owned())
}

/// The main checkout's bake (§6 "What the port already bakes"): every map
/// but the six space maps, BF1 Dune Sea and BF2 Mos Eisley in their legacy
/// directories; conquest missions for every era a map has.
fn maps() -> Vec<MapEntry> {
    let mut bakes: Vec<(String, String)> = Vec::new();
    for (code, ..) in BF1_MAPS {
        let key = if code == "tat1" {
            "tat1".to_owned()
        } else {
            format!("bf1_{code}")
        };
        bakes.push((key, "bf1".into()));
    }
    for (code, ..) in BF2_MAPS {
        if code.starts_with("spa") {
            continue;
        }
        let key = if code == "tat2" {
            "tat2".to_owned()
        } else {
            format!("bf2_{code}")
        };
        bakes.push((key, "bf2".into()));
    }
    let eras = |key: &str| -> Vec<Era> {
        let bf2 = BF2_MAPS
            .iter()
            .find(|(c, ..)| key == format!("bf2_{c}") || (key == *c && *c == "tat2"));
        if let Some((_, cw, gcw)) = bf2 {
            let mut v = Vec::new();
            if cw.contains(&Mode::Con) {
                v.push(Era::Cw);
            }
            if gcw.contains(&Mode::Con) {
                v.push(Era::Gcw);
            }
            return v;
        }
        let (_, _, _, cw, gcw) = BF1_MAPS
            .iter()
            .find(|(c, ..)| key == format!("bf1_{c}") || (key == *c && *c == "tat1"))
            .copied()
            .unwrap();
        [(cw, Era::Cw), (gcw, Era::Gcw)]
            .into_iter()
            .filter(|(on, _)| *on)
            .map(|(_, e)| e)
            .collect()
    };
    let missions = |key: &str, mode: Mode| -> Vec<[String; 2]> {
        if mode != Mode::Con {
            return Vec::new();
        }
        eras(key)
            .into_iter()
            // Missions name the defender first as often as not.
            .map(|e| {
                let [a, b] = e.armies();
                [b.to_owned(), a.to_owned()]
            })
            .collect()
    };
    let localized = |game, key: &str| match key {
        "planets.bespin.mapname2" => Some("  BESPIN: CLOUD CITY  ".to_owned()),
        _ => text(game, key),
    };
    catalog(&localized, &bakes, &missions)
}

fn ia() -> InstantAction {
    InstantAction::new(
        maps(),
        IaTexts::new(&|_, e| e.to_owned()),
        SessionOptions::default(),
    )
}

fn press(f: impl Fn(&mut IaInput)) -> IaInput {
    let mut i = IaInput::default();
    f(&mut i);
    i
}

fn cross() -> IaInput {
    press(|i| i.menu.confirm = true)
}

fn triangle() -> IaInput {
    press(|i| i.menu.back = true)
}

fn down() -> IaInput {
    press(|i| i.menu.down = true)
}

fn up() -> IaInput {
    press(|i| i.menu.up = true)
}

#[test]
fn map_navigation_wraps_over_disabled_rows() {
    let mut cursor = ListCursor::first(&[false, true, true, false], 2);
    assert!(cursor.step_wrapping(&[false, true, true, false], false, 2));
    assert_eq!(cursor.at, 2);
    assert!(cursor.step_wrapping(&[false, true, true, false], true, 2));
    assert_eq!(cursor.at, 1);
}

/// §6 "Merged list": the 40 rows in the case-folded order, each with its
/// game, then All Maps.
#[test]
fn the_merged_list_is_both_games_case_folded() {
    let m = maps();
    let labels: Vec<&str> = m.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Bespin: Cloud City (bf1)",
            "Bespin: Platforms (bf1)",
            "Coruscant (bf2)",
            "Dagobah (bf2)",
            "Death Star (bf2)",
            "Endor (bf2)",
            "Endor: Bunker (bf1)",
            "Felucia (bf2)",
            "Geonosis (bf2)",
            "Geonosis: Spire (bf1)",
            "Hoth (bf2)",
            "Hoth: Echo Base (bf1)",
            "Jabba's Palace (bf2)",
            "Kamino (bf2)",
            "Kamino: Tipoca City (bf1)",
            "Kashyyyk (bf2)",
            "Kashyyyk: Docks (bf1)",
            "Kashyyyk: Islands (bf1)",
            "Mos Eisley (bf2)",
            "Mustafar (bf2)",
            "Mygeeto (bf2)",
            "Naboo (bf2)",
            "Naboo: Plains (bf1)",
            "Naboo: Theed (bf1)",
            "Polis Massa (bf2)",
            "Rhen Var: Citadel (bf1)",
            "Rhen Var: Harbor (bf1)",
            "Space Felucia (bf2)",
            "Space Hoth (bf2)",
            "Space Kashyyyk (bf2)",
            "Space Mygeeto (bf2)",
            "Space Tatooine (bf2)",
            "Space Yavin (bf2)",
            "Tantive IV (bf2)",
            "Tatooine: Dune Sea (bf1)",
            "Tatooine: Mos Eisley (bf1)",
            "Utapau (bf2)",
            "Yavin 4 (bf2)",
            "Yavin 4: Arena (bf1)",
            "Yavin 4: Temple (bf1)",
        ]
    );
    // Keys: the bake's directories, the legacy two unprefixed; a map that
    // is not baked keeps `<game>_<code>`.
    let key = |l: &str| m.iter().find(|e| e.label == l).unwrap().key.as_str();
    assert_eq!(key("Hoth (bf2)"), "bf2_hot1");
    assert_eq!(key("Hoth: Echo Base (bf1)"), "bf1_hot1");
    assert_eq!(key("Mos Eisley (bf2)"), "tat2");
    assert_eq!(key("Tatooine: Mos Eisley (bf1)"), "bf1_tat2");
    assert_eq!(key("Tatooine: Dune Sea (bf1)"), "tat1");
    assert_eq!(key("Space Yavin (bf2)"), "bf2_spa1");
    let rows = ia().map_rows();
    assert_eq!(rows.len(), 41);
    assert_eq!(rows[40].text, "All Maps");
}

/// `[ours]` Grouped by source: the two games, then the mods by name, each
/// opening to its maps without the suffix; All Maps stays inside the open
/// folder.
#[test]
fn grouped_maps_open_by_source_folder() {
    let mod_map = |key: &str, name: &str, source: &str| ModMapEntry {
        key: key.into(),
        variant_keys: vec![key.into()],
        legacy_code: "abr".into(),
        name: name.into(),
        disambiguator: source.into(),
        description: String::new(),
        modes: [vec![Mode::Con], vec![Mode::Con]],
        source: source.into(),
    };
    let mod_maps = [
        mod_map("mod_z__abr", "Abregado-Rae", "Zeta Pack"),
        mod_map("mod_a__abr", "Yoda's Hut", "alpha maps"),
    ];
    let bakes = [
        ("bf1_hot1", "bf1"),
        ("bf1_bes1", "bf1"),
        ("bf2_hot1", "bf2"),
        ("mod_z__abr", "bf2"),
        ("mod_a__abr", "bf2"),
    ]
    .map(|(k, g)| (k.to_owned(), g.to_owned()));
    let missions = |_: &str, mode: Mode| match mode {
        Mode::Con => vec![["all".to_owned(), "imp".to_owned()]],
        _ => Vec::new(),
    };
    let mut m = InstantAction::new(
        catalog_with_mods(&text, &bakes, &missions, &mod_maps),
        IaTexts::new(&|_, e| e.to_owned()),
        SessionOptions::default(),
    );
    m.group_by_source();
    let texts =
        |m: &InstantAction| -> Vec<String> { m.map_rows().into_iter().map(|r| r.text).collect() };
    assert_eq!(
        texts(&m),
        [
            "Battlefront 1",
            "Battlefront 2",
            "alpha maps",
            "Zeta Pack",
            "All Maps"
        ]
    );

    m.focus_command(Cmd::Add);
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Map);
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Map, "a folder opens in place");
    let bf1 = texts(&m);
    assert_eq!(bf1.len(), BF1_MAPS.len() + 1);
    assert_eq!(bf1[0], "Bespin: Cloud City");
    assert_eq!(bf1[bf1.len() - 1], "All Maps");
    assert!(bf1.iter().all(|t| !t.contains("(bf")), "{bf1:?}");
    m.step(triangle(), 0.0);
    assert_eq!(m.state, State::Map);
    assert_eq!(m.folder, None);
    assert_eq!(m.map_list.at, 0);
    m.step(triangle(), 0.0);
    assert_eq!(m.state, State::Command);

    // The folder's All Maps, then every mode and era.
    m.focus_command(Cmd::Add);
    m.step(cross(), 0.0);
    m.step(cross(), 0.0);
    m.step(up(), 0.0);
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Mode);
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Era);
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Command);
    assert_eq!(m.folder, None);
    let added: Vec<&str> = m
        .playlist
        .iter()
        .map(|e| m.maps[e.map].source.as_str())
        .collect();
    assert!(!added.is_empty());
    assert!(added.iter().all(|s| *s == "Battlefront 1"), "{added:?}");

    // At the folders, All Maps is every map.
    m.playlist.clear();
    m.add(MapPick::All, ModePick::All, SidePick::AllEras);
    let mut all: Vec<usize> = m.playlist.iter().map(|e| e.map).collect();
    all.dedup();
    assert_eq!(all.len(), bakes.len());
}

/// §7: the unbaked space maps are dimmed and the cursor skips them both
/// ways; a mode the port does not run is dimmed, and so is All Modes with
/// only Conquest live.
#[test]
fn dimmed_rows_are_skipped_by_the_cursor() {
    let mut m = ia();
    let rows = m.map_rows();
    let dimmed: Vec<&str> = rows
        .iter()
        .filter(|r| !r.enabled)
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(dimmed.len(), 6, "{dimmed:?}");
    assert!(dimmed.iter().all(|t| t.starts_with("Space ")));
    m.step(cross(), 0.0); // ADD
    assert_eq!(m.state, State::Map);
    let polis = rows
        .iter()
        .position(|r| r.text == "Polis Massa (bf2)")
        .unwrap();
    m.map_list = ListCursor {
        at: polis,
        view: crate::scroll::ScrollView::at((polis - 4) as f32),
    };
    m.step(down(), 0.0);
    assert_eq!(rows[m.map_list.at].text, "Rhen Var: Citadel (bf1)");
    m.step(down(), 0.0);
    m.step(down(), 0.0);
    assert_eq!(rows[m.map_list.at].text, "Tantive IV (bf2)", "six skipped");
    m.step(up(), 0.0);
    assert_eq!(rows[m.map_list.at].text, "Rhen Var: Harbor (bf1)");
    // Hoth (bf2): Conquest live, 1-flag CTF and Hunt dimmed, All Modes too.
    let hoth = rows.iter().position(|r| r.text == "Hoth (bf2)").unwrap();
    m.map_list.at = hoth;
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Mode);
    let modes: Vec<(String, bool)> = m
        .mode_rows()
        .into_iter()
        .map(|r| (r.text, r.enabled))
        .collect();
    assert_eq!(
        modes,
        [
            ("Conquest".into(), true),
            ("1-flag CTF".into(), false),
            ("Hunt".into(), false),
            ("All Modes".into(), false),
        ]
    );
    m.step(down(), 0.0);
    assert_eq!(m.mode_list.at, 0, "nothing live below Conquest");
}

/// §1.3: every press of the five states, the dim rule and the focus.
#[test]
fn the_states_and_their_presses() {
    let mut m = ia();
    assert_eq!(m.state, State::Command);
    assert_eq!(COMMANDS[m.command], Cmd::Add, "LAUNCH is dimmed: ADD first");
    assert!(!m.command_live(Cmd::Launch));
    assert!(
        m.command_live(Cmd::Remove),
        "REMOVE is live on an empty list"
    );
    assert_eq!(m.info(), "Add a map to the playlist.");
    // Up wraps past the dimmed LAUNCH to OPTIONS.
    m.step(up(), 0.0);
    assert_eq!(COMMANDS[m.command], Cmd::Options);
    m.step(down(), 0.0);
    assert_eq!(COMMANDS[m.command], Cmd::Add);
    // ADD -> Planet; Triangle back; ADD again.
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Map);
    assert!(COMMANDS.iter().all(|c| !m.command_live(*c)), "all dimmed");
    assert_eq!(COMMANDS[m.command], Cmd::Add, "ADD keeps its focus");
    assert_eq!(m.info(), "Bespin: Cloud City", "a BF1 map: its name alone");
    m.step(triangle(), 0.0);
    assert_eq!(m.state, State::Command);
    m.step(cross(), 0.0);
    // Planet -> Mode -> Era, Triangle walks back one at a time.
    let hoth = m
        .map_rows()
        .iter()
        .position(|r| r.text == "Hoth (bf2)")
        .unwrap();
    m.map_list.at = hoth;
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Mode);
    assert!(m.info().starts_with("Conquest"));
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Era);
    let eras: Vec<String> = m.side_rows().into_iter().map(|r| r.text).collect();
    assert_eq!(eras[0], "Galactic Civil War", "Hoth has no Clone Wars");
    assert_eq!(eras.len(), 1, "no All Eras for one era, no cross-era pairs");
    assert_eq!(m.info(), "Galactic Civil War");
    m.step(triangle(), 0.0);
    assert_eq!(m.state, State::Mode);
    m.step(triangle(), 0.0);
    assert_eq!(m.state, State::Map);
    m.step(cross(), 0.0);
    m.step(cross(), 0.0);
    // Cross on the era adds the row, back to the stack on LAUNCH.
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Command);
    assert_eq!(COMMANDS[m.command], Cmd::Launch);
    assert_eq!(m.playlist.len(), 1);
    assert_eq!(m.play_rows()[1].text, "Hoth (bf2)");
    assert_eq!(m.play_rows()[1].icon.as_deref(), Some("mode_icon_con"));
    assert_eq!(m.play_rows()[1].icon2, Some("imp_icon"));
    // REMOVE -> the playlist on "Remove All" (info empty), Down, Cross
    // deletes Hoth; with only Remove All left the focus is ADD.
    m.step(down(), 0.0);
    m.step(down(), 0.0);
    assert_eq!(COMMANDS[m.command], Cmd::Remove);
    m.step(cross(), 0.0);
    assert_eq!(m.state, State::Playlist);
    m.play_list.at = 0;
    assert_eq!(m.info(), "");
    m.step(down(), 0.0);
    assert_eq!(m.info(), "Hoth (bf2)");
    m.step(cross(), 0.0);
    assert!(m.playlist.is_empty());
    assert_eq!(m.state, State::Command);
    assert_eq!(COMMANDS[m.command], Cmd::Add);
    // Triangle in the command state leaves for Singleplayer; Cross on a
    // dimmed LAUNCH does nothing.
    m.command = 0;
    assert_eq!(m.step(cross(), 0.0), None);
    assert_eq!(m.step(triangle(), 0.0), Some(IaCommand::Back));
}

#[test]
fn remove_all_clears_and_removing_keeps_the_rest() {
    let mut m = ia();
    for i in [2, 3, 4] {
        m.add(
            MapPick::One(i),
            ModePick::One(Mode::Con),
            SidePick::One(Side::Era(Era::Cw)),
        );
    }
    assert_eq!(m.playlist.len(), 3);
    m.focus_command(Cmd::Remove);
    m.step(cross(), 0.0);
    m.play_list.at = 2;
    m.step(cross(), 0.0);
    let left: Vec<usize> = m.playlist.iter().map(|e| e.map).collect();
    assert_eq!(left, [2, 4]);
    assert_eq!(COMMANDS[m.command], Cmd::Launch);
    m.focus_command(Cmd::Remove);
    m.step(cross(), 0.0);
    m.play_list.at = 0;
    m.step(cross(), 0.0);
    assert!(m.playlist.is_empty(), "Remove All");
}

/// §1.4: All Maps / All Modes / All Eras add what the port runs, eras
/// outer and modes inner; the list stops at 128 and says so.
#[test]
fn all_maps_modes_eras_and_the_cap() {
    let mut m = ia();
    let hoth = m
        .maps
        .iter()
        .position(|e| e.label == "Kashyyyk (bf2)")
        .unwrap();
    m.add(MapPick::One(hoth), ModePick::All, SidePick::AllEras);
    let sides: Vec<Side> = m.playlist.iter().map(|e| e.side).collect();
    assert_eq!(sides, [Side::Era(Era::Cw), Side::Era(Era::Gcw)]);
    assert!(m.playlist.iter().all(|e| e.mode == Mode::Con));
    m.playlist.clear();
    m.add(MapPick::All, ModePick::All, SidePick::AllEras);
    // 34 baked maps; geo1, hot1, end1 (both games), kam1 (bf1) have one era.
    let one_era = ["Endor (bf2)", "Endor: Bunker (bf1)", "Geonosis (bf2)"];
    assert!(one_era.iter().all(|l| m.maps.iter().any(|e| e.label == *l)));
    let expected: usize = m
        .maps
        .iter()
        .filter(|e| e.baked)
        .map(|e| ERAS.iter().filter(|x| e.has_era(**x)).count())
        .sum();
    assert_eq!(m.playlist.len(), expected);
    assert!(!m.full);
    // The order: map by map in list order, Clone Wars before Galactic
    // Civil War.
    assert_eq!(m.maps[m.playlist[0].map].label, "Bespin: Cloud City (bf1)");
    assert_eq!(m.playlist[0].side, Side::Era(Era::Cw));
    assert_eq!(m.playlist[1].side, Side::Era(Era::Gcw));
    // Twice more: 128 rows, then the popup; Cross closes it.
    m.add(MapPick::All, ModePick::All, SidePick::AllEras);
    m.add(MapPick::All, ModePick::All, SidePick::AllEras);
    assert_eq!(m.playlist.len(), MAX_MISSIONS);
    assert!(m.full);
    assert_eq!(m.texts.list_full, "Selected maplist is full.");
    m.step(cross(), 0.0);
    assert!(!m.full);
}

/// §2.3: every option's default, step, L1/R1 ×10 and clamp.
#[test]
fn option_ranges_steps_and_defaults() {
    let mut o = SessionOptions::default();
    assert_eq!(
        (
            o.num_bots,
            o.difficulty,
            o.con_numbots,
            o.con_mult,
            o.con_timer
        ),
        (100, 2, 100, 100, 0)
    );
    assert_eq!(
        (o.ctf_score, o.ctf_timer, o.hun_timer, o.hun_score),
        (5, 0, 5, HUNT_SCORE_PER_MAP)
    );
    assert_eq!(o.ass_score, 180);
    o.adjust(Opt::NumBots, 1, 10);
    assert_eq!(o.num_bots, 200);
    assert_eq!(o.con_numbots, 200);
    assert_eq!((o.ctf_numbots, o.ass_numbots), (16, 16));
    o.adjust(Opt::NumBots, -1, 100);
    assert_eq!(o.num_bots, 10);
    o.adjust(Opt::AiDifficulty, 1, 1);
    assert_eq!(o.difficulty, 3);
    o.adjust(Opt::AiDifficulty, -1, 10);
    assert_eq!(o.difficulty, 1, "clamped at Easy");
    o.adjust(Opt::AiDifficulty, 1, 10);
    assert_eq!(o.difficulty, DIFFICULTY_LEVELS, "clamped at Elite");
    o.adjust(Opt::ConMult, 1, 1);
    assert_eq!(o.con_mult, 110);
    o.adjust(Opt::ConMult, 1, 10);
    assert_eq!(o.con_mult, 210);
    for _ in 0..5 {
        o.adjust(Opt::ConMult, 1, 10);
    }
    assert_eq!(o.con_mult, 500);
    for _ in 0..6 {
        o.adjust(Opt::ConMult, -1, 10);
    }
    assert_eq!(o.con_mult, 10);
    o.adjust(Opt::ConTimer, 1, 1);
    assert_eq!(o.con_timer, 5);
    o.adjust(Opt::ConTimer, 1, 10);
    assert_eq!(o.con_timer, 55);
    o.adjust(Opt::ConTimer, 1, 1);
    o.adjust(Opt::ConTimer, 1, 1);
    assert_eq!(o.con_timer, 60);
    o.adjust(Opt::HunTimer, -1, 10);
    assert_eq!(o.hun_timer, 5, "Hunt's timer has no Off");
    o.adjust(Opt::HunScore, 1, 10);
    assert_eq!(o.hun_score, 100);
    o.adjust(Opt::HunScore, -1, 10);
    assert_eq!(o.hun_score, HUNT_SCORE_PER_MAP, "Per map is below 10");
    o.adjust(Opt::AssScore, -1, 10);
    assert_eq!(o.ass_score, 80);
    o.adjust(Opt::AssScore, -1, 10);
    assert_eq!(o.ass_score, 50);
    o.adjust(Opt::CtfScore, 1, 10);
    assert_eq!(o.ctf_score, 15);
    o.adjust(Opt::HeroAssign, 1, 1);
    assert_eq!(o.hero_assign, 4);
    o.adjust(Opt::HeroAssign, 1, 10);
    assert_eq!(o.hero_assign, 7);
    o.hero_respawn = 120;
    o.adjust(Opt::HeroRespawn, 1, 1);
    assert_eq!(o.hero_respawn, -1, "121 wraps to Never");
    o.adjust(Opt::HeroUnlock, 1, 1);
    assert_eq!(o.hero_unlock, 5);
    assert_eq!(o.rows(Group::Hero).len(), 7, "Heroes On");
    o.adjust(Opt::HeroesOnOff, 1, 1);
    assert_eq!(o.rows(Group::Hero), [Opt::HeroesOnOff, Opt::DlcHeroes]);
}

#[test]
fn dlc_heroes_stay_visible_but_cannot_be_changed_without_the_bake() {
    let texts = IaTexts::new(&|_, english| english.to_owned());
    let options = SessionOptions::default();
    let row = Opt::DlcHeroes;
    let mut ia = InstantAction::new(Vec::new(), texts, options);
    ia.page = Page::Group(Group::Hero, ListCursor::default());
    assert_eq!(ia.options.rows(Group::Hero)[1], row);
    assert!(!ia.options.option_enabled(row));
    assert_eq!(
        ia.texts.option(row, &ia.options),
        [Piece::Text("DLC Heroes: Off".into())]
    );
    assert!(!ia.focus_pointer(PointerTarget::Option(1, true)));
    ia.step(
        IaInput {
            menu: crate::menu::MenuInput {
                down: true,
                ..Default::default()
            },
            ..Default::default()
        },
        0.0,
    );
    assert_eq!(ia.page, Page::Group(Group::Hero, ListCursor::new(2)));
    ia.options.adjust(row, 1, 1);
    assert!(!ia.options.dlc_heroes);
    ia.options.adjust(Opt::HeroesOnOff, 1, 1);
    assert_eq!(ia.options.rows(Group::Hero), [Opt::HeroesOnOff, row]);
}

/// Supported modes use bounded percentages; unsupported modes keep raw counts.
#[test]
fn population_percent_and_unsupported_mode_counts_have_distinct_bounds() {
    assert_eq!(MAX_BOTS, 16);
    let mut o = SessionOptions::default();
    for opt in [Opt::NumBots, Opt::ConNumBots] {
        o.adjust(opt, 1, 100);
        assert_eq!(o.con_numbots, crate::population::MAX_TEAM_PCT as i32);
        o.adjust(opt, -1, 100);
        assert_eq!(o.con_numbots, 10);
    }
    for opt in [Opt::CtfNumBots, Opt::AssNumBots] {
        o.adjust(opt, 1, 100);
        assert_eq!((o.ctf_numbots, o.ass_numbots), (16, 16));
    }
}

/// menuref "Session Options" Hero rows (`so_hero`, `so_hero_ranges`,
/// `sheets/hero_all_steps.png`): the defaults and every range.
#[test]
fn hero_defaults_and_ranges() {
    let t = IaTexts::new(&|_, e| e.to_owned());
    let flat = |p: Vec<Piece>| -> String {
        p.into_iter()
            .map(|x| match x {
                Piece::Text(s) => s,
                Piece::Left => "<".into(),
                Piece::Right => ">".into(),
            })
            .collect()
    };
    let mut o = SessionOptions::default();
    // [ours] Heroes On by default (maintainer 2026-10-30; the PS2's is Off).
    assert!(o.heroes, "Heroes On by default");
    o.adjust(Opt::HeroesOnOff, 1, 1);
    assert!(!o.heroes);
    assert_eq!(o.rows(Group::Hero), [Opt::HeroesOnOff, Opt::DlcHeroes]);
    assert_eq!(flat(t.option(Opt::HeroesOnOff, &o)), "Heroes: < Off >");
    o.adjust(Opt::HeroesOnOff, 1, 1);
    let rows: Vec<String> = o
        .rows(Group::Hero)
        .into_iter()
        .map(|r| flat(t.option(r, &o)))
        .collect();
    assert_eq!(
        rows,
        [
            "Heroes: < On >",
            "DLC Heroes: Off",
            "AI heroes: < Off >",
            "Unlock Hero: < Points >",
            "First to < 10 > points",
            "Assign To: < Best Player >",
            "Timer: < 90 > seconds",
        ]
    );
    // Unlock: Points <-> Timer only (Right, Right, Left = Timer, Points,
    // Timer); one shared value.
    let mut seen = Vec::new();
    for dir in [1, 1, -1] {
        o.adjust(Opt::HeroUnlock, dir, 1);
        seen.push(o.hero_unlock);
    }
    assert_eq!(seen, [5, 3, 5]);
    assert_eq!(flat(t.option(Opt::HeroUnlockVal, &o)), "< 10 > seconds");
    // The value: 1..120, L1/R1 10 steps, clamped.
    for _ in 0..12 {
        o.adjust(Opt::HeroUnlockVal, 1, 10);
    }
    assert_eq!(o.hero_unlock_val, 120);
    o.adjust(Opt::HeroUnlockVal, 1, 1);
    assert_eq!(o.hero_unlock_val, 120);
    for _ in 0..13 {
        o.adjust(Opt::HeroUnlockVal, -1, 10);
    }
    assert_eq!(o.hero_unlock_val, 1);
    // Assign: Best -> Worst -> Random, no wrap; Left Random -> Worst.
    let mut seen = Vec::new();
    for dir in [1, 1, 1, -1] {
        o.adjust(Opt::HeroAssign, dir, 1);
        seen.push(o.hero_assign);
    }
    assert_eq!(seen, [4, 7, 7, 4]);
    // Timer: 90, Always (0) .. 120, then Never; Never stays, Left -> 120.
    assert_eq!(o.hero_respawn, 90);
    for _ in 0..10 {
        o.adjust(Opt::HeroRespawn, -1, 10);
    }
    assert_eq!(o.hero_respawn, 0);
    assert_eq!(flat(t.option(Opt::HeroRespawn, &o)), "Timer: < Always >");
    o.adjust(Opt::HeroRespawn, -1, 1);
    assert_eq!(o.hero_respawn, 0, "Left at Always stays");
    o.hero_respawn = 120;
    o.adjust(Opt::HeroRespawn, 1, 1);
    assert_eq!(o.hero_respawn, -1);
    assert_eq!(flat(t.option(Opt::HeroRespawn, &o)), "Timer: < Never >");
    o.adjust(Opt::HeroRespawn, 1, 1);
    assert_eq!(o.hero_respawn, -1, "Right at Never stays");
    o.adjust(Opt::HeroRespawn, -1, 1);
    assert_eq!(o.hero_respawn, 120);
}

/// menuref "Session Options": every value press clicks select, even at a
/// clamp; Up/Down select, silent at the ends; Cross nothing; Triangle one
/// exit. Overview: moves select, Cross accept.
#[test]
fn session_options_clicks() {
    use crate::shell_feel::MenuSound as S;
    let mut m = ia();
    m.focus_command(Cmd::Options);
    m.step(cross(), 0.0);
    assert_eq!(m.take_clicks(), [S::Accept]);
    m.step(down(), 0.0);
    m.step(press(|i| i.menu.left = true), 0.0);
    assert_eq!(
        m.take_clicks(),
        [S::Select],
        "Left on the overview: nothing"
    );
    m.step(up(), 0.0);
    m.step(cross(), 0.0);
    assert_eq!(m.take_clicks(), [S::Select, S::Accept]);
    // Global: percentage steps by 10; R1 advances 100.
    m.step(press(|i| i.menu.right = true), 0.0);
    m.step(press(|i| i.r1 = true), 0.0);
    assert_eq!(m.options.num_bots, crate::population::MAX_TEAM_PCT as i32);
    assert_eq!(m.take_clicks(), [S::Select, S::Select]);
    m.step(up(), 0.0);
    assert!(m.take_clicks().is_empty(), "silent at the first row");
    m.step(down(), 0.0);
    m.step(down(), 0.0);
    assert_eq!(m.take_clicks(), [S::Select], "silent at the last row");
    m.step(cross(), 0.0);
    assert!(m.take_clicks().is_empty(), "Cross does nothing");
    m.step(triangle(), 0.0);
    m.step(triangle(), 0.0);
    assert_eq!(m.take_clicks(), [S::Exit, S::Exit]);
}

/// menuref "Instant Action" and the sound table: stack moves click (wrap
/// too), Left/Right/L1/R1 nothing in the command state; a list click only
/// when the cursor moves; Triangle back from the stack is the
/// Singleplayer's re-Enter (no click here); Cross on "Remove All" error.
#[test]
fn instant_action_clicks() {
    use crate::shell_feel::MenuSound as S;
    let mut m = ia();
    m.step(up(), 0.0);
    m.step(down(), 0.0);
    let presses: [fn(&mut IaInput); 4] = [
        |i: &mut IaInput| i.menu.left = true,
        |i: &mut IaInput| i.menu.right = true,
        |i: &mut IaInput| i.l1 = true,
        |i: &mut IaInput| i.r1 = true,
    ];
    for f in presses {
        m.step(press(f), 0.0);
    }
    assert_eq!(m.take_clicks(), [S::Select, S::Select]);
    m.step(cross(), 0.0);
    assert_eq!(m.take_clicks(), [S::Accept]);
    m.step(up(), 0.0);
    assert_eq!(m.take_clicks(), [S::Select], "Up wraps to the last map row");
    assert_eq!(m.map_list.at, m.map_rows().len() - 1);
    m.step(down(), 0.0);
    m.step(press(|i| i.r1 = true), 0.0);
    assert_eq!(m.take_clicks(), [S::Select, S::Select]);
    m.step(triangle(), 0.0);
    assert_eq!(m.take_clicks(), [S::Exit]);
    assert_eq!(m.step(triangle(), 0.0), Some(IaCommand::Back));
    assert!(m.take_clicks().is_empty());
    m.add(
        MapPick::One(2),
        ModePick::One(Mode::Con),
        SidePick::One(Side::Era(Era::Cw)),
    );
    m.focus_command(Cmd::Remove);
    m.step(cross(), 0.0);
    m.play_list.at = 0;
    m.take_clicks();
    m.step(cross(), 0.0);
    assert_eq!(m.take_clicks(), [S::Error]);
    assert!(m.playlist.is_empty());
    // The full popup: closed by Triangle with accept.
    m.full = true;
    m.step(triangle(), 0.0);
    assert_eq!(m.take_clicks(), [S::Accept]);
    assert!(!m.full);
}

/// menuref §9.1.11 (`ia_list_scroll`, `ia_list_page`): on the PS2's 25
/// Planet rows Down scrolls one row per press once the list must move;
/// R1 / L1 page 8 rows (0 -> 8 -> 16 -> 24), not moving at the end. REMOVE
/// puts the cursor on the playlist's last row. `[ours]` (`crate::scroll`):
/// the list starts moving a row before the window's edge (the PS2 waits for
/// the last visible row), so the next row is always in sight.
#[test]
fn lists_scroll_one_row_and_page_eight() {
    let live = [true; 25];
    let mut c = ListCursor::default();
    for k in 1..LIST_ROWS - 1 {
        assert!(c.step(&live, 1, LIST_ROWS));
        assert_eq!((c.at, c.top()), (k, 0));
    }
    c.step(&live, 1, LIST_ROWS);
    assert_eq!((c.at, c.top()), (LIST_ROWS - 1, 1));
    c.step(&live, 1, LIST_ROWS);
    assert_eq!((c.at, c.top()), (LIST_ROWS, 2));
    let page = LIST_PAGE as isize;
    let mut c = ListCursor::default();
    let mut at = Vec::new();
    for _ in 0..4 {
        let moved = c.step(&live, page, LIST_ROWS);
        at.push((c.at, moved));
    }
    assert_eq!(at, [(8, true), (16, true), (24, true), (24, false)]);
    assert_eq!(c.top(), 25 - LIST_ROWS);
    let mut back = Vec::new();
    for _ in 0..3 {
        c.step(&live, -page, LIST_ROWS);
        back.push(c.at);
    }
    assert_eq!(back, [16, 8, 0]);
    // The same through the screen's R1.
    let mut m = ia();
    m.step(cross(), 0.0);
    m.step(press(|i| i.r1 = true), 0.0);
    assert_eq!(m.map_list.at, LIST_PAGE);
    // The playlist: entered on its last row.
    let mut m = ia();
    for i in [2, 3, 4] {
        m.add(
            MapPick::One(i),
            ModePick::One(Mode::Con),
            SidePick::One(Side::Era(Era::Cw)),
        );
    }
    m.play_list = ListCursor::default();
    m.focus_command(Cmd::Remove);
    m.step(cross(), 0.0);
    assert_eq!(m.play_list.at, 3);
}

#[test]
fn list_scrolling_moves_top_without_moving_focus() {
    let mut c = ListCursor::new(0);
    // 25 rows, 9 shown -> max top is 16
    assert!(!c.scroll(25, LIST_ROWS, -1.0), "cannot scroll above 0");
    assert_eq!((c.at, c.top()), (0, 0));

    assert!(c.scroll(25, LIST_ROWS, 1.0));
    assert_eq!(
        (c.at, c.top()),
        (0, 1),
        "scrolling down increments top without moving at"
    );

    assert!(c.scroll(25, LIST_ROWS, 3.0));
    assert_eq!((c.at, c.top()), (0, 4));

    // Scroll to end
    assert!(c.scroll(25, LIST_ROWS, 20.0));
    assert_eq!((c.at, c.top()), (0, 16), "clamped to max top (25 - 9)");
    assert!(!c.scroll(25, LIST_ROWS, 1.0), "cannot scroll past max top");

    // Scroll back up
    assert!(c.scroll(25, LIST_ROWS, -2.0));
    assert_eq!((c.at, c.top()), (0, 14));

    // InstantAction scroll_left and scroll_pointer
    let mut m = ia();
    assert_eq!((m.map_list.at, m.map_list.top()), (0, 0));
    assert!(m.scroll_left(1.0));
    assert_eq!(
        (m.map_list.at, m.map_list.top()),
        (0, 1),
        "scroll_left advances top without moving at"
    );
    assert_eq!(m.take_clicks(), vec![MenuSound::Select]);

    // scroll_pointer over right window scrolls playlist
    for i in 0..15 {
        m.playlist.push(Entry {
            map: i % m.maps.len(),
            mode: Mode::Con,
            side: Side::Era(Era::Cw),
        });
    }
    assert_eq!(m.play_list.top(), 0);
    let size = [640.0, 480.0];
    let pt_right = [400.0 * (640.0 / 512.0), 200.0];
    assert!(m.scroll_pointer(Some(pt_right), Some(size), 2.0, true));
    assert_eq!(
        m.play_list.top(),
        2,
        "scroll_pointer over right window scrolls playlist"
    );
    assert_eq!(m.map_list.top(), 1, "left window untouched");

    // scroll_pointer over left window scrolls map list
    let pt_left = [100.0 * (640.0 / 512.0), 200.0];
    assert!(m.scroll_pointer(Some(pt_left), Some(size), -1.0, true));
    assert_eq!(
        m.map_list.top(),
        0,
        "scroll_pointer over left window scrolls map list"
    );
    assert_eq!(m.map_list.at, 0, "focus unchanged without selecting");
    assert!(!m.scroll_pointer(Some([0.0, 0.0]), Some(size), 1.0, true));
    assert_eq!((m.map_list.top(), m.play_list.top()), (0, 2));
    assert!(m.scroll_pointer(Some([0.0, 0.0]), Some(size), 1.0, false));
    assert_eq!(
        m.map_list.top(),
        1,
        "a mouse wheel still scrolls the focused list"
    );
}

/// A trackpad sends fractions of a row: on an option group they add up
/// (review: they were dropped until a step crossed a row).
#[test]
fn option_groups_keep_a_trackpads_fractions_of_a_row() {
    let mut m = ia();
    let group = [
        Group::Global,
        Group::Hero,
        Group::Conquest,
        Group::Ctf,
        Group::Hunt,
        Group::Assault,
    ]
    .into_iter()
    .find(|g| m.options.rows(*g).len() >= OPTION_ROWS + 2)
    .expect("a group longer than its window");
    m.page = Page::Group(group, ListCursor::default());
    for _ in 0..20 {
        m.scroll_option(0.1);
    }
    let Page::Group(_, cur) = m.page else {
        panic!("still on the group");
    };
    assert!(
        (cur.view.target() - 2.0).abs() < 1e-4,
        "{}",
        cur.view.target()
    );
    assert_eq!(cur.at, 0, "the selection stays");
    assert_eq!(m.take_clicks().len(), 2, "one click per row crossed");
}

/// The maintainer, 2026-10-28 (a named deviation from the PS2's Normal <-> Elite
/// toggle): the AI Difficulty row offers exactly the port's four tiers, in
/// order, a step per press from the lowest to the highest.
#[test]
fn the_ai_difficulty_row_offers_exactly_four_tiers() {
    let t = IaTexts::new(&|_, e| e.to_owned());
    let mut o = SessionOptions {
        difficulty: 1,
        ..SessionOptions::default()
    };
    let name = |o: &SessionOptions| -> String {
        t.option(Opt::AiDifficulty, o)
            .into_iter()
            .map(|x| match x {
                Piece::Text(s) => s,
                Piece::Left => "<".into(),
                Piece::Right => ">".into(),
            })
            .collect()
    };
    let mut seen = vec![name(&o)];
    for _ in 0..6 {
        o.adjust(Opt::AiDifficulty, 1, 1);
        let n = name(&o);
        if seen.last() != Some(&n) {
            seen.push(n);
        }
    }
    assert_eq!(
        seen,
        [
            "AI Difficulty Level: < Easy >",
            "AI Difficulty Level: < Normal >",
            "AI Difficulty Level: < Hard >",
            "AI Difficulty Level: < Elite >",
        ]
    );
    assert_eq!(DIFFICULTY_LEVELS, 4);
}

/// menuref §9.1.5: the session options outlive leaving the screen.
#[test]
fn options_persist_across_leaving_instant_action() {
    let mut m = ia();
    m.options.adjust(Opt::NumBots, -1, 10);
    m.options.adjust(Opt::ConMult, 1, 5);
    let kept = m.options;
    assert_eq!(m.step(triangle(), 0.0), Some(IaCommand::Back));
    m.enter();
    assert_eq!(m.options, kept);
}

#[test]
fn option_rows_read_as_the_disc_writes_them() {
    let t = IaTexts::new(&|_, e| e.to_owned());
    let o = SessionOptions::default();
    let flat = |p: Vec<Piece>| -> String {
        p.into_iter()
            .map(|x| match x {
                Piece::Text(s) => s,
                Piece::Left => "<".into(),
                Piece::Right => ">".into(),
            })
            .collect()
    };
    assert_eq!(flat(t.option(Opt::NumBots, &o)), "Team Size: < 100% >");
    assert_eq!(
        flat(t.option(Opt::AiDifficulty, &o)),
        "AI Difficulty Level: < Normal >"
    );
    assert_eq!(flat(t.option(Opt::ConMult, &o)), "Reinforcements: < 100% >");
    assert_eq!(flat(t.option(Opt::ConTimer, &o)), "Timer: < Off >");
    let mut o2 = o;
    o2.hero_respawn = 1;
    assert_eq!(flat(t.option(Opt::HeroRespawn, &o2)), "Timer: < 1 > second");
    // The score rows show the limit each mode plays to.
    assert_eq!(
        flat(t.option(Opt::HunScore, &o)),
        "Score Limit: < Per map >"
    );
    assert_eq!(flat(t.option(Opt::AssScore, &o)), "Score Limit: < 180 >");
    o2.hun_score = 70;
    o2.ass_score = 360;
    assert_eq!(flat(t.option(Opt::HunScore, &o2)), "Score Limit: < 70 >");
    assert_eq!(flat(t.option(Opt::AssScore, &o2)), "Score Limit: < 360 >");
    assert_eq!(
        t.help(Opt::ConNumBots),
        "Set the number of AI units per team for all Conquest matches in your session."
    );
}

/// §2.1-2.2: OPTIONS opens the overview (CTF dimmed), a group page adjusts with Left/Right and L1/R1, Triangle
/// walks back and asks for the prefs to be kept.
#[test]
fn session_options_pages() {
    let mut m = ia();
    m.focus_command(Cmd::Options);
    m.step(cross(), 0.0);
    assert_eq!(m.page, Page::Overview(0));
    let live: Vec<bool> = m.overview().iter().map(|(_, l)| *l).collect();
    assert_eq!(live, [true, true, true, false, true, true]);
    m.step(up(), 0.0);
    assert_eq!(m.page, Page::Overview(5), "wraps to Assault");
    m.step(cross(), 0.0);
    assert!(matches!(m.page, Page::Group(Group::Assault, _)));
    m.step(down(), 0.0);
    m.step(press(|i| i.menu.right = true), 0.0);
    assert_eq!(m.options.ass_score, 190);
    assert_eq!(m.step(triangle(), 0.0), Some(IaCommand::Prefs));
    m.step(down(), 0.0);
    m.step(down(), 0.0);
    assert_eq!(m.page, Page::Overview(1), "Hero options are selectable");
    m.step(cross(), 0.0);
    m.step(down(), 0.0);
    m.step(press(|i| i.menu.right = true), 0.0);
    assert!(m.options.ai_heroes);
    assert_eq!(m.step(triangle(), 0.0), Some(IaCommand::Prefs));
    m.step(down(), 0.0);
    assert_eq!(m.page, Page::Overview(2));
    m.step(cross(), 0.0);
    assert!(matches!(m.page, Page::Group(Group::Conquest, _)));
    m.step(down(), 0.0);
    m.step(press(|i| i.menu.right = true), 0.0);
    m.step(press(|i| i.r1 = true), 0.0);
    assert_eq!(m.options.con_mult, 210);
    m.step(press(|i| i.l1 = true), 0.0);
    m.step(press(|i| i.menu.left = true), 0.0);
    assert_eq!(m.options.con_mult, 100);
    m.step(cross(), 0.0);
    assert!(matches!(m.page, Page::Group(..)), "Cross does nothing");
    assert_eq!(m.step(triangle(), 0.0), Some(IaCommand::Prefs));
    assert_eq!(m.page, Page::Overview(2));
    m.step(triangle(), 0.0);
    assert_eq!(m.page, Page::Missions);
    assert_eq!(m.state, State::Command);
}

/// §1.4, §7: LAUNCH hands the whole playlist over in order, map, mode and
/// armies per row; L1 / R1 page the lists.
#[test]
fn launch_hands_over_the_playlist_in_order() {
    let mut m = ia();
    let at = |l: &str| m.maps.iter().position(|e| e.label == l).unwrap();
    let (hoth, kam) = (at("Hoth (bf2)"), at("Kamino: Tipoca City (bf1)"));
    m.add(
        MapPick::One(hoth),
        ModePick::One(Mode::Con),
        SidePick::One(Side::Era(Era::Gcw)),
    );
    m.add(
        MapPick::One(kam),
        ModePick::One(Mode::Con),
        SidePick::One(Side::Mixed(1)),
    );
    m.focus_command(Cmd::Launch);
    let Some(IaCommand::Launch(q)) = m.step(cross(), 0.0) else {
        panic!("no launch");
    };
    assert_eq!(q.len(), 2);
    assert_eq!(q[0].key, "bf2_hot1");
    assert_eq!(q[0].armies, ["all".to_owned(), "imp".to_owned()]);
    assert_eq!(q[1].key, "bf1_kam1");
    assert_eq!(q[1].game, Game::Bf1);
    assert_eq!(q[1].armies, ["all".to_owned(), "cis".to_owned()]);
    assert!(q.iter().all(|e| e.mode == Mode::Con));
    // Paging: R1 moves a page down, the view follows.
    let mut m = ia();
    m.step(cross(), 0.0);
    m.step(press(|i| i.r1 = true), 0.0);
    m.step(press(|i| i.r1 = true), 0.0);
    assert_eq!(m.map_list.at, 2 * LIST_PAGE);
    // The view keeps a row of margin below the cursor (`crate::scroll`).
    assert_eq!(m.map_list.top(), 2 * LIST_PAGE + 2 - LIST_ROWS);
    m.step(press(|i| i.l1 = true), 0.0);
    m.step(press(|i| i.l1 = true), 0.0);
    assert_eq!(m.map_list.at, 0);
}

#[test]
fn hunt_queues_the_native_matchup_from_the_selected_map() {
    let bakes = [
        ("bf1_end1".into(), "bf1".into()),
        ("bf2_end1".into(), "bf2".into()),
    ];
    let maps = catalog(&text, &bakes, &|key, mode| {
        if key.ends_with("_end1") && mode == Mode::Hunt {
            vec![["imp".into(), "ewk".into()]]
        } else {
            Vec::new()
        }
    });
    let mut shell = InstantAction::new(
        maps,
        IaTexts::new(&|_, fallback| fallback.to_owned()),
        SessionOptions::default(),
    );
    let mode = ModePick::One(Mode::Hunt);
    let side = Side::Era(Era::Gcw);
    for game in [Game::Bf1, Game::Bf2] {
        let index = shell
            .maps
            .iter()
            .position(|map| map.game == game && map.key.ends_with("_end1"))
            .unwrap();
        let map = &shell.maps[index];
        assert!(map.runnable(&Mode::Hunt, side));
        assert!(!map.runnable(&Mode::Hunt, Side::Era(Era::Cw)));
        shell.add(MapPick::One(index), mode.clone(), SidePick::One(side));
    }
    shell.focus_command(Cmd::Launch);
    let Some(IaCommand::Launch(playlist)) = shell.step(cross(), 0.0) else {
        panic!("no Hunt launch");
    };
    assert_eq!(playlist.len(), 2);
    for launch in playlist {
        assert_eq!(launch.mode, Mode::Hunt);
        assert_eq!(launch.armies, ["imp".to_owned(), "ewk".to_owned()]);
    }
}

#[test]
fn population_rows_stop_at_the_percent_and_help_averages_every_map() {
    use crate::population::{Defaults, Entry};
    static TABLE: [Entry; 2] = [
        (
            "a",
            false,
            Defaults {
                combatants: [16, 16],
                tickets: [100, 100],
            },
        ),
        (
            "b",
            true,
            Defaults {
                combatants: [31, 25],
                tickets: [200, 220],
            },
        ),
    ];
    let texts = IaTexts::new(&|_, english| english.to_owned());
    let mut values = SessionOptions {
        population_table: &TABLE,
        con_mult: 125,
        ..Default::default()
    };
    let flat = |pieces: Vec<Piece>| {
        pieces
            .into_iter()
            .map(|p| match p {
                Piece::Text(s) => s,
                Piece::Left => "<".into(),
                Piece::Right => ">".into(),
            })
            .collect::<String>()
    };
    let help = |v: &SessionOptions, o| v.population_help(o).unwrap();
    for o in [Opt::NumBots, Opt::ConNumBots] {
        assert_eq!(flat(texts.option(o, &values)), "Team Size: < 100% >");
    }
    assert_eq!(
        flat(texts.option(Opt::ConMult, &values)),
        "Reinforcements: < 125% >"
    );
    let minutes = crate::population::match_minutes(100, 125).round();
    let length = format!("Average match length: {minutes} minutes");
    assert_eq!(
        help(&values, Opt::ConNumBots),
        format!("Average team size: 22 units\n{length}")
    );
    // 125% of 100/100 and of 200/220: 125 and 262.5 per side, averaged 193.75.
    assert_eq!(
        help(&values, Opt::ConMult),
        format!("Average reinforcements: 194 units\n{length}")
    );
    assert_eq!(values.population_help(Opt::ConTimer), None);
    // The value moves the averages: Right on the Global row moves both rows.
    values.adjust(Opt::NumBots, 1, 10);
    assert_eq!(values.con_numbots, 200);
    assert_eq!(
        flat(texts.option(Opt::ConNumBots, &values)),
        "Team Size: < 200% >"
    );
    assert!(help(&values, Opt::ConNumBots).starts_with("Average team size: 31 units\n"));
    assert!(help(&values, Opt::NumBots).starts_with("Average team size: 31 units\n"));
}

#[test]
fn hero_assault_needs_a_mission_and_a_single_era() {
    let bakes = [
        ("tat2".to_owned(), "bf2".to_owned()),
        ("bf2_cor1".to_owned(), "bf2".to_owned()),
    ];
    let pair = || vec![["all".to_owned(), "imp".to_owned()]];
    let with_coruscant = |key: &str, mode: Mode| {
        if mode == Mode::Eli && (key == "tat2" || key == "bf2_cor1") {
            pair()
        } else {
            Vec::new()
        }
    };
    let without_coruscant = |key: &str, mode: Mode| {
        if mode == Mode::Eli && key == "tat2" {
            pair()
        } else {
            Vec::new()
        }
    };
    let list = catalog(&text, &bakes, &without_coruscant);
    let map = |list: &[MapEntry], key: &str| list.iter().find(|m| m.key == key).unwrap().clone();
    let tat2 = map(&list, "tat2");
    assert!(tat2.runnable(&Mode::Eli, Side::Era(Era::Gcw)));
    assert!(!tat2.runnable(&Mode::Eli, Side::Era(Era::Cw)));
    assert!(!tat2.runnable(&Mode::Eli, Side::Mixed(0)));
    assert!(!map(&list, "bf2_cor1").runnable(&Mode::Eli, Side::Era(Era::Gcw)));
    let list = catalog(&text, &bakes, &with_coruscant);
    assert!(map(&list, "bf2_cor1").runnable(&Mode::Eli, Side::Era(Era::Gcw)));
}

/// Issue #172 (playtest): the Assault AI Units row's help is openbf's own,
/// over the localized "space Assault" line, and says what the count means
/// in each Assault mode; the count's range is unchanged.
#[test]
fn assault_ai_units_help_explains_each_assault_mode() {
    let mut o = SessionOptions::default();
    let help = o.population_help(Opt::AssNumBots).unwrap();
    assert_eq!(
        help,
        "Space Assault: up to 16 per side. Hero Assault: one hero per class (8 or 9) \
         on a side with a player; the other side up to 12."
    );
    o.adjust(Opt::AssNumBots, -1, 10);
    assert_eq!(o.ass_numbots, 6);
    let help = o.population_help(Opt::AssNumBots).unwrap();
    assert!(
        help.starts_with("Space Assault: up to 6 per side."),
        "{help}"
    );
    assert!(help.ends_with("the other side up to 6."), "{help}");
    o.dlc_heroes_installed = true;
    o.dlc_heroes = true;
    let help = o.population_help(Opt::AssNumBots).unwrap();
    assert!(help.contains("(9 or 10)"), "{help}");
    assert_eq!(o.population_help(Opt::AssScore), None);
    assert_eq!(o.population_help(Opt::CtfNumBots), None);
}

/// Issue #172: [`HERO_ASSAULT_POOL`] is Mos Eisley Hero Assault's units per
/// team. A public checkout has no missions until `scripts/pack.py` makes
/// them from the player's discs: the test then skips.
#[test]
fn hero_assault_pool_matches_the_mission() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions/tat2_eli.ron");
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("skip: no {path}");
        return;
    };
    let teams = &text[text.find("teams:").expect("teams")..];
    let units: Vec<i32> = teams
        .match_indices("units:")
        .map(|(i, k)| {
            let rest = teams[i + k.len()..].trim_start();
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            rest[..end].parse().expect("units")
        })
        .take(2)
        .collect();
    assert_eq!(
        units,
        [crate::shell::HERO_ASSAULT_POOL; 2],
        "the help's pool follows the mission"
    );
}

#[test]
fn bf2_only_catalog_hides_bf1_maps() {
    let bakes = vec![
        ("tat2".to_owned(), "bf2".to_owned()),
        ("bf2_hot1".to_owned(), "bf2".to_owned()),
    ];
    let entries = catalog(&text, &bakes, &|_, _| Vec::new());
    assert!(!entries.is_empty());
    assert!(
        entries.iter().all(|m| m.game == Game::Bf2),
        "BF2-only bakes should hide unbaked BF1 maps from Instant Action"
    );
}

#[test]
fn catalog_with_mods_supports_two_flag_ctf_and_disambiguates_duplicates() {
    let mod_maps = vec![
        ModMapEntry {
            key: "mod_abr__abr".into(),
            variant_keys: vec!["mod_abr__abr".into()],
            legacy_code: "abr".into(),
            name: "Abregado-Rae: Venezia".into(),
            disambiguator: "ABR".into(),
            source: "Abregado".into(),
            description: "Swamp canal city".into(),
            modes: [vec![Mode::Con, Mode::Ctf], vec![Mode::Con, Mode::Ctf]],
        },
        ModMapEntry {
            key: "mod_abr_v2__abr".into(),
            variant_keys: vec!["mod_abr_v2__abr".into()],
            legacy_code: "abr".into(),
            name: "Abregado-Rae: Venezia".into(),
            disambiguator: "ABR_v2".into(),
            source: "Abregado".into(),
            description: "Remaster".into(),
            modes: [vec![Mode::Ctf], vec![Mode::Ctf]],
        },
    ];
    let bakes = vec![
        ("mod_abr__abr".to_owned(), "bf2".to_owned()),
        ("mod_abr_v2__abr".to_owned(), "bf2".to_owned()),
    ];
    let missions = |key: &str, mode: Mode| {
        if key.starts_with("mod_abr") && matches!(mode, Mode::Con | Mode::Ctf) {
            vec![
                ["all".to_owned(), "imp".to_owned()],
                ["rep".to_owned(), "cis".to_owned()],
            ]
        } else {
            Vec::new()
        }
    };
    let list = catalog_with_mods(&text, &bakes, &missions, &mod_maps);
    let abr1 = list.iter().find(|m| m.key == "mod_abr__abr").unwrap();
    let abr2 = list.iter().find(|m| m.key == "mod_abr_v2__abr").unwrap();
    assert_ne!(abr1.label, abr2.label);
    assert!(abr1.label.contains("(mod)"));
    assert!(abr1.runnable(&Mode::Ctf, Side::Era(Era::Gcw)));
    assert!(abr1.runnable(&Mode::Ctf, Side::Era(Era::Cw)));
    assert!(abr1.runnable(&Mode::Con, Side::Mixed(0)));
}
