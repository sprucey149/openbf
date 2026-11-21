//! Sim-event and state-change dispatch for campaign scripts: the handlers a
//! script registered are called for the sim's events and for the transitions
//! the sim has no event of its own for. Timers live here because their elapse
//! is a script callback too. The facade keeps the handler table and
//! has_handlers; all state stays on [super::World].

use super::*;

impl CampaignRuntime {
    fn fire(&mut self, sim: &mut Sim, kind: EventKind, key: &EventKey, args: Vec<Value>) {
        let ids: Vec<usize> = self
            .w
            .handlers
            .iter()
            .enumerate()
            .filter(|(_, h)| !h.released && h.kind == kind && h.filter.matches(key))
            .map(|(i, _)| i)
            .collect();
        for i in ids {
            if self.w.handlers[i].released || sim.outcome.is_some() {
                continue;
            }
            let f = self.w.handlers[i].func.clone();
            let mut ctx = Ctx {
                w: &mut self.w,
                sim,
            };
            if let Err(e) = self.interp.call_value(&mut ctx, &f, args.clone()) {
                ctx.w.report(&e.0);
            }
        }
    }
    pub(super) fn dispatch_sim_events(&mut self, sim: &mut Sim) {
        let mut cursor = 0usize;
        while cursor < sim.events.len() && sim.outcome.is_none() {
            let events = sim.events[cursor..].to_vec();
            cursor = sim.events.len();
            self.dispatch_event_batch(sim, &events);
        }
    }

    fn dispatch_event_batch(&mut self, sim: &mut Sim, events: &[Event]) {
        for e in events {
            if let Event::Shot {
                unit, weapon_class, ..
            } = *e
            {
                let i = unit.index as usize;
                // An unmanned auto turret's shot names no unit (index
                // u32::MAX): no killer reads it, and the table stays the
                // units' size.
                if i >= sim.units.len() {
                    continue;
                }
                if self.w.last_shot.len() <= i {
                    self.w.last_shot.resize(i + 1, None);
                }
                self.w.last_shot[i] = Some(weapon_class);
            }
        }
        // `GetObjectLastHitWeaponClass` reads the victim's last hit, kept at
        // the hit: a round that strikes a hull names its weapon in the same
        // tick's `Impact`; a hit without one (a blast) is put down to the
        // shooter's weapon then. A walker's death comes at the end of its
        // death clip, after the shooter may have fired something else.
        for e in events {
            if let Event::VehicleHit {
                shooter: Some(s),
                vehicle,
                damage,
                ..
            } = *e
            {
                if damage <= 0.0 {
                    continue;
                }
                let struck = events.iter().find_map(|x| match *x {
                    Event::Impact {
                        weapon_class,
                        vehicle: Some(v),
                        ..
                    } if v == vehicle => Some(weapon_class),
                    _ => None,
                });
                let weapon = match struck {
                    Some(wc) => sim
                        .weapon_classes
                        .get(wc as usize)
                        .map(|w| w.name.to_ascii_lowercase()),
                    None => self.w.killer_weapon(sim, s),
                };
                self.w
                    .note_hit(Ud::Vehicle(vehicle.index, vehicle.gen), weapon);
            }
        }
        for e in events {
            match *e {
                Event::Kill { shooter, victim } => {
                    let obj = Ud::Unit(victim.index, victim.gen);
                    let weapon = self.w.killer_weapon(sim, shooter);
                    self.w.note_hit(obj, weapon);
                    let key = self.w.obj_key(sim, obj);
                    let k = char_value(shooter.index);
                    // WORKAROUND for a bug in the original fel1c_c script: its
                    // 6-acklay counter only advances for a killer that passes
                    // `IsCharacterHuman`, so allied AI kills leave the count
                    // at 6 and the second wave is never requested. The port's
                    // allies reliably get those kills. Credit an acklay kill
                    // by the player's side to the player so the script's own
                    // counter advances. Details and original-game evidence:
                    // analysis/re-bf2-campaign-kill-credit.md.
                    // OURS, replace: upstream script fix that counts friendly
                    // kills, or an authored player-only acklay damage rule.
                    let k = if key.class.as_deref() == Some("geo_inf_acklay")
                        && sim
                            .unit(shooter)
                            .is_some_and(|u| sim.allied(u.team, self.w.player_team))
                    {
                        char_value(self.w.player.index)
                    } else {
                        k
                    };
                    self.w.clear_obj_markers(obj);
                    self.fire(
                        sim,
                        EventKind::CharacterDeath,
                        &EventKey::default(),
                        vec![char_value(victim.index), k.clone()],
                    );
                    self.fire(sim, EventKind::ObjectKill, &key, vec![Value::Ud(obj), k]);
                }
                Event::EnvDeath { unit, .. } => {
                    let obj = Ud::Unit(unit.index, unit.gen);
                    self.w.note_hit(obj, None);
                    let key = self.w.obj_key(sim, obj);
                    self.w.clear_obj_markers(obj);
                    self.fire(
                        sim,
                        EventKind::CharacterDeath,
                        &EventKey::default(),
                        vec![char_value(unit.index), Value::Nil],
                    );
                    self.fire(
                        sim,
                        EventKind::ObjectKill,
                        &key,
                        vec![Value::Ud(obj), Value::Nil],
                    );
                }
                Event::VehicleDestroyed {
                    vehicle, killer, ..
                } => {
                    let obj = Ud::Vehicle(vehicle.index, vehicle.gen);
                    let hit = self.w.last_hit_weapon.iter().any(|(o, _)| *o == obj);
                    if let Some(k) = killer.filter(|_| !hit) {
                        let weapon = self.w.killer_weapon(sim, k);
                        self.w.note_hit(obj, weapon);
                    }
                    let key = self.w.obj_key(sim, obj);
                    let k = killer.map_or(Value::Nil, |k| char_value(k.index));
                    self.w.clear_obj_markers(obj);
                    self.fire(sim, EventKind::ObjectKill, &key, vec![Value::Ud(obj), k]);
                }
                Event::StationDamaged { station, attacker } => {
                    let obj = Ud::Station(station);
                    let key = self.w.obj_key(sim, obj);
                    let d = attacker.map_or(Value::Nil, |u| char_value(u.index));
                    self.fire(sim, EventKind::ObjectDamage, &key, vec![Value::Ud(obj), d]);
                }
                Event::VehicleHit {
                    shooter,
                    vehicle,
                    damage,
                    ..
                } if damage > 0.0 => {
                    let obj = Ud::Vehicle(vehicle.index, vehicle.gen);
                    let key = self.w.obj_key(sim, obj);
                    let d = shooter.map_or(Value::Nil, |u| char_value(u.index));
                    self.fire(sim, EventKind::ObjectDamage, &key, vec![Value::Ud(obj), d]);
                }
                Event::VehicleShieldDamaged { shooter, vehicle } => {
                    let obj = Ud::Vehicle(vehicle.index, vehicle.gen);
                    let key = self.w.obj_key(sim, obj);
                    let d = shooter.map_or(Value::Nil, |u| char_value(u.index));
                    self.fire(sim, EventKind::ObjectDamage, &key, vec![Value::Ud(obj), d]);
                }
                Event::StationDestroyed {
                    station, killer, ..
                } => {
                    let obj = Ud::Station(station);
                    if let Some(k) = killer {
                        let weapon = self.w.killer_weapon(sim, k);
                        self.w.note_hit(obj, weapon);
                    }
                    let key = self.w.obj_key(sim, obj);
                    let k = killer.map_or(Value::Nil, |k| char_value(k.index));
                    self.w.clear_obj_markers(obj);
                    self.fire(sim, EventKind::ObjectKill, &key, vec![Value::Ud(obj), k]);
                }
                Event::Respawn { unit } => {
                    if unit == self.w.player {
                        self.w.human_spawned = true;
                    }
                    self.fire(
                        sim,
                        EventKind::CharacterSpawn,
                        &EventKey::default(),
                        vec![char_value(unit.index)],
                    );
                }
                Event::Repair {
                    unit,
                    target,
                    amount,
                    ..
                } if amount > 0.0 => {
                    let obj = match target {
                        core::projectile::Target::Station(s) => Some(Ud::Station(s)),
                        core::projectile::Target::Vehicle(v) => Some(Ud::Vehicle(v.index, v.gen)),
                        _ => None,
                    };
                    if let Some(obj) = obj {
                        let key = self.w.obj_key(sim, obj);
                        self.fire(
                            sim,
                            EventKind::ObjectRepair,
                            &key,
                            vec![Value::Ud(obj), char_value(unit.index)],
                        );
                    }
                }
                Event::Captured { cp, .. } => {
                    let obj = Ud::Post(cp as u16);
                    let key = self.w.obj_key(sim, obj);
                    self.fire(sim, EventKind::FinishCapture, &key, vec![Value::Ud(obj)]);
                }
                Event::Neutralized { cp, .. } => {
                    let obj = Ud::Post(cp as u16);
                    let key = self.w.obj_key(sim, obj);
                    self.fire(sim, EventKind::FinishNeutralize, &key, vec![Value::Ud(obj)]);
                }
                Event::FlagPickedUp { flag, unit, .. } => {
                    if let Some(f) = self.w.flag_of_ctf(flag) {
                        let key = self.w.obj_key(sim, Ud::Flag(f));
                        self.fire(
                            sim,
                            EventKind::FlagPickUp,
                            &key,
                            vec![Value::Ud(Ud::Flag(f)), char_value(unit.index)],
                        );
                    }
                }
                Event::FlagDropped { flag, unit, .. } => {
                    if let Some(f) = self.w.flag_of_ctf(flag) {
                        let key = self.w.obj_key(sim, Ud::Flag(f));
                        self.fire(
                            sim,
                            EventKind::FlagDrop,
                            &key,
                            vec![Value::Ud(Ud::Flag(f)), char_value(unit.index)],
                        );
                    }
                }
                Event::FlagReset { flag } | Event::FlagReturned { flag, .. } => {
                    if let Some(f) = self.w.flag_of_ctf(flag) {
                        let key = self.w.obj_key(sim, Ud::Flag(f));
                        self.fire(
                            sim,
                            EventKind::FlagReset,
                            &key,
                            vec![Value::Ud(Ud::Flag(f))],
                        );
                    }
                }
                _ => {}
            }
            if sim.outcome.is_some() {
                return;
            }
        }
    }

    /// Callbacks with no sim event of their own: class changes, vehicle
    /// entry and exit, regions entered and left, reinforcement and team-point
    /// counts.
    pub(super) fn dispatch_state_changes(&mut self, sim: &mut Sim) {
        let mut fired: Vec<(EventKind, EventKey, Vec<Value>)> = Vec::new();
        if self.w.seen.len() < sim.units.len() {
            self.w.seen.resize(sim.units.len(), None);
        }
        for (i, u) in sim.units.iter().enumerate() {
            let landed = u
                .mount
                .and_then(|m| sim.vehicle(m.vehicle))
                .is_some_and(|v| {
                    sim.flyer_params
                        .get(v.class as usize)
                        .and_then(Option::as_ref)
                        .is_some()
                        && v.flyer.phase == core::vehicle::flyer::FlightPhase::Landed
                });
            let now = CharSeen {
                gen: u.gen,
                alive: u.alive,
                class: u.spec.class,
                mount: u.mount.map(|m| m.vehicle),
                landed,
            };
            let ch = char_value(i as u32);
            if let Some(prev) = self.w.seen[i] {
                // A recycled unit slot is a new character even when the kit
                // is byte-for-byte the old one: a same-class respawn must
                // reach `OnCharacterChangeClass` too, not just
                // `OnCharacterSpawn` from the sim's Respawn event.
                // trace: the PS2's object-generation to class-change callback
                // rule for a respawn that keeps its kit.
                if now.alive && (prev.gen != now.gen || prev.class != now.class) {
                    fired.push((
                        EventKind::CharacterChangeClass,
                        EventKey::default(),
                        vec![ch.clone()],
                    ));
                }
                if now.alive && prev.mount != now.mount {
                    if let Some(v) = prev.mount {
                        fired.push((
                            EventKind::CharacterExitVehicle,
                            EventKey::default(),
                            vec![ch.clone(), Value::Ud(Ud::Vehicle(v.index, v.gen))],
                        ));
                    }
                    if let Some(v) = now.mount {
                        fired.push((
                            EventKind::CharacterEnterVehicle,
                            EventKey::default(),
                            vec![ch.clone(), Value::Ud(Ud::Vehicle(v.index, v.gen))],
                        ));
                    }
                } else if now.alive && !prev.landed && now.landed {
                    if let Some(v) = now.mount {
                        fired.push((
                            EventKind::CharacterLandedFlyer,
                            EventKey::default(),
                            vec![ch.clone(), Value::Ud(Ud::Vehicle(v.index, v.gen))],
                        ));
                    }
                }
            }
            if now.alive {
                self.w.seen[i] = Some(now);
            } else if let Some(prev) = self.w.seen[i].as_mut() {
                prev.alive = false;
                prev.mount = None;
                prev.landed = false;
            }
        }
        if self.has_handlers(EventKind::EnterRegion) || self.has_handlers(EventKind::LeaveRegion) {
            let watched: Vec<u32> = self
                .w
                .handlers
                .iter()
                .filter(|h| {
                    !h.released && matches!(h.kind, EventKind::EnterRegion | EventKind::LeaveRegion)
                })
                .filter_map(|h| match h.filter {
                    Filter::Region(r) => Some(r),
                    _ => None,
                })
                .filter(|r| self.w.active_regions.contains(r))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            for r in watched {
                let region = self.w.regions[r as usize].1;
                for (i, u) in sim.units.iter().enumerate() {
                    let inside = u.alive && region.contains(unit_world_pos(sim, u));
                    let was = self.w.inside.contains(&(r, i as u32));
                    if inside == was {
                        continue;
                    }
                    let key = EventKey {
                        region: Some(r),
                        ..Default::default()
                    };
                    let args = vec![Value::Ud(Ud::Region(r)), char_value(i as u32)];
                    if inside {
                        self.w.inside.insert((r, i as u32));
                        fired.push((EventKind::EnterRegion, key, args));
                    } else {
                        self.w.inside.remove(&(r, i as u32));
                        fired.push((EventKind::LeaveRegion, key, args));
                    }
                }
            }
        }
        for t in 0..2 {
            let n = reinforcement_count(sim, t);
            if n != self.w.tickets_seen[t] {
                self.w.tickets_seen[t] = n;
                fired.push((
                    EventKind::TicketCountChange,
                    EventKey::default(),
                    vec![Value::Num(t as f32 + 1.0), Value::Num(n as f32)],
                ));
            }
            if self.w.team_points[t] != self.w.points_seen[t] {
                self.w.points_seen[t] = self.w.team_points[t];
                fired.push((
                    EventKind::TeamPointsChange,
                    EventKey::default(),
                    vec![
                        Value::Num(t as f32 + 1.0),
                        Value::Num(self.w.team_points[t] as f32),
                    ],
                ));
            }
        }
        for (kind, key, args) in fired {
            if sim.outcome.is_some() {
                return;
            }
            self.fire(sim, kind, &key, args);
        }
    }

    /// Running timers count down at their rate; reaching zero stops one and
    /// calls its `OnTimerElapse` handlers. [H] the PS2's timer update is not
    /// traced; the framework re-arms what it wants to repeat with
    /// `SetTimerValue` + `StartTimer` (MultiObjectiveContainer's victory
    /// poll) and stops what it is done with (`Objective:Complete`'s ditty).
    /// trace: the timer manager behind CreateTimer (`FUN_0018f4a0`).
    pub(super) fn step_timers(&mut self, sim: &mut Sim, dt: f32) {
        let mut elapsed = Vec::new();
        for (i, t) in self.w.timers.iter_mut().enumerate() {
            if !t.alive || !t.running {
                continue;
            }
            t.value -= dt * t.rate;
            if t.value <= 0.0 {
                t.value = 0.0;
                t.running = false;
                elapsed.push(i as u32);
            }
        }
        for i in elapsed {
            let key = EventKey {
                timer: Some(i),
                ..Default::default()
            };
            self.fire(
                sim,
                EventKind::TimerElapse,
                &key,
                vec![Value::Ud(Ud::Timer(i))],
            );
        }
    }
}
