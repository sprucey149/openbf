//! The `World` methods the bindings call. `World`'s state and its pure queries
//! stay in the parent module; these are the script-visible writers: objective
//! and timer state, regions, characters, objects and properties. Keeping them
//! here makes the seam explicit: `campaign_bindings` only dispatches Host calls.
//!
//! Character ids of native members that never spawned use `PLACEHOLDER` from
//! `crate::campaign_bindings`, the script-facing definition.

use super::*;
use crate::campaign_bindings::{bf2_team, PLACEHOLDER};
use core::sim::CtfState;
use core::unit::Controller;

impl World {
    /// A message or voice-over is for the player: no team given, or theirs.
    pub(crate) fn for_player(&self, team: &Value) -> bool {
        bf2_team(team).is_none_or(|t| t == 0 || t == self.player_team + 1)
    }

    /// `FUN_0038abd8(text, state)`: activating an objective the player's
    /// team holds that is not already active posts
    /// `player1.objectivesUpdated` 0.25 s later and its disable 6.0 s later
    /// (`0x3e800000`, `0x40c00000` through `FUN_003b0558`).
    pub(crate) fn set_objective(&mut self, text: &Value, state: u8) {
        let Some(text) = text.as_str() else {
            return;
        };
        let team = self.player_team + 1;
        let mut post = false;
        for o in self
            .objectives
            .iter_mut()
            .filter(|o| o.text.eq_ignore_ascii_case(text))
        {
            if state == 2 && o.state != 2 && o.team == team {
                post = true;
            }
            o.state = state;
        }
        if post {
            self.hud_events.push((0.25, "objectivesUpdated"));
            self.hud_events.push((6.0, "objectivesUpdated.disable"));
        }
    }

    pub(crate) fn find_timer(&self, name: &str) -> Option<u32> {
        self.timers
            .iter()
            .position(|t| t.alive && t.name.eq_ignore_ascii_case(name))
            .map(|i| i as u32)
    }

    pub(crate) fn timer_of(&self, v: &Value) -> Option<u32> {
        match v {
            Value::Ud(Ud::Timer(t)) => self.timers.get(*t as usize).filter(|t| t.alive).map(|_| *t),
            Value::Str(s) => self.find_timer(s),
            _ => None,
        }
    }

    pub(crate) fn region_of(&self, v: &Value) -> Option<u32> {
        match v {
            Value::Ud(Ud::Region(r)) => Some(*r),
            Value::Str(s) => self.region_id(s),
            _ => None,
        }
    }

    pub(crate) fn clear_inside(&mut self, r: u32) {
        self.inside.retain(|(x, _)| *x != r);
    }

    pub(crate) fn clear_obj_markers(&mut self, obj: Ud) {
        self.markers.retain(|m| m.target != MarkTarget::Obj(obj));
    }

    pub(crate) fn char_of(&self, v: &Value) -> Option<u32> {
        match v {
            Value::Ud(Ud::Char(c)) => Some(*c),
            Value::Num(n) if *n >= 0.0 => Some(*n as u32),
            _ => None,
        }
    }

    pub(crate) fn char_team(&self, sim: &Sim, c: u32) -> Option<u8> {
        if let Some(u) = sim.units.get(self.char_slot(c) as usize) {
            return Some(u.team);
        }
        Some((c.checked_sub(PLACEHOLDER)? / 1000) as u8)
    }

    pub(crate) fn team_size(&self, t: u8) -> u32 {
        if t < 2 {
            self.team_units[usize::from(t)]
        } else {
            self.native_units.get(&t).copied().unwrap_or(0)
        }
    }

    /// The team's i-th member: its slots in order, then (for a native team)
    /// the members that never spawned. A native team's list is taken once
    /// and kept: a member spawned again lands in a new slot
    /// (`Sim::spawn_native_character`) and keeps its place through
    /// `char_alias`, where a fresh scan would shift every later member onto
    /// the units just spawned. Death Star's 45 s hangar
    /// `Ambush("hangar_ambush_spawn", 6, 5)` after `defend_ambush`'s three
    /// fielded three of its six.
    pub(crate) fn team_member(&mut self, sim: &Sim, t: u8, i: usize) -> Option<u32> {
        let slots: Vec<u32> = sim
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.team == t)
            .map(|(k, _)| k as u32)
            .collect();
        if t < 2 {
            return slots.get(i).copied();
        }
        let size = self.team_size(t) as usize;
        if !self.native_members.contains_key(&t) {
            let mut members: Vec<u32> = slots
                .iter()
                .map(|&s| {
                    self.char_alias
                        .iter()
                        .find(|&(_, &slot)| slot == s)
                        .map_or(s, |(&c, _)| c)
                })
                .collect();
            let first_free = members.len();
            members
                .extend((first_free..size).map(|k| PLACEHOLDER + u32::from(t) * 1000 + k as u32));
            self.native_members.insert(t, members);
        }
        self.native_members.get(&t)?.get(i).copied()
    }

    pub(crate) fn point_of(&self, sim: &Sim, v: &Value) -> Option<(Vec3, f32)> {
        match v {
            Value::Ud(Ud::Point(p, i)) => {
                let nodes = &self.paths.get(*p as usize)?.1;
                nodes.get(*i as usize).or(nodes.last()).copied()
            }
            Value::Ud(Ud::Matrix(i)) => self.matrices.get(*i as usize).copied(),
            Value::Str(s) => {
                if let Some(p) = self.path(s) {
                    return self.paths[p].1.first().copied();
                }
                let o = self.resolve_name(sim, s)?;
                self.obj_pos(sim, o).map(|p| (p, 0.0))
            }
            Value::Ud(u) => self.obj_pos(sim, *u).map(|p| (p, 0.0)),
            _ => None,
        }
    }

    /// `SpawnCharacter(char, point)` (`FUN_001916e8`): the character's
    /// member enters at the point, once (`Sim::spawn_native_character`).
    pub(crate) fn spawn_character(&mut self, sim: &mut Sim, c: u32, pos: (Vec3, f32)) {
        let scripted = self.scripted_chars.get(&c).cloned();
        if scripted.is_none() && self.char_unit(sim, c).is_some() {
            return;
        }
        let team = scripted
            .as_ref()
            .and_then(|s| s.team)
            .or_else(|| self.char_team(sim, c));
        let Some(team) = team else {
            return;
        };
        // A scripted hero is spawned with the class the script selected, on
        // the army it moved the member to, at the path point it asked for.
        // The native pool's own (already spawned) template is not used.
        if let Some(class) = scripted.as_ref().and_then(|s| s.class.as_deref()) {
            if team >= 2 {
                // The member enters once: one already on the field stays.
                if self.char_unit(sim, c).is_some() {
                    return;
                }
                let Some(ci) = self.native_classes.get(&team).and_then(|classes| {
                    classes.iter().position(|n| n.eq_ignore_ascii_case(class))
                }) else {
                    self.report(&format!(
                        "SpawnCharacter: {class} is not in team {}'s roster",
                        team + 1
                    ));
                    return;
                };
                if let Some(&v) = self.native_health.get(&class.to_ascii_lowercase()) {
                    sim.set_native_class_max_health(team, ci, v);
                }
                if let Some(h) = sim.spawn_native_class(team, ci, pos, self.char_slot(c)) {
                    if h.index != c {
                        self.char_alias.insert(c, h.index);
                    }
                }
                return;
            }
            let Some(ci) = self
                .team_classes
                .get(usize::from(team))
                .and_then(|classes| classes.iter().position(|n| n.eq_ignore_ascii_case(class)))
            else {
                self.report(&format!(
                    "SpawnCharacter: {class} is not in team {}'s roster",
                    team + 1
                ));
                return;
            };
            let Some(uc) = sim
                .classes
                .get(usize::from(team))
                .and_then(|classes| classes.get(ci))
            else {
                return;
            };
            let spec = uc.spec(
                team,
                pos.0,
                pos.1,
                Controller::Ai,
                ci as u8,
                sim.bot_skill(team),
            );
            if let Some(h) = sim.spawn(&spec) {
                self.char_alias.insert(c, h.index);
            }
            return;
        }
        let old = sim
            .units
            .get(self.char_slot(c) as usize)
            .filter(|u| u.team == team && !u.alive)
            .map(|u| u.spec.class);
        if let Some(h) = sim.spawn_native_member(team, pos, old) {
            if h.index != c {
                self.char_alias.insert(c, h.index);
            }
        }
    }

    pub(crate) fn obj_health(&self, sim: &Sim, o: Ud) -> Option<f32> {
        match o {
            Ud::Unit(i, _) => sim.units.get(i as usize).map(|u| u.health.cur),
            Ud::Vehicle(i, _) => sim.vehicles.get(i as usize).map(|v| v.health.cur),
            Ud::Station(i) => sim.stations.get(usize::from(i)).map(|s| s.health.cur),
            _ => None,
        }
    }

    pub(crate) fn obj_shield(&self, sim: &Sim, o: Ud) -> (f32, f32, f32) {
        match o {
            Ud::Station(i) => sim
                .stations
                .get(usize::from(i))
                .map_or((0.0, 0.0, 0.0), |s| {
                    (s.health.shield, s.health.shield_max, s.add_shield)
                }),
            Ud::Vehicle(i, g) => sim
                .vehicles
                .get(i as usize)
                .filter(|v| v.gen == g)
                .map_or((0.0, 0.0, 0.0), |v| {
                    (v.health.shield, v.health.shield_max, v.add_shield)
                }),
            Ud::Unit(i, g) => {
                sim.units
                    .get(i as usize)
                    .filter(|u| u.gen == g)
                    .map_or((0.0, 0.0, 0.0), |u| {
                        u.droideka
                            .as_ref()
                            .and_then(|d| {
                                Some((d.shield.points, u.droideka_class()?.shield.max, 0.0))
                            })
                            .unwrap_or((u.health.shield, u.health.shield_max, 0.0))
                    })
            }
            _ => (0.0, 0.0, 0.0),
        }
    }

    /// Top a main army back up to its unit count where it may spawn.
    /// Collect spawn points distributed round-robin across owned CPs (then
    /// command walkers), so a CP with 20 nodes (e.g. Kamino's `cp1` in the DNA
    /// hall) does not absorb an entire team before the second CP (`cp4`) gets
    /// any spawns.
    pub(crate) fn fill_enabled_ai(&mut self, sim: &mut Sim) {
        for t in 0..2usize {
            let team = t as u8;
            if !sim.ai_spawn_allowed[t] || self.team_units[t] == 0 {
                continue;
            }
            if !sim.classes[t].is_empty()
                && !(0..sim.classes[t].len()).any(|c| sim.is_class_enabled(team, c))
            {
                continue;
            }
            if sim.spawn_posts(team).is_empty() {
                continue;
            }
            let existing = sim
                .units
                .iter()
                .filter(|u| !u.is_player && u.team == team)
                .count() as u32;
            if existing >= self.team_units[t] {
                continue;
            }
            // Opening wave (`tick <= 1` or team 0's first wave after `onfirstspawn`)
            // spawns immediately by `AISpawnWeight` (`SWBFRNT2.ELF` `FUN_002e7240`,
            // lines 260573-260620). Mid-mission team activation (`tick > 1` on
            // non-opening teams, e.g. Kamino `DEF` when `cp1` + `cp4` open) enters
            // through `sim.waves[t]` (`FUN_0036c750`, lines 353853-353875) so bots
            // release on the team's spawn wave instead of materializing all at once.
            let use_wave = sim.tick > 1 && t != 0;
            for k in existing..self.team_units[t] {
                if use_wave {
                    sim.queue_ai_for_wave(team);
                } else {
                    sim.spawn_ai_at_drawn_post(team, k as usize);
                }
            }
        }
    }

    /// `AddUnitClass(team, class, min, max)`: the class joins the army's
    /// selectable list (and `GetCharacterClass`'s numbering) and returns its
    /// 0-based index in that list.
    pub(crate) fn add_unit_class(
        &mut self,
        sim: &mut Sim,
        t: u8,
        class: &str,
        min: Option<f32>,
        max: Option<f32>,
    ) -> Option<usize> {
        if t >= 2 {
            let known = self.native_classes.get(&t).and_then(|classes| {
                classes.iter().position(|c| c.eq_ignore_ascii_case(class))
            });
            // Only the setup can still add a class to a team the mission's
            // file holds none for.
            if known.is_some() || self.post_loaded {
                return known;
            }
            let rows = self.native_requests.entry(t).or_default();
            if let Some(at) = rows.iter().position(|r| r.0.eq_ignore_ascii_case(class)) {
                return Some(at);
            }
            rows.push((
                class.to_ascii_lowercase(),
                min.map_or(0, |m| m as u32),
                max.map(|m| m as u32),
            ));
            return Some(rows.len() - 1);
        }
        let ti = usize::from(t);
        let Some(ci) = self.team_classes[ti]
            .iter()
            .position(|c| c.eq_ignore_ascii_case(class))
        else {
            self.report(&format!(
                "AddUnitClass: {class} is not in team {}'s roster",
                t + 1
            ));
            return None;
        };
        if let Some(uc) = sim.classes[ti].get_mut(ci) {
            if let Some(m) = min {
                uc.min = m as u32;
            }
            if let Some(m) = max {
                uc.max = Some(m as u32);
            }
        }
        let idx = if let Some(idx) = self.class_order[ti].iter().position(|&c| c == ci) {
            idx
        } else {
            self.class_order[ti].push(ci);
            self.class_order[ti].len() - 1
        };
        sim.set_class_enabled(t, ci, true);
        self.pending_ai_fill = true;
        Some(idx)
    }

    /// `CreateEntity(class, point, name)`: a `flag` class becomes a
    /// carryable flag (`ObjectiveCTF` scores it in script: its
    /// `OnEnterRegion(captureRegion)` with the carrier); anything else is
    /// recorded by name and place.
    pub(crate) fn create_entity(
        &mut self,
        sim: &mut Sim,
        class: &str,
        name: &str,
        pos: Vec3,
    ) -> Value {
        let is_flag = self.class_bases.get(class).is_some_and(|b| b == "flag");
        if !is_flag {
            self.entities
                .push((name.to_ascii_lowercase(), class.to_owned(), pos));
            return Value::Ud(Ud::Entity(
                (self.instances.len() + self.entities.len() - 1) as u32,
            ));
        }
        let far = Vec3::new(0.0, -10000.0, 0.0);
        let ctf = sim.ctf.get_or_insert_with(|| {
            let mut c = CtfState::new_ground_one_flag(u32::MAX, pos, far, far);
            c.flags.clear();
            for r in &mut c.capture_regions {
                r.radius = 0.0;
            }
            c
        });
        let mut flag = core::sim::ctf::Flag::new_ground_neutral(pos);
        flag.allow_ai_pickup = self.allow_ai_pickup;
        flag.flyer_pickup_radius = 0.0;
        ctf.flags.push(flag);
        let ci = ctf.flags.len() - 1;
        self.flags.push(FlagEnt {
            name: name.to_owned(),
            class: class.to_owned(),
            ctf: ci,
            dead: false,
        });
        Value::Ud(Ud::Flag(self.flags.len() as u8 - 1))
    }

    pub(crate) fn kill_object(&mut self, sim: &mut Sim, v: &Value, notify: bool) {
        let Some(o) = self.resolve(sim, v) else {
            return;
        };
        self.clear_obj_markers(o);
        match o {
            Ud::Post(ci) => {
                let ci = usize::from(ci);
                if self.saved_cp_owners.len() <= ci {
                    self.saved_cp_owners.resize(sim.cps.len(), None);
                }
                // `find_cp` can also match a stale `post_keys` name: a name
                // from another map layer has no live post behind this index.
                if let Some(cp) = sim.cps.get_mut(ci) {
                    if cp.owner.is_some() {
                        self.saved_cp_owners[ci] = cp.owner;
                    }
                    cp.killed = true;
                    cp.owner = None;
                    cp.capture = 0.0;
                    cp.neutral = 0.0;
                }
            }
            Ud::Station(si) => {
                if sim
                    .stations
                    .get(usize::from(si))
                    .is_some_and(|s| !s.destroyed)
                {
                    if notify && self.post_loaded {
                        sim.kill_station(si, None);
                    } else {
                        sim.install_station_wreck(si);
                    }
                }
            }
            Ud::Vehicle(i, g) => {
                let pad = sim
                    .vehicles
                    .get(i as usize)
                    .filter(|v| v.gen == g)
                    .and_then(|v| v.spawn_pad);
                if let Some(p) = pad.and_then(|p| sim.vehicle_pads.get_mut(p as usize)) {
                    p.team = None;
                    p.armed = false;
                }
                if notify && self.post_loaded {
                    sim.destroy_vehicle(core::vehicle::VehicleHandle { index: i, gen: g }, None);
                } else if let Some(v) = sim.vehicles.get_mut(i as usize).filter(|v| v.gen == g) {
                    v.health.cur = 0.0;
                    v.alive = false;
                }
            }
            Ud::Unit(i, g) => {
                if sim
                    .units
                    .get(i as usize)
                    .is_some_and(|u| u.alive && u.gen == g)
                {
                    sim.kill_environment(
                        core::unit::UnitHandle { index: i, gen: g },
                        core::sim::DeathCause::DeathRegion,
                    );
                }
            }
            Ud::Flag(f) => {
                let Some(ent) = self.flags.get_mut(usize::from(f)).filter(|e| !e.dead) else {
                    return;
                };
                ent.dead = true;
                let ci = ent.ctf;
                if let Some(ctf) = sim.ctf.as_mut() {
                    if ci < ctf.flags.len() {
                        ctf.flags.remove(ci);
                    }
                    for e in self.flags.iter_mut().filter(|e| !e.dead && e.ctf > ci) {
                        e.ctf -= 1;
                    }
                    if ctf.flags.is_empty() {
                        sim.ctf = None;
                    }
                }
            }
            _ => {}
        }
        if let Value::Str(name) = v {
            for pad in sim
                .vehicle_pads
                .iter_mut()
                .filter(|p| p.def.name.eq_ignore_ascii_case(name))
            {
                pad.team = None;
                pad.armed = false;
            }
        }
    }

    pub(crate) fn respawn_object(&mut self, sim: &mut Sim, v: &Value) {
        let Some(o) = self.resolve(sim, v) else {
            return;
        };
        match o {
            Ud::Post(ci) => {
                let ci = usize::from(ci);
                if let Some(cp) = sim.cps.get_mut(ci) {
                    cp.killed = false;
                    if let Some(&cap) = self.saved_cp_captures.get(ci) {
                        cp.def.capture = cap;
                        cp.capture_authored = true;
                    }
                    if cp.owner.is_none() {
                        cp.owner = self.saved_cp_owners.get(ci).copied().flatten();
                        cp.capture = 0.0;
                        cp.neutral = 0.0;
                    }
                }
                self.pending_ai_fill = true;
            }
            Ud::Station(si) => sim.respawn_station(si),
            _ => {}
        }
    }

    /// `SetProperty(object, key, value)` for the keys the campaigns set
    /// that the sim models.
    pub(crate) fn set_property(&mut self, sim: &mut Sim, target: &Value, key: &str, val: &str) {
        let key = key.trim().to_ascii_lowercase();
        let val = val.trim();
        // The PS2 creates a scripted vehicle at setup, so the mission can
        // author DisableTime/Team before anyone can board it. Our pads spawn
        // later (spa3's LAAT after SpawnTime = 15 s) and its name also names a
        // placed marker, so a ScriptPostLoad write resolves to that marker and
        // the vehicle arrives enabled for the AI to fly off. Bring the pad's
        // vehicle out first; `resolve` then finds it ahead of the marker.
        if let Some(name) = target.as_str() {
            let lower = name.to_ascii_lowercase();
            if let Some(pi) = sim
                .vehicle_pads
                .iter()
                .position(|p| p.def.name.eq_ignore_ascii_case(&lower))
            {
                let live = sim.vehicle_pads[pi]
                    .vehicles
                    .iter()
                    .any(|pv| sim.vehicle(pv.handle).is_some_and(|v| v.alive));
                if !live {
                    let team = sim.vehicle_pads[pi].team;
                    sim.spawn_from_pad(pi, team);
                }
            }
        }
        let Some(o) = self.resolve(sim, target) else {
            return;
        };
        match (o, key.as_str()) {
            (Ud::Flag(f), "allowaipickup") => {
                let allow = val != "0";
                if let Some(flag) = self
                    .flags
                    .get(usize::from(f))
                    .and_then(|e| sim.ctf.as_mut()?.flags.get_mut(e.ctf))
                {
                    flag.allow_ai_pickup = allow;
                }
            }
            (Ud::Unit(i, _), "captureposts") => {
                if let Some(u) = sim.units.get_mut(i as usize) {
                    u.capture_posts = val != "0";
                }
            }
            (Ud::Entity(i), "isvisible") if (i as usize) < self.instances.len() => {
                if val == "0" || val.eq_ignore_ascii_case("false") {
                    self.hidden_instances.insert(i);
                } else {
                    self.hidden_instances.remove(&i);
                }
            }
            (Ud::Entity(i), "iscollidable" | "iscollidible")
                if (i as usize) < self.instances.len() =>
            {
                let saved = &mut self.disabled_prop_collision;
                if val == "0" || val.eq_ignore_ascii_case("false") {
                    saved.entry(i).or_insert_with(|| {
                        let layers: Vec<_> = sim
                            .collision
                            .bodies()
                            .iter()
                            .enumerate()
                            .filter(|(_, body)| body.instance == Some(i))
                            .map(|(index, body)| (index, body.layers))
                            .collect();
                        sim.collision.set_body_layers(
                            layers
                                .iter()
                                .map(|&(index, _)| (index, core::collision::Layers::NONE)),
                        );
                        layers
                    });
                } else if let Some(layers) = saved.remove(&i) {
                    sim.collision.set_body_layers(layers);
                }
            }
            (Ud::Entity(_), "islocked") => {
                let name = self.obj_name(sim, o);
                let locked = val.parse::<f32>().is_ok_and(|n| n != 0.0);
                for (di, d) in self.door_names.iter().enumerate() {
                    if !d.is_empty() && *d == name {
                        if let Some(door) = sim.doors.get_mut(di) {
                            door.locked = locked;
                        }
                    }
                }
            }
            (Ud::Post(ci), _) => self.set_post_property(sim, usize::from(ci), &key, val),
            (Ud::Station(si), _) => {
                set_station_property(sim, usize::from(si), &key, val);
                if key == "curhealth" && val.parse::<f32>().is_ok_and(|n| n <= 0.0) {
                    self.clear_obj_markers(o);
                }
            }
            (
                Ud::Vehicle(i, g),
                "curhealth" | "maxhealth" | "curshield" | "maxshield" | "addshield" | "team"
                | "disabletime",
            ) => {
                let Some(v) = sim.vehicles.get_mut(i as usize).filter(|v| v.gen == g) else {
                    return;
                };
                let Ok(n) = val.parse::<f32>() else {
                    return;
                };
                match key.as_str() {
                    "disabletime" => v.disable = n,
                    "team" => v.team = (n >= 1.0).then(|| n as u8 - 1),
                    k => {
                        set_health_property(&mut v.health, &mut v.add_shield, k, n);
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn set_post_property(&mut self, sim: &mut Sim, ci: usize, key: &str, val: &str) {
        // `find_cp` also matches authored `post_keys` strings, which can name
        // a post from an unloaded layer: no live CP sits at that index.
        if sim.cps.get(ci).is_none() {
            return;
        }
        match key {
            "isvisible" => sim.cps[ci].visible = val != "0",
            "team" => {
                if let Ok(team) = val.parse::<f32>() {
                    let owner = (team >= 1.0).then(|| team as u8 - 1);
                    if self.saved_cp_owners.len() <= ci {
                        self.saved_cp_owners.resize(sim.cps.len(), None);
                    }
                    self.saved_cp_owners[ci] = owner;
                    if !sim.cps[ci].killed {
                        sim.cps[ci].owner = owner;
                        sim.cps[ci].capture = 0.0;
                        sim.cps[ci].neutral = 0.0;
                    }
                    self.pending_ai_fill = true;
                }
            }
            "spawnpath" => {
                if val.is_empty() {
                    sim.cps[ci].def.spawn_nodes.clear();
                } else if let Some(p) = self.path(val) {
                    sim.cps[ci].def.spawn_nodes = self.paths[p].1.clone();
                    self.pending_ai_fill = true;
                }
            }
            "captureregion" => {
                let r = self.region_id(val).map(|r| self.regions[r as usize].1);
                let cp = &mut sim.cps[ci];
                match r {
                    Some(r) => {
                        cp.def.capture = r;
                        cp.capture_authored = true;
                        cp.capturable = true;
                        if let Some(saved) = self.saved_cp_captures.get_mut(ci) {
                            *saved = r;
                        }
                    }
                    None => {
                        cp.def.capture = Region::sphere(Vec3::new(0.0, -10000.0, 0.0), 0.0);
                        cp.capture_authored = false;
                        cp.capturable = false;
                    }
                }
            }
            "controlregion" => {
                sim.cps[ci].def.control = self.region_id(val).map(|r| self.regions[r as usize].1);
            }
            "aispawnweight" => {
                if let Ok(wt) = val.parse::<f32>() {
                    sim.cps[ci].ai_spawn_weight = wt.fmax(0.0);
                }
            }
            k if k.starts_with("value_atk_") || k.starts_with("value_def_") => {
                if let Ok(v) = val.parse::<f32>() {
                    let row = crate::walk::faction_row(&k["value_atk_".len()..]);
                    for t in 0..2 {
                        if self.team_value_rows[t] != row {
                            continue;
                        }
                        if k.starts_with("value_atk_") {
                            sim.cps[ci].def.value_atk[t] = v as i32;
                        } else {
                            sim.cps[ci].def.value_def[t] = v as i32;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Apply a script's health/shield property to a health pool. Returns false
/// for keys the caller handles itself (a station's death/respawn, a vehicle's
/// team and disable timer), so both callers share one clamp policy.
fn set_health_property(
    health: &mut core::weapon::Health,
    add_shield: &mut f32,
    key: &str,
    n: f32,
) -> bool {
    match key {
        "curhealth" => health.cur = n.fmin(health.max.fmax(n)),
        "maxhealth" => {
            health.max = n.fmax(1.0);
            health.cur = health.cur.fmin(health.max);
        }
        "curshield" => {
            health.shield = if health.shield_max > 0.0 {
                n.clamp(0.0, health.shield_max)
            } else {
                n.fmax(0.0)
            };
        }
        "maxshield" => {
            health.shield_max = n.fmax(0.0);
            health.shield = health.shield.fmin(health.shield_max);
        }
        "addshield" => *add_shield = n.fmax(0.0),
        _ => return false,
    }
    true
}

fn set_station_property(sim: &mut Sim, si: usize, key: &str, val: &str) {
    if matches!(key, "iscollidable" | "iscollidible") {
        let collidable = val != "0" && !val.eq_ignore_ascii_case("false");
        sim.set_station_collidable(si as u16, collidable);
        return;
    }
    let Ok(n) = val.parse::<f32>() else {
        return;
    };
    let Some(st) = sim.stations.get_mut(si) else {
        return;
    };
    match key {
        "team" => st.team = (n >= 1.0).then(|| n as u8 - 1),
        "curhealth" => {
            if n <= 0.0 {
                if !st.destroyed {
                    sim.install_station_wreck(si as u16);
                }
            } else {
                st.health.max = st.health.max.fmax(n);
                st.health.cur = n;
                if st.destroyed {
                    sim.respawn_station(si as u16);
                    sim.stations[si].health.cur = n;
                }
            }
        }
        k => {
            set_health_property(&mut st.health, &mut st.add_shield, k, n);
        }
    }
}
