//! The whole-state hash the replay gates read.

use super::*;

impl Sim {
    /// FNV-1a-64 over the sim state, bit-exact.
    pub fn hash(&self) -> u64 {
        let mut h: u64 = 0xCBF2_9CE4_8422_2325;
        let mut eat = |bytes: &[u8]| {
            for &b in bytes {
                h ^= b as u64;
                h = h.wrapping_mul(0x1_0000_01B3);
            }
        };
        eat(&self.tick.to_le_bytes());
        if let Some(career) = &self.medal_career {
            career.hash_into(&mut eat);
        }
        if !self.mission_ai.allow_blind_jet_jumps {
            eat(b"NO_BLIND_JET_HOPS");
        }
        eat(&self.difficulty.bytes());
        if self.team_difficulty.iter().any(Option::is_some) {
            for d in self.team_difficulty {
                eat(&d.map_or([0xff], crate::skill::Difficulty::bytes));
            }
        }
        // Card DTEAM: the per-team rows, only when a human surplus moves them
        // off the round's row, so a sim with equal humans (every bot-only
        // round) keeps the hash it had.
        let rows = self.team_rows();
        if rows != crate::skill::team_skill(self.difficulty, [0, 0]) {
            eat(b"DTEAM");
            for r in rows {
                eat(&r.tier.bytes());
                eat(&[r.level]);
            }
        }
        eat(&self.rng.state().to_le_bytes());
        eat(&self.lcg.state().to_le_bytes());
        if self.melee_lcg_active {
            eat(b"SABER_RNG");
            eat(&self.melee_lcg.state().to_le_bytes());
        }
        if let Some(space) = &self.space_assault {
            eat(b"SPACE_ASSAULT");
            space.hash_into(&mut eat);
        }
        if let Some(heroes) = &self.heroes {
            heroes.hash_into(&mut eat);
        }
        if let Some(deathmatch) = &self.deathmatch {
            deathmatch.hash_into(&mut eat);
        }
        if let Some(ctf) = &self.ctf {
            eat(b"CTF");
            ctf.hash_into(&mut eat);
        }
        for (team, roster) in self.test_rosters.iter().enumerate() {
            if let Some((len, hero)) = roster {
                eat(b"TEST_CLASSES");
                eat(&[team as u8, hero.unwrap_or(u8::MAX)]);
                eat(&(*len as u32).to_le_bytes());
            }
        }
        let ground = !self.space_round();
        for u in &self.units {
            u.hash_for_round(&mut eat, ground);
        }
        // The vehicles (VENT): nothing at all when there are none, like the
        // doors, so the replay values stay put until a vehicle exists.
        if !self.vehicles.is_empty() {
            eat(&(self.vehicles.len() as u32).to_le_bytes());
            for v in &self.vehicles {
                v.hash_into(&mut eat);
                if self.ground_rules == crate::soldier::GroundRules::Bf2
                    && v.kind == crate::vehicle::VehicleKind::Flyer
                {
                    if let Some(setup) = self
                        .seat_setup
                        .get(v.class as usize)
                        .and_then(Option::as_ref)
                    {
                        for (seat, (gun, state)) in setup.guns.iter().zip(&v.guns).enumerate() {
                            if let (Some(gun), Some(state)) = (gun, state) {
                                if gun.pilot {
                                    for (slot, (weapon, state)) in
                                        gun.weapons.iter().zip(&state.weapons).enumerate()
                                    {
                                        let count = weapon.fire_points.len();
                                        if count > 1 {
                                            let next = state.shots_fired % count as u64;
                                            if next != 0 {
                                                eat(b"FLYER_MUZZLE");
                                                eat(&(seat as u32).to_le_bytes());
                                                eat(&(slot as u32).to_le_bytes());
                                                eat(&next.to_le_bytes());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if (self.hero_rider_frames.is_some() || self.tow_frames.is_some())
                    && v.kind == crate::vehicle::VehicleKind::Walker
                {
                    eat(b"RIDER_LAYERS");
                    for blend in v.walker.layer_blend {
                        eat(&blend.to_bits().to_le_bytes());
                    }
                }
            }
        }
        for (index, path) in self.flyer_paths.iter().enumerate() {
            if !path.enabled {
                eat(b"FLIGHT_PATH_DISABLED");
                eat(&(index as u32).to_le_bytes());
            }
        }
        for tow in &self.tow_cables {
            tow.hash_into(&mut eat);
        }
        for fall in &self.tow_falls {
            fall.hash_into(&mut eat);
        }
        // HASH: a live bolt carries its predictive hit and historical shooter.
        // Empty fixtures keep their pins; live-bolt replays need a sim re-baseline.
        if !self.bolts.is_empty() {
            eat(b"BOLTS");
            eat(&(self.bolts.len() as u64).to_le_bytes());
            for b in &self.bolts {
                b.hash_into(&mut eat);
            }
        }
        if !self.beams.is_empty() {
            eat(b"BEAMS");
            eat(&(self.beams.len() as u64).to_le_bytes());
            for b in &self.beams {
                b.hash_into(&mut eat);
            }
        }
        if self.next_mine_id > 0 {
            eat(b"MINES");
            eat(&self.next_mine_id.to_le_bytes());
            eat(&(self.mines.len() as u64).to_le_bytes());
            for mine in &self.mines {
                mine.hash_into(&mut eat);
            }
        }
        if self.next_guided_id > 0 {
            eat(b"GUIDED");
            eat(&self.next_guided_id.to_le_bytes());
            eat(&(self.guided.len() as u64).to_le_bytes());
            for r in &self.guided {
                r.hash_into(&mut eat);
            }
        }
        if self.next_droid_id > 0 {
            eat(b"DROIDS");
            eat(&self.next_droid_id.to_le_bytes());
            eat(&(self.droids.len() as u64).to_le_bytes());
            for d in &self.droids {
                d.hash_into(&mut eat);
            }
            eat(&(self.strikes.len() as u64).to_le_bytes());
            for r in &self.strikes {
                r.hash_into(&mut eat);
            }
        }
        if self.next_turret_id > 0 {
            eat(b"TURRETS");
            eat(&self.next_turret_id.to_le_bytes());
            eat(&(self.turrets.len() as u64).to_le_bytes());
            for t in &self.turrets {
                t.hash_into(&mut eat);
            }
        }
        for g in &self.stickies {
            g.hash_into(&mut eat);
        }
        for t in &self.battle.teams {
            eat(&t.reinforcements.to_bits().to_le_bytes());
            eat(&t.alive_players.to_le_bytes());
            eat(&t.alive_ai.to_le_bytes());
        }
        for w in &self.waves {
            eat(&w.timer.to_bits().to_le_bytes());
            eat(&w.wave.to_le_bytes());
            eat(&w.next_slot.to_le_bytes());
        }
        self.hash_native_teams(&mut eat);
        // The two commander plans (Phase B), each length-prefixed with `None`
        // as its own sentinel byte.
        for p in &self.commander {
            p.hash_into(&mut eat);
        }
        // Length-prefixed magnets and players preserve slot identity; None
        // uses (u32::MAX, u32::MAX). Flattening Some values loses the slot.
        let handle_or_none =
            |h: Option<UnitHandle>| h.map_or((u32::MAX, u32::MAX), |h| (h.index, h.gen));
        // A vehicle magnet (card HTGT) carries a marker byte first, so a
        // unit magnet hashes as it did before vehicles could be one.
        eat(&(self.magnet.len() as u32).to_le_bytes());
        for m in &self.magnet {
            let (i, g) = match *m {
                Some(super::super::input::AimTarget::Vehicle(h)) => {
                    eat(&[0x56]);
                    (h.index, h.gen)
                }
                Some(super::super::input::AimTarget::Turret(id)) => {
                    eat(&[0x54]);
                    (id as u32, (id >> 32) as u32)
                }
                m => handle_or_none(m.and_then(super::super::input::AimTarget::unit)),
            };
            eat(&i.to_le_bytes());
            eat(&g.to_le_bytes());
        }
        self.lock_on_hash(&mut eat);
        self.foot_lock_hash(&mut eat);
        if self.magnet_hidden.iter().any(|&h| h != 0) {
            eat(&[0x4d]);
            eat(&self.magnet_hidden);
        }
        eat(&(self.players.len() as u32).to_le_bytes());
        for p in &self.players {
            let (i, g) = handle_or_none(*p);
            eat(&i.to_le_bytes());
            eat(&g.to_le_bytes());
        }
        // D9: length-prefixed per-slot spawn state; class tables are setup.
        eat(&(self.spawn_choice.len() as u32).to_le_bytes());
        for c in &self.spawn_choice {
            eat(&[*c]);
        }
        eat(&(self.prev_spawn_bits.len() as u32).to_le_bytes());
        for b in &self.prev_spawn_bits {
            eat(&b.to_le_bytes());
        }
        // Held prompt presses: no bytes while none is held, preserving pins.
        if self.held_eaten.iter().any(|h| !h.is_clear()) {
            eat(b"HELD");
            eat(&(self.held_eaten.len() as u32).to_le_bytes());
            for h in &self.held_eaten {
                eat(&h.0.to_le_bytes());
            }
        }
        // SPSEL: no bytes without a post pick, preserving idle hash pins;
        // otherwise a tag, table length and each slot's pick.
        if self.spawn_post_choice.iter().any(Option::is_some) {
            eat(b"SPSEL");
            eat(&(self.spawn_post_choice.len() as u32).to_le_bytes());
            for c in &self.spawn_post_choice {
                match c {
                    Some(p) => p.hash_into(&mut eat),
                    None => eat(&[0]),
                }
            }
        }
        // SSELA: no bytes for disabled/default screens; otherwise a tag,
        // switch and each slot's stage and side row.
        if self.spawn_screens
            || self
                .spawn_stage
                .iter()
                .any(|&s| s != super::super::spawn::SpawnStage::Accepted)
            || self
                .side_choice
                .iter()
                .any(|&c| c != super::super::spawn::SideChoice::Team1)
        {
            eat(b"SSELA");
            eat(&[u8::from(self.spawn_screens)]);
            eat(&(self.spawn_stage.len() as u32).to_le_bytes());
            for (st, side) in self.spawn_stage.iter().zip(&self.side_choice) {
                eat(&[*st as u8, *side as u8]);
            }
        }
        // The class carousel (card SSELB): NOTHING while every slot's is idle.
        if self
            .class_carousel
            .iter()
            .any(|c| *c != super::super::spawn::Carousel::default())
        {
            eat(b"SSELB");
            eat(&(self.class_carousel.len() as u32).to_le_bytes());
            for c in &self.class_carousel {
                eat(&[u8::from(c.armed), c.slide]);
            }
        }
        // Issue #28's ammo memory: NOTHING while no slot remembers a class.
        if self.class_ammo.iter().any(|m| !m.is_empty()) {
            eat(b"CLSCH");
            eat(&(self.class_ammo.len() as u32).to_le_bytes());
            for m in &self.class_ammo {
                eat(&(m.len() as u32).to_le_bytes());
                for c in m {
                    eat(&[c.class]);
                    eat(&(c.rounds.len() as u32).to_le_bytes());
                    for (slot, clip, spare) in &c.rounds {
                        eat(&[*slot]);
                        eat(&clip.to_le_bytes());
                        eat(&spare.to_le_bytes());
                    }
                }
            }
        }
        // The AI fill cursor: which class the next AI of each team takes.
        for n in &self.ai_spawned {
            eat(&n.to_le_bytes());
        }
        for c in &self.cps {
            c.hash_into(&mut eat);
        }
        if let Some((t, secs)) = self.domination {
            eat(&[t]);
            eat(&secs.to_bits().to_le_bytes());
        }
        eat(&[self.outcome.unwrap_or(255)]);
        // The doors (DR2): nothing at all when there are none.
        if !self.doors.is_empty() {
            eat(&(self.doors.len() as u32).to_le_bytes());
            for d in &self.doors {
                eat(&d.open.to_bits().to_le_bytes());
            }
            if self.doors.iter().any(|d| d.locked) {
                for d in &self.doors {
                    eat(&[u8::from(d.locked)]);
                }
            }
        }
        // The grenades on the ground (HOLD): nothing when there are none.
        if !self.hazards.is_empty() {
            eat(&(self.hazards.len() as u32).to_le_bytes());
            for z in &self.hazards {
                for v in [z.pos.x, z.pos.y, z.pos.z, z.radius] {
                    eat(&v.to_bits().to_le_bytes());
                }
                eat(&z.since.to_le_bytes());
                eat(&z.until.to_le_bytes());
                if let Some(o) = z.owner {
                    eat(&o.index.to_le_bytes());
                }
            }
        }
        // The vehicle pads (VPAD): nothing at all when there are none.
        if !self.vehicle_pads.is_empty() {
            eat(&(self.vehicle_pads.len() as u32).to_le_bytes());
            for p in &self.vehicle_pads {
                p.hash_into(&mut eat);
            }
        }
        // A fallen building turret's health: nothing while none is down.
        for (i, t) in self.building_turrets.iter().enumerate() {
            for p in &t.self_pilots {
                p.hash_into(&mut eat);
            }
            if let Some(health) = t.down {
                eat(&(i as u32).to_le_bytes());
                eat(&health.to_bits().to_le_bytes());
            }
        }
        // The items on the field (PWUP): nothing at all when there are none.
        if !self.pickups.is_empty() {
            eat(&(self.pickups.len() as u32).to_le_bytes());
            for p in &self.pickups {
                p.hash_into(&mut eat);
            }
        }
        // The stations (PWST): nothing at all when there are none.
        if !self.stations.is_empty() {
            eat(&(self.stations.len() as u32).to_le_bytes());
            for s in &self.stations {
                s.hash_into(&mut eat);
            }
        }
        // Card ARCM: nothing until a closing is counted.
        if self.arc_fails.iter().any(|t| !t.is_empty()) {
            eat(b"ARCM");
            for t in &self.arc_fails {
                eat(&(t.len() as u32).to_le_bytes());
                for f in t {
                    eat(&[f.n]);
                    eat(&f.last.to_le_bytes());
                }
            }
        }
        // Card TOKN: nothing until a token is dealt.
        if !self.tokens.is_empty() {
            eat(b"TOKN");
            eat(&(self.tokens.len() as u32).to_le_bytes());
            for t in &self.tokens {
                t.hash_into(&mut eat);
            }
        }
        // Card DFOCUS: nothing until a human is focused on.
        if !self.focus_born.is_empty() {
            eat(b"DFOCUS");
            eat(&(self.focus_born.len() as u32).to_le_bytes());
            for b in &self.focus_born {
                b.hash_into(&mut eat);
            }
        }
        // Positive assist timers are controller history, even without a target.
        let active = self.assist.iter().filter(|a| a.magnet_timer > 0.0).count();
        if active != 0 {
            eat(b"ASSIST_TIMER");
            eat(&(active as u64).to_le_bytes());
            for (slot, a) in self
                .assist
                .iter()
                .enumerate()
                .filter(|(_, a)| a.magnet_timer > 0.0)
            {
                eat(&(slot as u64).to_le_bytes());
                eat(&a.magnet_timer.to_bits().to_le_bytes());
            }
        }
        // The lead's state (ours): nothing while no slot has one.
        if self.assist_lead.iter().any(Option::is_some) {
            eat(b"ASSIST_LEAD");
            for (slot, (t, l)) in self
                .assist_lead
                .iter()
                .enumerate()
                .filter_map(|(k, a)| a.map(|a| (k, a)))
            {
                eat(&(slot as u64).to_le_bytes());
                t.hash_into(&mut eat);
                l.hash_into(&mut eat);
            }
        }
        // Warning identities and their allocator survive hazard compaction.
        if self.last_warning_id != 0 || self.hazards.iter().any(|z| z.warning_id.is_some()) {
            eat(b"GRENADE_WARNINGS");
            eat(&self.last_warning_id.to_le_bytes());
            eat(&(self.hazards.len() as u64).to_le_bytes());
            for (index, z) in self.hazards.iter().enumerate() {
                eat(&(index as u64).to_le_bytes());
                eat(&[u8::from(z.warning_id.is_some())]);
                if let Some(id) = z.warning_id {
                    eat(&id.to_le_bytes());
                }
            }
        }
        // Historical owners can outlive a unit slot's generation.
        if self.hazards.iter().any(|z| z.owner.is_some()) {
            eat(b"HAZARD_OWNER");
            eat(&(self.hazards.len() as u64).to_le_bytes());
            for (index, z) in self.hazards.iter().enumerate() {
                eat(&(index as u64).to_le_bytes());
                eat(&[u8::from(z.owner.is_some())]);
                if let Some(o) = z.owner {
                    eat(&o.index.to_le_bytes());
                    eat(&o.gen.to_le_bytes());
                }
            }
        }
        h
    }
}
