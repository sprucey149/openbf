pub const FRAME_W: f32 = 512.0;
pub const FRAME_H: f32 = 448.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HintStyle {
    pub elem: usize,
    pub friendly: Option<[f32; 3]>,
    pub min_scale: f32,
    pub max_scale: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventValue {
    None,
    Number(f32),
    Text(String),
    Color([u8; 3]),
    Position([f32; 2]),
}

pub const LOCK_STYLE_NAMES: [&str; 4] = [
    "lockedonscreen",
    "lockedonscreenbehind",
    "lockedoffscreen",
    "lockedoffscreenbehind",
];

pub const HINT_STYLE_NAMES: [&str; 4] = [
    "hintonscreen",
    "hintonscreenbehind",
    "hintoffscreen",
    "hintoffscreenbehind",
];

pub const FLAG_STYLE_NAMES: [&str; 4] = [
    "hintflagonscreen",
    "hintflagonscreenbehind",
    "hintflagoffscreen",
    "hintflagoffscreenbehind",
];

pub const CARRIER_STYLE_NAMES: [&str; 4] = [
    "flagonscreen",
    "flagonscreenbehind",
    "flagoffscreen",
    "flagoffscreenbehind",
];

pub const OBJECTIVE_STYLE_NAMES: [&str; 4] = [
    "objectiveonscreen",
    "objectiveonscreenbehind",
    "objectiveoffscreen",
    "objectiveoffscreenbehind",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HintMarker {
    pub ndc: [f32; 2],
    pub distance: f32,
    pub friendly: bool,
    pub visible: bool,
    pub on_screen: bool,
    pub flag: bool,
    pub carrier: bool,
    pub flyer_view: bool,
    pub own_carrier: bool,
    pub objective: bool,
    pub pulse: Option<f32>,
    pub tint: Option<[f32; 3]>,
}
