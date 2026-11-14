//! GBOARD/WGORD call targets are consumed only at scheduled thinks over retained orders.
//! Foot-bot tick priority chain, valid boarding maintenance (CWAIT), and failed-walk recovery.

use crate::ai::brain::intent::Acts;
use crate::ai::census;
use crate::ai::draw::{stance_hold, unit_unit, SALT_DODGE};
use crate::ai::fix::{self, ServePlan};
use crate::ai::gunner_foot::{self, AimRequest};
use crate::ai::mover::{self, MoveRequest};
use crate::ai::route::{advance_route, leg_point, retarget, sub_point};
use crate::ai::state::{
    AiState, Behaviour, FightState, HoldState, LostState, MoverState, NavTick, RouteSource,
    SteerPick, Walk, WalkAsk,
};
use crate::ai::why::Why;
use crate::ai::{
    CIRCLE_PAUSE, CORNER_ARRIVE, DODGE_COOLDOWN, FIGHT_BEYOND, FLINCH_GAP, FLINCH_TICKS,
    GIVE_UP_AFTER, HOLD_ARRIVE, HOLD_CLOSE, HOLD_COMMIT, HOLD_QUIET, HOLD_RANGE, HOLD_STRAFE,
    LOST_RECYCLE, RETURN_ARRIVE, RETURN_ARRIVE_DY, RETURN_PLANS, RING_ARRIVE, SHUT_PROBE_UP,
    SHUT_RECYCLE, SIDESTEP_TICKS, THINK_PERIOD,
};
use crate::fmath;
use crate::nav;
use crate::perception;
use crate::sim::{PadState, Sim};
use crate::unit::UnitHandle;
use glam::{Vec2, Vec3};

use super::position::{fight_position, hazard_flee, react, react_ok, ring_walk, spot_free};
use super::Tick;

/// Produce this tick's pad for AI unit `me`, and say what it had to fall back
/// on (diagnostics only — see [`NavTick`]). Card WHY: the reason for a change
/// of behaviour, and a failed walk's, go into the bot's log
/// ([`crate::ai::why::log`]). Card UNSN: a recycle becomes an unseen move
/// when one is safe ([`crate::ai::unseen`]).
pub fn think(sim: &Sim, me: UnitHandle, st: &mut AiState) -> (PadState, NavTick) {
    let (was, cp) = (st.behaviour, st.target_cp);
    let (mut pad, mut nt, serve) = chain(sim, me, st);
    crate::ai::brain::finish_pad(sim, me, st, serve, &mut pad);
    // Card UNSN: a bot on the level given up for going nowhere is moved on
    // where no human sees it, else recycled.
    if nt.stalled != 0 && nt.recycle == 0 {
        if let Some(to) = crate::ai::unseen::spot(sim, me, st) {
            nt.stalled = 0;
            nt.relocate = Some(to.to_array().map(f32::to_bits));
            crate::ai::unseen::reset(st, sim.tick);
        }
    }
    crate::ai::why::log(st, sim.tick, was, cp, &nt);
    (pad, nt)
}

/// One tick of the priority chain ([`think`]).
fn chain(sim: &Sim, me: UnitHandle, st: &mut AiState) -> (PadState, NavTick, Option<ServePlan>) {
    let mut nt = NavTick::default();
    st.walk = WalkAsk::default();
    let Some(u) = sim.unit(me) else {
        st.jet.flight = None;
        return (PadState::default(), nt, None);
    };
    super::ride::settle(sim, st);
    let eye = u.ai_eye();
    let wc = &sim.weapon_classes[u.weapon_class];
    // The sight-gated aim (playtest C4) is a ground rule: a space round's
    // foot bots aim as they did.
    let ground = !sim.space_round();

    let tier = sim.unit_tier(u);
    let mend = crate::ai::proxy::mend_station(sim, u);
    // This unit's own skill vector, under the tier's overrides (D1/D9). It is
    // DERIVED per think, never stored: `Unit.skill` is the state. A proxy's
    // is its profile's (card DPRX, `Sim::skill_of`).
    let skill = sim.skill_of(u);

    // Card HYST: a hit, a near miss or a new enemy in sight since the last
    // think brings the next one forward (`crate::ai::wake`).
    st.wake |= crate::ai::wake::events(sim, me, u, st);
    let woken = st.wake != 0 && sim.tick >= st.last_think + crate::ai::wake::GAP;
    let thinking = sim.tick >= st.next_think || woken;
    if thinking {
        nt.thought = 1;
        if sim.tick < st.next_think {
            nt.wake = st.wake;
        }
        st.wake = 0;
        st.last_think = sim.tick;
        // Card MOVR: the think's PHASE is the unit's index (the design's
        // "one think rate everywhere, the phase by unit index"), not the tick
        // it spawned on: a wave that spawns together does not think together.
        // The first think is at once.
        st.next_think = sim.tick + THINK_PERIOD - (sim.tick + u64::from(me.index)) % THINK_PERIOD;
        // Phase C: LOOK and LISTEN (`crate::perception`), then pick a target
        // out of what this unit actually perceives — never out of the sim's
        // omniscient unit list, which is what `ENGAGE_RANGE` + `visible` did.
        let (held, heard) = perception::scan(sim, me, st, tier);
        // Card TEAM: then what the friends near by see.
        nt.told = perception::tell(sim, me, st);
        nt.acquired = u8::from(perception::choose_target(sim, me, st, tier));
        // Card DFOCUS: a holder of a human's token at Hard / Elite makes him
        // its target (`crate::ai::dfocus`).
        if let Some(h) = crate::ai::dfocus::pick(sim, me, st) {
            nt.levers |= census::LV_DFOCUS;
            nt.acquired |= u8::from(perception::take_target(st, h, sim.tick));
        }
        // [ours] A picking unit prefers threats (`proxy::pick`).
        if let Some(h) = crate::ai::proxy::pick(sim, me, st) {
            nt.levers |= census::LV_PICK;
            nt.acquired |= u8::from(perception::take_target(st, h, sim.tick));
        }
        // Bot census (diagnostics only, `crate::ai::census`): the target
        // this think holds, and the tier levers it shows. On foot in a
        // ground round.
        if ground && u.mount.is_none() {
            nt.d1 = census::d1(sim, me, st, nt.acquired != 0);
            nt.levers |= census::think_levers(sim, me, u, st, &tier);
            if mend.is_some() {
                nt.levers |= census::LV_MEND;
            }
        }
        nt.contacts = held;
        nt.heard = heard;
        // A target known only by memory (heard, told, or lost from sight) is
        // pitched at only if the sight ray reaches its point: behind a floor
        // or a wall it is faced level (`perception::can_aim`, playtest C4).
        // One ray a think; the aim reads the answer every tick. On foot in
        // a ground round only: a seat's gun is its own (`ride::drive`), and
        // a space round's foot bots keep the aim they had.
        st.memory_blocked = ground
            && u.mount.is_none()
            && st
                .target
                .and_then(|h| perception::contact_of(st, h))
                .filter(|c| !c.seen_now)
                .is_some_and(|c| !perception::can_aim(sim, me, eye, c.last_pos + Vec3::Y * 1.4));
        // Card UCALL: hearing that made a record while no enemy is in sight
        // turns the unit to the sound (B1's hearing check, the fourth of an
        // agent's interrupt checks, `FUN_00252dc8` 171455: re-unit-chatter.md
        // 2.4). PROVISIONAL [H]: the check's two early returns are not
        // traced; "no enemy seen now" stands in for them.
        nt.heard_turn = u8::from(heard > 0 && !st.contacts.iter().flatten().any(|c| c.seen_now));
        // BF1 (D10): the shooter's tracked-position error, once per think.
        if sim.ground_rules != crate::soldier::GroundRules::Bf2 {
            gunner_foot::track_error(sim, st, &skill);
        }
        // The HUNT destination is deliberately NOT bounded by perception
        // (brief D6): it is the "nothing capturable to walk at" fallback, and
        // the commander is omniscient by design too.
        let mut hunt: Option<(f32, Vec3)> = None;
        for (h, o) in sim.alive_units() {
            if h == me || !sim.hostile(u.team, o.team) {
                continue;
            }
            let d2 = (o.capsule().center() - eye).length_squared();
            if hunt.is_none_or(|(bd, _)| d2 < bd) {
                hunt = Some((d2, o.soldier.pos));
            }
        }
        // An order the player gave outranks the commander's plan. A Follow
        // or Hold unit is not in the plan at all, so `retarget` would put it
        // on the Phase A fallback; the command branch runs instead.
        // Card SNPR: a sniper's spot - kept, left when its lane is quiet,
        // picked (`snipe::upkeep`).
        let seen_any = st.contacts.iter().flatten().any(|c| c.seen_now);
        super::snipe::upkeep(sim, me, u, st, seen_any);
        // Card TBLD: a cutter carrier's broken turret (`fix::upkeep`).
        crate::ai::fix::upkeep(sim, me, u, st);
        let fixing = crate::ai::fix::walk(sim, me, st);
        // [ours] A hurt unit breaks off to a health droid (`proxy::mend`).
        match (u.command, mend, ring_walk(sim, st), super::snipe::walk(st)) {
            (Some(c), _, _, _) => crate::squad::steer(sim, me, st, &c, &mut nt),
            (_, Some(station), _, _) => {
                super::route::retarget_board(sim, me, st, station.pos, &mut nt)
            }
            // Card TBLD: a fixer walks to its turret over the graph.
            _ if fixing.is_some() => {
                super::route::retarget_board(sim, me, st, fixing.unwrap_or(eye), &mut nt)
            }
            // Card FGHT: a circling bot walks over the graph to its ring
            // point's hub and straight at the point from there (as a boarder
            // walks to its vehicle).
            (_, _, Some(p), _) => super::route::retarget_board(sim, me, st, p, &mut nt),
            // Card SNPR: a sniper, the same way to its spot.
            (_, _, None, Some(p)) => super::route::retarget_board(sim, me, st, p, &mut nt),
            _ => retarget(sim, me, st, hunt.map(|(_, p)| p), &mut nt),
        }
        if nt.released != 0
            && (st.pending_board.is_some() || st.ride.is_some_and(|r| r.ordered_call))
        {
            st.pending_board = None;
            st.cmd_clear();
        } else {
            super::ride::deliver_call(sim, me, u, st);
        }
        if st
            .ride
            .is_some_and(|r| r.ordered_call || r.player_board.is_some())
        {
            if let Some(at) = super::ride::board_point(sim, st) {
                if st.ride.is_some_and(|r| r.since == sim.tick) {
                    super::route::clear(st);
                    nt.claim = NavTick::CLAIM_NEW;
                }
                super::route::retarget_board(sim, me, st, at, &mut nt);
            }
        }
        // Card VAI: a bot on foot in the commander's pool may go for a
        // vehicle (`ride::claim`); an ordered one stays with its order.
        if u.mount.is_none() && u.command.is_none() {
            let had = super::ride::board_point(sim, st).is_some();
            super::ride::claim(sim, me, u, st);
            // Card WAI2: a new claim routes to its vehicle at once - card
            // TURR: one the offer pass made this tick too.
            let fresh = st.ride.is_some_and(|r| r.since == sim.tick);
            if let Some(at) = super::ride::board_point(sim, st).filter(|_| !had || fresh) {
                nt.claim = NavTick::CLAIM_NEW;
                super::route::retarget_board(sim, me, st, at, &mut nt);
            }
        }
        // Card BNDS: is this bot still on the playable level? Asked on the
        // ground only (a faller keeps what it had); two thinks off running
        // start the return walk, one think on ends it.
        // Card VMESH: one shut inside a WATERTIGHT map body (a closed prop,
        // never a room, cave or tunnel: those have open edges and bots walk
        // in them) is lost too, at once and whatever it is doing; it is
        // given up after `SHUT_RECYCLE`.
        let shut = u.mount.is_none()
            && sim
                .shut_in_solid(u.soldier.pos + Vec3::Y * SHUT_PROBE_UP)
                .is_some();
        if shut {
            st.lost = Some(match st.lost.filter(|l| l.shut) {
                Some(l) => l,
                None => LostState {
                    since: sim.tick,
                    confirmed: true,
                    shut: true,
                    ..LostState::default()
                },
            });
        } else if st.lost.is_some_and(|l| l.pinned) {
            // Card RLOOP: a pinned bot's walk is not the level test's; it
            // ends with the walk (below).
        } else if u.mount.is_none() && u.soldier.grounded {
            st.lost = match (sim.level_at(u.soldier.pos), st.lost.filter(|l| !l.shut)) {
                (crate::sim::Level::Off(_), None) => Some(LostState {
                    since: sim.tick,
                    ..LostState::default()
                }),
                (crate::sim::Level::Off(_), Some(l)) => Some(LostState {
                    confirmed: true,
                    ..l
                }),
                _ => None,
            };
        }
        // Card RETW: a confirmed spell looks for the FLOOR walk back once
        // (`Sim::return_path`), and again from where it stands when the walk
        // is spent with the bot still off — its kept points walked, or a
        // progress window with no progress (`mover::run`) —
        // [`RETURN_PLANS`] times in all; then the hubs.
        if let Some(mut l) = st.lost.filter(|l| l.confirmed && !l.shut && !l.pinned) {
            let spent = l.path.len > 0 && l.path_at >= l.path.len;
            if l.plans == 0 || (spent && l.plans < RETURN_PLANS) {
                l.plans += 1;
                l.path = sim.return_path(u.soldier.pos).unwrap_or_default();
                l.path_at = 0;
                st.lost = Some(l);
            }
        }
        // Card HOLD: the position's upkeep. With nobody perceived any more
        // the fight is over and the bot advances; else it keeps the hint's
        // stance and, every few seconds, may step to another spot near by.
        if let Some(mut h) = st.hold.filter(|h| sim.tick < h.until) {
            // (Card TEAM: what a friend tells keeps nobody on a position.)
            if held > 0 && st.contacts.iter().flatten().any(|c| !c.told) {
                h.last_contact = sim.tick;
            }
            // The holder faces the way the nearest enemy is. The look sets
            // the yaw only (it is aimed as a Travel, which keeps the gun
            // level), and `hunt` itself stays omniscient for the walk. A
            // sniper on its spot watches down its lane instead unless the
            // nearest enemy is in clear sight (card SNPR, `SnipeState::look`;
            // `perception::can_aim`, playtest C4: a BF1 Docks platform sniper
            // facing the fight beneath its floor). Ground rounds only.
            if let Some((_, p)) = hunt {
                let at = p + Vec3::Y * 1.4;
                let lane = st
                    .snipe
                    .cur
                    .filter(|c| c.arrived && c.spot == h.spot && c.look != Vec3::ZERO)
                    .map(|c| c.look);
                h.look = match lane {
                    Some(l)
                        if ground
                            && u.mount.is_none()
                            && !perception::can_aim(sim, me, eye, at) =>
                    {
                        l
                    }
                    _ => at,
                };
            }
            if sim.tick.saturating_sub(h.last_contact) >= HOLD_QUIET {
                h.until = sim.tick;
            } else {
                // (Card TPOS: no shuffle - a fighter's cycle and its next
                // position are the Tactician's, `tactic::fight`.)
                st.want_stance = stance_hold(st.want_stance, h.stance_mask, me, sim.tick);
            }
            st.hold = Some(h);
        }
        // Card TEAM: a bot with the COVER role (`team::board`) guards its
        // position, facing the approach, while no fight holds it.
        if let Some(g) = guard_post(sim, st) {
            let q = &sim.positions.positions[st.role.at as usize];
            let mut h = st
                .hold
                .filter(|h| sim.tick < h.until && h.spot == g)
                .unwrap_or(HoldState {
                    spot: g,
                    goal: g,
                    commit_until: sim.tick + HOLD_COMMIT,
                    ..HoldState::default()
                });
            h.until = sim.tick + THINK_PERIOD + HOLD_CLOSE;
            h.last_contact = sim.tick;
            h.look = st.role.look;
            let d = st.role.look - g;
            h.stance_mask = if q.shields_toward(Vec2::new(d.x, d.z)) {
                crate::commander::MASK_CROUCH
            } else {
                0
            };
            st.hold = Some(h);
        }
    }

    let serve = fix::serve_plan(sim, u, st);
    // Card VAI: a seated bot's think is its seat's (`ride::drive`).
    if let Some(m) = u.mount {
        st.jet.flight = None;
        st.stall = None; // a rider is not on the walker's watch (card RCYC)
        st.mv = MoverState::default();
        st.behaviour = Behaviour::Ride;
        let (pad, nt) = super::ride::drive(sim, me, m, st, nt, thinking);
        return (pad, nt, serve);
    }

    let mut pad = PadState::default();
    // Phase C decision 2 (brief D7): the target and what is KNOWN about it.
    // SEEN → aim at the live unit's lead point and fire once the tier's
    // reaction has passed; MEMORY → aim at the last known position and never
    // fire; nothing perceived → the travelling look.
    let held = st.target.and_then(|h| perception::contact_of(st, h));
    let seen = held.filter(|c| c.seen_now);
    let enemy = seen.and_then(|c| c.handle.live_unit(sim));
    let memory = held.filter(|c| !c.seen_now).map(|c| c.last_pos);
    let droid = seen.and_then(|c| {
        let id = c.handle.droid()?;
        let d = sim.live_droid(id)?;
        Some((d, perception::fireable(&c, sim.tick, tier.reaction_ticks)))
    });
    let turret = seen.and_then(|c| {
        let t = sim.live_turret(c.handle.turret()?)?;
        Some((t, perception::fireable(&c, sim.tick, tier.reaction_ticks)))
    });
    let pos = u.soldier.pos;
    let tk = Tick {
        me,
        u,
        eye,
        wc,
        tier,
        skill,
        rules: super::UnitRules::for_unit(sim, u),
    };
    // D9: the AI sniper distance table's spread, every tick (the gunner's).
    let seen_at = enemy
        .map(|e| e.capsule().center())
        .or(droid.map(|(d, _)| d.pos))
        .or(turret.map(|(t, _)| t.centre()));
    gunner_foot::range_table(&tk, st, seen_at);
    // Card GRAI: the weapon for this target, once a think (the gunner's).
    if thinking {
        match (droid, turret) {
            (Some((d, _)), _) => super::arsenal::think_droid(sim, &tk, st, d),
            (_, Some((t, _))) => super::arsenal::think_turret(sim, &tk, st, t),
            _ => super::arsenal::think(sim, &tk, st, enemy),
        }
    }

    let leg_hub = st.route().filter(|r| !r.is_last()).map(|r| r.next_hub);
    advance_route(sim, me, st, pos, 0.0, &mut nt);
    // Card HOLD: a hub just reached settles a hold that has run out.
    if leg_hub.is_some_and(|h| st.route().map(|r| r.next_hub) != Some(h)) {
        hub_reached(sim, st);
    }
    // ...and a grenade on the ground near by is a thing to run from.
    let flee = hazard_flee(sim, me, u, &tier);
    if thinking {
        if flee.is_some() {
            nt.levers |= census::LV_GRENADE_FLEE;
        } else if !sim.hazards.is_empty() && census::hazard_near(sim, me, u) {
            nt.levers |= census::LV_GRENADE_IGNORE;
        }
    }
    // The on-hit reaction as a ROLL (below): the side, this tick only.
    let mut roll: i8 = 0;

    // OURS (playtest C1): a hit by an enemy is a flinch, a short hold of the
    // trigger (`gunner_foot::run`), at most once per FLINCH_GAP. It runs
    // beside the moves below and the hit search (`perception::search_ticks`).
    if u.last_hit.is_some_and(|(s, t)| {
        sim.tick > 0
            && t == sim.tick - 1
            && sim.unit(s).is_some_and(|o| sim.hostile(u.team, o.team))
    }) && perception::hit_reaction_applies(sim, u)
        && (st.flinch_until == 0 || sim.tick >= st.flinch_until + (FLINCH_GAP - FLINCH_TICKS))
    {
        st.flinch_until = sim.tick + FLINCH_TICKS;
        nt.levers |= census::LV_FLINCH;
    }

    // OURS: soldier dodge on hit. FUN_0020a940 (B2:121236-121318)
    // belongs to flyer/carrier vtables (0x61a94c/0x61c1cc/0x62b39c),
    // not the soldier; its evasive re-plan does not establish this step.
    // Keep the current skill coin, 2 s cooldown and alternating side-step.
    // measure: PS2 soldier hit reactions in PCSX2 T5/T6 before replacing
    // this openbf deviation (analysis/re-ai-difficulty.md §9.4).
    // Card REACT: a bot on a position (a hub's, a fight's) answers the hit
    // by its tier's row instead (`react`).
    if u.last_hit
        .is_some_and(|(_, t)| sim.tick > 0 && t == sim.tick - 1)
        && sim.tick >= st.dodge_until
    {
        if st.fight.tactician() {
            // Card TPOS: a hit counts against the position (`tactic::hit`).
            super::tactic::hit(sim, &tk, st);
            nt.levers |= census::LV_POSITION_HIT;
        } else if let Some(h) = st.hold.filter(|h| sim.tick < h.until) {
            roll = react(sim, &tk, st, h, enemy, &mut nt);
        } else if unit_unit(me.index, sim.tick, me.gen, SALT_DODGE) < skill.dodge_chance {
            let side: i8 = if st.slide > 0 { -1 } else { 1 };
            st.dodge_until = sim.tick + DODGE_COOLDOWN;
            nt.levers |= census::LV_DODGE;
            // Nothing to offset: strafe the pad directly for the same window.
            if !super::route::dodge(st, side) {
                st.strafe_until = sim.tick + SIDESTEP_TICKS as u64;
                st.strafe_side = side;
            }
        }
    }

    // Card REACT: the roll a reaction asked for, now that the bot is under
    // way to where it ends.
    if let Some(mut h) = st.hold.filter(|h| h.roll_at != 0 && sim.tick >= h.roll_at) {
        roll = h.strafe_side;
        h.roll_at = 0;
        st.hold = Some(h);
    }

    let route_leg = st.route().filter(|r| !r.is_last());
    let post = st.target_cp.and_then(|i| sim.cps.get(i as usize));

    // The point this tick walks at: the current detour corner, else the leg's
    // waypoint, else (no graph at all) the target post. A corner is XZ only —
    // walking is XZ and the gun is levelled while travelling — so it takes the
    // unit's own y and costs no ground query.
    let corner = st.route().and_then(|r| r.corner());
    // Card DFOCUS: a focuser that lost sight of its human walks to where he
    // was last seen, when that is near (`dfocus::push`).
    let push = crate::ai::dfocus::push(sim, me, st, pos).filter(|_| st.cmd.is_none());
    if thinking && push.is_some() {
        nt.levers |= census::LV_PUSH;
    }
    let goal: Option<Vec3> = match (st.route(), st.cmd) {
        _ if push.is_some() => push,
        // On an arc leg, along the arc's line (`leg_point`) — but not while
        // the steering is going round something (`slide`): a wall on the line
        // is passed by leaving the line, and pulling back to it flipped the
        // slide's side every few ticks.
        (Some(r), _) => Some(if st.slide == 0 {
            let v = u.soldier.vel;
            leg_point(&r, pos, Vec2::new(v.x, v.z).length())
        } else {
            sub_point(&r, pos)
        }),
        // A COMMANDED unit with no route walks at the point its order gives
        // it — the leader, its standing point by him, or the spot it holds —
        // and stands once there. It has no post (the command branch clears
        // `target_cp`), so this arm is the whole of its walking.
        (None, Some(c)) => {
            Some(c.goal).filter(|g| nav::xz_dist(pos, *g) > crate::squad::STAND_ARRIVE)
        }
        // Brief D7: with no route leg and no post to walk at, a unit that has
        // LOST sight of its target walks at the last place it knew of, until
        // the memory expires. Nothing else about walking changes — an engaged
        // unit still walks its route (A.1 decision 4).
        (None, None) => post.map(|cp| cp.def.pos).or(memory),
    };
    // Card VAI: a boarding claim walks the bot at its vehicle (its own
    // give-up clock, not the route's stuck window, ends a claim that cannot
    // get there) — card WAI2: over the graph to the vehicle's hub
    // (`route::retarget_board`), straight at it on the last leg or with no
    // route.
    let boarding = super::ride::board_goal(sim, me, u, st, &mut nt);
    let item = crate::ai::items::goal(sim, u, st).filter(|_| flee.is_none());
    let goal = match boarding {
        Some(_) if st.route().is_some_and(|r| !r.is_last()) => goal,
        _ => boarding.or(goal),
    };
    // Card BNDS: a bot off the playable level walks BACK to it before
    // anything else — at the nearest hub it can stand level with
    // (`Sim::return_point`), the next one after a window with no progress.
    // Off it for `LOST_RECYCLE` with no way back, it is given up.
    // Card RETW: ...along the floor walk found for it when there is one
    // (each of its points reached in turn), and only then at a hub.
    if let Some(mut l) = st.lost.filter(|l| l.confirmed) {
        while l.path_at < l.path.len && {
            let p = l.path.pts[usize::from(l.path_at)];
            nav::xz_dist(pos, p) <= RETURN_ARRIVE && (pos.y - p.y).abs() <= RETURN_ARRIVE_DY
        } {
            // (A fresh progress window for the next point.)
            l.path_at += 1;
            l.window = 0;
            l.ref_dist = 0.0;
        }
        st.lost = Some(l);
    }
    // Card RLOOP: a pinned bot at the end of its walk is at its waypoint;
    // its route goes on from there.
    if st.lost.is_some_and(|l| l.pinned && l.path_at >= l.path.len) {
        st.lost = None;
    }
    let lost = st.lost.filter(|l| l.confirmed);
    let ret = lost.and_then(|l| {
        if l.path_at < l.path.len {
            Some(l.path.pts[usize::from(l.path_at)])
        } else {
            sim.return_point(pos, l.tries)
        }
    });
    if let Some(l) = lost {
        let after = if l.shut { SHUT_RECYCLE } else { LOST_RECYCLE };
        let over = sim.tick.saturating_sub(l.since) >= after;
        if l.pinned {
            // (On the level: given up as the last resort's, not as lost.)
            nt.stalled |= u32::from(over);
        } else {
            nt.lost = 1;
            nt.recycle |= u32::from(over);
        }
    }
    // These urgent walks exclude each optional positioning request below.
    let no_urgent_move = ret.is_none() && flee.is_none() && boarding.is_none() && item.is_none();
    // Card FGHT: the ring point a circling bot walks to — over the graph,
    // and straight at it on the last leg and with no route.
    let ring = ring_walk(sim, st).filter(|_| no_urgent_move && st.cmd.is_none() && mend.is_none());
    let goal = match ring {
        Some(p) if st.route().is_none_or(|r| r.is_last()) => Some(p),
        _ => goal,
    };
    let at_ring = ring.filter(|p| goal == Some(*p));
    // Card SNPR: the sniper's spot - over the graph, and straight at it on
    // the last leg and with no route.
    let spot = super::snipe::walk(st)
        .filter(|_| no_urgent_move && st.cmd.is_none() && ring.is_none() && mend.is_none());
    let goal = match spot {
        Some(p) if st.route().is_none_or(|r| r.is_last()) => Some(p),
        _ => goal,
    };
    let at_spot = spot.filter(|p| goal == Some(*p));
    // Card TBLD: a fixer's stand by its turret - over the graph, and
    // straight at it on the last leg and with no route; ahead of the ring
    // and the spot (it walks only while no enemy is seen).
    let fixing = crate::ai::fix::walk(sim, me, st).filter(|_| no_urgent_move && st.cmd.is_none());
    let goal = match fixing {
        Some(p) if st.route().is_none_or(|r| r.is_last()) => Some(p),
        _ => goal,
    };
    let at_fix = fixing.filter(|p| goal == Some(*p));
    // OURS: finish the walk at the droid's actual service sphere after the
    // graph has brought us to its hub. measure: Harbor Hard/Elite service
    // and recycles at the droid hubs (issue #108).
    let mend_approach = mend.filter(|station| {
        no_urgent_move
            && fixing.is_none()
            && push.is_none()
            && u.command.is_none()
            && st.cmd.is_none()
            && match (st.route(), sim.nav.as_ref()) {
                (Some(r), Some(g)) if r.is_last() && r.corner().is_none() => {
                    use super::route::{mover, start_hub, unit_goal_hub};
                    let hub = unit_goal_hub(sim, g, mover(sim, u), station.pos);
                    // A straight fallback can be the last leg before the
                    // bot has reached the station's hub.
                    hub == Some(r.dest_hub)
                        && (r.source != RouteSource::Straight || start_hub(sim, g, me) == hub)
                }
                (None, None) => true,
                _ => false,
            }
    });
    let goal = match mend_approach {
        Some(s) => (!s.reaches(pos)).then_some(s.pos),
        None => goal,
    };
    // Card HOLD: the position held (when nothing more urgent walks the bot).
    let holding = st.hold.filter(|h| {
        sim.tick < h.until && no_urgent_move && ring.is_none() && fixing.is_none() && mend.is_none()
    });
    // Diagnostic (card BLKF): which link of the chain below walks the bot.
    let walk_kind = match () {
        _ if ret.is_some() => WalkAsk::RETURN,
        _ if flee.is_some() => WalkAsk::FLEE,
        _ if item.is_some() => WalkAsk::PICKUP,
        _ if holding.is_some() => WalkAsk::HOLD,
        _ if boarding.is_some() && goal == boarding => WalkAsk::BOARD,
        _ if mend_approach.is_some() => WalkAsk::MEND,
        _ => WalkAsk::LEG,
    };
    if thinking && boarding.is_some() && ret.is_none() && flee.is_none() {
        super::ride::maintain(sim, me, st);
    }
    // Card MOVR: where the walk ENDS, when it ends somewhere — a held
    // position, the last leg's point (the last-node radius), a post or a
    // remembered enemy walked straight at. A commanded bot's stand is its
    // order's (`squad::STAND_ARRIVE`, above).
    let arrive = match walk_kind {
        WalkAsk::PICKUP => item.map(|p| (p, 0.2)),
        WalkAsk::HOLD => holding.map(|h| (h.goal, HOLD_ARRIVE)),
        // (Card TPOS: a position is walked onto, as a held one is.)
        WalkAsk::LEG if fixing.is_some() => at_fix.map(|p| (p, HOLD_ARRIVE)),
        WalkAsk::LEG if ring.is_some() && st.fight.mode == FightState::TAKE => {
            at_ring.map(|p| (p, HOLD_ARRIVE))
        }
        WalkAsk::LEG if ring.is_some() => at_ring.map(|p| (p, RING_ARRIVE)),
        WalkAsk::LEG if spot.is_some() => at_spot.map(|p| (p, RING_ARRIVE)),
        WalkAsk::LEG if push.is_some() && goal == push => push.map(|p| (p, CORNER_ARRIVE)),
        WalkAsk::LEG => match (st.route(), st.cmd) {
            (_, Some(_)) => None,
            (Some(r), None) => r.is_last().then_some((r.waypoint, CORNER_ARRIVE)),
            (None, None) => goal.map(|g| (g, CORNER_ARRIVE)),
        },
        _ => None,
    };
    let goal = ret.or(flee).or(item).or(holding.map(|h| h.goal)).or(goal);

    // Playtest C3: a bot walking somewhere faces its walk unless it can
    // fight what it looks at. The PS2 gives no aim order outside an attack
    // (`analysis/re-soldier-ai.md` 1.5), ends one past the weapon's range
    // + 10 m (4.2), never attacks a vehicle its weapons cannot hurt (4.3)
    // and shares no contacts (6). So a rider no weapon of the bot's hurts,
    // a target beyond its reach, a friend's report and a remembered rider
    // are not faced on the walk (they stay its target). A bot that has
    // arrived, holds or takes a position, and a space round's, aim as before.
    let walking_on = goal.is_some()
        && !st.mv.arrived
        && holding.is_none()
        && ring.is_none()
        && wc.saber.is_none()
        && u.proxy_human.is_none()
        && u.droideka_class().is_none()
        && !sim.space_round();
    let reach = fight_reach(sim, u);
    let within = |p: Vec3| (p - eye).length_squared() <= reach * reach;
    let faced = enemy.zip(seen).filter(|(e, _)| {
        !walking_on || (within(e.capsule().center()) && (e.mount.is_none() || !st.arm.spare))
    });
    let faced_memory = memory.filter(|p| {
        !walking_on
            || goal == Some(*p)
            || held.is_some_and(|c| {
                !c.told
                    && c.handle.live_unit(sim).is_none_or(|o| o.mount.is_none())
                    && within(c.last_pos)
            })
    });
    let walks_past = (enemy.is_some() && seen.is_some() && faced.is_none())
        || (memory.is_some() && faced_memory.is_none());

    // Where to look: the engaged enemy's lead point, else where we are
    // walking, else the target post (the last leg, and the no-graph
    // placeholder).
    let aim = if let Some((d, ready)) = droid {
        AimRequest::Droid(d, ready)
    } else if let Some((t, ready)) = turret {
        AimRequest::Turret(t, ready)
    } else {
        match (faced, faced_memory, corner, route_leg) {
            // A seen enemy: the gunner's own aim point (its lead point under the
            // D10 error), and the only target it fires on.
            (Some((e, c)), _, _, _) => AimRequest::Enemy(e, c),
            // A remembered contact is aimed at where it was last known to be —
            // a point on the target, no lead (the PS2 aims at a point, and there
            // is no velocity to lead with once the eyes are off it). A heard
            // shot, a friend's report or a lost contact behind geometry is a
            // direction to face, not a point to pitch at: the gun stays level
            // (`perception::can_aim`, playtest C4).
            (None, Some(p), _, _) if st.memory_blocked => {
                AimRequest::Travel(perception::level(eye, p + Vec3::Y * 1.4))
            }
            (None, Some(p), _, _) => AimRequest::Memory(p + Vec3::Y * 1.4),
            // Card HOLD: a holder standing on its point with nobody in sight
            // watches the way the nearest enemy is.
            (None, None, _, _)
                if holding.is_some_and(|h| {
                    h.look != Vec3::ZERO && nav::xz_dist(pos, h.goal) <= HOLD_ARRIVE
                }) =>
            {
                AimRequest::Travel(holding.map_or(pos, |h| h.look))
            }
            (None, None, _, _) if item.is_some() => {
                AimRequest::Travel(goal.unwrap_or(pos) + Vec3::Y * 1.4)
            }
            (None, None, Some(_), _) => AimRequest::Travel(goal.unwrap_or(pos)),
            // Running from a grenade, or walking back to the level: where it goes.
            (None, None, None, _) if flee.is_some() || ret.is_some() => {
                AimRequest::Travel(goal.unwrap_or(pos) + Vec3::Y * 1.4)
            }
            // A boarder on its last leg looks at the vehicle it walks to, not
            // at its post (playtest C3: it walked there sideways); ground
            // rounds only. PILEIN can send an uncommanded bot to a vehicle
            // without a post or graph; its approach must not take the idle
            // fallback either.
            (None, None, None, None)
                if boarding.is_some() && goal == boarding && !sim.space_round() =>
            {
                AimRequest::Travel(goal.unwrap_or(pos) + Vec3::Y * 1.4)
            }
            (None, None, None, None)
                if boarding.is_some() && st.ride.is_some_and(|r| r.player_board.is_some()) =>
            {
                AimRequest::Travel(goal.unwrap_or(pos) + Vec3::Y * 1.4)
            }
            // Aim at the turret or droid while approaching or servicing it.
            (None, None, None, None) if fixing.is_some() || mend_approach.is_some() => {
                let at = mend_approach.map_or_else(|| goal.unwrap_or(pos), |s| s.pos);
                AimRequest::Travel(at + Vec3::Y * 1.4)
            }
            (None, None, None, Some(r)) => AimRequest::Travel(r.waypoint),
            // (Playtest C3: a bot walking past what it does not face, with
            // no leg and no post, looks where it walks rather than stopping.)
            (None, None, None, None) if walks_past => {
                AimRequest::Travel(goal.unwrap_or(pos) + Vec3::Y * 1.4)
            }
            (None, None, None, None) => AimRequest::Travel(match post {
                Some(cp) => cp.def.pos + Vec3::Y * 1.4,
                // Hunting on the last leg: the destination hub itself.
                None => match st.route() {
                    Some(r) => r.waypoint,
                    // D6: a commanded unit LOOKS where it walks — the idle arm
                    // below is for a unit with no route, no post AND no order.
                    None => match st.cmd {
                        Some(c) => c.goal + Vec3::Y * 1.4,
                        None => {
                            st.steer = SteerPick::Idle;
                            st.behaviour = Behaviour::None;
                            st.jet.cancel(u.soldier.stance);
                            return (pad, nt, serve);
                        }
                    },
                },
            }),
        }
    };
    // The gunner: the look sticks and the trigger. With nothing to turn to
    // (the aim point is the muzzle itself) the tick ends here.
    let Some(gun) = gunner_foot::run(sim, &tk, st, &aim, &mut pad, &mut nt) else {
        st.steer = SteerPick::Idle;
        st.behaviour = Behaviour::None;
        st.jet.cancel(u.soldier.stance);
        return (pad, nt, serve);
    };
    // Card FFLN: a friend in the line of fire. The shot waits (the gunner), and a
    // holder whose last step's commit time is over steps aside, away from the
    // friend, through its goal's one writer. A bot walking its leg walks on:
    // the line clears by itself. Card TPOS: not a bot on a Tactician's
    // position - those are chosen out of the friends' lines - but still the
    // fallback's stander (without it, BF2 Hoth's fighters with a teammate
    // within 1.5 m went 3.6 -> 5.8 % over 8 seeds; with it 3.6 %). No strafe
    // on a new burst any more.
    if let (Some(side), Some(mut h), false) = (gun.line, holding, st.fight.tactician()) {
        let (sy, cy) = fmath::sin_cos(u.soldier.yaw);
        let p = pos + Vec3::new(cy, 0.0, -sy) * (f32::from(side) * HOLD_STRAFE);
        if sim.tick >= h.commit_until
            && react_ok(sim, u, p)
            && spot_free(sim, me, p)
            && h.step_to(p, sim.tick, false)
        {
            h.strafe_side = side;
            st.hold = Some(h);
        }
    }
    // The holder as it is now (the dodge and the step out of a friend's line
    // above may have moved its goal this tick).
    let holding = holding.and(st.hold);
    // A seen enemy inside [`HOLD_RANGE`]: stand and fight (a remembered one
    // never holds the bot: the design's "the unit may move toward it").
    let hold = gun.range.is_some_and(|r| r <= HOLD_RANGE);
    // Card FGHT: FIGHT — a seen target inside the weapon's range + 10 m
    // (`analysis/re-soldier-ai.md` 4.2: the combat helper's attack). A
    // RIDER only as the vehicle it rides, and only by a bot whose weapon
    // choice found one that hurts it with the hull in its band (card HVYW,
    // `ArmState::vehicle`: the launcher against a tank; a grenade is thrown
    // in passing): a bot that took a position against one it
    // cannot hurt kept it for good (BF1 Rhen Var, seed 4: twelve bots round
    // one gunner seat, no outcome in 60,000 ticks) — it walks on.
    let fight = enemy.zip(gun.range).filter(|(e, r)| {
        (e.mount.is_none() || st.arm.vehicle)
            && *r <= wc.ai_range[2] + FIGHT_BEYOND
            && crate::ai::droideka::takes_fight(sim, u, st, e, *r)
    });
    let objective = sim.commander[usize::from(u.team).min(1)].of(me.index);
    let defending = matches!(objective, Some(crate::commander::Objective::Defend(_)));
    // Card FGHT (it was card HOLD's close hold, "stand where it is while he
    // is inside 25 m"): a free bot with a target in reach fights from a
    // position of its style (`fight_position`), which lasts [`HOLD_CLOSE`]
    // past the last tick the target was in reach — the 2 s a fight lasts at
    // least, and the 2 s after which it is given up.
    let free =
        ret.is_none() && flee.is_none() && boarding.is_none() && item.is_none() && st.cmd.is_none();
    let melee_goal = if free
        && wc.saber.is_some()
        && !st.arm.spare
        && st.arm.pick.is_none_or(|slot| slot == u.weapon_slot)
    {
        enemy.filter(|e| e.mount.is_none()).map(|target| {
            let state = st
                .melee
                .get_or_insert_with(|| super::super::melee::MeleeBrain::begin(me, sim.tick));
            super::super::melee::goal(sim, me, u, target, state)
        })
    } else {
        None
    };
    if melee_goal.is_some() {
        super::tactic::release(st, sim.tick);
        st.hold = None;
        st.fight.mode = 0;
        super::route::clear(st);
        st.want_stance = 0;
    } else if wc.saber.is_none() {
        st.melee = None;
    }
    // Card TPOS: hidden at its position a bot does not see its target - the
    // hide phase's point - and it keeps the position while it remembers it
    // (without this the fight lapsed 2 s into the hide and the bot walked
    // off, then back: the Kamino playtest).
    // Card TEAM: not on a told record - a friend's sight is aimed at, it
    // does not keep the bot at a position it cannot fire from.
    // Card TPOSR: and the Tactician runs on the remembered target as on a
    // seen one - the walk there, the hide / fire cycle, the leave tests (hit
    // twice, round its cover, out of the band, the 5-12 s timer): the PS2's
    // cover state machine runs every update, its walk and hidden states
    // keeping the target (re-soldier-ai 4.6; AUDIT TPOS: a bot hit twice kept
    // its position 22 s, hidden, while its target was out of sight).
    if let (true, Some(c), true, Some(mut h)) = (
        st.fight.tactician(),
        held.filter(|c| !c.seen_now && !c.told),
        free,
        st.hold,
    ) {
        let foe = super::tactic::Foe::remembered(c.last_pos);
        if super::tactic::fight(sim, &tk, st, &foe, &mut h, thinking, &mut nt) {
            h.until = h.until.max(sim.tick + HOLD_CLOSE);
        }
        st.hold = Some(h);
    }
    let manning = boarding.is_some()
        && st
            .ride
            .and_then(|r| r.board)
            .is_some_and(|vh| super::man::is_turret(sim, vh));
    let close_on = gun.close_on.filter(|_| free);
    if close_on.is_some() {
        super::tactic::release(st, sim.tick);
        st.hold = None;
        st.fight.mode = 0;
    }
    match fight.filter(|_| free && melee_goal.is_none() && close_on.is_none()) {
        Some((e, r)) => fight_position(sim, &tk, st, e, r, thinking, &mut nt),
        None if !free || st.hold.is_none_or(|h| sim.tick >= h.until) => {
            if st.fight.mode == FightState::TAKE {
                nt.tpos_ev |= super::tactic::EV_WALK_DROPPED;
            }
            super::tactic::release(st, sim.tick);
            st.fight.mode = 0;
        }
        None => {}
    }
    // Card TPOSR: the position held stays reserved while it is held.
    super::tactic::keep(sim, st, thinking, &mut nt);
    // Card RCYC: who the last resort (the mover's progress watch) never
    // gives up — a bot on a held position, fighting an enemy it sees,
    // running from a grenade or under a squad order.
    let excused = holding.is_some()
        || ring.is_some()
        || flee.is_some()
        || enemy.is_some()
        || st.cmd.is_some();
    // The holder's goal may have moved this tick: its destination with it.
    let arrive = match (walk_kind, holding) {
        (WalkAsk::HOLD, Some(h)) => Some((h.goal, HOLD_ARRIVE)),
        _ => arrive,
    };
    let goal = match (walk_kind, holding) {
        (WalkAsk::HOLD, Some(h)) => Some(h.goal),
        _ => goal,
    };
    let mut mv = MoveRequest {
        kind: walk_kind,
        goal,
        arrive,
        // A ring point, pickup or health droid has its own destination.
        post: post.filter(|_| ring.is_none() && item.is_none() && mend_approach.is_none()),
        defending,
        // The stand of a boarder or a commanded bot with an enemy inside
        // 25 m — and of a free bot on the tick it takes its fight position
        // (from the next it is `holding`, or walks to its ring point). Card
        // TURR: not of a bot walking to a turret, whose gun is the fight (it
        // stood 0.4 m out of reach shooting the enemy it was sent to man
        // the turret against).
        in_range: (hold && !free && !manning && item.is_none())
            || (fight.is_some() && free && ring_walk(sim, st).is_none()),
        ret,
        hold: holding,
        flee: flee.is_some(),
        boarding: boarding.is_some(),
        serving: serve.is_some(),
        excused,
        acts: Acts {
            roll,
            melee_jump: st.melee.is_some_and(|m| m.jump_tick == Some(sim.tick)),
            // Card HVYW (OURS): ... but not for a throw at a VEHICLE that cannot
            // be made from here - the grenade is the one weapon most bots have
            // for a tank and no 3 s window ends that choice, so a bot under a
            // turret its arc does not reach stood until it was shot (BF1 Rhen
            // Var, seed 3: 40 s). measure: stock movement when the anti-vehicle throw arc is blocked.
            throw_stand: item.is_none()
                && enemy.is_some_and(|e| e.mount.is_none() || !gun.no_throw)
                && matches!(
                    super::arsenal::armed(sim, u, st),
                    super::arsenal::Armed::Thrown(..)
                ),
        },
    };
    if let Some(goal) = melee_goal.or(close_on) {
        mv.kind = WalkAsk::LEG;
        mv.goal = Some(goal);
        mv.arrive = None;
        mv.post = None;
        mv.in_range = false;
        mv.hold = None;
        mv.acts.throw_stand = false;
    }
    // The mover: the move sticks, the stance, the roll and the jump. It
    // answers whether the bot stands at its post.
    let moved = mover::run(sim, &tk, st, &mv, &mut pad, &mut nt);
    let arrived_at_post = moved.at_post;
    // Card MOVR: every walk has an outcome, and a failed one is handled.
    match moved.walk {
        // Card FGHT: at the ring point, or where the way to it is shut: the
        // circle's pause, where the bot stands.
        // Card TPOS: at the Tactician's position its cycle starts; a way
        // there that is shut leaves it (the next think asks again).
        Walk::Arrived if ring.is_some() && st.fight.mode == FightState::TAKE => {
            if let Some(mut h) = st.hold {
                if super::tactic::walk_arrived(sim, &tk, st, &mut h) {
                    nt.tpos_ev |= super::tactic::EV_WALK_ARRIVED;
                    st.hold = Some(h);
                }
            }
        }
        // (Given up near enough, it is there where it stopped.)
        Walk::Failed if ring.is_some() && st.fight.mode == FightState::TAKE => {
            let mut h = st.hold.unwrap_or_default();
            if super::tactic::cut_short(sim, &tk, st, &mut h) {
                nt.tpos_ev |= super::tactic::EV_WALK_NEAR;
                st.hold = st.hold.map(|_| h);
            } else {
                nt.tpos_ev |= super::tactic::EV_WALK_GIVEN_UP;
                if let Some(h) = st.hold.as_mut() {
                    h.settle(pos);
                }
                st.fight.mode = FightState::STAND;
            }
        }
        // (Not a bot already at its Tactician's position: `tactic::fight`
        // ended the walk there this tick.)
        Walk::Arrived | Walk::Failed if ring.is_some() && !st.fight.tactician() => {
            if let Some(h) = st.hold.as_mut() {
                h.settle(pos);
            }
            st.fight.mode = FightState::PAUSE;
            st.fight.pause_until = sim.tick + CIRCLE_PAUSE;
        }
        // Card SNPR: at the sniper's spot, or the way to it shut.
        Walk::Arrived if at_spot.is_some() && holding.is_none() => {
            super::snipe::arrive(sim, me, st);
        }
        Walk::Failed if fixing.is_some() => {
            crate::ai::fix::walk_failed(sim, st);
            recover(sim, st, &mut nt);
        }
        Walk::Failed if spot.is_some() && holding.is_none() => {
            super::snipe::walk_failed(sim, st);
            recover(sim, st, &mut nt);
        }
        Walk::Failed if item.is_some() => st.items.detour = None,
        Walk::Failed => recover(sim, st, &mut nt),
        _ => {}
    }
    // A bot that has arrived is not recovering any more.
    if moved.walk == Walk::Arrived {
        st.mv.recover = 0;
    }
    let was = st.behaviour;
    // Card WHY: each link names why it runs; a fight by what put the target
    // there (a hit in the last second, a seen one, else the memory).
    let hit = u
        .last_hit
        .is_some_and(|(_, t)| sim.tick.saturating_sub(t) < u64::from(crate::TICK_HZ));
    let fought = match () {
        _ if hit => Why::Hit,
        _ if enemy.is_some() => Why::Seen,
        _ => Why::Memory,
    };
    let why;
    (st.behaviour, why) = match moved.walk {
        _ if ret.is_some() => (Behaviour::Recover, Why::OffLevel),
        _ if flee.is_some() => (Behaviour::Evade, Why::Grenade),
        _ if item.is_some() => (Behaviour::Pickup, Why::Pickup),
        _ if super::snipe::holds(st, holding) => (Behaviour::Overwatch, Why::SnipeSpot),
        // Card TEAM: on its guard position (the Cover role).
        _ if holding.is_some_and(|h| guard_post(sim, st) == Some(h.spot)) => {
            (Behaviour::Guard, Why::GuardSpot)
        }
        _ if holding.is_some() || ring.is_some() => (Behaviour::Fight, fought),
        _ if walk_kind == WalkAsk::BOARD || boarding.is_some() => (Behaviour::Board, Why::Claim),
        _ if st.cmd.is_some() => (Behaviour::Follow, Why::Order),
        Walk::Arrived if post.is_some() && defending => (Behaviour::Guard, Why::Arrived),
        Walk::Arrived if post.is_some() => (Behaviour::Capture, Why::Arrived),
        _ if hold || (fight.is_some() && free) => (Behaviour::Fight, fought),
        Walk::Idle => (Behaviour::None, Why::Idle),
        // (The log names the failure that put it there.)
        _ if st.mv.recover > 0 => (Behaviour::Recover, Why::None),
        _ => (
            Behaviour::Travel,
            match was {
                Behaviour::Fight => Why::TargetLost,
                Behaviour::Evade => Why::Safe,
                Behaviour::Recover => Why::Recovered,
                Behaviour::Board => Why::ClaimEnded,
                _ => Why::SetOut,
            },
        ),
    };
    nt.why = why;
    // Card UTALK: the trooper agent's two talking points (B1 `FUN_0024b7a0`
    // 165905: it starts away from its goal; `FUN_0024baf0` 166190: it finds
    // itself at the goal - re-squad-orders.md 5). The port's trooper is the
    // walk to the post: set out from any other behaviour, or arrive.
    nt.trooper = match (was, st.behaviour) {
        (w, Behaviour::Travel) if w != Behaviour::Travel => NavTick::SET_OUT,
        (Behaviour::Travel, Behaviour::Guard | Behaviour::Capture) => NavTick::ARRIVED,
        _ => 0,
    };
    // The DWELL counter (Phase B diagnostics): consecutive ticks spent within
    // DWELL_RADIUS of the post of an objective this unit has not completed. It
    // is small for a unit walking past its post and large only for one that
    // stands beside an objective it cannot finish — a capture volume above it,
    // a door it cannot open — which is what the acceptance asks about.
    let near_objective = objective
        .and_then(|o| o.cp())
        .and_then(|c| sim.cps.get(c as usize))
        .is_some_and(|cp| nav::xz_dist(pos, cp.def.pos) <= crate::commander::DWELL_RADIUS);
    st.unreachable_dwell = if near_objective && !arrived_at_post {
        st.unreachable_dwell.saturating_add(1)
    } else {
        0
    };
    nt.dwell = st.unreachable_dwell;
    (pad, nt, serve)
}

/// The farthest a bot on foot fights: the longest AI range of the weapons it
/// holds and may use plus [`FIGHT_BEYOND`] (playtest C3; `re-soldier-ai.md`
/// 4.2). An award weapon it has no medal for does not count.
pub(crate) fn fight_reach(sim: &Sim, u: &crate::unit::Unit) -> f32 {
    let range = |c: usize| sim.weapon_classes.get(c).map_or(0.0, |w| w.ai_range[2]);
    let held = u
        .held
        .iter()
        .filter(|h| !sim.weapon_locked(u, h.class))
        .map(|h| range(h.class));
    held.fold(range(u.weapon_class), crate::fmath::max) + FIGHT_BEYOND
}

/// Card TEAM: the guard position of a bot with the Cover role, while no
/// fight holds it.
fn guard_post(sim: &Sim, st: &AiState) -> Option<Vec3> {
    st.role
        .post
        .filter(|_| st.fight.mode == 0)
        .and_then(|_| sim.positions.positions.get(st.role.at as usize))
        .map(|q| q.pos)
}

/// Card MOVR — RECOVER (`analysis/bot-design.md` 6.3): the walk has FAILED
/// again (the mover's progress window; `MoverState::recover` counts the
/// failures since the walk last made progress). The ladder:
///
/// 1. plan again from the hub the bot stands in and side-step — the window's
///    own cure (`route::progress`, `route::replan`), every failure;
/// 2. (card RETW: back along the floor to a hub it can stand level with);
/// 3. on every [`GIVE_UP_AFTER`]-th failure give the goal up — a boarder
///    drops its claim; any other reports its post and the commander draws
///    again without it (`commander::plan`, `MoverState::give_up`). Not
///    "stand where it is, in the post's region": a bot kept under its post's
///    floor would hold the post contested for good (card STEND);
/// 4. [`PIN_FAILS`](super::PIN_FAILS) failures at one place, whatever the
///    plan: the bot walks to its waypoint along the floor, or is recycled
///    when there is no such walk (card RLOOP, `stall::send_off`) — this count
///    starts again with every plan that is nearer its waypoint, so two
///    plans taken in turn never reached rung 3;
/// 5. the 120 s recycle (card RCYC, `stall::watch`), counted by the sweep.
fn recover(sim: &Sim, st: &mut AiState, nt: &mut NavTick) {
    st.mv.recover = st.mv.recover.saturating_add(1);
    if !st.mv.recover.is_multiple_of(GIVE_UP_AFTER) {
        return;
    }
    if st.ride.is_some_and(|r| r.board.is_some()) {
        super::ride::give_up(sim, st, nt);
    } else if let (Some(cp), None) = (st.target_cp, st.cmd) {
        st.mv.give_up = cp + 1;
    } else {
        return;
    }
    // Card WHY: the goal given up is this failure's outcome.
    nt.failed(Why::GaveUp);
}

/// Card HOLD: hub `hub` has just been reached: a hold that has run out is
/// dropped, or pays one of the hubs it owes. Card TPOS: no bot takes a hold
/// at a hub any more because an enemy is about (card HOLD's 10-24 s dwell at
/// a hint or a drawn hub point) - a fight's position is the Tactician's.
/// Those holders were 11-24 % of Yavin 2's, Utapau's and Polis Massa's fight
/// time with no position, standing on a target they did not see; without
/// them B12 rose on 7 of 8 maps and kills a minute x1.06 (8 seeds).
fn hub_reached(sim: &Sim, st: &mut AiState) {
    if let Some(mut h) = st.hold {
        if sim.tick < h.until {
            return;
        }
        h.skip = h.skip.saturating_sub(1);
        st.hold = (h.skip > 0).then_some(h);
    }
}
