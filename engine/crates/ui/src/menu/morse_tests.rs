use super::*;

fn edge(up: bool, down: bool, left: bool, right: bool) -> MenuInput {
    MenuInput {
        up,
        down,
        left,
        right,
        confirm: false,
        back: false,
    }
}

/// Type a word the PS2 way: symbols, Left after each letter, Right at the end.
fn type_word(m: &mut MorseCheats, word: &str) -> Option<&'static str> {
    let mut hit = None;
    for ch in word.bytes() {
        let code = MORSE.iter().find(|(c, _)| *c == ch).unwrap().1;
        for sym in code.bytes() {
            assert!(m
                .feed(edge(sym == b'.', sym == b'-', false, false))
                .is_none());
        }
        assert!(m.feed(edge(false, false, true, false)).is_none());
    }
    hit = hit.or(m.feed(edge(false, false, false, true)));
    hit
}

#[test]
fn all_seven_words_recognised() {
    let mut m = MorseCheats::new();
    for (w, h) in CHEAT_WORDS {
        assert_eq!(fnv1a_lower(w.as_bytes()), h, "{w}");
        assert_eq!(type_word(&mut m, w), Some(w));
        assert_eq!(m.pending(), (String::new(), String::new()));
    }
}

/// `[ours]` clip, run, all: recognised, and no PS2 hash collides.
#[test]
fn our_words_recognised_without_collisions() {
    let mut m = MorseCheats::new();
    for w in OUR_WORDS {
        let h = fnv1a_lower(w.as_bytes());
        assert!(CHEAT_WORDS.iter().all(|(_, ps2)| *ps2 != h), "{w}");
        assert_eq!(type_word(&mut m, w), Some(w));
    }
}

#[test]
fn unknown_word_resets_silently_then_next_word_works() {
    let mut m = MorseCheats::new();
    assert_eq!(type_word(&mut m, "sso"), None);
    assert_eq!(type_word(&mut m, "sos"), Some("sos"));
}

#[test]
fn unrecognised_letter_is_dropped_but_word_continues() {
    let mut m = MorseCheats::new();
    // "..-.-" is no Morse letter: dropped, the word buffer is untouched.
    for sym in "..-.-".bytes() {
        m.feed(edge(sym == b'.', sym == b'-', false, false));
    }
    m.feed(edge(false, false, true, false));
    assert_eq!(m.pending().1, "");
    assert_eq!(type_word(&mut m, "at"), Some("at"));
}

#[test]
fn letter_buffer_wraps_at_six_symbols() {
    // Six dots wrap mlen to 0; a seventh symbol overwrites slot 0 and the
    // commit sees a one-symbol string: '-' -> 't'.
    let mut m = MorseCheats::new();
    for _ in 0..6 {
        m.feed(edge(true, false, false, false));
    }
    assert_eq!(m.pending().0, "");
    m.feed(edge(false, true, false, false));
    m.feed(edge(false, false, true, false));
    assert_eq!(m.pending().1, "t");
}

#[test]
fn priority_up_over_down_and_state_persists_across_idle_frames() {
    let mut m = MorseCheats::new();
    m.feed(edge(true, true, false, false));
    assert_eq!(m.pending().0, ".");
    for _ in 0..1000 {
        m.feed(edge(false, false, false, false));
    }
    assert_eq!(m.pending().0, ".");
    m.feed(edge(true, false, false, false));
    m.feed(edge(true, false, false, false));
    m.feed(edge(false, false, true, false));
    assert_eq!(m.pending(), (String::new(), "s".into()));
}
