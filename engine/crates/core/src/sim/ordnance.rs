//! Card GREN in the tick (`analysis/re-grenades.md` [O]): a soldier's two
//! weapon channels — which trigger has it, the two switch inputs, the held
//! weapons' cycles (§2.3, §2.4) — the throw (§3.3) and the thrown grenades'
//! flight, sweep, contact, fuse and blast (§4, [`crate::sticky`]).

use super::input::{buttons, PadState};
use super::state::{Event, Sim};
use crate::collision::{ray_terrain_ps2, HullFilter, Layers};
use crate::projectile::{Bolt, Shooter};
use crate::soldier::{GroundRules, Stance};
use crate::sticky::{self, Anchor, Contact, Sticky, Surface};
use crate::unit::Controller;
use crate::vehicle::VehicleKind;
use crate::weapon::{shot_direction, FireInput, FireState, OrdKind, ShotSpec};
use glam::Vec3;

mod tracers;

/// How long a grenade that has touched down is expected to keep bouncing
/// before its fuse starts (s) — the bots' warning needs an end time at the
/// first contact, the fuse only starts at the rest. PROVISIONAL (ours): the
/// PS2 rows rested 1.3 s after their first contact (§8); corrected at the
/// rest.
const HAZARD_BOUNCE_TIME: f32 = 1.5;

impl Sim {
    /// Phase 4 for a soldier that holds more than one weapon: pick the
    /// active channel, step the two switch inputs, run every held weapon's
    /// cycle and throw what channel 1 released. Returns whether the PRIMARY
    /// trigger reaches channel 0's weapon this tick.
    ///
    /// §2.3 (B1 `FUN_00205cc8` 117099-117161): channel 1 has the soldier
    /// while its weapon's fire state runs, or while its trigger is down on a
    /// weapon that is not out of ammo; the other trigger is swallowed. §2.4:
    /// a switch input steps its channel to the next slot of that channel
    /// (wrapping, at once) and only while neither trigger is down.
    pub(super) fn held_weapons(&mut self, i: usize, p: &PadState, blocked: bool, dt: f32) -> bool {
        let pri = p.pressed(buttons::FIRE);
        let sec = p.pressed(buttons::SECONDARY_FIRE);
        // `infinite`: a free reload always refills, so never out of ammo.
        let (infinite, no_reload) = self.ammo_cheats(i);

        // The active channel.
        let ch1_has_it = self.units[i].cur1.is_some_and(|k| {
            let w = &self.units[i].held[k as usize];
            let c = &self.weapon_classes[w.class];
            w.state.state == FireState::Firing
                || ((sec || w.state.fire_cmd != 0)
                    && (infinite || !w.state.out_of_ammo(c) || self.has_remote_items(i, w.class)))
        });
        self.units[i].active_channel = u8::from(ch1_has_it);
        let held0 = pri && !ch1_has_it;
        let held1 = sec && ch1_has_it;

        // The switches: edges, and dead while a channel is active.
        let now = u8::from(p.pressed(buttons::SWITCH_WEAPON))
            | u8::from(p.pressed(buttons::SWITCH_SECONDARY)) << 1;
        let edges = now & !self.units[i].switch_held;
        self.units[i].switch_held = now;
        // A trigger let go last tick still counts (the PS2's release byte).
        let released = self.units[i].weapon.fire_cmd > 0;
        if !pri && !sec && !ch1_has_it && !released {
            if edges & 1 != 0 {
                self.switch_channel0(i);
            } else {
                self.award_fixup(i);
            }
            if edges & 2 != 0 {
                self.switch_channel1(i);
            }
        }

        // Every held weapon's cycle; only channel 1's current one sees a
        // trigger. An idle cycle draws nothing from the LCG.
        let cur1 = self.units[i].cur1.map(usize::from);
        let mut shots = Vec::new();
        for k in 0..self.units[i].held.len() {
            let class = self.units[i].held[k].class;
            if self.tick_repair_weapon(i, Some(k), Some(k) == cur1 && held1, blocked, dt) {
                continue;
            }
            let held = Some(k) == cur1 && held1;
            if self.cloak_input(i, Some(k), held, blocked) {
                continue;
            }
            if self.force_input(i, Some(k), held, blocked) {
                continue;
            }
            let remote = self.remote_input(i, Some(k), held, blocked);
            let input = FireInput {
                held: held && !remote,
                blocked,
                infinite_ammo: infinite,
                no_reload,
                ai_dispersion: None,
            };
            shots.clear();
            let wc = &self.weapon_classes[class];
            let u = &mut self.units[i];
            if u.held[k].channel == 1 && wc.ordnance.sticky.is_none() {
                let stance = match u.soldier.stance {
                    crate::soldier::Stance::Crouch => 1,
                    crate::soldier::Stance::Prone => 2,
                    _ => 0,
                };
                u.held[k].state.set_posture(
                    wc,
                    stance,
                    PadState::axis(p.ly).abs().max(PadState::axis(p.lx).abs()),
                    false,
                );
            }
            let was_reloading = u.held[k].state.reloading();
            self.units[i].held[k]
                .state
                .update(dt, wc, input, &mut self.lcg, &mut shots);
            // The empty click, as channel 0's (`re-weapon-sounds.md` 7.3): both
            // channels' current weapons are selected (B1 `FUN_0020cbd0`
            // 121397). A press on one out of ammunition does not take the
            // soldier (B1 `FUN_00205cc8` 117125-117137), so the weapon's
            // cycle never sees it here, but the PS2 leaves its trigger byte
            // at 1 and the weapon in state 8 clicks.
            // PROVISIONAL [D], measure: a PS2 log of a human's L1 press on an
            // empty and on a reloading channel 1 weapon.
            let u = &self.units[i];
            let refused = Some(k) == cur1
                && u.is_player
                && (u.held[k].state.press_clicks(wc, input, !shots.is_empty())
                    || (!ch1_has_it && !blocked && self.pressed_now(i, buttons::SECONDARY_FIRE)));
            if refused {
                self.push_fire_empty(i, class);
            }
            if !was_reloading
                && self.units[i].held[k].state.reloading()
                && self.weapon_classes[class].ordnance.sticky.is_none()
            {
                self.events.push(Event::Reload {
                    unit: self.handle(i),
                    weapon_class: class as u32,
                });
            }
            self.soldier_shots(i, Some(k), &shots);
        }
        held0
    }

    /// Whether `button` went down this tick for a human (`press_edges`).
    pub(super) fn pressed_now(&self, i: usize, button: u32) -> bool {
        self.press_edges.get(i).is_some_and(|e| e & button != 0)
    }

    /// A human's empty click at its soldier (`Event::FireEmpty`).
    pub(super) fn push_fire_empty(&mut self, i: usize, class: usize) {
        self.events.push(Event::FireEmpty {
            unit: self.handle(i),
            weapon_class: class as u32,
            pos: self.units[i].soldier.pos,
        });
    }

    /// Stage 5's shells (issue #29, `analysis/re-ordnance-kinds.md` §6): each
    /// falls (v.y -= g dt, then the move), and the world is met along this
    /// tick's chord of its arc - the terrain, water and the map's bodies, as a
    /// bolt's spawn ray meets them; soldiers and hulls are the per-tick sweep
    /// that follows ([`Sim::sweep_bolts`]).
    pub(super) fn fly_shells(&mut self, dt: f32) {
        for b in self.bolts.iter_mut() {
            if b.gravity > 0.0 && b.hit == crate::projectile::Hit::None {
                b.vel.y -= b.gravity * dt;
            }
        }
        for k in 0..self.bolts.len() {
            if self.bolts[k].gravity > 0.0 {
                self.probe_flight(k, dt);
            }
        }
    }

    pub(super) fn shot_frame(&self, i: usize) -> (Vec3, (Vec3, Vec3, Vec3)) {
        if matches!(self.units[i].controller, Controller::Player { .. }) {
            self.human_shot(i)
        } else if let Some(frame) = self.bot_droideka_shot(i) {
            frame
        } else {
            (
                self.units[i]
                    .droideka_muzzle()
                    .unwrap_or_else(|| self.units[i].muzzle()),
                self.units[i].aim_frame(),
            )
        }
    }

    /// Bind presentation identity at the event producer, before rounds can be
    /// removed or become indistinguishable from another pellet's physics.
    pub(super) fn note_bolt_shot(&mut self, bolt: &mut Bolt, origin: Vec3, dir: Vec3) {
        bolt.visual_shot = Some(crate::projectile::VisualShot {
            tick: self.tick,
            event: self.events.len(),
        });
        self.events.push(Event::Shot {
            unit: bolt.owner,
            weapon_class: bolt.weapon_class as u32,
            origin,
            dir,
        });
    }

    pub(super) fn soldier_shots(&mut self, i: usize, held_slot: Option<usize>, shots: &[ShotSpec]) {
        if shots.is_empty() {
            return;
        }
        let u = &self.units[i];
        let class = held_slot.map_or(u.weapon_class, |k| u.held[k].class);
        let unit = self.handle(i);
        // A human's shot leaves the fire point toward what the reticle's
        // ray hits (card AIMA); a bot's leaves the chest along its aim
        // (card FPNT has the fire point for bots).
        let (origin, (fwd, right, up)) = self.shot_frame(i);
        let who = Shooter {
            owner: unit,
            team: self.units[i].team,
            weapon_class: class,
            is_player: self.units[i].is_player,
        };
        if let Some(d) = self.units[i].droideka.as_mut() {
            d.barrel = ((usize::from(d.barrel) + shots.len()) % 2) as u8;
        }
        for spec in shots {
            // A charge stage's round is its own class's (issue #29): its
            // ordnance flies and strikes; a sticky one (a mortar's held
            // round, BF1's smuggler's mortar) is launched.
            let stage = self.weapon_classes[class].stage_index(class, usize::from(spec.stage));
            if self.weapon_classes[stage].guided.is_some() {
                self.launch_guided(i, stage);
                continue;
            }
            if self.weapon_classes[stage].remote.is_some() {
                self.launch_droid(i, stage, spec.charge);
                continue;
            }
            if self.weapon_classes[stage].turret.is_some() {
                self.place_turret(i, stage, spec.charge);
                continue;
            }
            if self.weapon_classes[stage].saber_throw.is_some() {
                self.launch_saber_throw(i, held_slot, stage);
                continue;
            }
            if self.weapon_classes[stage].force.is_some() {
                self.fire_force(i, stage, spec.charge);
                continue;
            }
            if self.weapon_classes[stage].ability.is_some() {
                self.fire_ability(i, stage);
                continue;
            }
            // An emitter (card UORDE, `analysis/re-ordnance-kinds.md` §8):
            // an arc caster, incinerator or flamethrower spends its shot as
            // one instant cone-and-chain query, never a bolt. The CLASS
            // dispatches (`ConeLength > 0`, the PS2's own shot test), not its
            // damage: a zero-damage cone (the Force push / pull family) still
            // runs its query, and its links are pure shoves. A radius
            // `emitterordnance` (ConeLength 0) is not a cone and stays on the
            // `Ability` path above, and a class with an ability never reaches
            // here.
            let wc = &self.weapon_classes[stage];
            if wc.emitter.as_ref().is_some_and(|e| e.is_cone()) {
                self.units[i].last_shot_tick = self.tick;
                self.fire_emitter(i, stage, spec.charge);
                continue;
            }
            if self.weapon_classes[stage].mine.is_some() {
                self.place_mine(i, stage, spec.charge);
                continue;
            }
            if self.weapon_classes[stage].dispenser.is_some() {
                self.dispense_shots(i, held_slot, stage, std::slice::from_ref(spec));
                continue;
            }
            let sticky = self.weapon_classes[stage].ordnance.sticky.is_some();
            // Perception's hearing reads this (`perception::scan`): anyone
            // within HEAR_RADIUS of a shot knows a shooter is there, with no
            // line of sight. A thrown grenade (a held slot's sticky) is not
            // heard, as before this path was shared.
            if !(sticky && held_slot.is_some()) {
                self.units[i].last_shot_tick = self.tick;
            }
            if sticky {
                self.launch(i, stage, spec.charge);
                continue;
            }
            // BF2's secondary ordnance (`SecondaryOrdnanceName`): count the
            // round here, after every path that is not a bolt has gone, so
            // the counter is the weapon's rounds and not its trigger pulls.
            let secondary = self.weapon_classes[stage].secondary.is_some();
            let round = if secondary {
                match held_slot {
                    Some(k) => &mut self.units[i].held[k].state,
                    None => &mut self.units[i].weapon,
                }
                .count_secondary_round()
            } else {
                0
            };
            let dir = shot_direction(
                &mut self.lcg,
                fwd,
                right,
                up,
                spec.pitch_spread,
                spec.yaw_spread,
                spec.tilt,
            );
            let wc = &self.weapon_classes[stage];
            // The grenade family's launch (§3.3, B1 `FUN_0028ef70`
            // 215847-215862): a HUMAN owner's round is lifted by the class's
            // `ShotElevate`, added to the direction's y and renormalised; a
            // bot's aim is used as it is (§7.3). The mortar is a `grenade`
            // class and its `_ord1` shell leaves through the same launcher,
            // so the shell takes the lift too (issue #29).
            let dir = match wc.thrown {
                Some(t) if wc.ordnance.kind == OrdKind::Shell && who.is_player => {
                    sticky::throw_dir(dir, t.shot_elevate)
                }
                _ => dir,
            };
            let who = Shooter {
                weapon_class: stage,
                ..who
            };
            // The launcher's dwell lock (card UORD §5): a missile leaves
            // guided only when the clock reached LockTime; the shot consumes
            // the lock (B1 class+0x258 && +0x254) either way.
            let lock = {
                let st = match held_slot {
                    Some(k) => &self.units[i].held[k].state,
                    None => &self.units[i].weapon,
                };
                st.locked(wc)
            };
            let lock_target = if lock {
                match held_slot {
                    Some(k) => self.units[i].held[k].state.lock_target,
                    None => self.units[i].weapon.lock_target,
                }
            } else {
                None
            };
            // The round's speed and life at its charge (its ordnance's charge
            // points, re-weapon-charge §4).
            let blend = wc.ordnance.charged(spec.charge);
            let mut bolt = Bolt::fire(
                origin,
                dir,
                blend.velocity,
                blend.life_span,
                who,
                self.heightfield.as_ref(),
                self.water_plane(),
                &self.collision,
                HullFilter::All,
            )
            .shaped(&self.weapon_classes[stage].ordnance);
            bolt.charge = spec.charge;
            if let Some(mc) = self.weapon_classes[stage].ordnance.missile {
                let ord = self.weapon_classes[stage].ordnance.clone();
                bolt.as_missile(&ord, &mc, lock_target, &mut self.lcg);
                // A human's locked rocket goes to the vehicle's weak part
                // in his sight (`weak_lock`); space rounds keep the centre.
                let part = match lock_target {
                    Some(crate::missile::MissileTarget::Vehicle(vh))
                        if who.is_player && !self.space_round() =>
                    {
                        let (eye, aim) = self.units[i].aim_ray(self.ground_rules, self.views());
                        self.weak_part(vh, eye, aim).map(|(k, _)| k)
                    }
                    _ => None,
                };
                if let Some(m) = bolt.missile.as_mut() {
                    m.part = part;
                }
                let st = match held_slot {
                    Some(k) => &mut self.units[i].held[k].state,
                    None => &mut self.units[i].weapon,
                };
                st.lock_target = None;
                st.lock_held = 0.0;
            }
            self.note_bolt_shot(&mut bolt, origin, dir);
            self.launch_round(bolt, true);
            // The tracer rides the SAME ray the primary round took - it draws
            // no extra LCG step (the PS2 spawns both from one packet).
            if secondary {
                self.fire_secondary(who, stage, round, origin, dir, spec.charge);
            }
        }
    }

    /// Step channel 0 to its next slot: the weapon in the hands trades
    /// places with the held one (§2.4; the same slot again does nothing).
    pub(super) fn switch_channel0(&mut self, i: usize) {
        let u = &self.units[i];
        let cur = u.weapon_slot;
        let next = u
            .held
            .iter()
            .enumerate()
            .filter(|(_, w)| w.channel == 0 && !self.weapon_locked(u, w.class))
            .min_by_key(|(_, w)| (w.slot < cur, w.slot))
            .map(|(k, _)| k);
        if let Some(k) = next {
            self.take_channel0(i, k);
        }
    }

    /// Held slot `k` goes into unit `i`'s hands, the weapon there into `k`.
    pub(super) fn take_channel0(&mut self, i: usize, k: usize) {
        let u = &mut self.units[i];
        let w = &mut u.held[k];
        std::mem::swap(&mut u.weapon, &mut w.state);
        std::mem::swap(&mut u.weapon_class, &mut w.class);
        std::mem::swap(&mut u.weapon_slot, &mut w.slot);
        w.state.active = false;
        u.weapon.active = true;
        // A switch of the weapon in hand ends the zoom (`re-zoom.md` §4).
        u.zoom = None;
        let weapon_class = u.weapon_class as u32;
        let unit = self.handle(i);
        self.events.push(Event::WeaponSelect { unit, weapon_class });
    }

    /// Step channel 1 to its next slot (§2.4).
    fn switch_channel1(&mut self, i: usize) {
        let u = &mut self.units[i];
        let Some(cur) = u.cur1.map(usize::from) else {
            return;
        };
        let slot = u.held[cur].slot;
        let next = u
            .held
            .iter()
            .enumerate()
            .filter(|(k, w)| w.channel == 1 && *k != cur)
            .min_by_key(|(_, w)| (w.slot < slot, w.slot))
            .map(|(k, _)| k);
        if let Some(k) = next {
            u.held[cur].state.active = false;
            u.held[k].state.active = true;
            u.cur1 = Some(k as u8);
            let weapon_class = u.held[k].class as u32;
            let unit = self.handle(i);
            self.events.push(Event::WeaponSelect { unit, weapon_class });
        }
    }

    /// One shot of weapon class `wc` from unit `i`, when that weapon throws:
    /// spawn the grenade (§3.3) and say so. `false` for a weapon that fires
    /// bolts — the caller's business.
    pub(super) fn launch(&mut self, i: usize, wc: usize, charge: f32) -> bool {
        let class = &self.weapon_classes[wc];
        let Some(sc) = class.ordnance.sticky.as_ref() else {
            return false;
        };
        let u = &self.units[i];
        let (aim, _, _) = u.aim_frame();
        let thrown = class.thrown;
        // A human's throw is lifted by ShotElevate; a bot's aim is used as it
        // is (B1 215847-215862, the owner's player index).
        let dir = match thrown {
            Some(t) if matches!(u.controller, Controller::Player { .. }) => {
                sticky::throw_dir(aim, t.shot_elevate)
            }
            _ => aim,
        };
        // BF1: the weapon's fire point; BF2: the hand, in the soldier's frame.
        // OURS: the stock scaled hand applies to standing and crouched
        // releases. trace: retail launch positions in other stances and
        // the fire-entry variant through wind-up posture changes.
        let scale = if self.ground_rules == GroundRules::Bf2
            && matches!(u.soldier.stance, Stance::Stand | Stance::Crouch)
        {
            u.consts.skeleton_root_scale
        } else {
            1.0
        };
        let origin = u.launch_origin(thrown.and_then(|t| t.release_hand(scale)));
        let owner = self.handle(i);
        // Sticky inherits the shell/bullet/base charge evaluation
        // (B1 FUN_0027eb18:203821; B2 FUN_003207e8:313521).
        let blend = class.ordnance.charged(charge);
        let mut g = Sticky::new(
            origin,
            dir * blend.velocity,
            sc,
            blend.life_span,
            owner,
            u.team,
            wc,
        );
        g.direct = (self.is_hunt() && blend.damage > 0.0).then_some((charge, 0.0, u.is_player));
        self.stickies.push(g);
        self.events.push(Event::Throw {
            unit: owner,
            weapon_class: wc as u32,
            origin,
        });
        true
    }

    /// Phase 5b: every thrown grenade's tick, in throw order — the fuse, the
    /// flight, then the sweep and its contact (§4.1, §4.2: the PS2 updates
    /// the object and its collision manager sweeps it afterwards).
    pub(super) fn tick_stickies(&mut self, dt: f32) {
        if self.stickies.is_empty() {
            return;
        }
        let mut list = std::mem::take(&mut self.stickies);
        list.retain_mut(|g| self.tick_sticky(g, dt));
        // Nothing can throw during the pass; keep the order regardless.
        list.append(&mut self.stickies);
        self.stickies = list;
    }

    /// One grenade's tick; `false` when it is gone.
    fn tick_sticky(&mut self, g: &mut Sticky, dt: f32) -> bool {
        if g.burn(dt) {
            if !g.lost() {
                self.detonate(g);
            }
            return false;
        }
        match g.anchor {
            Some(Anchor::Vehicle { handle, local }) => {
                if let Some(v) = self.vehicle(handle) {
                    g.pos = v.pos + v.rot * local;
                }
            }
            Some(Anchor::Unit { handle, local }) => {
                if let Some(u) = self.unit(handle) {
                    g.pos = u.soldier.pos + local;
                }
            }
            _ => {}
        }
        if !g.moving {
            return true;
        }
        if let Some((_, age, _)) = g.direct.as_mut() {
            *age += dt;
        }
        let prev = g.pos;
        let level = self.water_under(prev);
        g.fly(dt, level.is_some_and(|l| prev.y < l));
        let step = g.pos - prev;
        let len = step.length();
        if len <= 0.0 {
            return true;
        }
        let dir = step / len;
        let Some((point, n, what)) = self.sweep_sticky(g, prev, dir, len) else {
            return true;
        };
        let wc = g.weapon_class;
        let Some(class) = self.weapon_classes[wc].ordnance.sticky.clone() else {
            return false;
        };
        let first = g.touched.is_none();
        if first {
            g.touched = Some(self.tick);
        }
        // BF2 FUN_00320fe8 and BF1 FUN_0027f1e8 apply direct damage on
        // entity contact, whether the projectile sticks or bounces.
        if let Some((charge, age, player)) = g.direct {
            let desc = self.weapon_classes[wc]
                .ordnance
                .damage_desc_charged(age, false, charge);
            match what {
                Surface::Unit { handle, .. } => {
                    let scale = self.buff_scale(Some(g.owner), handle);
                    if let Some(damage) =
                        self.shot_strikes_unit(g.team, player, handle, &desc, scale, true)
                    {
                        self.finish_direct_damage(handle, Some(g.owner), damage, age, dir);
                    }
                }
                Surface::Vehicle { handle, .. } => {
                    self.damage_vehicle_by(
                        handle,
                        &desc,
                        1.0,
                        Some(g.owner),
                        Some(g.team),
                        Some(dir),
                    );
                }
                _ => {}
            }
        }
        let room = self.push_off_room(point, n);
        let out = g.contact(&class, point, n, what, room);
        if out == Contact::Stuck {
            g.anchor = Some(match what {
                Surface::Vehicle { handle, .. } => match self.vehicle(handle) {
                    Some(v) => Anchor::Vehicle {
                        handle,
                        local: v.rot.inverse() * (g.pos - v.pos),
                    },
                    None => Anchor::World,
                },
                Surface::Unit { handle, .. } => match self.unit(handle) {
                    Some(u) => Anchor::Unit {
                        handle,
                        local: g.pos - u.soldier.pos,
                    },
                    None => Anchor::World,
                },
                _ => Anchor::World,
            });
        }
        self.events.push(Event::StickyContact {
            pos: g.pos,
            weapon_class: wc as u32,
            water: out == Contact::Water,
        });
        // The bots' warning (card HOLD, §7.4): from the first contact, and
        // put right — the place and the real fuse — once it lies still.
        let ticks = |s: f32| (s * crate::TICK_HZ as f32).ceil() as u64;
        if first {
            let id = self
                .last_warning_id
                .checked_add(1)
                .expect("grenade warning identity exhausted");
            self.add_hazard_by(g.pos, ticks(g.life + HAZARD_BOUNCE_TIME), Some(g.owner));
            self.last_warning_id = id;
            g.warning_id = Some(id);
            self.hazards
                .last_mut()
                .expect("warning just inserted")
                .warning_id = Some(id);
        }
        if matches!(out, Contact::Rest | Contact::Stuck) {
            let now = self.tick;
            let fuse = ticks(g.life);
            if let Some(z) = self
                .hazards
                .iter_mut()
                .find(|z| g.warning_id.is_some() && z.warning_id == g.warning_id)
            {
                z.pos = g.pos;
                z.until = now + fuse;
            }
        }
        true
    }

    /// The sweep ray (§4.2, §13.2; B1 `FUN_0027eff8`, B2 `FUN_00320de8`):
    /// from [`sticky::SWEEP_BACK`] behind the previous position over the
    /// step plus [`sticky::SWEEP_EXTRA`] — so 0.1 m past the new position —
    /// the owner ignored. The nearest of the water surface, the terrain by
    /// BF2's own segment test ([`ray_terrain_ps2`]: not the crossing), the
    /// world's bodies and hulls (PROVISIONAL: the ordnance subset, as a bolt
    /// — the PS2's mask is 0x9de / 0x13ba) and the soldiers.
    ///
    /// DEVIATION (card GRFL, PROVISIONAL): the bodies' ray is NOT pulled
    /// back. A bounce leaves the grenade [`sticky::PUSH_OFF`] off the face
    /// and a steep one leaves along the normal, so a ray from
    /// [`sticky::SWEEP_BACK`] behind starts ON the face; the port's mesh ray
    /// meets a face from either side, took it from behind and bounced the
    /// grenade under the floor (8.2 % of the census's grenades, the maintainer's
    /// Kamino decks). The PS2's grenades rest on those decks; what its mesh
    /// test does behind the origin is unread (re-grenades §13.2), and the
    /// baked meshes' windings are mixed, so a one-sided ray is no answer.
    /// How far a bounce may push the grenade off along `n`
    /// ([`sticky::PUSH_OFF`]), or half the way to a surface it would meet
    /// first. Ours (issue #37): the underside of a step's lip, 2.5 cm over
    /// the floor, pushed it 4.5 cm down through that floor.
    pub(super) fn push_off_room(&self, point: Vec3, n: Vec3) -> f32 {
        let start = point + n * sticky::PUSH_ROOM_START;
        let len = sticky::PUSH_OFF - sticky::PUSH_ROOM_START;
        let mut t = len;
        if let Some(h) = self
            .collision
            .ray_with(start, n, len, Layers::ORDNANCE, HullFilter::All)
        {
            t = t.min(h.t);
        }
        if let Some(hf) = self.heightfield.as_ref() {
            if let Some(f) = ray_terrain_ps2(hf, &self.collision.cutters, start, start + n * len) {
                t = t.min(len * f);
            }
        }
        if t < len {
            sticky::PUSH_ROOM_START + t * 0.5
        } else {
            sticky::PUSH_OFF
        }
    }

    fn sweep_sticky(
        &self,
        g: &Sticky,
        prev: Vec3,
        dir: Vec3,
        len: f32,
    ) -> Option<(Vec3, Vec3, Surface)> {
        let origin = prev - dir * sticky::SWEEP_BACK;
        let max = len + sticky::SWEEP_EXTRA;
        let mut best: Option<(f32, Vec3, Surface)> = None;
        if let Some(hf) = self.heightfield.as_ref() {
            let end = origin + dir * max;
            if let Some(f) = ray_terrain_ps2(hf, &self.collision.cutters, origin, end) {
                let t = max * f;
                let p = origin + dir * t;
                let n = Vec3::from(hf.normal_at(p.x, p.z));
                best = Some((t, n, Surface::Terrain));
            }
        }
        let mut nearer = |t: f32, n: Vec3, s: Surface| {
            if best.is_none_or(|b| t < b.0) {
                best = Some((t, n, s));
            }
        };
        if let Some(t) = self
            .water_plane()
            .filter(|w| prev.y >= w.level)
            .and_then(|w| w.crossing(origin, dir))
        {
            if t <= max {
                nearer(t, Vec3::Y, Surface::Water);
            }
        }
        // The bodies' ray starts AT the previous position and ends where the
        // others do (card GRFL, a named deviation: see the header above).
        let back = sticky::SWEEP_BACK;
        if let Some(mut h) =
            self.collision
                .ray_with(prev, dir, max - back, Layers::ORDNANCE, HullFilter::All)
        {
            h.t += back;
            let s = match h.owner {
                Some(v) => Surface::Vehicle {
                    handle: v,
                    building: self
                        .vehicle(v)
                        .is_some_and(|v| v.kind == VehicleKind::Turret),
                },
                None => Surface::World,
            };
            nearer(h.t, h.normal, s);
        }
        for (h, u) in self.alive_units() {
            if h == g.owner || u.mount.is_some() {
                continue;
            }
            let cap = u.capsule();
            if let Some(t) = cap.ray_hit(origin, dir, max) {
                // The capsule's side: the normal is level, away from its axis.
                let p = origin + dir * t;
                let away = Vec3::new(p.x - cap.base.x, 0.0, p.z - cap.base.z);
                let n = away.try_normalize().unwrap_or(Vec3::Y);
                let kind = u.kind();
                nearer(t, n, Surface::Unit { handle: h, kind });
            }
        }
        best.map(|(t, n, s)| (origin + dir * t, n, s))
    }

    /// The fuse ran out: the `ExplosionExpire` class goes off where the
    /// grenade lies, the thrower credited (§4.5).
    fn detonate(&mut self, g: &Sticky) {
        let wc = g.weapon_class;
        self.events.push(Event::StickyBlast {
            pos: g.pos,
            weapon_class: wc as u32,
        });
        if g.warning_id.is_some() {
            let now = self.tick;
            if let Some(z) = self
                .hazards
                .iter_mut()
                .find(|z| g.warning_id.is_some() && z.warning_id == g.warning_id)
            {
                z.until = now;
            }
        }
        let explosion = self.weapon_classes[wc]
            .ordnance
            .sticky
            .as_ref()
            .and_then(|s| s.explosion.clone());
        if let Some(e) = explosion {
            self.explode(&e, g.pos, Some(g.owner), Some(g.team));
        }
    }
}

#[cfg(test)]
mod tests;
