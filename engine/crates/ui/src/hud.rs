//! The HUD's model and its two halves (card BHUD2): [`feed`] publishes the
//! model as the events BF2's own HUD tree listens to (`crate::hud_tree`,
//! drawn by `crate::hud_draw` and the renderer's HUD pass) — the bars and
//! counts, the event lines of `player1systemMessages` and the statistics
//! pop-up (card EVMSG, `analysis/re-event-messages.md`), the target's name
//! and health (card HTGT, `analysis/re-target-hud.md`) — and [`draw_hud`]
//! paints in egui what that tree does not carry yet: the
//! round timer, the pad warning.

use crate::hud_tree::{EventValue, Hud, INFINITE};
use egui::{Align2, Color32, FontId, Frame, RichText, Vec2};

/// The held weapon as the HUD shows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HudWeapon {
    /// What the PS2's `.change` event carries (`re-hud.md` §3 [V][O]): the
    /// class's `HUDTag`, else its own `GeometryName`, else its name. The
    /// `hudtransforms` table turns it into a mesh.
    pub key: String,
    /// The weapon class's own name. `[ours]` a BF1 class keys BF2's table by
    /// it when `key` (its own `GeometryName`) has no entry there: BF2's
    /// classes of the same name carry no geometry and send their name.
    pub class: String,
    /// The class is a BF2 disc's: the table is read with `key`, else the
    /// model named `key` is shown, as the PS2 does (`re-hud.md` §2.5); only
    /// a key naming no model the bake has falls back to `class` (`[ours]`:
    /// `hud_rep_defense_buff` lives in the side LVL, not in the bake).
    pub bf2: bool,
    /// `[ours]` a weapon with no BF2 mesh: its `IconTexture` sheet, drawn in
    /// the mesh's place.
    pub icon: Option<String>,
    /// The localised name (shown for a moment on a change).
    pub name: String,
    /// Clip + spare rounds as ONE number (`re-hud.md` §3 [V][O]); `None` =
    /// infinite.
    pub total: Option<u32>,
    /// The clip's fill, 0..1 (the ring around the reticle).
    pub clip_fraction: f32,
}

/// The heat ring and its overheat signs around the reticle
/// (`analysis/re-heat-hud.md` 1, 2). The model carries them for the first row's
/// weapon, as BF2's layout has the ring for weapon 1 alone.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HudGauge {
    /// What the ring shows, 0..1: an unlimited weapon's heat, a counted weapon's
    /// charge while the trigger is held, else 0.
    pub ring: f32,
    /// The two red bits under the reticle while overheated, 0..1, shrinking to
    /// nothing at the mark where the weapon clears.
    pub refire: f32,
    /// Either row's weapon is locked: the "OVERHEATED!" line.
    pub overheated: bool,
}

/// The element of the "Leaving Battlefield" line ([`add_leaving_line`]).
pub const LEAVING_ELEM: &str = "player1leavingbattlefield";

/// Add the "Leaving Battlefield" line to BF2's layout: the PS2 draws it in code, in
/// `gamefont_large` at the screen's centre (`re-boundary.md` T4, T5). It stays off
/// until [`feed`] sends a line, and [`LeavingPulse`] sets its alpha.
pub fn add_leaving_line(layout: &mut manifest::hud::HudLayout) {
    use manifest::hud::{HudArg, HudNode};
    fn node(name: &str, args: Vec<HudArg>) -> HudNode {
        HudNode {
            name: name.to_owned(),
            args,
            children: Vec::new(),
        }
    }
    let (s, n) = (|v: &str| HudArg::Str(v.to_owned()), HudArg::Num);
    layout.nodes.push(HudNode {
        name: "Text".to_owned(),
        args: vec![s(LEAVING_ELEM)],
        children: vec![
            node("textstyle", vec![s("Shadow")]),
            node("textalignment", vec![s("Center"), s("Center")]),
            node("textfont", vec![s("gamefont_large")]),
            node("Position", vec![n(0.5), n(0.5), n(0.0), s("Viewport")]),
            node("zorder", vec![n(5.0)]),
            node("eventtext", vec![s("player1.leaving")]),
            node("eventenable", vec![s("player1.leaving")]),
            node("eventdisable", vec![s("player1.leavingOff")]),
        ],
    });
}

/// The line's alpha, an undamped spring round 128 that the PS2 steps each frame from
/// 254 at rest when it first shows, clamped to 2..=255 (`re-boundary.md` T4: B1
/// `FUN_002d89c0`, B2 `FUN_00391fe0`). Stepped at the PS2's 30 updates a second, so
/// its 0.28 s pulse does not change with the frame rate.
#[derive(Debug, Clone, Copy, Default)]
pub struct LeavingPulse {
    state: Option<(f32, f32)>,
    time: f32,
}

impl LeavingPulse {
    const STEP: f32 = 1.0 / 30.0;

    /// One frame of `dt` seconds: the alpha 0..1 while `on`, else `None`.
    pub fn step(&mut self, on: bool, dt: f32) -> Option<f32> {
        if !on {
            *self = Self::default();
            return None;
        }
        let (alpha, rate) = self.state.get_or_insert((254.0, 0.0));
        self.time += dt;
        while self.time >= Self::STEP {
            self.time -= Self::STEP;
            *alpha += Self::STEP * *rate;
            *rate += Self::STEP * (128.0 - *alpha) * 500.0;
        }
        Some(alpha.clamp(2.0, 255.0) / 255.0)
    }
}

/// One event line for the message box (`player1.message` after its
/// `player1.message.color`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HudMessage {
    pub text: String,
    pub color: [u8; 3],
}

/// The message colours (`re-event-messages.md` §1.2, §2: PS2 RAM [M]).
pub mod message_color {
    /// The viewer's own team.
    pub const OWN: [u8; 3] = [1, 86, 213];
    /// The other team.
    pub const ENEMY: [u8; 3] = [200, 100, 100];
    /// Nobody's (team index 0: a death with no killer).
    pub const NEUTRAL: [u8; 3] = [192, 192, 192];
    /// Bonuses and the mission's `ShowMessageText` (`DAT_0076c250`).
    pub const YELLOW: [u8; 3] = [223, 223, 32];
}

/// The player's statistics as BF2 publishes them (`re-event-messages.md` §5
/// [V] `FUN_003b5de8`): five numbers, of which the 1-player layout shows the
/// points and the kills; a change in any of the five opens the pop-up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HudStats {
    /// Ordinary and hero points together.
    pub points: i32,
    /// Kills less two per team kill.
    pub kills: i32,
    pub deaths: i32,
    pub hero_points: i32,
    pub captures: i32,
}

/// What the reticle picks, as BF2's target readout shows it
/// (`re-target-hud.md` §3 [V][O]): the class's name over a bar of its health
/// (or its shield while that holds), in the relation's colour.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HudTarget {
    /// Which object: a change of id is a new target (disable, then the new
    /// name and value, §3).
    pub id: u64,
    /// The class's localised name (`entity.<side>.<rest>`, §4).
    pub name: String,
    /// Health / max health, 0..1.
    pub health_fraction: f32,
    /// The viewer's relation to the target's team: > 0 friend, < 0 enemy,
    /// 0 neutral.
    pub relation: i8,
    /// The target's team is 0 (props, empty vehicles, unowned turrets):
    /// white whatever the relation.
    pub team0: bool,
    /// While a shield holds: its localised label ("Shield") and its
    /// fraction, shown in the bar's place.
    pub shield: Option<(String, f32)>,
}

/// BF2's `target.teamColor` values (`re-target-hud.md` §3 [V][V-RAM][O]:
/// `FUN_0036fec8`'s four words, read in RAM as 0156d5ff, c86464ff, ffffffff,
/// dfdf20ff).
pub mod target_color {
    pub const FRIEND: [u8; 3] = [1, 86, 213];
    pub const ENEMY: [u8; 3] = [200, 100, 100];
    /// A team-0 target, and the colour sent when the target is lost.
    pub const NONE: [u8; 3] = [255, 255, 255];
    /// A non-zero team neutral to the viewer.
    pub const NEUTRAL: [u8; 3] = [223, 223, 32];
}

impl HudTarget {
    /// The bar's and the reticle's colour (`FUN_003b3398` B2:410224–410253).
    pub fn color(&self) -> [u8; 3] {
        match self.relation {
            _ if self.team0 => target_color::NONE,
            r if r > 0 => target_color::FRIEND,
            r if r < 0 => target_color::ENEMY,
            _ => target_color::NEUTRAL,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HudModel {
    /// Faction keys or display names, team 0 / team 1.
    pub team_names: [String; 2],
    pub tickets: [u32; 2],
    /// Reinforcements lost per second to the post bleed, team 0 / team 1.
    pub bleed: [f32; 2],
    pub hp: f32,
    pub hp_max: f32,
    /// The hero's saber-shaped health bar replaces ordinary health.
    pub hero: bool,
    /// Localised offer message and seconds remaining.
    pub hero_offer: Option<(String, f32)>,
    /// Sprint stamina 0..energy_max.
    pub energy: f32,
    pub energy_max: f32,
    /// Rechargeable jet fuel and its activation border; None disables the bar.
    pub jet: Option<(f32, f32)>,
    /// The weapon in hand; `None` hides the weapon group (dead, or riding).
    pub weapon: Option<HudWeapon>,
    /// The secondary channel's current weapon — the grenade row (card GREN);
    /// `None` = the row is off.
    pub weapon2: Option<HudWeapon>,
    pub gauge: HudGauge,
    /// The flown droid is outside the map's boundary and counting down: the line
    /// the HUD shows at the screen's centre (`re-boundary.md` 5).
    pub leaving: Option<String>,
    /// The reticle: `None` hides it; else the relation of what the crosshair
    /// picks (−1 enemy, 1 friend, 0 nothing) — `Sim::reticle_pick`. A
    /// [`HudModel::target`] colours it instead.
    pub reticle: Option<i8>,
    /// The critical-hit flash (hitmark): `Some(colour)` while the PS2's
    /// 0.5 s hit timer of the player's last sniper-rifle headshot runs - the
    /// victim's relation colour, sent as `target.hitColor` with
    /// `target.hitCritical` every frame (`analysis/re-target-hud.md` §12).
    /// The layout's `player1reticule_criticalhit` holds it 0.6 s and 0.5 s
    /// more after the last send.
    pub hit_critical: Option<[u8; 3]>,
    /// The plain hit flash ([ours], the maintainer 2026-10-29): `Some(colour)`
    /// while the 0.5 s hit timer of the player's last body hit on an enemy
    /// runs, any weapon, sent as `target.hitColor` with `target.hit`; the
    /// layout's `player1reticule_hit` holds it 0.33 s more. A critical hit
    /// wins: the two are never sent together.
    pub hit_body: Option<[u8; 3]>,
    /// The reticle's pick as the target readout shows it (card HTGT);
    /// `None` = nothing picked.
    pub target: Option<HudTarget>,
    /// Where the reticle sits: a fraction of the window's height from the
    /// top (`game::walk::WalkSim::reticle_y`; [`reticle_y`] is the PS2's
    /// place for a camera section's tilt).
    pub reticle_y: f32,
    pub dead: bool,
    /// Seconds until the dead player's spawn slot (member timer ∨ spawn wave).
    pub respawn_in: Option<f32>,
    /// Winning team once the round is decided; 2 = the time limit ran out
    /// on equal reinforcements and both teams win (`core::sim::OUTCOME_DRAW`).
    pub outcome: Option<u8>,
    /// The domination countdown (issue #33, `analysis/re-round-end.md` §1.2):
    /// (the viewer's side holds every post, seconds left) - "Victory In" to
    /// the holders, "Defeat In" to the others; None hides both.
    pub countdown: Option<(bool, f32)>,
    /// The end of the round (issue #33): its banner, then its stats screens.
    pub round_end: Option<RoundEndModel>,
    /// Seconds left on the round's time limit (BF2's `ShowTimer`, queue
    /// TLO); `None` = no limit, and nothing is drawn.
    pub time_left: Option<f32>,
    /// The event lines that arrived since the last frame, oldest first.
    /// Each is shown once: hand a frame's lines over and forget them.
    pub messages: Vec<HudMessage>,
    /// `None` publishes no statistics (no pop-up).
    pub stats: Option<HudStats>,
    /// The local player's team (colors the "you" side).
    pub player_team: u8,
    /// The minimap (card BMAP); `None` = no Map element is shown.
    pub map: Option<crate::hud_map::MapModel>,
    /// The squad order cross (card SQHUD); `None` = no order is shown.
    pub squad: Option<crate::hud_squad::SquadCross>,
    /// The command-post capture icon (card CPBAR); `None` = not in a post's
    /// capture region, or nothing to show there.
    pub capture: Option<crate::hud_capture::CaptureIcon>,
    /// The edges the player's hits lit since the last frame (card HATK,
    /// `crate::hud_ambush::edge`); each restarts its chevron once.
    pub ambush: Vec<crate::hud_ambush::Edge>,
    /// Full-screen local damage flash opacity, 0..1.
    pub damage_flash: f32,
    /// The spawn screen drawn in the HUD's frame (card SSELB,
    /// `crate::spawn_draw`); `None` = none up.
    pub spawn: Option<crate::spawn_draw::SpawnPage>,
    /// The sniper scope's overlay (`crate::hud_scope`); `None` = not scoped.
    /// While it is up the weapon panel is off (BF2's `weaponsDisable`).
    pub scope: Option<crate::hud_scope::ScopeView>,
}

/// PROVISIONAL team accents for the egui HUD and spawn menu (orange/green);
/// these RGB values have not been measured from the PS2 HUD.
pub const TEAM_COLORS: [Color32; 2] = [
    Color32::from_rgb(242, 100, 40),
    Color32::from_rgb(80, 220, 90),
];

/// The reticle's place for a chase camera that looks `tilt` radians below
/// the aim (`analysis/re-aim-assist.md` §10.4): tan(tilt) / tan(fov_y / 2) of
/// the half height above the centre, as a fraction of the height from the
/// top. BF1 (5 deg, 0.34131): line 178.5 of 480; BF2 (10 deg, 0.48838):
/// 153.35.
pub fn reticle_y(tilt: f32, tan_half_fov_y: f32) -> f32 {
    0.5 - 0.5 * tilt.tan() / tan_half_fov_y
}

/// The reticle's colour by what it picks: enemy, nothing, friend — BF2's
/// `target.teamColor`, which the reticle listens to (`re-target-hud.md` §3
/// [V][O]: BF2's friend is blue, not BF1's green). Nothing is the tree's own
/// white — the PS2's reticle element holds ff ff ff in RAM with no target
/// (HOT1 state, element 0x187d000).
pub const RETICLE_COLORS: [[u8; 3]; 3] = [
    target_color::ENEMY,
    target_color::NONE,
    target_color::FRIEND,
];

/// Publish `m` as the 1-player HUD's events (`re-hud.md` §3). Change-gated
/// like the PS2's publishers, so call it every frame. `has_mesh` answers
/// whether a model of that name can be drawn (the NameMesh fallback).
pub fn feed(hud: &mut Hud, m: &HudModel, has_mesh: &dyn Fn(&str) -> bool) {
    let mut put = |event: &str, v: EventValue| hud.publish(event, v, has_mesh);
    put("initialize", EventValue::None);
    let alive = !m.dead;

    // Health and stamina: shown while alive.
    if alive {
        let frac = if m.hp_max > 0.0 { m.hp / m.hp_max } else { 0.0 };
        let health = if m.hero {
            "player1.hero.health"
        } else {
            "player1.health"
        };
        put(health, EventValue::Number(m.hp.max(0.0)));
        put(
            if m.hero {
                "player1.hero.healthFraction"
            } else {
                "player1.healthFraction"
            },
            EventValue::Number(frac.clamp(0.0, 1.0)),
        );
        put(
            if m.hero {
                "player1.healthDisable"
            } else {
                "player1.hero.healthDisable"
            },
            EventValue::None,
        );
        let e = if m.energy_max > 0.0 {
            m.energy / m.energy_max
        } else {
            0.0
        };
        put(
            "player1.energyFraction",
            EventValue::Number(e.clamp(0.0, 1.0)),
        );
    }

    if let Some((fuel, border)) = m.jet.filter(|_| alive) {
        let fuel = fuel.clamp(0.0, 1.0);
        put("player1.jetFuelThreshold", EventValue::Number(border));
        put("player1.jetFuelFraction", EventValue::Number(fuel));
        put(
            "player1.jetFuelWarning",
            EventValue::Number(if border > 0.0 && fuel <= border {
                1.0 - fuel / border
            } else {
                0.0
            }),
        );
    }

    if let Some((message, seconds)) = &m.hero_offer {
        put(
            "player1.heroSelect.message",
            EventValue::Text(message.clone()),
        );
        put(
            "player1.heroSelect.timer",
            EventValue::Number(seconds.max(0.0)),
        );
        put(
            "player1.heroSelect.timerFraction",
            EventValue::Number((seconds / 30.0).clamp(0.0, 1.0)),
        );
    } else {
        put("player1.heroSelect.disable", EventValue::None);
    }

    // The weapon in hand (row 1) and the secondary channel's (row 2: the
    // grenade and its count, clip + reserve — `re-grenades.md` §2.5).
    if !(alive && m.weapon2.is_some()) {
        put("player1.weapon2.disable", EventValue::None);
    }
    // The scope switches the weapon panel off (`crate::hud_scope`: BF2
    // sends `weaponsDisable` while its ScopeDisplay is up, `weaponsEnable`
    // again after); the rows keep their values.
    let scoped = alive && m.scope.is_some();
    if alive && m.weapon.is_some() && !scoped {
        put("player1.weaponsEnable", EventValue::None);
    }
    for (row, w) in [(1, &m.weapon), (2, &m.weapon2)] {
        let (Some(w), true) = (w, alive) else {
            continue;
        };
        // Row 2 needs row 1's group on; it is never shown alone.
        if row == 2 && m.weapon.is_none() {
            continue;
        }
        let ev = |name: &str| format!("player1.weapon{row}.{name}");
        // The mesh: the PS2's `.change` through the NameMesh table. `[ours]`
        // what its tree has no event for: a weapon with no model shows its
        // icon sheet, else nothing in the last one's place (the backdrop
        // stays). Each path forgets the other's event, so a switch back
        // sends again.
        let drawable = hud.name_mesh(&ev("change"), &w.key, has_mesh).is_some();
        let shown = match (&w.icon, drawable) {
            (Some(sheet), _) => Some(format!("bitmap:{sheet}")),
            (None, false) => Some(String::new()),
            (None, true) => None,
        };
        match shown {
            Some(mesh) => {
                hud.forget(&ev("change"));
                hud.publish(&ev("mesh"), EventValue::Text(mesh), has_mesh);
            }
            None => {
                hud.forget(&ev("mesh"));
                hud.publish(&ev("change"), EventValue::Text(w.key.clone()), has_mesh);
            }
        }
        let mut put = |event: &str, v: EventValue| hud.publish(event, v, has_mesh);
        put(&ev("name"), EventValue::Text(w.name.clone()));
        // An infinite weapon sends the count as -1 beside `ammoInfinite`
        // (`re-hud.md` §3 [V][O]); the count's Text prints nothing for it.
        match w.total {
            Some(n) => put(&ev("totalAmmoBullets"), EventValue::Number(n as f32)),
            None => {
                put(&ev("totalAmmoBullets"), EventValue::Number(INFINITE));
                put(&ev("ammoInfinite"), EventValue::None);
            }
        }
        put(
            &ev("totalClipFraction"),
            EventValue::Number(w.clip_fraction.clamp(0.0, 1.0)),
        );
        // `[ours]` a counted weapon takes the infinity ring away.
        if hud.falls(&format!("hud.gate.weapon{row}.infinite"), w.total.is_none()) {
            if let Some(i) = hud.find(&format!("player1weapon{row}ammoInfinite")) {
                hud.elems[i].fader.hide();
            }
            hud.forget(&ev("ammoInfinite"));
        }
    }

    // The minimap's Small view: while there is a controlled soldier.
    hud.map = m.map.clone();
    hud.squad = m.squad.clone();
    hud.capture = m.capture;
    hud.damage_flash = m.damage_flash;
    for &e in &m.ambush {
        hud.ambush.hit(e);
    }
    hud.spawn = m.spawn.clone();
    hud.scope = m.scope.filter(|_| alive);
    hud.banner = m
        .round_end
        .as_ref()
        .filter(|r| r.stats.is_none())
        .map(|r| r.result);
    hud.stats = m
        .round_end
        .as_ref()
        .and_then(|r| r.stats.clone())
        .map(Box::new);
    let mut put = |event: &str, v: EventValue| hud.publish(event, v, has_mesh);
    if alive && m.map.is_some() {
        put("player1.map.enable", EventValue::None);
    }

    // Reinforcements: team1 is the viewer's own side, team2 the other
    // (`FUN_003b59a0`); the count pulses once per lost reinforcement.
    let own = usize::from(m.player_team.min(1));
    for (slot, team) in [(1, own), (2, 1 - own)] {
        put(
            &format!("player1.team{slot}.reinforcements"),
            EventValue::Number(m.tickets[team] as f32),
        );
        let period = if m.bleed[team] > 0.0 {
            1.0 / m.bleed[team]
        } else {
            f32::MAX
        };
        put(
            &format!("player1.team{slot}.bleedRate"),
            EventValue::Number(period),
        );
    }

    match m.leaving.as_ref().filter(|_| alive) {
        Some(text) => hud.publish("player1.leaving", EventValue::Text(text.clone()), has_mesh),
        None => {
            if hud.last("player1.leaving").is_some() {
                hud.forget("player1.leaving");
                hud.send("player1.leavingOff", EventValue::None, has_mesh);
            }
        }
    }

    // Sent every frame, as the PS2's publisher sends them unconditionally
    // (`re-heat-hud.md` 2): the line holds and fades on the last send, and the
    // ring's colour transform follows each value.
    if alive && m.weapon.is_some() {
        let g = &m.gauge;
        let (ring, refire) = (g.ring.clamp(0.0, 1.0), g.refire.clamp(0.0, 1.0));
        hud.send("player1.weapon1.heat", EventValue::Number(ring), has_mesh);
        hud.send(
            "player1.weapon1.refire",
            EventValue::Number(refire),
            has_mesh,
        );
        if g.overheated {
            hud.send("player1.weaponsOverheat", EventValue::None, has_mesh);
        }
    }

    // The reticle sits where the aim ray goes on screen (card AIMA).
    // The target first: a lost one sends white, then the reticle's own.
    let target = m.target.as_ref().filter(|_| alive && m.reticle.is_some());
    feed_target(hud, target, has_mesh);
    feed_countdown(hud, m.countdown, has_mesh);
    let mut put = |event: &str, v: EventValue| hud.publish(event, v, has_mesh);
    if let (Some(rel), true) = (m.reticle, alive) {
        put(
            "player1.weapon1.reticule.position",
            EventValue::Position([0.5, m.reticle_y]),
        );
        let c = match target {
            Some(t) => t.color(),
            None => RETICLE_COLORS[(rel.clamp(-1, 1) + 1) as usize],
        };
        put("player1.weapon1.target.teamColor", EventValue::Color(c));
        // The hit flash (`FUN_003b3398` B2:410434-410467): every update the
        // hit timer runs, `hitCritical` (the factor over 1) or `hit`, and
        // its colour. [ours] (the maintainer, 2026-10-29): `hit` for every
        // enemy body hit, any weapon; the critical mark wins.
        let flash = match (m.hit_critical, m.hit_body) {
            (Some(c), _) => Some((c, "player1.weapon1.target.hitCritical")),
            (None, Some(c)) => Some((c, "player1.weapon1.target.hit")),
            (None, None) => None,
        };
        if let Some((c, event)) = flash {
            hud.send(
                "player1.weapon1.target.hitColor",
                EventValue::Color(c),
                has_mesh,
            );
            hud.send(event, EventValue::None, has_mesh);
        }
    }

    // The event lines: the colour first, then the text (the three senders,
    // `re-event-messages.md` §2 [V]). Never change-gated: the same line
    // twice is two messages.
    for msg in &m.messages {
        hud.send(
            "player1.message.color",
            EventValue::Color(msg.color),
            has_mesh,
        );
        hud.send(
            "player1.message",
            EventValue::Text(msg.text.clone()),
            has_mesh,
        );
    }

    // The statistics (`FUN_003b5de8` [V]): each number that differs from the
    // last one sent goes out with its change as text — "+n" when it rose,
    // the bare negative number when it fell, nothing the first time — and
    // one `changed` after them opens the pop-up.
    if let Some(st) = m.stats {
        let mut changed = false;
        for (name, v) in [
            ("points", st.points),
            ("kills", st.kills),
            ("deaths", st.deaths),
            ("heropoints", st.hero_points),
            ("captures", st.captures),
        ] {
            let event = format!("player1.statistic.{name}");
            let last = match hud.last(&event) {
                Some(EventValue::Number(l)) => Some(*l as i32),
                _ => None,
            };
            if last == Some(v) {
                continue;
            }
            let delta = v - last.unwrap_or(0);
            let text = match delta {
                0 => String::new(),
                d if d > 0 => format!("+{d}"),
                d => format!("{d}"),
            };
            hud.publish(&event, EventValue::Number(v as f32), has_mesh);
            hud.send(&format!("{event}Delta"), EventValue::Text(text), has_mesh);
            changed = true;
        }
        if changed {
            hud.send("player1.statistic.changed", EventValue::None, has_mesh);
        }
    }

    // Hiding: a disable the PS2 sends once, sent here on the falling edge;
    // what was published is forgotten so the next show sends it again.
    let hide = |hud: &mut Hud, gate: &str, on: bool, disables: &[&str], forget: &[&str]| {
        if hud.falls(gate, on) {
            for d in disables {
                hud.send(d, EventValue::None, has_mesh);
            }
            for f in forget {
                hud.forget(f);
            }
        }
    };
    hide(
        hud,
        "hud.gate.jet",
        alive && m.jet.is_some(),
        &["player1.jetDisable"],
        &[
            "player1.jetFuelFraction",
            "player1.jetFuelThreshold",
            "player1.jetFuelWarning",
        ],
    );
    hide(
        hud,
        "hud.gate.hero_offer",
        m.hero_offer.is_some(),
        &["player1.heroSelect.disable"],
        &[
            "player1.heroSelect.message",
            "player1.heroSelect.timer",
            "player1.heroSelect.timerFraction",
        ],
    );
    hide(
        hud,
        "hud.gate.ordinary_health",
        alive && !m.hero,
        &["player1.healthDisable"],
        &["player1.health", "player1.healthFraction"],
    );
    hide(
        hud,
        "hud.gate.hero_health",
        alive && m.hero,
        &["player1.hero.healthDisable"],
        &["player1.hero.health", "player1.hero.healthFraction"],
    );
    hide(
        hud,
        "hud.gate.alive",
        alive,
        &[
            "player1.healthDisable",
            "player1.hero.healthDisable",
            "player1.energyDisable",
        ],
        &[
            "player1.health",
            "player1.healthFraction",
            "player1.hero.health",
            "player1.hero.healthFraction",
            "player1.energyFraction",
        ],
    );
    hide(
        hud,
        "hud.gate.weapon",
        alive && m.weapon.is_some(),
        &["player1.weaponsDisable", "player1.weapon1.disable"],
        &[
            "player1.weaponsEnable",
            "player1.weapon1.change",
            "player1.weapon1.mesh",
            "player1.weapon1.name",
            "player1.weapon1.totalAmmoBullets",
            "player1.weapon1.ammoInfinite",
            "player1.weapon1.totalClipFraction",
        ],
    );
    // Scoped: the panel's group alone goes off (not `weapon1.disable`,
    // which the reticle listens to too).
    hide(
        hud,
        "hud.gate.unscoped",
        !scoped,
        &["player1.weaponsDisable"],
        &["player1.weaponsEnable"],
    );
    hide(
        hud,
        "hud.gate.weapon2",
        alive && m.weapon.is_some() && m.weapon2.is_some(),
        &["player1.weapon2.disable"],
        &[
            "player1.weapon2.change",
            "player1.weapon2.mesh",
            "player1.weapon2.name",
            "player1.weapon2.totalAmmoBullets",
            "player1.weapon2.ammoInfinite",
            "player1.weapon2.totalClipFraction",
        ],
    );
    hide(
        hud,
        "hud.gate.reticle",
        alive && m.reticle.is_some(),
        &[
            "player1.weapon1.reticule.disable",
            "player1.weapon1.target.disable",
        ],
        &["player1.weapon1.reticule.position"],
    );
    hide(
        hud,
        "hud.gate.map",
        alive && m.map.is_some(),
        &["player1.map.disable"],
        &["player1.map.enable"],
    );
}

/// The target readout's events (`re-target-hud.md` §3 [V][O],
/// `FUN_003b3398`): on a change of target `disable` and `disableShieldName`
/// first, then — no new target — `teamColor` white, else the new `className`;
/// every update with a target its colour and the shield's fraction and name
/// while one holds, else `disableShieldName` and the health's fraction.
fn feed_target(hud: &mut Hud, t: Option<&HudTarget>, has_mesh: &dyn Fn(&str) -> bool) {
    const ID: &str = "hud.target.id";
    let id = EventValue::Text(t.map(|t| t.id.to_string()).unwrap_or_default());
    if hud.last(ID) != Some(&id) {
        let first = hud.last(ID).is_none();
        hud.publish(ID, id, has_mesh);
        if !first {
            hud.send("player1.weapon1.target.disable", EventValue::None, has_mesh);
            hud.send(
                "player1.weapon1.target.disableShieldName",
                EventValue::None,
                has_mesh,
            );
        }
        for e in [
            "player1.weapon1.target.className",
            "player1.weapon1.target.healthFraction",
            "player1.weapon1.target.shieldName",
        ] {
            hud.forget(e);
        }
        if t.is_none() {
            hud.forget("player1.weapon1.target.teamColor");
            hud.publish(
                "player1.weapon1.target.teamColor",
                EventValue::Color(target_color::NONE),
                has_mesh,
            );
        }
    }
    let Some(t) = t else {
        return;
    };
    let mut put = |event: &str, v: EventValue| hud.publish(event, v, has_mesh);
    put(
        "player1.weapon1.target.className",
        EventValue::Text(t.name.clone()),
    );
    put(
        "player1.weapon1.target.teamColor",
        EventValue::Color(t.color()),
    );
    let fraction = match &t.shield {
        Some((label, f)) => {
            put(
                "player1.weapon1.target.shieldName",
                EventValue::Text(label.clone()),
            );
            *f
        }
        None => {
            if hud.last("player1.weapon1.target.shieldName").is_some() {
                hud.forget("player1.weapon1.target.shieldName");
                hud.send(
                    "player1.weapon1.target.disableShieldName",
                    EventValue::None,
                    has_mesh,
                );
            }
            t.health_fraction
        }
    };
    hud.publish(
        "player1.weapon1.target.healthFraction",
        EventValue::Number(fraction.clamp(0.0, 1.0)),
        has_mesh,
    );
}

/// `[ours]` "Defeat In" over its number, where "Victory In" sits: BF2's
/// layout puts the defeat label left of the number (0.664, 0.088 against
/// the victory label's 0.845, 0.040; re-round-end.md §1.2). A named
/// deviation, both labels in one place.
pub fn place_countdown_labels(hud: &mut Hud) {
    if let (Some(v), Some(d)) = (hud.find("victorytext"), hud.find("defeattext")) {
        hud.elems[d].pos = hud.elems[v].pos;
    }
}

/// The domination countdown's events (B2 HUD update `FUN_003b61e0`,
/// re-round-end.md §1.2): `victorytimer` / `defeattimer` with the seconds
/// left each update (the layout's VictoryTimerGroup / DefeatTimerGroup: the
/// label and the "%m:%02s" number), the other side's `.disable`, and both
/// `.disable` when it ends.
fn feed_countdown(hud: &mut Hud, c: Option<(bool, f32)>, has_mesh: &dyn Fn(&str) -> bool) {
    const SIDES: [&str; 2] = ["victorytimer", "defeattimer"];
    let on = c.map(|(victory, _)| usize::from(!victory));
    for (k, side) in SIDES.iter().enumerate() {
        if Some(k) != on && hud.last(side).is_some() {
            hud.forget(side);
            hud.send(&format!("{side}.disable"), EventValue::None, has_mesh);
        }
    }
    if let Some((victory, secs)) = c {
        let side = SIDES[usize::from(!victory)];
        hud.publish(side, EventValue::Number(secs.max(0.0)), has_mesh);
    }
}

fn panel() -> Frame {
    Frame::new()
        .fill(Color32::from_black_alpha(140))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .corner_radius(4.0)
}

/// A controller-mapping warning (card PADM: a pad whose camera stick or
/// FIRE resolved to nothing), bottom-centre above the HUD bars, on every
/// screen — never a silently dead camera.
pub fn draw_pad_warning(ctx: &egui::Context, text: &str) {
    egui::Area::new(egui::Id::new("pad_warning"))
        .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -96.0))
        .show(ctx, |ui| {
            panel().show(ui, |ui| {
                ui.set_max_width(640.0);
                ui.label(
                    RichText::new(text)
                        .font(FontId::proportional(15.0))
                        .color(Color32::from_rgb(255, 120, 90)),
                );
            });
        });
}

/// What the end of the round shows (issue #33, `analysis/re-round-end.md`
/// §2.1; the stats pages issue #50).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoundEndModel {
    /// 0 victory, 1 defeat, 2 both teams win (the time limit on equal
    /// reinforcements).
    pub result: u8,
    /// The stats pages once the banner is down (drawn by the HUD pass,
    /// `crate::stats_draw`); `None` = the banner.
    pub stats: Option<crate::stats::StatsScreens>,
}

/// BF1's banner sheet, size and UV halves (`re-round-end.md` §2.1 [V]).
/// [ours] Both games use BF1's green victory and the same baked art.
pub(crate) fn draw_round_end(result: u8, size: [f32; 2]) -> crate::hud_draw::HudFrame {
    use crate::hud_draw::{HudFrame, HudVertex};
    let defeat = result == 1;
    let color = if defeat {
        [192.0, 64.0, 64.0]
    } else {
        [64.0, 192.0, 64.0]
    };
    let color = [color[0] / 255.0, color[1] / 255.0, color[2] / 255.0, 1.0];
    let scale = crate::hud_draw::frame_scale(size);
    let (hw, hh) = (128.0 * scale[0], 32.0 * scale[1]);
    let (cx, cy) = (size[0] * 0.5, size[1] * 0.5);
    let v = if defeat { 0.5 } else { 0.0 };
    let mut frame = HudFrame::default();
    frame.verts = [
        ([cx - hw, cy - hh, 0.0], [0.0, v]),
        ([cx + hw, cy - hh, 0.0], [1.0, v]),
        ([cx + hw, cy + hh, 0.0], [1.0, v + 0.5]),
        ([cx - hw, cy + hh, 0.0], [0.0, v + 0.5]),
    ]
    .map(|(pos, uv)| HudVertex { pos, uv, color })
    .into();
    frame.indices = vec![0, 1, 2, 0, 2, 3];
    frame.batch("common_bf1/ig_victory_defeat", false, false, 0);
    frame
}

/// The egui half of the HUD for this frame (module doc).
pub fn draw_hud(ctx: &egui::Context, m: &HudModel) {
    if m.round_end.is_some() {
        return;
    }
    // The round timer, top centre under the reinforcement counts.
    if let Some(t) = m.time_left {
        let secs = t.max(0.0).ceil() as u32;
        egui::Area::new(egui::Id::new("hud_timer"))
            .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 12.0))
            .show(ctx, |ui| {
                panel().show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{}:{:02}", secs / 60, secs % 60))
                            .font(FontId::monospace(18.0))
                            .color(Color32::WHITE),
                    );
                });
            });
    }

    // Center banner: round over / respawning.
    let banner = match m.outcome {
        Some(2) => Some(("TIME UP — BOTH TEAMS WIN".to_string(), Color32::WHITE)),
        Some(w) => Some((
            format!(
                "ROUND OVER — {} WINS",
                m.team_names[usize::from(w.min(1))].to_uppercase()
            ),
            TEAM_COLORS[usize::from(w.min(1))],
        )),
        None if m.dead => Some((
            match m.respawn_in {
                Some(s) => format!("RESPAWNING IN {}", s.ceil() as u32),
                None => "RESPAWNING…".to_string(),
            },
            Color32::LIGHT_GRAY,
        )),
        None => None,
    };
    if let Some((text, color)) = banner {
        egui::Area::new(egui::Id::new("hud_banner"))
            .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, -40.0))
            .show(ctx, |ui| {
                panel().show(ui, |ui| {
                    ui.label(RichText::new(text).size(30.0).color(color).strong());
                });
            });
    }
}

#[cfg(test)]
mod leaving_tests {
    use super::LeavingPulse;

    /// `re-boundary.md` T4: starts bright, swings round 128 in 0.28 s, resets.
    #[test]
    fn the_pulse_swings_round_the_middle_and_restarts() {
        let mut p = LeavingPulse::default();
        assert_eq!(p.step(false, 0.1), None);
        let first = p.step(true, 0.0).unwrap();
        assert!((first - 254.0 / 255.0).abs() < 1e-3);
        let (mut low, mut high, mut crossed) = (1.0f32, 0.0f32, 0);
        let mut last = first;
        for _ in 0..60 {
            let a = p.step(true, 1.0 / 60.0).unwrap();
            (low, high) = (low.min(a), high.max(a));
            crossed += usize::from((last - 0.5) * (a - 0.5) < 0.0);
            last = a;
        }
        assert!(low < 0.1 && high > 0.9, "{low}..{high}");
        assert!(
            (6..=8).contains(&crossed),
            "two crossings a 0.28 s swing: {crossed}"
        );
        assert_eq!(p.step(false, 1.0 / 30.0), None);
        assert!(
            (p.step(true, 0.0).unwrap() - first).abs() < 1e-6,
            "from the top"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> HudModel {
        HudModel {
            damage_flash: 0.0,
            gauge: Default::default(),
            leaving: None,
            team_names: ["rep".into(), "cis".into()],
            tickets: [150, 143],
            bleed: [0.0; 2],
            hp: 320.0,
            hp_max: 400.0,
            hero: false,
            hero_offer: None,
            energy: 55.0,
            energy_max: 100.0,
            jet: None,
            weapon: None,
            weapon2: None,
            reticle: Some(0),
            hit_critical: None,
            hit_body: None,
            target: None,
            reticle_y: 0.5,
            dead: false,
            respawn_in: None,
            outcome: None,
            countdown: None,
            round_end: None,
            time_left: Some(125.0),
            messages: Vec::new(),
            stats: None,
            player_team: 0,
            map: None,
            squad: None,
            capture: None,
            ambush: Vec::new(),
            spawn: None,
            scope: None,
        }
    }

    /// The real feed selects the banner sheet and clears it on the next round.
    #[test]
    fn round_banner_uses_the_sheet_half_and_hides_the_battle_hud() {
        for (result, v, rgb) in [
            (0, 0.0, [64.0, 192.0, 64.0]),
            (1, 0.5, [192.0, 64.0, 64.0]),
            (2, 0.0, [64.0, 192.0, 64.0]),
        ] {
            let mut hud = crate::hud_tree::fixture::target_hud();
            let mut model = model();
            model.round_end = Some(RoundEndModel {
                result,
                stats: None,
            });
            feed(&mut hud, &model, &|_| false);
            let frame = crate::hud_draw::draw(&hud, &Default::default(), [1280.0, 896.0]);
            assert_eq!(frame.verts.len(), 4);
            assert_eq!(frame.batches[0].texture, "common_bf1/ig_victory_defeat");
            assert_eq!(frame.verts[0].uv, [0.0, v]);
            assert_eq!(frame.verts[2].uv, [1.0, v + 0.5]);
            let a = frame.verts[0].pos;
            let b = frame.verts[2].pos;
            assert!((a[0] + b[0] - 1280.0).abs() < 0.001);
            assert!((a[1] + b[1] - 896.0).abs() < 0.001);
            assert!((b[0] - a[0] - 605.6363).abs() < 0.001);
            assert!((b[1] - a[1] - 134.1952).abs() < 0.001);
            assert_eq!(
                frame.verts[0].color,
                [rgb[0] / 255.0, rgb[1] / 255.0, rgb[2] / 255.0, 1.0]
            );
            model.round_end = None;
            feed(&mut hud, &model, &|_| false);
            assert_eq!(hud.banner, None);
        }
    }

    /// The egui overlay also lays out at wide aspects.
    #[test]
    fn hud_lays_out_headless_at_two_aspects() {
        for (w, h) in [(1280.0, 720.0), (2560.0, 1080.0)] {
            let ctx = egui::Context::default();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(w, h))),
                ..Default::default()
            };
            // Anchored areas size themselves on frame 1 and lay out on
            // frame 2 (egui_kittest runs the same warm-up), so judge frame 2.
            let _ = ctx.run(input.clone(), |ctx| draw_hud(ctx, &model()));
            let out = ctx.run(input, |ctx| draw_hud(ctx, &model()));
            assert!(!out.shapes.is_empty(), "{w}x{h}: no shapes");
            // Shapes sharing a clip rect merge into one primitive; count
            // geometry instead.
            let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
            let indices: usize = prims
                .iter()
                .map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.indices.len(),
                    _ => 0,
                })
                .sum();
            assert!(indices > 100, "{w}x{h}: {indices} indices");
        }
        // Banner states don't panic.
        let ctx = egui::Context::default();
        let mut m = model();
        m.outcome = Some(1);
        m.dead = true;
        let _ = ctx.run(egui::RawInput::default(), |ctx| draw_hud(ctx, &m));
        m.outcome = Some(2);
        m.time_left = Some(0.0);
        let _ = ctx.run(egui::RawInput::default(), |ctx| draw_hud(ctx, &m));
    }

    /// The target readout's feed (`re-target-hud.md` §3): a picked enemy at
    /// full then half health sends its name, 1.0 then 0.5 and the enemy red
    /// to the bar and the reticle alike; a friend is blue, a team-0 object
    /// white; a switch keeps the readout up; losing the pick fades it out
    /// and turns the bar white.
    #[test]
    fn the_target_readout_follows_the_pick() {
        let mut hud = crate::hud_tree::fixture::target_hud();
        let fill = hud.find("player1targethealth_fill").unwrap();
        let name = hud.find("player1targetinfo_classNameNEW").unwrap();
        let shield = hud.find("player1targetinfo_shieldName").unwrap();
        let color = "player1.weapon1.target.teamColor";
        let enemy = HudTarget {
            id: 7,
            name: "Clone Trooper".into(),
            health_fraction: 1.0,
            relation: -1,
            team0: false,
            shield: None,
        };
        let mut m = model();
        m.target = Some(enemy.clone());
        let step = |hud: &mut Hud, m: &HudModel| {
            feed(hud, m, &|_| false);
            hud.advance(1.0 / 30.0);
        };
        step(&mut hud, &m);
        assert_eq!(hud.elems[name].text, "Clone Trooper");
        assert_eq!(hud.elems[fill].value, 1.0);
        assert_eq!(hud.elems[fill].fader.state, 2, "shown at once");
        assert_eq!(hud.elems[name].fader.state, 2);
        assert_eq!(hud.elems[fill].color, [200.0, 100.0, 100.0]);
        assert_eq!(hud.last(color), Some(&EventValue::Color([200, 100, 100])));
        m.target.as_mut().unwrap().health_fraction = 0.5;
        step(&mut hud, &m);
        assert_eq!(hud.elems[fill].value, 0.5);
        assert!(hud.elems[fill].flash.is_some_and(|f| f.fall()));
        // A friend at the same health: a new target, kept up.
        m.target = Some(HudTarget {
            id: 8,
            name: "Super Battle Droid".into(),
            relation: 1,
            ..enemy.clone()
        });
        step(&mut hud, &m);
        assert_eq!(hud.elems[name].text, "Super Battle Droid");
        assert_eq!(hud.elems[fill].color, [1.0, 86.0, 213.0]);
        assert_eq!(hud.elems[fill].fader.state, 2, "a switch keeps the bar up");
        assert_eq!(hud.elems[name].fader.state, 2);
        // A team-0 turret is white, whatever the relation; a droideka's
        // shield shows its label and the shield's fraction.
        m.target = Some(HudTarget {
            id: 9,
            name: "Laser Turret".into(),
            team0: true,
            shield: Some(("Shield".into(), 0.25)),
            ..enemy.clone()
        });
        step(&mut hud, &m);
        assert_eq!(hud.elems[fill].color, [255.0; 3]);
        assert_eq!(hud.elems[shield].text, "Shield");
        assert_eq!(hud.elems[shield].fader.state, 2);
        assert_eq!(hud.elems[fill].value, 0.25);
        m.target.as_mut().unwrap().shield = None;
        step(&mut hud, &m);
        assert!(hud.elems[shield].fader.state >= 3, "the shield line goes");
        assert_eq!(hud.elems[fill].value, 1.0);
        // A neutral team: yellow.
        m.target = Some(HudTarget {
            id: 10,
            relation: 0,
            ..enemy.clone()
        });
        step(&mut hud, &m);
        assert_eq!(hud.elems[fill].color, [223.0, 223.0, 32.0]);
        // Lost: everything fades out over its 1 s, keeping the last value;
        // the bar turns white at once.
        m.target = None;
        step(&mut hud, &m);
        for i in [fill, name] {
            assert_eq!(
                hud.elems[i].fader.state, 4,
                "{} fades out",
                hud.elems[i].name
            );
        }
        assert_eq!(hud.elems[fill].color, [255.0; 3]);
        assert_eq!(hud.last(color), Some(&EventValue::Color([255, 255, 255])));
        for _ in 0..31 {
            step(&mut hud, &m);
        }
        assert_eq!(hud.elems[fill].fader.state, 0, "gone after 1 s");
    }

    /// The critical-hit flash (hitmark, `re-target-hud.md` §12): while the
    /// model carries it, `player1reticule_criticalhit` is up in the colour
    /// sent; after the last send it holds 0.6 s, sustains 0.5 s and goes.
    /// The plain `hit` flash never lights, and nothing lights without it.
    #[test]
    fn a_critical_hit_lights_the_reticle_mark() {
        let mut hud = crate::hud_tree::fixture::reticle_hud();
        let crit = hud.find("player1reticule_criticalhit").unwrap();
        let hit = hud.find("player1reticule_hit").unwrap();
        let step = |hud: &mut Hud, m: &HudModel| {
            feed(hud, m, &|_| false);
            hud.advance(1.0 / 30.0);
        };
        let mut m = model();
        for _ in 0..10 {
            step(&mut hud, &m);
        }
        assert_eq!(hud.elems[crit].fader.state, 0, "no hit: no mark");
        m.hit_critical = Some(target_color::ENEMY);
        // The 0.5 s hit timer: 15 frames of sends.
        for _ in 0..15 {
            step(&mut hud, &m);
            assert_eq!(hud.elems[crit].fader.state, 2, "up while sent");
        }
        assert_eq!(hud.elems[crit].color, [200.0, 100.0, 100.0], "enemy red");
        assert!(hud.alpha(crit) > 0.0);
        assert_eq!(hud.elems[hit].fader.state, 0, "no plain hit flash");
        m.hit_critical = None;
        // Hold 0.6 s + sustain 0.5 s after the last send, then out.
        for _ in 0..30 {
            step(&mut hud, &m);
        }
        assert!(hud.elems[crit].fader.state >= 2, "still up after 1 s");
        for _ in 0..5 {
            step(&mut hud, &m);
        }
        assert_eq!(hud.elems[crit].fader.state, 0, "gone after 1.1 s");
    }

    /// [ours] (the maintainer, 2026-10-29): an enemy body hit lights the
    /// plain `hit` flash while its 0.5 s timer is sent, enemy red; a
    /// critical hit wins - the two are never up for one hit.
    #[test]
    fn a_body_hit_lights_the_plain_hit_flash() {
        let mut hud = crate::hud_tree::fixture::reticle_hud();
        let crit = hud.find("player1reticule_criticalhit").unwrap();
        let hit = hud.find("player1reticule_hit").unwrap();
        let step = |hud: &mut Hud, m: &HudModel| {
            feed(hud, m, &|_| false);
            hud.advance(1.0 / 30.0);
        };
        let mut m = model();
        for _ in 0..10 {
            step(&mut hud, &m);
        }
        assert_eq!(hud.elems[hit].fader.state, 0, "no hit: no flash");
        m.hit_body = Some(target_color::ENEMY);
        for _ in 0..15 {
            step(&mut hud, &m);
            assert_eq!(hud.elems[hit].fader.state, 2, "up while sent");
        }
        assert_eq!(hud.elems[hit].color, [200.0, 100.0, 100.0], "enemy red");
        assert!(hud.alpha(hit) > 0.0);
        assert_eq!(hud.elems[crit].fader.state, 0, "no critical mark");
        m.hit_body = None;
        for _ in 0..30 {
            step(&mut hud, &m);
        }
        assert_eq!(hud.elems[hit].fader.state, 0, "gone after 1 s");
        // A headshot: the body hit's timer runs too, the claws alone show.
        m.hit_body = Some(target_color::ENEMY);
        m.hit_critical = Some(target_color::ENEMY);
        for _ in 0..15 {
            step(&mut hud, &m);
        }
        assert_eq!(hud.elems[crit].fader.state, 2, "the claws");
        assert_eq!(hud.elems[hit].fader.state, 0, "crit wins");
    }

    /// BF2's reticle colours (`re-target-hud.md` §3): friend blue, not BF1's
    /// green.
    #[test]
    fn the_reticle_takes_bf2s_target_colours() {
        assert_eq!(
            RETICLE_COLORS,
            [[200, 100, 100], [255, 255, 255], [1, 86, 213]]
        );
    }
}

#[cfg(test)]
mod jet_tests {
    use super::*;
    #[test]
    fn jet_fuel_events_warn_at_the_border_and_disable_on_death_or_class_change() {
        let mut hud = crate::hud_tree::fixture::target_hud();
        let mut model = HudModel {
            jet: Some((0.12, 0.24)),
            ..Default::default()
        };
        feed(&mut hud, &model, &|_| false);
        assert_eq!(
            hud.last("player1.jetFuelFraction"),
            Some(&EventValue::Number(0.12))
        );
        assert_eq!(
            hud.last("player1.jetFuelThreshold"),
            Some(&EventValue::Number(0.24))
        );
        assert_eq!(
            hud.last("player1.jetFuelWarning"),
            Some(&EventValue::Number(0.5))
        );
        for (fuel, border) in [(0.24, 0.24), (1.0, 0.24), (0.0, 0.0)] {
            model.jet = Some((fuel, border));
            feed(&mut hud, &model, &|_| false);
            assert_eq!(
                hud.last("player1.jetFuelWarning"),
                Some(&EventValue::Number(0.0))
            );
        }
        model.dead = true;
        feed(&mut hud, &model, &|_| false);
        assert_eq!(hud.last("player1.jetFuelFraction"), None);
        model.dead = false;
        model.jet = None;
        feed(&mut hud, &model, &|_| false);
        assert_eq!(hud.last("player1.jetFuelFraction"), None);
    }
}
