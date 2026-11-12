//! Advance soldier animators from the current simulation state.

use super::{
    drawn, rider_now, seat_now, tp, turn_number, AnimInput, ClassAnim, Mat4, Rig, SeatClock,
    SoldierAsset, SoldierVisuals, UnitVisual, WalkSim,
};

impl SoldierVisuals {
    /// Advance every unit's animator from the sim state (`dt` = real seconds).
    pub fn update(&mut self, walk: &WalkSim, dt: f32) {
        self.update_with(walk, dt, None);
    }

    /// [`SoldierVisuals::update`], counting what each animator holds.
    pub fn update_with(
        &mut self,
        walk: &WalkSim,
        dt: f32,
        mut census: Option<&mut crate::animator::census::Census>,
    ) {
        self.note_shots(walk);
        self.step_hover_nines(walk);
        let fired: Vec<u32> = walk
            .sim
            .events
            .iter()
            .filter_map(|e| match e {
                core::sim::Event::Shot {
                    unit, weapon_class, ..
                } if !walk.sim.is_item_gun(*weapon_class as usize) => Some(unit.index),
                _ => None,
            })
            .collect();
        for (i, u) in walk.sim.units.iter().enumerate() {
            // Split the borrows by FIELD: the asset lives in `self.assets`,
            // the animator in `self.units`.
            let Some(ai) = self.asset_index(u.team, u.spec.class) else {
                continue;
            };
            let sa = &self.assets[ai];
            let v = &mut self.units[i];
            v.palette = Default::default();
            if !u.alive {
                v.dead_for += dt;
            }
            if fired.contains(&(i as u32)) {
                v.shot_tick = Some(walk.sim.tick);
            }
            if u.droideka.is_some() {
                continue;
            }
            // Seated in a turret that shows him (card VTURG) or on a
            // vehicle that does (card VRIDE): hung at the seat's hardpoint,
            // posed by the seat's clip.
            v.seat_model = None;
            let at = walk.blend.shown_at(walk.sim.tick);
            // A full-circle seat's yaw number: BF2 the yaw rate over the
            // turn speed (B2:189542-189546), BF1 the raw stick
            // (B1:137715-137719, T+0x78), which turns the yaw against its
            // sign on the PS2 as the port's `rx` does: -rate / speed.
            let bf2 = self.bf2;
            let turn = |n: f32| if bf2 { n } else { -n };
            let rider = u.mount.filter(|_| u.alive).and_then(|m| {
                rider_now(
                    &self.walker_rigs,
                    &self.hover_rigs,
                    &self.hover_nine,
                    walk,
                    m,
                    self.bf2,
                )
            });
            if let Some(r) = rider {
                let c = SeatClock::advance(&mut v.seat_clock, at, r.look.0, Mat4::IDENTITY);
                let n = turn_number(r.rate, c, r.yaw_rate);
                v.seat_model = Some(r.model);
                let Some(pose) = &r.seat.pilot_pose else {
                    continue;
                };
                if let Some(clip) = crate::animator::find_clip(&sa.asset.clips, &pose.clip()) {
                    let nine = pose.nine.then(|| match r.hover {
                        Some(h) => h.pose(),
                        None => tp::nine_pose(
                            -r.look.1,
                            r.look.0,
                            turn(n),
                            r.seat.pitch_limits,
                            r.seat.yaw_limits,
                        ),
                    });
                    v.animator
                        .seat(&sa.asset.clips, &sa.asset.skeleton, clip, nine, dt);
                }
                continue;
            }
            let seated = u
                .mount
                .filter(|_| u.alive)
                .and_then(|m| seat_now(&self.turrets, walk, m, self.bf2));
            let Some(s) = seated else {
                v.seat_clock = None;
                let Some(rig) = &sa.rig else { continue };
                Self::step_on_foot(
                    walk,
                    self.native
                        .get(&u.team)
                        .map_or(&self.anim[usize::from(u.team.min(1))], |side| &side.anim),
                    i,
                    u,
                    sa,
                    rig,
                    v,
                    &fired,
                    dt,
                    census.as_deref_mut(),
                );
                continue;
            };
            let held = s.hang.parent.map_or(Mat4::IDENTITY, |p| s.now[p]);
            let c = SeatClock::advance(&mut v.seat_clock, at, s.look.0, held);
            let n = turn_number(s.rate, c, s.yaw_rate);
            let mut now = s.now;
            if let (true, Some(p)) = (s.lags, s.hang.parent) {
                now[p] = c.held;
            }
            v.seat_model = Some(s.hang.matrix(s.model, &now));
            let Some(pose) = &s.seat.pilot_pose else {
                continue;
            };
            if let Some(clip) = crate::animator::find_clip(&sa.asset.clips, &pose.clip()) {
                let nine = pose.nine.then(|| {
                    tp::nine_pose(
                        -s.look.1,
                        s.look.0,
                        turn(n),
                        s.seat.pitch_limits,
                        s.seat.yaw_limits,
                    )
                });
                v.animator
                    .seat(&sa.asset.clips, &sa.asset.skeleton, clip, nine, dt);
            }
        }
        self.step_cloths(walk, dt);
        self.step_saber_trails(walk, dt);
        self.fp.update(walk, dt, &self.units);
    }

    /// One unit's animator on foot (not in a seat that shows him).
    #[allow(clippy::too_many_arguments)]
    fn step_on_foot(
        walk: &WalkSim,
        team: &[ClassAnim],
        i: usize,
        u: &core::unit::Unit,
        sa: &SoldierAsset,
        rig: &Rig,
        v: &mut UnitVisual,
        fired: &[u32],
        dt: f32,
        census: Option<&mut crate::animator::census::Census>,
    ) {
        let (wc, _, weapon) = drawn(u, &walk.sim.weapon_classes, rig.throws_offhand());
        let class = walk.sim.weapon_classes.get(wc);
        let anim = team.get(usize::from(u.spec.class).min(team.len().saturating_sub(1)));
        let set = match v.set {
            Some((c, set)) if c == wc => set,
            _ => {
                let bank = anim.zip(class).map_or("rifle", |(a, w)| a.bank_of(&w.name));
                let set = rig.weapon_set(anim.map_or("human", |a| &a.family), bank);
                v.set = Some((wc, set));
                set
            }
        };
        let s = &u.soldier;
        let pending_throw = v.pending_throw.take();
        let drop_shot = rig.game == crate::animator::Game::Bf1
            && pending_throw == Some(0)
            && class.is_some_and(|c| c.dispenser.is_some());
        let aim = if class.is_some_and(|c| c.saber.is_some()) {
            u.weapon
                .melee
                .as_ref()
                .map(|state| {
                    let direction = state.weapon_forward(u.aim_frame().0).normalize_or_zero();
                    let yaw = core::fmath::atan2(-direction.x, -direction.z) - s.yaw;
                    let yaw = (yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI;
                    (
                        yaw,
                        core::fmath::atan2(
                            direction.y,
                            (direction.x * direction.x + direction.z * direction.z).sqrt(),
                        ),
                    )
                })
                .unwrap_or((0.0, s.pitch))
        } else {
            (0.0, s.pitch)
        };
        let input = AnimInput {
            stance: s.stance,
            vel: crate::animator::body_vel(s.vel, s.yaw),
            yaw: s.yaw,
            mouse_aim: s.prev_buttons() & core::sim::buttons::MOUSE_AIM != 0,
            max_speed: u.consts.max_speed,
            root_scale: anim.map_or(1.0, |a| a.root_scale),
            dead: !u.alive,
            hero_dead: walk.sim.hero_class(u.team) == Some(u.spec.class)
                || walk
                    .team_template(u.team)
                    .class(u.spec.class)
                    .flee_like_a_hero,
            hard_lock: s.look_frozen(),
            set,
            fired: drop_shot
                || (u.mount.is_none()
                    && v.throwing.is_none()
                    && (v.repair_shot.take() == Some(wc) || fired.contains(&(i as u32)))),
            throw: pending_throw.filter(|_| !drop_shot),
            reloading: weapon.reloading(),
            reload_time: class.map_or(0.0, |c| c.reload_time),
            // PROVISIONAL and ours (FEEL_NOTES; the PS2's two soldier
            // flags are unread): he fires, holds the trigger (a charge
            // too), or has a target (a bot's; the player's aim magnet).
            // trace: PS2 soldier alert-flag writes and matching firing, charging and targeting
            // poses before replacing this alert predicate.
            alert: u.mount.is_none()
                && u.alive
                && (fired.contains(&(i as u32))
                    || (weapon.fire_cmd > 0 && class.is_none_or(|c| c.thrown.is_none()))
                    || u.ai.target.is_some()
                    || u.spec.player_slot.is_some_and(|p| {
                        walk.sim
                            .magnet
                            .get(usize::from(p))
                            .is_some_and(Option::is_some)
                    })),
            // The body faces the aim's yaw; the animator takes the legs'
            // twist off it.
            aim,
        };
        let cast = anim.and_then(|class| {
            v.cast
                .frame(u, &walk.sim.weapon_classes, &sa.asset.clips, class)
        });
        let custom = (rig.game == crate::animator::Game::Bf2 && u.alive && u.mount.is_none())
            .then(|| {
                let (name, _, _) = u.weapon.melee.as_ref()?.pose()?;
                let descriptor = anim?.custom.upper(name);
                Some((descriptor.aim, descriptor.blend_in))
            })
            .flatten();
        v.animator
            .offhand_aim(cast.as_ref().map(|frame| frame.aim).or(custom));
        if !u.alive || u.mount.is_some() {
            v.cast.clear();
        }
        if u.alive && u.mount.is_none() && cast.is_none() {
            if let (Some(state), Some(class)) = (&u.weapon.melee, anim) {
                if let Some((name, time, looping)) = state.pose() {
                    v.animator.combo_pose(
                        &sa.asset.clips,
                        class,
                        &walk.sim.weapon_classes[u.weapon_class].name,
                        name,
                        time,
                        looping,
                        state.movement_active,
                    );
                } else {
                    v.animator.clear_combo_pose();
                }
            } else {
                v.animator.clear_combo_pose();
            }
        } else {
            v.animator.clear_combo_pose();
        }
        v.animator
            .hero_jump(s.jump_animation.map(|d| (d, s.jump_animation_serial)));
        v.animator
            .step(rig, &sa.asset.clips, &sa.asset.skeleton, &input, dt);
        if u.alive && u.mount.is_none() {
            if let Some(class) = anim {
                if let Some(frame) = &cast {
                    v.animator.offhand_pose(
                        rig,
                        &sa.asset.clips,
                        &sa.asset.skeleton,
                        class,
                        &walk.sim.weapon_classes[u.weapon_class].name,
                        frame.animation,
                        frame.seconds,
                        frame.legs,
                    );
                }
            }
            if let Some(choked) = u.choked {
                let family = anim.map_or("human", |a| &a.family);
                v.animator
                    .status_pose(&sa.asset.clips, &sa.asset.skeleton, family, choked.age);
            }
        }
        v.cast.advance(dt);
        if let Some(c) = census {
            if u.alive && u.mount.is_none() {
                c.note(rig, &sa.asset.clips, &v.animator, &input);
                c.note_aim(rig, &sa.asset.skeleton, sa.hp_weapons, &v.animator, &input);
            }
        }
    }
}
