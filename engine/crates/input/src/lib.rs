//! Controller input: gilrs -> deterministic `core::sim::PadState`.
//!
//! The sim consumes quantized i16 axes + logical button bits only (replay
//! logs are bit-exact); this crate owns the physical mapping. Default layout
//! mirrors the PS2 originals per the locked design (PLAN §Platform & meta:
//! controller-only, PS2 default mapping, full remap later):
//!   left stick move / right stick aim, R2(RightTrigger2) fire,
//!   L2(LeftTrigger2) secondary, Cross(South) jump, Circle(East)
//!   roll/stance (contextual), L3(LeftThumb) sprint (one click, see
//!   [`SprintLatch`]), Square(West) reload, Triangle(North) action,
//!   R1(RightTrigger) switch weapon, L1(LeftTrigger) switch secondary,
//!   R3(RightThumb) zoom (the maintainer, 2026-10-04: the PS2's shoulder layout with
//!   fire on the triggers; a press steps the zoom of the weapon in hand, card
//!   ZOOM).
//!
//! The spawn screen's class select is PROVISIONAL (lead placeholder, no RE):
//! R1 (`RightTrigger`) next class and Select previous. The PS2's own
//! spawn-screen buttons are UNTRACED.
//!
//! The spawn screens' accept and back (card SSELA, `SPAWN_ACCEPT` /
//! `SPAWN_BACK`) are the PS2's X and Triangle (re-spawn-flow §1): Cross and
//! Triangle carry them beside JUMP and ACTION; the sim reads them only from a
//! slot that is not on the field, or whose live soldier opened the class
//! screen at a friendly post with ACTION (issue #28).
//!
//! The D-PAD is the squad commands' (Phase D, card SQACK) and, while the
//! player is dead, the spawn map's post push (card SPSEL, `SPAWN_POST_*`): the
//! sim reads the squad bits only from a live player and the post bits only
//! from a dead one. The pause menu still reads it UI-side; a paused player's
//! pad reaches the sim as idle.
//!
//! Pads (card PADM): every `Gilrs` comes from [`open_gilrs`], which adds the
//! project's own SDL mappings (`pads.txt`, pads the SDL_GameControllerDB
//! lacks — the maintainer's GameSir G7 Pro first) to gilrs's bundled DB, and
//! [`log_pads`] prints how each connected pad resolved and returns an
//! on-screen warning when the camera stick or FIRE maps to nothing.

use core::sim::{buttons, PadState};
use gilrs::Button;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Mapping {
    pub table: Vec<(Button, u32)>,
}

impl Mapping {
    /// Saved layouts may change buttons/order, not the supported action set.
    pub fn is_valid(&self) -> bool {
        let mut remaining = Self::ps2_default().table;
        if self.table.len() != remaining.len() {
            return false;
        }
        for (i, (button, bits)) in self.table.iter().enumerate() {
            if matches!(button, Button::Unknown | Button::Start)
                || self.table[..i].iter().any(|(b, _)| b == button)
            {
                return false;
            }
            let Some(at) = remaining.iter().position(|(_, action)| action == bits) else {
                return false;
            };
            remaining.swap_remove(at);
        }
        true
    }

    pub fn remap(&mut self, index: usize, button: Button) {
        if index >= self.table.len() || matches!(button, Button::Unknown | Button::Start) {
            return;
        }
        if let Some(other) = self.table.iter().position(|(b, _)| *b == button) {
            self.table[other].0 = self.table[index].0;
        }
        self.table[index].0 = button;
    }

    /// The default layout the module header describes.
    pub fn ps2_default() -> Mapping {
        Mapping {
            table: vec![
                (Button::RightTrigger2, buttons::FIRE),
                (Button::LeftTrigger2, buttons::SECONDARY_FIRE),
                // X / Triangle are also the spawn screens' accept / back
                // (card SSELA), read by the sim only off the field.
                (Button::South, buttons::JUMP | buttons::SPAWN_ACCEPT),
                (Button::East, buttons::ROLL_STANCE),
                (Button::LeftThumb, buttons::SPRINT),
                (Button::LeftTrigger, buttons::SWITCH_SECONDARY),
                (Button::West, buttons::RELOAD),
                (Button::North, buttons::ACTION | buttons::SPAWN_BACK),
                (Button::RightThumb, buttons::ZOOM),
                // The four squad orders, on the PS2's D-pad arms (card
                // SQACK, re-squad-orders.md section 1). The same arms move
                // the spawn map's post while dead (card SPSEL, as the PS2's
                // spawn map reads its D-pad): the sim reads squad bits only
                // alive and post bits only dead.
                (
                    Button::DPadUp,
                    buttons::SQUAD_MOVE_OUT | buttons::SPAWN_POST_UP,
                ),
                (
                    Button::DPadDown,
                    buttons::SQUAD_HOLD | buttons::SPAWN_POST_DOWN,
                ),
                (
                    Button::DPadLeft,
                    buttons::SQUAD_AT_EASE | buttons::SPAWN_POST_LEFT | buttons::HERO_ACCEPT,
                ),
                (
                    Button::DPadRight,
                    buttons::SQUAD_FOLLOW | buttons::SPAWN_POST_RIGHT | buttons::HERO_DECLINE,
                ),
                // The spawn-class bits are PROVISIONAL (lead placeholder, no
                // RE) and read by the sim only while the player is dead, so
                // R1 carries both.
                (
                    Button::RightTrigger,
                    buttons::SPAWN_CLASS_NEXT | buttons::SWITCH_WEAPON,
                ),
                (Button::Select, buttons::SPAWN_CLASS_PREV),
            ],
        }
    }
}

impl Default for Mapping {
    fn default() -> Self {
        Mapping::ps2_default()
    }
}

/// Per-axis deadzone applied before quantization. PROVISIONAL and ours: no
/// PS2 value has been traced (the DualShock 2 deadzone lives in the game's
/// pad code, untouched by the RE so far); 0.15 is a common modern-pad
/// default. Settled by an `analysis/` citation or the maintainer's feel check.
pub const DEADZONE: f32 = 0.15;

/// Left-stick travel that reads as a full push (card SPUP, ours, FEEL_NOTES):
/// the stick is scaled by 1 / this and clamped. The maintainer's pad peaks at 0.984
/// (32255 in his snapshots of 2026-10-04/05), which ran his soldier at 6.89
/// of 7.0 m/s and, up Kamino's 14.8 deg ramp, at 0.890 x MaxSpeed where the
/// PS2's full stick gives 0.904 - under the sprint's 0.9 floor. The look
/// stick keeps its own scale ([`LOOK_SCALE`]).
pub const STICK_FULL: f32 = 0.95;

/// An analog trigger (`RightTrigger2` / `LeftTrigger2` mapped from an axis)
/// counts as pressed at this travel. PROVISIONAL and ours (FEEL_NOTES): the
/// PS2 reads L2/R2 as digital buttons [H — the pad-read code is not RE'd],
/// so any threshold is a modern-pad choice; a digital trigger reads 1.0.
pub const TRIGGER_PRESS: f32 = 0.5;

/// The right stick's scale before quantization (defined in core, where the
/// aim assist reads it).
pub use core::sim::LOOK_SCALE;

/// Calls of [`SprintLatch::apply`] a click waits for the sim to start the
/// sprint (it needs 0.9 x MaxSpeed first) before it is dropped: 1.5 s at 60.
pub const SPRINT_WAIT: u32 = 90;

/// Forward stick (quantized `ly`) under which a latched sprint is dropped.
pub const SPRINT_KEEP_LY: i16 = 16384;

/// One click of L3 sprints (the maintainer, 2026-10-04, ours: the sim's sprint is
/// hold-to-sprint, and holding a stick click while pushing the stick is
/// awkward). The latch holds the SPRINT bit after a click until the sim's
/// sprint ends (energy, the stick's cone), the stick leaves forward, FIRE is
/// pressed, a second click, or [`SPRINT_WAIT`] passes with no sprint started.
/// Holding L3 still sprints as before. On foot only: in a seat SPRINT is the
/// boost and passes through.
#[derive(Clone, Copy, Debug, Default)]
pub struct SprintLatch {
    on: bool,
    prev_down: bool,
    seen: bool,
    age: u32,
}

impl SprintLatch {
    /// `pad` as sampled; `on_foot` and `sprinting` are the player's soldier
    /// before this tick. Call once per sampled pad.
    pub fn apply(&mut self, mut pad: PadState, on_foot: bool, sprinting: bool) -> PadState {
        let down = pad.buttons & buttons::SPRINT != 0;
        let edge = down && !self.prev_down;
        self.prev_down = down;
        if !on_foot {
            self.on = false;
            return pad;
        }
        if edge {
            self.on = !self.on;
            self.seen = false;
            self.age = 0;
        }
        if self.on {
            self.age += 1;
            self.seen |= sprinting;
            if (self.seen && !sprinting && !edge)
                || (!self.seen && self.age > SPRINT_WAIT)
                || pad.ly < SPRINT_KEEP_LY
                || pad.buttons & buttons::FIRE != 0
            {
                self.on = false;
            }
        }
        if self.on {
            pad.buttons |= buttons::SPRINT;
        }
        pad
    }
}

/// The project's SDL mappings, one line per pad (see the file's header).
pub const PADS_TXT: &str = include_str!("../pads.txt");

/// The one way the game opens gilrs. Mapping precedence (gilrs 0.11
/// `GilrsBuilder::build` inserts in this order, later wins per GUID):
/// `pads.txt` < gilrs's bundled SDL_GameControllerDB < `SDL_GAMECONTROLLERCONFIG`.
/// So `pads.txt` fills gaps in the DB; the env var still overrides anything.
/// The error is gilrs's, as text (its `Error` carries a whole dummy `Gilrs`).
pub fn open_gilrs() -> Result<gilrs::Gilrs, String> {
    gilrs::GilrsBuilder::new()
        .add_mappings(PADS_TXT)
        .build()
        .map_err(|e| e.to_string())
}

/// The SDL GUID string of a pad (the key of a `pads.txt` / DB line).
pub fn guid(pad: &gilrs::Gamepad<'_>) -> String {
    pad.uuid().iter().map(|b| format!("{b:02x}")).collect()
}

/// The logical controls a pad leaves unmapped: the sticks by axis name and
/// every button of `map` by button name.
pub fn unresolved(pad: &gilrs::Gamepad<'_>, map: &Mapping) -> Vec<String> {
    use gilrs::Axis;
    let mut out = Vec::new();
    for a in [
        Axis::LeftStickX,
        Axis::LeftStickY,
        Axis::RightStickX,
        Axis::RightStickY,
    ] {
        if pad.axis_code(a).is_none() {
            out.push(format!("{a:?}"));
        }
    }
    for &(b, _) in &map.table {
        if pad.button_code(b).is_none() {
            out.push(format!("{b:?}"));
        }
    }
    out
}

/// The warning for a pad whose camera stick or FIRE resolves to nothing
/// (from [`unresolved`]'s list), or `None` when both work.
pub fn pad_warning(name: &str, guid: &str, missing: &[String]) -> Option<String> {
    pad_warning_with(name, guid, missing, &Mapping::ps2_default())
}

/// Like [`pad_warning`], but FIRE follows the active layout's physical button.
pub fn pad_warning_with(
    name: &str,
    guid: &str,
    missing: &[String],
    map: &Mapping,
) -> Option<String> {
    let fire_buttons: Vec<String> = map
        .table
        .iter()
        .filter(|(_, bits)| *bits & buttons::FIRE != 0)
        .map(|(button, _)| format!("{button:?}"))
        .collect();
    let dead: Vec<&str> = missing
        .iter()
        .map(String::as_str)
        .filter(|m| {
            matches!(*m, "RightStickX" | "RightStickY")
                || fire_buttons.iter().any(|button| button.as_str() == *m)
        })
        .collect();
    (!dead.is_empty()).then(|| {
        format!(
            "controller '{name}' has no mapping for {} — camera/fire will not work; \
             add an SDL line for GUID {guid} to crates/input/pads.txt or SDL_GAMECONTROLLERCONFIG",
            dead.join(", ")
        )
    })
}

/// Print one line per connected pad (name, GUID, mapping source, what did
/// not resolve) — "0 pads" when none — and return the warning
/// ([`pad_warning`]) of the pad that is read, which the caller shows on screen.
pub fn log_pads(g: &gilrs::Gilrs) -> Option<String> {
    log_pads_with(g, &Mapping::ps2_default(), &ActivePad::default())
}

/// Like [`log_pads`], using the player's active layout for missing controls.
/// Every pad is logged; only the pad `active` picks gives the returned warning.
pub fn log_pads_with(
    g: &gilrs::Gilrs,
    map: &Mapping,
    active: &ActivePad<gilrs::GamepadId>,
) -> Option<String> {
    let mut n = 0;
    let usable = usable_ids(g);
    for (gid, pad) in g.gamepads() {
        n += 1;
        let (name, id) = (pad.name(), guid(&pad));
        let missing = unresolved(&pad, map);
        println!(
            "pad {n}: '{name}' guid {id} mapping {:?} ({}); unresolved: {}",
            pad.mapping_source(),
            pad.map_name().unwrap_or("no SDL entry"),
            if missing.is_empty() {
                "none".to_string()
            } else {
                missing.join(" ")
            }
        );
        if !usable.contains(&gid) {
            println!("pad {n}: ignored, the twin of a standard-layout pad");
        } else if let Some(w) = pad_warning_with(name, &id, &missing, map) {
            eprintln!("WARNING: {w}");
        }
    }
    if n == 0 {
        println!("0 pads connected — controls need one (gilrs)");
    }
    active_pad_warning(g, map, active)
}

/// The on-screen warning for the pad that is read now, if any.
pub fn active_pad_warning(
    g: &gilrs::Gilrs,
    map: &Mapping,
    active: &ActivePad<gilrs::GamepadId>,
) -> Option<String> {
    let (_, pad) = active_gamepad(g, active)?;
    pad_warning_with(pad.name(), &guid(&pad), &unresolved(&pad, map), map)
}

/// How long the active pad must be idle before input on another pad takes
/// over, so one press seen on two devices at once cannot flip between them.
pub const PAD_SWITCH_IDLE: std::time::Duration = std::time::Duration::from_millis(500);

/// How far a stick or trigger must travel to read as the player using the
/// pad, not as drift. `[ours]`
pub const DELIBERATE: f32 = 0.5;

/// The pad the player is using (issue #56): the one that most recently sent
/// real input, not gilrs's first pad, which on Windows can be a silent HID
/// twin of an XInput pad. Generic over the ID so it tests without hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivePad<Id> {
    pub id: Option<Id>,
    /// When the active pad last sent input.
    last: Option<std::time::Instant>,
    /// Counts deliberate input ([`DELIBERATE`] or more) on the active pad.
    /// A drifting stick keeps a pad awake but does not count here, so it
    /// cannot take the prompts off the keyboard.
    pub deliberate: u64,
}

impl<Id> Default for ActivePad<Id> {
    fn default() -> Self {
        Self {
            id: None,
            last: None,
            deliberate: 0,
        }
    }
}

impl<Id: Copy + PartialEq> ActivePad<Id> {
    /// A press, or a stick/trigger value past [`DEADZONE`], at `now`: it
    /// keeps the active pad awake, or takes over once the active pad has
    /// been idle for [`PAD_SWITCH_IDLE`]. Resting noise does neither.
    pub fn input(&mut self, id: Id, value: f32, now: std::time::Instant) {
        if value.abs() < DEADZONE {
            return;
        }
        let idle = self
            .last
            .is_none_or(|t| now.saturating_duration_since(t) >= PAD_SWITCH_IDLE);
        if self.id == Some(id) || self.id.is_none() || idle {
            self.id = Some(id);
            self.last = Some(now);
            if value.abs() >= DELIBERATE {
                self.deliberate += 1;
            }
        }
    }

    /// The active pad left: fall back to the first pad until another moves.
    pub fn disconnected(&mut self, id: Id) {
        if self.id == Some(id) {
            self.id = None;
            self.last = None;
        }
    }

    /// The active pad when it is still connected, else the first connected.
    pub fn pick(&self, connected: impl IntoIterator<Item = Id>) -> Option<Id> {
        let mut first = None;
        for id in connected {
            if self.id == Some(id) {
                return Some(id);
            }
            first.get_or_insert(id);
        }
        first
    }
}

/// How gilrs knows a pad's layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadKind {
    /// The OS gives the standard layout: a Windows.Gaming.Input `Gamepad`
    /// (XInput-class), which gilrs reports with an all-zero GUID.
    Unified,
    /// An SDL mapping line (gilrs's DB, `pads.txt` or the env var).
    Sdl,
    /// Neither: raw button numbers read as gilrs's native codes.
    Raw,
}

/// [`PadKind`] of a connected pad.
pub fn pad_kind(pad: &gilrs::Gamepad<'_>) -> PadKind {
    if pad.uuid() == [0; 16] {
        PadKind::Unified
    } else if pad.mapping_source() == gilrs::MappingSource::SdlMappings {
        PadKind::Sdl
    } else {
        PadKind::Raw
    }
}

/// Which pad a controller is, as far as the input library can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadFamily {
    /// DualShock 2 or 3, through an adapter or natively.
    Ds2,
    Ds4,
    Ds5,
    XboxOriginal,
    /// Xbox One, Series and 360: the same buttons, the XInput class. Also any
    /// pad nothing identifies, as most third-party pads copy this layout.
    XboxOne,
}

const SONY: u16 = 0x054c;
const MICROSOFT: u16 = 0x045e;
/// GameSir's USB vendor ID: its G7 Pro is 3537:1022.
const GAMESIR: u16 = 0x3537;
const SONY_DS3: [u16; 1] = [0x0268];
const SONY_DS4: [u16; 3] = [0x05c4, 0x09cc, 0x0ba0];
const SONY_DS5: [u16; 2] = [0x0ce6, 0x0df2];
const MS_DUKE: [u16; 5] = [0x0202, 0x0285, 0x0287, 0x0288, 0x0289];
const MS_XINPUT: [u16; 14] = [
    0x028e, 0x028f, 0x0291, 0x02a1, 0x0719, 0x02d1, 0x02dd, 0x02e3, 0x02ea, 0x02fd, 0x0b00, 0x0b05,
    0x0b12, 0x0b13,
];

/// [`PadFamily`] from what the OS reports: the USB IDs first, then the name,
/// then what is left of a Sony ID. Anything else is [`PadFamily::XboxOne`], the
/// layout most third-party pads copy; a wrong guess only changes prompts.
pub fn family_of(name: &str, hw: Option<HwId>, kind: PadKind) -> PadFamily {
    match hw {
        Some((SONY, p)) if SONY_DS3.contains(&p) => return PadFamily::Ds2,
        Some((SONY, p)) if SONY_DS4.contains(&p) => return PadFamily::Ds4,
        Some((SONY, p)) if SONY_DS5.contains(&p) => return PadFamily::Ds5,
        Some((MICROSOFT, p)) if MS_DUKE.contains(&p) => return PadFamily::XboxOriginal,
        Some((MICROSOFT, p)) if MS_XINPUT.contains(&p) => return PadFamily::XboxOne,
        Some((GAMESIR, _)) => return PadFamily::XboxOne,
        _ => {}
    }
    let name = name.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    if has(&["gamesir"]) {
        PadFamily::XboxOne
    } else if has(&["dualsense", "ps5"]) {
        PadFamily::Ds5
    } else if has(&[
        "dualshock 4",
        "dualshock4",
        "ps4",
        // The kernel's name for the first DS4 alone.
        "sony computer entertainment",
    ]) {
        PadFamily::Ds4
    } else if has(&[
        "dualshock",
        "playstation",
        "ps2",
        "ps3",
        "sony interactive entertainment wireless",
    ]) {
        // The kernel names a DS4 and a DS5 alike; their IDs were read above.
        PadFamily::Ds2
    } else if has(&[
        "xbox one",
        "xbox series",
        "xbox 360",
        "x-box 360",
        "x-box one",
        // The Linux xpad driver's name for a modern pad not in its table.
        "generic x-box pad",
    ]) || has(&["xbox wireless", "xinput"])
    {
        PadFamily::XboxOne
    } else if has(&["x-box", "xbox"]) {
        // Windows' own name for its XInput class says Xbox and nothing more.
        if kind == PadKind::Unified {
            PadFamily::XboxOne
        } else {
            PadFamily::XboxOriginal
        }
    } else if matches!(hw, Some((SONY, _))) {
        // A Sony pad newer than the IDs above: still PlayStation symbols.
        PadFamily::Ds4
    } else {
        PadFamily::XboxOne
    }
}

/// [`PadFamily`] of a connected pad.
pub fn pad_family(pad: &gilrs::Gamepad<'_>) -> PadFamily {
    family_of(pad.name(), hw_id(pad), pad_kind(pad))
}

/// A pad's hardware identity: its USB vendor and product IDs, when the OS
/// reports both.
pub type HwId = (u16, u16);

/// [`HwId`] of a connected pad.
pub fn hw_id(pad: &gilrs::Gamepad<'_>) -> Option<HwId> {
    Some((pad.vendor_id()?, pad.product_id()?))
}

/// The pads worth reading, in order. An unmapped raw pad is left out only when
/// a [`PadKind::Unified`] pad with the same [`HwId`] is connected: on Windows
/// that is the HID twin of an XInput pad, whose raw buttons land on the wrong
/// controls (its raw R3, button 9, reads as Start). Any other raw pad stays.
pub fn usable_pads<Id>(
    pads: impl IntoIterator<Item = (Id, PadKind, Option<HwId>)>,
) -> Vec<(Id, PadKind)> {
    let pads: Vec<_> = pads.into_iter().collect();
    let unified: Vec<HwId> = pads
        .iter()
        .filter(|(_, k, _)| *k == PadKind::Unified)
        .filter_map(|(_, _, hw)| *hw)
        .collect();
    pads.into_iter()
        .filter(|(_, k, hw)| *k != PadKind::Raw || !hw.is_some_and(|h| unified.contains(&h)))
        .map(|(id, k, _)| (id, k))
        .collect()
}

fn usable_ids(g: &gilrs::Gilrs) -> Vec<gilrs::GamepadId> {
    usable_pads(
        g.gamepads()
            .map(|(id, pad)| (id, pad_kind(&pad), hw_id(&pad))),
    )
    .into_iter()
    .map(|(id, _)| id)
    .collect()
}

/// Drain gilrs's event queue (keeping the cached state fresh), noting which pad
/// last sent input in `active`; true when a pad connected or disconnected, so the
/// caller re-runs [`log_pads`]. Pads [`usable_pads`] leaves out never become active.
pub fn pump_tracking(g: &mut gilrs::Gilrs, active: &mut ActivePad<gilrs::GamepadId>) -> bool {
    use gilrs::EventType as E;
    let usable = usable_ids(g);
    let skipped: Vec<_> = g
        .gamepads()
        .map(|(id, _)| id)
        .filter(|id| !usable.contains(id))
        .collect();
    let now = std::time::Instant::now();
    let mut changed = false;
    while let Some(ev) = g.next_event() {
        let tracked = !skipped.contains(&ev.id);
        match ev.event {
            E::ButtonPressed(..) if tracked => active.input(ev.id, 1.0, now),
            E::ButtonChanged(_, v, _) | E::AxisChanged(_, v, _) if tracked => {
                active.input(ev.id, v, now)
            }
            E::Disconnected => active.disconnected(ev.id),
            _ => {}
        }
        changed |= matches!(ev.event, E::Connected | E::Disconnected);
    }
    changed
}

/// The pad to read this frame: [`ActivePad::pick`] over [`usable_pads`].
pub fn active_gamepad<'a>(
    g: &'a gilrs::Gilrs,
    active: &ActivePad<gilrs::GamepadId>,
) -> Option<(gilrs::GamepadId, gilrs::Gamepad<'a>)> {
    let id = active.pick(usable_ids(g))?;
    Some((id, g.gamepad(id)))
}

/// A mapped button is down: pressed, or (an analog trigger) past
/// [`TRIGGER_PRESS`] — gilrs's own axis-to-button press is at 0.75.
pub fn is_down(pad: &gilrs::Gamepad<'_>, b: Button) -> bool {
    pad.is_pressed(b)
        || (matches!(b, Button::RightTrigger2 | Button::LeftTrigger2)
            && pad
                .button_data(b)
                .is_some_and(|d| d.value() >= TRIGGER_PRESS))
}

fn dz(v: f32) -> f32 {
    if v.abs() < DEADZONE {
        0.0
    } else {
        v
    }
}

/// A left-stick axis: the deadzone, then [`STICK_FULL`] reads as 1.
fn move_axis(v: f32) -> f32 {
    (dz(v) / STICK_FULL).clamp(-1.0, 1.0)
}

/// Card (the maintainer, 2026-10-20): the left stick works the menus and the spawn map
/// like the D-pad. A push past this reads as an arm; one already held stays
/// until the stick falls under [`STICK_ARM_OFF`], so a wobble near the edge
/// does not click twice.
pub const STICK_ARM_ON: f32 = 0.6;
pub const STICK_ARM_OFF: f32 = 0.35;
/// The D-pad arm bits a stick position stands for (1 up, 2 down, 4 left,
/// 8 right: the menu's edge bits): the dominant axis only, so a diagonal
/// never presses two arms. `held` is the arms read last frame (hysteresis);
/// +y is up, as gilrs reports it.
pub fn stick_arms(x: f32, y: f32, held: u32) -> u32 {
    let (ax, ay) = (x.abs(), y.abs());
    let (v, along_x) = if ax >= ay { (ax, true) } else { (ay, false) };
    let arm = match (along_x, if along_x { x } else { y } > 0.0) {
        (true, true) => 8,
        (true, false) => 4,
        (false, true) => 1,
        (false, false) => 2,
    };
    if v >= STICK_ARM_ON || (held & arm != 0 && v >= STICK_ARM_OFF) {
        arm
    } else {
        0
    }
}

/// Sample with the previous gameplay stick arms, returning the new latch.
/// The caller owns controller selection and resets `held` on disconnect/change.
pub fn pad_from_gamepad_with_arms(
    pad: &gilrs::Gamepad<'_>,
    map: &Mapping,
    held: u32,
) -> (PadState, u32) {
    use gilrs::Axis;
    let mut b = 0u32;
    for &(gb, bit) in &map.table {
        if is_down(pad, gb) {
            b |= bit;
        }
    }

    // The left stick also moves the spawn map's post while dead (the sim
    // reads post bits only dead; alive the stick walks).
    let arms = stick_arms(
        pad.value(Axis::LeftStickX),
        pad.value(Axis::LeftStickY),
        held,
    );
    for (arm, bit) in [
        (1, buttons::SPAWN_POST_UP),
        (2, buttons::SPAWN_POST_DOWN),
        (4, buttons::SPAWN_POST_LEFT),
        (8, buttons::SPAWN_POST_RIGHT),
    ] {
        if arms & arm != 0 {
            b |= bit;
        }
    }

    (
        PadState {
            lx: PadState::quantize(move_axis(pad.value(Axis::LeftStickX))),
            ly: PadState::quantize(move_axis(pad.value(Axis::LeftStickY))),
            rx: PadState::quantize(dz(pad.value(Axis::RightStickX)) * LOOK_SCALE),
            ry: PadState::quantize(dz(pad.value(Axis::RightStickY)) * LOOK_SCALE),
            buttons: b,
            ..PadState::default()
        },
        arms,
    )
}

/// A flyer pilot's sticks: the left stick pitches and turns, the right throttles
/// and rolls. Both games' default tables for a flyer swap the two sticks' functions
/// against the soldier's and the ground vehicle's (BF2 0x600ee0 words 16..19, 32 33 30
/// 31, against 30 31 32 33 at 0x600e40; BF1 file offset 0x3ba210; read back by BF2
/// `FUN_004333a0`, BF1 `FUN_0034a7e8`). The look stick's own [`LOOK_SCALE`] comes
/// off: a flyer's stick reaches 1, and BF2's rate blend runs over 0.7 to 1.
/// Only the controller's axes: the keyboard's walk keys are merged after this.
pub fn flyer_sticks(pad: PadState) -> PadState {
    let unscaled = |a: i16| PadState::quantize((PadState::axis(a) / LOOK_SCALE).clamp(-1.0, 1.0));
    PadState {
        lx: unscaled(pad.rx),
        ly: unscaled(pad.ry),
        rx: pad.lx,
        ry: pad.ly,
        ..pad
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flyer_pilots_sticks_swap_and_reach_full() {
        let pad = PadState {
            lx: PadState::quantize(1.0),
            ly: PadState::quantize(-1.0),
            rx: PadState::quantize(LOOK_SCALE),
            ry: PadState::quantize(-LOOK_SCALE / 2.0),
            buttons: buttons::FIRE,
            ..PadState::default()
        };
        let f = flyer_sticks(pad);
        assert_eq!((f.rx, f.ry), (pad.lx, pad.ly));
        assert!((PadState::axis(f.lx) - 1.0).abs() < 1e-3);
        assert!((PadState::axis(f.ly) + 0.5).abs() < 1e-3);
        assert_eq!(f.buttons, pad.buttons);
    }

    /// The USB IDs decide first; the name only when they say nothing.
    #[test]
    fn pads_are_named_by_id_then_by_name() {
        use PadFamily as F;
        use PadKind::{Raw, Sdl, Unified};
        for (hw, family) in [
            ((0x054c, 0x05c4), F::Ds4),
            ((0x054c, 0x09cc), F::Ds4),
            ((0x054c, 0x0ce6), F::Ds5),
            ((0x054c, 0x0df2), F::Ds5),
            ((0x054c, 0x0268), F::Ds2),
            ((0x045e, 0x0202), F::XboxOriginal),
            ((0x045e, 0x0289), F::XboxOriginal),
            ((0x045e, 0x02ea), F::XboxOne),
            ((0x045e, 0x0b12), F::XboxOne),
            ((0x045e, 0x028e), F::XboxOne),
            // The maintainer's GameSir G7 Pro, and any other GameSir ID.
            ((0x3537, 0x1022), F::XboxOne),
            ((0x3537, 0x0001), F::XboxOne),
            // A Sony ID not listed stays PlayStation.
            ((0x054c, 0x0fff), F::Ds4),
            // Vendors nothing names: the fallback.
            ((0x2dc8, 0x3106), F::XboxOne),
            ((0x057e, 0x2009), F::XboxOne),
        ] {
            assert_eq!(family_of("pad", Some(hw), Sdl), family, "{hw:04x?}");
        }
        // The kernel's name for a DS4 and a DS5 alike: the ID tells them apart.
        let kernel = "Sony Interactive Entertainment Wireless Controller";
        assert_eq!(family_of(kernel, Some((0x054c, 0x0ce6)), Sdl), F::Ds5);
        assert_eq!(family_of(kernel, Some((0x054c, 0x09cc)), Sdl), F::Ds4);
        for (name, kind, family) in [
            ("DualSense Wireless Controller", Sdl, F::Ds5),
            ("PS4 Controller", Sdl, F::Ds4),
            ("Sony DualShock 4", Sdl, F::Ds4),
            ("PS2 to USB adapter", Raw, F::Ds2),
            ("Microsoft X-Box pad v1 (US)", Sdl, F::XboxOriginal),
            ("Microsoft X-Box 360 pad", Sdl, F::XboxOne),
            ("Microsoft X-Box One pad", Sdl, F::XboxOne),
            ("Xbox Series X Controller", Sdl, F::XboxOne),
            ("Xbox Controller", Unified, F::XboxOne),
            ("Controller (GameSir G7 Pro)", Unified, F::XboxOne),
            ("GameSir G7 Pro", Sdl, F::XboxOne),
            ("GameSir-T4 PS4 mode", Raw, F::XboxOne),
            ("Generic X-Box pad", Sdl, F::XboxOne),
            (
                "Sony Computer Entertainment Wireless Controller",
                Sdl,
                F::Ds4,
            ),
            ("Sony PLAYSTATION(R)3 Controller", Sdl, F::Ds2),
            ("Twin USB Joystick", Raw, F::XboxOne),
            ("", Raw, F::XboxOne),
        ] {
            assert_eq!(family_of(name, None, kind), family, "{name}");
        }
    }

    /// A drifting stick keeps a pad awake and does not count as using it.
    #[test]
    fn only_deliberate_input_counts_as_using_the_pad() {
        let now = std::time::Instant::now();
        let mut a = ActivePad::<u8>::default();
        a.input(1, 0.2, now);
        assert_eq!(a.id, Some(1), "drift wakes the pad");
        assert_eq!(a.deliberate, 0);
        a.input(1, 0.9, now);
        assert_eq!(a.deliberate, 1);
        // A press on a pad that cannot take over yet counts for nothing.
        a.input(2, 1.0, now);
        assert_eq!((a.id, a.deliberate), (Some(1), 1));
    }

    #[test]
    fn active_pad_follows_input_and_falls_back_to_first() {
        let t0 = std::time::Instant::now();
        let later = t0 + PAD_SWITCH_IDLE;
        let mut a = ActivePad::<u8>::default();
        assert_eq!(a.pick([1, 2]), Some(1), "no input yet: the first pad");
        a.input(1, 0.05, t0);
        a.input(2, -0.05, t0);
        assert_eq!(a.pick([1, 2]), Some(1), "stick noise does not steal focus");
        a.input(2, 1.0, t0);
        assert_eq!(a.pick([1, 2]), Some(2), "the pad that sent input");
        a.input(1, 1.0, t0 + PAD_SWITCH_IDLE / 2);
        assert_eq!(a.id, Some(2), "the active pad was not idle long enough");
        a.input(1, 1.0, later);
        assert_eq!(
            a.pick([1, 2]),
            Some(1),
            "idle active pad: the other takes over"
        );
        assert_eq!(a.pick([2, 3]), Some(2), "active pad gone: the first");
        a.disconnected(2);
        assert_eq!(a.id, Some(1), "another pad leaving keeps the active one");
        a.disconnected(1);
        assert_eq!(a.id, None);
        a.input(3, 1.0, later);
        assert_eq!(
            a.id,
            Some(3),
            "after a disconnect the next input wins at once"
        );
        assert_eq!(a.pick(std::iter::empty()), None);
    }

    #[test]
    fn twin_pads_pressed_together_do_not_flip() {
        // One physical press reaches the XInput pad (1) and its HID twin (2)
        // in the same pump, in either order, frame after frame.
        let t0 = std::time::Instant::now();
        let frame = std::time::Duration::from_millis(16);
        let mut a = ActivePad::<u8>::default();
        a.input(1, 1.0, t0);
        a.input(2, 1.0, t0);
        for f in 1..100 {
            let now = t0 + frame * f;
            a.input(2, 1.0, now);
            a.input(1, 1.0, now);
            assert_eq!(a.id, Some(1));
        }
        // The raw HID twin is not even a candidate while its twin is there,
        let hw = Some((0x045e, 0x02ea));
        let pads = [(2, PadKind::Raw, hw), (1, PadKind::Unified, hw)];
        let ids: Vec<_> = usable_pads(pads).into_iter().map(|(id, _)| id).collect();
        assert_eq!(ids, [1]);
        assert_eq!(a.pick(ids), Some(1));
        // but a raw pad alone, or beside an SDL-mapped one, is still read.
        assert_eq!(usable_pads([(2, PadKind::Raw, hw)]).len(), 1);
        assert_eq!(
            usable_pads([(2, PadKind::Raw, hw), (3, PadKind::Sdl, hw)]).len(),
            2
        );
    }

    #[test]
    fn independent_raw_pad_beside_a_standard_pad_is_still_read() {
        let xbox = Some((0x045e, 0x02ea));
        let other = Some((0x3537, 0x1022));
        for raw_hw in [other, None] {
            let pads = [(1u8, PadKind::Unified, xbox), (2, PadKind::Raw, raw_hw)];
            let ids: Vec<_> = usable_pads(pads).into_iter().map(|(id, _)| id).collect();
            assert_eq!(ids, [1, 2], "raw pad {raw_hw:?} is not a confirmed twin");
        }
        // A standard pad with no identity confirms no twin either.
        let ids: Vec<_> = usable_pads([(1u8, PadKind::Unified, None), (2, PadKind::Raw, None)])
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids, [1, 2]);
    }

    #[test]
    fn start_and_unknown_cannot_replace_gameplay_controls() {
        let mut mapping = Mapping::default();
        let before = mapping.clone();
        for button in [Button::Start, Button::Unknown] {
            mapping.remap(0, button);
            assert_eq!(mapping, before);
        }
    }

    #[test]
    fn saved_controller_layouts_keep_each_action_and_unique_known_buttons() {
        let mut valid = Mapping::default();
        valid.remap(0, Button::West);
        valid.table.reverse();
        assert!(valid.is_valid());

        let mut duplicate = valid.clone();
        duplicate.table[1].0 = duplicate.table[0].0;
        let mut unknown_action = valid.clone();
        unknown_action.table[0].1 |= buttons::RESPAWN;
        let mut missing = valid.clone();
        missing.table.pop();
        let mut repeated_action = valid.clone();
        repeated_action.table[1].1 = repeated_action.table[0].1;
        let mut unknown_button = valid.clone();
        unknown_button.table[0].0 = Button::Unknown;
        for invalid in [
            duplicate,
            unknown_action,
            missing,
            repeated_action,
            unknown_button,
        ] {
            assert!(!invalid.is_valid(), "{invalid:?}");
        }
    }

    #[test]
    fn fire_warnings_follow_the_remapped_button_and_keep_default_api_behavior() {
        let mut mapping = Mapping::default();
        mapping.remap(0, Button::West);
        let west = vec!["West".to_string()];
        let r2 = vec!["RightTrigger2".to_string()];
        assert!(pad_warning("p", "g", &west).is_none());
        assert!(pad_warning_with("p", "g", &west, &mapping)
            .unwrap()
            .contains("West"));
        assert!(pad_warning("p", "g", &r2).is_some());
        assert!(pad_warning_with("p", "g", &r2, &mapping).is_none());
        assert!(pad_warning_with("p", "g", &["RightStickY".to_string()], &mapping).is_some());
        assert!(pad_warning_with("p", "g", &[], &mapping).is_none());
    }

    #[test]
    fn remapping_swaps_conflicting_buttons_without_duplicate_actions() {
        let mut mapping = Mapping::default();
        let fire = mapping.table[0];
        let reload = mapping
            .table
            .iter()
            .position(|(button, _)| *button == Button::West)
            .unwrap();
        let old_reload = mapping.table[reload];
        mapping.remap(0, Button::West);
        assert_eq!(mapping.table[0], (Button::West, fire.1));
        assert_eq!(mapping.table[reload], (fire.0, old_reload.1));
        assert_eq!(
            mapping
                .table
                .iter()
                .filter(|(button, _)| *button == Button::West)
                .count(),
            1
        );
    }

    #[test]
    fn the_left_stick_reads_as_the_dpad_arms() {
        assert_eq!(stick_arms(0.0, 1.0, 0), 1, "up");
        assert_eq!(stick_arms(0.0, -1.0, 0), 2, "down");
        assert_eq!(stick_arms(-1.0, 0.0, 0), 4, "left");
        assert_eq!(stick_arms(1.0, 0.0, 0), 8, "right");
        assert_eq!(stick_arms(0.0, 0.0, 0), 0, "centred");
        assert_eq!(stick_arms(0.5, 0.0, 0), 0, "a half push is not a click");
        // A diagonal presses its stronger axis only, never two arms.
        assert_eq!(stick_arms(0.7, 0.9, 0), 1);
        assert_eq!(stick_arms(0.9, 0.7, 0), 8);
        // Hysteresis: a held arm stays down to 0.35 and a new one needs 0.6.
        assert_eq!(stick_arms(0.0, 0.45, 1), 1);
        assert_eq!(stick_arms(0.0, 0.45, 0), 0);
        assert_eq!(stick_arms(0.0, 0.3, 1), 0);
    }

    fn fwd(buttons: u32) -> PadState {
        PadState {
            ly: 32767,
            buttons,
            ..PadState::default()
        }
    }

    /// One click sprints until the sim's sprint ends; the stick, FIRE, a
    /// second click and a sprint that never starts each drop it; a seat's
    /// boost is not latched.
    #[test]
    fn a_sprint_click_latches_until_the_sprint_ends() {
        use buttons::{FIRE, SPRINT};
        let on = |p: PadState| p.buttons & SPRINT != 0;
        let mut l = SprintLatch::default();
        assert!(on(l.apply(fwd(SPRINT), true, false)));
        assert!(
            on(l.apply(fwd(0), true, false)),
            "released, not yet sprinting"
        );
        assert!(on(l.apply(fwd(0), true, true)));
        assert!(!on(l.apply(fwd(0), true, false)), "the sim's sprint ended");

        let mut l = SprintLatch::default();
        l.apply(fwd(SPRINT), true, false);
        assert!(
            !on(l.apply(PadState::default(), true, true)),
            "stick let go"
        );

        let mut l = SprintLatch::default();
        l.apply(fwd(SPRINT), true, false);
        assert!(!on(l.apply(fwd(FIRE), true, true)), "FIRE");

        let mut l = SprintLatch::default();
        l.apply(fwd(SPRINT), true, false);
        l.apply(fwd(0), true, true);
        assert!(
            on(l.apply(fwd(SPRINT), true, true)),
            "held: still the button's"
        );
        assert!(
            !on(l.apply(fwd(0), true, true)),
            "the second click took it off"
        );

        let mut l = SprintLatch::default();
        l.apply(fwd(SPRINT), true, false);
        for _ in 0..SPRINT_WAIT {
            l.apply(fwd(0), true, false);
        }
        assert!(
            !on(l.apply(fwd(0), true, false)),
            "no sprint started in time"
        );

        let mut l = SprintLatch::default();
        assert!(
            on(l.apply(fwd(SPRINT), false, false)),
            "a seat's boost passes"
        );
        assert!(!on(l.apply(fwd(0), false, false)), "and is not latched");
    }

    /// PADM: the G7 Pro line in `pads.txt`, resolved the way SDL resolves a
    /// generic Linux HID pad (`bN` = the N-th key code the device reports,
    /// `aN` = the N-th abs axis) against the device Claude probed on the maintainer's
    /// pad: keys 304..=318, abs X Y Z RZ GAS BRAKE (+ hat 0). The camera is
    /// on ABS_Z/ABS_RZ, SDL's x/y (West/North) are 307/308, both triggers are
    /// analog, and every logical control the game reads has a source.
    #[test]
    fn the_g7_pro_line_resolves_camera_triggers_and_face_buttons() {
        let line = PADS_TXT
            .lines()
            .find(|l| l.starts_with("03000000373500002210000010010000,"))
            .expect("G7 Pro line in pads.txt");
        let field = |k: &str| {
            line.split(',')
                .find_map(|f| f.strip_prefix(k).and_then(|v| v.strip_prefix(':')))
                .unwrap_or_else(|| panic!("{k} missing"))
        };
        let keys: Vec<u16> = (304..=318).collect();
        let abs: [u16; 6] = [0x00, 0x01, 0x02, 0x05, 0x09, 0x0a]; // X Y Z RZ GAS BRAKE
        let key = |k: &str| {
            keys[field(k)
                .strip_prefix('b')
                .unwrap()
                .parse::<usize>()
                .unwrap()]
        };
        let axis = |k: &str| {
            abs[field(k)
                .strip_prefix('a')
                .unwrap()
                .parse::<usize>()
                .unwrap()]
        };
        assert_eq!((axis("rightx"), axis("righty")), (0x02, 0x05)); // ABS_Z, ABS_RZ
        assert_eq!((axis("leftx"), axis("lefty")), (0x00, 0x01));
        assert_eq!((key("x"), key("y")), (307, 308));
        assert_eq!((key("a"), key("b")), (304, 305));
        assert_eq!((axis("righttrigger"), axis("lefttrigger")), (0x09, 0x0a));
        for k in [
            "leftshoulder",
            "rightshoulder",
            "leftstick",
            "rightstick",
            "back",
            "start",
            "dpup",
            "dpdown",
            "dpleft",
            "dpright",
        ] {
            field(k);
        }
        assert!(line.ends_with("platform:Linux,"));
    }

    #[test]
    fn a_dead_camera_or_fire_warns_and_nothing_else_does() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(pad_warning("p", "g", &[]).is_none());
        assert!(pad_warning("p", "g", &s(&["Select", "LeftThumb"])).is_none());
        let w = pad_warning("G7", "0300", &s(&["RightStickX", "RightStickY"])).unwrap();
        assert!(
            w.contains("RightStickX, RightStickY") && w.contains("0300"),
            "{w}"
        );
        assert!(pad_warning("p", "g", &s(&["RightTrigger2"])).is_some());
    }

    #[test]
    fn deadzone_and_quantization_round_trip() {
        assert_eq!(dz(0.1), 0.0);
        assert_eq!(dz(-0.14), 0.0);
        assert_eq!(dz(0.2), 0.2);
        // A pad that tops out short of 1.0 still gives a full push (SPUP).
        assert_eq!(PadState::quantize(move_axis(0.984)), 32767);
        assert_eq!(PadState::quantize(move_axis(-0.96)), -32767);
        assert_eq!(move_axis(0.1), 0.0);
        assert!((move_axis(0.475) - 0.5).abs() < 1e-6);
        // Quantize/axis round-trip stays within one quantum.
        for v in [-1.0f32, -0.5, 0.0, 0.33, 0.999, 1.0] {
            let q = PadState::quantize(v);
            assert!((PadState::axis(q) - v).abs() < 1.0 / 32000.0, "{v}");
        }
        // Out-of-range hardware values clamp instead of wrapping.
        assert_eq!(PadState::quantize(1.7), 32767);
        assert_eq!(PadState::quantize(-1.7), -32767);
    }

    #[test]
    fn button_bits_are_distinct() {
        use core::sim::buttons::*;
        let all = [
            FIRE,
            JUMP,
            ROLL_STANCE,
            SPRINT,
            ZOOM,
            RELOAD,
            ACTION,
            SWITCH_WEAPON,
            SECONDARY_FIRE,
            SQUAD_FOLLOW,
            SQUAD_HOLD,
            SQUAD_AT_EASE,
            SQUAD_MOVE_OUT,
            SPAWN_CLASS_NEXT,
            SPAWN_CLASS_PREV,
            SWITCH_SECONDARY,
            SPAWN_POST_UP,
            SPAWN_POST_DOWN,
            SPAWN_POST_LEFT,
            SPAWN_POST_RIGHT,
            SPAWN_ACCEPT,
            SPAWN_BACK,
        ];
        let mut seen = 0u32;
        for b in all {
            assert_eq!(seen & b, 0, "overlapping bit {b:#x}");
            seen |= b;
        }
    }

    fn of_bits(m: &Mapping, gb: Button) -> Option<u32> {
        m.table.iter().find(|(b, _)| *b == gb).map(|(_, x)| *x)
    }

    /// D10: the default table binds every bit the sim reads, binds each
    /// PHYSICAL button once, and gives the d-pad to the four squad commands
    /// with `SWITCH_WEAPON` moved off it.
    #[test]
    fn the_default_mapping_binds_each_button_and_each_bit_once() {
        use core::sim::buttons::*;
        let m = Mapping::ps2_default();
        let mut bits = 0u32;
        let mut seen: Vec<Button> = Vec::new();
        for &(gb, bit) in &m.table {
            assert_eq!(bits & bit, 0, "bit {bit:#x} bound twice");
            assert!(!seen.contains(&gb), "{gb:?} bound twice");
            bits |= bit;
            seen.push(gb);
        }
        for bit in [
            FIRE,
            SECONDARY_FIRE,
            JUMP,
            ROLL_STANCE,
            SPRINT,
            ZOOM,
            RELOAD,
            ACTION,
            SWITCH_WEAPON,
            SQUAD_FOLLOW,
            SQUAD_HOLD,
            SQUAD_AT_EASE,
            SQUAD_MOVE_OUT,
            SPAWN_CLASS_NEXT,
            SPAWN_CLASS_PREV,
            SPAWN_POST_UP,
            SPAWN_POST_DOWN,
            SPAWN_POST_LEFT,
            SPAWN_POST_RIGHT,
            SPAWN_ACCEPT,
            SPAWN_BACK,
        ] {
            assert_ne!(bits & bit, 0, "nothing presses {bit:#x}");
        }
        assert_eq!(of_bits(&m, Button::South), Some(JUMP | SPAWN_ACCEPT));
        assert_eq!(of_bits(&m, Button::North), Some(ACTION | SPAWN_BACK));
        let of = |gb: Button| m.table.iter().find(|(b, _)| *b == gb).map(|(_, x)| *x);
        assert_eq!(of(Button::DPadUp), Some(SQUAD_MOVE_OUT | SPAWN_POST_UP));
        assert_eq!(of(Button::DPadDown), Some(SQUAD_HOLD | SPAWN_POST_DOWN));
        assert_eq!(
            of(Button::DPadLeft),
            Some(SQUAD_AT_EASE | SPAWN_POST_LEFT | HERO_ACCEPT)
        );
        assert_eq!(
            of(Button::DPadRight),
            Some(SQUAD_FOLLOW | SPAWN_POST_RIGHT | HERO_DECLINE)
        );
        assert_eq!(of(Button::RightThumb), Some(ZOOM));
        assert_eq!(of(Button::LeftTrigger), Some(SWITCH_SECONDARY));
        assert_eq!(
            of(Button::RightTrigger),
            Some(SPAWN_CLASS_NEXT | SWITCH_WEAPON)
        );
    }
}
