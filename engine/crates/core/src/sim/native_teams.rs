//! Conquest's native members have their own spawn waves and unlimited tickets.
//! Independent hostile natives can take posts; only the armies pay tickets and decide the outcome.

use super::{Sim, SpawnWaves};
use crate::fmath::FloatExt;
use crate::unit::{Controller, UnitSpec};
use glam::Vec3;
use std::collections::VecDeque;

/// A command post's AllyCount contribution and separate AllyPath nodes.
/// Its owner's SpawnPath lives on `CommandPost::def`, even without AllyPath.
#[derive(Clone, Debug)]
pub struct AllySpawn {
    pub post: usize,
    pub count: u32,
    pub nodes: Vec<(Vec3, f32)>,
}

/// An owned, non-capturable native spawn post, outside the conquest objectives.
#[derive(Clone, Debug)]
pub struct NativeSpawnPost {
    pub count: u32,
    pub nodes: Vec<(Vec3, f32)>,
}

#[derive(Clone, Debug)]
pub(super) struct NativeTeam {
    team: u8,
    pending: VecDeque<UnitSpec>,
    spawn_posts: Vec<NativeSpawnPost>,
    waves: SpawnWaves,
}

impl Sim {
    /// Set an authored relation in one direction. Indices use sim numbering.
    pub fn set_team_relation(&mut self, from: u8, to: u8, friendly: bool) {
        assert!(from >= 2 || to >= 2);
        self.team_relations.insert((from, to), friendly);
    }

    /// A same-team unit and any explicitly friendly team are allies.
    pub fn allied(&self, from: u8, to: u8) -> bool {
        from == to
            || ((from >= 2 || to >= 2) && self.team_relations.get(&(from, to)) == Some(&true))
    }

    pub fn hostile(&self, from: u8, to: u8) -> bool {
        !self.allied(from, to)
    }

    /// Queue the authored native member pool. Member templates stay tied to
    /// their slot through death, independent of the main armies' class draws.
    pub fn add_native_team(&mut self, team: u8, members: Vec<UnitSpec>) {
        self.add_native_team_with_posts(team, members, Vec::new());
    }

    /// Keep invisible native spawn posts separate from capturable objectives.
    /// Their authored capacity and paths still participate in native waves.
    pub fn add_native_team_with_posts(
        &mut self,
        team: u8,
        members: Vec<UnitSpec>,
        spawn_posts: Vec<NativeSpawnPost>,
    ) {
        assert!(team >= 2 && self.native_team_index(team).is_none());
        assert!(members.iter().all(|spec| {
            spec.team == team && spec.controller == Controller::Ai && !spec.is_player()
        }));
        self.native_teams.push(NativeTeam {
            team,
            pending: members.into(),
            spawn_posts,
            waves: SpawnWaves {
                timer: 0.0,
                wave: 0,
                next_slot: 0,
                ..self.waves[0]
            },
        });
    }

    pub(super) fn native_team_index(&self, team: u8) -> Option<usize> {
        self.native_teams
            .iter()
            .position(|entry| entry.team == team)
    }

    pub(super) fn assign_unit_wave(&mut self, team: u8) -> Option<(u32, u32)> {
        if let Some(i) = self.native_team_index(team) {
            Some(self.native_teams[i].waves.assign())
        } else {
            self.waves
                .get_mut(usize::from(team))
                .map(SpawnWaves::assign)
        }
    }

    /// Install or replace ally spawn inputs before populating the roster.
    pub fn set_ally_spawns(&mut self, posts: Vec<AllySpawn>) {
        self.ally_spawns = posts;
    }

    // CanTeamSpawnAt: own SpawnPath, otherwise the owner's directed friendship
    // and a distinct AllyPath (BF1 FUN_0029de80:227148-227170;
    // BF2 FUN_00356880:350959-350978; re-cp-capture §6).
    fn native_spawn_nodes(&self, team: u8, post: usize) -> Option<&[(Vec3, f32)]> {
        let Some(cp) = self.cps.get(post) else {
            let entry = self.native_team_index(team)?;
            let nodes = &self.native_teams[entry]
                .spawn_posts
                .get(post - self.cps.len())?
                .nodes;
            return (!nodes.is_empty()).then_some(nodes.as_slice());
        };
        let owner = cp.owner?;
        let nodes = if owner == team {
            &cp.def.spawn_nodes
        } else if self.allied(owner, team) {
            &self.ally_spawns.iter().find(|p| p.post == post)?.nodes
        } else {
            return None;
        };
        (!nodes.is_empty()).then_some(nodes.as_slice())
    }

    pub(super) fn admit_native(&self, team: u8) -> bool {
        // Capacity is summed independently of usable paths, then effectively
        // clamped by the team's member pool. BF1 FUN_002ac378:237768-237783;
        // BF2 FUN_0036c750:366105-366110,366161-366164.
        let owned = self.native_team_index(team).map_or(0, |entry| {
            self.native_teams[entry]
                .spawn_posts
                .iter()
                .map(|post| post.count)
                .fold(0, u32::saturating_add)
        });
        let limit = self
            .ally_spawns
            .iter()
            .filter(|post| {
                self.cps
                    .get(post.post)
                    .and_then(|cp| cp.owner)
                    .is_some_and(|owner| self.allied(owner, team))
            })
            .map(|post| post.count)
            .fold(owned, u32::saturating_add);
        let alive = self.alive_units().filter(|(_, u)| u.team == team).count();
        alive < limit as usize
    }

    fn native_spawn_point(&mut self, team: u8) -> Option<(Vec3, f32)> {
        if !self.admit_native(team) {
            return None;
        }
        let owned = self
            .native_team_index(team)
            .map_or(0, |entry| self.native_teams[entry].spawn_posts.len());
        let eligible = (0..self.cps.len() + owned)
            .filter(|&post| self.native_spawn_nodes(team, post).is_some())
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            return None;
        }
        // PROVISIONAL, replace: measure the native-team post/node selection
        // when multiple owned or allied posts have spawn capacity.
        let post = eligible[self.lcg.index(eligible.len() as u32) as usize];
        let count = self.native_spawn_nodes(team, post)?.len();
        let node = self.lcg.index(count as u32) as usize;
        Some(self.native_spawn_nodes(team, post)?[node])
    }

    /// Open all eligible native slots; posts changing hands can release
    /// capacity for a member that could not spawn on the first tick.
    pub fn populate_native_teams(&mut self) {
        for i in 0..self.native_teams.len() {
            let team = self.native_teams[i].team;
            while !self.native_teams[i].pending.is_empty() {
                let Some((pos, yaw)) = self.native_spawn_point(team) else {
                    break;
                };
                let mut spec = self.native_teams[i].pending.pop_front().unwrap();
                spec.pos = pos;
                spec.yaw = yaw;
                if self.spawn_roster_slot(&spec).is_none() {
                    self.native_teams[i].pending.push_front(spec);
                    break;
                }
            }
        }
    }

    pub(super) fn tick_native_waves(&mut self, dt: f32) {
        for entry in &mut self.native_teams {
            entry.waves.tick(dt);
        }
    }

    pub(super) fn respawn_native(&mut self, i: usize) {
        let old = &self.units[i];
        let Some(entry) = self.native_team_index(old.team) else {
            return;
        };
        if old
            .spawn_slot
            .is_some_and(|slot| !self.native_teams[entry].waves.ready(slot))
        {
            return;
        }
        let mut spec = old.spec.clone();
        let Some((pos, yaw)) = self.native_spawn_point(spec.team) else {
            return;
        };
        spec.pos = pos;
        spec.yaw = yaw;
        self.replace_respawned_unit(i, spec, None);
    }

    pub(super) fn hash_native_teams(&self, eat: &mut impl FnMut(&[u8])) {
        if self.native_teams.is_empty() && self.team_relations.is_empty() {
            return;
        }
        eat(&(self.native_teams.len() as u32).to_le_bytes());
        for entry in &self.native_teams {
            eat(&[entry.team]);
            eat(&(entry.pending.len() as u32).to_le_bytes());
            eat(&entry.waves.timer.hash_bits().to_le_bytes());
            eat(&entry.waves.wave.to_le_bytes());
            eat(&entry.waves.next_slot.to_le_bytes());
            if !entry.spawn_posts.is_empty() {
                eat(b"native-spawn-posts");
                eat(&(entry.spawn_posts.len() as u32).to_le_bytes());
                for post in &entry.spawn_posts {
                    eat(&post.count.to_le_bytes());
                    eat(&(post.nodes.len() as u32).to_le_bytes());
                    for (pos, yaw) in &post.nodes {
                        for value in [pos.x, pos.y, pos.z, *yaw] {
                            eat(&value.hash_bits().to_le_bytes());
                        }
                    }
                }
            }
        }
        eat(&(self.team_relations.len() as u32).to_le_bytes());
        for (&(from, to), &friendly) in &self.team_relations {
            eat(&[from, to, u8::from(friendly)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cp::CpDef;
    use crate::region::Region;

    fn post(sim: &mut Sim, owner: u8, x: f32) -> AllySpawn {
        let pos = Vec3::new(x, 0.0, 0.0);
        let post = sim.add_cp(
            CpDef {
                name: format!("cp{owner}"),
                pos,
                capture: Region::sphere(pos, 5.0),
                control: None,
                value_bleed: 10,
                value_atk: [10; 2],
                value_def: [10; 2],
                spawn_nodes: Vec::new(),
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            Some(owner),
        );
        AllySpawn {
            post,
            count: 1,
            nodes: vec![(pos, 1.25)],
        }
    }

    #[test]
    fn native_only_posts_use_capacity_and_respawn_without_changing_objectives() {
        let mut sim = Sim::empty(4);
        sim.heightfield = Some(crate::heightfield::Heightfield {
            size: 2,
            unit: 50.0,
            offset_x: 25.0,
            offset_z: 25.0,
            heights: vec![0.0; 4],
        });
        post(&mut sim, 0, -10.0);
        post(&mut sim, 1, 10.0);
        let mut native = UnitSpec::trooper(2, Vec3::ZERO);
        native.controller = Controller::Ai;
        sim.add_native_team_with_posts(
            2,
            vec![native],
            vec![NativeSpawnPost {
                count: 1,
                nodes: vec![(Vec3::new(0.0, 0.0, 10.0), 1.25)],
            }],
        );
        sim.populate_native_teams();
        assert_eq!(sim.cps.len(), 2);
        assert_eq!(sim.alive_units().count(), 1);
        let h = sim.handle(0);
        assert_eq!(sim.unit(h).unwrap().spec.pos, Vec3::new(0.0, 0.0, 10.0));
        assert!(!sim.admit_native(2), "authored capacity is full");
        let tickets = [0, 1].map(|t| sim.battle.teams[t].tickets());
        let waves = sim.waves.clone();
        sim.kill_environment(h, crate::sim::DeathCause::DeathRegion);
        assert_eq!([0, 1].map(|t| sim.battle.teams[t].tickets()), tickets);
        assert_eq!(sim.waves, waves);
        let wave = &mut sim.native_teams[0].waves;
        wave.wave = 1;
        wave.timer = wave.interval;
        sim.tick = sim.units[0].respawn_at.unwrap();
        sim.step(&[]);
        let reborn = sim.unit(sim.handle(0)).unwrap();
        assert!(reborn.alive && reborn.gen != h.gen);
        assert_eq!(reborn.spec.pos, Vec3::new(0.0, 0.0, 10.0));
        assert_eq!(reborn.spec.yaw, 1.25);
        assert_eq!([0, 1].map(|t| sim.battle.teams[t].tickets()), tickets);
        for cp in &mut sim.cps {
            cp.owner = Some(0);
        }
        let mut domination = None;
        assert_eq!(
            crate::cp::tick_domination(
                &sim.cps,
                &mut domination,
                crate::battle::BF2_DOMINATION_DEFEAT_SECS
                    + crate::battle::BF2_DOMINATION_DITTY_SECS,
                2
            ),
            Some(1)
        );
    }

    #[test]
    fn native_only_posts_require_capacity_and_nodes_and_hash_their_inputs() {
        let mut native = UnitSpec::trooper(2, Vec3::ZERO);
        native.controller = Controller::Ai;
        for (count, nodes) in [(0, vec![(Vec3::ZERO, 0.0)]), (1, Vec::new())] {
            let mut sim = Sim::empty(4);
            sim.add_native_team_with_posts(
                2,
                vec![native.clone()],
                vec![NativeSpawnPost { count, nodes }],
            );
            sim.populate_native_teams();
            assert_eq!(sim.alive_units().count(), 0);
        }
        let with = |posts| {
            let mut sim = Sim::empty(4);
            sim.add_native_team_with_posts(2, Vec::new(), posts);
            sim.hash()
        };
        let mut ordinary = Sim::empty(4);
        ordinary.add_native_team(2, Vec::new());
        assert_eq!(ordinary.hash(), with(Vec::new()));
        assert_ne!(
            with(vec![NativeSpawnPost {
                count: 1,
                nodes: vec![(Vec3::ZERO, 0.0)]
            }]),
            with(vec![NativeSpawnPost {
                count: 1,
                nodes: vec![(Vec3::X, 0.0)]
            }])
        );
    }

    #[test]
    fn native_members_use_friendly_ally_nodes_without_spending_tickets() {
        let mut sim = Sim::empty(4);
        sim.heightfield = Some(crate::heightfield::Heightfield {
            size: 2,
            unit: 50.0,
            offset_x: 25.0,
            offset_z: 25.0,
            heights: vec![0.0; 4],
        });
        let a = post(&mut sim, 0, -10.0);
        let b = post(&mut sim, 1, 10.0);
        sim.set_ally_spawns(vec![a, b]);
        let mut native = UnitSpec::trooper(2, Vec3::ZERO);
        native.controller = Controller::Ai;
        sim.add_native_team(2, vec![native; 2]);
        sim.set_team_relation(2, 0, true);
        sim.set_team_relation(0, 2, true);
        sim.set_team_relation(2, 1, false);
        sim.populate_native_teams();
        assert_eq!(sim.alive_units().count(), 1);
        let h = sim.handle(0);
        let unit = sim.unit(h).unwrap();
        assert_eq!(unit.soldier.pos.x, -10.0);
        assert_eq!(unit.soldier.yaw, 1.25);
        assert!(!sim.counts_in_cp(0, 0));

        let tickets = [0, 1].map(|team| sim.battle.teams[team].tickets());
        let waves = sim.waves.clone();
        sim.kill_environment(h, crate::sim::DeathCause::DeathRegion);
        assert_eq!([0, 1].map(|team| sim.battle.teams[team].tickets()), tickets);
        assert_eq!(sim.waves, waves);
        let wave = &mut sim.native_teams[0].waves;
        wave.wave = 1;
        wave.timer = wave.interval;
        sim.tick = sim.units[0].respawn_at.unwrap();
        sim.cps[0].owner = Some(1);
        sim.step(&[]);
        assert!(!sim.units[0].alive);

        sim.cps[1].owner = Some(0);
        sim.step(&[]);
        assert!(sim.unit(h).is_none());
        let reborn = sim.unit(sim.handle(0)).unwrap();
        assert_eq!(reborn.spec.pos.x, 10.0);
        assert_eq!(reborn.spec.yaw, 1.25);
        assert_eq!(reborn.deaths, 1);
        assert_eq!(reborn.team, 2);
        assert_eq!(sim.alive_units().count(), 1);
    }

    #[test]
    fn killing_an_allied_native_spends_no_army_ticket() {
        let mut sim = Sim::empty(4);
        sim.heightfield = Some(crate::heightfield::Heightfield {
            size: 2,
            unit: 50.0,
            offset_x: 25.0,
            offset_z: 25.0,
            heights: vec![0.0; 4],
        });
        let mut a = post(&mut sim, 0, -10.0);
        a.count = 2;
        let b = post(&mut sim, 1, 10.0);
        sim.set_ally_spawns(vec![a, b]);
        let mut soldier = UnitSpec::trooper(1, Vec3::Z * 20.0);
        soldier.controller = Controller::Ai;
        let enemy = sim.spawn(&soldier).unwrap();
        soldier.team = 0;
        soldier.pos = Vec3::Z * -20.0;
        let ally = sim.spawn(&soldier).unwrap();
        let mut native = UnitSpec::trooper(2, Vec3::ZERO);
        native.controller = Controller::Ai;
        sim.add_native_team(2, vec![native; 2]);
        sim.set_team_relation(2, 0, true);
        sim.set_team_relation(0, 2, true);
        sim.set_team_relation(2, 1, false);
        sim.populate_native_teams();
        let natives: Vec<_> = sim
            .alive_units()
            .filter(|(_, u)| u.team == 2)
            .map(|(h, _)| h)
            .collect();
        assert_eq!(natives.len(), 2);

        let tickets = [0, 1].map(|team| sim.battle.teams[team].tickets());
        let alive = [0, 1].map(|team| sim.battle.teams[team].alive());
        let waves = sim.waves.clone();
        // A credited kill by the enemy army, then a team kill by the ally.
        for (native, killer) in natives.into_iter().zip([enemy, ally]) {
            sim.events.clear();
            sim.unit_killed(native, Some(killer));
            assert!(sim.events.contains(&crate::sim::Event::Kill {
                shooter: killer,
                victim: native,
            }));
            assert_eq!([0, 1].map(|team| sim.battle.teams[team].tickets()), tickets);
            assert_eq!([0, 1].map(|team| sim.battle.teams[team].alive()), alive);
            assert_eq!(sim.waves, waves);
        }
    }

    #[test]
    fn native_owned_posts_cause_no_army_bleed_through_step() {
        let mut sim = Sim::empty(4);
        for (i, owner) in [0, 0, 1, 1, 2, 2].into_iter().enumerate() {
            post(&mut sim, owner, i as f32 * 100.0);
        }
        sim.add_native_team(2, Vec::new());
        for _ in 0..300 {
            sim.step(&[]);
        }
        for team in &sim.battle.teams {
            assert_eq!(team.bleed_per_sec, 0.0);
            assert_eq!(team.tickets(), 250);
        }
        assert_eq!(sim.outcome, None);
    }

    #[test]
    fn authored_relations_are_directed_and_last_write_wins() {
        let mut sim = Sim::empty(9);
        sim.add_native_team(2, Vec::new());
        assert!(sim.hostile(0, 1));
        assert!(sim.hostile(0, 2));
        assert!(!sim.hostile(2, 2));
        sim.set_team_relation(2, 0, true);
        assert!(sim.allied(2, 0));
        assert!(!sim.allied(0, 2));
        assert!(!sim.hostile(2, 0));
        assert!(sim.hostile(0, 2));
        sim.set_team_relation(2, 0, false);
        assert!(!sim.allied(2, 0));
        assert!(sim.hostile(2, 0));
    }

    fn cp(sim: &mut Sim, owner: Option<u8>, nodes: Vec<(Vec3, f32)>) -> usize {
        sim.add_cp(
            CpDef {
                name: "native".into(),
                pos: Vec3::ZERO,
                capture: Region::sphere(Vec3::ZERO, 5.0),
                control: None,
                value_bleed: 10,
                value_atk: [10; 2],
                value_def: [10; 2],
                spawn_nodes: nodes,
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            owner,
        )
    }
    fn members(sim: &mut Sim, count: usize) {
        let mut spec = UnitSpec::trooper(2, Vec3::ZERO);
        spec.controller = Controller::Ai;
        sim.add_native_team(2, vec![spec; count]);
    }

    #[test]
    fn native_owned_post_uses_own_nodes_and_member_pool_not_ally_nodes() {
        let mut sim = Sim::empty(42);
        sim.heightfield = Some(crate::heightfield::Heightfield {
            size: 2,
            unit: 50.0,
            offset_x: 25.0,
            offset_z: 25.0,
            heights: vec![0.0; 4],
        });
        let own = (Vec3::new(2.0, 0.0, 3.0), 0.75);
        let post = cp(&mut sim, Some(2), vec![own]);
        sim.set_ally_spawns(vec![AllySpawn {
            post,
            count: 65536,
            nodes: vec![(Vec3::X * 40.0, 2.0)],
        }]);
        members(&mut sim, 14);
        let tickets = sim.battle.teams.map(|t| t.tickets());
        sim.populate_native_teams();
        assert_eq!(sim.alive_units().count(), 14);
        assert!(sim
            .alive_units()
            .all(|(_, u)| (u.spec.pos, u.spec.yaw) == own));
        assert_eq!(sim.battle.teams.map(|t| t.tickets()), tickets);
        assert!(sim.counts_in_cp(0, post));
        assert_eq!(sim.cp_inside(post), [0, 0, 14, 0, 0, 0, 0, 0]);
        sim.cps[post].neutral = 1.0;
        // A sparse caller must not index the two-army array with native owner 2.
        sim.cps[post].update(crate::TICK_DT, &[0, 0]);
        assert_eq!(sim.cps[post].neutral, 1.0);
        let inside = sim.cp_inside(post);
        sim.cps[post].update(crate::TICK_DT, &inside);
        assert!(sim.cps[post].neutral < 1.0);
    }

    #[test]
    fn native_spawn_checks_the_post_owners_directed_friendship() {
        let mut sim = Sim::empty(42);
        let post = cp(&mut sim, Some(0), vec![(Vec3::X * 30.0, 0.0)]);
        let ally = (Vec3::X * 2.0, 1.25);
        sim.set_ally_spawns(vec![AllySpawn {
            post,
            count: 1,
            nodes: vec![ally],
        }]);
        members(&mut sim, 1);
        sim.set_team_relation(2, 0, true);
        sim.populate_native_teams();
        assert_eq!(
            sim.alive_units().count(),
            0,
            "reverse-only friendship grants no capacity/path"
        );
        sim.set_team_relation(0, 2, true);
        sim.set_team_relation(2, 0, false);
        sim.populate_native_teams();
        assert_eq!(sim.alive_units().count(), 1);
        let unit = sim.units.first().unwrap();
        assert_eq!((unit.spec.pos, unit.spec.yaw), ally);
    }

    #[test]
    fn native_budget_is_separate_from_paths_and_neutral_posts_grant_neither() {
        let mut sim = Sim::empty(42);
        let own = (Vec3::X * 2.0, 0.25);
        let post = cp(&mut sim, Some(2), vec![own]);
        let capacity = cp(&mut sim, Some(0), Vec::new());
        sim.set_team_relation(0, 2, true);
        sim.set_ally_spawns(vec![
            AllySpawn {
                post,
                count: 0,
                nodes: Vec::new(),
            },
            AllySpawn {
                post: capacity,
                count: 1,
                nodes: Vec::new(),
            },
        ]);
        members(&mut sim, 2);
        sim.cps[post].def.spawn_nodes.clear();
        sim.populate_native_teams();
        assert_eq!(
            sim.alive_units().count(),
            0,
            "capacity with no usable path must hold safely"
        );
        sim.cps[post].def.spawn_nodes.push(own);
        sim.cps[capacity].owner = None;
        sim.populate_native_teams();
        assert_eq!(
            sim.alive_units().count(),
            0,
            "a neutral post contributes no budget"
        );
        sim.cps[capacity].owner = Some(0);
        sim.populate_native_teams();
        assert_eq!(
            sim.alive_units().count(),
            1,
            "capacity, not path count or roster minimum"
        );
        assert_eq!(sim.units[0].spec.pos, own.0);
    }
}
