//! BF2's HUD as the PS2 runs it (card BHUD2, `analysis/re-hud.md` §2): the
//! baked `hud_` tree (`manifest::hud`) turned into elements that listen to
//! named events, fade in and out, ease and pulse their colour — the state half
//! of the interpreter. [`crate::hud_draw`] lays the same elements out into
//! triangles; [`crate::hud::HudModel`] is what feeds the events.
//!
//! What is read from the ELF and what is the port's is tagged as in the RE
//! doc: `[V]` read, `[D]` derived, `[H]` hypothesis, `[ours]` a port call.
//! Everything not `[V]` is PROVISIONAL.
//!
//! `MultiLineText` (card EVMSG, `analysis/re-event-messages.md` §2) is the
//! event-message box: [`MultiLine`] queues, admits, expires and scrolls its
//! lines.
//!
//! `Map` is laid out by `crate::hud_map` (card BMAP).
//! A BarBitmap's flash copy over the span its value left or gained (card
//! HTGT, `analysis/re-target-hud.md` §5) is [`BarFlash`]; a group's `Color`
//! multiplies what its children draw ([`Hud::tint`], §6).
//!
//! Not interpreted (they wait for their own cards): `Target`,
//! `ObjectiveList`, `BorderedBox`, the localised static `Text("key")` lines,
//! `UseChangeColor`.

//! Runtime state and the shared target fixture stay here; regression tests
//! live in `hud_tree/tests.rs`.

use glam::{Mat4, Vec3};
use manifest::hud::{HudArg, HudFont, HudLayout, HudNode};
use std::collections::{HashMap, VecDeque};

/// The PS2 frame buffer the layout is authored against (`re-hud.md` §2.1 [M]:
/// DAT_007d5f70/74). Every element-local length is one of its pixels.
pub const FRAME_W: f32 = 512.0;
pub const FRAME_H: f32 = 448.0;

/// A value an event carries.
#[derive(Debug, Clone, PartialEq)]
pub enum EventValue {
    /// Only shows / hides.
    None,
    Number(f32),
    Text(String),
    Color([u8; 3]),
    /// A window position as a fraction of the window (the reticle).
    Position([f32; 2]),
}

/// What an element does with an event (`re-hud.md` §2.1–§2.4 [V]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Listen {
    Enable,
    Disable,
    Color,
    PulseRate,
    Value,
    Number,
    Text,
    Mesh,
    Bitmap,
    Position,
}

/// The fader (`re-hud.md` §2.1 [V][O], `FUN_003afca8` ff.): 0 hidden → 1 in →
/// 2 hold → 3 sustain → 4 out → 0. A zero time skips its state; the hold
/// defaults to forever.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fader {
    pub state: u8,
    pub t: f32,
    pub fade_in: f32,
    pub hold: f32,
    pub sustain: f32,
    pub fade_out: f32,
}

impl Default for Fader {
    fn default() -> Self {
        Self {
            state: 0,
            t: 0.0,
            fade_in: 0.0,
            hold: f32::MAX,
            sustain: 0.0,
            fade_out: 0.0,
        }
    }
}

impl Fader {
    fn time(&self, state: u8) -> f32 {
        match state {
            1 => self.fade_in,
            2 => self.hold,
            3 => self.sustain,
            4 => self.fade_out,
            _ => f32::MAX,
        }
    }

    /// Step to the next state whose time is not zero.
    fn settle(&mut self) {
        while self.state != 0 && self.time(self.state) <= 0.0 {
            self.state = (self.state + 1) % 5;
        }
    }

    pub fn alpha(&self) -> f32 {
        match self.state {
            0 => 0.0,
            1 => (self.t / self.fade_in).clamp(0.0, 1.0),
            4 => (1.0 - self.t / self.fade_out).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    pub fn show(&mut self) {
        match self.state {
            0 => {
                self.state = 1;
                self.t = 0.0;
            }
            2 => self.t = 0.0,
            3 => {
                self.state = 2;
                self.t = 0.0;
            }
            4 => {
                // In at the mirrored alpha.
                let a = self.alpha();
                self.state = 1;
                self.t = a * self.fade_in;
            }
            _ => {}
        }
        self.settle();
    }

    pub fn hide(&mut self) {
        match self.state {
            1 => {
                let a = self.alpha();
                self.state = 4;
                self.t = (1.0 - a) * self.fade_out;
            }
            2 => {
                self.state = 3;
                self.t = 0.0;
            }
            _ => {}
        }
        self.settle();
    }

    pub fn advance(&mut self, dt: f32) {
        if self.state == 0 {
            return;
        }
        self.t += dt;
        while self.state != 0 && self.t >= self.time(self.state) {
            self.t -= self.time(self.state);
            self.state = (self.state + 1) % 5;
            self.settle();
        }
        if self.state == 0 {
            self.t = 0.0;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

/// `BitmapRect` / `Rect`: a size in frame pixels and how it hangs on the
/// element's origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub w: f32,
    pub h: f32,
    pub h_align: HAlign,
    pub v_align: VAlign,
}

impl Rect {
    /// The rect's top-left corner in element-local pixels.
    pub fn origin(&self) -> (f32, f32) {
        let x = match self.h_align {
            HAlign::Left => 0.0,
            HAlign::Center => -self.w * 0.5,
            HAlign::Right => -self.w,
        };
        let y = match self.v_align {
            VAlign::Top => 0.0,
            VAlign::Center => -self.h * 0.5,
            VAlign::Bottom => -self.h,
        };
        (x, y)
    }
}

/// A `MeshInfo` record: the transform that REPLACES a Model3D's own while
/// that mesh is shown (`re-hud.md` §2.4 [V]).
#[derive(Debug, Clone, PartialEq)]
pub struct MeshInfo {
    pub mesh: String,
    pub pos: Vec3,
    pub rot: Vec3,
    pub scale: Vec3,
}

/// What an element draws.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Group,
    Bitmap,
    /// A Bitmap clipped by its value from the right (`re-hud.md` §2.2 [V]).
    BarBitmap,
    /// Its `segment` child's bitmap repeated around an arc or along a line.
    BarSegmented,
    Text,
    Model3D,
    /// The message box: its lines are [`Elem::lines`].
    MultiLineText,
    /// The minimap: drawn by `crate::hud_map` from [`Hud::map`].
    Map,
    /// A type this interpreter does not draw (module doc).
    Other,
}

/// Extra pen advance per glyph, in texels, of a line set with TextStyle
/// Shadow ([M] the PS2 frame `ref/evmsg/bf2_hot1_kill_line.png`: the 24
/// glyphs of "Ben Pulastra killed Player 1" sit at their advances plus one
/// texel each, no drift over 310 px; [H] that the style, not a spacing
/// default of the element, is what adds it).
pub const SHADOW_SPACING: f32 = 1.0;

/// One line of a [`MultiLine`].
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub color: [f32; 3],
    /// Where it sits, in lines down from the box's top.
    pub y: f32,
    /// 0.5 in, held until the box expires it, 0.5 out (PS2 RAM, §2 [M]).
    pub fader: Fader,
    /// Seconds fully shown, the frame it became so included ([M]: the PS2
    /// line starts fading 2.969 s after its first full frame, 89 frames of
    /// a DisplayTime of 3).
    pub held: f32,
}

/// What a [`MultiLine`] is busy with; it does one thing at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineState {
    Idle,
    /// The top line fades out (the PS2's state 2).
    Expiring,
    /// The lines under a gone top line move up (state 3).
    Scrolling,
    /// The newest line fades in (state 4).
    Admitting,
}

/// A MultiLineText's lines (`re-event-messages.md` §2 [V][O]: ctor
/// `FUN_003aa3e0`, add `FUN_003ab1e0`, admit `FUN_003aa908`, expire
/// `FUN_003aa620`, scroll `FUN_003aa9f8`).
#[derive(Debug, Clone, PartialEq)]
pub struct MultiLine {
    /// `NumLines`: how many lines the box holds, queued ones included.
    pub num_lines: usize,
    /// `DisplayTime`: how long a line is held before it may go.
    pub display_time: f32,
    /// `ScrollSpeed`, lines per second.
    pub scroll_speed: f32,
    /// `AlwaysScroll`: the top line goes when its time is up, wanted or not.
    pub always_scroll: bool,
    /// `#09771c74`: lines stack downward from the top; clear, a new line
    /// enters on the box's bottom line.
    pub stack_down: bool,
    /// `#cf649993`: the element hides when its last line is gone.
    pub hide_empty: bool,
    /// The Format's `TextStyle("Shadow")`.
    pub shadow: bool,
    /// The Format's `TextBreak("Word")`.
    pub word_break: bool,
    /// The colour the next message takes (`EventColor`); the Format's
    /// `Color` until an event sets one.
    pub color: [f32; 3],
    fade_in: f32,
    fade_out: f32,
    /// Wrapped lines waiting for a place, oldest first.
    pub pending: VecDeque<(String, [f32; 3])>,
    /// The lines on screen, top first.
    pub shown: Vec<Line>,
    pub state: LineState,
}

impl Default for MultiLine {
    fn default() -> Self {
        Self {
            num_lines: 1,
            // The ctor's defaults [V].
            display_time: 2.0,
            scroll_speed: 1.0,
            always_scroll: false,
            stack_down: false,
            hide_empty: false,
            shadow: false,
            word_break: false,
            color: [255.0; 3],
            fade_in: 0.0,
            fade_out: 0.0,
            pending: VecDeque::new(),
            shown: Vec::new(),
            state: LineState::Idle,
        }
    }
}

impl MultiLine {
    /// A message arrives, already wrapped into `lines`: with no free line it
    /// is dropped whole; else all its lines are queued in the current colour.
    /// `[ours]` The PS2 queues only as many lines as are free
    /// (`re-event-messages.md` section 2), which cuts a message off mid-sentence
    /// ("Ben Pulastra killed"); here the rest wait their turn for a line.
    pub fn add(&mut self, lines: Vec<String>) {
        if self.pending.len() + self.shown.len() >= self.num_lines {
            return;
        }
        for l in lines {
            self.pending.push_back((l, self.color));
        }
    }

    /// One frame. Returns true when the last line just went and the box
    /// hides itself (`hide_empty`).
    pub fn advance(&mut self, dt: f32) -> bool {
        for l in &mut self.shown {
            l.fader.advance(dt);
            if l.fader.state == 2 {
                l.held += dt;
            }
        }
        match self.state {
            LineState::Admitting => {
                if self.shown.last().is_none_or(|l| l.fader.state != 1) {
                    self.state = LineState::Idle;
                }
            }
            LineState::Expiring => {
                if self.shown.first().is_none_or(|l| l.fader.state == 0) {
                    if !self.shown.is_empty() {
                        self.shown.remove(0);
                    }
                    self.state = if self.shown.is_empty() {
                        LineState::Idle
                    } else {
                        LineState::Scrolling
                    };
                    return self.shown.is_empty() && self.pending.is_empty() && self.hide_empty;
                }
            }
            LineState::Scrolling => {
                // Line height × ScrollSpeed × clamp(y / line height, 0.5, 1)
                // a second, y the top line's way to go.
                let y = self.shown.first().map_or(0.0, |l| l.y);
                let step = self.scroll_speed * y.clamp(0.5, 1.0) * dt;
                if y - step <= 0.0 {
                    for (k, l) in self.shown.iter_mut().enumerate() {
                        l.y = k as f32;
                    }
                    self.state = LineState::Idle;
                } else {
                    for l in &mut self.shown {
                        l.y -= step;
                    }
                }
            }
            LineState::Idle => {}
        }
        if self.state == LineState::Idle {
            let full = self.shown.len() >= self.num_lines;
            let due = self
                .shown
                .first()
                .is_some_and(|l| l.fader.state == 2 && l.held >= self.display_time - 1.0e-4);
            let wanted = !self.pending.is_empty() && (!self.stack_down || full);
            // Where a new line goes, when there is room for one.
            let place = if self.stack_down {
                (!full).then_some(self.shown.len() as f32)
            } else {
                let bottom = self.num_lines.saturating_sub(1) as f32;
                self.shown
                    .last()
                    .is_none_or(|l| l.y <= bottom - 1.0)
                    .then_some(bottom)
            };
            if due && (self.always_scroll || wanted) {
                self.shown[0].fader.hide();
                self.state = LineState::Expiring;
            } else if let Some(y) = place {
                if let Some((text, color)) = self.pending.pop_front() {
                    let mut fader = Fader {
                        fade_in: self.fade_in,
                        fade_out: self.fade_out,
                        ..Fader::default()
                    };
                    fader.show();
                    self.shown.push(Line {
                        text,
                        color,
                        y,
                        fader,
                        held: 0.0,
                    });
                    self.state = LineState::Admitting;
                }
            }
        }
        false
    }
}

/// A BarBitmap's flash (`re-target-hud.md` §5 [V][O], `FUN_0039f978` /
/// `FUN_0039fce8` / `FUN_0039f790`): a copy of the bar one z in front over
/// the span between the old and the new value, fading out over the
/// direction's time; on a fall its top edge sinks toward its bottom, to
/// `FlashyScale` frame pixels tall at the end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarFlash {
    /// The value before the change.
    pub from: f32,
    /// The value after it.
    pub to: f32,
    pub t: f32,
    pub time: f32,
}

impl BarFlash {
    /// 0 at the change, 1 when the copy is gone.
    pub fn progress(&self) -> f32 {
        (self.t / self.time).clamp(0.0, 1.0)
    }

    pub fn fall(&self) -> bool {
        self.to < self.from
    }
}

#[derive(Debug, Clone)]
pub struct Elem {
    pub name: String,
    pub kind: Kind,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Frame pixels (x, y); z is kept as written and not drawn.
    pub pos: Vec3,
    /// Euler degrees, applied X then Y then Z.
    pub rot: Vec3,
    pub scale: Vec3,
    pub z_order: u8,
    pub alpha: f32,
    pub propagate_alpha: bool,
    pub additive: bool,
    /// `Color`: what the drawn colour eases toward.
    pub color: [f32; 3],
    /// `ColorChange`: what the pulse snaps the drawn colour to.
    pub color_change: [f32; 3],
    pub color_change_rate: f32,
    pub color_pulse_rate: f32,
    pulse_timer: f32,
    /// The colour drawn this frame, bytes as floats.
    pub drawn: [f32; 3],
    pub fader: Fader,
    /// A window position (fractions) that replaces `pos` (`EventPosition`).
    pub window_pos: Option<[f32; 2]>,
    // Bitmap / bars.
    pub texture: Option<String>,
    /// u0, v0, u1, v1.
    pub tex_coords: [f32; 4],
    pub rect: Rect,
    pub shadow: bool,
    pub value: f32,
    pub scale_texture: bool,
    /// `FlashyScale`: a falling flash's last height (default 0.2,
    /// B2:397156–397161).
    pub flashy_scale: f32,
    /// The flash times by direction (`#2b76908f` rise, `#2deba703` fall;
    /// default 0 = no flash).
    pub flash_rise: f32,
    pub flash_fall: f32,
    /// The flash under way.
    pub flash: Option<BarFlash>,
    /// Whether the element was up at the end of the last [`Hud::advance`]
    /// (the PS2's "drawn last update", +0x370 bit 35): a value change while
    /// hidden flashes nothing.
    pub shown_last: bool,
    // BarSegmented.
    pub angle_start: f32,
    pub angle_end: f32,
    pub segments: u32,
    pub circular: bool,
    /// The `segment("Segment")` child, an element of its own (never drawn by
    /// itself).
    pub segment: Option<usize>,
    pub is_segment: bool,
    // Text.
    pub text: String,
    /// `Text("key")`: the text key the layout gives, localised by
    /// [`Hud::set_texts`].
    pub key: Option<String>,
    pub font: String,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub text_scale: [f32; 2],
    /// What an INFINITE number prints (`re-hud.md` §2.3, property
    /// 0x7a4e6e71): "--" when set, nothing when clear.
    pub infinite_dashes: bool,
    /// `numbertotime`: the number is seconds, printed as minutes and two-digit
    /// seconds ("%m:%02s", the domination countdown's "0:20"; issue #33).
    pub as_time: bool,
    // Model3D.
    pub mesh: Option<String>,
    pub mesh_info: Vec<MeshInfo>,
    /// MultiLineText: its lines.
    pub lines: Option<Box<MultiLine>>,
}

impl Elem {
    fn new(name: String, kind: Kind, parent: Option<usize>) -> Self {
        let is_text = kind == Kind::Text;
        Self {
            name,
            lines: (kind == Kind::MultiLineText).then(Box::default),
            kind,
            parent,
            children: Vec::new(),
            pos: Vec3::ZERO,
            rot: Vec3::ZERO,
            scale: Vec3::ONE,
            // BF2 FUN_005a30b0 / FUN_005a3178: render +0x48 byte = 0x80.
            // Unordered frames sit behind explicit foreground bars.
            z_order: 128,
            alpha: 1.0,
            propagate_alpha: false,
            additive: false,
            // [D] DAT_0076c200 (B2:395831–395838) taken as white.
            color: [255.0; 3],
            color_change: [255.0; 3],
            color_change_rate: 0.3,
            color_pulse_rate: f32::MAX,
            pulse_timer: f32::MAX,
            drawn: [255.0; 3],
            fader: Fader::default(),
            window_pos: None,
            texture: None,
            tex_coords: [0.0, 0.0, 1.0, 1.0],
            rect: Rect {
                w: 0.0,
                h: 0.0,
                h_align: HAlign::Left,
                v_align: VAlign::Top,
            },
            shadow: false,
            value: 1.0,
            scale_texture: true,
            flashy_scale: 0.2,
            flash_rise: 0.0,
            flash_fall: 0.0,
            flash: None,
            shown_last: false,
            angle_start: 0.0,
            angle_end: 0.0,
            segments: 0,
            circular: true,
            segment: None,
            is_segment: false,
            text: String::new(),
            key: None,
            // A Text without `TextFont` draws in the ctor's default,
            // `gamefont_tiny` (B2 D:405704 `FUN_005a90a8(.., DAT_0060c794)`,
            // ELF word 0x46f060d5 = the key hash of "gamefont_tiny" [V]).
            font: if is_text {
                DEFAULT_FONT.to_owned()
            } else {
                String::new()
            },
            h_align: HAlign::Left,
            v_align: VAlign::Top,
            text_scale: [1.0, 1.0],
            infinite_dashes: false,
            as_time: false,
            mesh: None,
            mesh_info: Vec::new(),
        }
    }

    /// The element's own matrix in its parent's frame-pixel space:
    /// translate · rotate (X, then Y, then Z) · scale.
    pub fn local(&self) -> Mat4 {
        trs(self.pos, self.rot, self.scale)
    }
}

/// translate · rotate (X, then Y, then Z; degrees) · scale.
pub fn trs(pos: Vec3, rot: Vec3, scale: Vec3) -> Mat4 {
    Mat4::from_translation(Vec3::new(pos.x, pos.y, 0.0))
        * Mat4::from_rotation_z(rot.z.to_radians())
        * Mat4::from_rotation_y(rot.y.to_radians())
        * Mat4::from_rotation_x(rot.x.to_radians())
        * Mat4::from_scale(scale)
}

/// A `hudtransforms` record: an input event rewritten into an output event
/// (`re-hud.md` §2.5 [V]).
#[derive(Debug, Clone)]
enum Transform {
    /// `NameMesh(in, out)` pairs: the output is the table's mesh, else the
    /// model named like the input, else no event.
    NameMesh {
        input: String,
        output: String,
        pairs: Vec<(String, String)>,
    },
    /// `NumberColor(v, r, g, b)` keys, [H] interpolated between keys.
    NumberColor {
        input: String,
        output: String,
        keys: Vec<(f32, [f32; 3])>,
    },
}

/// One viewport's HUD.
pub struct Hud {
    pub elems: Vec<Elem>,
    /// The top-level elements in file order.
    pub roots: Vec<usize>,
    listeners: HashMap<String, Vec<(usize, Listen)>>,
    transforms: Vec<Transform>,
    /// The last value sent per event: the PS2 publishes on change.
    sent: HashMap<String, EventValue>,
    /// What the Map element shows this frame (`crate::hud_map`); `None`
    /// draws no minimap.
    pub map: Option<crate::hud_map::MapModel>,
    /// The squad order cross this frame (`crate::hud_squad`); `None` draws
    /// none.
    pub squad: Option<crate::hud_squad::SquadCross>,
    /// The command-post capture icon this frame (`crate::hud_capture`);
    /// `None` draws none.
    pub capture: Option<crate::hud_capture::CaptureIcon>,
    /// The hit-direction chevrons' timers (`crate::hud_ambush`, card HATK):
    /// set by [`crate::hud::feed`], faded by [`Hud::advance`].
    pub ambush: crate::hud_ambush::Ambush,
    pub damage_flash: f32,
    /// The spawn screen up this frame (`crate::spawn_draw`, card SSELB);
    /// `None` draws none.
    pub spawn: Option<crate::spawn_draw::SpawnPage>,
    /// The sniper scope's overlay this frame (`crate::hud_scope`); `None`
    /// draws none.
    pub scope: Option<crate::hud_scope::ScopeView>,
    /// The shell's page up this frame and whether its movie has a frame
    /// (`crate::shell_ps2`, card BSHLA); it draws alone.
    pub shell: Option<(crate::shell_ps2::ShellMenus, bool)>,
    /// The Instant Action screen over the shell's page (`crate::shell_ia`).
    pub instant: Option<Box<crate::shell::InstantAction>>,
    /// The end of the round's stats page up this frame (`crate::stats_draw`,
    /// issue #50); it draws alone, over black.
    pub stats: Option<Box<crate::stats::StatsScreens>>,
    /// The round-end banner result; drawn alone over the scene.
    pub banner: Option<u8>,
    /// Per font: each glyph's pen advance in texels (what a MultiLineText
    /// wraps its words by). Empty until [`Hud::set_fonts`].
    advances: HashMap<String, HashMap<u32, f32>>,
    /// Seconds of HUD time, wrapped at [`CLOCK_WRAP`]: what a scrolling mesh
    /// part's UVs move by (`crate::hud_draw`).
    pub clock: f32,
}

/// `[ours]` [`Hud::clock`] wraps here so the f32 never grows coarse. A BF2
/// UV scroll is a whole number of 1/255 repeats per second (the MTRL's
/// bytes over 255, `mungefs::model::Segment::uv_scroll`), so every one is a
/// whole number of repeats at 255 s and the wrap is seamless.
pub const CLOCK_WRAP: f32 = 255.0;

fn num(n: &HudNode, i: usize) -> Option<f32> {
    match n.args.get(i) {
        Some(HudArg::Num(v)) => Some(*v),
        _ => None,
    }
}

fn text(n: &HudNode, i: usize) -> Option<&str> {
    match n.args.get(i) {
        Some(HudArg::Str(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn vec3(n: &HudNode) -> Vec3 {
    Vec3::new(
        num(n, 0).unwrap_or(0.0),
        num(n, 1).unwrap_or(0.0),
        num(n, 2).unwrap_or(0.0),
    )
}

fn rgb(n: &HudNode, first: usize) -> [f32; 3] {
    [0, 1, 2].map(|i| num(n, first + i).unwrap_or(0.0).clamp(0.0, 255.0))
}

/// A length pair in frame pixels: `mode` is the record's coordinate-mode
/// argument (`re-hud.md` §2.1 [V]: Pixels as written, the others a fraction
/// of the 512 × 448 frame — one player, one viewport, so Screen = Viewport).
fn to_pixels(x: f32, y: f32, mode: Option<&HudArg>) -> (f32, f32) {
    let pixels = match mode {
        Some(HudArg::Str(s)) => s.eq_ignore_ascii_case("pixels"),
        Some(HudArg::Num(v)) => *v == 0.0,
        None => true,
    };
    if pixels {
        (x, y)
    } else {
        (x * FRAME_W, y * FRAME_H)
    }
}

fn position(n: &HudNode) -> Vec3 {
    let v = vec3(n);
    let (x, y) = to_pixels(v.x, v.y, n.args.get(3));
    Vec3::new(x, y, v.z)
}

fn rect(n: &HudNode) -> Rect {
    let (w, h) = to_pixels(
        num(n, 0).unwrap_or(0.0),
        num(n, 1).unwrap_or(0.0),
        n.args.get(4),
    );
    Rect {
        w,
        h,
        h_align: h_align(text(n, 2)),
        v_align: v_align(text(n, 3)),
    }
}

fn h_align(s: Option<&str>) -> HAlign {
    match s.map(str::to_ascii_lowercase).as_deref() {
        Some("center") => HAlign::Center,
        Some("right") => HAlign::Right,
        _ => HAlign::Left,
    }
}

fn v_align(s: Option<&str>) -> VAlign {
    match s.map(str::to_ascii_lowercase).as_deref() {
        Some("center") => VAlign::Center,
        Some("bottom") => VAlign::Bottom,
        _ => VAlign::Top,
    }
}

/// The font a Text draws in without `TextFont`.
pub const DEFAULT_FONT: &str = "gamefont_tiny";

fn kind_of(name: &str) -> Option<Kind> {
    Some(match name {
        "group" => Kind::Group,
        "bitmap" => Kind::Bitmap,
        "barbitmap" => Kind::BarBitmap,
        "barsegmented" => Kind::BarSegmented,
        "text" => Kind::Text,
        "model3d" => Kind::Model3D,
        "multilinetext" => Kind::MultiLineText,
        "map" => Kind::Map,
        "target" | "objectivelist" | "borderedbox" | "#86e6ef58" => Kind::Other,
        _ => return None,
    })
}

impl Hud {
    /// Build the HUD from a player layout (`1playerhud`) and `hudtransforms`.
    pub fn new(layout: &HudLayout, transforms: &HudLayout) -> Self {
        let mut hud = Hud {
            elems: Vec::new(),
            roots: Vec::new(),
            listeners: HashMap::new(),
            transforms: Vec::new(),
            sent: HashMap::new(),
            map: None,
            squad: None,
            capture: None,
            ambush: Default::default(),
            damage_flash: 0.0,
            spawn: None,
            scope: None,
            shell: None,
            instant: None,
            stats: None,
            banner: None,
            advances: HashMap::new(),
            clock: 0.0,
        };
        for n in &layout.nodes {
            let lname = n.name.to_ascii_lowercase();
            if let Some(kind) = kind_of(&lname) {
                let i = hud.element(n, kind, None);
                hud.roots.push(i);
            }
        }
        for n in &transforms.nodes {
            for t in &n.children {
                hud.transform(t);
            }
        }
        hud
    }

    fn listen(&mut self, n: &HudNode, elem: usize, what: Listen) {
        if let Some(event) = text(n, 0) {
            self.listeners
                .entry(event.to_ascii_lowercase())
                .or_default()
                .push((elem, what));
        }
    }

    fn element(&mut self, node: &HudNode, kind: Kind, parent: Option<usize>) -> usize {
        let i = self.elems.len();
        let name = text(node, 0).unwrap_or_default().to_owned();
        self.elems.push(Elem::new(name, kind.clone(), parent));
        for c in &node.children {
            let lname = c.name.to_ascii_lowercase();
            // An element type with a body is a child element; without one it
            // is a property of the same name (`Bitmap("tex")`, `Text("key")`).
            if !c.children.is_empty() || lname == "group" {
                if lname == "segment" {
                    let s = self.element(c, Kind::Bitmap, Some(i));
                    self.elems[s].is_segment = true;
                    self.elems[i].segment = Some(s);
                    continue;
                }
                if lname == "meshinfo" {
                    let mut info = MeshInfo {
                        mesh: text(c, 0).unwrap_or_default().to_ascii_lowercase(),
                        pos: Vec3::ZERO,
                        rot: Vec3::ZERO,
                        scale: Vec3::ONE,
                    };
                    for p in &c.children {
                        match p.name.to_ascii_lowercase().as_str() {
                            "position" => info.pos = position(p),
                            "rotation" => info.rot = vec3(p),
                            "scale" => info.scale = vec3(p),
                            _ => {}
                        }
                    }
                    self.elems[i].mesh_info.push(info);
                    continue;
                }
                if lname == "format" && kind == Kind::MultiLineText {
                    self.line_format(c, i);
                    continue;
                }
                if let Some(k) = kind_of(&lname) {
                    // An uninterpreted type's children are its own records.
                    if kind != Kind::Other && kind != Kind::Map {
                        let ch = self.element(c, k, Some(i));
                        self.elems[i].children.push(ch);
                    }
                    continue;
                }
            }
            let e = &mut self.elems[i];
            if let Some(ml) = e.lines.as_deref_mut() {
                let on = num(c, 0).unwrap_or(0.0) != 0.0;
                match lname.as_str() {
                    "numlines" => ml.num_lines = num(c, 0).unwrap_or(1.0).max(1.0) as usize,
                    "displaytime" => ml.display_time = num(c, 0).unwrap_or(2.0),
                    "scrollspeed" => ml.scroll_speed = num(c, 0).unwrap_or(1.0),
                    "alwaysscroll" => ml.always_scroll = on,
                    "#09771c74" => ml.stack_down = on,
                    "#cf649993" => ml.hide_empty = on,
                    _ => {}
                }
            }
            match lname.as_str() {
                "position" => e.pos = position(c),
                "rotation" => e.rot = vec3(c),
                "scale" => e.scale = vec3(c),
                "zorder" => e.z_order = num(c, 0).unwrap_or(0.0).clamp(0.0, 255.0) as u8,
                "alpha" => e.alpha = num(c, 0).unwrap_or(1.0),
                "propagatealpha" => e.propagate_alpha = num(c, 0).unwrap_or(0.0) != 0.0,
                "blendmode" => {
                    e.additive = text(c, 0).is_some_and(|s| s.eq_ignore_ascii_case("additive"))
                }
                "color" => {
                    e.color = rgb(c, 0);
                    e.drawn = e.color;
                }
                "colorchange" => e.color_change = rgb(c, 0),
                "colorchangerate" => e.color_change_rate = num(c, 0).unwrap_or(0.3),
                "colorpulserate" => {
                    e.color_pulse_rate = num(c, 0).unwrap_or(f32::MAX);
                    e.pulse_timer = e.color_pulse_rate;
                }
                "fadeintime" => e.fader.fade_in = num(c, 0).unwrap_or(0.0),
                "fadeholdtime" => e.fader.hold = num(c, 0).unwrap_or(f32::MAX),
                "fadesustaintime" => e.fader.sustain = num(c, 0).unwrap_or(0.0),
                "fadeouttime" => e.fader.fade_out = num(c, 0).unwrap_or(0.0),
                "bitmap" => e.texture = text(c, 0).map(str::to_ascii_lowercase),
                "texcoords" => {
                    e.tex_coords = [0, 1, 2, 3].map(|k| num(c, k).unwrap_or(0.0));
                }
                "bitmaprect" | "rect" => e.rect = rect(c),
                "bitmapstyle" => {
                    e.shadow = text(c, 0).is_some_and(|s| s.eq_ignore_ascii_case("shadow"))
                }
                "value" => e.value = num(c, 0).unwrap_or(1.0),
                "scaletexture" => e.scale_texture = num(c, 0).unwrap_or(1.0) != 0.0,
                "flashyscale" => e.flashy_scale = num(c, 0).unwrap_or(0.2),
                "#2b76908f" => e.flash_rise = num(c, 0).unwrap_or(0.0),
                "#2deba703" => e.flash_fall = num(c, 0).unwrap_or(0.0),
                "anglestart" => e.angle_start = num(c, 0).unwrap_or(0.0),
                "angleend" => e.angle_end = num(c, 0).unwrap_or(0.0),
                "numsegments" => e.segments = num(c, 0).unwrap_or(0.0).max(0.0) as u32,
                "circular" => e.circular = num(c, 0).unwrap_or(1.0) != 0.0,
                "text" => e.key = text(c, 0).map(str::to_owned),
                "textfont" => e.font = text(c, 0).unwrap_or_default().to_ascii_lowercase(),
                "textalignment" => {
                    e.h_align = h_align(text(c, 0));
                    e.v_align = v_align(text(c, 1));
                }
                "textscale" => {
                    e.text_scale = [num(c, 0).unwrap_or(1.0), num(c, 1).unwrap_or(1.0)];
                }
                "#7a4e6e71" => e.infinite_dashes = num(c, 0).unwrap_or(0.0) != 0.0,
                "numbertotime" => e.as_time = true,
                "mesh" => e.mesh = text(c, 0).map(str::to_ascii_lowercase),
                "eventenable" => self.listen(c, i, Listen::Enable),
                "eventdisable" => self.listen(c, i, Listen::Disable),
                "eventcolor" => self.listen(c, i, Listen::Color),
                "eventpulserate" => self.listen(c, i, Listen::PulseRate),
                "eventvalue" => self.listen(c, i, Listen::Value),
                "eventnumber" => self.listen(c, i, Listen::Number),
                "eventtext" => self.listen(c, i, Listen::Text),
                "eventmesh" => self.listen(c, i, Listen::Mesh),
                "eventbitmap" => self.listen(c, i, Listen::Bitmap),
                "eventposition" => self.listen(c, i, Listen::Position),
                _ => {}
            }
        }
        i
    }

    /// A MultiLineText's `Format` block: how each of its lines is set.
    fn line_format(&mut self, node: &HudNode, i: usize) {
        let e = &mut self.elems[i];
        for p in &node.children {
            match p.name.to_ascii_lowercase().as_str() {
                "textfont" => e.font = text(p, 0).unwrap_or_default().to_ascii_lowercase(),
                "textalignment" => {
                    e.h_align = h_align(text(p, 0));
                    e.v_align = v_align(text(p, 1));
                }
                "textscale" => {
                    e.text_scale = [num(p, 0).unwrap_or(1.0), num(p, 1).unwrap_or(1.0)];
                }
                _ => {}
            }
            let Some(ml) = e.lines.as_deref_mut() else {
                continue;
            };
            match p.name.to_ascii_lowercase().as_str() {
                "textstyle" => {
                    ml.shadow = text(p, 0).is_some_and(|s| s.eq_ignore_ascii_case("shadow"))
                }
                "textbreak" => {
                    ml.word_break = text(p, 0).is_some_and(|s| s.eq_ignore_ascii_case("word"))
                }
                "color" => ml.color = rgb(p, 0),
                "fadeintime" => ml.fade_in = num(p, 0).unwrap_or(0.0),
                "fadeouttime" => ml.fade_out = num(p, 0).unwrap_or(0.0),
                _ => {}
            }
        }
    }

    /// Localise every `Text("key")` by `look`; a key it lacks prints raw,
    /// as the PS2's `FUN_005ac980` does (`re-hud.md` §2.3).
    pub fn set_texts(&mut self, look: &dyn Fn(&str) -> Option<String>) {
        for e in &mut self.elems {
            if let Some(key) = &e.key {
                e.text = look(key).unwrap_or_else(|| key.clone());
            }
        }
    }

    /// Give the HUD its fonts' glyph advances: a MultiLineText breaks a
    /// message at the words that no longer fit its box.
    pub fn set_fonts(&mut self, fonts: &HashMap<String, HudFont>) {
        self.advances = fonts
            .iter()
            .map(|(name, f)| {
                let glyphs = f
                    .glyphs
                    .iter()
                    .map(|g| (u32::from(g.code), f32::from(g.advance)))
                    .collect();
                (name.to_ascii_lowercase(), glyphs)
            })
            .collect();
    }

    /// `message` as the lines of element `i`'s box: broken at spaces where
    /// the next word would cross the box's width. One line when the box has
    /// no width, no word break or no font to measure with.
    fn wrap(&self, i: usize, message: &str) -> Vec<String> {
        let e = &self.elems[i];
        let width = e.rect.w;
        let advances = self.advances.get(&e.font);
        let word_break = e.lines.as_deref().is_some_and(|m| m.word_break);
        let (Some(adv), true, true) = (advances, word_break, width > 0.0) else {
            return vec![message.to_owned()];
        };
        let spacing = if e.lines.as_deref().is_some_and(|m| m.shadow) {
            SHADOW_SPACING
        } else {
            0.0
        };
        let measure = |t: &str| -> f32 {
            t.chars()
                .filter_map(|c| adv.get(&(c as u32)))
                .map(|a| a + spacing)
                .sum::<f32>()
                * e.text_scale[0]
        };
        let mut out: Vec<String> = Vec::new();
        let mut line = String::new();
        for word in message.split(' ') {
            let with = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure(&with) > width {
                out.push(std::mem::take(&mut line));
                line = word.to_owned();
            } else {
                line = with;
            }
        }
        out.push(line);
        out
    }

    fn transform(&mut self, node: &HudNode) {
        let (mut input, mut output) = (String::new(), String::new());
        let mut pairs = Vec::new();
        let mut keys = Vec::new();
        for c in &node.children {
            match c.name.to_ascii_lowercase().as_str() {
                "eventinput" => input = text(c, 0).unwrap_or_default().to_ascii_lowercase(),
                "eventoutput" => output = text(c, 0).unwrap_or_default().to_ascii_lowercase(),
                "namemesh" => pairs.push((
                    text(c, 0).unwrap_or_default().to_ascii_lowercase(),
                    text(c, 1).unwrap_or_default().to_ascii_lowercase(),
                )),
                "numbercolor" => keys.push((num(c, 0).unwrap_or(0.0), rgb(c, 1))),
                _ => {}
            }
        }
        match node.name.to_ascii_lowercase().as_str() {
            "transformnamemesh" => self.transforms.push(Transform::NameMesh {
                input,
                output,
                pairs,
            }),
            "transformnumbercolor" => self.transforms.push(Transform::NumberColor {
                input,
                output,
                keys,
            }),
            _ => {}
        }
    }

    /// Find an element by name (case-blind).
    pub fn find(&self, name: &str) -> Option<usize> {
        self.elems
            .iter()
            .position(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// Publish `event` only when its value changed since the last send — the
    /// PS2's publishers are change-gated (`re-hud.md` §3 [V]).
    pub fn publish(&mut self, event: &str, value: EventValue, has_mesh: &dyn Fn(&str) -> bool) {
        if self.sent.get(event) == Some(&value) {
            return;
        }
        self.sent.insert(event.to_owned(), value.clone());
        self.send(event, value, has_mesh);
    }

    /// The value `event` was last published with ([`Hud::publish`]).
    pub fn last(&self, event: &str) -> Option<&EventValue> {
        self.sent.get(event)
    }

    /// True when `gate` is off now and was not off at the last call — the
    /// edge a one-shot disable event is sent on.
    pub fn falls(&mut self, gate: &str, on: bool) -> bool {
        let v = EventValue::Number(f32::from(u8::from(on)));
        let changed = self.sent.get(gate) != Some(&v);
        self.sent.insert(gate.to_owned(), v);
        changed && !on
    }

    /// Forget what was published, so the next [`Hud::publish`] sends again.
    pub fn forget(&mut self, event: &str) {
        self.sent.remove(event);
    }

    /// The mesh a weapon-change `event` carrying `key` ends up showing, or
    /// `None` when no NameMesh transform finds a model for it.
    pub fn name_mesh(
        &self,
        event: &str,
        key: &str,
        has_mesh: &dyn Fn(&str) -> bool,
    ) -> Option<String> {
        let event = event.to_ascii_lowercase();
        self.transforms.iter().find_map(|t| match t {
            Transform::NameMesh { input, pairs, .. } if *input == event => {
                resolve_mesh(pairs, key, has_mesh)
            }
            _ => None,
        })
    }

    /// Send `event` to every listener, then through the transforms.
    /// `has_mesh` says whether a model of that name can be drawn (the PS2's
    /// model-table lookup in the NameMesh fallback).
    pub fn send(&mut self, event: &str, value: EventValue, has_mesh: &dyn Fn(&str) -> bool) {
        let event = event.to_ascii_lowercase();
        if let Some(list) = self.listeners.get(&event) {
            for &(i, what) in list {
                // A message box takes a text as a new message and a colour
                // as that of the messages to come.
                if self.elems[i].lines.is_some() {
                    match (what, &value) {
                        (Listen::Text, EventValue::Text(s)) => {
                            let lines = self.wrap(i, s);
                            if let Some(ml) = self.elems[i].lines.as_deref_mut() {
                                ml.add(lines);
                            }
                            continue;
                        }
                        (Listen::Color, EventValue::Color(c)) => {
                            if let Some(ml) = self.elems[i].lines.as_deref_mut() {
                                ml.color = c.map(f32::from);
                            }
                            continue;
                        }
                        _ => {}
                    }
                }
                let e = &mut self.elems[i];
                match (what, &value) {
                    (Listen::Enable, _) => e.fader.show(),
                    (Listen::Disable, _) => e.fader.hide(),
                    (Listen::Color, EventValue::Color(c)) => e.color = c.map(f32::from),
                    (Listen::PulseRate, EventValue::Number(v)) => {
                        // [D] the handler stores the rate (+0xbc); the timer
                        // restarts from it at its next expiry — restarted
                        // here when the new rate is the shorter wait.
                        e.color_pulse_rate = *v;
                        e.pulse_timer = e.pulse_timer.min(*v);
                    }
                    (Listen::Value, EventValue::Number(v)) => {
                        let v = v.clamp(0.0, 1.0);
                        let time = if v < e.value {
                            e.flash_fall
                        } else {
                            e.flash_rise
                        };
                        if e.kind == Kind::BarBitmap
                            && (v - e.value).abs() > 1.0e-4
                            && e.shown_last
                            && time > 1.0e-4
                        {
                            e.flash = Some(BarFlash {
                                from: e.value,
                                to: v,
                                t: 0.0,
                                time,
                            });
                        }
                        e.value = v;
                    }
                    (Listen::Number, EventValue::Number(v)) => {
                        e.text = if is_infinite(*v) {
                            let dashes = if e.infinite_dashes { "--" } else { "" };
                            dashes.to_owned()
                        } else if e.as_time {
                            let s = v.max(0.0) as i64;
                            format!("{}:{:02}", s / 60, s % 60)
                        } else {
                            format!("{}", *v as i64)
                        }
                    }
                    (Listen::Text, EventValue::Text(s)) => e.text = s.clone(),
                    (Listen::Mesh, EventValue::Text(s)) => e.mesh = Some(s.to_ascii_lowercase()),
                    (Listen::Bitmap, EventValue::Text(s)) => {
                        e.texture = Some(s.to_ascii_lowercase())
                    }
                    (Listen::Position, EventValue::Position(p)) => e.window_pos = Some(*p),
                    _ => {}
                }
            }
        }
        let mut out: Vec<(String, EventValue)> = Vec::new();
        for t in &self.transforms {
            match (t, &value) {
                (
                    Transform::NameMesh {
                        input,
                        output,
                        pairs,
                    },
                    EventValue::Text(s),
                ) if *input == event => {
                    if let Some(m) = resolve_mesh(pairs, s, has_mesh) {
                        out.push((output.clone(), EventValue::Text(m)));
                    }
                }
                (
                    Transform::NumberColor {
                        input,
                        output,
                        keys,
                    },
                    EventValue::Number(v),
                ) if *input == event => {
                    if let Some(c) = number_color(keys, *v) {
                        out.push((output.clone(), EventValue::Color(c)));
                    }
                }
                _ => {}
            }
        }
        for (e, v) in out {
            self.send(&e, v, has_mesh);
        }
    }

    /// One frame of fades, colour easing and pulses (`re-hud.md` §2.1 [V]:
    /// `FUN_0039da30`).
    pub fn advance(&mut self, dt: f32) {
        self.clock = (self.clock + dt).rem_euclid(CLOCK_WRAP);
        self.ambush.advance(dt);
        for e in &mut self.elems {
            e.fader.advance(dt);
            if let Some(ml) = e.lines.as_deref_mut() {
                if ml.advance(dt) {
                    e.fader.hide();
                }
            }
            if let Some(f) = &mut e.flash {
                f.t += dt;
                if f.t >= f.time {
                    e.flash = None;
                }
            }
            e.shown_last = e.fader.state != 0;
            if e.fader.state == 0 {
                continue;
            }
            let k = if e.color_change_rate == 0.0 {
                1.0
            } else {
                (dt / e.color_change_rate).clamp(0.0, 1.0)
            };
            for c in 0..3 {
                e.drawn[c] = e.drawn[c] * (1.0 - k) + e.color[c] * k;
            }
            if e.color_pulse_rate < f32::MAX {
                e.pulse_timer -= dt;
                if e.pulse_timer <= 0.0 {
                    e.drawn = e.color_change;
                    e.pulse_timer = e.color_pulse_rate.max(dt);
                }
            }
        }
    }

    /// What element `i`'s ancestors multiply its drawn colour by, 0..1 per
    /// channel: the product of their drawn colours over 255
    /// (`re-target-hud.md` §6 [M][O]: the target bar measured at teamColor ×
    /// (192, 224, 224) and the health bar at its fill × (255, 231, 255), each
    /// within 2 levels). [D] every ancestor, not only the parent — the data
    /// colours no nested group.
    pub fn tint(&self, i: usize) -> [f32; 3] {
        let mut t = [1.0; 3];
        let mut p = self.elems[i].parent;
        while let Some(k) = p {
            let d = self.elems[k].drawn;
            for c in 0..3 {
                t[c] *= (d[c] / 255.0).clamp(0.0, 1.0);
            }
            p = self.elems[k].parent;
        }
        t
    }

    /// Element `i`'s drawn colour with its ancestors' tint ([`Hud::tint`]).
    pub fn drawn(&self, i: usize) -> [f32; 3] {
        let t = self.tint(i);
        let d = self.elems[i].drawn;
        [0, 1, 2].map(|c| d[c] * t[c])
    }

    /// The alpha an element is drawn at: its fade × `Alpha`, times its
    /// ancestors' where they `PropagateAlpha`; an ancestor that is hidden
    /// hides it either way. [H] — the group draw is not in the corpus.
    pub fn alpha(&self, i: usize) -> f32 {
        let e = &self.elems[i];
        let own = e.fader.alpha() * e.alpha;
        match e.parent {
            None => own,
            Some(p) => {
                let pa = self.alpha(p);
                if self.elems[p].propagate_alpha {
                    own * pa
                } else if pa > 0.0 {
                    own
                } else {
                    0.0
                }
            }
        }
    }
}

/// The numbers a Text element takes as "infinite" (`FUN_003ae6c0`
/// B2:406176–406193: an int event equal to -1 or 0x7fffffff): it prints the
/// element's infinite string instead of the number.
pub const INFINITE: f32 = -1.0;

fn is_infinite(v: f32) -> bool {
    v == INFINITE || v >= i32::MAX as f32
}

/// `FUN_003bafa8` (`re-hud.md` §2.5 [V]): the table's mesh for `key`, else the
/// model named `key` itself — also when the table's mesh is not loaded.
fn resolve_mesh(
    pairs: &[(String, String)],
    key: &str,
    has_mesh: &dyn Fn(&str) -> bool,
) -> Option<String> {
    let key = key.to_ascii_lowercase();
    pairs
        .iter()
        .find(|(k, m)| *k == key && has_mesh(m))
        .map(|(_, m)| m.clone())
        .or_else(|| has_mesh(&key).then_some(key))
}

/// A `TransformNumberColor` lookup: the keys' colours, interpolated.
fn number_color(keys: &[(f32, [f32; 3])], v: f32) -> Option<[u8; 3]> {
    let (first, last) = (keys.first()?, keys.last()?);
    let c = if v <= first.0 {
        first.1
    } else if v >= last.0 {
        last.1
    } else {
        let hi = keys.iter().position(|k| k.0 >= v)?;
        let (a, b) = (keys[hi - 1], keys[hi]);
        let t = if b.0 > a.0 {
            (v - a.0) / (b.0 - a.0)
        } else {
            1.0
        };
        [0, 1, 2].map(|i| a.1[i] + (b.1[i] - a.1[i]) * t)
    };
    Some(c.map(|x| x.round().clamp(0.0, 255.0) as u8))
}

/// The 1-player layout's target readout as the disc has it
/// (`re-target-hud.md` §6, the `1playerhud` dump lines 863–946): the bar
/// group, its backing and fill, and the name group's class name.
#[cfg(test)]
pub(crate) mod fixture {
    use super::Hud;
    use manifest::hud::{HudArg, HudLayout, HudNode};

    fn node(name: &str, args: &[HudArg], children: Vec<HudNode>) -> HudNode {
        HudNode {
            name: name.into(),
            args: args.to_vec(),
            children,
        }
    }

    fn p(name: &str, args: &[HudArg]) -> HudNode {
        node(name, args, vec![])
    }

    fn s(v: &str) -> HudArg {
        HudArg::Str(v.into())
    }

    fn n(v: f32) -> HudArg {
        HudArg::Num(v)
    }

    pub(crate) fn target_hud() -> Hud {
        let vp = || s("Viewport");
        let ev = |k: &str| s(&format!("player1.weapon1.target.{k}"));
        let bar = |name: &str, own: Vec<HudNode>| {
            let mut c = vec![
                p("flashyscale", &[n(1.0)]),
                p("Bitmap", &[s("hud_jetpack_and_energybar_fill")]),
                p("texcoords", &[n(0.0), n(0.5), n(1.0), n(1.0)]),
                p("Viewport", &[n(1.0)]),
                p("colorchange", &[n(0.0), n(0.0), n(0.0)]),
                p("fadeouttime", &[n(1.0)]),
                p("eventenable", &[ev("healthFraction")]),
                p("eventdisable", &[ev("disable")]),
            ];
            c.extend(own);
            node("barbitmap", &[s(name)], c)
        };
        let layout = HudLayout {
            name: "1playerhud".into(),
            nodes: vec![
                node(
                    "Group",
                    &[s("player1targethealthbar")],
                    vec![
                        p("propagatealpha", &[n(1.0)]),
                        p("Viewport", &[n(1.0)]),
                        p("Position", &[n(0.5), n(0.059272), n(0.0), vp()]),
                        p("Scale", &[n(0.263237), n(0.23), n(0.23)]),
                        p("alpha", &[n(0.8)]),
                        p("color", &[n(192.0), n(224.0), n(224.0)]),
                        p("eventenable", &[s("initialize")]),
                        bar(
                            "player1targethealth_blankfill",
                            vec![
                                p("value", &[n(1.0)]),
                                p(
                                    "bitmaprect",
                                    &[n(1.18923), n(0.181946), s("Center"), s("Center"), vp()],
                                ),
                                p("Position", &[n(0.0), n(0.253896), n(0.0), vp()]),
                                p("Scale", &[n(0.924622), n(0.498552), n(0.540316)]),
                                p("zorder", &[n(103.0)]),
                                p("color", &[n(39.0), n(40.0), n(46.0)]),
                                p("colorchangerate", &[n(0.0)]),
                                p("fadeintime", &[n(0.1)]),
                            ],
                        ),
                        bar(
                            "player1targethealth_fill",
                            vec![
                                p("#2b76908f", &[n(1.0)]),
                                p("#2deba703", &[n(1.0)]),
                                p("eventvalue", &[ev("healthFraction")]),
                                p(
                                    "bitmaprect",
                                    &[n(1.18923), n(0.181946), s("Center"), s("Top"), vp()],
                                ),
                                p("Position", &[n(0.0), n(0.227078), n(0.0), vp()]),
                                p("Scale", &[n(0.890714), n(0.222995), n(0.9)]),
                                p("zorder", &[n(1.0)]),
                                p("colorchangerate", &[n(0.01)]),
                                p("eventcolor", &[ev("teamColor")]),
                            ],
                        ),
                    ],
                ),
                node(
                    "Group",
                    &[s("player1targetnamegroup")],
                    vec![
                        p("Viewport", &[n(1.0)]),
                        p("Position", &[n(0.5), n(0.012799), n(0.0), vp()]),
                        p("eventenable", &[s("initialize")]),
                        node(
                            "Text",
                            &[s("player1targetinfo_classNameNEW")],
                            vec![
                                p("textalignment", &[s("Center"), s("Top")]),
                                p("textfont", &[s("gamefont_tiny")]),
                                p("eventtext", &[ev("className")]),
                                p("Position", &[n(0.0), n(0.065248), n(0.0), vp()]),
                                p("alpha", &[n(0.6)]),
                                p("fadeouttime", &[n(1.0)]),
                                p("eventenable", &[ev("className")]),
                                p("eventdisable", &[ev("disable")]),
                            ],
                        ),
                        node(
                            "Text",
                            &[s("player1targetinfo_shieldName")],
                            vec![
                                p("eventtext", &[ev("shieldName")]),
                                p("color", &[n(40.0), n(255.0), n(255.0)]),
                                p("fadeouttime", &[n(1.0)]),
                                p("eventenable", &[ev("shieldName")]),
                                p("eventdisable", &[ev("disableShieldName")]),
                            ],
                        ),
                    ],
                ),
            ],
        };
        let none = HudLayout {
            name: "none".into(),
            nodes: vec![],
        };
        let mut hud = Hud::new(&layout, &none);
        hud.send("initialize", super::EventValue::None, &|_| false);
        hud.advance(0.0);
        hud
    }

    /// The 1-player reticle group's hit and critical-hit flashes as the
    /// disc has them (`common_bf2/hud/1playerhud.ron`, `player1reticule`;
    /// `re-target-hud.md` §12).
    pub(crate) fn reticle_hud() -> Hud {
        let ev = |k: &str| s(&format!("player1.weapon1.target.{k}"));
        let flash = |name: &str, mesh: &str, own: Vec<HudNode>| {
            let mut c = vec![
                p("mesh", &[s(mesh)]),
                p("Scale", &[n(0.26), n(0.26), n(0.26)]),
                p("color", &[n(1.0), n(86.0), n(213.0)]),
                p("colorchangerate", &[n(0.01)]),
                p("eventcolor", &[ev("hitColor")]),
            ];
            c.extend(own);
            node("model3d", &[s(name)], c)
        };
        let layout = HudLayout {
            name: "1playerhud".into(),
            nodes: vec![node(
                "Group",
                &[s("player1reticule")],
                vec![
                    p("eventposition", &[s("player1.weapon1.reticule.position")]),
                    p("Position", &[n(0.5), n(0.410624), n(0.0), s("Viewport")]),
                    p("alpha", &[n(0.65)]),
                    p("eventenable", &[s("player1.weapon1.reticule.position")]),
                    p("eventdisable", &[s("player1.weapon1.reticule.disable")]),
                    flash(
                        "player1reticule_hit",
                        "hud_main_reticule_hit",
                        vec![
                            p("alpha", &[n(0.5)]),
                            p("fadeholdtime", &[n(0.33)]),
                            p("eventenable", &[ev("hit")]),
                        ],
                    ),
                    flash(
                        "player1reticule_criticalhit",
                        "hud_main_reticule_criticalhit",
                        vec![
                            p("alpha", &[n(0.6)]),
                            p("fadeholdtime", &[n(0.6)]),
                            p("fadesustaintime", &[n(0.5)]),
                            p("fadeouttime", &[n(0.01)]),
                            p("eventenable", &[ev("hitCritical")]),
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
        hud.send("initialize", super::EventValue::None, &|_| false);
        hud.advance(0.0);
        hud
    }
}

#[cfg(test)]
mod tests;
