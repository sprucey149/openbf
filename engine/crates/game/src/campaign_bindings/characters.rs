//! Character, team, object, entity, animation and flag bindings: resolving
//! script names into sim handles and spawning the ones a script creates.

use super::*;

impl Ctx<'_> {
    pub(super) fn characters(&mut self, _it: &mut Interp, name: &str, a: &[Value]) -> Option<Ret> {
        let sim = &mut *self.sim;
        let w = &mut *self.w;
        Some(match name {
            "GetCharacterUnit" => one(w
                .char_of(arg(a, 0))
                .and_then(|c| w.char_unit(sim, c))
                .map_or(Value::Nil, |h| Value::Ud(Ud::Unit(h.index, h.gen)))),
            "GetCharacterVehicle" => one(w
                .char_of(arg(a, 0))
                .and_then(|c| w.char_unit(sim, c))
                .and_then(|h| sim.unit(h))
                .and_then(|u| u.mount)
                .map_or(Value::Nil, |m| {
                    Value::Ud(Ud::Vehicle(m.vehicle.index, m.vehicle.gen))
                })),
            "ExitVehicle" => {
                if let Some(h) = w.char_of(arg(a, 0)).and_then(|c| w.char_unit(sim, c)) {
                    if let Some(u) = sim.units.get(h.index as usize) {
                        if u.alive && u.mount.is_some() {
                            sim.exit_vehicle(h);
                        }
                    }
                }
                nothing()
            }
            "GetCharacterTeam" => one(w
                .char_of(arg(a, 0))
                .and_then(|c| w.char_team(sim, c))
                .map_or(Value::Nil, |t| Value::Num(f32::from(t) + 1.0))),
            "IsCharacterHuman" => one(Value::Bool(
                w.char_of(arg(a, 0))
                    .and_then(|c| sim.units.get(w.char_slot(c) as usize))
                    .is_some_and(|u| u.is_player),
            )),
            "GetCharacterClass" => one(w
                .char_of(arg(a, 0))
                .and_then(|c| sim.units.get(w.char_slot(c) as usize))
                .map_or(Value::Nil, |u| {
                    let ci = usize::from(u.spec.class);
                    let idx = w
                        .class_order
                        .get(usize::from(u.team))
                        .and_then(|o| o.iter().position(|c| *c == ci))
                        .unwrap_or(ci);
                    Value::Num(idx as f32)
                })),
            "CanCharacterInteractWithFlag" => one(Value::Bool(
                w.char_of(arg(a, 0))
                    .and_then(|c| w.char_unit(sim, c))
                    .is_some(),
            )),
            "GetTeamSize" => one(Value::Num(
                sim_team(arg(a, 0)).map_or(0, |t| w.team_size(t)) as f32,
            )),
            "GetTeamMember" => {
                let (Some(t), Some(i)) = (sim_team(arg(a, 0)), num(a, 1)) else {
                    return Some(one(Value::Nil));
                };
                one(w
                    .team_member(sim, t, i.fmax(0.0) as usize)
                    .map_or(Value::Nil, char_value))
            }
            "GetNumTeamMembersAlive" => one(Value::Num(sim_team(arg(a, 0)).map_or(0, |t| {
                sim.units.iter().filter(|u| u.alive && u.team == t).count()
            }) as f32)),
            // BF2's scripted hero path: `BatchChangeTeams(from, to, count)`
            // moves native members to a main army, and
            // `SelectCharacterClass(member, class)` gives one a hero kit.
            // Utapau's Objective 3a.OnComplete uses both to field Grievous
            // for the "Fight General Grievous" target objective.
            "BatchChangeTeams" => {
                if let (Some(from), Some(to), Some(n)) =
                    (bf2_team(arg(a, 0)), bf2_team(arg(a, 1)), num(a, 2))
                {
                    let from = from.saturating_sub(1);
                    let to = to.saturating_sub(1);
                    for i in 0..(n.max(0.0) as u32).min(1000) {
                        let c = PLACEHOLDER + u32::from(from) * 1000 + i;
                        w.scripted_chars.entry(c).or_default().team = Some(to);
                    }
                }
                nothing()
            }
            "SelectCharacterClass" => {
                if let Some(c) = w.char_of(arg(a, 0)) {
                    let class = if let Some(s) = arg(a, 1).as_str() {
                        Some(s.to_ascii_lowercase())
                    } else if let Some(idx) = num(a, 1).filter(|n| *n >= 0.0) {
                        let team = w
                            .scripted_chars
                            .get(&c)
                            .and_then(|s| s.team)
                            .or_else(|| w.char_team(sim, c));
                        team.and_then(|t| {
                            let ti = usize::from(t);
                            let ci = *w.class_order.get(ti)?.get(idx as usize)?;
                            w.team_classes
                                .get(ti)?
                                .get(ci)
                                .map(|s| s.to_ascii_lowercase())
                        })
                    } else {
                        None
                    };
                    if let Some(class) = class {
                        w.scripted_chars.entry(c).or_default().class = Some(class);
                    }
                }
                nothing()
            }
            "SpawnCharacter" => {
                let (Some(c), Some(pos)) = (w.char_of(arg(a, 0)), w.point_of(sim, arg(a, 1)))
                else {
                    return Some(nothing());
                };
                w.spawn_character(sim, c, pos);
                nothing()
            }

            "GetObjectPtr" | "GetEntityPtr" => one(arg(a, 0)
                .as_str()
                .and_then(|n| w.resolve_name(sim, n))
                .map_or(Value::Nil, Value::Ud)),
            "GetEntityName" | "GetEntityname" => one(w
                .resolve(sim, arg(a, 0))
                .map_or(Value::Nil, |o| Value::str(&w.obj_name(sim, o)))),
            "GetEntityClass" => {
                let c = w.resolve(sim, arg(a, 0)).and_then(|o| w.obj_class(sim, o));
                one(c.map_or(Value::Nil, |c| Value::Ud(Ud::Class(w.class_id(&c)))))
            }
            "GetEntityClassPtr" | "FindEntityClass" => one(arg(a, 0)
                .as_str()
                .map_or(Value::Nil, |c| Value::Ud(Ud::Class(w.class_id(c))))),
            "GetObjectTeam" | "GetCommandPostTeam" => one(Value::Num(
                w.resolve(sim, arg(a, 0)).map_or(0, |o| w.obj_team(sim, o)) as f32,
            )),
            "GetEntityMatrix" => {
                let pos = w.resolve(sim, arg(a, 0)).and_then(|o| w.obj_pos(sim, o));
                one(pos.map_or(Value::Nil, |p| {
                    let idx = w.matrices.len() as u32;
                    w.matrices.push((p, 0.0));
                    Value::Ud(Ud::Matrix(idx))
                }))
            }
            "SetEntityMatrix" => {
                if let (Some(obj), Some((pos, yaw))) =
                    (w.resolve(sim, arg(a, 0)), w.point_of(sim, arg(a, 1)))
                {
                    match obj {
                        Ud::Unit(i, gen) => {
                            if let Some(u) = sim.unit_mut(core::unit::UnitHandle { index: i, gen })
                            {
                                u.soldier.pos = pos;
                                u.soldier.yaw = yaw;
                            }
                        }
                        Ud::Vehicle(i, gen) => {
                            if let Some(v) =
                                sim.vehicle_mut(core::vehicle::VehicleHandle { index: i, gen })
                            {
                                v.pos = pos;
                            }
                        }
                        Ud::Entity(ei) => {
                            if let Some(ent) = w.entities.get_mut(ei as usize) {
                                ent.2 = pos;
                            }
                        }
                        _ => {}
                    }
                }
                nothing()
            }
            "SetObjectTeam" => {
                let t = arg(a, 1).clone();
                w.set_property(sim, arg(a, 0), "team", &text(&t));
                nothing()
            }
            "IsObjectAlive" => one(Value::Bool(
                w.resolve(sim, arg(a, 0))
                    .is_some_and(|o| w.obj_alive(sim, o)),
            )),
            "GetObjectHealth" => one(w
                .resolve(sim, arg(a, 0))
                .and_then(|o| w.obj_health(sim, o))
                .map_or(Value::Nil, Value::Num)),
            "GetObjectShield" => {
                let (cur, max, add) = w
                    .resolve(sim, arg(a, 0))
                    .map_or((0.0, 0.0, 0.0), |o| w.obj_shield(sim, o));
                Ok(vec![Value::Num(cur), Value::Num(max), Value::Num(add)])
            }
            "EnableLockOn" | "EnableBuildingLockOn" => {
                let enable =
                    !matches!(arg(a, 1), Value::Bool(false)) && num(a, 1).is_none_or(|n| n != 0.0);
                if let Some(Ud::Station(si)) = w.resolve(sim, arg(a, 0)) {
                    sim.set_station_lock_on(si, enable);
                }
                nothing()
            }
            "GetObjectLastHitWeaponClass" => one(w
                .resolve(sim, arg(a, 0))
                .and_then(|o| w.last_hit_weapon.iter().find(|(x, _)| *x == o))
                .map_or(Value::Nil, |(_, c)| Value::str(c))),
            "GetCommandPostBleedValue" => one(Value::Num(0.0)),
            "SetProperty" => {
                let key = text(arg(a, 1));
                let val = text(arg(a, 2));
                w.set_property(sim, arg(a, 0), &key, &val);
                nothing()
            }
            "SetClassProperty" => {
                if let Some(cls) = arg(a, 0).as_str() {
                    let key = text(arg(a, 1));
                    let val = text(arg(a, 2));
                    crate::campaign_runtime::apply_class_property(
                        sim,
                        &w.team_classes,
                        cls,
                        &key,
                        &val,
                    );
                }
                nothing()
            }
            "KillObject" => {
                w.kill_object(sim, arg(a, 0), true);
                nothing()
            }
            "DeleteEntity" => {
                w.kill_object(sim, arg(a, 0), false);
                nothing()
            }
            "RespawnObject" => {
                w.respawn_object(sim, arg(a, 0));
                nothing()
            }
            "PlayAnimation" => {
                w.anims.play(&text(arg(a, 0)));
                nothing()
            }
            "PauseAnimation" => {
                w.anims.pause(&text(arg(a, 0)));
                nothing()
            }
            "RewindAnimation" => {
                w.anims.rewind(&text(arg(a, 0)));
                nothing()
            }
            "PlayAnimationFromTo" => {
                if let (Some(from), Some(to)) = (num(a, 1), num(a, 2)) {
                    w.anims.play_from_to(&text(arg(a, 0)), from, to);
                }
                nothing()
            }
            "SetAnimationStartPoint" => {
                w.anims.set_start_point(&text(arg(a, 0)), sim);
                nothing()
            }
            "CreateEntity" => {
                let class = arg(a, 0).as_str().unwrap_or_default().to_ascii_lowercase();
                let name = arg(a, 2).as_str().unwrap_or_default().to_owned();
                let pos = w
                    .point_of(sim, arg(a, 1))
                    .map(|(p, _)| p)
                    .unwrap_or(Vec3::ZERO);
                one(w.create_entity(sim, &class, &name, pos))
            }
            "GetFlagCarrier" => one(w
                .resolve(sim, arg(a, 0))
                .and_then(|o| match o {
                    Ud::Flag(f) => w.flags.get(usize::from(f)),
                    _ => None,
                })
                .and_then(|f| sim.ctf.as_ref()?.flags.get(f.ctf)?.carrier())
                .map_or(Value::Nil, |h| char_value(h.index))),

            _ => return None,
        })
    }
}
