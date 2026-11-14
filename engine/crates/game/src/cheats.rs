//! The session's cheats (card CHEAT2, `analysis/re-cheat-codes.md`): the
//! flags, the pause menu's Morse recogniser and the PS2's display cheats.
//!
//! `[ours]` They last for the session: kept across missions, playlist maps
//! and restarts, all off on return to the main menu or on exit, never saved
//! to the profile. The PS2 keeps them until power-off.

use core::sim::{Cheats, Sim};
use ui::menu::{MenuInput, MorseCheats};

/// `[ours]` The original's display and sound cheat words (re-cheat-codes.md):
/// `hud` hides the HUD, `slow` plays positional sound effects at 0.3x pitch,
/// and `80s`, `at` and `alias` have no effect in openbf yet. They are
/// soft-locked (the maintainer, November 13, 2026): the code stays, but
/// while [`WORDS_LOCKED`] is set no word toggles anything, in single player
/// or multiplayer, until the maintainer decides what to do with them.
pub const LOCKED_WORDS: [&str; 5] = ["hud", "slow", "80s", "at", "alias"];

/// The soft lock on [`LOCKED_WORDS`]; clear it to give the words back.
pub const WORDS_LOCKED: bool = true;

#[derive(Debug, Clone, Default)]
pub struct Session {
    /// The flags each battle's `Sim` gets ([`Self::install`]).
    pub flags: Cheats,
    /// `hud`: the HUD hidden.
    pub hud_hidden: bool,
    /// `slow`: positional SFX at 0.3x pitch.
    pub sfx_slow: bool,
    /// The Morse buffers: never reset, as on the PS2.
    morse: MorseCheats,
}

impl Session {
    pub fn new(flags: Cheats) -> Self {
        Self {
            flags,
            ..Default::default()
        }
    }

    /// One paused frame of the pausing player's D-pad rising edges. A
    /// recognised word toggles its cheat (in `sim` too) and gives the HUD
    /// line; the caller plays `shell_menu_accept`.
    pub fn feed(&mut self, input: MenuInput, sim: &mut Sim) -> Option<String> {
        let word = self.morse.feed(input)?;
        Some(self.apply_word(word, sim))
    }

    /// One paused frame's D-pad edges into the Morse buffers: the word
    /// recognised, if any, without applying it (a LAN round sends it to the
    /// host instead, issue #168).
    pub fn word(&mut self, input: MenuInput) -> Option<&'static str> {
        self.morse.feed(input)
    }

    /// Toggle the cheat a recognised word names; the confirmation line.
    pub fn apply_word(&mut self, word: &str, sim: &mut Sim) -> String {
        let on = |b: bool| if b { "on" } else { "off" };
        if let Some(id) = Cheats::id_for_word(word) {
            self.flags.toggle(id);
            self.install(sim);
            let label = self
                .flags
                .entries()
                .iter()
                .find(|e| e.0 == id)
                .map_or(id, |e| e.1);
            return format!("cheat {word}: {label} {}", on(self.flags.is_on(id)));
        }
        if WORDS_LOCKED && LOCKED_WORDS.contains(&word) {
            return format!("cheat {word}: not available yet");
        }
        match word {
            "cheat" => {
                let now = self.flags.toggle_five();
                self.install(sim);
                format!("cheat cheat: all five cheats {}", on(now))
            }
            "hud" => {
                self.hud_hidden = !self.hud_hidden;
                format!(
                    "cheat hud: HUD {}",
                    if self.hud_hidden { "hidden" } else { "shown" }
                )
            }
            "slow" => {
                self.sfx_slow = !self.sfx_slow;
                format!("cheat slow: positional SFX pitch x{}", self.sfx_rate())
            }
            "80s" => "cheat 80s: low-res soldiers (no soldier LOD models yet: no-op)".into(),
            "at" => "cheat at: comic hit effects (no melee AddHitEffect yet: no-op)".into(),
            "alias" => "cheat alias: PS2 GS post-filter toggle (no-op on PC)".into(),
            other => format!("cheat {other}: unknown"),
        }
    }

    /// The pause menu's rows changed `sim`'s flags: keep them.
    pub fn follow(&mut self, sim: &Sim) {
        self.flags = sim.cheats;
    }

    /// A new battle's `Sim` takes the session's flags.
    pub fn install(&self, sim: &mut Sim) {
        sim.cheats = self.flags;
    }

    /// The sound library's pitch factor (`SoundLib::rate_mul`).
    pub fn sfx_rate(&self) -> f32 {
        if self.sfx_slow {
            0.3
        } else {
            1.0
        }
    }

    /// Back at the main menu: every cheat off. The Morse buffers stay.
    pub fn end(&mut self) {
        self.flags = Cheats::default();
        self.hud_hidden = false;
        self.sfx_slow = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui::menu::MORSE;

    /// One D-pad edge: 'U', 'D', 'L' or 'R'.
    fn pad(k: u8) -> MenuInput {
        MenuInput {
            up: k == b'U',
            down: k == b'D',
            left: k == b'L',
            right: k == b'R',
            ..Default::default()
        }
    }

    /// A word's D-pad sequence: dots Up, dashes Down, Left after each
    /// letter, Right at the end.
    fn dpad(word: &str) -> String {
        let mut s = String::new();
        for ch in word.bytes() {
            let code = MORSE.iter().find(|(c, _)| *c == ch).unwrap().1;
            s.extend(code.chars().map(|c| if c == '.' { 'U' } else { 'D' }));
            s.push('L');
        }
        s.push('R');
        s
    }

    /// Feed a D-pad sequence; the lines of every recognised word.
    fn press(s: &mut Session, sim: &mut Sim, keys: &str) -> Vec<String> {
        keys.bytes().filter_map(|k| s.feed(pad(k), sim)).collect()
    }

    /// Each word's exact sequence (as the RE note's Port section lists
    /// them) toggles its cheat in the session and the battle; the second
    /// entry toggles it back.
    #[test]
    fn each_word_toggles_its_cheat_and_back() {
        let words = [
            ("sos", "UUULDDDLUUULR", "invincible"),
            ("ammo", "UDLDDLDDLDDDLR", "ammo"),
            ("clip", "DUDULUDUULUULUDDULR", "clip"),
            ("run", "UDULUUDLDULR", "stamina"),
            ("all", "UDLUDUULUDUULR", "classes"),
        ];
        let mut s = Session::default();
        let mut sim = Sim::empty(1);
        for (word, keys, id) in words {
            assert_eq!(dpad(word), keys, "{word}");
            let lines = press(&mut s, &mut sim, keys);
            assert_eq!(lines.len(), 1, "{word}");
            assert!(lines[0].ends_with(" on"), "{}", lines[0]);
            assert!(s.flags.is_on(id) && sim.cheats.is_on(id), "{word}");
            press(&mut s, &mut sim, keys);
            assert!(!s.flags.is_on(id) && !sim.cheats.is_on(id), "{word} off");
        }
        assert_eq!(s.flags, Cheats::default());
    }

    /// `cheat` turns all five on when any is off, and all five off when all
    /// are on; the other flags stay as they are.
    #[test]
    fn the_cheat_word_toggles_all_five() {
        let keys = "DUDULUUUULULUDLDLR";
        assert_eq!(dpad("cheat"), keys);
        let mut s = Session::default();
        let mut sim = Sim::empty(1);
        s.flags.all_awards = true;
        let all_on = |c: &Cheats| Cheats::FIVE.iter().all(|id| c.is_on(id));
        let none_on = |c: &Cheats| Cheats::FIVE.iter().all(|id| !c.is_on(id));
        assert_eq!(press(&mut s, &mut sim, keys).len(), 1);
        assert!(all_on(&s.flags) && all_on(&sim.cheats));
        // One off: the word turns it back on, not the rest off.
        press(&mut s, &mut sim, &dpad("ammo"));
        assert!(!s.flags.infinite_ammo);
        let lines = press(&mut s, &mut sim, keys);
        assert!(lines[0].ends_with(" on"), "{}", lines[0]);
        assert!(all_on(&s.flags) && all_on(&sim.cheats));
        // All on: all off.
        press(&mut s, &mut sim, keys);
        assert!(none_on(&s.flags) && none_on(&sim.cheats));
        assert!(s.flags.all_awards);
    }

    /// A wrong letter is dropped (the word goes on); a wrong word resets.
    #[test]
    fn a_wrong_letter_is_dropped_and_a_wrong_word_resets() {
        let mut s = Session::default();
        let mut sim = Sim::empty(1);
        // "..-.-" is no letter: dropped, so this still spells "run".
        let lines = press(&mut s, &mut sim, "UUDUDLUDULUUDLDULR");
        assert_eq!(lines.len(), 1);
        assert!(s.flags.infinite_stamina);
        // "rum" is no word: nothing toggles, then "run" works at once.
        assert!(press(&mut s, &mut sim, "UDULUUDLDDLR").is_empty());
        assert!(s.flags.infinite_stamina);
        press(&mut s, &mut sim, &dpad("run"));
        assert!(!s.flags.infinite_stamina);
    }

    /// The flags outlive a battle: a new mission's `Sim` gets them, the
    /// menu rows' toggles are kept, and the main menu turns all off.
    #[test]
    fn flags_survive_a_mission_change_and_clear_at_the_main_menu() {
        let mut s = Session::default();
        let mut first = Sim::empty(1);
        press(&mut s, &mut first, &dpad("sos"));
        // `hud` is soft-locked (`WORDS_LOCKED`): it toggles nothing.
        press(&mut s, &mut first, &dpad("hud"));
        // A pause-menu row in the first battle.
        first.cheats.toggle("clip");
        s.follow(&first);
        let mut second = Sim::empty(2);
        s.install(&mut second);
        assert!(second.cheats.invincible && second.cheats.no_reload);
        assert!(!s.hud_hidden, "a locked word changes nothing");
        s.end();
        let mut third = Sim::empty(3);
        third.cheats.all_classes = true;
        s.install(&mut third);
        assert_eq!(third.cheats, Cheats::default());
        assert!(!s.hud_hidden && !s.sfx_slow);
    }

    /// Every soft-locked word answers "not available yet" and leaves the
    /// session and the sim alone.
    #[test]
    fn locked_words_toggle_nothing() {
        let mut s = Session::default();
        let mut sim = Sim::empty(1);
        for w in LOCKED_WORDS {
            let line = s.apply_word(w, &mut sim);
            assert_eq!(line, format!("cheat {w}: not available yet"));
        }
        assert!(!s.hud_hidden && !s.sfx_slow);
        assert_eq!(s.sfx_rate(), 1.0);
        assert_eq!(sim.cheats, Cheats::default());
    }
}
