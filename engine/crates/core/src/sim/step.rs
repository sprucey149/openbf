//! [`Sim::step`] itself — one fixed tick, in the order `sim.rs`'s module
//! header lists — and `separate_units`, its step 3b. The helpers the other
//! stages call live with the feature they serve (`spawn`, `input`, `combat`,
//! `round`). WGCAL/WGOUT publish BF1 wait-start and first routed departure calls after all
//! AI write-backs, before use/entry and movement. The movement (unit separation, the A.4 step, the wall pass),
//! stamina cheat, AI fire (D9's spread multiplier, the fairness rule) and a
//! human's shot (card AIMA, `re-aim-assist.md` §10.2: from the fire point at
//! what the aim ray hits, `Sim::aim_hit`) live here. Their tests are in
//! `step/tests.rs` and `step/tests/`.

use super::input::{buttons, PadState};
use super::state::{Call, DeathCause, Event, Sim, LETHAL_LANDING_SPEED};
use crate::ai;
use crate::collision::{Capsule, AIR_STEP, STEP_HEIGHT};
use crate::projectile::TERRAIN_RAY_STEP;
use crate::soldier::{GroundRules, Stance};
use crate::unit::Controller;
use crate::weapon::FireInput;
use crate::{TICK_DT, TICK_HZ};
use glam::Vec3;

const FLYER_OWN_DECAY: f32 = 0.5;

impl Sim {
    /// Push overlapping alive units apart in XZ (see step 3b). Deterministic:
    /// pairs visited in index order, a coincident pair separates along +X.
    fn separate_units(&mut self) {
        let n = self.units.len();
        for i in 0..n {
            if !self.units[i].alive || self.units[i].mount.is_some() {
                continue;
            }
            for j in i + 1..n {
                if !self.units[j].alive || self.units[j].mount.is_some() {
                    continue;
                }
                let (pi, pj) = (self.units[i].soldier.pos, self.units[j].soldier.pos);
                let min_d = self.units[i].separation_radius() + self.units[j].separation_radius();
                let d = Vec3::new(pj.x - pi.x, 0.0, pj.z - pi.z);
                let len = d.length();
                if len >= min_d {
                    continue;
                }
                let axis = if len > 1e-4 { d / len } else { Vec3::X };
                let (bi, bj) = (self.units[i].unpushed(), self.units[j].unpushed());
                let (si, sj) = match (bi, bj) {
                    (true, false) => (0.0, 1.0),
                    (false, true) => (1.0, 0.0),
                    _ => (0.5, 0.5),
                };
                let gap = min_d - len;
                let ni = pi - axis * (gap * si);
                let nj = pj + axis * (gap * sj);
                // A grounded unit follows the ground under its new footprint
                // only within a STEP of where it stands; a push over a
                // platform edge leaves its y alone so it FALLS next tick
                // (`Soldier::resolve_ground` finds no floor) instead of being
                // teleported straight down — measured on BF2 COR1: sixteen
                // AI spawned on one node pushed each other off the platform
                // and every one went 15 -> -300 in a single tick, grounded,
                // never landing, so no fall could ever kill it. Airborne
                // units keep their y as before.
                for (k, n) in [(i, ni), (j, nj)] {
                    let mut y = n.y;
                    if self.units[k].soldier.grounded {
                        let g = self.ground_under(n, false);
                        if (g - n.y).abs() <= STEP_HEIGHT {
                            y = g;
                        }
                    }
                    self.units[k].soldier.pos = Vec3::new(n.x, y, n.z);
                }
            }
        }
    }

    /// BF2: think each living unit's [`crate::unit::AimBlock`] - a bot's
    /// every 5th tick, phased by its index, a human's every tick (card
    /// CLSE2, `analysis/re-soldier-ai.md` 18.1). The decay and the floor are
    /// the skill of who shoots at the unit: the other side's bots' (OURS: the
    /// PS2 keeps a skill per team and moves it by auto-balance), its per-team
    /// tier (card ARVS) or its DTEAM row (`Sim::enemy_skill_of`).
    fn aim_blocks(&mut self) {
        if self.ground_rules != crate::soldier::GroundRules::Bf2 {
            return;
        }
        let skills = [0u8, 1, 2].map(|t| self.enemy_skill_of(t));
        let tick = self.tick;
        let space = self.space_assault.is_some();
        for (i, u) in self.units.iter_mut().enumerate() {
            let period = if u.is_human() {
                1
            } else {
                crate::unit::AimBlock::PERIOD
            };
            if !u.alive || !(tick + i as u64).is_multiple_of(period) {
                continue;
            }
            let draw = |k: u32| {
                let salt = ai::SALT_ERR_AXIS.wrapping_add(k);
                2.0 * ai::unit_unit(i as u32, tick, u.gen, salt) - 1.0
            };
            let jolt = Vec3::new(draw(0), draw(0x51ED_2701), draw(0xA3DA_4E03));
            let skill = &skills[usize::from(u.team.min(2))];
            let flown = u
                .mount
                .filter(|_| space)
                .and_then(|m| self.vehicles.get(m.vehicle.index as usize))
                .filter(|v| v.kind == crate::vehicle::VehicleKind::Flyer)
                .and_then(|v| {
                    self.flyer_params
                        .get(v.class as usize)
                        .and_then(Option::as_ref)
                        .filter(|p| p.bf2)
                        .map(|p| (v.vel, p.max_speed))
                });
            let (vel, top, own) = match flown {
                Some((vel, top)) => (vel, top, FLYER_OWN_DECAY),
                None => (u.soldier.vel, u.consts.max_speed, 1.0),
            };
            u.aim.think_scaled(
                period as f32 * TICK_DT,
                vel,
                top,
                skill.err_decay,
                own,
                skill.err_floor,
                jolt,
            );
        }
    }

    /// Advance one fixed tick. `pads` is indexed by PLAYER SLOT: the unit in
    /// slot `s` is driven by `pads[s]`. A slot past the end of the slice gets
    /// `PadState::default()` — a slot without a pad this tick simply idles,
    /// which is not an error (a splitscreen player who has not joined, a net
    /// peer whose packet has not arrived).
    pub fn step(&mut self, pads: &[PadState]) {
        let dt = TICK_DT;
        // The rays since the last step were not the sim's (the chase camera's).
        crate::collision::cost::take();
        self.events.clear();
        self.shot_flights.clear();
        // The hulls where their vehicles are (card VHULL): a caller may have
        // moved one, or installed a new world, since the last tick.
        self.sync_hulls();
        self.sync_tow_falls();
        // Existing hero lives drain before this tick's replacements/spawns.
        self.tick_hero_health(dt);
        // A press a prompt answered stays out of play until released.
        let eaten = self.eat_held_pads(pads);
        let pads = eaten.as_deref().unwrap_or(pads);
        let hero_pads = self.step_heroes(pads);
        let pads = hero_pads.as_deref().unwrap_or(pads);

        // Confirmed pause-menu Respawn runs inside the tick so its death
        // event reaches every normal observer (re-spawn-flow §7/§6).
        if self.outcome.is_none() {
            for (slot, pad) in pads.iter().enumerate() {
                if pad.pressed(buttons::RESPAWN) {
                    if let Some(Some(h)) = self.players.get(slot) {
                        self.kill_environment(*h, DeathCause::Suicide);
                    }
                }
            }
        }

        // 0a. The spawn screen's CLASS choice, before the respawn loop below
        //     can act on it.
        self.spawn_select(pads);
        if let Some(counts) = self.team_combatants {
            // A human may have switched teams on the spawn screens.
            self.set_combatants(counts);
        }

        // 0a'. Issue #28: a live human's use press at a friendly post opens
        //      the class screen; while it is up his soldier gets no pad.
        self.class_change_open(pads);
        let screen_pads = self.class_screen_pads(pads);
        let pads = screen_pads.as_deref().unwrap_or(pads);

        // 0. Respawns (respawn timer expired, then the team's spawn wave for
        //    a bot - a human skips it, card SSELA; AI additionally capped by the alive-AI cap: BF2 FUN_0034cea8
        //    waits 5 s when NumBots is reached).
        for w in &mut self.waves {
            w.tick(dt);
        }
        self.tick_native_waves(dt);
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.alive || u.respawn_at.is_none_or(|t| t > self.tick) {
                continue;
            }
            if u.team >= 2 {
                self.respawn_native(i);
                continue;
            }
            // A HUMAN waits for no spawn wave (BF2 `FUN_0034cea8` skips the
            // wave tests for a human, `B2:344186-344192`, card SSELA). Behind
            // the spawn screens its timer running out opens the spawn map
            // (when its team still has tickets, `B2:392963`), and the class
            // accept is what spawns it (`Sim::spawn_screen_input`).
            if u.is_player {
                match u.spec.player_slot.map(usize::from) {
                    Some(s) if self.spawn_screens => {
                        let team = u.team;
                        if self.spawn_stage.get(s) == Some(&super::spawn::SpawnStage::Accepted)
                            && self.battle.teams[usize::from(team.min(1))].tickets() > 0
                        {
                            self.open_spawn_map(s, team);
                        }
                    }
                    _ => self.respawn_slot(i),
                }
                continue;
            }
            // A bot asks for its post when the timer runs out and keeps the
            // answer through the waits below (card BSPN).
            self.keep_or_draw_spawn_post(i);
            let u = &self.units[i];
            if let Some(slot) = u.spawn_slot {
                if !self.waves[usize::from(u.team).min(1)].ready(slot) {
                    continue;
                }
            }
            {
                let team = u.team;
                let alive_ai = self
                    .units
                    .iter()
                    .filter(|o| o.alive && !o.is_player && o.team == team)
                    .count() as u32;
                if alive_ai >= self.ai_limit(team) {
                    self.units[i].respawn_at = Some(self.tick + 5 * u64::from(TICK_HZ));
                    continue;
                }
            }
            self.respawn_slot(i);
        }

        self.populate_native_teams();

        // 0b. SQUAD ORDERS (`squad`): one edge per player slot in slot order,
        //     then each unit whose think is due takes the order it was
        //     given. Before the commander, so a bot that just left the pool
        //     is not given an objective this same tick.
        self.squad_pass(pads);

        // Card HITCH: the AI's wall time (the frame recorder's; no sim input).
        let ai_t0 = std::time::Instant::now();
        // 1. The COMMANDER (Phase B), before anyone thinks, so the think loop
        //    below reads the plan made this same tick. One team per tick at
        //    most: team t plans when the tick reaches its `next_plan_tick`,
        //    which starts at t and advances by `PLAN_PERIOD`.
        for t in 0..2u8 {
            if self.tick >= self.commander[usize::from(t)].next_plan_tick {
                crate::commander::plan(self, t);
            }
        }
        // 1a'. Card TURR: the turret offers (`ai::man`), after the plan and
        //      before the thinks, so a bot handed one walks to it this tick.
        if !self.vehicles.is_empty() {
            crate::ai::man::offer_pass(self);
        }

        // 1b. Controllers → per-unit pads (AI think reads the sim, writes back
        //    its scratch state afterwards).
        let mut unit_pads: Vec<PadState> = self
            .units
            .iter()
            .map(|u| match u.controller {
                Controller::Player { slot } => {
                    pads.get(usize::from(slot)).copied().unwrap_or_default()
                }
                _ => PadState::default(),
            })
            .collect();
        // 1a. Every BF2 unit's own aim-error block (card CLSE2), before the
        //     think that reads it.
        self.aim_blocks();
        // 1a''. Card TEAM: the team board's roles, before the thinks.
        crate::ai::team::board(self);
        // 1a'''. Card TOKN: the human players' hit tokens, before the thinks.
        crate::ai::tokn::deal(self);
        let mut ai_updates = std::mem::take(&mut self.ai_updates);
        ai_updates.clear();
        // Card TEAM: the unit grid answers "who is near" for the thinks, which
        // read a sim no one moves until the write-back.
        self.unit_grid.build(&self.units);
        // The bot that took longest, and the rays it cast (the slow-frame log).
        let mut worst_think: Option<crate::sim::UnitCost> = None;
        crate::ai::think_memo::during_thinks(|| {
            for (i, u) in self.units.iter().enumerate() {
                if u.alive && u.controller == Controller::Ai {
                    let mut st = u.ai;
                    let think_t0 = std::time::Instant::now();
                    let queries_before = crate::collision::cost::queries();
                    let (p, nt) = ai::think(self, self.handle(i), &mut st);
                    let ms = think_t0.elapsed().as_secs_f32() * 1000.0;
                    if worst_think.is_none_or(|w| ms > w.ms) {
                        worst_think = Some(crate::sim::UnitCost {
                            unit: i as u32,
                            ms,
                            ray_queries: crate::collision::cost::queries() - queries_before,
                        });
                    }
                    ai_updates.push((i, u.gen, u.mount, st, p, nt));
                }
            }
        });
        self.unit_grid.clear();
        self.ai_wall_ms = ai_t0.elapsed().as_secs_f32() * 1000.0;
        self.tick_cost.worst_think = worst_think;
        let mut recycled: Vec<(usize, DeathCause)> = Vec::new();
        // Card UNSN: the bots moved on unseen this tick, and where to.
        let mut relocated: Vec<(usize, Vec3)> = Vec::new();
        // Card TPOS: positions taken this tick, granted in unit-index order.
        let mut granted: Vec<glam::Vec3> = Vec::new();
        // WGCAL/WGOUT/WGNXT: wait-start, first and later post departures.
        // Sources publish in unit order after every write-back.
        // Publish only after every scratch AiState has been written back, so
        // neither index ordering can erase a receiver's pending target.
        let mut wait_calls = Vec::new();
        for (i, _, _, mut st, p, nt) in ai_updates.iter().copied() {
            if self.ground_rules == GroundRules::Bf1 {
                if let Some(w) = st.ride.and_then(|r| r.pickup) {
                    let previous = self.units[i].ai.ride.and_then(|r| r.pickup);
                    if w.started == Some(self.tick)
                        && previous.is_some_and(|old| {
                            old.vehicle == w.vehicle
                                && old.seated == w.seated
                                && old.started.is_none()
                        })
                    {
                        wait_calls.push((self.handle(i), w.vehicle, w.seated));
                    }
                    if w.post_departure == Some(self.tick)
                        && previous.is_some_and(|old| {
                            old.vehicle == w.vehicle
                                && old.seated == w.seated
                                && old.post_departure != w.post_departure
                        })
                    {
                        wait_calls.push((self.handle(i), w.vehicle, w.seated));
                    }
                    if w.departed
                        && previous.is_some_and(|old| {
                            old.vehicle == w.vehicle && old.seated == w.seated && !old.departed
                        })
                    {
                        wait_calls.push((self.handle(i), w.vehicle, w.seated));
                    }
                }
            }
            ai::tactic::grant(&self.positions, self.tick, &mut st, &mut granted);
            self.nav_stats.cover_ticks += u64::from(st.role.post.is_some());
            self.nav_stats.flank_ticks += u64::from(st.flank.route.is_some());
            self.nav_stats.overwatch_ticks += u64::from(st.behaviour == ai::Behaviour::Overwatch);
            self.units[i].ai = st;
            // Card BNDS: a bot off the level with no way back is given up
            // here, in the write-back's fixed order — and (card RCYC) one
            // on it that made no net progress.
            if nt.recycle != 0 {
                recycled.push((i, DeathCause::Lost));
            } else if nt.stalled != 0 {
                recycled.push((i, DeathCause::Stalled));
                self.nav_stats.flank_stalled += u64::from(self.units[i].ai.flank.route.is_some());
            } else if let Some(b) = nt.relocate {
                relocated.push((i, Vec3::from_array(b.map(f32::from_bits))));
            }
            if self.units[i].mount.is_some() {
                self.nav_stats.ai_drive_ticks += 1;
            }
            unit_pads[i] = p;
            self.nav_stats.bfs_fallbacks += u64::from(nt.bfs);
            self.nav_stats.straight_fallbacks += u64::from(nt.straight);
            self.nav_stats.flank_legs += u64::from(nt.flank);
            self.nav_stats.stuck_max = self.nav_stats.stuck_max.max(nt.stuck_run);
            // Card ARCM: a closing counts against the arc for the team.
            if let Some(a) = nt.closed_arc {
                self.nav_stats.arcs_closed += 1;
                let team = self.units[i].team;
                if self.note_arc_closed(team, a, i as u32) {
                    self.nav_stats.arcs_shunned += 1;
                }
            }
            self.nav_stats.shun_round += u64::from(nt.shun_round);
            self.nav_stats.tokn_aimed_off += u64::from(nt.tokn_off);
            self.nav_stats.shun_kept += u64::from(nt.shun_kept);
            for w in ai::why::fail_codes(&nt) {
                if let Some(n) = w.fail_index().and_then(|i| self.nav_stats.fails.get_mut(i)) {
                    *n += 1;
                }
            }
            self.nav_stats.detoured_legs += u64::from(nt.detoured);
            self.nav_stats.blocked_ticks += u64::from(nt.blocked);
            self.nav_stats.level_rejected += u64::from(nt.level_rejected);
            self.nav_stats.level_unrouted += u64::from(nt.level_unrouted);
            self.nav_stats.sweep_hits += u64::from(nt.sweep_hits);
            self.nav_stats.unroutable_objectives += u64::from(nt.unroutable);
            self.nav_stats.unreachable_dwell_max =
                self.nav_stats.unreachable_dwell_max.max(nt.dwell);
            self.nav_stats.contacts_max = self.nav_stats.contacts_max.max(nt.contacts);
            self.nav_stats.heard_contacts += u64::from(nt.heard);
            self.nav_stats.told_contacts += u64::from(nt.told);
            self.nav_stats.line_held += u64::from(nt.line_held);
            self.nav_stats.tpos_queries += u64::from(nt.tpos_query != 0);
            self.nav_stats.tpos_ceiling += u64::from(nt.tpos_query == 2);
            for (b, n) in self.nav_stats.tpos_ev.iter_mut().take(16).enumerate() {
                *n += u64::from(nt.tpos_ev >> b & 1);
            }
            for (n, f) in self.nav_stats.tpos_funnel.iter_mut().zip(nt.tpos_funnel) {
                *n += u64::from(f);
            }
            self.nav_stats.tpos_flips += u64::from(nt.tpos_hyst & ai::tactic::HY_FLIP != 0);
            self.nav_stats.tpos_kept += u64::from(nt.tpos_hyst & ai::tactic::HY_KEPT != 0);
            // Card HYST: the early thinks, and what they changed (the WHY
            // log's row of this tick, if the think wrote one).
            if nt.wake != 0 {
                for (b, n) in self.nav_stats.wakes.iter_mut().enumerate() {
                    *n += u64::from(nt.wake >> b & 1);
                }
                if let Some(r) = self.units[i]
                    .ai
                    .why
                    .last()
                    .filter(|r| r.tick == self.tick && !r.failed)
                {
                    if let Some(n) = self.nav_stats.wake_why.get_mut(r.why as usize) {
                        *n += 1;
                    }
                }
            }
            self.nav_stats.tpos_ev[16] += u64::from(
                nt.tpos_query == 2 && nt.tpos_ev & crate::ai::tactic::EV_PICK_COVERED == 0,
            );
            // Card UCALL: the think's combat calls; on BF2 only the
            // tracker's take, BF2's AcquiredTarget line (card UVO2,
            // `FUN_002cde30` B2:255580).
            if nt.acquired != 0 {
                self.events.push(Event::Call {
                    unit: self.handle(i),
                    call: Call::Acquired,
                });
            }
            if self.ground_rules != crate::soldier::GroundRules::Bf2 {
                let unit = self.handle(i);
                if nt.heard_turn != 0 {
                    self.events.push(Event::Call {
                        unit,
                        call: Call::Heard,
                    });
                }
                // Card UTALK: the trooper's "follow me" (issued with chance
                // 0.3) as it sets out, "move out" as it arrives.
                let issued = match nt.trooper {
                    crate::ai::NavTick::SET_OUT => {
                        let draw =
                            crate::ai::unit_unit(unit.index, self.tick, 0, crate::ai::SALT_FOLLOW);
                        (draw < super::state::FOLLOW_CHANCE).then_some(Call::Follow)
                    }
                    crate::ai::NavTick::ARRIVED => Some(Call::MoveOut),
                    _ => None,
                };
                if let Some(call) = issued.filter(|_| self.units[i].mount.is_none()) {
                    self.events.push(Event::Call { unit, call });
                }
            }
            if let Some(n) = self.nav_stats.man_codes.get_mut(usize::from(nt.man)) {
                *n += u64::from(nt.man != 0);
            }
            match nt.claim {
                crate::ai::NavTick::CLAIM_NEW => self.nav_stats.ai_claims += 1,
                crate::ai::NavTick::CLAIM_GIVEUP => self.nav_stats.ai_claim_giveups += 1,
                crate::ai::NavTick::CLAIM_LOST => self.nav_stats.ai_claim_lost += 1,
                _ => {}
            }
            // `think` takes `&Sim`, so a unit whose order is over — a follower
            // or a holder whose leader is gone, or has been beyond LEAVE_DIST
            // for LEAVE_TICKS of thinks — reports it and the ORDER IS ENDED HERE, inside
            // `Sim::step`'s fixed order, like every other write-back, so a
            // release is replay-exact.
            if nt.released != 0 {
                self.units[i].command = None;
                self.units[i].ai.cmd = None;
                self.nav_stats.releases += 1;
            }
        }
        for (i, cause) in recycled {
            let h = self.handle(i);
            self.kill_environment(h, cause);
        }
        self.relocate_unseen(relocated);
        for (pilot, vehicle, seated) in wait_calls {
            // Recycled units and stale seating tenures cannot issue a call.
            if self.unit(pilot).is_some_and(|u| {
                u.alive
                    && u.controller == Controller::Ai
                    && u.mount.is_some_and(|m| m.vehicle == vehicle && m.seat == 0)
                    && u.ai
                        .ride
                        .and_then(|r| r.pickup)
                        .is_some_and(|w| w.vehicle == vehicle && w.seated == seated)
            }) && self.vehicle(vehicle).is_some_and(|v| {
                matches!(
                    v.kind,
                    crate::vehicle::VehicleKind::Hover | crate::vehicle::VehicleKind::Walker
                ) && v.seats.first().is_some_and(|s| s.occupant == Some(pilot))
            }) {
                self.need_gunner_call(pilot, vehicle);
            }
        }

        self.force_locks(&mut unit_pads);
        let guided_pads = self.guided_pads(&mut unit_pads);
        let droid_pads = if self.droids.is_empty() {
            Vec::new()
        } else {
            self.droid_pads(&mut unit_pads)
        };

        // 1c. The use input (VSEAT, `seats`): an ACTION edge enters what the
        //     soldier looks at, or exits. No bot presses it: a bot's enter /
        //     exit is the request its think left (card VAI), served after.
        if !self.vehicles.is_empty() {
            self.use_pass(&unit_pads);
            self.switch_pass(&unit_pads);
            self.ai_use_pass();
        }
        // A pad belongs to the controller/seat that computed it. Boarding
        // happens after the thinks: a foot cloak/repair press must not become
        // a new seat's gun trigger (nor its movement become driving input).
        // Same-seat updates still reach the weapon's ordinary carry state.
        for &(i, gen, mount, _, _, _) in &ai_updates {
            let u = &self.units[i];
            if u.gen != gen || u.controller != Controller::Ai || u.mount != mount {
                unit_pads[i] = PadState::default();
            }
        }
        ai_updates.clear();
        self.ai_updates = ai_updates;

        // 2. BF1 aim assist on each player slot's sticks (FUN_0025fea8 via
        //    core::aim_assist) toward LAST tick's magnet, then this tick's
        //    acquisition (FUN_0025e098), in the PS2's order
        //    (`analysis/re-aim-assist.md` §3.1). A slot whose player is dead
        //    keeps its last magnet (nothing re-acquires until it respawns).
        self.sync_slot_tables();
        for slot in 0..self.players.len() {
            let Some(ph) = self.players[slot] else {
                continue;
            };
            if self.controlled_guided(ph).is_some() {
                self.magnet[slot] = None;
                self.reticle_pick[slot] = None;
                continue;
            }
            if self.unit(ph).is_none_or(|u| u.mount.is_some()) {
                self.acquire_flyer_pick(slot, ph);
                continue;
            }
            let p = unit_pads[ph.index as usize];
            let flying = self.controlled_droid(ph).is_some();
            if let Some(m) = self.magnet[slot].filter(|_| !p.mouse_aim() && !flying) {
                unit_pads[ph.index as usize] = self.apply_assist(slot, ph, m, &p, dt);
            } else {
                self.assist_lead[slot] = None;
            }
            let steer = super::input::Steer::of(&p);
            self.acquire_magnet_steering(slot, ph, p.pressed(buttons::FIRE), steer);
        }
        // 2a. The launchers' dwell locks (card UORD §5): a human's on-foot
        //     lock (`foot_lock`), the magnet target in space, a bot's
        //     combat target, counted against LockTime.
        self.tick_lock_on(&unit_pads, dt);
        self.tick_foot_lock(&mut unit_pads, dt);
        self.tick_locks(dt);

        // 2b. DOORS (DR2): open or close by who stands in their triggers
        //     now, before anyone moves against their boxes (`crate::door`;
        //     the placement in the tick is PROVISIONAL).
        self.tick_doors(dt);
        // 2b'. Card TCOL: a building turret's prims on its gun's nodes,
        //     placed at the gun's pose (`BuildingTurret::posed`).
        self.pose_building_turrets();

        // [ours] A FIRE press with a fire lag (`PadState::fire_lag`): the
        // aim before the turn, so phase 4 fires part way into it.
        let lagged: Vec<(usize, (f32, f32))> = unit_pads
            .iter()
            .enumerate()
            .filter(|&(i, p)| {
                let u = &self.units[i];
                p.fire_lag != 0
                    && u.alive
                    && u.mount.is_none()
                    && matches!(u.controller, Controller::Player { .. })
                    && p.pressed(buttons::FIRE)
                    && u.soldier.prev_buttons() & buttons::FIRE == 0
            })
            .map(|(i, _)| (i, (self.units[i].soldier.yaw, self.units[i].soldier.pitch)))
            .collect();

        self.press_edges.clear();
        self.press_edges
            .extend(unit_pads.iter().enumerate().map(|(i, p)| {
                let u = &self.units[i];
                if u.is_player {
                    // A droideka never runs the soldier's step: its own are the
                    // previous buttons.
                    let prev = u
                        .droideka
                        .as_ref()
                        .map_or(u.soldier.prev_buttons(), |d| d.prev_buttons);
                    p.buttons & !prev
                } else {
                    0
                }
            }));
        self.saber_inputs(&mut unit_pads);
        // 3. Movement.
        for (i, pad_i) in unit_pads.iter_mut().enumerate() {
            // The contact accumulator is per TICK, so it is cleared for every
            // unit — dead ones included — before anything can set it, and the
            // wall pass below is the only writer.
            self.units[i].wall_push = Vec3::ZERO;
            if !self.units[i].alive || self.units[i].mount.is_some() {
                // Death and a seat end the zoom (`re-zoom.md` §4).
                self.units[i].zoom = None;
                self.units[i].first_person = false;
            }
            // Card KNOCK (re-knockback §3): a dead unit's body still moves -
            // the death block first, then the soldier's own update with no
            // stick, the wall pass and the floor.
            let corpse = !self.units[i].alive && self.units[i].body;
            if !self.units[i].alive && !corpse {
                continue;
            }
            if corpse {
                self.death_block(i);
                // A droid's burst took the body out of the world (DROIDDEATH).
                if !self.units[i].body {
                    continue;
                }
            }
            if self.units[i].mount.is_some() {
                self.ride(i, pad_i);
                continue;
            }
            if self.units[i].droideka.is_some() {
                if self.units[i].alive {
                    self.step_droideka(i, pad_i, dt);
                }
                continue;
            }
            // Where the tick's move starts: the wall pass tests the move, not
            // only its end (card WPSH).
            let from = self.units[i].capsule();
            // Some(was_grounded) when the soldier stepped: his floor contact
            // resolves AFTER the wall pass (card STHOP).
            let mut stepped: Option<bool> = None;
            match self.units[i].controller {
                _ if corpse => {
                    stepped = Some(self.units[i].soldier.grounded);
                    let (consts, rules) = (self.units[i].consts, self.ground_rules);
                    self.units[i]
                        .soldier
                        .step(dt, &PadState::default(), &consts, rules);
                }
                Controller::Pacer { vel, period } => {
                    let next = self.units[i].soldier.pos + vel * dt;
                    let g = self.ground_under(next, false);
                    let reverse =
                        period > 0 && self.tick % u64::from(period) == u64::from(period) - 1;
                    let u = &mut self.units[i];
                    u.soldier.vel = vel;
                    u.soldier.pos = Vec3::new(next.x, g, next.z);
                    if reverse {
                        u.controller = Controller::Pacer { vel: -vel, period };
                    }
                }
                _ => {
                    // PROVISIONAL (BF2 feel): firing interrupts a sprint —
                    // mask SPRINT while FIRE is held so the stance machine
                    // drops to stand and stays there.
                    if pad_i.pressed(buttons::FIRE) {
                        pad_i.buttons &= !buttons::SPRINT;
                    }
                    let stamina_cheat = self.cheats.infinite_stamina;
                    let mut consts = self.units[i].consts;
                    consts.hero =
                        self.hero_class(self.units[i].team) == Some(self.units[i].spec.class);
                    self.saber_constants(i, &mut consts);
                    self.force_constants(i, &mut consts);
                    // The zoom press and its cancels, then the turn: zoomed,
                    // the yaw and pitch rates are divided by the weapon's
                    // turn divisor (`re-zoom.md` §4, §6).
                    self.units[i].first_person =
                        matches!(self.units[i].controller, Controller::Player { .. })
                            && pad_i.pressed(buttons::FIRST_PERSON);
                    self.tick_zoom(i, pad_i);
                    if let Some(z) = self.units[i].zoom {
                        consts.max_turn_speed /= z.divisor;
                        consts.max_pitch_speed /= z.divisor;
                    }
                    stepped = Some(self.units[i].soldier.grounded);
                    let rules = self.ground_rules;
                    let look = self.units[i].look_scale();
                    // Human taps and the AI mover have separate jet commands.
                    let env = crate::soldier::AirEnv {
                        ceiling: self.fly_ceiling,
                        human: self.units[i].is_player,
                        bot_jet: self.units[i].ai.jet.command(),
                    };
                    self.saber_gravity(i);
                    self.units[i]
                        .soldier
                        .step_looking(dt, pad_i, &consts, rules, look, env);
                    let u = &mut self.units[i];
                    if stamina_cheat && u.is_player {
                        u.soldier.energy = consts.energy_bar;
                        u.soldier.energy_depleted = false;
                    }
                    // The stamina cheat also keeps a player's jet tank full - a
                    // class with a jet only, so it gives no rifleman a jet.
                    if stamina_cheat && u.is_player && consts.has_jet() {
                        u.soldier.fuel = 1.0;
                    }
                }
            }
            // 3a. Wall pass (PROVISIONAL, ours — core::collision): push the
            // capsule out of anything it walked into. The push is horizontal,
            // so it never fights the floor contact. EVERY alive unit, player or
            // AI (Phase A design decision 5): the AI follows the authored
            // corridors now, so it no longer needs to walk through buildings
            // to get anywhere.
            //
            // The capsule asked is the STEPPED one (`Capsule::stepped`, A.4) —
            // the very shape the AI's forward probe uses. A kerb lower than
            // STEP_HEIGHT is something a soldier walks up, so the pass must
            // not push him off it: he walks his feet into the riser and the
            // ground query (3a', below) puts him on top, which is what climbing a
            // step IS. A wall whose top is above STEP_HEIGHT is untouched.
            // A LEDGE (the stepped capsule's floor-like contact with something
            // taller than the step) is a wall too: `push_out_soldier`, card JWAL.
            // The pass runs along the tick's move from `from`, in sub-steps of
            // half a radius, and never pushes a soldier to the far side of a
            // mesh face he came at from the near one (`push_out_soldier_from`,
            // card WPSH): a BF2 sprint-jump no longer tunnels a sheet.
            //
            // The push then takes the velocity's INTO-wall part off and keeps
            // the along-wall part. The PS2 does exactly that on a wall contact
            // (`analysis/re-soldier-walls.md` §5.1, PCSX2 BF1 KAS2: −8.50 → 0.00
            // m/s in one frame, and a slide along a curved wall). So a jump
            // pressed against a wall goes straight up, as on the PS2, instead
            // of carrying the run's speed into it.
            //
            // In the AIR (the jump and fall stances, off the floor) this
            // pass's step is `AIR_STEP` (card JARC): the capsule is asked
            // that much lower, so the stepped one's bottom is `AIR_STEP` over
            // the feet. With the whole step a jump mantled a ledge 0.6 m over
            // its apex.
            let s0 = &self.units[i].soldier;
            // (The jets too, card JETP: `Stance::airborne`.)
            let in_air = !s0.grounded && s0.stance.airborne();
            if !self.collision.is_empty() {
                let to = self.units[i].capsule();
                if let Some((t, normal)) = self.collision.ceiling_sweep(&from, &to) {
                    let delta = to.base - from.base;
                    let rest = delta * (1.0 - t);
                    let slide = rest - normal * rest.dot(normal).min(0.0);
                    let soldier = &mut self.units[i].soldier;
                    soldier.pos = from.base + delta * t + slide;
                    let into = soldier.vel.dot(normal);
                    soldier.vel -= normal * into.min(0.0);
                }
                let drop = if in_air { STEP_HEIGHT - AIR_STEP } else { 0.0 };
                let low = |c: Capsule| Capsule {
                    base: c.base - Vec3::Y * drop,
                    height: c.height + drop,
                    ..c
                };
                let push = self
                    .collision
                    .push_out_soldier_from(&low(from), &low(self.units[i].capsule()));
                self.units[i].soldier.pos += push;
                let len = (push.x * push.x + push.z * push.z).sqrt();
                if len > 0.0 {
                    let (nx, nz) = (push.x / len, push.z / len);
                    let v = &mut self.units[i].soldier.vel;
                    let into = v.x * nx + v.z * nz;
                    if into < 0.0 {
                        v.x -= into * nx;
                        v.z -= into * nz;
                    }
                }
                // RECORDED, not re-applied: the AI reads it next tick as "I am
                // in contact on this side" (`Unit::wall_push`, PLAN "PHASE A.2
                // DESIGN" decision 2). Motion is exactly what it was.
                self.units[i].wall_push = push;
            }
            self.push_turrets(i, from);
            // 3a'. The floor under the NEW feet, where the wall pass left them
            // (card STHOP, analysis/re-soldier-ground.md §3: the PS2 finds the
            // contact after the move; resolving before the wall pass judged a
            // soldier pressed on an edge railing at his pre-push position,
            // over the pit, and he sank through the edge). The impact speed
            // is read before it resolves. `rising` = a JUMP going up:
            // plane-following gives a walking soldier vel.y > 0 uphill, and
            // probing from the feet then misses the ramp above them.
            if let Some(was_grounded) = stepped {
                let s1 = &self.units[i].soldier;
                // A jet going up rises too (card JETP): a hover over a deck
                // does not snap onto it while it climbs.
                let rising = matches!(
                    s1.stance,
                    crate::soldier::Stance::Jump
                        | crate::soldier::Stance::Jet
                        | crate::soldier::Stance::Hover
                ) && s1.vel.y > 0.0;
                // The ground query keeps the whole step in the air (a
                // floor the feet dipped under within the tick's move is
                // still found), except for a face too steep to stand on
                // over the air's step: the wall pass has just called it
                // a wall, and lifting him onto it hangs him there.
                let (mut fy, mut fnrm) = self.ground_under_n(s1.pos, rising, from.base.y);
                if in_air
                    && !rising
                    && fnrm.y <= crate::soldier::FLOOR_MIN_NY
                    && fy > s1.pos.y + AIR_STEP
                {
                    (fy, fnrm) = self.ground_under_falling(s1.pos, from.base.y);
                }
                let vy_before = s1.vel.y;
                let rules = self.ground_rules;
                self.units[i]
                    .soldier
                    .resolve_ground(dt, fy, fnrm, from.base.y, rules);
                let landed = self.units[i].alive && !was_grounded && self.units[i].soldier.grounded;
                // A hard landing UNDER THE MAP kills, cheat or not (card
                // KFLR, `sim/bounds.rs`); any other one at the lethal
                // speed is the fall it was.
                if landed && self.lands_under_the_map(self.units[i].soldier.pos, vy_before) {
                    let h = self.handle(i);
                    self.kill_environment(h, DeathCause::UnderMap);
                } else if landed
                    && vy_before <= -LETHAL_LANDING_SPEED
                    && !self.units[i].consts.collision_damage_immune
                {
                    let h = self.handle(i);
                    self.kill_environment(h, DeathCause::Fall);
                }
            }
            if corpse {
                // BF2's get-up ends straight in the dead state (no standing
                // update, §4); a corpse leaves the world after its 4 s.
                if self.ground_rules == GroundRules::Bf2 {
                    self.death_block(i);
                }
                let s = &self.units[i].soldier;
                if s.stance == Stance::Dead && s.time_in_stance > crate::soldier::CORPSE_SECS {
                    self.units[i].body = false;
                }
            }
        }

        // 3a, after the soldiers' wall pass. Hovers (HOVR): drive, springs,
        // contacts, one vehicle at a time in slot order; then the walkers
        // (WALK): input and locomotion, probes, legs, contacts.
        self.clear_driverless();
        self.tick_hovers(dt);
        self.tick_walkers(dt);
        self.tick_flyers(dt);
        self.sync_hulls();
        self.carry_riders();

        // 3b. Unit-vs-unit separation (PROVISIONAL, no RE: soldiers must not
        // share a footprint). One pass over index-ordered pairs, equal push
        // along the XZ offset, capsule radius each; re-grounded afterwards.
        self.separate_units();

        // 3c. Death regions: the mission's `AddDeathRegion` volumes, tested on
        // the entity ORIGIN — the same point `update_cps` uses
        // (analysis/re-cp-capture.md §4). Units in index order, regions in the
        // mission's call order, first region wins; every region kills the same
        // way, so nothing is compared.
        // The kill floor first (card KFLR): below it nothing is playable.
        if let Some(floor) = self.kill_floor {
            for i in 0..self.units.len() {
                if self.units[i].alive && self.units[i].soldier.pos.y < floor {
                    // Aboard a flyer he is in the air: the floor is a soldier's
                    // and a fallen craft's, 15 m under the lowest post. On BF1
                    // Bespin that is 24 m under the decks, and the mission lets
                    // a pilot fly to y -200.
                    let flying = self.units[i]
                        .mount
                        .and_then(|m| self.vehicle(m.vehicle))
                        .is_some_and(|v| {
                            v.kind == crate::vehicle::VehicleKind::Flyer && v.has_pilot()
                        });
                    if flying {
                        continue;
                    }
                    let h = self.handle(i);
                    self.kill_environment(h, DeathCause::KillFloor);
                }
            }
            // A craft that went over the edge (BF2 Mygeeto's decks) would fall
            // forever, alive, and its pad would never make another. A piloted
            // flyer is flying, not falling; one with only a gunner aboard is
            // falling. OURS (PROVISIONAL), as the kill floor is.
            let fallen: Vec<_> = self
                .alive_vehicles()
                .filter(|(_, v)| {
                    v.pos.y < floor
                        && (v.kind != crate::vehicle::VehicleKind::Flyer || !v.has_pilot())
                })
                .map(|(h, _)| h)
                .collect();
            for h in fallen {
                self.destroy_vehicle(h, None);
            }
        }
        if !self.death_regions.is_empty() {
            for i in 0..self.units.len() {
                if !self.units[i].alive {
                    continue;
                }
                let p = self.units[i].soldier.pos;
                if self.death_regions.iter().any(|r| r.contains(p)) {
                    let h = self.handle(i);
                    self.kill_environment(h, DeathCause::DeathRegion);
                }
            }
        }

        self.tick_soldier_water();
        self.tick_damage_regions();
        self.saber_hits();
        // 4. Weapons → bolts.
        let mut shots = Vec::new();
        // A lagged press: its unit and the turned aim, put back after it fires.
        let mut turned: Option<(usize, (f32, f32))> = None;
        for (i, &p) in unit_pads.iter().enumerate() {
            if let Some((j, aim)) = turned.take() {
                self.put_aim(j, aim);
            }
            if let Some(&(_, from)) = lagged.iter().find(|(j, _)| *j == i) {
                // Turn the share drawn, fire, then the rest of the turn.
                let s = &self.units[i].soldier;
                let to = (s.yaw, s.pitch);
                let k = p.fire_share();
                self.put_aim(
                    i,
                    (from.0 + (to.0 - from.0) * k, from.1 + (to.1 - from.1) * k),
                );
                turned = Some((i, to));
            }
            if !self.units[i].alive || matches!(self.units[i].controller, Controller::Pacer { .. })
            {
                continue;
            }
            // A rider fires its seat's guns, not its own weapon (card VTUR).
            if self.units[i].mount.is_some() {
                self.seat_gun(i, &p);
                continue;
            }
            // Both channels lose their triggers below the fire point, including
            // a latched salvo or charge. Seat guns keep their own rules.
            let submerged = self.water_blocks_fire(i);
            let mut p = p;
            if submerged {
                p.buttons &= !(buttons::FIRE | buttons::SECONDARY_FIRE);
                // WeaponState's blocked gate prevents a new cycle, but an
                // already latched salvo can still emit. Discard those shots.
                self.units[i].weapon.salvo_counter = 0;
                for w in &mut self.units[i].held {
                    w.state.salvo_counter = 0;
                }
            }
            let move_mag = PadState::axis(p.ly).abs().max(PadState::axis(p.lx).abs());
            let stance_idx = match self.units[i].soldier.stance {
                Stance::Stand => 0,
                Stance::Crouch => 1,
                Stance::Prone => 2,
                _ => 0,
            };
            // BF1 skips fire processing in the roll states (re-stance-roll
            // §3.4); BF2 rolls believed to block too. A throw, its tumble and
            // get-up and the lock after it hold the trigger off too (card
            // KNOCK, re-knockback §2: the PS2 zeroes the trigger bytes;
            // PROVISIONAL: the port skips the fire processing, so a held
            // charge is neither fired nor lost). So does a jump: B1
            // FUN_00205cc8 sends states 4-6 (jump, thrown, fall) through
            // 0x532980 to 0x207980, which zeroes both channels' trigger
            // bytes; B2 FUN_00240bf8 cases 4 / 8 / 9 likewise. (The fall
            // state is not blocked here yet.)
            let blocked = submerged
                || matches!(self.units[i].soldier.stance, Stance::Roll | Stance::Jump)
                || self.units[i].soldier.no_fire()
                || self.units[i].choked.is_some()
                || self.units[i].droideka.as_ref().is_some_and(|d| d.no_fire());
            // A soldier with more slots than the one in its hands (GREN):
            // the channels decide whether FIRE reaches the primary.
            let mut fire_held = if self.units[i].held.is_empty() {
                p.pressed(buttons::FIRE)
            } else {
                self.held_weapons(i, &p, blocked, dt)
            };
            // Card RKSLF (OURS): a bot's exploding shot that this update
            // could release - a salvo latched in the cycle, a held trigger,
            // or a charge weapon's trigger coming up (it fires on the
            // release, `fire_predicate`) - whose blast would come back on its
            // own side is not let go: the salvo is cancelled as the PS2's
            // put-away cancels one, and the trigger stays as it was before
            // the shot (up, or a charge still held).
            if self.units[i].controller == Controller::Ai {
                let w = &self.units[i].weapon;
                let charge = self.weapon_classes[self.units[i].weapon_class].max_pressed_time > 0.0;
                let releasing = w.salvo_counter >= 1
                    || if charge {
                        !fire_held && w.fire_cmd > 0
                    } else {
                        fire_held
                    };
                if releasing && crate::ai::release_blasts_own_side(self, i) {
                    fire_held = charge && self.units[i].weapon.fire_cmd > 0;
                    self.units[i].weapon.salvo_counter = 0;
                    self.nav_stats.blast_holds += 1;
                }
            }
            let remote = self.remote_input(i, None, fire_held, blocked);
            fire_held &= !remote;
            if self.weapon_classes[self.units[i].weapon_class]
                .saber
                .is_some()
            {
                continue;
            }
            if self.force_input(i, None, fire_held, blocked) {
                continue;
            }
            if self.tick_repair_weapon(i, None, fire_held, blocked, dt) {
                continue;
            }
            if self.cloak_input(i, None, fire_held, blocked) {
                continue;
            }
            let (infinite_ammo, no_reload) = self.ammo_cheats(i);
            let input = FireInput {
                held: fire_held,
                blocked,
                infinite_ammo,
                no_reload,
                // Phase C D9: the AI sniper distance table's yaw term, chosen
                // by this unit's own `think` this tick from its range to its
                // target (`analysis/re-ai-sniper.md` §2). A player's is
                // always `None` — nothing writes `AiState` for one.
                ai_dispersion: self.units[i].ai.dispersion,
            };
            let wc_idx = self.units[i].weapon_class;
            shots.clear();
            // Phase C D9: the skill's SPREAD multiplier is the PS2's
            // `weapon+0x134`, written by the AI every aim tick (RE §4.2 step
            // 2). Ours is written once per fire tick, which has the same
            // effect — `spread_mult` is only read when a shot adds spread
            // (`weapon.rs`, `spread_per_shot`). A player keeps 1.0.
            let ai_spread_mult = (self.units[i].controller == Controller::Ai)
                .then(|| self.skill_of(&self.units[i]).spread_mult);
            let fire_empty;
            let reload_started = {
                let wc = &self.weapon_classes[wc_idx];
                let u = &mut self.units[i];
                if let Some(m) = ai_spread_mult {
                    u.weapon.spread_mult = m;
                }
                u.weapon
                    .set_posture(wc, stance_idx, move_mag, u.zoom.is_some());
                let was_reloading = u.weapon.reloading();
                if p.pressed(buttons::RELOAD) {
                    u.weapon.start_reload_as(wc, infinite_ammo);
                }
                u.weapon.update(dt, wc, input, &mut self.lcg, &mut shots);
                // The empty click: a human's trigger press on a weapon that
                // refuses it (PS2: the owner's player index >= 0,
                // re-weapon-sounds.md §7.3).
                fire_empty = u.is_player && u.weapon.press_clicks(wc, input, !shots.is_empty());
                !was_reloading && u.weapon.reloading()
            };
            if fire_empty {
                self.push_fire_empty(i, wc_idx);
            }
            if reload_started {
                let h = self.handle(i);
                self.events.push(Event::Reload {
                    unit: h,
                    weapon_class: wc_idx as u32,
                });
            }
            if shots.is_empty() {
                continue;
            }
            // Design decision 2's fairness rule, asserted where the shot is
            // made: an AI unit may only fire at a contact its own perception
            // holds. The think of this same tick has already written `ai`
            // back, so the record is this tick's.
            let ai_shooter = self.units[i].controller == Controller::Ai;
            let perceived = !ai_shooter || {
                let st = &self.units[i].ai;
                st.target
                    .is_some_and(|t| st.contacts.iter().flatten().any(|c| c.handle == t))
            };
            for _ in &shots {
                if ai_shooter {
                    self.nav_stats.ai_shots += 1;
                    // The rule is about the AI's DECISION, so it is checked
                    // where the decision is: the tick's own FIRE press. A
                    // weapon whose cycle is already running emits its last
                    // bolt after the trigger comes up (`WeaponState::update`
                    // stays in `FireState::Firing` until `cycle_finished`),
                    // and MEASURED over the 32-map sweep that is the only way
                    // a shot ever leaves an AI with no target: 1-7 per map,
                    // every one of them `fire_pressed false, target None,
                    // contacts 0, state Firing` (the probe is in the phase-c2
                    // report). Those are counted apart, not asserted on.
                    if p.pressed(buttons::FIRE) {
                        debug_assert!(
                            perceived,
                            "AI unit {i} fired at an unperceived target at tick {}",
                            self.tick
                        );
                        if !perceived {
                            self.nav_stats.unperceived_shots += 1;
                        }
                    } else {
                        self.nav_stats.carry_shots += 1;
                    }
                }
            }
            self.soldier_shots(i, None, &shots);
        }

        if let Some((j, aim)) = turned.take() {
            self.put_aim(j, aim);
        }

        // 5. Bolt flight + impacts (in spawn order): first what each bolt
        //    passes through on this tick's flight (issue #41).
        self.tick_cloaks(dt);
        self.tick_saber_throws();
        self.fly_missiles(dt);
        self.fly_shells(dt);
        self.fly_bolts(dt);
        self.fly_tow_cables(dt);

        // 5b. The thrown grenades (GREN).
        self.tick_stickies(dt);
        self.tick_mines(dt);
        self.tick_droids(dt, &droid_pads);
        self.tick_turrets(dt);
        self.tick_self_turrets();
        self.tick_pilot_threats();
        self.tick_guided(&guided_pads, dt);

        // 5c. The pickups (PWUP): flight, life, the touch.
        self.tick_buffs(dt);
        self.tick_burns(dt);
        self.tick_pickups(dt);

        // 5c'. The health / ammo droids (PWST): each station's clock and pass.
        self.tick_stations(dt);

        // 6. Command posts → bleed → domination → outcome.
        self.tick_space_assault(dt);
        self.tick_space_balance();
        self.tick_ctf(dt);
        self.tick_target_hints();
        self.tick_flyer_seek(dt);
        self.update_cps(dt);
        self.step_deathmatch();

        // 7. Vehicle pads: follow the posts just updated, decay, spawn.
        self.tick_vehicle_pads(dt);

        // The grenades that have gone off are gone (card HOLD).
        if !self.hazards.is_empty() {
            let now = self.tick;
            self.hazards.retain(|z| z.until > now);
        }
        // ...and the ones still live warn the bots near them (card UCALL;
        // on BF2 the same rule asks the Grenade line, card UVO2:
        // `FUN_002a89b8` B2:229141 - its delay PROVISIONAL, BF1's).
        self.grenade_calls();
        self.tick_cost.rays = crate::collision::cost::take();
        self.tick += 1;
    }
}

impl Sim {
    /// Card UCALL, BF1's "grenade!" (B1 `FUN_00236be8` 150870-150939,
    /// re-unit-chatter.md 2.3): a live grenade, after the reaction delay
    /// ([`super::state::GRENADE_WARN_DELAY`], drawn per grenade) and then
    /// once a second, lists the units within [`super::HAZARD_RADIUS`]; the
    /// FIRST of them that is an AI soldier says its `FleeSound`. A human's
    /// soldier never does; a rider is its vehicle, not a soldier; the
    /// thrower is not warned of its own (card GRAI's reading of the PS2).
    /// PROVISIONAL [H]: "first" is the lowest unit slot - the PS2's order is
    /// its collision query's.
    fn grenade_calls(&mut self) {
        let (lo, hi) = super::state::GRENADE_WARN_DELAY;
        let hz = crate::TICK_HZ as f32;
        let mut calls = Vec::new();
        for z in &self.hazards {
            let draw =
                crate::ai::unit_unit(z.since as u32, z.since, 0, crate::ai::SALT_GRENADE ^ 2);
            let first = z.since + ((lo + (hi - lo) * draw) * hz).round() as u64;
            if self.tick < first || !(self.tick - first).is_multiple_of(u64::from(crate::TICK_HZ)) {
                continue;
            }
            let speaker = self.units.iter().enumerate().find(|(i, u)| {
                u.alive
                    && u.controller == crate::unit::Controller::Ai
                    && u.mount.is_none()
                    && z.owner != Some(self.handle(*i))
                    && (u.soldier.pos - z.pos).length() < super::HAZARD_RADIUS
            });
            if let Some((i, _)) = speaker {
                calls.push(self.handle(i));
            }
        }
        for unit in calls {
            self.events.push(Event::Call {
                unit,
                call: Call::Flee,
            });
        }
    }
}

/// The aim ray starts this far past the camera's distance behind the eye
/// (`re-aim-assist.md` §10.2 step 5: `lui 0x4000`), and is this long (`lui
/// 0x4270`): what is nearer than the start is not seen at all (the PS2's
/// blind spot, FEEL_NOTES AIMA).
pub const AIM_RAY_START: f32 = 2.0;
pub const AIM_RAY_LENGTH: f32 = 60.0;
/// In the scope view the aim's far point is this far along it, with no ray.
pub const SCOPE_AIM_LENGTH: f32 = 200.0;
/// OURS (playtest 2026-11-05, B6): when the aim ray meets nothing within
/// its reach and the reticle picks nothing, a second ray goes on from its
/// far end this much further through the terrain and the ordnance layer,
/// so a shot at the ground or a wall out to 300 m converges on the
/// reticle's point instead of the ray's far end. Past a launcher's
/// MaxRange (150 m), close to a torpedo's 320 m flight. Ground maps only:
/// a foot shot in a space hangar keeps the PS2's far end. replace: a
/// maintainer playtest value if 300 m reads as too far or too near.
pub const FAR_AIM_LENGTH: f32 = 240.0;

impl Sim {
    /// The views drawn: one per player slot (split screen tilts the camera
    /// less, `unit::SPLIT_TILT`).
    pub fn views(&self) -> usize {
        self.players.len().max(1)
    }

    /// H of §10.2 step 5 for unit `i`: the first ordnance-solid thing on its
    /// aim ray between [`AIM_RAY_START`] past the track offset and
    /// [`AIM_RAY_LENGTH`] further - another unit's capsule, a vehicle's hull,
    /// the map's ordnance layer, the terrain - else the ray's far end.
    pub fn aim_hit(&self, i: usize) -> Vec3 {
        let u = &self.units[i];
        let (a, d) = u.aim_ray(self.ground_rules, self.views());
        // The scope view casts no ray (§10.2 step 7): the point 200 m along
        // the aim.
        if u.scope_path() {
            return a + d * SCOPE_AIM_LENGTH;
        }
        let sec = u.section();
        let start = a + d * (sec.offset.z + AIM_RAY_START);
        let mut best = AIM_RAY_LENGTH;
        // A terrain ray that starts under the ground reports no hit (§10.5
        // item 4 [M]); the port's own ray would find the way back out.
        let under = self.heightfield.as_ref().is_some_and(|h| {
            start.y < h.height_at(start.x, start.z)
                && !crate::collision::is_cut_by(&self.collision.cutters, start)
        });
        if !under {
            if let Some(t) = self.terrain_ray(start, d, best, TERRAIN_RAY_STEP) {
                best = best.min(t);
            }
        }
        let own = crate::collision::HullFilter::except(u.mount.map(|m| m.vehicle));
        let layer = crate::collision::Layers::ORDNANCE;
        if let Some(h) = self.collision.ray_with(start, d, best, layer, own) {
            best = best.min(h.t);
        }
        let me = self.handle(i);
        // A human sees the others `HUMAN_SHOT_LAG` behind (OURS, as his
        // bolt's sweep meets them).
        let lag = if matches!(u.controller, Controller::Player { .. }) {
            crate::unit::HUMAN_SHOT_LAG
        } else {
            0.0
        };
        for (h, o) in self.alive_units() {
            if h != me {
                let mut cap = o.capsule();
                cap.base -= o.soldier.vel * lag;
                if let Some(t) = cap.ray_hit(start, d, best) {
                    best = best.min(t);
                }
            }
        }
        // An auto-turret or remote droid is a body on the PS2's aim ray as on
        // a bolt's flight (its `p_-so_sphere`, mask soldier + ordnance): the
        // shot flies to the one the reticle is on. Its owner's own bolts pass
        // it, so his reticle's ray does too.
        let items = self
            .turrets
            .iter()
            .filter(|t| t.owner.index != me.index)
            .map(|t| (t.centre(), crate::autoturret::RADIUS))
            .chain(
                self.droids
                    .iter()
                    .filter(|o| o.owner.index != me.index)
                    .map(|o| (o.pos, crate::remote::RADIUS)),
            );
        for (at, radius) in items {
            if let Some(t) = crate::unit::ray_sphere(start, d, at, radius).filter(|&t| t < best) {
                best = t;
            }
        }
        if best >= AIM_RAY_LENGTH {
            if let Some(t) = self.beyond_reach(i, start, d) {
                return start + d * t;
            }
        }
        start + d * best
    }

    /// OURS (playtest 2026-10-28, BF1 Yavin 4, "far away shooting"): when
    /// the reticle's ray meets nothing within its reach and what the
    /// reticle picks (`Sim::reticle_pick`: the cone's pick, a unit or a
    /// vehicle) is past that reach, the shot is aimed at the point of the
    /// ray abreast of the pick, not at the ray's far end. The PS2 aims at
    /// the far end 65.5 m out (§10.5 item 2): the fire point is 0.51 m
    /// (BF1) / 0.77 m (BF2) under the ray, so the bolt crosses the ray there
    /// and climbs over it - 0.35 m at 100 m, 0.65 m at 150 m: a shot at the
    /// chest of a soldier past 110 m flies over his head. Two lines of 480
    /// on the PS2's screen; on today's it is most of a soldier. Within the
    /// reach the PS2's rule stands.
    ///
    /// Without such a pick (playtest 2026-11-05, B6: a rocket at the beach
    /// from Kashyyyk's docks), the ray goes on [`FAR_AIM_LENGTH`] further
    /// through the terrain and the ordnance layer, and the shot converges
    /// on the ground, wall or vehicle hull it meets. Soldiers, auto-turrets
    /// and droids past the reach that the reticle does not pick stay unseen
    /// (the PS2's rule). `None` when the far ray meets nothing either, and
    /// always on a space map (a foot shot in a hangar is the PS2's): the
    /// far end.
    fn beyond_reach(&self, i: usize, start: Vec3, d: Vec3) -> Option<f32> {
        let me = self.handle(i);
        let slot = self.players.iter().position(|p| *p == Some(me))?;
        let pick = self.reticle_pick.get(slot).copied().flatten();
        if let Some(at) = pick.and_then(|(p, _)| self.aim_candidate(p)).map(|c| c.ctr) {
            let t = (at - start).dot(d);
            if t > AIM_RAY_LENGTH {
                return Some(t);
            }
        }
        // Space Assault and space CTF: the far ray is a ground-map rule.
        if self.space_round() {
            return None;
        }
        let far = start + d * AIM_RAY_LENGTH;
        let mut best = FAR_AIM_LENGTH;
        let mut hit = false;
        // The same rule as the near ray: from under the ground, no hit.
        let under = self.heightfield.as_ref().is_some_and(|h| {
            far.y < h.height_at(far.x, far.z)
                && !crate::collision::is_cut_by(&self.collision.cutters, far)
        });
        if !under {
            if let Some(t) = self.terrain_ray(far, d, best, TERRAIN_RAY_STEP) {
                best = best.min(t);
                hit = true;
            }
        }
        let u = &self.units[i];
        let own = crate::collision::HullFilter::except(u.mount.map(|m| m.vehicle));
        let layer = crate::collision::Layers::ORDNANCE;
        if let Some(h) = self.collision.ray_with(far, d, best, layer, own) {
            best = best.min(h.t);
            hit = true;
        }
        hit.then_some(AIM_RAY_LENGTH + best)
    }

    /// The soldier's zoom routine (B1 `FUN_00204578`, B2 `FUN_0023ead8`,
    /// `analysis/re-zoom.md` §4), run before unit `i` moves, so it sees the
    /// stance and the weapon as the last tick left them. Only a human's
    /// soldier zooms. The zoom ends in a roll or a sprint (BF2's rule; BF1
    /// has no sprint and the port's takes it, FEEL_NOTES), when a blast
    /// throws him (the PS2's thrown state; the port also holds it off
    /// through the tumble, the get-up and its lock, FEEL_NOTES), on a weapon that
    /// cannot zoom - the secondary's grenade once it is the weapon in hand -
    /// and while the primary reloads; otherwise a press of ZOOM steps it
    /// (`WeaponClass::zoom_press`). Death, a seat and the primary's switch
    /// end it where they happen. Not ported: the continuous zoom of a class
    /// with `ZoomRate` (no hand weapon), BF2's melee test (no disc lightsaber
    /// has a `ZoomMax` over 1).
    fn tick_zoom(&mut self, i: usize, pad: &PadState) {
        let u = &self.units[i];
        let human = matches!(u.controller, Controller::Player { .. });
        let press = pad.pressed(buttons::ZOOM) && u.soldier.prev_buttons() & buttons::ZOOM == 0;
        let cancel =
            matches!(u.soldier.stance, Stance::Roll | Stance::Sprint) || u.soldier.no_fire();
        let (class, reloading) = match u.cur1.filter(|_| u.active_channel == 1) {
            Some(k) => (u.held[usize::from(k)].class, false),
            None => (u.weapon_class, u.weapon.reloading()),
        };
        let wc = &self.weapon_classes[class];
        let step = u.zoom.map(|z| z.step);
        let step = if !human || cancel || !wc.zooms() || reloading {
            None
        } else if press {
            wc.zoom_press(step)
        } else {
            step
        };
        let zoom = step.map(|step| {
            let (value, divisor) = wc.zoom_at(step);
            crate::unit::Zoom {
                step,
                value,
                divisor,
                scope: wc.zoom_first_person,
            }
        });
        self.units[i].zoom = zoom;
    }

    fn put_aim(&mut self, i: usize, (yaw, pitch): (f32, f32)) {
        let s = &mut self.units[i].soldier;
        (s.yaw, s.pitch) = (yaw, pitch);
    }

    /// A human's shot (§10.2 step 6): from the fire point F along
    /// normalize(H - F), with the spread's frame built on that line.
    pub(super) fn human_shot(&self, i: usize) -> (Vec3, (Vec3, Vec3, Vec3)) {
        let u = &self.units[i];
        // The scope view: from the scope point along the aim itself.
        if u.scope_path() {
            return (u.scope_point(self.ground_rules), u.aim_frame());
        }
        let f = u.droideka_muzzle().unwrap_or_else(|| u.fire_point());
        let (aim, aim_right, _) = u.aim_frame();
        let fwd = (self.aim_hit(i) - f).normalize_or(aim);
        let right = fwd.cross(Vec3::Y).normalize_or(aim_right);
        (f, (fwd, right, right.cross(fwd).normalize()))
    }

    /// Card UNSN: put the bots the think moved on unseen
    /// ([`crate::ai::unseen`]) on their spots, in unit order, each with an
    /// [`Event::Relocated`]. Two picked one spot this tick: the later is
    /// recycled as before (and counted so).
    fn relocate_unseen(&mut self, moves: Vec<(usize, Vec3)>) {
        let mut taken: Vec<Vec3> = Vec::new();
        for (i, to) in moves {
            let h = self.handle(i);
            let clash = taken.iter().any(|t| {
                crate::nav::xz_dist(*t, to) < ai::unseen::CLEAR_XZ
                    && (t.y - to.y).abs() < ai::unseen::CLEAR_DY
            });
            if clash {
                let fails = &mut self.nav_stats.fails;
                for (w, d) in [
                    (ai::why::Why::Relocated, -1i64),
                    (ai::why::Why::Recycled, 1),
                ] {
                    if let Some(n) = w.fail_index().and_then(|k| fails.get_mut(k)) {
                        *n = n.saturating_add_signed(d);
                    }
                }
                self.kill_environment(h, DeathCause::Stalled);
                continue;
            }
            taken.push(to);
            let u = &mut self.units[i];
            let from = u.soldier.pos;
            u.soldier.pos = to;
            u.soldier.vel = Vec3::ZERO;
            u.soldier.air_time = 0.0;
            u.wall_push = Vec3::ZERO;
            self.events.push(Event::Relocated { unit: h, from, to });
        }
    }
}

#[cfg(test)]
mod tests;
