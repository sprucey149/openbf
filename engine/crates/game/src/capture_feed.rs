//! The capture icon's rule (card CPBAR, `analysis/re-capture-display.md`
//! §3.1 steps 3-6 [V][O]): which post the player's icon shows and how, as
//! one pure function of the post's meters, the units inside per team and the
//! battle clock — [`icon`] — and the pick of the post, [`for_unit`].
//!
//! The PS2's rule, both games:
//! - the FIRST post whose capture region holds the player while he counts
//!   (`Sim::counts_in_cp`, the capture tick's own test);
//! - a neutral post: the line at `1 - capture / CaptureTime`, the capture
//!   meter's team below (grey when it has none), grey above; with two sides
//!   inside the top alternates each 0.5 s of battle clock with the colour of
//!   the first team inside that is not the meter's;
//! - an enemy post: the line at `neutralize / NeutralizeTime`, the owner below,
//!   grey above (no sign of defenders);
//! - his own post: nothing.
//!
//! `[ours]`, each behind its own switch in [`Rules`] (FEEL_NOTES):
//! - [`Rules::own_post`]: his own post while it is being lost — the mirror of
//!   what the attacker sees (his colour below, grey coming down at
//!   `neutralize / NeutralizeTime`) while that meter is above 0 or an enemy is
//!   inside (the maintainer's "losing" ask; the PS2 draws nothing);
//! - [`Rules::mark`]: which way the line moves this tick, from the sign of
//!   the meter's change by `core::cp::CommandPost::update`'s own rule (none
//!   while it holds or is frozen; the maintainer's "which way").
//!
//! Teams are the port's: 0-based, `None` = neutral.

use core::cp::CommandPost;
use core::sim::Sim;
use ui::hud_capture::{CaptureIcon, Mark, Side};

/// Which parts of the rule are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    pub own_post: bool,
    pub mark: bool,
}

impl Rules {
    /// The PS2's rule alone.
    pub const PS2: Rules = Rules {
        own_post: false,
        mark: false,
    };
    /// The port's: the PS2's and both additions.
    pub const PORT: Rules = Rules {
        own_post: true,
        mark: true,
    };
}

/// A post's state as the icon reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PostState {
    pub owner: Option<u8>,
    pub capture: f32,
    pub capture_team: Option<u8>,
    pub neutral: f32,
    pub capture_time: f32,
    pub neutralize_time: f32,
}

impl From<&CommandPost> for PostState {
    fn from(c: &CommandPost) -> Self {
        Self {
            owner: c.owner,
            capture: c.capture,
            capture_team: c.capture_team,
            neutral: c.neutral,
            capture_time: c.def.capture_time,
            neutralize_time: c.def.neutralize_time,
        }
    }
}

/// Which way the post's running meter moves this tick under
/// `CommandPost::update`: +1 up, -1 down, 0 holds.
pub fn meter_sign(post: &PostState, inside: &[u32]) -> i8 {
    let present = |t: usize| inside.get(t).is_some_and(|&n| n > 0);
    match post.owner {
        None => {
            let mut teams = (0..inside.len()).filter(|&t| present(t));
            match (teams.next(), teams.next()) {
                // One side: its own meter fills, another's drains first.
                (Some(t), None) => {
                    if post.capture_team.is_none_or(|c| usize::from(c) == t) {
                        1
                    } else {
                        -1
                    }
                }
                // Empty: holds; contested: frozen.
                _ => 0,
            }
        }
        Some(o) => {
            if (0..inside.len()).any(|t| t != usize::from(o) && present(t)) {
                1
            } else if post.neutral > 0.0 && present(usize::from(o)) {
                -1
            } else {
                0
            }
        }
    }
}

fn side(team: Option<u8>, viewer: u8) -> Side {
    match team {
        None => Side::Neutral,
        Some(t) if t == viewer => Side::Own,
        Some(_) => Side::Enemy,
    }
}

/// The icon for a viewer of team `viewer` who counts inside `post`, with
/// `inside[t]` counting units of team t and `clock` seconds of battle; `None`
/// draws nothing.
pub fn icon(
    post: &PostState,
    inside: &[u32],
    viewer: u8,
    clock: f32,
    rules: Rules,
) -> Option<CaptureIcon> {
    let sign = meter_sign(post, inside);
    let (split, top, bottom, line_down) = match post.owner {
        None => {
            let chg = post.capture_team;
            let mut teams = (0..inside.len()).filter(|&t| inside[t] > 0);
            let contested = teams.next().is_some() && teams.next().is_some();
            // The first team inside that is not the meter's (§3.1 step 4).
            let other = contested
                .then(|| {
                    (0..inside.len())
                        .find(|&t| inside[t] > 0 && chg.is_none_or(|c| usize::from(c) != t))
                })
                .flatten();
            let top = match other {
                Some(t) if (2.0 * clock) as i64 & 1 == 0 => side(Some(t as u8), viewer),
                _ => Side::Neutral,
            };
            // A filling meter raises the line.
            (
                1.0 - post.capture / post.capture_time,
                top,
                side(chg, viewer),
                sign < 0,
            )
        }
        Some(o) => {
            if o == viewer {
                let enemy = (0..inside.len()).any(|t| t != usize::from(o) && inside[t] > 0);
                if !rules.own_post || !(post.neutral > 0.0 || enemy) {
                    return None;
                }
            }
            // A rising neutralize meter brings the grey down.
            (
                post.neutral / post.neutralize_time,
                Side::Neutral,
                side(Some(o), viewer),
                sign > 0,
            )
        }
    };
    let mark = (rules.mark && sign != 0).then_some(if line_down { Mark::Down } else { Mark::Up });
    Some(CaptureIcon {
        split: split.clamp(0.0, 1.0),
        top,
        bottom,
        mark,
    })
}

/// The first post whose capture region holds unit `unit` while he counts,
/// with its units inside per team.
pub fn post_of(sim: &Sim, unit: usize) -> Option<(usize, [u32; 2])> {
    (0..sim.cps.len())
        .find(|&ci| sim.counts_in_cp(unit, ci))
        .map(|ci| (ci, sim.cp_inside(ci)))
}

/// The icon unit `unit` sees now under `rules`; `None` outside every post,
/// dead (he does not count) and after the outcome (the PS2's gates; the
/// pause menu's is the caller's).
pub fn for_unit(sim: &Sim, unit: usize, rules: Rules) -> Option<CaptureIcon> {
    if sim.outcome.is_some() {
        return None;
    }
    let (ci, inside) = post_of(sim, unit)?;
    let clock = sim.tick as f32 / core::TICK_HZ as f32;
    icon(
        &PostState::from(&sim.cps[ci]),
        &inside,
        sim.units[unit].team,
        clock,
        rules,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cp::CpDef;
    use core::region::Region;
    use glam::Vec3;

    fn post(owner: Option<u8>, capture: f32, team: Option<u8>, neutral: f32) -> PostState {
        PostState {
            owner,
            capture,
            capture_team: team,
            neutral,
            capture_time: 10.0,
            neutralize_time: 12.0,
        }
    }

    /// §3.1 step 4: a neutral post the viewer's side is taking.
    #[test]
    fn a_neutral_post_fills_from_the_bottom() {
        let i = icon(&post(None, 2.5, Some(0), 0.0), &[1, 0], 0, 0.0, Rules::PS2).unwrap();
        assert!((i.split - 0.75).abs() < 1e-6);
        assert_eq!((i.top, i.bottom, i.mark), (Side::Neutral, Side::Own, None));
        let i = icon(&post(None, 2.5, Some(0), 0.0), &[1, 0], 0, 0.0, Rules::PORT).unwrap();
        assert_eq!(i.mark, Some(Mark::Up));
        // The enemy's meter, drained by the viewer: the line goes down.
        let i = icon(&post(None, 4.0, Some(1), 0.0), &[1, 0], 0, 0.0, Rules::PORT).unwrap();
        assert_eq!((i.bottom, i.mark), (Side::Enemy, Some(Mark::Down)));
        // No meter's team yet: grey below.
        let i = icon(&post(None, 0.0, None, 0.0), &[0, 0], 0, 0.0, Rules::PORT).unwrap();
        assert_eq!((i.split, i.bottom, i.mark), (1.0, Side::Neutral, None));
    }

    /// Two sides inside: frozen, the top alternates each half second with
    /// the colour of the side that is not the meter's.
    #[test]
    fn a_contested_post_blinks_the_other_side() {
        let p = post(None, 3.0, Some(0), 0.0);
        let at = |clock| icon(&p, &[1, 1], 0, clock, Rules::PORT).unwrap();
        assert_eq!(at(0.1).top, Side::Enemy);
        assert_eq!(at(0.6).top, Side::Neutral);
        assert_eq!(at(1.1).top, Side::Enemy);
        assert_eq!(at(0.1).mark, None);
        // The meter has no team: the other side is the first inside.
        let p = post(None, 0.0, None, 0.0);
        assert_eq!(
            icon(&p, &[1, 1], 0, 0.1, Rules::PS2).unwrap().top,
            Side::Own
        );
        assert_eq!(
            icon(&p, &[1, 1], 1, 0.1, Rules::PS2).unwrap().top,
            Side::Enemy
        );
        // One side only: no blink.
        assert_eq!(
            icon(&p, &[1, 0], 1, 0.1, Rules::PS2).unwrap().top,
            Side::Neutral
        );
    }

    /// §3.1 step 5-6: an enemy post, and his own (nothing on the PS2).
    #[test]
    fn enemy_and_own_posts() {
        let p = post(Some(1), 0.0, None, 3.0);
        let i = icon(&p, &[1, 2], 0, 0.1, Rules::PORT).unwrap();
        assert!((i.split - 0.25).abs() < 1e-6);
        assert_eq!(
            (i.top, i.bottom, i.mark),
            (Side::Neutral, Side::Enemy, Some(Mark::Down))
        );
        assert!(icon(&p, &[1, 2], 1, 0.1, Rules::PS2).is_none());
        // Ours: his own post with the meter above 0 or an enemy inside.
        let i = icon(&p, &[0, 1], 1, 0.1, Rules::PORT).unwrap();
        assert_eq!((i.bottom, i.mark), (Side::Own, Some(Mark::Up)));
        assert!(icon(&post(Some(1), 0.0, None, 0.0), &[0, 1], 1, 0.1, Rules::PORT).is_none());
        let i = icon(&post(Some(1), 0.0, None, 0.0), &[1, 1], 1, 0.1, Rules::PORT).unwrap();
        assert_eq!((i.split, i.mark), (0.0, Some(Mark::Down)));
    }

    /// Acceptance B: one post driven through `CommandPost::update` tick by
    /// tick — enemy -> neutral -> his, a leave and return, a contest on each
    /// side's meter, his own post attacked and saved. Every tick he stands
    /// in it the icon's line is the running meter's fraction, the colours
    /// follow owner / capture_team, the top blinks only with two sides
    /// inside, the mark points where the next update moves the line, and
    /// under the PS2's rule his own post shows nothing.
    #[test]
    fn the_icon_follows_the_post_tick_by_tick() {
        let (capture_time, neutralize_time) = CpDef::class_times(false);
        let mut cp = CommandPost::new(
            CpDef {
                name: "cp".into(),
                pos: Vec3::ZERO,
                capture: Region::sphere(Vec3::ZERO, 8.0),
                control: None,
                value_bleed: 10,
                value_atk: [100, 100],
                value_def: [100, 100],
                spawn_nodes: Vec::new(),
                capture_time,
                neutralize_time,
            },
            Some(1),
        );
        const DT: f32 = 1.0 / 30.0;
        // (units inside per team, seconds); he is team 0.
        let script: [([u32; 2], f32); 11] = [
            ([1, 0], 6.0),  // neutralizes the enemy's post
            ([0, 0], 2.0),  // leaves: it holds
            ([0, 1], 2.0),  // a defender winds it back
            ([1, 0], 12.0), // back: neutral, then his meter rises
            ([1, 1], 3.0),  // contested on his meter
            ([0, 1], 5.0),  // the enemy drains it and charges its own
            ([1, 1], 3.0),  // contested on the enemy's meter
            ([2, 0], 9.0),  // his side drains it and captures
            ([1, 1], 3.0),  // his own post attacked with him inside
            ([1, 0], 4.0),  // saved
            ([1, 0], 1.0),  // his, nobody against: nothing
        ];
        let (mut tick, mut seen) = (0u32, [0usize; 7]);
        for (inside, secs) in script {
            for _ in 0..(secs / DT).round() as u32 {
                let clock = tick as f32 * DT;
                let before = PostState::from(&cp);
                let now = icon(&before, &inside, 0, clock, Rules::PORT);
                let ps2 = icon(&before, &inside, 0, clock, Rules::PS2);
                cp.update(DT, &inside);
                tick += 1;
                let after = PostState::from(&cp);
                if inside[0] == 0 {
                    continue;
                }
                let two_sides = inside.iter().all(|&n| n > 0);
                // Line and colours from the post's own fields.
                let (fraction, bottom) = match before.owner {
                    None => (
                        1.0 - before.capture / before.capture_time,
                        side(before.capture_team, 0),
                    ),
                    Some(o) => (before.neutral / before.neutralize_time, side(Some(o), 0)),
                };
                let own = before.owner == Some(0);
                assert_eq!(
                    ps2.is_none(),
                    own,
                    "PS2: nothing on his own post, tick {tick}"
                );
                let Some(i) = now else {
                    assert!(own && before.neutral == 0.0 && !two_sides, "tick {tick}");
                    seen[0] += 1;
                    continue;
                };
                assert!((i.split - fraction).abs() < 1e-6, "tick {tick}");
                assert_eq!(i.bottom, bottom, "tick {tick}");
                let blink = before.owner.is_none() && two_sides && (2.0 * clock) as i64 & 1 == 0;
                let other = if before.capture_team == Some(0) {
                    Side::Enemy
                } else {
                    Side::Own
                };
                assert_eq!(
                    i.top,
                    if blink { other } else { Side::Neutral },
                    "tick {tick}"
                );
                seen[1] += usize::from(blink && other == Side::Enemy);
                seen[6] += usize::from(blink && other == Side::Own);
                // The mark against the update's own move of the line (a
                // change of owner or meter team is a new line).
                if after.owner != before.owner || after.capture_team != before.capture_team {
                    seen[2] += 1;
                    continue;
                }
                let moved = match after.owner {
                    None => before.capture - after.capture,
                    Some(_) => after.neutral - before.neutral,
                };
                let want = if moved > 0.0 {
                    Some(Mark::Down)
                } else if moved < 0.0 {
                    Some(Mark::Up)
                } else {
                    None
                };
                assert_eq!(i.mark, want, "tick {tick}: the line moved {moved}");
                seen[3 + i.mark.map_or(0, |m| 1 + usize::from(m == Mark::Down))] += 1;
            }
        }
        assert_eq!(cp.owner, Some(0));
        println!(
            "hidden {} blinks enemy {} own {} changes {} held {} up {} down {}",
            seen[0], seen[1], seen[6], seen[2], seen[3], seen[4], seen[5]
        );
        assert!(seen.iter().all(|&n| n > 0), "every case was met: {seen:?}");
    }
}
