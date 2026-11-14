//! Navigation tick diagnostics, neutral AI state and state serialization.

use super::{
    AiState, ArmState, Behaviour, FightState, FlankState, MoverState, NavState, RoleState, Route,
    SnipeBook, SteerPick, TposState, Vec3, WalkAsk, ERR_NO_TARGET,
};
use crate::fmath::FloatExt;

#[cfg(doc)]
use super::StallState;

/// What one `think` had to fall back on — folded into `Sim::nav_stats`, which
/// is a diagnostic OUTPUT and not hashed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NavTick {
    /// Hops answered by `route_bfs` because the table had no entry.
    pub bfs: u32,
    /// Hops neither the table nor BFS could answer.
    pub straight: u32,
    /// Card FLNK: hops taken from a flank route (on it or joining it).
    pub flank: u32,
    /// This unit's current run of ticks without progress.
    pub stuck_run: u16,
    /// Legs planned this tick whose straight line crossed a barrier and got a
    /// corner path instead.
    pub detoured: u32,
    /// 1 when the unit wanted to move and the way ahead was NOT clear.
    pub blocked: u32,
    /// Legs whose chosen next hub had no floor at the unit's own level and
    /// were rejected (Phase A.2, `hub_has_floor`).
    pub level_rejected: u32,
    /// Of those, the ones where EVERY alternative also failed and the unit
    /// fell back to the straight line.
    pub level_unrouted: u32,
    /// 1 when the forward wall SWEEP found a wall this tick.
    pub sweep_hits: u32,
    /// 1 when the commander handed this unit an objective the graph could not
    /// route to from where it stands (Phase B). The assignment is LEFT alone —
    /// the next plan sees the same unreachability and drops it — and this is
    /// the count that says how often that happens.
    pub unroutable: u32,
    /// This unit's current [`AiState::unreachable_dwell`].
    pub dwell: u16,
    /// Contacts this unit's perception held after this think (Phase C).
    pub contacts: u8,
    /// Records this think made by HEARING rather than sight.
    pub heard: u8,
    /// Card TEAM: records this think made or refreshed from what a friend
    /// near by sees (`perception::tell`).
    pub told: u8,
    /// 1 when this think ENDED a squad order (brief D6/D9): the leader is
    /// gone, or the follower has been beyond [`crate::squad::LEAVE_DIST`]
    /// of its leader — or the holder of its anchor — for
    /// [`crate::squad::LEAVE_TICKS`]. `think` takes `&Sim` and cannot
    /// clear the order itself, so it reports it and `Sim::step`'s write-back
    /// does it, in the same fixed order as every other write-back.
    pub released: u32,
    /// Card WAI2, a boarding claim this tick: [`NavTick::CLAIM_NEW`] made,
    /// [`NavTick::CLAIM_GIVEUP`] dropped on its clock, [`NavTick::CLAIM_LOST`]
    /// dropped because another bot has the vehicle; 0 otherwise.
    pub claim: u8,
    /// 1 when this unit walked its RETURN this tick (card BNDS): off the
    /// playable level and heading back for it.
    pub lost: u32,
    /// 1 when this think gave the unit up (card BNDS): off the level for
    /// [`crate::ai::LOST_RECYCLE`] with no way back. `Sim::step`'s write-back
    /// kills it ([`crate::sim::DeathCause::Lost`]), like every other
    /// write-back.
    pub recycle: u32,
    /// 1 when this think gave the unit up for making no net progress (card
    /// RCYC, [`StallState`]): the write-back kills it as
    /// [`crate::sim::DeathCause::Stalled`].
    pub stalled: u32,
    /// Card UNSN: in place of `stalled`, the spot the bot is moved to
    /// unseen (`f32` bits: the tick is `Eq`); `Sim::step`'s write-back
    /// moves it ([`crate::ai::unseen`]).
    pub relocate: Option<[u32; 3]>,
    /// Card TOKN: 1 when this unit fired beside a human it holds no token
    /// of.
    pub tokn_off: u8,
    /// 1 when this think withheld a shot it was on target for, because a
    /// friend stood in the line of fire (card FFLN).
    pub line_held: u32,
    /// Card TPOS: this think asked the Tactician (1), and a candidate with
    /// cover from the target passed its filters (2) - B12's ceiling.
    pub tpos_query: u8,
    /// Card TPOS: what this think's Tactician did, one bit per event
    /// (`crate::ai::tactic::EV_*`: the query's answer, why a position was
    /// left, how a walk to one ended) - the fight-time split's counters.
    pub tpos_ev: u16,
    /// Card TPOS: this think's query's candidates with cover from the
    /// target, stage by stage (the sweep's `tfunnel` line: within the box,
    /// on the storey, kept by the cap, each filter's rejects, passed, the
    /// drop test's and the confirming ray's rejects, scored with no pick).
    /// ([14], card HYST: the covered candidates the closing filter took.)
    pub tpos_funnel: [u8; 15],
    /// Card HYST: [`crate::ai::tactic::HY_FLIP`] / [`crate::ai::tactic::HY_KEPT`].
    pub tpos_hyst: u8,
    /// Card HYST: this tick's think was woken early, and by what
    /// ([`crate::ai::wake`]'s bits); 0 = none (a scheduled think or none).
    pub wake: u8,
    /// Card UCALL: 1 when this think's tracker TOOK a target (the first, or
    /// another in place of the held one) - BF1's `AcquiredTargetSound` call.
    pub acquired: u8,
    /// Card UCALL: 1 when this think's hearing turned the unit to a heard
    /// enemy - BF1's `HeardEnemySound` call.
    pub heard_turn: u8,
    /// Card UTALK: [`NavTick::SET_OUT`] when this tick the bot began walking
    /// to its post from anything else, [`NavTick::ARRIVED`] when its walk
    /// reached the post - the trooper agent's two points where BF1's AI says
    /// "follow me" / "move out" (re-squad-orders.md 5).
    pub trooper: u8,
    /// Card TURR: this think's turret business - 1..=5 the exit rule a
    /// gunner left its turret by ([`crate::ai::man`]), [`NavTick::MAN_UNREACHED`]
    /// a claimed turret its walk did not reach; 0 otherwise.
    pub man: u8,
    /// Card WHY: the reason the chain gave for the behaviour it chose this
    /// tick ([`super::why`]; `Why::None` = the log derives it).
    pub why: super::why::Why,
    /// Card WHY: this tick's failed walk and what failed it, then what came
    /// of it (the goal given up, the floor walk, the recycle) — in order,
    /// `Why::None` past the last ([`NavTick::failed`])...
    pub fails: [super::why::Why; 4],
    /// ...and the nav arc the failed leg walked (card ARCM counts it against
    /// the team).
    pub fail_arc: Option<u16>,
    /// Card ARCM: the arc this think closed to the bot (the team counts it),
    /// and the legs drawn round a closed or shunned arc / kept for want of
    /// another way.
    pub closed_arc: Option<u16>,
    pub shun_round: u8,
    pub shun_kept: u8,
    /// Bot census: 1 when the think ran this tick (scheduled or woken).
    pub thought: u8,
    /// Bot census: the target this think, as `crate::ai::census::D1_*` bits
    /// (0 off a think, in a seat or in a space round).
    pub d1: u8,
    /// Bot census: the tier levers that worked this tick, as
    /// `crate::ai::census::LV_*` bits.
    pub levers: u32,
    /// Bot census: the wall time of this call, in nanoseconds. `Sim::step`
    /// writes it after the think returns; nothing in the think reads it.
    pub think_ns: u32,
}
impl NavTick {
    /// Card WHY: note a failed walk, or what came of one.
    pub fn failed(&mut self, why: super::why::Why) {
        if let Some(f) = self.fails.iter_mut().find(|f| **f == super::why::Why::None) {
            *f = why;
        }
    }

    /// The first failed walk this tick reported, if any.
    pub fn fail(&self) -> super::why::Why {
        self.fails[0]
    }
}
impl NavTick {
    pub const CLAIM_NEW: u8 = 1;
    pub const CLAIM_GIVEUP: u8 = 2;
    pub const CLAIM_LOST: u8 = 3;
    pub const SET_OUT: u8 = 1;
    pub const ARRIVED: u8 = 2;
    /// [`NavTick::man`]: a claimed turret the walk did not reach.
    pub const MAN_UNREACHED: u8 = 6;
}
impl Default for AiState {
    /// Neutral state with tap_gate enabled and flank NONE sentinels. The
    /// tracked-position error starts at the PS2's no-target floor (LEAD
    /// ADDENDUM 4a): a unit that has never tracked anything is in exactly the state of one that has been target-less for
    /// a while, and its first target does not begin with a perfect aim point.
    fn default() -> Self {
        Self {
            target: None,
            contacts: [None; crate::perception::CONTACTS],
            last_switch: 0,
            next_think: 0,
            last_think: 0,
            wake: 0,
            items: super::items::ItemSeek::default(),
            supply: crate::ai::supply::SupplyState::default(),
            melee: None,
            target_cp: None,
            squad: None,
            cmd: None,
            ride: None,
            pending_board: None,
            lost: None,
            hold: None,
            fight: FightState::default(),
            tpos: TposState::default(),
            want_stance: 0,
            wander: None,
            dodge_until: 0,
            flinch_until: 0,
            unreachable_dwell: 0,
            nav: NavState::default(),
            steer: SteerPick::default(),
            slide: 0,
            walk: WalkAsk::default(),
            strafe_until: 0,
            strafe_side: 0,
            stall: None,
            mv: MoverState::default(),
            jet: super::jet::JetState::default(),
            behaviour: Behaviour::None,
            step: crate::ai::StepKind::None,
            why: super::why::WhyLog::default(),
            arcm: super::arcm::ArcmState::default(),
            aim_dir: Vec3::ZERO,
            aim_vel: Vec3::ZERO,
            memory_blocked: false,
            err: ERR_NO_TARGET,
            err_pos: Vec3::ZERO,
            dispersion: None,
            burst: 0,
            pause_until: 0,
            // As an idle tick leaves it (BF2 `FUN_002baf70`).
            tap_gate: true,
            tap_until: 0,
            snipe_until: 0,
            tap_byte: 0,
            charge_until: 0,
            arm: ArmState::default(),
            snipe: SnipeBook::default(),
            fix: super::fix::FixState::default(),
            role: RoleState::default(),
            flank: FlankState::default(),
            droid_rest: 0,
        }
    }
}
impl AiState {
    /// The leg the navigator is currently following.
    pub fn route(&self) -> Option<Route> {
        self.nav.route()
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if let Some(melee) = &self.melee {
            melee.hash_into(eat);
        }
        self.items.hash_into(eat);
        self.supply.hash_into(eat);
        self.jet.hash_into(eat);
        eat(&self.target_cp.unwrap_or(u32::MAX).to_le_bytes());
        match self.target {
            Some(crate::perception::Foe::Unit(e)) => {
                eat(&[1]);
                eat(&e.index.to_le_bytes());
                eat(&e.gen.to_le_bytes());
            }
            Some(crate::perception::Foe::Droid(id)) => {
                eat(&[2]);
                eat(&id.to_le_bytes());
            }
            Some(crate::perception::Foe::Turret(id)) => {
                eat(&[3]);
                eat(&id.to_le_bytes());
            }
            None => eat(&[0]),
        }
        // Every slot, `None` a sentinel byte and never skipped: what a unit
        // remembers is state, and two units with the same target but
        // different memories will fight differently three seconds from now.
        for slot in &self.contacts {
            match slot {
                Some(c) => {
                    eat(&[1]);
                    c.hash_into(eat);
                }
                None => eat(&[0]),
            }
        }
        eat(&self.last_switch.to_le_bytes());
        self.nav.hash_into(eat);
        eat(&self.steer.bytes());
        eat(&[self.slide as u8]);
        match self.squad {
            Some(k) => {
                eat(&[1]);
                eat(&k.to_le_bytes());
            }
            None => eat(&[0]),
        }
        eat(&self.unreachable_dwell.to_le_bytes());
        eat(&[self.burst, self.strafe_side as u8, self.want_stance]);
        if let Some((at, hub)) = self.wander {
            for c in at.to_array() {
                eat(&c.hash_bits().to_le_bytes());
            }
            eat(&hub.to_le_bytes());
        }
        eat(&self.pause_until.to_le_bytes());
        eat(&self.charge_until.to_le_bytes());
        eat(&self.dodge_until.to_le_bytes());
        // Tagged and only once set: a round where no bot flinched hashes as
        // it did before the flinch existed (space maps never set it).
        if self.flinch_until != 0 {
            eat(b"FLINCH");
            eat(&self.flinch_until.to_le_bytes());
        }
        eat(&self.strafe_until.to_le_bytes());
        match self.dispersion {
            Some(d) => {
                eat(&[1]);
                eat(&d.hash_bits().to_le_bytes());
            }
            None => eat(&[0]),
        }
        for v in [
            self.aim_dir.x,
            self.aim_dir.y,
            self.aim_dir.z,
            self.aim_vel.x,
            self.aim_vel.y,
            self.aim_vel.z,
            self.err,
            self.err_pos.x,
            self.err_pos.y,
            self.err_pos.z,
        ] {
            eat(&v.hash_bits().to_le_bytes());
        }
        // Optional extensions add no bytes when absent, preserving command-free
        // replay pins (brief D6/addendum 3). Each extension carries a tag.
        if let Some(c) = self.cmd {
            c.hash_into(eat);
        }
        if let Some(r) = self.ride {
            r.hash_into(eat);
        }
        if let Some((owner, target)) = self.pending_board {
            eat(b"GBOARD");
            for v in [owner.index, owner.gen, target.index, target.gen] {
                eat(&v.to_le_bytes());
            }
        }
        if let Some(l) = self.lost {
            l.hash_into(eat);
        }
        if let Some(h) = self.hold {
            h.hash_into(eat);
        }
        if let Some(s) = self.stall {
            s.hash_into(eat);
        }
        if self.mv != MoverState::default() {
            self.mv.hash_into(eat);
        }
        if self.fight != FightState::default() {
            self.fight.hash_into(eat);
        }
        if self.tpos != TposState::default() {
            self.tpos.hash_into(eat);
        }
        // Card STTAP: the trigger state, eaten once a single-shot weapon has
        // moved it — like the optional states above, a bot that never held
        // one costs the hash nothing.
        if (
            self.tap_gate,
            self.tap_byte,
            self.tap_until,
            self.snipe_until,
        ) != (true, 0, 0, 0)
        {
            eat(&[u8::from(self.tap_gate), self.tap_byte as u8]);
            eat(&self.tap_until.to_le_bytes());
            eat(&self.snipe_until.to_le_bytes());
        }
        if self.arm != ArmState::default() {
            self.arm.hash_into(eat);
        }
        if self.memory_blocked {
            eat(b"MWALL");
        }
        if self.snipe != SnipeBook::default() {
            self.snipe.hash_into(eat);
        }
        if self.fix != super::fix::FixState::default() {
            self.fix.hash_into(eat);
        }
        if self.role != RoleState::default() {
            self.role.hash_into(eat);
        }
        if self.flank != FlankState::default() {
            self.flank.hash_into(eat);
        }
        if self.droid_rest != 0 {
            eat(b"DROIDAI");
            eat(&self.droid_rest.to_le_bytes());
        }
        if self.arcm != super::arcm::ArcmState::default() {
            self.arcm.hash_into(eat);
        }
    }
}
