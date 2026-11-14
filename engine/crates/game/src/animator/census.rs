//! A census of what the animators hold over a headless round (card ANIML's
//! referee; `ANIM_CENSUS=1` on `round_to_outcome`): of the soldier updates
//! with a shot fired on the move, how many show the shoot clip over a
//! movement clip, and per weapon set how often the upper body plays a clip
//! of the set's own. Card ROLLS: per roll direction, how far the root's turn
//! is from the travel from the roll's 9th update on and at its end, and the
//! turn left 0.5 s after it. Card ANIMM: the legs' clips, the updates
//! a moving soldier's legs stand (BF1's chooser gives idle over 8 m/s), and
//! how far the feet's ground speed is from his with the plain clock and
//! with the PS2's rate.

use super::{aim, move_rate, wrap, ActionKind, AnimInput, Animator, Body, Move, Rig};
use crate::skin::{Clip, Pose, Skeleton};
use core::fmath::FloatExt;
use glam::{Quat, Vec3};
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, Copy)]
struct SetRow {
    updates: u64,
    /// The upper clip differs from the legs' (the set's own, or an action).
    own: u64,
    /// ... and it is a weapon action.
    action: u64,
}

#[derive(Debug, Default)]
pub struct Census {
    /// Updates with a shot fired at over 0.3 m/s, on his feet.
    moving_shots: u64,
    /// ... whose upper player holds the shoot clip and whose lower player a
    /// movement clip.
    layered: u64,
    /// Shots fired standing still, and those whose legs joined the action.
    still_shots: u64,
    still_joined: u64,
    /// Shots in a state that shows no action (jump, fall, roll, BF2 sprint).
    special_shots: u64,
    /// Card ANIMA: updates on his feet, those alert, and those on an alert
    /// row (the upper clip is not the plain row's).
    feet: u64,
    alert: u64,
    alert_row: u64,
    /// Alert updates with the aim over 0.1 rad off level: their count, and
    /// the summed angle (rad) between the gun (`hp_weapons`' +Z) and the aim
    /// line with the aim pose and without it.
    aimed: u64,
    gun_off: f64,
    gun_off_plain: f64,
    /// Those with the torso at full weight, and the summed angle between
    /// the gun and its clip's own line pitched by the eased aim.
    full: u64,
    turn_off: f64,
    /// Per set name.
    sets: BTreeMap<String, SetRow>,
    /// (upper clip over lower clip) while an action runs on the move.
    pairs: BTreeMap<(String, String), u64>,
    /// Card ROLLS, per direction of travel ([`RollRow::DIRS`]).
    rolls: [RollRow; 4],
    /// Rolls 0.5 s behind, and those with the twist still over 10 degrees
    /// (the largest, degrees).
    after: u64,
    after_over: u64,
    after_max: f32,
    /// Card ANIMM: updates on his feet per movement clip.
    legs: BTreeMap<String, u64>,
    /// Updates over 0.3 m/s with the legs standing (idle or a turn), and
    /// the fastest.
    standing: u64,
    standing_max: f32,
    /// Moving updates with a baked root speed: the summed |feet - speed| /
    /// speed with the PS2's rate and with the plain clock, and those over 5 %.
    slide: u64,
    slide_rate: f64,
    slide_plain: f64,
    slide_rate_over: u64,
    slide_plain_over: u64,
}

/// The root twist against the travel in rolls of one direction (degrees).
#[derive(Debug, Default, Clone, Copy)]
struct RollRow {
    updates: u64,
    /// Updates from the roll's 9th on: their count, the largest and summed
    /// |travel - twist|.
    late: u64,
    late_max: f32,
    late_sum: f64,
    /// Rolls ended, and the largest |travel - twist| on the last update.
    ended: u64,
    end_max: f32,
}

impl RollRow {
    /// |travel| bands (degrees): forward, forward-diagonal, side, behind.
    const DIRS: [(&'static str, f32); 4] = [
        ("forward", 10.0),
        ("diagonal", 60.0),
        ("side", 120.0),
        ("behind", 180.0),
    ];

    fn of(travel: f32) -> usize {
        let t = travel.to_degrees().abs();
        Self::DIRS.iter().position(|d| t < d.1).unwrap_or(3)
    }
}

impl Census {
    /// One unit after its animator's update for `input`.
    pub fn note(&mut self, rig: &Rig, clips: &[Clip], a: &Animator, input: &AnimInput) {
        let name = |c: Option<usize>| c.map_or("-", |c| clips[c].name.as_str()).to_owned();
        let row = self.sets.entry(rig.set_name(input.set)).or_default();
        row.updates += 1;
        row.own += u64::from(a.upper_clip() != a.lower_clip());
        row.action += u64::from(a.action().is_some());
        let moving = input.speed() > 0.3;
        let Some(body) = a.body() else { return };
        if let Body::Feet(_, m) = body {
            *self.legs.entry(format!("{m:?}")).or_default() += 1;
            if moving && !m.moving() {
                self.standing += 1;
                self.standing_max = self.standing_max.fmax(input.speed());
            }
            let vc = a.lower_clip().and_then(|c| rig.clip_speed(c));
            if let (true, Some(vc)) = (m.moving() && a.action().is_none_or(|(_, legs)| !legs), vc) {
                let vc = vc[usize::from(!matches!(
                    m,
                    Move::WalkLeft | Move::WalkRight | Move::RunLeft | Move::RunRight
                ))]
                .abs();
                let v = input.speed();
                let scale = if input.root_scale > 0.0 {
                    input.root_scale
                } else {
                    1.0
                };
                let rate = (move_rate(v / scale, vc) * vc * scale - v).abs() / v;
                let plain = (vc * scale - v).abs() / v;
                self.slide += 1;
                self.slide_rate += f64::from(rate);
                self.slide_plain += f64::from(plain);
                self.slide_rate_over += u64::from(rate > 0.05);
                self.slide_plain_over += u64::from(plain > 0.05);
            }
        }
        if input.fired {
            match (body, a.action()) {
                (Body::Special(_), _) => self.special_shots += 1,
                (Body::Feet(..), Some((ActionKind::Shoot, legs))) if moving => {
                    self.moving_shots += 1;
                    let (_, legs_clip) = rig.body_slots(input.set, body, a.aim().alert());
                    self.layered += u64::from(!legs && a.lower_clip() == legs_clip.map(|s| s.clip));
                }
                (Body::Feet(..), _) if moving => self.moving_shots += 1,
                (Body::Feet(..), act) => {
                    self.still_shots += 1;
                    self.still_joined += u64::from(act.is_some_and(|(_, legs)| legs));
                }
            }
        }
        let r = a.roll();
        let off = wrap(a.travel() - a.twist()).to_degrees().abs();
        if r.updates > 0 {
            let row = &mut self.rolls[RollRow::of(a.travel())];
            row.updates += 1;
            if r.updates >= 9 {
                row.late += 1;
                row.late_max = row.late_max.fmax(off);
                row.late_sum += f64::from(off);
            }
        }
        if let Some(gap) = r.ended {
            let row = &mut self.rolls[RollRow::of(a.travel())];
            row.ended += 1;
            row.end_max = row.end_max.fmax(gap.to_degrees().abs());
        }
        // The first update at or past 0.5 s after a roll (30 updates a
        // second, as the round steps them).
        if r.after
            .is_some_and(|t| (0.5 - 1e-3..0.5 + 1.0 / 30.0 - 1e-3).contains(&t))
        {
            let tw = a.twist().to_degrees().abs();
            self.after += 1;
            self.after_over += u64::from(tw > 10.0);
            self.after_max = self.after_max.fmax(tw);
        }
        if moving && a.action().is_some() {
            *self
                .pairs
                .entry((name(a.upper_clip()), name(a.lower_clip())))
                .or_default() += 1;
        }
    }

    /// The alert state and where the gun points, after the same update.
    pub fn note_aim(
        &mut self,
        rig: &Rig,
        skel: &Skeleton,
        hp_weapons: Option<usize>,
        a: &Animator,
        input: &AnimInput,
    ) {
        let Some(body @ Body::Feet(..)) = a.body() else {
            return;
        };
        self.feet += 1;
        if !a.aim().alert() {
            return;
        }
        self.alert += 1;
        let plain = rig.body_slots(input.set, body, false).0.map(|s| s.clip);
        self.alert_row += u64::from(a.action().is_none() && a.upper_clip() != plain);
        let (Some(hp), true) = (hp_weapons, input.aim.1.abs() > 0.1) else {
            return;
        };
        let p = input.aim.1.clamp(-aim::PITCH_LIMIT, aim::PITCH_LIMIT);
        let line = Vec3::new(0.0, p.sin(), p.cos());
        let off =
            |pose: &Pose| f64::from((aim::global(pose, skel, hp) * Vec3::Z).angle_between(line));
        self.aimed += 1;
        let (with, kept) = (a.pose(skel), a.kept_pose(skel));
        self.gun_off += off(&with);
        self.gun_off_plain += off(&kept);
        if a.aim().weights().0 >= 1.0 {
            let gun = |pose: &Pose| aim::global(pose, skel, hp) * Vec3::Z;
            let (yaw, pitch) = a.aim().turn(a.twist());
            let turn = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch);
            self.full += 1;
            self.turn_off += f64::from(gun(&with).angle_between(turn * gun(&kept)));
        }
    }

    pub fn report(&self, key: &str) -> String {
        let pct_f = |sum: f64, n: u64| {
            if n == 0 {
                "-".to_owned()
            } else {
                format!("{:.1} %", 100.0 * sum / n as f64)
            }
        };
        let pct = |a: u64, b: u64| {
            if b == 0 {
                "-".to_owned()
            } else {
                format!("{:.1} %", 100.0 * a as f64 / b as f64)
            }
        };
        let mut out = format!(
            "{key} anim census: shots on the move {} with the shoot clip over a movement clip {} ({}); \
             shots standing {} legs joined {} ({}); shots in a jump / fall / roll / sprint {}\n",
            self.moving_shots,
            self.layered,
            pct(self.layered, self.moving_shots),
            self.still_shots,
            self.still_joined,
            pct(self.still_joined, self.still_shots),
            self.special_shots,
        );
        let mean = |sum: f64| {
            if self.aimed == 0 {
                "-".to_owned()
            } else {
                format!("{:.3}", sum / self.aimed as f64)
            }
        };
        out += &format!(
            "{key} alert census: updates on his feet {} alert {} ({}) on an alert row {} ({}); alert and aiming \
             over 0.1 rad off level {}: the gun off the aim line, mean rad, {} with the aim pose, {} without; \
             torso at full weight {}: the gun off its clip's line turned by the eased aim, mean rad, {:.4}\n",
            self.feet,
            self.alert,
            pct(self.alert, self.feet),
            self.alert_row,
            pct(self.alert_row, self.alert),
            self.aimed,
            mean(self.gun_off),
            mean(self.gun_off_plain),
            self.full,
            self.turn_off / self.full.max(1) as f64,
        );
        for (row, (dir, _)) in self.rolls.iter().zip(RollRow::DIRS) {
            out += &format!(
                "{key} roll census {dir:8}: updates {:6}; from the 9th on {:6}, travel - twist max {:.1} mean {:.2} deg; \
                 rolls ended {:5}, at the end max {:.2} deg\n",
                row.updates,
                row.late,
                row.late_max,
                row.late_sum / row.late.max(1) as f64,
                row.ended,
                row.end_max,
            );
        }
        out += &format!(
            "{key} roll census after: rolls 0.5 s behind {}, twist over 10 deg {}, max {:.2} deg\n",
            self.after, self.after_over, self.after_max
        );
        let legs: Vec<String> = self.legs.iter().map(|(m, n)| format!("{m} {n}")).collect();
        out += &format!(
            "{key} legs census: {}; moving over 0.3 m/s with the legs standing {} (fastest {:.2} m/s); \
             moving with a root speed {}: feet off his speed mean {} with the rate ({} over 5 %), {} plain ({} over 5 %)\n",
            legs.join(", "),
            self.standing,
            self.standing_max,
            self.slide,
            pct_f(self.slide_rate, self.slide),
            pct(self.slide_rate_over, self.slide),
            pct_f(self.slide_plain, self.slide),
            pct(self.slide_plain_over, self.slide),
        );
        for (set, r) in &self.sets {
            out += &format!(
                "{key}   set {set:18} updates {:8}  upper clip not the legs' {:8} ({})  in an action {:7}\n",
                r.updates,
                r.own,
                pct(r.own, r.updates),
                r.action
            );
        }
        let mut pairs: Vec<_> = self.pairs.iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(a.1));
        for ((up, low), n) in pairs.iter().take(12) {
            out += &format!("{key}   {n:7}  {up}  over  {low}\n");
        }
        out
    }
}
