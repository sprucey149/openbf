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

pub fn install(
    sim: &mut Sim,
    def: &SpaceAssaultDef,
    score_limit: u32,
    instances: &[ManifestInstance],
    db: &ClassDb,
) {
    let parts = Parts { instances, db };
    let mut sa = SpaceAssault::new(score_limit);
    for (name, _, points) in &def.critical {
        let Some(station) = parts.station(sim, name) else {
            continue;
        };
        add_regen(&mut sa, sim, &parts, station);
        sa.critical.push(CriticalSystem {
            station,
            points: *points as i32,
            full_health: sim.stations[usize::from(station)].health.max,
        });
    }
    for shields in &def.shields {
        let controller = parts.station(sim, &shields.controller);
        let mut objects = Vec::new();
        for name in &shields.objects {
            let Some(station) = parts.station(sim, name) else {
                continue;
            };
            add_regen(&mut sa, sim, &parts, station);
            objects.push(station);
        }
        sa.shields.push(ShieldGroup::new(
            objects,
            controller,
            shields.max,
            shields.add,
        ));
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
