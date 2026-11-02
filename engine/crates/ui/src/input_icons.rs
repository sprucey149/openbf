//! The one lookup every drawn control prompt goes through: which sheet shows
//! a [`Prompt`] for the device family in use, and what the egui menus call it.
//!
//! The HUD pass reads [`Icons`] from [`crate::hud_draw::HudArt`], so the shell,
//! the spawn screens, the stats pages and the HUD tree's D-pad all follow one
//! setting. A lookup is a `match` that answers a `&'static str`.
//!
//! **The art.** [`IconSet::Ds2`] keeps the PS2 disc's own Cross, Triangle and
//! D-pad (`btna`, `btnb`, `btn_directional_pad`), so a PS2 player sees the
//! screens as before; every other sheet named `icon_*` comes from
//! `game::input_icons`. `icon_duke_*` is the original Xbox pad: the bake writes
//! those from the player's Xbox disc when one was supplied, and the game draws
//! its own otherwise. Nothing from a disc is in the repo.
//!
//! **Which button.** Menus read fixed pad buttons, so [`Prompt::Accept`],
//! [`Prompt::Back`] and [`Prompt::Done`] are fixed too. The spawn screens read
//! the player's bindings, so [`Icons::spawn_accept`], [`Icons::spawn_back`] and
//! [`Icons::class_keys`] follow a remap. The D-pad prompts draw the whole pad
//! and do not.

/// The icons drawn, one per device family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconSet {
    KeyboardMouse,
    /// The PS2 pad; the default, so the port draws what the PS2 draws until
    /// the player's setting says otherwise.
    #[default]
    Ds2,
    Ds4,
    Ds5,
    XboxOriginal,
    XboxOne,
}

/// A physical pad control, by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadButton {
    South,
    East,
    West,
    North,
    L1,
    R1,
    L2,
    R2,
    L3,
    R3,
    Select,
    Start,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

impl PadButton {
    pub const ALL: [PadButton; 16] = [
        PadButton::South,
        PadButton::East,
        PadButton::West,
        PadButton::North,
        PadButton::L1,
        PadButton::R1,
        PadButton::L2,
        PadButton::R2,
        PadButton::L3,
        PadButton::R3,
        PadButton::Select,
        PadButton::Start,
        PadButton::DPadUp,
        PadButton::DPadDown,
        PadButton::DPadLeft,
        PadButton::DPadRight,
    ];
}

/// What a help line or legend asks the player to press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    /// The shell's and the stats pages' accept.
    Accept,
    /// The shell's and the stats pages' back.
    Back,
    /// The stats pages' Done (the PS2's Square).
    Done,
    /// The spawn screens' accept, as bound.
    SpawnAccept,
    /// The spawn screens' back, as bound.
    SpawnBack,
    /// The whole D-pad.
    DPad,
    /// The D-pad's left and right arms.
    DPadLr,
}

impl Prompt {
    pub const ALL: [Prompt; 7] = [
        Prompt::Accept,
        Prompt::Back,
        Prompt::Done,
        Prompt::SpawnAccept,
        Prompt::SpawnBack,
        Prompt::DPad,
        Prompt::DPadLr,
    ];
}

/// The set in use and the spawn screens' buttons, as the player bound them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icons {
    pub set: IconSet,
    pub spawn_accept: PadButton,
    pub spawn_back: PadButton,
    pub class_next: PadButton,
    pub class_prev: PadButton,
}

impl Default for Icons {
    fn default() -> Self {
        Icons::of(IconSet::default())
    }
}

impl Icons {
    /// `set` with the default bindings.
    pub const fn of(set: IconSet) -> Icons {
        Icons {
            set,
            spawn_accept: PadButton::South,
            spawn_back: PadButton::North,
            class_next: PadButton::R1,
            class_prev: PadButton::Select,
        }
    }

    /// The pad button `prompt` means; `None` for the whole-D-pad prompts.
    fn button(&self, prompt: Prompt) -> Option<PadButton> {
        match prompt {
            Prompt::Accept => Some(PadButton::South),
            Prompt::Back => Some(PadButton::North),
            Prompt::Done => Some(PadButton::West),
            Prompt::SpawnAccept => Some(self.spawn_accept),
            Prompt::SpawnBack => Some(self.spawn_back),
            Prompt::DPad | Prompt::DPadLr => None,
        }
    }

    /// The sheet that draws `prompt`.
    pub fn sheet(&self, prompt: Prompt) -> &'static str {
        if self.set == IconSet::KeyboardMouse {
            return match prompt {
                Prompt::Accept | Prompt::SpawnAccept => "icon_kb_enter",
                Prompt::Back | Prompt::SpawnBack => "icon_kb_backspace",
                Prompt::Done => "icon_kb_space",
                Prompt::DPad => "icon_kb_arrows",
                Prompt::DPadLr => "icon_kb_arrows_lr",
            };
        }
        match self.button(prompt) {
            Some(b) => pad_sheet(self.set, b),
            None => dpad_sheet(self.set, prompt == Prompt::DPadLr),
        }
    }

    /// The sheet a HUD layout's own `name` draws: its D-pad prompts follow
    /// the set, every other sheet is itself.
    pub fn layout_sheet<'a>(&self, name: &'a str) -> &'a str {
        if name.eq_ignore_ascii_case("btn_directional_pad") {
            self.sheet(Prompt::DPad)
        } else if name.eq_ignore_ascii_case("btn_directional_pad_lr") {
            self.sheet(Prompt::DPadLr)
        } else {
            name
        }
    }

    /// What the egui panels call `prompt`.
    pub fn text(&self, prompt: Prompt) -> &'static str {
        if self.set == IconSet::KeyboardMouse {
            return match prompt {
                Prompt::Accept | Prompt::SpawnAccept => "Enter",
                Prompt::Back | Prompt::SpawnBack => "Backspace",
                Prompt::Done => "Space",
                Prompt::DPad | Prompt::DPadLr => "Arrow keys",
            };
        }
        match self.button(prompt) {
            Some(b) => pad_name(self.set, b),
            None => "D-pad",
        }
    }

    /// [`Icons::text`] as the panels' prompt lines write it: PlayStation's Cross
    /// is its letter. The other symbols have no glyph in the panels' font.
    pub fn symbol(&self, prompt: Prompt) -> &'static str {
        let playstation = matches!(self.set, IconSet::Ds2 | IconSet::Ds4 | IconSet::Ds5);
        match self.button(prompt) {
            Some(PadButton::South) if playstation => "X",
            _ => self.text(prompt),
        }
    }

    /// The pause and settings menus' footer: how those menus are worked.
    /// In battle a paused D-pad and arrows' Left / Right type the cheat code, so
    /// a value steps by the left stick or A / D (`game::pause::rows_input`).
    pub fn menu_footer(&self, in_battle: bool) -> String {
        let adjust = match (self.set, in_battle) {
            (_, false) => "◀▶",
            (IconSet::KeyboardMouse, true) => "A / D",
            (_, true) => "Left stick / A / D",
        };
        match self.set {
            IconSet::KeyboardMouse => {
                format!("Arrows move · Enter select · {adjust} adjust · Esc resume")
            }
            set => format!(
                "D-pad / arrows move · {} / Enter select · {adjust} adjust · {} / {} / Esc resume",
                pad_name(set, PadButton::South),
                pad_name(set, PadButton::East),
                pad_name(set, PadButton::Start),
            ),
        }
    }

    /// The panels' name for the buttons that change class.
    pub fn class_keys(&self) -> String {
        if self.set == IconSet::KeyboardMouse {
            return "Q / Tab".to_owned();
        }
        format!(
            "{} / {}",
            pad_name(self.set, self.class_next),
            pad_name(self.set, self.class_prev)
        )
    }
}

/// The D-pad's sheet. Original Xbox art comes from the disc when baked.
fn dpad_sheet(set: IconSet, lr: bool) -> &'static str {
    match (set, lr) {
        (IconSet::Ds2, false) => "btn_directional_pad",
        (IconSet::Ds2, true) => "btn_directional_pad_lr",
        (IconSet::Ds4 | IconSet::Ds5, false) => "icon_ps_dpad",
        (IconSet::Ds4 | IconSet::Ds5, true) => "icon_ps_dpad_lr",
        (IconSet::XboxOriginal, false) => "icon_duke_dpad",
        (IconSet::XboxOriginal, true) => "icon_duke_dpad_lr",
        (_, false) => "icon_xbone_dpad",
        (_, true) => "icon_xbone_dpad_lr",
    }
}

/// The sheet of one pad control in `set`.
pub fn pad_sheet(set: IconSet, b: PadButton) -> &'static str {
    use PadButton as P;
    match set {
        IconSet::Ds2 => match b {
            P::South => "btna",
            P::North => "btnb",
            P::Select => "icon_ps2_select",
            P::Start => "icon_ps2_start",
            _ => playstation(b),
        },
        IconSet::Ds4 => match b {
            P::Select => "icon_ds4_share",
            P::Start => "icon_ds4_options",
            _ => playstation(b),
        },
        IconSet::Ds5 => match b {
            P::Select => "icon_ds5_create",
            P::Start => "icon_ds5_options",
            _ => playstation(b),
        },
        IconSet::XboxOriginal => match b {
            P::South => "icon_duke_a",
            P::East => "icon_duke_b",
            P::West => "icon_duke_x",
            P::North => "icon_duke_y",
            // The Duke's White and Black sit where the later pads' bumpers
            // map; its triggers are the analog L and R.
            P::L1 => "icon_duke_white",
            P::R1 => "icon_duke_black",
            P::L2 => "icon_duke_l",
            P::R2 => "icon_duke_r",
            P::L3 => "icon_duke_l3",
            P::R3 => "icon_duke_r3",
            P::Select => "icon_duke_back",
            P::Start => "icon_duke_start",
            P::DPadUp => "icon_duke_dpad_up",
            P::DPadDown => "icon_duke_dpad_down",
            P::DPadLeft => "icon_duke_dpad_left",
            P::DPadRight => "icon_duke_dpad_right",
        },
        // Reached only through a direct call: a keyboard has no pad buttons,
        // so its answer is the commonest layout.
        IconSet::XboxOne | IconSet::KeyboardMouse => xbox_one(b),
    }
}

fn playstation(b: PadButton) -> &'static str {
    use PadButton as P;
    match b {
        P::South => "icon_ps_cross",
        P::East => "icon_ps_circle",
        P::West => "icon_ps_square",
        P::North => "icon_ps_triangle",
        P::L1 => "icon_ps_l1",
        P::R1 => "icon_ps_r1",
        P::L2 => "icon_ps_l2",
        P::R2 => "icon_ps_r2",
        P::L3 => "icon_ps_l3",
        P::R3 => "icon_ps_r3",
        P::Select => "icon_ps2_select",
        P::Start => "icon_ps2_start",
        P::DPadUp => "icon_ps_dpad_up",
        P::DPadDown => "icon_ps_dpad_down",
        P::DPadLeft => "icon_ps_dpad_left",
        P::DPadRight => "icon_ps_dpad_right",
    }
}

fn xbox_one(b: PadButton) -> &'static str {
    use PadButton as P;
    match b {
        P::South => "icon_xbone_a",
        P::East => "icon_xbone_b",
        P::West => "icon_xbone_x",
        P::North => "icon_xbone_y",
        P::L1 => "icon_xbone_lb",
        P::R1 => "icon_xbone_rb",
        P::L2 => "icon_xbone_lt",
        P::R2 => "icon_xbone_rt",
        P::L3 => "icon_xbone_l3",
        P::R3 => "icon_xbone_r3",
        P::Select => "icon_xbone_view",
        P::Start => "icon_xbone_menu",
        P::DPadUp => "icon_xbone_dpad_up",
        P::DPadDown => "icon_xbone_dpad_down",
        P::DPadLeft => "icon_xbone_dpad_left",
        P::DPadRight => "icon_xbone_dpad_right",
    }
}

/// A pad control's name in `set`: the panels' prompts and the Controls page.
pub fn pad_name(set: IconSet, b: PadButton) -> &'static str {
    use PadButton as P;
    let playstation = matches!(
        set,
        IconSet::Ds2 | IconSet::Ds4 | IconSet::Ds5 | IconSet::KeyboardMouse
    );
    match b {
        P::DPadUp => return "D-pad up",
        P::DPadDown => return "D-pad down",
        P::DPadLeft => return "D-pad left",
        P::DPadRight => return "D-pad right",
        _ => {}
    }
    if playstation {
        return match b {
            P::South => "Cross",
            P::East => "Circle",
            P::West => "Square",
            P::North => "Triangle",
            P::L1 => "L1",
            P::R1 => "R1",
            P::L2 => "L2",
            P::R2 => "R2",
            P::L3 => "L3",
            P::R3 => "R3",
            P::Select => match set {
                IconSet::Ds4 => "Share",
                IconSet::Ds5 => "Create",
                _ => "Select",
            },
            _ => match set {
                IconSet::Ds4 | IconSet::Ds5 => "Options",
                _ => "Start",
            },
        };
    }
    let duke = set == IconSet::XboxOriginal;
    match b {
        P::South => "A",
        P::East => "B",
        P::West => "X",
        P::North => "Y",
        P::L1 if duke => "White",
        P::R1 if duke => "Black",
        P::L2 if duke => "L",
        P::R2 if duke => "R",
        P::L1 => "LB",
        P::R1 => "RB",
        P::L2 => "LT",
        P::R2 => "RT",
        P::L3 => "Left stick press",
        P::R3 => "Right stick press",
        P::Select if duke => "Back",
        P::Select => "View",
        _ if duke => "Start",
        _ => "Menu",
    }
}

/// Every set, for the tests that check each has art.
pub const ALL_SETS: [IconSet; 6] = [
    IconSet::KeyboardMouse,
    IconSet::Ds2,
    IconSet::Ds4,
    IconSet::Ds5,
    IconSet::XboxOriginal,
    IconSet::XboxOne,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The default is the PS2's own art: what the goldens and a PS2 player
    /// see does not move.
    #[test]
    fn the_default_is_the_discs_own_art() {
        let i = Icons::default();
        assert_eq!(i.sheet(Prompt::Accept), "btna");
        assert_eq!(i.sheet(Prompt::Back), "btnb");
        assert_eq!(i.sheet(Prompt::SpawnAccept), "btna");
        assert_eq!(i.sheet(Prompt::SpawnBack), "btnb");
        assert_eq!(i.sheet(Prompt::DPad), "btn_directional_pad");
        assert_eq!(i.sheet(Prompt::DPadLr), "btn_directional_pad_lr");
    }

    /// Each pad draws its own face buttons at the same positions.
    #[test]
    fn a_position_is_the_same_button_on_every_pad() {
        for (set, south, north) in [
            (IconSet::Ds4, "icon_ps_cross", "icon_ps_triangle"),
            (IconSet::Ds5, "icon_ps_cross", "icon_ps_triangle"),
            (IconSet::XboxOne, "icon_xbone_a", "icon_xbone_y"),
            (IconSet::XboxOriginal, "icon_duke_a", "icon_duke_y"),
        ] {
            let i = Icons::of(set);
            assert_eq!(i.sheet(Prompt::Accept), south, "{set:?}");
            assert_eq!(i.sheet(Prompt::Back), north, "{set:?}");
        }
    }

    /// DS4 and DS5 share the PlayStation symbols and differ where the pads do.
    #[test]
    fn ds4_and_ds5_differ_only_on_their_middle_buttons() {
        for b in PadButton::ALL {
            let differs = pad_sheet(IconSet::Ds4, b) != pad_sheet(IconSet::Ds5, b);
            assert_eq!(differs, matches!(b, PadButton::Select | PadButton::Start));
        }
    }

    /// The spawn screens follow a remap; the menus, which read fixed
    /// buttons, do not.
    #[test]
    fn only_the_spawn_prompts_follow_the_bindings() {
        let i = Icons {
            spawn_accept: PadButton::West,
            spawn_back: PadButton::East,
            ..Icons::of(IconSet::XboxOne)
        };
        assert_eq!(i.sheet(Prompt::SpawnAccept), "icon_xbone_x");
        assert_eq!(i.sheet(Prompt::SpawnBack), "icon_xbone_b");
        assert_eq!(i.sheet(Prompt::Accept), "icon_xbone_a");
        assert_eq!(i.sheet(Prompt::Back), "icon_xbone_y");
        assert_eq!(i.text(Prompt::SpawnAccept), "X");
    }

    /// The panels' class keys name the buttons bound, not the defaults.
    #[test]
    fn the_class_keys_follow_the_bindings() {
        let i = Icons {
            class_next: PadButton::L1,
            class_prev: PadButton::North,
            ..Icons::of(IconSet::XboxOne)
        };
        assert_eq!(i.class_keys(), "LB / Y");
        assert_eq!(
            Icons {
                set: IconSet::KeyboardMouse,
                ..i
            }
            .class_keys(),
            "Q / Tab"
        );
    }

    /// The HUD tree's D-pads follow the set, whatever case the layout uses;
    /// its other sheets stand.
    #[test]
    fn a_layouts_dpad_follows_the_set() {
        let i = Icons::of(IconSet::KeyboardMouse);
        assert_eq!(
            i.layout_sheet("btn_directional_pad_LR"),
            "icon_kb_arrows_lr"
        );
        assert_eq!(i.layout_sheet("btn_directional_pad"), "icon_kb_arrows");
        assert_eq!(i.layout_sheet("hud_flag_icon"), "hud_flag_icon");
        let ps2 = Icons::default();
        assert_eq!(
            ps2.layout_sheet("btn_directional_pad_LR"),
            "btn_directional_pad_lr"
        );
    }

    /// The panels name what the player holds.
    #[test]
    fn the_panels_name_the_device() {
        let name = |set| Icons::of(set).text(Prompt::Accept);
        assert_eq!(name(IconSet::Ds2), "Cross");
        assert_eq!(name(IconSet::XboxOne), "A");
        assert_eq!(name(IconSet::KeyboardMouse), "Enter");
        assert_eq!(Icons::of(IconSet::XboxOne).class_keys(), "RB / View");
        assert_eq!(
            Icons::of(IconSet::XboxOriginal).class_keys(),
            "Black / Back"
        );
        assert_eq!(Icons::of(IconSet::KeyboardMouse).class_keys(), "Q / Tab");
        assert_eq!(Icons::default().symbol(Prompt::SpawnAccept), "X");
        assert_eq!(Icons::default().symbol(Prompt::SpawnBack), "Triangle");
        assert_eq!(Icons::of(IconSet::XboxOne).symbol(Prompt::SpawnBack), "Y");
    }

    /// Everything the egui panels write has a glyph: a missing one draws as an
    /// empty box, which on a PlayStation footer read as Square.
    #[test]
    fn the_panels_text_has_glyphs() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            let font = egui::FontId::proportional(12.0);
            for set in ALL_SETS {
                let icons = Icons::of(set);
                let mut lines = vec![
                    icons.menu_footer(false),
                    icons.menu_footer(true),
                    icons.class_keys(),
                ];
                lines.extend(Prompt::ALL.map(|p| icons.symbol(p).to_owned()));
                for line in lines {
                    assert!(
                        ctx.fonts_mut(|f| f.has_glyphs(&font, &line)),
                        "{set:?}: {line}"
                    );
                }
            }
        });
    }

    /// The footer names each pad's own buttons.
    #[test]
    fn the_default_footer_stands() {
        assert_eq!(
            Icons::default().menu_footer(false),
            "D-pad / arrows move · Cross / Enter select · ◀▶ adjust · Circle / Start / Esc resume"
        );
        assert_eq!(
            Icons::default().menu_footer(true),
            "D-pad / arrows move · Cross / Enter select · Left stick / A / D adjust · Circle / Start / Esc resume"
        );
        assert!(Icons::of(IconSet::XboxOne)
            .menu_footer(false)
            .contains("A / Enter"));
        assert!(Icons::of(IconSet::KeyboardMouse)
            .menu_footer(false)
            .starts_with("Arrows"));
    }
}
