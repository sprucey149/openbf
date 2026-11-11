//! The AI side of a squad order (Phase D, brief D6): [`CmdState`], the
//! scratch state a commanded unit keeps between thinks, and
//! [`AiState::cmd_clear`], which takes an order's leftovers off a unit. The
//! command branch itself is [`crate::squad::steer`], which `think` calls; its
//! tests — follow, hold, release — live here.

use super::state::AiState;
use glam::Vec3;

// Only for `CmdState::goal`'s docs, which link `Route`.
#[cfg(doc)]
use super::state::Route;

impl AiState {
    /// Forget everything this unit was doing on someone else's behalf: the
    /// route and the post it was walking to. Phase D calls it wherever a squad
    /// order is put on a unit or taken off it (`squad::issue_command`,
    /// `clear_all`, `on_unit_died`) so the unit
    /// re-plans from scratch instead of finishing a leg it was given as part
    /// of a plan it is no longer in — and the follower scratch state
    /// ([`CmdState`]), so the next order starts in no follow state and with no
    /// release count. WGORD also cancels the active temporary boarding goal;
    /// explicit pending delivery is canceled by actual order writers, not pool expiry.
    pub fn cmd_clear(&mut self) {
        if let Some(r) = self
            .ride
            .as_mut()
            .filter(|r| r.ordered_call || r.called_board.is_some() || r.player_board.is_some())
        {
            r.board = None;
            r.called_board = None;
            r.player_board = None;
            r.ordered_call = false;
            r.maintenance = None;
            r.use_req = false;
        }
        // An authoritative order change ends any witnessed post-stop source.
        if let Some(w) = self.ride.as_mut().and_then(|r| r.pickup.as_mut()) {
            w.post_stop = None;
        }
        super::route::clear(self);
        self.target_cp = None;
        self.cmd = None;
    }
}

/// What a COMMANDED unit is doing about its order (brief D6) — the scratch
/// state `squad::steer` keeps between thinks, and nothing else.
///
/// It exists only while [`crate::unit::Unit::command`] does: it is written by
/// the command branch, cleared with the order ([`AiState::cmd_clear`]) and by
/// the release in `Sim::step`'s write-back. Hashed ONLY when `Some`, like the
/// order itself, so a round in which nobody commands hashes byte-for-byte as
/// it did before Phase D.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CmdState {
    /// The point this unit walks at when it has no [`Route`]: the leader, its
    /// standing point by him or the spot it holds.
    pub goal: Vec3,
    /// Ticks of thinks spent with the leader beyond
    /// [`crate::squad::LEAVE_DIST`], never refilled — the order ends at
    /// [`crate::squad::LEAVE_TICKS`] of them.
    pub far_ticks: u16,
    /// A follower's state in `squad::steer` (approach 2, follow 3, stand 4);
    /// `u64::MAX` is an order not thought about yet.
    pub roll: u64,
}

impl CmdState {
    /// The bytes [`AiState::hash_into`] eats for a unit that has one.
    pub fn hash_into(self, eat: &mut impl FnMut(&[u8])) {
        eat(&[1]);
        for v in [self.goal.x, self.goal.y, self.goal.z] {
            eat(&v.to_bits().to_le_bytes());
        }
        eat(&self.far_ticks.to_le_bytes());
        eat(&self.roll.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tests::l_graph;
    use crate::ai::{think, SteerPick, THINK_PERIOD};
    use crate::sim::{PadState, Sim};
    use crate::unit::UnitHandle;

    /// A leader in slot 0 at the origin and one AI follower beside him, on the
    /// L graph's flat ground (brief D6's rig): the AI is unit 1, the player 0.
    fn follow_sim(bot_at: Vec3) -> (Sim, UnitHandle, UnitHandle) {
        use crate::heightfield::Heightfield;
        use crate::unit::{Controller, UnitSpec};
        let mut sim = Sim::empty(7);
        sim.heightfield = Some(Heightfield {
            size: 2,
            unit: 400.0,
            offset_x: 200.0,
            offset_z: 200.0,
            heights: vec![0.0; 4],
        });
        sim.nav = Some(l_graph());
        let leader = sim
            .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
            .expect("leader");
        let bot = sim
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, bot_at)
            })
            .expect("bot");
        (sim, leader, bot)
    }

    /// D6: a follower further than `FOLLOW_NEAR` routes to its LEADER's hub —
    /// `retarget_hunt`'s shape with the leader in place of the enemy — and
    /// KEEPS that leg across thinks while the destination hub is unchanged.
    #[test]
    fn a_far_follower_routes_to_its_leaders_hub_and_keeps_the_leg() {
        let (mut sim, leader, bot) = follow_sim(Vec3::new(2.0, 0.0, 0.0));
        assert_eq!(
            sim.issue_command(0, crate::squad::CommandKind::Follow(leader)),
            1
        );
        // The leader walks off to hub 2 (40, -40); the bot is at hub 0.
        sim.unit_mut(leader).expect("leader").soldier.pos = Vec3::new(40.0, 0.0, -40.0);
        sim.step(&[PadState::default()]);
        let st = |s: &Sim| s.unit(bot).expect("bot").ai;
        let r = st(&sim).route().expect("a leg to the leader");
        assert_eq!(r.dest_hub, 2, "the leader's hub, not a post's");
        assert_eq!(st(&sim).target_cp, None, "a commanded bot takes no post");
        // Across the next thinks the destination hub does not move, so the leg
        // is kept rather than re-planned (the cadence deviation is harmless).
        for _ in 0..THINK_PERIOD * 3 {
            sim.unit_mut(leader).expect("leader").soldier.pos = Vec3::new(40.0, 0.0, -40.0);
            sim.step(&[PadState::default()]);
            assert_eq!(
                st(&sim).route().map(|r| r.dest_hub),
                Some(2),
                "still walking at the leader's hub"
            );
        }
        assert_eq!(sim.nav_stats.releases, 0, "nobody was released");
    }

    /// A follower whose leader is beyond `LEAVE_DIST` is released after exactly
    /// `LEAVE_TICKS` — not one think earlier — and the release ends the ORDER,
    /// not just the follower state.
    #[test]
    fn a_follower_left_behind_is_released_after_the_full_ten_seconds() {
        let (mut sim, leader, bot) = follow_sim(Vec3::new(2.0, 0.0, 0.0));
        assert_eq!(
            sim.issue_command(0, crate::squad::CommandKind::Follow(leader)),
            1
        );
        let mut last_far = 0u16;
        let mut released_at = None;
        for t in 0..2000u64 {
            // The leader keeps exactly LEAVE_DIST + 5 m ahead, so the
            // follower can never close the gap and the counter only grows.
            let p = sim.unit(bot).expect("bot").soldier.pos;
            sim.unit_mut(leader).expect("leader").soldier.pos =
                p + Vec3::new(crate::squad::LEAVE_DIST + 5.0, 0.0, 0.0);
            sim.step(&[PadState::default()]);
            match sim.unit(bot).expect("bot").command {
                Some(_) => last_far = sim.unit(bot).expect("bot").ai.cmd.expect("state").far_ticks,
                None => {
                    released_at = Some(t);
                    break;
                }
            }
        }
        assert!(released_at.is_some(), "the follower was released");
        assert_eq!(
            last_far,
            crate::squad::LEAVE_TICKS - THINK_PERIOD as u16,
            "the think BEFORE the release had not reached LEAVE_TICKS"
        );
        let u = sim.unit(bot).expect("bot");
        assert_eq!(u.command, None, "the order itself is gone");
        assert_eq!(u.ai.cmd, None, "and the follower state with it");
        assert_eq!(sim.nav_stats.releases, 1);
    }

    /// D6: a follower never walks at a leader who is not there. TWO different
    /// rules end it, and this pins which is which: the LEADER'S DEATH clears
    /// the order through D4 (the issuer's death clears everything they gave),
    /// while a handle that no longer resolves — a leader who has respawned
    /// under a new generation — is ended by the think branch as a RELEASE.
    #[test]
    fn a_follower_whose_leader_is_gone_ends_by_death_or_by_release() {
        let (mut sim, leader, bot) = follow_sim(Vec3::new(3.0, 0.0, 0.0));
        assert_eq!(
            sim.issue_command(0, crate::squad::CommandKind::Follow(leader)),
            1
        );
        sim.step(&[PadState::default()]);
        assert!(sim.unit(bot).expect("bot").command.is_some());
        sim.kill_environment(leader, crate::sim::DeathCause::Fall);
        assert_eq!(sim.unit(bot).expect("bot").command, None, "D4 cleared it");
        assert_eq!(sim.nav_stats.releases, 0, "the death rule, not a release");

        // The same order pointing at a generation that no longer exists.
        let (mut sim, leader, bot) = follow_sim(Vec3::new(3.0, 0.0, 0.0));
        sim.unit_mut(bot).expect("bot").command = Some(crate::squad::Command {
            kind: crate::squad::CommandKind::Follow(UnitHandle {
                index: leader.index,
                gen: leader.gen + 1,
            }),
            issuer: 0,
            issued_tick: 0,
        });
        for _ in 0..THINK_PERIOD * 2 {
            sim.step(&[PadState::default()]);
        }
        assert_eq!(sim.unit(bot).expect("bot").command, None, "released");
        assert_eq!(sim.nav_stats.releases, 1);
    }

    /// D6: a commanded bot still FIGHTS. The Phase C block is untouched by the
    /// command branch, so a follower with an enemy inside `HOLD_RANGE` stops
    /// and shoots exactly as an uncommanded unit does — and one with an enemy
    /// beyond it keeps walking at its leader while engaging.
    #[test]
    fn a_follower_still_engages_and_holds_inside_hold_range() {
        use crate::unit::UnitSpec;
        let engaged = |enemy_at: Vec3| {
            let (mut sim, leader, bot) = follow_sim(Vec3::new(3.0, 0.0, 0.0));
            sim.spawn(&UnitSpec::trooper(1, enemy_at)).expect("enemy");
            assert_eq!(
                sim.issue_command(0, crate::squad::CommandKind::Follow(leader)),
                1
            );
            // `think` directly, the shape
            // `an_engaged_unit_walks_on_until_hold_range` uses, so the pad this
            // tick is the thing under test.
            let mut st = sim.unit(bot).expect("bot").ai;
            let (pad, _) = think(&sim, bot, &mut st);
            assert!(
                sim.unit(bot).expect("bot").command.is_some(),
                "still under the order"
            );
            (st, pad)
        };
        // 40 m: past HOLD_RANGE, inside the Normal view distance.
        let (st, pad) = engaged(Vec3::new(0.0, 0.0, -40.0));
        assert!(st.target.is_some(), "a follower perceives and engages");
        assert_ne!(st.steer, SteerPick::Idle, "and walks on at its leader");
        assert_ne!((pad.lx, pad.ly), (0, 0), "{pad:?}");
        // 10 m: inside HOLD_RANGE, so it stops — the same rule as everyone.
        let (st, pad) = engaged(Vec3::new(0.0, 0.0, -10.0));
        assert!(st.target.is_some(), "engaged");
        assert_eq!(st.steer, SteerPick::Idle, "holds inside HOLD_RANGE");
        assert_eq!((pad.ly, pad.lx), (0, 0), "{pad:?}");
    }

    /// RE §4.3: a follower walks up to its still leader, takes a point within
    /// the +-3 m square about him (at most 4.3 m away) and STANDS there.
    #[test]
    fn a_follower_walks_up_to_a_still_leader_and_stands_by_him() {
        let (mut sim, leader, bot) = follow_sim(Vec3::new(8.0, 0.0, 0.0));
        assert_eq!(
            sim.issue_command(0, crate::squad::CommandKind::Follow(leader)),
            1
        );
        let mut nearest = f32::MAX;
        for _ in 0..240 {
            sim.step(&[PadState::default()]);
            let d = sim.unit(bot).expect("bot").soldier.pos.length();
            nearest = nearest.min(d);
        }
        assert!(nearest <= crate::squad::APPROACH_BODY + 0.3, "{nearest}");
        let at = sim.unit(bot).expect("bot").soldier.pos;
        let st = sim.unit(bot).expect("bot").ai;
        assert!(st.route().is_none(), "no leg beside the leader");
        let lp = sim.unit(leader).expect("leader").soldier.pos;
        let d = (at - lp).length();
        assert!((0.5..=4.3).contains(&d), "stands {d} m from him");
        for _ in 0..90 {
            sim.step(&[PadState::default()]);
        }
        let moved = (sim.unit(bot).expect("bot").soldier.pos - at).length();
        assert!(moved <= 0.05, "it stands: moved {moved} m in 3 s");
        assert_eq!(sim.unit(bot).expect("bot").ai.steer, SteerPick::Idle);
    }

    /// RE §4.3: a standing follower stays while the leader is within 9 m and
    /// follows again past it; it trails a walking leader.
    #[test]
    fn a_standing_follower_follows_again_past_nine_metres() {
        let (mut sim, leader, bot) = follow_sim(Vec3::new(4.0, 0.0, 0.0));
        sim.issue_command(0, crate::squad::CommandKind::Follow(leader));
        for _ in 0..150 {
            sim.step(&[PadState::default()]);
        }
        let at = sim.unit(bot).expect("bot").soldier.pos;
        // 6 m from the bot: inside 9 m, it stays.
        let near = at + Vec3::new(0.0, 0.0, 6.0);
        for _ in 0..60 {
            sim.unit_mut(leader).expect("leader").soldier.pos = near;
            sim.step(&[PadState::default()]);
        }
        let moved = (sim.unit(bot).expect("bot").soldier.pos - at).length();
        assert!(moved <= 0.05, "the leader is inside 9 m: moved {moved}");
        // 15 m: it follows, and stands again within reach of him.
        let far = at + Vec3::new(0.0, 0.0, 15.0);
        for _ in 0..240 {
            sim.unit_mut(leader).expect("leader").soldier.pos = far;
            sim.step(&[PadState::default()]);
        }
        let d = (sim.unit(bot).expect("bot").soldier.pos - far).length();
        assert!(d <= 5.0, "it closed on him again: {d} m");
        assert!(sim.unit(bot).expect("bot").command.is_some());
    }

    /// RE §4.2: a holder does not move at all, whatever its leader does
    /// inside 20 m; with him beyond 20 m it is released after 10 s of thinks.
    #[test]
    fn a_holder_stands_still_and_is_released_ten_seconds_after_the_leader_leaves() {
        let (mut sim, leader, bot) = follow_sim(Vec3::new(3.0, 0.0, 0.0));
        let spot = sim.unit(bot).expect("bot").soldier.pos;
        assert_eq!(
            sim.issue_command(0, crate::squad::CommandKind::Hold(Vec3::ZERO)),
            1
        );
        assert_eq!(
            sim.unit(bot).expect("bot").command.map(|c| c.kind),
            Some(crate::squad::CommandKind::Hold(spot)),
            "the spot is the bot's own position"
        );
        for _ in 0..crate::squad::LEAVE_TICKS as u64 + THINK_PERIOD * 2 {
            sim.unit_mut(leader).expect("leader").soldier.pos = Vec3::new(-16.0, 0.0, 0.0);
            sim.step(&[PadState::default()]);
            let u = sim.unit(bot).expect("bot");
            assert!(u.command.is_some(), "the hold stands");
            assert!(u.ai.route().is_none(), "a hold is never a leg");
            assert!((u.soldier.pos - spot).length() <= 0.3, "it moved");
        }
        assert_eq!(sim.nav_stats.releases, 0);
        // The leader passes 20 m and stays there.
        let left = sim.tick;
        let mut released_at = None;
        for _ in 0..2000u64 {
            sim.unit_mut(leader).expect("leader").soldier.pos = Vec3::new(-18.0, 0.0, 0.0);
            sim.step(&[PadState::default()]);
            if sim.unit(bot).expect("bot").command.is_none() {
                released_at = Some(sim.tick - left);
                break;
            }
            assert!((sim.unit(bot).expect("bot").soldier.pos - spot).length() <= 0.3);
        }
        let ticks = released_at.expect("the holder was released");
        let secs = ticks as f32 / crate::TICK_HZ as f32;
        assert!((9.0..=11.0).contains(&secs), "released after {secs} s");
        assert_eq!(sim.unit(bot).expect("bot").ai.cmd, None);
        assert_eq!(sim.nav_stats.releases, 1);
    }
}
