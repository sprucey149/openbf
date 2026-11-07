use crate::classdb::{fnv1a_lowercase, ClassDb};
use crate::mission::SpaceAssaultDef;
use core::sim::{
    CriticalSystem, FrigateTurrets, LinkedTurretSet, ShieldGroup, Sim, SpaceAssault, Station,
};
use manifest::ManifestInstance;

fn instance_of(instances: &[ManifestInstance], name: &str) -> Option<u32> {
    instances
        .iter()
        .position(|i| i.name.eq_ignore_ascii_case(name))
        .map(|i| i as u32)
}

fn team_of(inst: &ManifestInstance) -> Option<u8> {
    inst.props
        .iter()
        .rev()
        .find(|(k, _)| *k == fnv1a_lowercase("Team"))
        .and_then(|(_, v)| v.trim().parse::<u8>().ok())
        .filter(|t| (1..=2).contains(t))
        .map(|t| t - 1)
}

fn add_part(
    sim: &mut Sim,
    instances: &[ManifestInstance],
    db: &ClassDb,
    instance: u32,
) -> Option<u16> {
    let inst = &instances[instance as usize];
    let name = db.resolve(&inst.class)?;
    if !crate::vehicle_class::is_turret(db, name) {
        return None;
    }
    let class = crate::roster::station_class_of(db, name)?;
    if !class.max_health.is_finite() || class.max_health <= 0.0 {
        return None;
    }
    let c = sim.add_station_class(class);
    let class = &sim.station_classes[usize::from(c)];
    let mut s = Station::new(c, glam::Vec3::from(inst.translation), 0.0);
    s.health = core::weapon::Health::new(class.max_health, class.health_type);
    s.instance = Some(instance);
    s.transform = crate::viewer::instance_matrix(inst.rotation, inst.translation);
    s.team = team_of(inst);
    s.motion = core::sim::StationMotion::new(instance + 1);
    let at = sim.stations.len() as u16;
    sim.collision.bind_station(instance, at);
    sim.stations.push(s);
    Some(at)
}

struct Parts<'a> {
    instances: &'a [ManifestInstance],
    db: &'a ClassDb,
}

impl Parts<'_> {
    fn seated(&self, instance: u32) -> bool {
        let inst = &self.instances[instance as usize];
        self.db
            .resolve(&inst.class)
            .is_some_and(|name| !crate::vehicle_class::seats(self.db, name).is_empty())
    }

    fn station(&self, sim: &mut Sim, name: &str) -> Option<u16> {
        let instance = instance_of(self.instances, name)?;
        if let Some(i) = sim
            .stations
            .iter()
            .position(|s| s.instance == Some(instance))
        {
            return Some(i as u16);
        }
        if self.seated(instance) {
            return None;
        }
        let at = add_part(sim, self.instances, self.db, instance)?;
        Some(at)
    }
}

fn add_regen(sa: &mut SpaceAssault, sim: &Sim, parts: &Parts, station: u16) {
    if sa.regen.iter().any(|(s, _)| *s == station) {
        return;
    }
    let Some(instance) = sim.stations[usize::from(station)].instance else {
        return;
    };
    let inst = &parts.instances[instance as usize];
    let add = parts
        .db
        .resolve(&inst.class)
        .and_then(|name| parts.db.prop_f32(name, "AddHealth"))
        .filter(|a| a.is_finite() && *a != 0.0);
    if let Some(add) = add {
        sa.regen.push((station, add));
    }
}

#[derive(Debug)]
pub struct SpaceObjective {
    pub u: f32,
    pub v: f32,
    pub kind: String,
    pub names: Vec<String>,
    pub att_vo: String,
    pub def_vo: String,
}

impl SpaceObjective {
    pub fn down_text(&self, attacking: bool) -> String {
        let role = if attacking { "atk" } else { "def" };
        format!("level.spa.hangar.{}.{role}.down", self.kind)
    }
}

pub fn objectives(def: &SpaceAssaultDef) -> Vec<SpaceObjective> {
    def.objectives
        .iter()
        .flat_map(|side| &side.systems)
        .map(|s| SpaceObjective {
            u: s.u,
            v: s.v,
            kind: s.kind.clone(),
            names: s.names.clone(),
            att_vo: s.att_vo.clone(),
            def_vo: s.def_vo.clone(),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default)]
struct Flash {
    last: f32,
    left: f32,
}

impl Flash {
    fn step(&mut self, value: f32, dt: f32) -> f32 {
        if value < self.last && self.left <= 0.0 {
            self.left = 1.0;
        }
        self.last = value;
        let shown = self.left.max(0.0);
        if self.left > 0.0 {
            self.left -= dt;
        }
        shown
    }
}

const HEALTH_RED: [f32; 3] = [200.0, 100.0, 100.0];
const HEALTH_YELLOW: [f32; 3] = [223.0, 223.0, 32.0];
const HEALTH_GREEN: [f32; 3] = [100.0, 200.0, 100.0];
const FLASH_WHITE: f32 = 255.0;

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] * (1.0 - t) + b[i] * t)
}

fn health_color(fraction: f32) -> [f32; 3] {
    if fraction > 0.5 {
        mix(HEALTH_YELLOW, HEALTH_GREEN, (fraction - 0.5) * 2.0)
    } else {
        mix(HEALTH_RED, HEALTH_YELLOW, fraction * 2.0)
    }
}

fn shield_color(fraction: f32) -> [f32; 4] {
    [
        (1.0 - fraction) * 255.0,
        fraction * 200.0,
        fraction * 150.0,
        fraction * 255.0,
    ]
}

#[derive(Debug, Default)]
pub struct SpaceHud {
    systems: Vec<Flash>,
    shields: [Flash; 2],
}

impl SpaceHud {
    pub fn status(
        &mut self,
        sim: &Sim,
        round: &SpaceRound,
        my_team: u8,
        dt: f32,
    ) -> Option<ui::hud_space::SpaceStatus> {
        let sa = sim.space_assault.as_ref()?;
        let meters = sim.capital_meters()?;
        let own = usize::from(my_team.min(1));
        let teams = [own, 1 - own];
        if self.systems.len() != round.objectives.len() {
            self.systems = vec![Flash::default(); round.objectives.len()];
        }
        let mut systems = Vec::new();
        for (index, objective) in round.objectives.iter().enumerate() {
            let parts: Vec<&core::sim::Station> = sa
                .critical
                .iter()
                .filter(|c| c.objective == Some(index as u16))
                .filter_map(|c| sim.stations.get(usize::from(c.station)))
                .collect();
            let Some(first) = parts.first() else {
                continue;
            };
            let (mut cur, mut max) = (0.0, 0.0);
            for s in &parts {
                cur += s.health.cur.max(0.0);
                max += s.health.max;
            }
            let fraction = if max > 0.0 { cur / max } else { 0.0 };
            let shown = self.systems[index].step(fraction, dt);
            if objective.u < 0.0 || objective.v < 0.0 {
                continue;
            }
            let rgb = if first.destroyed {
                [0.0; 3]
            } else {
                mix(health_color(fraction), [FLASH_WHITE; 3], shown)
            };
            systems.push(ui::hud_space::SpaceSystem {
                own: first.team == Some(my_team),
                u: objective.u,
                v: objective.v,
                rgb,
            });
        }
        let shield_rgba = teams.map(|team| {
            let fraction = meters[team].shield;
            let shown = self.shields[team].step(fraction, dt);
            let [r, g, b, a] = shield_color(fraction);
            [
                r * (1.0 - shown) + FLASH_WHITE * shown,
                g * (1.0 - shown) + FLASH_WHITE * shown,
                b * (1.0 - shown) + FLASH_WHITE * shown,
                a * (1.0 - shown) + FLASH_WHITE * shown,
            ]
        });
        Some(ui::hud_space::SpaceStatus {
            sheet_dir: round.sheet_dir.clone(),
            ship: teams.map(|t| format!("{}_icon_ass", round.faction[t])),
            shield: teams.map(|t| format!("{}_icon_ass_shield", round.faction[t])),
            ship_rgb: [
                ui::hud::target_color::FRIEND.map(f32::from),
                ui::hud::target_color::ENEMY.map(f32::from),
            ],
            shield_rgba,
            system: "spa_icon_critsys".to_owned(),
            systems,
        })
    }
}

#[derive(Debug, Default)]
pub struct SpaceRound {
    pub faction: [String; 2],
    pub sheet_dir: Option<std::path::PathBuf>,
    pub objectives: Vec<SpaceObjective>,
    pub scripted: Vec<crate::mission::SpaceScriptedDef>,
    pub team_text: [String; 2],
    pub win_vo: [String; 2],
    pub lose_vo: [String; 2],
    pub briefing_vo: [String; 2],
}

impl SpaceRound {
    pub fn new(def: &SpaceAssaultDef) -> Self {
        let mut round = Self {
            objectives: objectives(def),
            scripted: def.scripted.clone(),
            ..Self::default()
        };
        for side in &def.objectives {
            let team = usize::from(!side.attacker);
            round.faction[team] = side.side.clone();
            round.team_text[team] = side.text.clone();
            round.win_vo[team] = side.win_vo.clone();
            round.lose_vo[team] = side.lose_vo.clone();
            let prefix = match side.side.to_ascii_lowercase().as_str() {
                "all" => "A",
                "imp" => "I",
                "rep" => "R",
                "cis" => "C",
                _ => "",
            };
            if !prefix.is_empty() {
                let suffix = if side.attacker { "01" } else { "02" };
                round.briefing_vo[team] = format!("{prefix}OSMP_obj_{suffix}");
            }
        }
        round
    }

    /// Opening spawn command voice line ("head toward the enemy capital ship..." /
    /// "defend the flagship...") for the given team (0 attacker, 1 defender).
    pub fn briefing_for_team(&self, team: usize) -> Option<String> {
        if let Some(vo) = self.briefing_vo.get(team).filter(|s| !s.is_empty()) {
            return Some(vo.clone());
        }
        let faction = self.faction.get(team)?;
        let prefix = match faction.to_ascii_lowercase().as_str() {
            "all" => "A",
            "imp" => "I",
            "rep" => "R",
            "cis" => "C",
            _ => return None,
        };
        let is_attacker = team == 0;
        let suffix = if is_attacker { "01" } else { "02" };
        Some(format!("{prefix}OSMP_obj_{suffix}"))
    }

    pub fn scripted(
        &self,
        kind: u8,
        index: u16,
        up: bool,
    ) -> Option<&crate::mission::SpaceScriptedDef> {
        self.scripted
            .iter()
            .find(|e| e.kind == kind && e.index == index && e.up == up)
    }
}

pub fn install(
    sim: &mut Sim,
    def: &SpaceAssaultDef,
    score_limit: u32,
    instances: &[ManifestInstance],
    db: &ClassDb,
) {
    let parts = Parts { instances, db };
    let mut sa = SpaceAssault::new(score_limit);
    let objectives = objectives(def);
    sa.objectives_left = vec![0; objectives.len()];
    for (name, _, points) in &def.critical {
        let Some(station) = parts.station(sim, name) else {
            continue;
        };
        add_regen(&mut sa, sim, &parts, station);
        let objective = objectives
            .iter()
            .position(|o| o.names.iter().any(|n| n == name));
        if let Some(i) = objective {
            sa.objectives_left[i] += 1;
        }
        sa.lock_on.push(station);
        sa.critical.push(CriticalSystem {
            station,
            points: *points as i32,
            full_health: sim.stations[usize::from(station)].health.max,
            objective: objective.map(|i| i as u16),
            shown: objective.is_some_and(|i| objectives[i].names.first() == Some(name)),
        });
    }
    for (group, shields) in def.shields.iter().enumerate() {
        let controller = parts.station(sim, &shields.controller);
        let mut objects = Vec::new();
        for name in &shields.objects {
            let Some(station) = parts.station(sim, name) else {
                continue;
            };
            add_regen(&mut sa, sim, &parts, station);
            objects.push(station);
        }
        let mut linked = ShieldGroup::new(objects, controller, shields.max, shields.add);
        linked.lock_when_down = def.lock_shields.contains(&(group as u16));
        sa.shields.push(linked);
    }
    for sets in &def.destroyables {
        let mut linked = Vec::new();
        for set in sets {
            let stations: Vec<u16> = set
                .iter()
                .filter_map(|name| parts.station(sim, name))
                .collect();
            if !stations.is_empty() {
                linked.push(stations);
            }
        }
        if linked.len() > 1 {
            sa.destroyables.push(linked);
        }
    }
    for set in &def.turrets {
        sa.turrets.push(LinkedTurretSet {
            mainframe: parts.station(sim, &set.mainframe),
            team: set.team.saturating_sub(1),
            turrets: set
                .turrets
                .iter()
                .filter_map(|name| instance_of(instances, name))
                .collect(),
            disabled: false,
        });
    }
    for (frigate, turrets) in &def.frigate_turrets {
        let Some(frigate) = parts.station(sim, frigate) else {
            continue;
        };
        sa.frigates.push(FrigateTurrets {
            frigate,
            turrets: turrets
                .iter()
                .filter_map(|name| instance_of(instances, name))
                .collect(),
        });
    }
    sim.install_space_assault(sa);
}
