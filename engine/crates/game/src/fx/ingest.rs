//! Turning a tick's sim events into live effects.

use super::*;

impl Fx {
    /// [`Fx::ingest`] over the sim itself.
    pub fn ingest_sim(&mut self, sim: &Sim) {
        use core::sim::Event;
        self.tick = sim.tick;
        self.follow_turrets(sim);
        self.follow_station_damage(sim);
        self.follow_vehicle_damage(sim);
        // A link lives the record's FadeTime (18 ticks); its clock is the
        // sim tick, so a pause freezes it (card UORDE, `arc.rs`).
        self.arcs.retain(|a| a.alive(sim.tick));
        let mut arcs_this_tick = 0usize;
        // Riding effects follow their hull; a hull gone leaves them where
        // they are.
        for e in &mut self.live {
            if let Some((h, _)) = e.unit {
                if let Some(u) = sim.unit(h) {
                    e.follow_unit(u);
                } else {
                    e.stop();
                    e.unit = None;
                }
            }
            if let Some((h, local)) = e.ride {
                match sim.vehicle(h) {
                    Some(v) => {
                        e.frame.pos = v.pos + v.rot * local;
                        e.frame.vel = v.vel;
                    }
                    None => e.ride = None,
                }
            }
        }
        self.follow_rounds(sim);
        self.follow_repairs(sim);
        self.follow_buffs(sim);
        self.ingest_force(sim);
        self.follow_burns(sim);
        self.flame_shots(sim);
        let baked = self.has_effects();
        for e in &sim.events {
            match e {
                Event::Impact {
                    pos,
                    dir,
                    weapon_class,
                    surface,
                    vehicle,
                } if baked => {
                    let look = sim
                        .weapon_classes
                        .get(*weapon_class as usize)
                        .and_then(|w| w.ordnance.impact.as_ref());
                    // No look: a fixture weapon built without the class db
                    // — the showcase rifle's impact on every surface.
                    let name = match look {
                        Some(l) => l.effect(*surface),
                        None => Some(self.impact_name()),
                    };
                    let level =
                        (*surface == Surface::Water).then(|| sim.water_level.unwrap_or(pos.y));
                    if let Some(level) = level {
                        self.water
                            .spawn(ripple::BOLT, Vec3::new(pos.x, level, pos.z));
                    }
                    if let Some(name) = name {
                        let spawned = match (vehicle, level) {
                            (Some(h), _) if *surface == Surface::Vehicle => {
                                self.spawn_riding(name, *pos, *h, sim)
                            }
                            (_, Some(level)) => self.spawn_floating(name, *pos, level),
                            _ if matches!(*surface, Surface::Shield | Surface::ShieldLarge) => {
                                let normal = shield_normal(sim, *pos, *dir);
                                let frame = Frame {
                                    rot: super::trail::along(normal),
                                    pos: *pos + normal * SHIELD_EFFECT_LIFT,
                                    vel: Vec3::ZERO,
                                };
                                self.spawn(name, frame)
                            }
                            _ => self.spawn(name, Frame::at(*pos)),
                        };
                        if spawned {
                            self.queue_last_effect_sound(*pos);
                        }
                    }
                    if let Some(x) = look.and_then(|l| l.explosion.as_ref()) {
                        let at = *pos - *dir * EXPLOSION_OFFSET;
                        self.explode(x, at, sim.water_under(at));
                    }
                }
                // A blade's contact (B2 FUN_00340c60 336314-336333, FUN_0031d688,
                // FUN_0033e8b8 334744-334751): the authored effect, unriding.
                Event::SaberContact {
                    pos,
                    weapon_class,
                    deflect,
                } if baked => {
                    let saber = sim
                        .weapon_classes
                        .get(*weapon_class as usize)
                        .and_then(|w| w.saber.as_ref());
                    let name = saber.and_then(|s| {
                        if *deflect {
                            s.deflect_effect.as_deref()
                        } else {
                            s.hit_effect.as_deref()
                        }
                    });
                    if let Some(name) = name {
                        self.spawn(name, Frame::at(*pos));
                    }
                }
                Event::BoltExpired { pos, struck } if !baked => {
                    // Air expiry gets no puff: nothing was struck.
                    if *struck {
                        self.puffs.push(Puff {
                            // Lifted off the surface or half the quad is
                            // buried in the ground it just hit.
                            pos: *pos + Vec3::Y * 0.12,
                            kind: Kind::Impact,
                            team: 0,
                            age: 0.0,
                        });
                    }
                }
                Event::Hit { victim, .. } if !baked => {
                    if let Some(v) = sim.units.get(victim.index as usize) {
                        self.puffs.push(Puff {
                            pos: v.soldier.pos + Vec3::Y * HIT_HEIGHT,
                            kind: Kind::Hit,
                            team: v.team,
                            age: 0.0,
                        });
                    }
                }
                Event::MineBlast {
                    pos,
                    weapon_class,
                    cause,
                } => {
                    let look = sim
                        .weapon_classes
                        .get(*weapon_class as usize)
                        .and_then(|w| w.mine.as_ref())
                        .and_then(|d| d.item.explosion(*cause))
                        .map(|e| &e.look);
                    if let Some(look) = look {
                        self.explode(look, *pos, sim.water_under(*pos));
                    }
                }
                Event::GuidedGone {
                    pos,
                    weapon_class,
                    end,
                } => {
                    if let Some(e) = sim.weapon_classes[*weapon_class as usize]
                        .guided
                        .as_ref()
                        .and_then(|g| g.explosion(*end))
                    {
                        self.explode(&e.look, *pos, sim.water_under(*pos));
                    }
                }
                Event::DroidGone {
                    pos,
                    weapon_class,
                    end,
                } => {
                    let r = sim
                        .weapon_classes
                        .get(*weapon_class as usize)
                        .and_then(|w| w.remote.as_ref());
                    for x in r.into_iter().flat_map(|r| r.end_explosions(*end)) {
                        self.explode(&x.look, *pos, sim.water_under(*pos));
                    }
                }
                Event::TurretGone {
                    pos,
                    weapon_class,
                    end,
                } => {
                    let e = sim
                        .weapon_classes
                        .get(*weapon_class as usize)
                        .and_then(|w| w.turret.as_ref())
                        .and_then(|d| d.item.end_explosion(*end));
                    if let Some(e) = e {
                        self.explode(&e.look, *pos, sim.water_under(*pos));
                    }
                }
                // A thrown grenade's fuse ran out (GREN): its explosion
                // class's effect where it lay.
                Event::WeaponBlast { pos, weapon_class } => {
                    if let Some(explosion) = sim.weapon_classes[*weapon_class as usize]
                        .ordnance
                        .explosion
                        .as_ref()
                    {
                        self.explode(&explosion.look, *pos, sim.water_under(*pos));
                    }
                }
                Event::OrdnanceExpired { pos, weapon_class } => {
                    if let Some(e) = sim
                        .weapon_classes
                        .get(*weapon_class as usize)
                        .and_then(|w| w.ordnance.expire.as_ref())
                    {
                        self.explode(&e.look, *pos, sim.water_under(*pos));
                    }
                }
                Event::StickyBlast { pos, weapon_class } => {
                    let look = sim
                        .weapon_classes
                        .get(*weapon_class as usize)
                        .and_then(|w| w.ordnance.sticky.as_ref())
                        .and_then(|s| s.explosion.as_ref())
                        .map(|x| &x.look);
                    match look {
                        Some(look) if baked => {
                            self.explode(look, *pos, sim.water_under(*pos));
                        }
                        _ => self.puffs.push(Puff {
                            pos: *pos + Vec3::Y * 0.5,
                            kind: Kind::Blast(BLAST_MIN_SIZE),
                            team: 0,
                            age: 0.0,
                        }),
                    }
                }
                // An item was taken (PWUP, re-powerups §5.6): the taker's
                // `HealthGainEffect` / `AmmoGainEffect` / BF2's
                // `EnergyGainEffect` — the class defaults, which no disc
                // class overrides.
                // A station's give runs the same effects (PWST,
                // re-powerup-station §5.1); a vehicle served shows none.
                Event::ItemTaken {
                    unit,
                    health,
                    ammo,
                    energy,
                    ..
                }
                | Event::StationServed {
                    unit: Some(unit),
                    health,
                    ammo,
                    energy,
                    ..
                } => {
                    for (gave, kind, names) in [
                        (
                            *health,
                            Gain::Health,
                            ["Powerup_blue", "com_sfx_powerup_blue"],
                        ),
                        (*ammo, Gain::Ammo, ["Powerup_red", "com_sfx_powerup_red"]),
                        (*energy, Gain::Energy, ["", "com_sfx_powerup_green"]),
                    ] {
                        if gave {
                            self.gain(*unit, kind, names, sim);
                        }
                    }
                }
                Event::StationDestroyed { class, pos, .. } => {
                    if let Some(x) = sim
                        .station_classes
                        .get(usize::from(*class))
                        .and_then(|c| c.explosion.as_ref())
                    {
                        self.explode(&x.look, *pos, sim.water_under(*pos));
                    }
                }
                // A droid's chunk death (card DROIDDEATH): its class's
                // `ExplosionName` effect at the body.
                Event::DroidBurst {
                    pos, team, class, ..
                } => match sim.burst_explosion(*team, *class) {
                    Some(x) if baked => {
                        self.explode(&x.look, *pos, sim.water_under(*pos));
                    }
                    _ => self.puffs.push(Puff {
                        pos: *pos,
                        kind: Kind::Blast(1.5),
                        team: 0,
                        age: 0.0,
                    }),
                },
                Event::VehicleDestroyed { class, pos, .. } => {
                    let setup = sim
                        .blast_setup
                        .get(*class as usize)
                        .and_then(|b| b.as_ref());
                    if baked {
                        // Where the sim detonates it (`Sim::destroy_vehicle`).
                        if let Some(x) = setup.and_then(|b| b.explosion.as_ref()) {
                            self.explode(&x.look, *pos, sim.water_under(*pos));
                        }
                        continue;
                    }
                    let w = setup.map_or(0.0, |b| b.sphere_radius);
                    self.puffs.push(Puff {
                        pos: *pos + Vec3::Y * (w * 0.5),
                        kind: Kind::Blast((w * 1.5).max(BLAST_MIN_SIZE)),
                        team: 0,
                        age: 0.0,
                    });
                }
                // An emitter shot's link (card UORDE): the effect its
                // class's `LightningEffect` names. Render-only — the damage
                // is already in `Event::Hit` — and a class that names none
                // (the flamethrowers) draws no arc.
                Event::Emitter {
                    weapon_class,
                    from,
                    to,
                    ..
                } => {
                    let class = sim
                        .weapon_classes
                        .get(*weapon_class as usize)
                        .and_then(|w| w.emitter.as_ref())
                        .and_then(|e| e.lightning.as_deref());
                    if let Some(class) = class {
                        self.arcs.push(arc::Arc::new(
                            *from,
                            *to,
                            class,
                            sim.tick,
                            arcs_this_tick as u32,
                        ));
                        arcs_this_tick += 1;
                    }
                }
                Event::FlyerImpact { pos, normal, .. } => {
                    let candidates = [
                        "com_sfx_vehiclespark_sc",
                        "com_sfx_vehiclespark",
                        "vehiclespark",
                    ];
                    let mut spawned = false;
                    if baked {
                        let dir = normal.try_normalize().unwrap_or(Vec3::Y);
                        let z = dir;
                        let x = Vec3::Y.cross(z).try_normalize().unwrap_or(Vec3::X);
                        let frame = Frame {
                            pos: *pos,
                            rot: glam::Mat3::from_cols(x, z.cross(x), z),
                            vel: Vec3::ZERO,
                        };
                        for name in candidates {
                            if self.spawn(name, frame) {
                                spawned = true;
                                break;
                            }
                        }
                    }
                    if !spawned {
                        self.puffs.push(Puff {
                            pos: *pos,
                            kind: Kind::Hit,
                            team: 0,
                            age: 0.0,
                        });
                    }
                }
                _ => {}
            }
        }
    }
}

const SHIELD_EFFECT_LIFT: f32 = 0.1;
const SHIELD_NORMAL_PROBE: f32 = 2.0;

fn shield_normal(sim: &Sim, pos: Vec3, dir: Vec3) -> Vec3 {
    let back = -dir.normalize_or_zero();
    sim.collision
        .ray_with(
            pos + back * SHIELD_NORMAL_PROBE,
            -back,
            SHIELD_NORMAL_PROBE * 2.0,
            core::collision::Layers::ALL,
            core::collision::HullFilter::Static,
        )
        .and_then(|hit| hit.normal.try_normalize())
        .map_or(back, |n| if n.dot(back) < 0.0 { -n } else { n })
}
