use super::*;

/// The fader's walk (`re-hud.md` §2.1): in, hold, hide → sustain → out,
/// zero times skipped, a show during the fade-out mirrors the alpha.
#[test]
fn fader_states_follow_the_ps2() {
    let mut f = Fader {
        fade_in: 1.0,
        fade_out: 0.5,
        ..Fader::default()
    };
    assert_eq!(f.alpha(), 0.0);
    f.show();
    f.advance(0.25);
    assert_eq!((f.state, f.alpha()), (1, 0.25));
    f.advance(1.0);
    assert_eq!((f.state, f.alpha()), (2, 1.0));
    f.advance(1.0e6);
    assert_eq!(f.state, 2, "the default hold is forever");
    f.hide(); // sustain 0 → straight to out
    assert_eq!(f.state, 4);
    f.advance(0.25);
    assert_eq!(f.alpha(), 0.5);
    f.show();
    assert_eq!((f.state, f.alpha()), (1, 0.5));
    f.hide();
    f.advance(10.0);
    assert_eq!((f.state, f.alpha()), (0, 0.0));
    // No times at all: show is instant and holds.
    let mut g = Fader::default();
    g.show();
    assert_eq!((g.state, g.alpha()), (2, 1.0));
    // A finite hold runs out by itself.
    let mut h = Fader {
        hold: 2.5,
        fade_out: 1.0,
        ..Fader::default()
    };
    h.show();
    h.advance(3.0);
    assert_eq!((h.state, h.alpha()), (4, 0.5));
}

fn node(name: &str, args: &[HudArg], children: Vec<HudNode>) -> HudNode {
    HudNode {
        name: name.into(),
        args: args.to_vec(),
        children,
    }
}

#[test]
fn target_hint_and_flag_styles_keep_their_own_slots_and_scales() {
    let s = |v: &str| HudArg::Str(v.into());
    let n = HudArg::Num;
    let style = |name: &str, min: f32, max: f32| {
        node(
            name,
            &[s(name)],
            vec![
                node("colorfriendly", &[n(0.0), n(86.0), n(213.0)], vec![]),
                node("minscale", &[n(min)], vec![]),
                node("maxscale", &[n(max)], vec![]),
            ],
        )
    };
    let layout = HudLayout {
        name: "t".into(),
        nodes: vec![node(
            "Target",
            &[s("player1target")],
            vec![
                style("HintOnScreen", 1.0, 0.5),
                style("HintOnScreenBehind", 1.0, 0.5),
                style("HintOffScreen", 1.0, 0.5),
                style("HintOffScreenBehind", 1.0, 0.5),
                style("HintFlagOnScreen", 1.5, 0.75),
                style("HintFlagOnScreenBehind", 1.5, 0.75),
                style("HintFlagOffScreen", 1.2, 0.9),
                style("HintFlagOffScreenBehind", 1.2, 0.9),
                style("ObjectiveOnScreen", 2.0, 1.0),
                style("ObjectiveOnScreenBehind", 2.0, 1.0),
                style("ObjectiveOffScreen", 2.0, 1.0),
                style("ObjectiveOffScreenBehind", 2.0, 1.0),
                style("FlagOnScreen", 3.0, 2.0),
            ],
        )],
    };
    let hud = Hud::new(&layout, &layout);
    let scales =
        |styles: &[Option<HintStyle>; 4]| styles.map(|s| s.map(|s| (s.min_scale, s.max_scale)));
    assert_eq!(
        scales(&hud.hint_styles),
        [
            Some((1.0, 0.5)),
            Some((1.0, 0.5)),
            Some((1.0, 0.5)),
            Some((1.0, 0.5))
        ]
    );
    assert_eq!(
        scales(&hud.flag_styles),
        [
            Some((1.5, 0.75)),
            Some((1.5, 0.75)),
            Some((1.2, 0.9)),
            Some((1.2, 0.9))
        ]
    );
    assert_eq!(scales(&hud.objective_styles), [Some((2.0, 1.0)); 4]);
    assert_eq!(
        hud.carrier_styles[0].map(|s| (s.min_scale, s.max_scale)),
        Some((3.0, 2.0))
    );
}

/// The 1-player layout's `player1systemMessages`, record for record
/// (`re-event-messages.md` §2), with a font of 8-texel glyphs.
fn message_hud() -> Hud {
    let s = |v: &str| HudArg::Str(v.into());
    let n = HudArg::Num;
    let p = |name: &str, args: &[HudArg]| node(name, args, vec![]);
    let layout = HudLayout {
        name: "t".into(),
        nodes: vec![node(
            "multilinetext",
            &[s("player1systemMessages")],
            vec![
                p("eventtext", &[s("player1.message")]),
                p("eventcolor", &[s("player1.message.color")]),
                p("numlines", &[n(4.0)]),
                p("displaytime", &[n(3.0)]),
                p("alwaysscroll", &[n(1.0)]),
                p("#09771c74", &[n(1.0)]),
                p(
                    "Rect",
                    &[n(0.5), n(0.25), s("Left"), s("Top"), s("Viewport")],
                ),
                p("eventenable", &[s("player1.message")]),
                node(
                    "format",
                    &[s("Format")],
                    vec![
                        p("textbox", &[n(0.5), n(0.25)]),
                        p("textbreak", &[s("Word")]),
                        p("textstyle", &[s("Shadow")]),
                        p("textalignment", &[s("Left"), s("Center")]),
                        p("textfont", &[s("gamefont_tiny")]),
                        p("color", &[n(192.0), n(192.0), n(192.0)]),
                        p("fadeintime", &[n(0.5)]),
                        p("fadeouttime", &[n(0.5)]),
                    ],
                ),
            ],
        )],
    };
    let none = HudLayout {
        name: "none".into(),
        nodes: vec![],
    };
    let mut hud = Hud::new(&layout, &none);
    let glyph = |code: u16| manifest::hud::HudGlyph {
        code,
        page: 0,
        advance: 8,
        x0: 0,
        x1: 8,
        y0: 0,
        y1: 8,
        uv: [0.0; 4],
    };
    let font = HudFont {
        name: "gamefont_tiny".into(),
        line_height: 15,
        pages: vec![],
        glyphs: (32..127).map(glyph).collect(),
    };
    hud.set_fonts(&HashMap::from([("gamefont_tiny".to_string(), font)]));
    hud
}

fn say(hud: &mut Hud, text: &str, color: [u8; 3]) {
    hud.send("player1.message.color", EventValue::Color(color), &|_| {
        false
    });
    hud.send("player1.message", EventValue::Text(text.into()), &|_| false);
}

fn lines(hud: &Hud) -> &MultiLine {
    hud.elems[0].lines.as_deref().expect("a message box")
}

/// Card EVMSG's timing, at the PS2's 30 Hz: a line queued at t = 0 is
/// fully shown 0.534 s later, starts fading 2.969 s after that and is
/// gone 0.534 s after that (PS2 RAM, `re-event-messages.md` §2 [M]) —
/// each within two frames.
#[test]
fn a_message_line_fades_in_holds_and_fades_out_on_the_ps2s_clock() {
    let mut hud = message_hud();
    let ml = lines(&hud);
    assert_eq!((ml.num_lines, ml.display_time), (4, 3.0));
    assert!(ml.always_scroll && ml.stack_down && ml.shadow && ml.word_break);
    assert_eq!(hud.elems[0].font, "gamefont_tiny");
    assert_eq!(hud.elems[0].v_align, VAlign::Center);

    say(&mut hud, "Ben Pulastra killed Player 1", [1, 86, 213]);
    assert_eq!(hud.elems[0].fader.state, 2, "the message shows the box");
    let (mut full, mut fading, mut gone) = (None, None, None);
    for frame in 1..=150u32 {
        hud.advance(1.0 / 30.0);
        let t = frame as f32 / 30.0;
        match lines(&hud).shown.first() {
            Some(l) => {
                assert_eq!(l.color, [1.0, 86.0, 213.0]);
                if l.fader.state == 2 && full.is_none() {
                    full = Some(t);
                }
                if l.fader.state == 4 && fading.is_none() {
                    fading = Some(t);
                }
            }
            None if full.is_some() && gone.is_none() => gone = Some(t),
            None => {}
        }
    }
    let (full, fading, gone) = (full.unwrap(), fading.unwrap(), gone.unwrap());
    println!("full {full:.3} s, fading {fading:.3} s, gone {gone:.3} s");
    let two_frames = 2.0 / 30.0 + 1.0e-4;
    assert!((full - 0.534).abs() <= two_frames, "full at {full}");
    assert!(
        (fading - (0.534 + 2.969)).abs() <= two_frames,
        "fading at {fading}"
    );
    assert!(
        (gone - (0.534 + 2.969 + 0.534)).abs() <= two_frames,
        "gone at {gone}"
    );
    assert_eq!(lines(&hud).state, LineState::Idle);
}

/// A message that wraps past the box's free lines is never cut off: its last
/// lines wait for room and all of it shows, in order.
#[test]
fn a_wrapped_message_is_never_cut_off() {
    let mut hud = message_hud();
    let ml = hud
        .elems
        .iter_mut()
        .find_map(|e| e.lines.as_deref_mut())
        .unwrap();
    ml.add(vec!["one".into(), "two".into(), "three".into()]);
    ml.add(vec!["Ben Pulastra killed".into(), "Player 1".into()]);
    assert_eq!(ml.pending.len(), 5, "both messages whole");
    // The box is over-full now: the next is dropped, as with no free line.
    ml.add(vec!["dropped".into()]);
    assert_eq!(ml.pending.len(), 5);
    let mut seen = Vec::new();
    for _ in 0..600 {
        hud.advance(1.0 / 30.0);
        for l in &lines(&hud).shown {
            if !seen.contains(&l.text) {
                seen.push(l.text.clone());
            }
        }
    }
    assert_eq!(
        seen,
        ["one", "two", "three", "Ben Pulastra killed", "Player 1"]
    );
}

/// Four lines fill the box, a fifth message is dropped; when the top
/// line goes the others move up one line, in order.
#[test]
fn a_full_message_box_drops_the_next_line_and_scrolls_up() {
    let mut hud = message_hud();
    for k in 1..=5 {
        say(&mut hud, &format!("line {k}"), [200, 100, 100]);
    }
    assert_eq!(lines(&hud).pending.len(), 4, "the fifth is dropped");
    // One line is admitted at a time, each under the one before.
    for _ in 0..90 {
        hud.advance(1.0 / 30.0);
    }
    let ml = lines(&hud);
    let shown: Vec<(&str, f32)> = ml.shown.iter().map(|l| (l.text.as_str(), l.y)).collect();
    assert_eq!(
        shown,
        [
            ("line 1", 0.0),
            ("line 2", 1.0),
            ("line 3", 2.0),
            ("line 4", 3.0)
        ]
    );
    // Still full: a message now is dropped too.
    say(&mut hud, "line 6", [200, 100, 100]);
    assert!(lines(&hud).pending.is_empty());
    // The top line expires; the rest end up one line higher.
    let mut scrolled = false;
    for _ in 0..90 {
        hud.advance(1.0 / 30.0);
        let ml = lines(&hud);
        scrolled |= ml.state == LineState::Scrolling;
        if scrolled && ml.state != LineState::Scrolling {
            break;
        }
    }
    assert!(scrolled, "the lines scrolled");
    let ml = lines(&hud);
    let shown: Vec<(&str, f32)> = ml.shown.iter().map(|l| (l.text.as_str(), l.y)).collect();
    assert_eq!(
        shown[..3],
        [("line 2", 0.0), ("line 3", 1.0), ("line 4", 2.0)]
    );
    // With a line free again the next message is taken.
    say(&mut hud, "line 7", [1, 86, 213]);
    let ml = lines(&hud);
    assert_eq!(ml.pending.len() + ml.shown.len(), 4);
    // Left alone, the box empties.
    for _ in 0..900 {
        hud.advance(1.0 / 30.0);
    }
    assert!(lines(&hud).shown.is_empty());
}

/// A message wider than the box breaks at a word (the box is 256 frame
/// pixels: 28 of the test font's glyphs with the shadow's spacing), and
/// takes a line per piece.
#[test]
fn a_long_message_breaks_at_words() {
    let mut hud = message_hud();
    say(
        &mut hud,
        "Rebels captured the Collection Plant (Lower level)",
        [1, 86, 213],
    );
    let pending: Vec<&str> = lines(&hud)
        .pending
        .iter()
        .map(|(t, _)| t.as_str())
        .collect();
    assert_eq!(
        pending,
        ["Rebels captured the", "Collection Plant (Lower", "level)"]
    );
    // One line free of four: the second message still comes whole, its last
    // lines waiting for room (`MultiLine::add`).
    say(&mut hud, &["word"; 15].join(" "), [1, 86, 213]);
    let ml = lines(&hud);
    assert_eq!(ml.pending.len(), 6);
    assert_eq!(ml.pending[3].0, "word word word word word");
}

#[test]
fn number_colour_interpolates_between_keys() {
    let keys = [
        (0.0, [200.0, 0.0, 0.0]),
        (0.29, [200.0, 0.0, 0.0]),
        (0.3, [255.0, 200.0, 50.0]),
        (0.6, [0.0, 150.0, 213.0]),
        (1.0, [0.0, 150.0, 213.0]),
    ];
    assert_eq!(number_color(&keys, 0.1), Some([200, 0, 0]));
    assert_eq!(number_color(&keys, 1.0), Some([0, 150, 213]));
    assert_eq!(number_color(&keys, 0.73), Some([0, 150, 213]));
    let ramp = [(0.0, [0.0; 3]), (1.0, [200.0, 100.0, 50.0])];
    assert_eq!(number_color(&ramp, 0.5), Some([100, 50, 25]));
}

/// The BarBitmap flash (`re-target-hud.md` §5): a fall from 1.0 to 0.6
/// of a bar that was up leaves a copy over 0.6–1.0 for the fall time;
/// the first value of a bar that was hidden flashes nothing; a rise
/// flashes over its own span for the rise time.
#[test]
fn a_bar_flashes_the_span_it_lost_or_gained() {
    let mut hud = fixture::target_hud();
    let fill = hud.find("player1targethealth_fill").unwrap();
    assert_eq!(hud.elems[fill].flashy_scale, 1.0);
    assert_eq!(
        (hud.elems[fill].flash_rise, hud.elems[fill].flash_fall),
        (1.0, 1.0)
    );
    let ev = "player1.weapon1.target.healthFraction";
    // The first show: the value lands while the bar is still hidden.
    hud.send(ev, EventValue::Number(1.0), &|_| false);
    hud.advance(1.0 / 30.0);
    assert!(
        hud.elems[fill].flash.is_none(),
        "no flash on the first show"
    );
    hud.send(ev, EventValue::Number(0.4), &|_| false);
    // Hidden before: a fresh bar from 0.6 would not flash; this one was up.
    let f = hud.elems[fill].flash.expect("a fall flashes");
    assert_eq!((f.from, f.to, f.time), (1.0, 0.4, 1.0));
    assert!(f.fall());
    hud.advance(0.5);
    assert!((hud.elems[fill].flash.unwrap().progress() - 0.5).abs() < 1e-6);
    hud.advance(0.51);
    assert!(hud.elems[fill].flash.is_none(), "gone after the fall time");
    hud.send(ev, EventValue::Number(0.8), &|_| false);
    let f = hud.elems[fill].flash.expect("a rise flashes");
    assert!(!f.fall() && f.from == 0.4 && f.to == 0.8);
    // A bar with no flash times (the backing, defaults 0) never flashes.
    let back = hud.find("player1targethealth_blankfill").unwrap();
    assert!(hud.elems[back].flash.is_none());
    assert_eq!(hud.elems[back].flashy_scale, 1.0);
    // A bar that is hidden again flashes nothing.
    hud.send("player1.weapon1.target.disable", EventValue::None, &|_| {
        false
    });
    hud.advance(2.0);
    hud.send(ev, EventValue::Number(0.2), &|_| false);
    assert!(hud.elems[fill].flash.is_none());
}

/// A group's Color multiplies its children (`re-target-hud.md` §6): the
/// fill at teamColor × (192, 224, 224) / 255; the name group has no
/// colour and leaves its text alone.
#[test]
fn a_group_colour_tints_its_children() {
    let mut hud = fixture::target_hud();
    hud.send(
        "player1.weapon1.target.healthFraction",
        EventValue::Number(1.0),
        &|_| false,
    );
    hud.send(
        "player1.weapon1.target.teamColor",
        EventValue::Color([200, 100, 100]),
        &|_| false,
    );
    hud.advance(1.0);
    let fill = hud.find("player1targethealth_fill").unwrap();
    let d = hud.drawn(fill);
    let want = [
        200.0 * 192.0 / 255.0,
        100.0 * 224.0 / 255.0,
        100.0 * 224.0 / 255.0,
    ];
    for c in 0..3 {
        assert!((d[c] - want[c]).abs() < 1e-3, "{d:?} vs {want:?}");
    }
    let name = hud.find("player1targetinfo_classNameNEW").unwrap();
    assert_eq!(hud.drawn(name), [255.0; 3]);
    assert_eq!(hud.tint(hud.elems[fill].parent.unwrap()), [1.0; 3]);
}

/// The domination countdown's label (`1playerhud` VictoryTimerGroup): a Text
/// whose `Text("common.victoryin")` is localised, in the default font when
/// the layout names none; a key the table lacks prints raw.
#[test]
fn a_text_key_is_localised_in_the_default_font() {
    let s = |v: &str| HudArg::Str(v.into());
    let p = |name: &str, args: &[HudArg]| node(name, args, vec![]);
    let layout = HudLayout {
        name: "t".into(),
        nodes: vec![node(
            "group",
            &[s("VictoryTimerGroup")],
            vec![
                node(
                    "text",
                    &[s("victorytext")],
                    vec![
                        p("text", &[s("common.victoryin")]),
                        p("textalignment", &[s("Center"), s("Top")]),
                    ],
                ),
                node(
                    "text",
                    &[s("other")],
                    vec![
                        p("text", &[s("game.nokey")]),
                        p("textfont", &[s("gamefont_medium")]),
                    ],
                ),
            ],
        )],
    };
    let none = HudLayout {
        name: "none".into(),
        nodes: vec![],
    };
    let mut hud = Hud::new(&layout, &none);
    hud.set_texts(&|key| (key == "common.victoryin").then(|| "Victory In".to_owned()));
    let label = &hud.elems[hud.find("victorytext").unwrap()];
    assert_eq!(
        (label.text.as_str(), label.font.as_str()),
        ("Victory In", DEFAULT_FONT)
    );
    let other = &hud.elems[hud.find("other").unwrap()];
    assert_eq!(
        (other.text.as_str(), other.font.as_str()),
        ("game.nokey", "gamefont_medium")
    );
}
