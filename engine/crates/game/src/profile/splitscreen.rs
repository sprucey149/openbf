//! The Splitscreen options saved on the host (P1) profile (issue #194,
//! `analysis/plan/SPLITSCREEN.md` 18.1 item 4), and which settings belong to
//! each player and which to the host (18.1 item 6).
//!
//! Every profile shares one `profiles.ron`, and the catalog refuses the whole
//! file on any parse error (`Store::access`), so this entry tolerates what an
//! older, newer or hand-edited file is likely to hold: every field has a
//! default and the struct has `#[serde(default)]`, so missing fields load;
//! `cells` and `name_tags` take any well-formed RON value and fall back to
//! the default on the wrong shape, length, range or type; the enums take an
//! unknown unit variant as `Unknown`, which
//! [`SplitscreenSettings::normalize`] turns back into the default. Not
//! covered: an enum value that carries data (`two_player: Custom(2)`) or text
//! that is not RON at all, which fail like any other strict `Settings` field.

use serde::{Deserialize, Serialize};

/// How two players share the window. The profile's own enum, so the layout's
/// [`crate::split_layout::TwoPlayer`] needs no serde.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TwoPlayerSplit {
    /// One left, one right (today's default).
    #[default]
    SideBySide,
    /// One over the other (the original's layout).
    TopAndBottom,
    /// A value from another build; reads as the default.
    #[serde(other)]
    Unknown,
}

impl TwoPlayerSplit {
    pub fn layout(self) -> crate::split_layout::TwoPlayer {
        match self {
            Self::TopAndBottom => crate::split_layout::TwoPlayer::TopBottom,
            Self::SideBySide | Self::Unknown => crate::split_layout::TwoPlayer::SideBySide,
        }
    }
}

/// The split HUD's size in each cell (18.1 item 5's `hud_k`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HudSize {
    #[default]
    Full,
    Smaller,
    Smallest,
    /// A value from another build; reads as the default.
    #[serde(other)]
    Unknown,
}

impl HudSize {
    /// The factor on the HUD box: 1.0, 0.85, 0.7.
    pub fn k(self) -> f32 {
        match self {
            Self::Full | Self::Unknown => 1.0,
            Self::Smaller => 0.85,
            Self::Smallest => 0.7,
        }
    }
}

/// The identity order: P1 top-left through P4 bottom-right.
pub const IDENTITY_CELLS: [u8; 4] = [0, 1, 2, 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SplitscreenSettings {
    pub two_player: TwoPlayerSplit,
    /// `cells[seat]` is the 2x2 grid position of seat P1..P4 (0 top-left to 3
    /// bottom-right). Always a permutation of 0..4 after
    /// [`normalize`](Self::normalize).
    #[serde(deserialize_with = "lenient_cells")]
    pub cells: [u8; 4],
    /// Name tags over other players' heads in split cells.
    #[serde(deserialize_with = "lenient_name_tags")]
    pub name_tags: bool,
    pub hud_size: HudSize,
}

/// Any RON value; four integers in 0..=255 load as they are (`normalize`
/// then checks the permutation), anything else as [`IDENTITY_CELLS`].
fn lenient_cells<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[u8; 4], D::Error> {
    let value = ron::Value::deserialize(d)?;
    Ok(<[u8; 4]>::deserialize(value).unwrap_or(IDENTITY_CELLS))
}

/// Any RON value; a bool loads as it is, anything else as `true`.
fn lenient_name_tags<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(match ron::Value::deserialize(d)? {
        ron::Value::Bool(b) => b,
        _ => true,
    })
}

/// Written by hand: a derived one would give `name_tags: false` and
/// `cells: [0; 4]`.
impl Default for SplitscreenSettings {
    fn default() -> Self {
        Self {
            two_player: TwoPlayerSplit::SideBySide,
            cells: IDENTITY_CELLS,
            name_tags: true,
            hud_size: HudSize::Full,
        }
    }
}

impl SplitscreenSettings {
    /// Unknown enum values become the defaults; a `cells` that is not a
    /// permutation of 0..4 becomes the identity.
    pub fn normalize(&mut self) {
        if self.two_player == TwoPlayerSplit::Unknown {
            self.two_player = TwoPlayerSplit::default();
        }
        if self.hud_size == HudSize::Unknown {
            self.hud_size = HudSize::default();
        }
        let mut seen = [false; 4];
        let permutation = self.cells.iter().all(|&c| {
            let fresh = (c as usize) < 4 && !seen[c as usize];
            if fresh {
                seen[c as usize] = true;
            }
            fresh
        });
        if !permutation {
            self.cells = IDENTITY_CELLS;
        }
    }
}

/// Whose profile a Game Menu settings row edits (18.1 item 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingScope {
    /// The player who opened the menu: their own profile.
    Player,
    /// Shared by the match: always the host's (P1's) profile.
    Host,
}

/// Every [`super::Settings`] field and whose it is in a split match. The
/// Game Menu rows follow it (`row_scope`, B10): a row edits the opener's
/// profile when its field is `Player`, P1's when `Host`. Per-seat input in
/// battle (bindings, layouts, flight, pad look, viewpoint, icons, rumble)
/// reads the `Player` fields from each seat's own profile (B3).
/// `settings_version` is each profile's own bookkeeping: every profile
/// migrates itself on load, so it is listed as `Player`. A test checks that
/// the list names each field once. Rows not backed by a `Settings` field get
/// their scope in `row_scope` itself: cheats are `Host`; the controls editor
/// is `Player` (it edits the binding and layout fields below).
pub const SETTING_SCOPES: &[(&str, SettingScope)] = {
    use SettingScope::{Host, Player};
    &[
        ("settings_version", Player),
        // Assist, pad look, mouse, invert, FOV, viewpoint, career awards.
        ("assist_intensity", Player),
        ("aim_assist", Player),
        ("career_awards", Player),
        ("fov_scale", Player),
        ("mouse_sensitivity", Player),
        ("pad_look_sensitivity", Player),
        ("invert_y", Player),
        ("viewpoint", Player),
        // Rumble and prompts.
        ("rumble_strength", Player),
        ("rumble_on_fire", Player),
        ("input_icons", Player),
        // The controls editor and flight controls.
        ("keyboard_bindings", Player),
        ("controller_bindings", Player),
        ("keyboard_layouts", Player),
        ("gamepad_layouts", Player),
        ("flight_controls", Player),
        // Match rules.
        ("difficulty", Host),
        ("team_handicap_on", Host),
        ("team_size_pct", Host),
        ("reinforcement_pct", Host),
        // Volumes.
        ("music_volume", Host),
        ("effects_volume", Host),
        ("dialogue_volume", Host),
        // Counters.
        ("fps_counter", Host),
        ("network_meter", Host),
        // Graphics and the window.
        ("low_power_resolution", Host),
        ("auto_quality", Host),
        ("terrain_lod", Host),
        ("hard_shadows", Host),
        ("scene_softness", Host),
        ("window_mode", Host),
        ("last_fullscreen", Host),
        // The LAN join list (dev-phase, read once).
        ("recent_hosts", Host),
        // The Splitscreen page.
        ("splitscreen", Host),
    ]
};

/// The scope of a [`super::Settings`] field by name; `None` for no such
/// field.
pub fn setting_scope(field: &str) -> Option<SettingScope> {
    SETTING_SCOPES
        .iter()
        .find(|(name, _)| *name == field)
        .map(|&(_, scope)| scope)
}
