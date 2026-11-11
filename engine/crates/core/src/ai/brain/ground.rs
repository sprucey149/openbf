//! GBOARD/WGORD call targets are consumed only at scheduled thinks over retained orders.
//! Foot-bot tick priority chain, valid boarding maintenance (CWAIT), and failed-walk recovery.

use crate::ai::brain::behaviours::{self, board, fight, melee, order, pickup, recover, travel};
use crate::ai::brain::doing;
use crate::ai::brain::intent::{self, Acts, Intent};
use crate::ai::brain::selector::{self, StepKind};
use crate::ai::brain::situation::{Cands, Frame, Post, Sense};
use crate::ai::census;
use crate::ai::fix::{self, ServePlan};
use crate::ai::gunner_foot;
use crate::ai::mover::{self, MoveRequest};
use crate::ai::state::{
    AiState, Behaviour, FightState, MoverState, NavTick, RouteSource, SteerPick, WalkAsk,
};
use crate::ai::{FIGHT_BEYOND, FLINCH_GAP, FLINCH_TICKS, HOLD_ARRIVE, HOLD_RANGE, THINK_PERIOD};
use crate::nav;
use crate::perception;
use crate::sim::{PadState, Sim};
use crate::unit::UnitHandle;
use glam::Vec3;

use super::Tick;
use fight::{hazard_flee, ring_walk};

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
        st.step = StepKind::None;
        return (PadState::default(), nt, None);
    };
    super::ride::settle(sim, st);
    let eye = u.ai_eye();
    let wc = &sim.weapon_classes[u.weapon_class];
    // The sight-gated aim (playtest C4) is a ground rule: a space round's
    // foot bots aim as they did.
    let rules = crate::ai::UnitRules::for_unit(sim, u);
    let ground = rules.round.ground();

    let tier = sim.unit_tier(u);
    let mend = crate::ai::proxy::mend_station(sim, u);
    // This unit's own skill vector, under the tier's overrides (D1/D9). It is
    // DERIVED per think, never stored: `Unit.skill` is the state. A proxy's
    // is its profile's (card DPRX, `Sim::skill_of`).
    let skill = sim.skill_of(u);
    let tk = Tick {
        me,
        u,
        eye,
        wc,
        tier,
        skill,
        rules,
    };

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
        let mut sense = Sense {
            held,
            heard,
            ..Sense::default()
        };
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
        nt.contacts = sense.held;
        nt.heard = sense.heard;
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
        // traced. trace: both hearing exits in FUN_00252dc8 with and without a visible target.
        nt.heard_turn =
            u8::from(sense.heard > 0 && !st.contacts.iter().flatten().any(|c| c.seen_now));
        // BF1 (D10): the shooter's tracked-position error, once per think.
        if sim.ground_rules != crate::soldier::GroundRules::Bf2 {
            gunner_foot::track_error(sim, st, &skill);
        }
        // The HUNT destination is deliberately NOT bounded by perception
        // (brief D6): it is the "nothing capturable to walk at" fallback, and
        // the commander is omniscient by design too.
        for (h, o) in sim.alive_units() {
            if h == me || !sim.hostile(u.team, o.team) {
                continue;
            }
            let d2 = (o.capsule().center() - eye).length_squared();
            if sense.hunt.is_none_or(|(bd, _)| d2 < bd) {
                sense.hunt = Some((d2, o.soldier.pos));
            }
        }
        // An order the player gave outranks the commander's plan. A Follow
        // or Hold unit is not in the plan at all, so `retarget` would put it
        // on the Phase A fallback; the command branch runs instead.
        // Card SNPR: a sniper's spot - kept, left when its lane is quiet,
        // picked (`snipe::upkeep`).
        sense.seen_any = st.contacts.iter().flatten().any(|c| c.seen_now);
        super::snipe::upkeep(sim, me, u, st, sense.seen_any);
        // Card TBLD: a cutter carrier's broken turret (`fix::upkeep`).
        crate::ai::fix::upkeep(sim, me, u, st);
        sense.fixing_dest = fix::walk(sim, me, st);
        // [ours] A hurt unit breaks off to a health droid (`proxy::mend`).
        let direct = [
            mend.map(|s| s.pos),
            sense.fixing_dest,
            ring_walk(sim, st),
            super::snipe::walk(st),
        ];
        selector::destination(
            sim,
            me,
            st,
            &mut nt,
            u.command,
            direct,
            sense.hunt.map(|(_, p)| p),
        );
        board::think(sim, me, u, st, &mut nt);
        recover::think(sim, u, st);
        fight::hold_upkeep(sim, &tk, st, &sense, ground);
        order::keep_guard(sim, st);
    }

    let serve = fix::serve_plan(sim, u, st);
    // Card VAI: a seated bot's think is its seat's (`ride::drive`).
    if let Some(m) = u.mount {
        st.jet.flight = None;
        st.stall = None; // a rider is not on the walker's watch (card RCYC)
        st.mv = MoverState::default();
        st.behaviour = Behaviour::Ride;
        st.step = StepKind::None;
        let (pad, nt) = super::ride::drive(sim, me, m, st, nt, thinking);
        return (pad, nt, serve);
    }

    let mut pad = PadState::default();
    let frame = Frame::read(sim, st, tk, mend);
    let pos = frame.pos;
    // D9: the AI sniper distance table's spread, every tick (the gunner's).
    gunner_foot::range_table(&frame.tick, st, frame.seen_at());
    // Card GRAI: the weapon for this target, once a think (the gunner's).
    if thinking {
        match (frame.droid, frame.turret) {
            (Some((d, _)), _) => super::arsenal::think_droid(sim, &tk, st, d),
            (_, Some((t, _))) => super::arsenal::think_turret(sim, &tk, st, t),
            _ => super::arsenal::think(sim, &tk, st, frame.enemy),
        }
    }

    travel::advance(sim, me, st, pos, &mut nt);
    // ...and a grenade on the ground near by is a thing to run from.
    let flee = hazard_flee(sim, me, u, &tier);
    if thinking {
        if flee.is_some() {
            nt.levers |= census::LV_GRENADE_FLEE;
        } else if !sim.hazards.is_empty() && census::hazard_near(sim, me, u) {
            nt.levers |= census::LV_GRENADE_IGNORE;
        }
    }
    // OURS (playtest C1): a hit by an enemy is a flinch, a short hold of the
    // trigger (`gunner_foot::run`), at most once per FLINCH_GAP. It runs
    // beside the hit search. measure: trigger delay and repeat gap after stock enemy hits.
    let hit_now = u
        .last_hit
        .filter(|(_, tick)| sim.tick > 0 && *tick == sim.tick - 1);
    if hit_now.is_some_and(|(s, _)| sim.unit(s).is_some_and(|o| sim.hostile(u.team, o.team)))
        && perception::hit_reaction_applies(sim, u)
        && (st.flinch_until == 0 || sim.tick >= st.flinch_until + (FLINCH_GAP - FLINCH_TICKS))
    {
        st.flinch_until = sim.tick + FLINCH_TICKS;
        nt.levers |= census::LV_FLINCH;
    }

    let roll = fight::react_to_hit(sim, &tk, st, frame.enemy, hit_now.is_some(), &mut nt);

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
    let base_goal = travel::goal(st, &frame, post, push);
    // Card VAI: a boarding claim walks the bot at its vehicle (its own
    // give-up clock, not the route's stuck window, ends a claim that cannot
    // get there) — card WAI2: over the graph to the vehicle's hub
    // (`route::retarget_board`), straight at it on the last leg or with no
    // route.
    let boarding = super::ride::board_goal(sim, me, u, st, &mut nt);
    let item = pickup::goal(sim, u, st, flee);
    let last_leg = st.route().is_none_or(|r| r.is_last());
    let ret = recover::goal(sim, st, pos, &mut nt);
    // These urgent walks exclude each optional positioning request below.
    let no_urgent_move = ret.is_none() && flee.is_none() && boarding.is_none() && item.is_none();
    // Card FGHT: the ring point a circling bot walks to — over the graph,
    // and straight at it on the last leg and with no route.
    let ring =
        ring_walk(sim, st).filter(|_| no_urgent_move && st.cmd.is_none() && frame.mend.is_none());
    // Card SNPR: the sniper's spot - over the graph, and straight at it on
    // the last leg and with no route.
    let spot = super::snipe::walk(st)
        .filter(|_| no_urgent_move && st.cmd.is_none() && ring.is_none() && frame.mend.is_none());
    // Card TBLD: a fixer's stand by its turret - over the graph, and
    // straight at it on the last leg and with no route; ahead of the ring
    // and the spot (it walks only while no enemy is seen).
    let fixing = crate::ai::fix::walk(sim, me, st).filter(|_| no_urgent_move && st.cmd.is_none());
    // OURS: finish the walk at the droid's actual service sphere after the
    // graph has brought us to its hub. measure: Harbor Hard/Elite service
    // and recycles at the droid hubs (issue #108).
    let mend_approach = frame.mend.filter(|station| {
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
    // Card HOLD: the position held (when nothing more urgent walks the bot).
    let holding = st.hold.filter(|h| {
        sim.tick < h.until
            && no_urgent_move
            && ring.is_none()
            && fixing.is_none()
            && frame.mend.is_none()
    });
    let cands = Cands {
        last_leg,
        route_leg,
        post,
        corner,
        push,
        base_goal,
        boarding,
        item,
        ret,
        flee,
        ring,
        spot,
        fixing,
        mend: mend_approach,
        holding,
    };
    let composed = intent::compose_goal(&cands, pos);
    let step = selector::select(&cands, composed, st.fight.mode == FightState::TAKE);
    if thinking && boarding.is_some() && ret.is_none() && flee.is_none() {
        super::ride::maintain(sim, me, st);
    }
    let route = st.route();
    let arrive = step.arrive(
        &cands,
        composed,
        route.filter(|r| r.is_last()).map(|r| r.waypoint),
        route.is_none(),
        st.cmd.is_some(),
    );
    let goal = composed.immediate(&cands);

    let Some(aim) = intent::compose_aim(sim, st, &frame, &cands, goal) else {
        st.step = StepKind::None;
        st.steer = SteerPick::Idle;
        st.behaviour = Behaviour::None;
        st.jet.cancel(u.soldier.stance);
        return (pad, nt, serve);
    };
    let intent = Intent {
        aim,
        step,
        goal,
        arrive,
    };
    // The gunner: the look sticks and the trigger. With nothing to turn to
    // (the aim point is the muzzle itself) the tick ends here.
    let Some(gun) = gunner_foot::run(sim, &tk, st, &intent.aim, &mut pad, &mut nt) else {
        st.step = StepKind::None;
        st.steer = SteerPick::Idle;
        st.behaviour = Behaviour::None;
        st.jet.cancel(u.soldier.stance);
        return (pad, nt, serve);
    };
    fight::step_aside(sim, &tk, st, pos, gun.line, holding);
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
    let fight = frame.enemy.zip(gun.range).filter(|(e, r)| {
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
    let after_aim = Post {
        gun,
        holding,
        hold,
        fight,
        defending,
        free,
    };
    let melee_goal = melee::goal(sim, &frame, st, after_aim.free);
    fight::keep_remembered(sim, &tk, st, frame.held, after_aim.free, thinking, &mut nt);
    let manning = boarding.is_some()
        && st
            .ride
            .and_then(|r| r.board)
            .is_some_and(|vh| super::man::is_turret(sim, vh));
    fight::update_position(
        sim,
        &tk,
        st,
        after_aim
            .fight
            .filter(|_| after_aim.free && melee_goal.is_none()),
        after_aim.free,
        thinking,
        &mut nt,
    );
    // Card RCYC: who the last resort (the mover's progress watch) never
    // gives up — a bot on a held position, fighting an enemy it sees,
    // running from a grenade or under a squad order.
    let excused = after_aim.holding.is_some()
        || ring.is_some()
        || flee.is_some()
        || frame.enemy.is_some()
        || st.cmd.is_some();
    // The holder's goal may have moved this tick: its destination with it.
    let held_goal = after_aim
        .holding
        .filter(|_| intent.step == StepKind::Hold)
        .map(|h| h.goal);
    let arrive = held_goal.map(|p| (p, HOLD_ARRIVE)).or(intent.arrive);
    let goal = held_goal.or(intent.goal);
    let step = intent.step.after_aim(melee_goal);
    st.step = step;
    let mut mv = MoveRequest {
        kind: step.move_kind() as u8,
        goal,
        arrive,
        // A ring point, pickup or health droid has its own destination.
        post: post.filter(|_| ring.is_none() && item.is_none() && mend_approach.is_none()),
        defending: after_aim.defending,
        // The stand of a boarder or a commanded bot with an enemy inside
        // 25 m — and of a free bot on the tick it takes its fight position
        // (from the next it is `holding`, or walks to its ring point). Card
        // TURR: not of a bot walking to a turret, whose gun is the fight (it
        // stood 0.4 m out of reach shooting the enemy it was sent to man
        // the turret against).
        in_range: (after_aim.hold && !after_aim.free && !manning && item.is_none())
            || (after_aim.fight.is_some() && after_aim.free && ring_walk(sim, st).is_none()),
        ret,
        hold: after_aim.holding,
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
                && frame
                    .enemy
                    .is_some_and(|e| e.mount.is_none() || !after_aim.gun.no_throw)
                && matches!(
                    super::arsenal::armed(sim, u, st),
                    super::arsenal::Armed::Thrown(..)
                ),
        },
    };
    melee::steer(&mut mv, melee_goal);
    // The mover: the move sticks, the stance, the roll and the jump. It
    // answers whether the bot stands at its post.
    let moved = mover::run(sim, &tk, st, &mv, &mut pad, &mut nt);
    let arrived_at_post = moved.at_post;
    let walk = behaviours::WalkContext {
        walk: moved.walk,
        cands: &cands,
        at_sniper_spot: composed.at_spot.is_some(),
        post: &after_aim,
    };
    behaviours::on_walk(sim, &frame, st, &walk, &mut nt);
    let was = st.behaviour;
    let active = doing::settle(sim, st, &frame, &walk, intent.step);
    st.behaviour = active.tag();
    nt.why = active.why();
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

#[cfg(test)]
mod tests;
