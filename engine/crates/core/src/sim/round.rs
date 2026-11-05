//! The command posts and how the round ends: [`Sim::add_cp`] registers a
//! post, and `update_cps` — step 6 — counts each post's occupants, applies
//! the conquest bleed and D12's Elite team handicap, sheds excess AI after
//! mission death drains, and decides the outcome (a team at 0 tickets,
//! domination, or the time limit). The capture, defeat, handicap and time-limit
//! tests live here.

use super::state::{Event, Sim, OUTCOME_DRAW};
use crate::cp::{self, CommandPost, CpChange, CpDef};
use crate::TICK_HZ;

/// The Instant Action reinforcement option (`con_mult`, 10..500 %, default
/// 100) applied to one team's authored count: BF2's `SetReinforcementCount`
/// scales each team's OWN count in integers, `count * pct / 100`
/// (`analysis/re-time-limit.md` §5 [V][O], decomp 32452).
pub fn scale_reinforcements(count: u32, pct: u32) -> u32 {
    (u64::from(count) * u64::from(pct) / 100) as u32
}

impl Sim {
    /// The domination countdown the HUD shows (issue #33,
    /// `analysis/re-round-end.md` §1.2): while one team holds every post,
    /// (that team, seconds left of the 20 s), the winner's side reading
    /// "Victory In" and the other "Defeat In"; through BF2's 1 s step after
    /// 0 it reads 0. None while no team holds every post, or once the round
    /// is over.
    pub fn domination_left(&self) -> Option<(u8, f32)> {
        if self.outcome.is_some() {
            return None;
        }
        self.domination.map(|(t, secs)| {
            (
                t,
                (crate::battle::BF2_DOMINATION_DEFEAT_SECS - secs).max(0.0),
            )
        })
    }

    /// Register a command post with its starting owner (0-based team, None =
    /// neutral). Returns its index.
    /// Add a post without [`Sim::resolve_posts`], which must follow before the
    /// sim runs.
    pub fn add_cp_unresolved(&mut self, def: CpDef, owner: Option<u8>) -> usize {
        let mut cp = CommandPost::new(def, owner);
        // Resolve the post's planning hub once: `locate` is O(hubs + arcs) and
        // a capture centre never moves. Requires `nav` to be set first —
        // `WalkSim::with_teams` does that before `setup_conquest`.
        cp.nav_hub = self
            .nav
            .as_ref()
            .and_then(|g| g.hub_for(cp.def.capture.center, crate::nav::Mover::SOLDIER));
        self.cps.push(cp);
        self.cps.len() - 1
    }

    /// Add a post and walk everything that stands on the posts again.
    pub fn add_cp(&mut self, def: CpDef, owner: Option<u8>) -> usize {
        let at = self.add_cp_unresolved(def, owner);
        self.resolve_posts();
        at
    }

    /// What stands on the posts, walked from all of them: the playable level
    /// (`sim/bounds.rs`), the spawn seats and the flank table. Each of the
    /// three starts over from the posts, so a whole map's posts need it once,
    /// after the last ([`Sim::add_cp_unresolved`]): the walk is most of a
    /// map's loading.
    pub fn resolve_posts(&mut self) {
        self.resolve_nav_floors();
        self.resolve_spawn_seats();
        self.build_flank();
    }

    /// Card FLNK: (re)build the flank table between the posts' hubs
    /// (`nav::flank`), each route's share drawn from the sim's seed - the
    /// generator's state, read and not advanced, so no draw moves.
    pub fn build_flank(&mut self) {
        let posts: Vec<u16> = self.cps.iter().filter_map(|c| c.nav_hub).collect();
        let seed = self.rng.state();
        self.flank = self
            .nav
            .as_ref()
            .map(|g| g.flank_table(&posts, seed))
            .unwrap_or_default();
    }

    /// D12's team handicap: at Elite, a team that HAS a human whose
    /// opponent has none bleeds `bleed_handicap` times faster. A host option
    /// (`team_handicap_on`), inert when both sides have a human or neither.
    ///
    /// OURS, PROVISIONAL — the PS2 has no such lever
    /// (`analysis/re-ai-difficulty.md` §9.2); BF2's ±4 rubber band and a
    /// per-team `SetAIDifficulty` delta are deliberately NOT built (design:
    /// symmetric competence). The "reinforcement bias" the design names has
    /// no number and is an open lead decision.
    ///
    /// Must run AFTER [`cp::apply_bleed`] (which OVERWRITES `bleed_per_sec`)
    /// and before [`Battle::tick_bleed`] spends it. Dead units keep their
    /// team, so the test is `is_player` over `units`, not `alive_players`.
    fn apply_team_handicap(&mut self) {
        if !self.team_handicap_on {
            return;
        }
        let mult = self.difficulty.tier().bleed_handicap;
        if mult == 1.0 {
            return;
        }
        let teams = self.battle.teams.len();
        let mut has_human = vec![false; teams];
        for u in &self.units {
            if u.is_human() && (u.team as usize) < teams {
                has_human[u.team as usize] = true;
            }
        }
        // Two-team rule (the sim's `battle` is two teams; `cp::apply_bleed`
        // is the same shape): the opponent of team t is 1 - t.
        for t in 0..teams.min(2) {
            if has_human[t] && !has_human[1 - t] {
                self.battle.teams[t].bleed_per_sec *= mult;
            }
        }
    }

    /// Whether unit `ui` counts for post `ci`'s capture now: an alive
    /// CapturePosts soldier of a battle team or independent hostile native on
    /// foot, tested as a POINT at the entity origin (report §4). The capture tick's rule; the HUD's capture
    /// icon (card CPBAR) asks the same. A rider (a vehicle's or a turret's
    /// seat) is not in the PS2's scan (card WCREW: in BF1 RAM, 73 of 73 post
    /// rows with a seated soldier in the region count only those on foot;
    /// BF2 not sampled). Card MAIS: nor a bot of a team the mission bars
    /// from the post (`AICanCaptureCP`, [`crate::cp::CommandPost::ai_no_capture`]).
    // Independent native affiliation is its own index (BF1 FUN_002ad490:
    // 238511-238527). Friendly-native affiliation remains separate scope;
    // retain its existing capture exclusion here.
    pub fn counts_in_cp(&self, ui: usize, ci: usize) -> bool {
        let u = &self.units[ui];
        self.deathmatch.is_none()
            && u.alive
            && u.capture_posts
            && u.mount.is_none()
            && ((u.team as usize) < self.battle.teams.len()
                || (u.team < 8
                    && self.native_team_index(u.team).is_some()
                    && self.hostile(u.team, 0)
                    && self.hostile(u.team, 1)))
            && !(u.controller == crate::unit::Controller::Ai && self.cps[ci].ai_no_capture(u.team))
            && self.cps[ci].def.capture.contains(u.soldier.pos)
    }

    /// Per team, the units that count inside post `ci`'s capture region
    /// ([`Self::counts_in_cp`]): what [`CommandPost::update`] is fed.
    pub fn cp_inside(&self, ci: usize) -> [u32; 8] {
        let mut inside = [0u32; 8];
        for k in 0..self.units.len() {
            if self.counts_in_cp(k, ci) {
                inside[self.units[k].team as usize] += 1;
            }
        }
        inside
    }

    pub(super) fn update_cps(&mut self, dt: f32) {
        if self.cps.is_empty() {
            return;
        }
        let teams = self.battle.teams.len();
        for ci in 0..self.cps.len() {
            let inside = self.cp_inside(ci);
            match self.cps[ci].update(dt, &inside) {
                CpChange::None => {}
                CpChange::Saved { team } => self.events.push(Event::Saved {
                    cp: ci as u32,
                    team,
                }),
                CpChange::Captured { by } => self.events.push(Event::Captured {
                    cp: ci as u32,
                    team: by,
                }),
                CpChange::Neutralized { by } => {
                    self.events.push(Event::Neutralized { cp: ci as u32, by })
                }
            }
        }
        if self.deathmatch.is_some() {
            return;
        }
        // BF2 conquest bleed (strict CP-value majority, clamped at alive).
        cp::apply_bleed(&mut self.battle, &self.cps);
        self.apply_team_handicap();
        let shed = self.battle.tick_bleed(dt);
        for (t, &n) in shed.iter().enumerate() {
            if n > 0 {
                self.shed_ai(t as u8, n);
            }
        }
        if self.outcome.is_none() {
            // BF2 Objective.lua OnTicketCountChange: count <= 0 → MissionDefeat.
            for t in 0..teams {
                if self.battle.teams[t].tickets() == 0 {
                    self.outcome = Some(if t == 0 { 1 } else { 0 });
                    self.events.push(Event::TeamDefeated { team: t as u8 });
                }
            }
            if let Some(loser) = cp::tick_domination(&self.cps, &mut self.domination, dt, 2) {
                if self.outcome.is_none() {
                    self.outcome = Some(if loser == 0 { 1 } else { 0 });
                    self.events.push(Event::TeamDefeated { team: loser });
                }
            }
            self.check_time_limit();
        }
    }

    /// The conquest time limit (queue TLO): once `time_limit` seconds of sim
    /// clock have run — this is the step that makes `tick` reach
    /// `limit × TICK_HZ` — BF2's `ObjectiveConquest:GameOptionsTimeLimitUp`
    /// gives the round to the team with MORE reinforcements, and equal counts
    /// to both ([`OUTCOME_DRAW`]) (`analysis/re-time-limit.md` §1 hop 9
    /// [V][O]). Runs after the 0-ticket and domination rules, only while the
    /// round is undecided; Off (`None`) never ends a round.
    fn check_time_limit(&mut self) {
        let Some(limit) = self.time_limit else {
            return;
        };
        if self.outcome.is_some() || self.tick + 1 < u64::from(limit) * u64::from(TICK_HZ) {
            return;
        }
        let t = [
            self.battle.teams[0].tickets(),
            self.battle.teams[1].tickets(),
        ];
        let winner = match t[0].cmp(&t[1]) {
            std::cmp::Ordering::Greater => 0,
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Equal => OUTCOME_DRAW,
        };
        self.outcome = Some(winner);
        self.events.push(Event::TimeUp { winner });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::Battle;
    use crate::sim::tests::{ai, fire, flat_ground, sphere_cp};
    use crate::sim::PadState;
    use crate::skill::Difficulty;
    use crate::unit::{Controller, UnitSpec};
    use glam::Vec3;

    /// Issue #33 (`analysis/re-round-end.md` §1.1-1.2): one team holding every
    /// post counts 20 s down - the HUD's Victory In / Defeat In - and the
    /// round ends 1 s after 0 (BF2's `dittyTimer`); a neutral post stops it.
    #[test]
    fn the_domination_countdown_reads_20_s_and_the_round_ends_at_21() {
        let mut s = Sim::empty(3);
        s.heightfield = Some(flat_ground());
        s.battle = Battle::new([150, 150]);
        for (i, x) in [0.0f32, 60.0].into_iter().enumerate() {
            s.add_cp(
                sphere_cp(&format!("cp{i}"), Vec3::new(x, 0.0, -200.0), 8.0, false),
                Some(0),
            );
        }
        let mut left = Vec::new();
        let mut ended = None;
        for t in 0..(30 * 25) {
            s.step(&[PadState::default()]);
            if t % 150 == 149 {
                left.push(s.domination_left().map(|(w, secs)| (w, secs.round())));
            }
            if s.outcome.is_some() && ended.is_none() {
                ended = Some(s.tick as f32 / 30.0);
            }
        }
        assert_eq!(left[0], Some((0, 15.0)));
        assert_eq!(left[2], Some((0, 5.0)));
        let ended = ended.expect("the round ends");
        assert!((ended - 21.0).abs() < 0.1, "ended at {ended} s");
        assert_eq!(s.outcome, Some(0));
        assert_eq!(s.domination_left(), None, "over: no countdown");
        // A neutral post stops it.
        let mut s = Sim::empty(3);
        s.add_cp(
            sphere_cp("cp0", Vec3::new(0.0, 0.0, -200.0), 8.0, false),
            Some(0),
        );
        s.add_cp(
            sphere_cp("cp1", Vec3::new(60.0, 0.0, -200.0), 8.0, false),
            None,
        );
        s.step(&[PadState::default()]);
        assert_eq!(s.domination_left(), None);
    }

    /// An AI walks to the nearest non-friendly post and captures it.
    #[test]
    fn ai_walks_to_neutral_cp_and_captures_it() {
        let mut s = Sim::empty(3);
        s.heightfield = Some(flat_ground());
        s.add_cp(
            sphere_cp("cp1", Vec3::new(0.0, 0.0, -40.0), 8.0, false),
            None,
        );
        let h = s.spawn(&ai(0, Vec3::ZERO, 1.0)).unwrap(); // facing off-axis
        let mut captured_tick = None;
        for _ in 0..(30 * 30) {
            s.step(&[PadState::default()]);
            if s.events
                .iter()
                .any(|e| matches!(e, Event::Captured { team: 0, .. }))
            {
                captured_tick = Some(s.tick);
                break;
            }
        }
        let t = captured_tick.expect("captured within 30 s") as f32 / 30.0;
        // ~32 m walk at 9 m/s (+turn) ≈ 4-5 s, then CaptureTime 10 s.
        assert!(t > 13.0 && t < 20.0, "captured at {t} s");
        assert_eq!(s.cps[0].owner, Some(0));
        let u = s.unit(h).unwrap();
        assert!(s.cps[0].def.capture.contains(u.soldier.pos));
        assert!(u.soldier.vel.length() < 0.5, "holds inside the post");
        assert_eq!(s.spawn_points(0).len(), 2);
        assert!(s.spawn_points(1).is_empty());
    }

    /// Card WCREW: a rider inside a post's region does not count (the PS2's
    /// scan holds only soldiers on foot), so an enemy parked in a vehicle on
    /// a neutral post neither contests it nor stops it being captured - BF1
    /// Geonosis seed 3 never ended while an AT-TE's pilot sat in cp2.
    #[test]
    fn a_rider_in_a_post_neither_counts_nor_contests() {
        use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
        use crate::weapon::HealthType;
        let mut s = Sim::empty(5);
        s.heightfield = Some(flat_ground());
        s.add_cp(
            sphere_cp("cp", Vec3::new(0.0, 0.0, -20.0), 8.0, false),
            None,
        );
        let idle = |team, pos| UnitSpec {
            controller: Controller::Idle,
            ..UnitSpec::trooper(team, pos)
        };
        s.spawn(&idle(0, Vec3::new(2.0, 0.0, -20.0))).unwrap();
        let rider = s.spawn(&idle(1, Vec3::new(-2.0, 0.0, -20.0))).unwrap();
        let v = s.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: Some(1),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::new(-2.0, 1.0, -20.0),
            yaw: 0.0,
            max_health: 900.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        // On foot both count: the post is contested.
        assert_eq!(s.cp_inside(0), [1, 1, 0, 0, 0, 0, 0, 0]);
        assert!(s.seat_unit(rider, v, 0));
        let u = s.unit(rider).unwrap();
        assert!(
            s.cps[0].def.capture.contains(u.soldier.pos),
            "{:?}",
            u.soldier.pos
        );
        assert_eq!(
            s.cp_inside(0),
            [1, 0, 0, 0, 0, 0, 0, 0],
            "the rider is not counted"
        );
        for _ in 0..(12 * 30) {
            s.step(&[]);
        }
        assert_eq!(
            s.cps[0].owner,
            Some(0),
            "team 0 captured it beside the rider"
        );
    }

    /// Card MAIS: `AICanCaptureCP(post, team, false)` - a bot of the barred
    /// team in the region does not count (nor contest); its human does, and
    /// the other team's bots do.
    #[test]
    fn a_post_barred_to_a_teams_bots_does_not_count_them() {
        let mut s = Sim::empty(5);
        s.heightfield = Some(flat_ground());
        s.add_cp(
            sphere_cp("cp", Vec3::new(0.0, 0.0, -20.0), 8.0, false),
            None,
        );
        let bot = |team, pos| UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(team, pos)
        };
        s.spawn(&bot(0, Vec3::new(2.0, 0.0, -20.0))).unwrap();
        let b1 = s.spawn(&bot(1, Vec3::new(-2.0, 0.0, -20.0))).unwrap();
        assert_eq!(s.cp_inside(0), [1, 1, 0, 0, 0, 0, 0, 0]);
        s.cps[0].ai_capture_ban = 1 << 1;
        assert_eq!(
            s.cp_inside(0),
            [1, 0, 0, 0, 0, 0, 0, 0],
            "team 1's bot is barred"
        );
        s.unit_mut(b1).unwrap().controller = Controller::Player { slot: 1 };
        assert_eq!(s.cp_inside(0), [1, 1, 0, 0, 0, 0, 0, 0], "its human is not");
    }

    /// BF2 rule: a team at 0 tickets is defeated at once; AI never respawn
    /// past the pool (no-overspawn), and bleed shedding keeps the invariant.
    #[test]
    fn defeat_at_zero_tickets_and_no_overspawn() {
        let mut s = Sim::empty(4);
        s.battle = Battle::new([50, 1]);
        s.add_cp(
            sphere_cp("cp", Vec3::new(30.0, 0.0, -30.0), 8.0, false),
            Some(1),
        );
        s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        s.spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0)))
            .unwrap();
        // A second AI is refused: 1 alive == 1 ticket.
        assert!(s
            .spawn(&UnitSpec::trooper(1, Vec3::new(3.0, 0.0, -20.0)))
            .is_none());
        for _ in 0..90 {
            s.step(&[fire()]);
            if s.outcome.is_some() {
                break;
            }
        }
        assert_eq!(s.outcome, Some(0), "team 1 lost its last ticket");
        assert!(s
            .events
            .iter()
            .any(|e| matches!(e, Event::TeamDefeated { team: 1 })));
        assert_eq!(s.battle.teams[1].tickets(), 0);
        for t in &s.battle.teams {
            assert!(t.invariant_holds());
        }
    }

    /// BLEED: exercise the actual CP clamp and Elite handicap through fixed
    /// sim ticks, with legal admissions at the last free ticket.
    #[test]
    fn conquest_bleed_stops_before_shedding_live_ai() {
        for team in 0..2u8 {
            for difficulty in Difficulty::ALL {
                for handicap in [false, true] {
                    for human in [false, true] {
                        let mut s = Sim::empty(4);
                        s.heightfield = Some(flat_ground());
                        s.battle = Battle::new([4, 4]);
                        s.difficulty = difficulty;
                        s.team_handicap_on = handicap;
                        // The enemy owns 2/3 of the bleed value; neither
                        // side dominates and the units cannot capture here.
                        for i in 0..3 {
                            s.add_cp(
                                sphere_cp(
                                    "cp",
                                    Vec3::new(100.0 + i as f32 * 50.0, 0.0, 0.0),
                                    8.0,
                                    false,
                                ),
                                Some(if i == 0 { team } else { 1 - team }),
                            );
                        }
                        for i in 0..3 {
                            let pos = Vec3::new(i as f32 * 3.0, 0.0, 0.0);
                            let mut spec = if human && i == 0 {
                                UnitSpec::player(team, pos)
                            } else {
                                UnitSpec::trooper(team, pos)
                            };
                            spec.controller = Controller::Idle;
                            s.spawn(&spec).unwrap();
                        }
                        for tick in 0..120 {
                            s.step(&[PadState::default()]);
                            let t = &s.battle.teams[team as usize];
                            assert_eq!(t.alive(), 3);
                            assert_eq!(t.tickets(), 3);
                            assert!(t.reinforcements > 3.0 && t.reinforcements < 4.0);
                            assert!(t.invariant_holds());
                            assert!(s.units.iter().all(|u| u.alive));
                            assert!(!s.events.iter().any(|e| matches!(e, Event::Despawn { .. })));
                            assert_eq!(t.bleed_per_sec > 0.0, tick == 0);
                        }
                    }
                }
            }
        }
    }

    /// BLEED: Death Star's extra drain reaches the repair loop from valid
    /// spawns, even with no CP majority. Keep both counters and units in sync.
    #[test]
    fn mission_death_drain_sheds_ai_through_step_without_cp_bleed() {
        for extra in [0, 1] {
            let mut s = Sim::empty(4);
            s.heightfield = Some(flat_ground());
            s.battle = Battle::new([3, 100]);
            s.battle.death_drain[0][0] = extra;
            for team in 0..2 {
                s.add_cp(
                    sphere_cp(
                        "cp",
                        Vec3::new(100.0 + team as f32 * 50.0, 0.0, 0.0),
                        8.0,
                        false,
                    ),
                    Some(team),
                );
            }
            let player = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
            let survivor = s
                .spawn(&UnitSpec::trooper(0, Vec3::new(3.0, 0.0, 0.0)))
                .unwrap();
            let victim = s
                .spawn(&UnitSpec::trooper(0, Vec3::new(6.0, 0.0, 0.0)))
                .unwrap();
            assert!(s.battle.teams[0].invariant_holds());
            assert_eq!(s.battle.teams[0].alive(), 3);
            s.death_regions.push(crate::region::Region::sphere(
                s.unit(victim).unwrap().soldier.pos,
                0.5,
            ));
            s.step(&[PadState::default()]);
            let t = &s.battle.teams[0];
            assert_eq!(t.bleed_per_sec, 0.0);
            assert_eq!(t.tickets(), 2 - extra, "shedding spends no ticket");
            assert_eq!(t.alive_players, 1);
            assert_eq!(t.alive_ai, 1 - extra);
            assert!(t.invariant_holds());
            assert!(s.unit(player).unwrap().alive, "players are never shed");
            let survivor_state = &s.units[survivor.index as usize];
            assert_eq!(survivor_state.gen, survivor.gen);
            assert_eq!(survivor_state.alive, extra == 0);
            if extra > 0 {
                assert_eq!(survivor_state.respawn_at, None);
            }
            let deaths: Vec<_> = s
                .events
                .iter()
                .filter_map(|e| match e {
                    Event::EnvDeath {
                        unit,
                        cause: crate::sim::DeathCause::DeathRegion,
                    } => Some(*unit),
                    _ => None,
                })
                .collect();
            assert_eq!(deaths, vec![victim]);
            let despawned: Vec<_> = s
                .events
                .iter()
                .filter_map(|e| match e {
                    Event::Despawn { unit } => Some(*unit),
                    _ => None,
                })
                .collect();
            assert_eq!(despawned, if extra > 0 { vec![survivor] } else { vec![] });
            assert_eq!(s.outcome, None);
        }
    }

    /// D12 (b): at Elite the team that HAS a human, facing a side that has
    /// none, bleeds `bleed_handicap` times faster. Inert at Normal, inert
    /// with no human anywhere, inert with the host option off.
    #[test]
    fn the_elite_team_handicap_bleeds_only_the_lone_humans_side() {
        // One post, owned by team 1 → team 0 is the bleeding side, and team 0
        // is where the human is put.
        fn bleed(d: Difficulty, on: bool, human: bool) -> f32 {
            let mut s = Sim::empty(4);
            s.difficulty = d;
            s.team_handicap_on = on;
            s.battle = Battle::new([50, 50]);
            s.add_cp(
                sphere_cp("cp", Vec3::new(30.0, 0.0, -30.0), 8.0, false),
                Some(1),
            );
            let spec = if human {
                UnitSpec {
                    controller: Controller::Idle,
                    ..UnitSpec::player(0, Vec3::ZERO)
                }
            } else {
                UnitSpec {
                    controller: Controller::Idle,
                    ..UnitSpec::trooper(0, Vec3::ZERO)
                }
            };
            s.spawn(&spec).unwrap();
            s.spawn(&UnitSpec {
                controller: Controller::Idle,
                ..UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0))
            })
            .unwrap();
            s.step(&[PadState::default()]);
            s.battle.teams[0].bleed_per_sec
        }

        let base = bleed(Difficulty::Normal, true, true);
        assert!(base > 0.0, "team 0 bleeds at all: {base}");
        let h = Difficulty::Elite.tier().bleed_handicap;
        assert!((h - 1.25).abs() < 1e-6, "the tier's placeholder: {h}");
        assert!(
            (bleed(Difficulty::Elite, true, true) - base * h).abs() < 1e-6,
            "{}",
            bleed(Difficulty::Elite, true, true)
        );
        // Bots only on both sides, and the option off: the plain rate.
        assert!((bleed(Difficulty::Elite, true, false) - base).abs() < 1e-6);
        assert!((bleed(Difficulty::Elite, false, true) - base).abs() < 1e-6);
    }
}
