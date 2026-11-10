//! BF2 hero offers and replacement lives. PS2 manager: 0x003ef9d0;
//! accept/decline: 0x003ef6c0/0x003ef7e8. Heroes keep their class health,
//! lose health with game time and regain it from enemy kills.

use super::{buttons, Event, PadState, Sim};
use crate::unit::{Controller, UnitHandle};
use crate::TICK_HZ;

/// The offered slot's buttons the hero prompt owns while it is up: the
/// answer and every arm sharing its physical button (D-pad Left / Right, F1
/// / F2) - the squad orders and the spawn map's left / right.
const PROMPT_BITS: u32 = buttons::HERO_ACCEPT
    | buttons::HERO_DECLINE
    | buttons::SQUAD_AT_EASE
    | buttons::SQUAD_FOLLOW
    | buttons::SQUAD_HOLD
    | buttons::SPAWN_POST_LEFT
    | buttons::SPAWN_POST_RIGHT;

/// How a team unlocks its hero: the shell's "Unlock Hero" row
/// (`HeroPrefs.iHeroUnlock`, `analysis/re-bf2-menus.md` §2.3) as the
/// manager's unlock rule `DAT_0060052c` reads it (FUN_003ef9d0,
/// B2:449257-449375).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeroUnlock {
    /// Rule 3: the team's best human reaches this many ordinary points
    /// (no human on the team: the sum of its members').
    Points(i32),
    /// Rule 5: this many seconds of game time have passed (strictly:
    /// `start + value < now`, B2:449356-449367).
    Timer(u32),
}

/// Who among the eligible members gets the offer: the "Assign To" row
/// (`iHeroPlayer` 1 / 4 / 7) as the manager's player rule `DAT_00600538`
/// scores them (B2:449132-449160): 1 the ordinary points, 4 their
/// negation, 7 no case - every score 0, so the game LCG picks among all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeroAssign {
    Best,
    Worst,
    Random,
}

/// The session's hero rules. Defaults: the retail Instant Action row (ELF
/// 0x005fa8f8..0x005fa94c) - unlock by 10 points, best player, a 30 s
/// offer, a new offer 90 s after the hero dies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeroRules {
    pub unlock: HeroUnlock,
    pub assign: HeroAssign,
    pub offer_ticks: u64,
    /// The "Timer" row (`iHeroRespawnVal`): ticks after the hero's death
    /// (and after a declined cycle) before the next offer; `Some(0)` is
    /// "Always", `None` "Never" - respawn rule 1, which never offers again
    /// once a hero was accepted (ScriptCB_SetNetHeroDefaults B2:44487-44495,
    /// FUN_003ef9d0 B2:449040-449043).
    pub respawn_ticks: Option<u64>,
    /// Disabled for normal PS2 single player: a team needs an eligible human.
    pub ai_candidates: bool,
    pub health: HeroHealthRules,
}

/// The PS2's conquest hero time drain, HP per game second (B2 `FUN_0034cea8`).
pub const PS2_TIME_DRAIN: f32 = 10.0;

/// PS2 hero health; class MaxHealth remains the disc's own value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeroHealthRules {
    pub streak_bounds: [u8; 2],
    pub heal_gains: [f32; 3],
    /// HP per game second; zero disables exhaustion for a supplied ruleset.
    pub time_drain_per_second: f32,
}

impl Default for HeroHealthRules {
    fn default() -> Self {
        Self {
            streak_bounds: [15, 30],
            heal_gains: [150.0, 100.0, 50.0],
            // [ours] Half the PS2's [`PS2_TIME_DRAIN`] (maintainer 2026-10-30).
            time_drain_per_second: 5.0,
        }
    }
}

impl HeroRules {
    /// Validate a supplied configuration before installing it in a battle.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.health.time_drain_per_second.is_finite() || self.health.time_drain_per_second < 0.0
        {
            return Err("hero time drain must be finite and nonnegative");
        }
        if self
            .health
            .heal_gains
            .iter()
            .any(|gain| !gain.is_finite() || *gain < 0.0)
        {
            return Err("hero healing must contain finite nonnegative gains");
        }
        match self.unlock {
            HeroUnlock::Points(n) if n < 0 => return Err("hero unlock points must be nonnegative"),
            _ => {}
        }
        if self.health.streak_bounds[0] == 0
            || self.health.streak_bounds[0] >= self.health.streak_bounds[1]
        {
            return Err("hero healing streak bounds must be positive and increasing");
        }
        Ok(())
    }
}

impl Default for HeroRules {
    fn default() -> Self {
        Self {
            unlock: HeroUnlock::Points(10),
            assign: HeroAssign::Best,
            offer_ticks: 30 * u64::from(TICK_HZ),
            respawn_ticks: Some(90 * u64::from(TICK_HZ)),
            ai_candidates: false,
            health: HeroHealthRules::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeroOffer {
    /// Member index, retained through deaths and respawns.
    pub member: u32,
    pub expires: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeroTeam {
    pub unlocked: bool,
    pub owner: Option<u32>,
    pub offer: Option<HeroOffer>,
    pub declined: Vec<u32>,
    pub ready_at: u64,
    /// The PS2's byte counter, reset on death (FUN_00362c60 onward).
    pub kill_streak: u8,
    /// The owner is on the field AS the hero. The ownership ends when that
    /// soldier is gone (FUN_0034f650 calls FUN_003ef328 only for a hero
    /// soldier, B2:346003-346033); a life in another class keeps it
    /// (FUN_0034e1f0 B2:345046-345058).
    pub spawned: bool,
    /// The owner's unit generation when he accepted: a dead hero-class
    /// unit of a later generation is a hero life that ended.
    pub owner_gen: u32,
    /// Respawn "Never": a hero was accepted and is gone - no more offers.
    pub exhausted: bool,
}

#[derive(Clone, Debug)]
pub struct Heroes {
    /// Separate from the ordinary AI mix; these are appended roster entries.
    pub classes: [Option<u8>; 2],
    pub rules: HeroRules,
    pub teams: [HeroTeam; 2],
    /// Round scores supplied by the same event feed as the spawn-screen gate.
    /// A cache of the existing score authority, hashed because the next tick
    /// uses it for eligibility and candidate selection.
    pub points: Vec<i32>,
    pub prev_buttons: Vec<u32>,
}

impl Heroes {
    pub fn new(classes: [Option<u8>; 2]) -> Self {
        Self {
            classes,
            rules: HeroRules::default(),
            teams: Default::default(),
            points: Vec::new(),
            prev_buttons: Vec::new(),
        }
    }

    pub fn with_rules(classes: [Option<u8>; 2], rules: HeroRules) -> Result<Self, &'static str> {
        rules.validate()?;
        Ok(Self {
            rules,
            ..Self::new(classes)
        })
    }

    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"HERO");
        for c in self.classes {
            eat(&[c.unwrap_or(u8::MAX)]);
        }
        match self.rules.unlock {
            HeroUnlock::Points(n) => {
                eat(&[3]);
                eat(&n.to_le_bytes());
            }
            HeroUnlock::Timer(n) => {
                eat(&[5]);
                eat(&n.to_le_bytes());
            }
        }
        eat(&[self.rules.assign as u8]);
        eat(&self.rules.offer_ticks.to_le_bytes());
        eat(&self.rules.respawn_ticks.unwrap_or(u64::MAX).to_le_bytes());
        eat(&[u8::from(self.rules.ai_candidates)]);
        eat(&self.rules.health.streak_bounds);
        for gain in self.rules.health.heal_gains {
            eat(&gain.to_bits().to_le_bytes());
        }
        eat(&self
            .rules
            .health
            .time_drain_per_second
            .to_bits()
            .to_le_bytes());
        for t in &self.teams {
            eat(&[u8::from(t.unlocked)]);
            eat(&t.owner.unwrap_or(u32::MAX).to_le_bytes());
            eat(&t.offer.map_or(u32::MAX, |o| o.member).to_le_bytes());
            eat(&t.offer.map_or(0, |o| o.expires).to_le_bytes());
            eat(&t.ready_at.to_le_bytes());
            eat(&[t.kill_streak, u8::from(t.spawned), u8::from(t.exhausted)]);
            eat(&t.owner_gen.to_le_bytes());
            eat(&(t.declined.len() as u32).to_le_bytes());
            for member in &t.declined {
                eat(&member.to_le_bytes());
            }
        }
        eat(&(self.prev_buttons.len() as u32).to_le_bytes());
        for bits in &self.prev_buttons {
            eat(&bits.to_le_bytes());
        }
        eat(&(self.points.len() as u32).to_le_bytes());
        for points in &self.points {
            eat(&points.to_le_bytes());
        }
    }
}

impl Sim {
    /// FUN_0034cea8 (0x34d0f4..1ac) sets HP -= dt * 10 directly, retaining
    /// the last damage attribution. It drains the soldier even in a vehicle
    /// and bypasses ordinary damage modifiers and the damage invincibility
    /// cheat. [ours] Configured AI heroes use the same rate as humans.
    pub(super) fn tick_hero_health(&mut self, dt: f32) {
        if self.outcome.is_some() {
            return;
        }
        let Some(heroes) = self.heroes.as_ref() else {
            return;
        };
        let rate = heroes.rules.health.time_drain_per_second;
        if rate == 0.0 {
            return;
        }
        let owners = [0, 1].map(|team| {
            heroes.teams[team]
                .owner
                .zip(heroes.classes[team])
                .map(|(member, class)| (team, member, class))
        });
        for (team, member, class) in owners.into_iter().flatten() {
            let i = member as usize;
            let Some(u) = self.units.get(i) else {
                continue;
            };
            if !u.alive
                || usize::from(u.team) != team
                || u.spec.class != class
                || (u.is_player && self.hero_human_member(member).is_none())
            {
                continue;
            }
            let hero = self.handle(i);
            self.units[i].health.cur -= dt * rate;
            if !self.units[i].health.alive() {
                let killer = self.units[i].last_hit.map(|(h, _)| h).filter(|h| {
                    self.units
                        .get(h.index as usize)
                        .is_some_and(|u| u.gen == h.gen)
                });
                self.unit_killed_with_cause(hero, killer, super::DeathCause::HeroExhausted);
            }
        }
    }

    /// PS2 enemy-kill credit FUN_00362268 (0x36240c..0x362488): the
    /// current-life counter is tested before its increment.
    pub(super) fn heal_hero_kill(&mut self, killer: UnitHandle) {
        if !self.is_hero(killer) {
            return;
        }
        let team = usize::from(self.units[killer.index as usize].team);
        let Some(heroes) = self.heroes.as_mut() else {
            return;
        };
        let state = &mut heroes.teams[team];
        if state.owner != Some(killer.index) {
            return;
        }
        let health_rules = heroes.rules.health;
        let bucket = if state.kill_streak < health_rules.streak_bounds[0] {
            0
        } else if state.kill_streak < health_rules.streak_bounds[1] {
            1
        } else {
            2
        };
        let gain = health_rules.heal_gains[bucket];
        state.kill_streak = state.kill_streak.wrapping_add(1);
        let health = &mut self.units[killer.index as usize].health;
        health.cur = (health.cur + gain).min(health.max);
    }

    /// A registered member may be dead behind the spawn screen. A departed
    /// player's old unit slot is no longer a human candidate.
    fn hero_human_member(&self, member: u32) -> Option<u8> {
        let u = self.units.get(member as usize)?;
        let slot = u.spec.player_slot?;
        let handle = UnitHandle {
            index: member,
            gen: u.gen,
        };
        (u.is_player && self.players.get(usize::from(slot)).copied().flatten() == Some(handle))
            .then_some(slot)
    }

    pub fn hero_class(&self, team: u8) -> Option<u8> {
        self.heroes
            .as_ref()?
            .classes
            .get(usize::from(team))
            .copied()
            .flatten()
    }

    pub fn is_hero(&self, h: UnitHandle) -> bool {
        self.unit(h)
            .is_some_and(|u| self.hero_class(u.team) == Some(u.spec.class))
    }

    pub fn hero_offer(&self, slot: usize) -> Option<(u8, f32)> {
        let h = self.players.get(slot).copied().flatten()?;
        let u = self.unit(h)?;
        let heroes = self.heroes.as_ref()?;
        let offer = heroes.teams.get(usize::from(u.team))?.offer?;
        (offer.member == h.index).then(|| {
            (
                heroes.classes[usize::from(u.team)].unwrap(),
                offer.expires.saturating_sub(self.tick) as f32 / TICK_HZ as f32,
            )
        })
    }

    /// The hero manager, once a tick (FUN_003ef9d0 per team). The offer is
    /// the PS2's: a live human is offered the hero for 30 s (Yes / No on the
    /// d-pad); No or the timeout declines it; a Yes removes his soldier
    /// without a death and opens the spawn map; if he loses his soldier
    /// while the offer is up he is the owner anyway (B2:448990-449000). The
    /// owner keeps the hero through lives in other classes until his hero
    /// soldier is gone ([`HeroTeam::spawned`]); then the respawn rule runs.
    pub(super) fn step_heroes(&mut self, pads: &[PadState]) -> Option<Vec<PadState>> {
        let mut filtered = pads
            .iter()
            .any(|p| p.buttons & (buttons::HERO_ACCEPT | buttons::HERO_DECLINE) != 0)
            .then(|| {
                pads.iter()
                    .map(|p| PadState {
                        buttons: p.buttons & !(buttons::HERO_ACCEPT | buttons::HERO_DECLINE),
                        ..*p
                    })
                    .collect::<Vec<_>>()
            });
        let Some(mut heroes) = self.heroes.take() else {
            return filtered;
        };
        if self.outcome.is_some() {
            self.heroes = Some(heroes);
            return filtered;
        }
        heroes.prev_buttons.resize(pads.len(), 0);
        for team in 0..2usize {
            let Some(class) = heroes.classes[team] else {
                continue;
            };
            let state = &mut heroes.teams[team];
            if let Some(member) = state.owner {
                let u = self.units.get(member as usize);
                let human = u.is_some_and(|u| u.is_player);
                let kept = u.is_some_and(|u| {
                    u.team as usize == team
                        && (!u.is_player || self.hero_human_member(member).is_some())
                });
                let hero_now = u.is_some_and(|u| u.alive && u.spec.class == class);
                // A hero life since the accept that is over (a death the
                // tick it began included).
                let hero_died = u
                    .is_some_and(|u| !u.alive && u.spec.class == class && u.gen != state.owner_gen);
                // An AI owner (a mode admitting AI candidates) that came
                // back in another class has no hero life to wait for.
                let ai_lost = !human && u.is_some_and(|u| u.alive) && !hero_now;
                let gone = !kept || (state.spawned && !hero_now) || hero_died || ai_lost;
                if gone {
                    state.owner = None;
                    state.spawned = false;
                    state.kill_streak = 0;
                    state.declined.clear();
                    match heroes.rules.respawn_ticks {
                        Some(t) => state.ready_at = self.tick + t,
                        None => state.exhausted = true,
                    }
                    if let Some(slot) = u.and_then(|u| u.spec.player_slot) {
                        if self.spawn_choice.get(usize::from(slot)) == Some(&class) {
                            self.spawn_choice[usize::from(slot)] = 0;
                        }
                    }
                    if let Some((_, original)) = self.test_rosters[team] {
                        if heroes.classes[team] != original {
                            heroes.classes[team] = original;
                            continue;
                        }
                    }
                } else {
                    state.spawned = hero_now;
                }
            }
            if let Some(offer) = state.offer {
                let slot = self.hero_human_member(offer.member);
                if let Some(slot) = slot.filter(|s| usize::from(*s) < pads.len()) {
                    let s = usize::from(slot);
                    let output = filtered.get_or_insert_with(|| pads.to_vec());
                    output[s].buttons &= !PROMPT_BITS;
                    // The press that answers the offer (or is held across
                    // it) stays out of play until released: the squad order
                    // and spawn-map arm on the same button do not fire on the
                    // next tick, when the offer is gone.
                    if self.held_eaten.len() <= s {
                        self.held_eaten.resize(s + 1, Default::default());
                    }
                    self.held_eaten[s].eat(pads[s].buttons & PROMPT_BITS);
                }
                let pressed = slot
                    .and_then(|s| {
                        pads.get(usize::from(s))
                            .map(|p| p.buttons & !heroes.prev_buttons[usize::from(s)])
                    })
                    .unwrap_or(0);
                // B2:448990-449000: the offered member has no soldier any
                // more (he died, or left for the spawn screens) - accepted.
                let lost_soldier = slot.is_some()
                    && self
                        .units
                        .get(offer.member as usize)
                        .is_some_and(|u| !u.alive);
                if lost_soldier
                    || (slot.is_some()
                        && pressed & buttons::HERO_ACCEPT != 0
                        && self.tick <= offer.expires)
                {
                    let gen = self.units[offer.member as usize].gen;
                    if self.accept_hero_member(offer.member, class) {
                        state.owner = Some(offer.member);
                        state.owner_gen = gen;
                        state.kill_streak = 0;
                        state.spawned = false;
                    }
                    state.offer = None;
                } else if slot.is_none()
                    || self.tick > offer.expires
                    || pressed & buttons::HERO_DECLINE != 0
                {
                    state.declined.push(offer.member);
                    state.offer = None;
                }
            }
            if state.owner.is_some()
                || state.offer.is_some()
                || state.exhausted
                || (state.ready_at != 0 && self.tick <= state.ready_at)
            {
                continue;
            }
            let members: Vec<_> = self
                .units
                .iter()
                .enumerate()
                .filter(|(i, u)| {
                    u.team as usize == team
                        && u.gen != 0
                        && (!u.is_player || self.hero_human_member(*i as u32).is_some())
                })
                .collect();
            let humans: Vec<_> = members.iter().filter(|(_, u)| u.is_player).collect();
            let score = |i: usize| heroes.points.get(i).copied().unwrap_or(0);
            if !state.unlocked {
                state.unlocked = match heroes.rules.unlock {
                    HeroUnlock::Points(n) => {
                        let earned = if humans.is_empty() {
                            members.iter().map(|(i, _)| score(*i)).sum()
                        } else {
                            humans.iter().map(|(i, _)| score(*i)).max().unwrap_or(0)
                        };
                        earned >= n
                    }
                    HeroUnlock::Timer(secs) => self.tick > u64::from(secs) * u64::from(TICK_HZ),
                };
            }
            if !state.unlocked {
                continue;
            }
            // The PS2 resets this cycle when no human on the team remains
            // eligible. AI fallback is explicit for modes which admit it.
            let eligible: Vec<_> = members
                .iter()
                .filter(|(i, u)| {
                    !state.declined.contains(&(*i as u32))
                        && (u.is_player
                            || (heroes.rules.ai_candidates && u.spec.controller == Controller::Ai))
                })
                .collect();
            let human_exists = eligible.iter().any(|(_, u)| u.is_player);
            let rank = |i: usize| match heroes.rules.assign {
                HeroAssign::Best => score(i),
                HeroAssign::Worst => -score(i),
                HeroAssign::Random => 0,
            };
            let best = eligible
                .iter()
                .filter(|(_, u)| !human_exists || u.is_player)
                .map(|(i, _)| rank(*i))
                .max();
            let Some(best) = best else {
                state.declined.clear();
                match heroes.rules.respawn_ticks {
                    Some(t) => state.ready_at = self.tick + t,
                    // Respawn rule 1 clears the unlock instead
                    // (B2:449095-449103): it is earned again at once.
                    None => {
                        state.unlocked = false;
                        state.ready_at = 0;
                    }
                }
                continue;
            };
            let tied: Vec<_> = eligible
                .iter()
                .filter(|(i, u)| (!human_exists || u.is_player) && rank(*i) == best)
                .map(|(i, _)| *i as u32)
                .collect();
            let member = tied[if tied.len() == 1 {
                0
            } else {
                self.lcg.index(tied.len() as u32) as usize
            }];
            if self.units[member as usize].is_player {
                // A human without a soldier is not offered: the PS2 drops
                // the pick and tries again next update (B2:449210-449214).
                if self.units[member as usize].alive {
                    state.offer = Some(HeroOffer {
                        member,
                        expires: self.tick + heroes.rules.offer_ticks,
                    });
                    self.events.push(Event::HeroOffered {
                        unit: self.handle(member as usize),
                        class,
                    });
                }
            } else {
                let gen = self.units[member as usize].gen;
                if self.accept_hero_member(member, class) {
                    state.owner = Some(member);
                    state.owner_gen = gen;
                    state.kill_streak = 0;
                    state.spawned = false;
                }
            }
        }
        for (old, pad) in heroes.prev_buttons.iter_mut().zip(pads) {
            *old = pad.buttons;
        }
        self.heroes = Some(heroes);
        filtered
    }

    /// The member `member` becomes the owner: a live soldier is removed
    /// with no death, ticket or drop (AcceptHero 0x18cfc8 / the pad's
    /// accept, B2:234886-234899), and a human goes to the spawn map, then
    /// the class screen with the hero chosen. With the screens off (a
    /// headless run) he is put back at once as the hero; a refused spawn
    /// keeps him the owner and the ordinary respawn retries it.
    fn accept_hero_member(&mut self, member: u32, class: u8) -> bool {
        let i = member as usize;
        let Some(u) = self.units.get(i) else {
            return false;
        };
        if u.is_player && self.hero_human_member(member).is_none() {
            return false;
        }
        let (team, slot, alive, gen) = (u.team, u.spec.player_slot, u.alive, u.gen);
        if usize::from(class) >= self.classes[usize::from(team.min(1))].len() {
            return false;
        }
        let h = UnitHandle { index: member, gen };
        if alive {
            let t = &mut self.battle.teams[usize::from(team.min(1))];
            if slot.is_some() {
                t.alive_players = t.alive_players.saturating_sub(1);
            } else {
                t.alive_ai = t.alive_ai.saturating_sub(1);
            }
            self.unseat_unit(h);
            crate::squad::on_unit_died(self, h);
            self.class_change_life_ended(h);
            self.events.push(Event::Despawn { unit: h });
            self.retire_saber_flights(i);
            let u = &mut self.units[i];
            u.alive = false;
            u.body = false;
            u.drop_due = false;
        }
        let u = &mut self.units[i];
        u.spec.class = class;
        u.spawn_slot = None;
        if let Some(slot) = slot {
            self.sync_slot_tables();
            let s = usize::from(slot);
            self.spawn_choice[s] = class;
            if self.spawn_screens {
                self.units[i].respawn_at = None;
                // A member already on the screens stays where he is.
                if !matches!(
                    self.spawn_stage.get(s),
                    Some(super::SpawnStage::Map | super::SpawnStage::Class)
                ) {
                    self.open_spawn_map(s, team);
                }
            } else {
                self.units[i].respawn_at = Some(self.tick);
                self.respawn_slot_with_class(i, Some(class));
            }
        } else {
            self.units[i].respawn_at = Some(self.tick);
            self.respawn_slot_with_class(i, Some(class));
        }
        true
    }

    /// The hero class player slot `s` owns on `team`, if it owns one: the
    /// class screen opens on it (FUN_00396a00 selects the hero row for the
    /// owner).
    pub(super) fn owned_hero_class(&self, s: usize, team: u8) -> Option<u8> {
        let member = self.players.get(s).copied().flatten()?;
        let heroes = self.heroes.as_ref()?;
        let state = heroes.teams.get(usize::from(team))?;
        (state.owner == Some(member.index) && state.unlocked)
            .then(|| heroes.classes[usize::from(team)])
            .flatten()
    }
}

#[cfg(test)]
mod tests;
