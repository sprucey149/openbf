//! The commander's voice-overs, fed from the sim (card ANNC): the mission's
//! lines and the posts' `VO_*` tables go into `core::announcer`'s rules, this
//! frame's requests onto its voice queue, and whatever the queue starts is
//! played through the caller's `SoundLib` (`main/windowed.rs`). Presentation
//! only: nothing here writes to the sim. RE: `analysis/re-announcer.md`.
//!
//! Post events are read off the OWNER changes between two frames, not off
//! `Sim::events`: a frame that runs several ticks keeps only the last tick's
//! events, and a capture must not be missed.

use crate::layout::MapLayout;
use crate::mission::{MissionDef, VoiceDef};
use crate::walk::WalkSim;
use core::announcer::{
    faction_index, Announcer, BattleLines, Line, PostEvent, PostLines, ScriptBleed, VoiceQueue,
};
use core::sim::Sim;
use core::unit::UnitHandle;

/// `name` as `faction`'s commander says it: the mission's line names lead
/// with the speaker's faction (`cis_off_com_report_enemy_losing`), so another
/// army's line of the same role differs in that prefix alone. None when the
/// name has no faction prefix to replace.
fn line_for(name: &str, faction: &str) -> Option<String> {
    let (head, rest) = name.split_once('_')?;
    faction_index(head)?;
    Some(format!("{faction}_{rest}"))
}

/// What `listener`'s commander says when he leaves the boundary: the script's
/// `SetOutOfBoundsVoiceOver` for the team of his army (`re-boundary.md` T1b). An army
/// the mission does not have takes the line of its slot, in its own commander's name.
/// A script that sets none says nothing, as some BF1 scripts do.
pub fn leaving_line(
    voice: &VoiceDef,
    mission_factions: [&str; 2],
    factions: [&str; 2],
    listener: usize,
) -> Option<String> {
    let side = *factions.get(listener)?;
    faction_index(side)?;
    let line_of = |team: usize| {
        voice
            .out_of_bounds
            .iter()
            .find(|(t, _)| usize::from(*t) == team + 1)
            .map(|(_, line)| line.clone())
    };
    match mission_factions.iter().position(|f| *f == side) {
        Some(team) => line_of(team),
        None => line_of(listener).map(|_| format!("{side}leaving")),
    }
}

/// The mission's setters as the rules read them, for the two armies that are
/// really in the battle (card ANNC2). A script's lines are keyed by its own
/// team numbers (Lua team n is `mission_factions[n - 1]`), but the shell can
/// seat any army in either slot, and the PS2's listener hears HIS army's
/// commander. So port team p hears the lines the script gives the team of
/// p's faction, about its own team (the script's listener = team entries)
/// and about the other one; an army the mission does not have (an era mix)
/// hears the same roles of the script team in its slot with its own
/// commander's prefix — a line the map's sound library has no take of is
/// silence (`VoiceQueue::step`). A team with no commander (the built-in
/// troopers) hears nothing. `tickets` are the teams' start counts (the
/// low-reinforcement threshold is a fraction of the listener team's count at
/// the call).
pub fn battle_lines(
    voice: &VoiceDef,
    mission_factions: [&str; 2],
    factions: [&str; 2],
    tickets: [u32; 2],
) -> BattleLines {
    let team = |t: u8| matches!(t, 1 | 2).then(|| usize::from(t) - 1);
    let mut out = BattleLines::default();
    for p in 0..2 {
        let side = factions[p];
        if faction_index(side).is_none() {
            continue;
        }
        // The script team whose lines this army hears, and whether they are
        // its own commander's as written.
        let (m, own) = match mission_factions.iter().position(|f| *f == side) {
            Some(m) => (m, true),
            None => (p, false),
        };
        let said = |name: &str| {
            if own {
                Some(name.to_owned())
            } else {
                line_for(name, side)
            }
        };
        // The script's subject team as a port team: its own, or the other.
        let subject = |t: usize| if t == m { p } else { 1 - p };
        for (l, t, name, flag) in &voice.bleeding {
            // A 4th argument of 0 fills the PS2's second table, which no retail
            // script does and nothing here plays.
            if let (Some(l), Some(t), true) = (team(*l), team(*t), *flag) {
                if l == m {
                    out.bleeding[p][subject(t)] = said(name);
                }
            }
        }
        for (l, t, name, x, flag) in &voice.low_reinforcements {
            if let (Some(l), Some(t)) = (team(*l), team(*t)) {
                if l == m {
                    out.set_low(p, subject(t), "", *x, *flag, tickets[p]);
                    out.low[p][subject(t)] = said(name);
                }
            }
        }
    }
    out
}

/// Takes of a side's `ai_<s>_inf_com_chatter_tac_yes` (re-squad-orders.md
/// section 3: three for each of the four armies).
pub const ANSWER_TAKES: u8 = 3;

/// Seconds a take of a side's answer cannot be drawn again: every take of
/// `ai_rep` / `ai_cis` is locked 1.0 s, of `ai_all` / `ai_imp` 2.0 s (RE
/// section 3, gate 1). From the RE's table: the bake does not carry a
/// segment's interval.
pub fn answer_lock_secs(faction: &str) -> f32 {
    match faction {
        "all" | "imp" => 2.0,
        _ => 1.0,
    }
}

/// A unit chatter line's 3-D level `dist` metres from the ears (the answer's
/// template: MinDistance 7 m, RollOff 3, mute at 150 m).
pub fn chatter_level(dist: f32) -> f32 {
    if dist >= 150.0 {
        0.0
    } else {
        7.0 / (7.0 + 3.0 * (dist - 7.0).max(0.0))
    }
}

/// The squad's answers to an order (card SQACK): every unit that takes one
/// asks for its side's "yes"; the request draws one of the line's takes, and
/// a take drawn inside its lock is silent — so an order is answered by one to
/// three lines.
#[derive(Debug, Clone, Default)]
pub struct Answers {
    seed: u32,
    /// (line, take, the second its lock ends).
    locks: Vec<(String, u8, f32)>,
}

impl Answers {
    /// One request for `name` at `now` seconds: the take that will be heard,
    /// or None when the draw fell on a locked one.
    pub fn draw(&mut self, name: &str, lock: f32, now: f32) -> Option<u8> {
        self.seed = self
            .seed
            .wrapping_mul(0x0019_660D)
            .wrapping_add(0x3C6E_F35F);
        let take = ((self.seed >> 8) % u32::from(ANSWER_TAKES)) as u8;
        self.locks.retain(|(_, _, until)| now < *until);
        if self.locks.iter().any(|(n, t, _)| n == name && *t == take) {
            return None;
        }
        self.locks.push((name.to_owned(), take, now + lock));
        Some(take)
    }
}

/// The unclamped bleed rule over the sim's posts, as BF2's script computes
/// it at a post event.
fn script_rates(sim: &Sim) -> [f32; 2] {
    [0, 1].map(|t| core::cp::bf2_bleed_rate(&sim.cps, t))
}

pub struct Commander {
    pub announcer: Announcer,
    pub queue: VoiceQueue,
    /// The two teams' faction keys (`rep`, `cis`, ...).
    factions: [String; 2],
    pub answers: Answers,
    /// Each sim post's line table, by `Sim::cps` index.
    posts: Vec<PostLines>,
    /// The posts' owners as of the last frame.
    owners: Vec<Option<u8>>,
    /// The lines the last [`Commander::frame`] asked for (a log's view; the
    /// queue already holds them).
    pub asked: Vec<Line>,
    /// BF2 maps: the conquest script's bleed rate, which is what the PS2's
    /// manager hears (card ANNC2). None on a BF1 map, whose engine rate is
    /// the sim's `bleed_per_sec`.
    pub script_bleed: Option<ScriptBleed>,
    /// The units' combat calls asked since the last
    /// [`Commander::gate_calls`] (card UCALL).
    pub calls_asked: Vec<CallAsk>,
    /// A sound bank gates the calls: until one does (a headless round, a
    /// sweep) each tick's asks replace the last's.
    gating: bool,
    /// Card UVO2: a BF2 battle's unit voice (`crate::vo`); None on BF1.
    pub vo: Option<crate::vo::Vo>,
    /// The player's line for leaving the boundary, set for the battle.
    leaving_line: Option<String>,
    /// The player's flown droid or flyer was outside at the last [`Commander::leaving`].
    was_leaving: bool,
    pub space_round: Option<std::sync::Arc<crate::space_assault::SpaceRound>>,
    speaker_asks: Vec<SpeakerAsk>,
    first_reinforcements: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SpeakerAsk {
    unit: u32,
    region: u32,
    center: glam::Vec3,
    reinforcements: f32,
    side: usize,
}

/// One unit's combat call (card UCALL): its class's property for it (None:
/// the class authors none) and where it is said.
#[derive(Debug, Clone, PartialEq)]
pub struct CallAsk {
    pub unit: u32,
    pub call: core::sim::Call,
    pub name: Option<String>,
    pub pos: glam::Vec3,
    /// Card UVO2: the speaker is the event's subject - the line is played
    /// 2-D, at the ears (B2 `FUN_002e6180` 272041-272079).
    pub own: bool,
}

impl Commander {
    /// `posts[i]` and `owners[i]` describe post i; `factions` are the two
    /// teams' mission faction keys; the listener is `listener`'s team.
    pub fn new(
        lines: BattleLines,
        factions: [&str; 2],
        listener: usize,
        posts: Vec<PostLines>,
        owners: Vec<Option<u8>>,
    ) -> Commander {
        Commander {
            announcer: Announcer::new(lines, listener, factions.map(faction_index)),
            queue: VoiceQueue::default(),
            factions: factions.map(str::to_owned),
            answers: Answers::default(),
            posts,
            owners,
            asked: Vec::new(),
            script_bleed: None,
            calls_asked: Vec::new(),
            gating: false,
            vo: None,
            leaving_line: None,
            was_leaving: false,
            space_round: None,
            speaker_asks: Vec::new(),
            first_reinforcements: None,
        }
    }

    /// The commander of a battle set up from `mission` over `layout`
    /// (`WalkSim::setup_conquest` has run: the sim's posts and tickets are in
    /// place). The listener is the local player's team.
    pub fn for_battle(mission: &MissionDef, layout: &MapLayout, walk: &WalkSim) -> Commander {
        let sim = &walk.sim;
        let tickets = [0, 1].map(|t| sim.battle.teams[t].tickets());
        let posts = sim
            .cps
            .iter()
            .map(|c| {
                layout
                    .cps
                    .iter()
                    .find(|d| d.name == c.def.name)
                    .map(|d| d.vo.clone())
                    .unwrap_or_default()
            })
            .collect();
        let factions = [0, 1].map(|t| walk.teams[t].faction.as_str());
        let mission_factions = [0, 1].map(|t| mission.teams[t].faction.as_str());
        let mut c = Commander::new(
            battle_lines(&mission.voice, mission_factions, factions, tickets),
            factions,
            usize::from(walk.player_team()),
            posts,
            sim.cps.iter().map(|c| c.owner).collect(),
        );
        c.space_round = walk.space_round.clone();
        if mission.source_game == "bf2" {
            c.script_bleed = Some(ScriptBleed::new(script_rates(sim)));
            let mut vo = crate::vo::Vo::new(0x5d0_u32 ^ sim.tick as u32);
            vo.space = sim.space_round();
            c.vo = Some(vo);
        }
        c.leaving_line = leaving_line(
            &mission.voice,
            mission_factions,
            factions,
            usize::from(walk.player_team()),
        );
        c
    }

    /// One frame of the battle's state: the post lines of every owner change
    /// since the last call, then the manager's update (the PS2's order — a
    /// capture that starts a bleed says the capture first, re-announcer.md §4
    /// row 11). The lines asked for are queued, and returned for the log.
    pub fn observe(
        &mut self,
        dt: f32,
        owners: &[Option<u8>],
        bleed: [f32; 2],
        tickets: [u32; 2],
        spawned: bool,
        over: bool,
    ) -> Vec<Line> {
        if spawned {
            self.announcer.player_spawned();
        }
        if over {
            self.announcer.battle_over();
        }
        let mut asked = Vec::new();
        for (i, &now) in owners.iter().enumerate() {
            let was = self.owners.get(i).copied().flatten();
            if was == now {
                continue;
            }
            let Some(post) = self.posts.get(i) else {
                continue;
            };
            let a = &self.announcer;
            asked.extend(was.and_then(|o| a.post(post, PostEvent::Lost, usize::from(o))));
            asked.extend(now.and_then(|o| a.post(post, PostEvent::Capture, usize::from(o))));
        }
        self.owners = owners.to_vec();
        asked.extend(self.announcer.update(dt, bleed, tickets));
        for l in &asked {
            self.queue.ask(l.clone());
        }
        asked
    }

    /// The squad's answers of the tick the sim just stepped (card SQACK,
    /// re-squad-orders.md section 3): each unit that took an order asks for
    /// its side's `ai_<s>_inf_com_chatter_tac_yes` at its own position, on
    /// the battle's voice — queued behind what is playing, a commander line
    /// ahead of it, a far unit's behind a near one's. BF1's names: a BF2
    /// map's banks hold none, so the queue finds no take there. Returns the
    /// lines queued. Once per stepped tick (`WalkSim::step_tick`).
    pub fn orders(&mut self, sim: &Sim, player: UnitHandle) -> Vec<Line> {
        let ears = sim.units.get(player.index as usize).map(|u| u.soldier.pos);
        let now = sim.tick as f32 * core::TICK_DT;
        let mut asked = Vec::new();
        for e in &sim.events {
            let core::sim::Event::OrderTaken { unit, .. } = e else {
                continue;
            };
            let Some(u) = sim.unit(*unit) else { continue };
            let side = self.factions[usize::from(u.team).min(1)].as_str();
            if faction_index(side).is_none() {
                continue;
            }
            let name = format!("ai_{side}_inf_com_chatter_tac_yes");
            let Some(take) = self.answers.draw(&name, answer_lock_secs(side), now) else {
                continue;
            };
            let dist = ears.map_or(0.0, |p| (u.soldier.pos - p).length());
            asked.push(Line {
                name,
                // PROVISIONAL: the chatter template's Bias / Priority were
                // not read; half the level keeps a commander line ahead.
                key: 0.5 * chatter_level(dist),
                mode_3d: true,
                take: Some(take),
                at: Some(u.soldier.pos),
                speaker: None,
            });
        }
        for l in &asked {
            self.queue.ask(l.clone());
        }
        asked
    }

    /// Say the player's leaving line when his flown droid starts a countdown: once an
    /// excursion, to him alone (`re-boundary.md` T1, T5). Returns it when asked. Once
    /// per stepped tick (`WalkSim::step_tick`).
    pub fn leaving(&mut self, sim: &Sim, player: UnitHandle) -> Option<Line> {
        self.leaving_now(sim.is_leaving(player))
    }

    fn leaving_now(&mut self, now: bool) -> Option<Line> {
        let started = now && !self.was_leaving;
        self.was_leaving = now;
        // Crossing the line again and again queues it once: the queue does not
        // read the line's `PlayInterval`, which keeps the PS2's from repeating.
        let name = self
            .leaving_line
            .as_deref()
            .filter(|n| started && !self.queue.waiting(n))?;
        let line = Line::commander(name);
        self.queue.ask(line.clone());
        Some(line)
    }

    fn space_calls(&mut self, sim: &Sim) {
        let Some(round) = self.space_round.clone() else {
            return;
        };
        let listener = self.announcer.listener();
        for e in &sim.events {
            match *e {
                core::sim::Event::SpaceObjectiveDown { objective, owner } => {
                    let Some(o) = round.objectives.get(usize::from(objective)) else {
                        continue;
                    };
                    let name = if usize::from(owner) == listener {
                        &o.def_vo
                    } else {
                        &o.att_vo
                    };
                    self.queue.ask(Line::commander(name));
                }
                core::sim::Event::SpaceLinkChanged { kind, index, up } => {
                    let Some(event) = round.scripted(kind, index, up) else {
                        continue;
                    };
                    for (name, team) in &event.vos {
                        if usize::from(*team) == listener {
                            self.queue.ask(Line::commander(name));
                        }
                    }
                }
                core::sim::Event::TeamDefeated { team } => {
                    let name = if usize::from(team) == listener {
                        round.lose_vo.get(listener)
                    } else {
                        round.win_vo.get(listener)
                    };
                    if let Some(name) = name.filter(|n| !n.is_empty()) {
                        self.queue.ask(Line::commander(name));
                    }
                }
                _ => {}
            }
        }
    }

    /// The combat calls of the tick the sim just stepped (card UCALL,
    /// `core::sim::Event::Call`): each asks its unit's class's property, at
    /// the unit. Held for [`Commander::gate_calls`]: the gates are the
    /// sound bank's. Once per stepped tick (`WalkSim::step_tick`).
    pub fn calls(&mut self, sim: &Sim, teams: &[crate::walk::TeamTemplate; 2]) {
        if !self.gating {
            self.calls_asked.clear();
            self.speaker_asks.clear();
        }
        // Hero manager lines use the same 2-D voice queue as the commander.
        for event in &sim.events {
            use core::sim::Event;
            let (unit, class, phase) = match *event {
                Event::HeroOffered { unit, class } => (unit, Some(class), 0),
                Event::Respawn { unit } => (unit, None, 1),
                Event::Kill { victim, .. } | Event::EnvDeath { unit: victim, .. } => {
                    (victim, None, 2)
                }
                _ => continue,
            };
            let Some(u) = sim
                .units
                .get(unit.index as usize)
                .filter(|u| u.gen == unit.gen)
            else {
                continue;
            };
            let class = class.unwrap_or(u.spec.class);
            let hero = &teams[usize::from(u.team.min(1))].class(class).cries.hero;
            // OURS (maintainer, 2026-11-05: Hero Assault is better with its
            // heroes' voices; `tat2g_eli` switches hero VO off): in Hero
            // Assault every hero says its own spawn and defeat line where it
            // stands, as the hero manager says the spawn line at the hero
            // (re-announcer.md 2.6); the player's own line is 2-D.
            if sim.deathmatch.is_some() {
                let (name, call) = match phase {
                    1 => (&hero.spawned, core::sim::Call::HeroSpawned),
                    2 => (&hero.defeated, core::sim::Call::HeroDefeated),
                    _ => continue,
                };
                if let Some(name) = name {
                    self.calls_asked.push(CallAsk {
                        unit: unit.index,
                        call,
                        name: Some(name.clone()),
                        pos: u.soldier.pos,
                        own: u.is_player,
                    });
                }
                continue;
            }
            if sim.hero_class(u.team) != Some(class) {
                continue;
            }
            let name = match phase {
                0 => &hero.offer,
                1 => &hero.spawned,
                _ => &hero.defeated,
            };
            if let Some(name) = name {
                self.queue.ask(Line::commander(name));
            }
        }
        self.space_calls(sim);
        for e in &sim.events {
            let core::sim::Event::Call { unit, call } = *e else {
                continue;
            };
            let Some(u) = sim.unit(unit) else { continue };
            // A BF2 tracker asks no AcquiredTargetSound: its take is the
            // VOSound event's (card UVO2, re-unit-chatter.md 3.7); BF2's
            // grenade warning also asks the class's FleeSound, which only a
            // few locals author - a class without one asks nothing.
            let class = teams[usize::from(u.team).min(1)].class(u.spec.class);
            let name = class.cries.call(call).map(str::to_owned);
            if self.vo.is_some() && (call == core::sim::Call::Acquired || name.is_none()) {
                continue;
            }
            self.calls_asked.push(CallAsk {
                unit: unit.index,
                call,
                name,
                pos: u.soldier.pos,
                own: false,
            });
        }
        if let Some(vo) = self.vo.as_mut() {
            let who: Vec<crate::vo::Who> = sim
                .units
                .iter()
                .map(|u| {
                    let class = teams[usize::from(u.team).min(1)].class(u.spec.class);
                    crate::vo::Who {
                        alive: u.alive,
                        team: u.team,
                        body: vo_body(sim, u, class.cries.vo.unit_type),
                        lines: &class.cries.vo,
                    }
                })
                .collect();
            let player = sim
                .players
                .first()
                .copied()
                .flatten()
                .filter(|h| sim.unit(*h).is_some())
                .map(|h| h.index as usize);
            for said in vo.tick(sim, &who, player) {
                let pos = sim.units[said.speaker].soldier.pos;
                self.calls_asked.push(CallAsk {
                    unit: said.speaker as u32,
                    call: core::sim::Call::Vo(said.event),
                    name: Some(said.sound),
                    pos,
                    own: said.own,
                });
            }
        }
        // Card UTALK, the low-health voice (re-unit-chatter.md 2.8; B1 the
        // soldier update `FUN_00205cc8` 116846-116861, both games): a
        // living unit at or under its class's MaxHealth x
        // `LowHealthThreshold` asks its `LowHealthSound` every tick its own
        // handle is idle (`Commander::gate_calls` skips one whose line is
        // still waiting or playing); dead, or above the threshold, its
        // waiting line is dropped. Every class that authors the sound sets
        // the threshold over 1 (the Gungans, Ewoks, the Jawa: 1.1), so they
        // say it for as long as they live.
        for (i, u) in sim.units.iter().enumerate() {
            let class = teams[usize::from(u.team).min(1)].class(u.spec.class);
            let Some(name) = class.cries.low_health.as_deref() else {
                continue;
            };
            let low = u.health.cur <= u.health.max * class.cries.low_threshold;
            if !u.alive || !low {
                self.queue.release(i as u32);
                continue;
            }
            self.calls_asked.push(CallAsk {
                unit: i as u32,
                call: core::sim::Call::LowHealth,
                name: Some(name.to_owned()),
                pos: u.soldier.pos,
                own: false,
            });
        }
    }

    pub fn cockpit(
        &mut self,
        sim: &Sim,
        player: UnitHandle,
        sounds: &[crate::vehicle_class::VehicleSoundClass],
    ) {
        let Some(u) = sim.unit(player) else { return };
        let Some(m) = u.mount.filter(|m| m.seat == 0) else {
            return;
        };
        let Some(v) = sim.vehicle(m.vehicle).filter(|v| v.alive) else {
            return;
        };
        let name = sounds
            .get(v.class as usize)
            .and_then(|s| s.cockpit_sound.clone());
        if name.is_some() {
            self.calls_asked.push(CallAsk {
                unit: player.index,
                call: core::sim::Call::Cockpit,
                name,
                pos: v.pos,
                own: true,
            });
        }
    }

    pub fn loudspeakers(
        &mut self,
        sim: &Sim,
        player: UnitHandle,
        regions: &[crate::ambience::SoundTrigger],
        teams: &[crate::walk::TeamTemplate; 2],
    ) {
        let first = *self
            .first_reinforcements
            .get_or_insert([0, 1].map(|t| sim.battle.teams[t].reinforcements));
        let Some(u) = sim.unit(player) else { return };
        let team = usize::from(u.team).min(1);
        let Some(side) = crate::ambience::trigger_side(&teams[team].faction) else {
            return;
        };
        let pos = u
            .mount
            .and_then(|m| sim.vehicle(m.vehicle))
            .map_or(u.soldier.pos, |v| v.pos);
        let reinforcements = if first[team] > 0.0 {
            sim.battle.teams[team].reinforcements / first[team]
        } else {
            1.0
        };
        for r in regions.iter().filter(|r| r.region.contains(pos)) {
            self.speaker_asks.push(SpeakerAsk {
                unit: player.index,
                region: r.name,
                center: r.region.center,
                reinforcements,
                side,
            });
        }
    }

    /// The calls asked since the last call, through the sound bank's gates
    /// (`SoundLib::ask_line`: the take's lock, the property's interval and
    /// chance, the level at `ears`): what passes joins the voice queue.
    /// Returns every ask with its outcome (a log's view).
    pub fn gate_calls(
        &mut self,
        sounds: &mut crate::sounds::SoundLib,
        ears: glam::Vec3,
    ) -> Vec<(CallAsk, Result<Line, crate::sounds::Refused>)> {
        self.gating = true;
        let mut asks = std::mem::take(&mut self.calls_asked);
        for s in std::mem::take(&mut self.speaker_asks) {
            if !self.queue.is_idle() || !self.queue.pending().is_empty() {
                continue;
            }
            let sound = sounds.trigger_sound(s.region, s.reinforcements, s.side);
            if let Some(sound) = sound {
                asks.push(CallAsk {
                    unit: s.unit,
                    call: core::sim::Call::Loudspeaker,
                    name: Some(format!("#{sound:08x}")),
                    pos: s.center,
                    own: false,
                });
            }
        }
        let mut out = Vec::with_capacity(asks.len());
        for a in asks {
            let own = a.call == core::sim::Call::LowHealth;
            // A low-health ask waits while the unit's own handle holds its
            // line (card UTALK): no gate is touched, no row is logged. Only
            // a voice-stream line is the queue's (the Gungans'); a
            // sample-bank one (a saber's hum, card HERO) is not played here.
            let stream = || a.name.as_deref().is_some_and(|n| sounds.streamed(n));
            if own && (self.queue.holds(a.unit) || !stream()) {
                continue;
            }
            // BF2 steps past a locked take (card UVO2); its subject's own
            // line is 2-D at the ears.
            let at = if a.own { ears } else { a.pos };
            let r = match &a.name {
                Some(n) => sounds.ask_line_with(n, at, ears, self.vo.is_some()),
                None => Err(crate::sounds::Refused::Unknown),
            };
            if let Ok(l) = &r {
                let mut l = l.clone();
                l.speaker = own.then_some(a.unit);
                l.mode_3d = !a.own;
                self.queue.ask(l);
            }
            out.push((a, r));
        }
        out
    }

    /// [`Commander::observe`] from the sim, then the voice queue's frame:
    /// `play` starts a line and returns its length in seconds (None = no
    /// take). Returns the line that started this frame.
    pub fn frame(
        &mut self,
        sim: &Sim,
        player: UnitHandle,
        dt: f32,
        mut play: impl FnMut(&Line) -> Option<f32>,
    ) -> Option<Line> {
        let me = sim.unit(player);
        if let Some(u) = me {
            self.announcer.set_listener(usize::from(u.team));
        }
        let owners: Vec<Option<u8>> = sim.cps.iter().map(|c| c.owner).collect();
        let bleed = match &mut self.script_bleed {
            Some(b) => {
                if owners != self.owners {
                    b.post_event(script_rates(sim));
                }
                b.step(
                    dt,
                    [0, 1].map(|t| sim.battle.teams[t].alive()),
                    [0, 1].map(|t| sim.battle.teams[t].tickets()),
                )
            }
            None => [0, 1].map(|t| sim.battle.teams[t].bleed_per_sec),
        };
        self.asked = self.observe(
            dt,
            &owners,
            bleed,
            [0, 1].map(|t| sim.battle.teams[t].tickets()),
            me.is_some(),
            sim.outcome.is_some(),
        );
        self.queue.step(dt, |l| play(l))
    }
}

/// What BF2's context reads of a unit (card UVO2): where it stands and
/// faces, what it is or sits in - a vehicle's kind, a soldier holding a
/// scoped rifle a sniper (PROVISIONAL [H]: the PS2's test is a class slot
/// not read, `FUN_002a8e88`) - and its class's `VOUnitType` (0 for a rider:
/// the vehicle class's type is not looked up).
fn vo_body(sim: &Sim, u: &core::unit::Unit, unit_type: u8) -> crate::vo::Body {
    use core::vehicle::VehicleKind;
    let seat = u.mount.and_then(|m| sim.vehicle(m.vehicle)).map(|v| v.kind);
    let kind = match seat {
        Some(VehicleKind::Flyer) => crate::vo::KIND_FLYER,
        Some(VehicleKind::Hover) => crate::vo::KIND_HOVER,
        Some(VehicleKind::Walker) => crate::vo::KIND_WALKER,
        Some(VehicleKind::Turret) => crate::vo::KIND_TURRET,
        None if core::ai::snipe::is_sniper(sim, u) => crate::vo::KIND_SNIPER,
        None => 0,
    };
    crate::vo::Body {
        pos: u.soldier.pos,
        yaw: u.soldier.yaw,
        kind,
        unit_type: if seat.is_some() { 0 } else { unit_type },
    }
}

#[cfg(test)]
mod leaving_tests {
    use super::*;

    fn voice() -> VoiceDef {
        VoiceDef {
            out_of_bounds: vec![(2, "Allleaving".into()), (1, "Impleaving".into())],
            ..Default::default()
        }
    }

    #[test]
    fn the_leaving_line_is_the_one_the_script_gives_the_listeners_army() {
        let v = voice();
        let line = |f, l| leaving_line(&v, ["imp", "all"], f, l);
        assert_eq!(line(["imp", "all"], 0).as_deref(), Some("Impleaving"));
        assert_eq!(line(["imp", "all"], 1).as_deref(), Some("Allleaving"));
        // The shell can seat the armies either way round.
        assert_eq!(line(["all", "imp"], 0).as_deref(), Some("Allleaving"));
        // An army the mission lacks takes its slot's, in its commander's name.
        assert_eq!(line(["rep", "all"], 0).as_deref(), Some("repleaving"));
    }

    #[test]
    fn the_line_is_asked_once_an_excursion() {
        let lines = BattleLines::default();
        let mut c = Commander::new(lines, ["imp", "all"], 0, Vec::new(), Vec::new());
        assert_eq!(c.leaving_now(true), None, "a script with no line");
        c.leaving_now(false);
        c.leaving_line = Some("Impleaving".into());
        let said = [true, true, true, false, false].map(|now| c.leaving_now(now).is_some());
        assert_eq!(said, [true, false, false, false, false]);
        c.queue.step(0.0, |_| Some(1.0));
        assert!(c.leaving_now(true).is_some(), "the next excursion");
        let line = c.leaving_now(false);
        assert_eq!(line, None);
    }

    #[test]
    fn crossing_the_edge_again_and_again_queues_the_line_once() {
        let lines = BattleLines::default();
        let mut c = Commander::new(lines, ["imp", "all"], 0, Vec::new(), Vec::new());
        c.leaving_line = Some("Impleaving".into());
        let said = [true, false, true, false, true].map(|now| c.leaving_now(now).is_some());
        assert_eq!(said, [true, false, false, false, false]);
        // It has started, so the next crossing may say it again.
        c.queue.step(0.0, |_| Some(1.0));
        c.leaving_now(false);
        assert!(c.leaving_now(true).is_some());
    }

    #[test]
    fn a_script_with_no_line_says_nothing() {
        let v = VoiceDef::default();
        assert_eq!(leaving_line(&v, ["imp", "all"], ["imp", "all"], 0), None);
        assert_eq!(leaving_line(&v, ["imp", "all"], ["rep", "all"], 0), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A script's four bleeding and four low-reinforcement calls: team 1 is
    /// `a`, team 2 is `b`.
    fn voice(a: &str, b: &str) -> VoiceDef {
        let mut v = VoiceDef::default();
        for (l, side) in [(1u8, a), (2, b)] {
            for t in [1u8, 2] {
                let (bl, low) = if l == t {
                    ("us_overwhelmed", "defeat_im")
                } else {
                    ("enemy_losing", "victory_im")
                };
                v.bleeding
                    .push((l, t, format!("{side}_off_com_report_{bl}"), true));
                v.low_reinforcements
                    .push((l, t, format!("{side}_off_{low}"), 0.1, true));
            }
        }
        v
    }

    fn name(l: &Option<String>) -> &str {
        l.as_deref().unwrap_or("-")
    }

    /// Card ANNC2, acceptance 1: the lines follow the listener's FACTION.
    #[test]
    fn factions() {
        let v = voice("cis", "rep");
        // The mission as written: the script's team numbers are the port's.
        let same = battle_lines(&v, ["cis", "rep"], ["cis", "rep"], [150, 200]);
        assert_eq!(
            name(&same.bleeding[0][0]),
            "cis_off_com_report_us_overwhelmed"
        );
        assert_eq!(
            name(&same.bleeding[0][1]),
            "cis_off_com_report_enemy_losing"
        );
        assert_eq!(
            name(&same.bleeding[1][1]),
            "rep_off_com_report_us_overwhelmed"
        );
        assert_eq!(name(&same.low[1][0]), "rep_off_victory_im");
        assert_eq!(same.low_threshold, [15, 20]);
        // The other side: the Republic sits in team 0's slot.
        let swap = battle_lines(&v, ["cis", "rep"], ["rep", "cis"], [150, 200]);
        assert_eq!(
            name(&swap.bleeding[0][0]),
            "rep_off_com_report_us_overwhelmed"
        );
        assert_eq!(
            name(&swap.bleeding[0][1]),
            "rep_off_com_report_enemy_losing"
        );
        assert_eq!(name(&swap.low[0][0]), "rep_off_defeat_im");
        assert_eq!(name(&swap.low[0][1]), "rep_off_victory_im");
        assert_eq!(
            name(&swap.bleeding[1][1]),
            "cis_off_com_report_us_overwhelmed"
        );
        assert_eq!(
            name(&swap.bleeding[1][0]),
            "cis_off_com_report_enemy_losing"
        );
        assert_eq!(name(&swap.low[1][1]), "cis_off_defeat_im");
        assert_eq!(name(&swap.low[1][0]), "cis_off_victory_im");
        // The threshold is a fraction of the listener team's own tickets.
        assert_eq!(swap.low_threshold, [15, 20]);
        // An era mix: an army the mission does not have hears its own
        // commander say the same roles; the army it does have, its own.
        let mix = battle_lines(&v, ["cis", "rep"], ["imp", "rep"], [150, 200]);
        assert_eq!(
            name(&mix.bleeding[0][0]),
            "imp_off_com_report_us_overwhelmed"
        );
        assert_eq!(name(&mix.bleeding[0][1]), "imp_off_com_report_enemy_losing");
        assert_eq!(name(&mix.low[0][0]), "imp_off_defeat_im");
        assert_eq!(name(&mix.low[0][1]), "imp_off_victory_im");
        assert_eq!(
            name(&mix.bleeding[1][1]),
            "rep_off_com_report_us_overwhelmed"
        );
        assert_eq!(name(&mix.bleeding[1][0]), "rep_off_com_report_enemy_losing");
        let gcw = battle_lines(&v, ["cis", "rep"], ["all", "imp"], [150, 200]);
        for l in 0..2 {
            for t in 0..2 {
                let side = ["all", "imp"][l];
                assert!(name(&gcw.bleeding[l][t]).starts_with(side));
                assert!(name(&gcw.low[l][t]).starts_with(side));
            }
        }
        // No commander (the built-in troopers): nothing.
        let none = battle_lines(&v, ["cis", "rep"], ["team", "team"], [150, 200]);
        assert_eq!(none.bleeding, BattleLines::default().bleeding);
        assert_eq!(none.low, BattleLines::default().low);
        // A line with no faction prefix cannot be another army's.
        assert_eq!(line_for("Allleaving", "rep"), None);
        assert_eq!(
            line_for("all_off_victory_im", "rep").as_deref(),
            Some("rep_off_victory_im")
        );
    }

    /// The whole path: through the manager, a Republic player on the script's
    /// CIS slot hears the Republic's commander for both teams' events.
    #[test]
    fn a_bf2_take_asks_the_vo_line_for_the_targets_unit_type() {
        use core::sim::{Call, Event};
        use core::unit::{Controller, UnitSpec};
        let mut w = crate::walk::WalkSim::new(1, glam::Vec3::ZERO, None);
        let me = w.player.index as usize;
        let mine = w.sim.units[me].team;
        let lines = |v: &[&str], unit_type: u8| {
            std::sync::Arc::new(crate::vo::VoLines {
                entries: v.iter().filter_map(|e| crate::vo::parse_entry(e)).collect(),
                unit_type,
                radius: crate::vo::VO_RADIUS,
            })
        };
        for (t, team) in w.teams.iter_mut().enumerate() {
            for c in &mut team.classes {
                c.cries.acquired = Some("bf1_acquired".into());
                c.cries.vo = if t == usize::from(mine) {
                    lines(
                        &["plain AcquiredTarget", "t7 AcquiredTarget +VOUnitNum7"],
                        3,
                    )
                } else {
                    lines(&[], 7)
                };
            }
        }
        let spawn = |w: &mut crate::walk::WalkSim, team: u8, x: f32| {
            let spec = UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(team, glam::Vec3::new(x, 0.0, 0.0))
            };
            w.sim.spawn(&spec).expect("spawn")
        };
        let ally = spawn(&mut w, mine, 10.0);
        let enemy = spawn(&mut w, 1 - mine, 30.0);
        w.sim.units[ally.index as usize].ai.target = Some(enemy.into());
        let mut v = Commander::new(
            BattleLines::default(),
            ["rep", "cis"],
            0,
            Vec::new(),
            Vec::new(),
        );
        v.vo = Some(crate::vo::Vo::new(9));
        let mut said = Vec::new();
        for _ in 0..20 {
            w.sim.events = vec![Event::Call {
                unit: ally,
                call: Call::Acquired,
            }];
            v.calls(&w.sim, &w.teams);
            said.extend(std::mem::take(&mut v.calls_asked));
        }
        // Kept 71 in 101; never BF1's AcquiredTargetSound.
        assert!(said.len() > 5 && said.len() < 20, "{said:?}");
        for a in &said {
            assert_eq!(
                (a.unit, a.call, a.name.as_deref(), a.own),
                (
                    ally.index,
                    Call::Vo(crate::vo::ACQUIRED_TARGET),
                    Some("t7"),
                    false
                )
            );
        }
        // The ally 60 m from the player: silent.
        w.sim.units[ally.index as usize].soldier.pos.x = 60.0;
        w.sim.events = vec![Event::Call {
            unit: ally,
            call: Call::Acquired,
        }];
        for _ in 0..10 {
            v.calls(&w.sim, &w.teams);
        }
        assert!(v.calls_asked.is_empty(), "{:?}", v.calls_asked);
    }

    #[test]
    fn the_other_side_hears_its_own_commander() {
        let lines = battle_lines(
            &voice("cis", "rep"),
            ["cis", "rep"],
            ["rep", "cis"],
            [150; 2],
        );
        let mut c = Commander::new(lines, ["rep", "cis"], 0, Vec::new(), Vec::new());
        let third = 1.0 / 3.0;
        let dt = 1.0 / 30.0;
        let mut said = Vec::new();
        said.extend(c.observe(dt, &[], [third, 0.0], [150; 2], true, false));
        said.extend(c.observe(dt, &[], [0.0, 0.0], [150; 2], true, false));
        said.extend(c.observe(dt, &[], [0.0, third], [150; 2], true, false));
        said.extend(c.observe(dt, &[], [0.0, third], [150, 15], true, false));
        let names: Vec<&str> = said.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "rep_off_com_report_us_overwhelmed",
                "rep_off_com_report_enemy_losing",
                "rep_off_victory_im"
            ]
        );
    }
}
