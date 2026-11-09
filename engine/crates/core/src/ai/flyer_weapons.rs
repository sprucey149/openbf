use super::arsenal::reach;
use crate::collision::HullFilter;
use crate::perception::los_seat;
use crate::sim::Sim;
use crate::unit::UnitHandle;
use crate::vehicle::flyer::{FlightPhase, FlyerAi, ThreatObject};
use crate::vehicle::{Vehicle, VehicleHandle, VehicleKind};
use crate::weapon::{OrdKind, WeaponClass, WeaponState};
use glam::Vec3;

const THINK: u64 = 120;
const THINK_STEP: u64 = 37;
const CLAIM_TICKS: u64 = 10 * crate::TICK_HZ as u64;
const SPLASH_AT: f32 = 2.0;
const SLOW_FLYER_SQ: f32 = 256.0;
const SLOW_LOB_SQ: f32 = 144.0;
const STRONG_SECONDS: f32 = 10.0;
const STRONG_SHARE: f32 = 0.5;
const PLENTY: u32 = 2;
const RAYS: usize = 2;
const STATION_HANDLE: u32 = 0xffff_0000;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Foe {
    pub handle: VehicleHandle,
    pub kind: u32,
    pub max_health: f32,
    pub speed_sq: f32,
    pub d2: f32,
    pub soft: bool,
    pub seats: usize,
}

fn splash(e: &crate::weapon::ExplosionClass, k: usize) -> f32 {
    let falloff = if SPLASH_AT < e.damage_inner {
        1.0
    } else if SPLASH_AT >= e.damage_outer {
        0.0
    } else {
        (e.damage_outer - SPLASH_AT) / (e.damage_outer - e.damage_inner)
    };
    e.damage * e.scales[k] * falloff
}

pub(super) fn dps(wc: &WeaponClass, kind: u32) -> f32 {
    let o = &wc.ordnance;
    let k = (kind as usize).min(o.scales.len() - 1);
    let mut dmg = o.max_damage * o.scales[k];
    for e in [o.explosion.as_ref(), o.expire.as_ref()]
        .into_iter()
        .flatten()
    {
        dmg += splash(e, k);
    }
    let delay = (0.5 * (wc.shot_delay[0] + wc.shot_delay[1])).max(1e-3);
    let salvo = wc.shots_per_salvo.max(1) as f32;
    let rounds = wc.rounds_per_clip as f32;
    if wc.reload_time <= 0.0 || wc.rounds_per_clip == 0 {
        salvo * dmg / delay
    } else {
        rounds * dmg
            / ((wc.rounds_per_clip / wc.shots_per_salvo.max(1)) as f32 * delay + wc.reload_time)
    }
}

pub(super) fn causes_damage(wc: &WeaponClass, ws: &WeaponState, foe: &Foe) -> Option<f32> {
    if ws.overheated || wc.ai_target_kind >> foe.kind.min(31) & 1 == 0 {
        return None;
    }
    let dps = dps(wc, foe.kind);
    if dps * STRONG_SECONDS > foe.max_health * STRONG_SHARE {
        return Some(dps);
    }
    if !foe.soft || dps <= 0.0 {
        return None;
    }
    match wc.ordnance.kind {
        OrdKind::Lobbed => (foe.speed_sq < SLOW_LOB_SQ).then_some(dps),
        OrdKind::Missile | OrdKind::Beam | OrdKind::Shell => Some(dps),
        OrdKind::Other => None,
    }
}

pub(super) fn claims_on(sim: &Sim, target: VehicleHandle, team: u8, me: VehicleHandle) -> usize {
    sim.alive_vehicles()
        .filter(|(h, v)| {
            *h != me
                && v.kind == VehicleKind::Flyer
                && v.team == Some(team)
                && v.flyer
                    .ai
                    .heavy_claim
                    .is_some_and(|(t, until)| t == target && sim.tick < until)
        })
        .count()
}

pub(crate) struct Choice {
    pub index: u8,
    pub claim: bool,
}

pub(crate) fn choose(
    sim: &Sim,
    v: &Vehicle,
    me: VehicleHandle,
    seat: usize,
    foe: &Foe,
    coin: bool,
) -> Option<Choice> {
    let setup = sim.seat_setup.get(v.class as usize)?.as_ref()?;
    let gun = setup.guns.get(seat)?.as_ref()?;
    let state = v.guns.get(seat)?.as_ref()?;
    let team = v.team?;
    let held = v
        .flyer
        .ai
        .heavy_claim
        .is_some_and(|(t, until)| t == foe.handle && sim.tick < until);
    let open = held || claims_on(sim, foe.handle, team, me) < foe.seats.max(1);
    let mut best: Option<Choice> = None;
    let (mut best_plenty, mut best_dps, mut best_in) = (false, -1.0f32, false);
    for (i, w) in gun.weapons.iter().enumerate().take(2) {
        let (wc, ws) = (&sim.weapon_classes[w.class], &state.weapons[i]);
        if ws.out_of_ammo(wc) {
            continue;
        }
        let heavy = matches!(wc.ordnance.kind, OrdKind::Missile | OrdKind::Lobbed);
        if (heavy && !open) || ws.reloading() {
            continue;
        }
        let Some(dmg) = causes_damage(wc, ws, foe) else {
            continue;
        };
        let plenty = ws.issued(wc).is_none_or(|r| r > PLENTY);
        let better = dmg > best_dps || (dmg == best_dps && coin);
        let [min, _, max] = wc.ai_range;
        let by_rule = if best_plenty {
            plenty && better
        } else {
            plenty || better
        };
        let take = if min * min < foe.d2 && foe.d2 < max * max {
            let take = !best_in || by_rule;
            best_in |= take;
            take
        } else {
            let far = reach(wc).max(max);
            !best_in && far * far > foe.d2 && by_rule
        };
        if take {
            best = Some(Choice {
                index: i as u8,
                claim: heavy,
            });
            best_plenty = plenty;
            best_dps = dmg;
        }
    }
    best
}

fn foe_of(h: VehicleHandle, t: &Vehicle, d2: f32) -> Foe {
    Foe {
        handle: h,
        kind: t.health.health_type as u32,
        max_health: t.health.max,
        speed_sq: t.vel.length_squared(),
        d2,
        soft: match t.kind {
            VehicleKind::Flyer => {
                matches!(
                    t.flyer.phase,
                    FlightPhase::Landed | FlightPhase::Takeoff | FlightPhase::Landing
                ) || t.vel.length_squared() < SLOW_FLYER_SQ
            }
            _ => true,
        },
        seats: t.seats.len(),
    }
}

struct Eyes {
    eye: Vec3,
    nose: Vec3,
    team: u8,
    reach: f32,
    view_mult: f32,
}

pub(super) fn armed_point(sim: &Sim, ai: &FlyerAi) -> Option<Vec3> {
    if let Some(s) = ai.armed_station {
        let s = sim
            .stations
            .get(usize::from(s))
            .filter(|s| !s.destroyed && s.health.alive())?;
        return Some(s.sphere_centre);
    }
    let t = sim.vehicle(ai.armed_target?)?;
    t.health.alive().then_some(t.pos)
}

pub(crate) fn object_foe(sim: &Sim, object: ThreatObject, eye: Vec3) -> Option<Foe> {
    match object {
        ThreatObject::Station(i) => {
            let st = sim.stations.get(usize::from(i))?;
            Some(Foe {
                handle: VehicleHandle {
                    index: STATION_HANDLE + u32::from(i),
                    gen: 0,
                },
                kind: st.health.health_type as u32,
                max_health: st.health.max,
                speed_sq: 0.0,
                d2: (st.sphere_centre - eye).length_squared(),
                soft: true,
                seats: 0,
            })
        }
        ThreatObject::Vehicle(h) => {
            let t = sim.vehicle(h)?;
            let (centre, _) = sim.vehicle_blast_sphere(t);
            Some(foe_of(h, t, (centre - eye).length_squared()))
        }
    }
}

fn armed_ahead(sim: &Sim, h: VehicleHandle, own: Option<u32>, e: &Eyes) -> Option<VehicleHandle> {
    let mut near: Vec<(f32, u64, VehicleHandle, Vec3, Option<u32>)> = Vec::new();
    for t in &sim.building_turrets {
        if t.down.is_some() {
            continue;
        }
        let Some(b) = sim.vehicle(t.handle).filter(|b| b.health.alive()) else {
            continue;
        };
        if !b.team.is_some_and(|x| sim.hostile(e.team, x)) {
            continue;
        }
        let (centre, radius) = sim.vehicle_blast_sphere(b);
        let to = centre - e.eye;
        let d2 = to.length_squared();
        let view = (20.0 * radius + 30.0) * e.view_mult * 2.0;
        let far = view.min(e.reach);
        if to.dot(e.nose) <= 0.0 || d2 >= far * far {
            continue;
        }
        near.push((d2, u64::from(t.handle.index), t.handle, centre, b.instance));
    }
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    near.into_iter()
        .take(RAYS)
        .find_map(|(_, _, tv, centre, instance)| {
            los_seat(
                sim,
                e.eye,
                centre,
                HullFilter::Except([Some(h), Some(tv)]),
                [own, instance],
            )
            .then_some(tv)
        })
}

pub(super) struct Pilot<'a> {
    pub me: UnitHandle,
    pub vehicle: &'a Vehicle,
    pub handle: VehicleHandle,
    pub seat: usize,
}

pub(super) fn think(
    sim: &Sim,
    pilot: &Pilot,
    ai: &mut FlyerAi,
    flyer_target: Option<VehicleHandle>,
    burst: &mut u8,
) {
    let Pilot {
        me,
        vehicle: v,
        handle: h,
        ..
    } = *pilot;
    if !(sim.tick + u64::from(me.index) * THINK_STEP).is_multiple_of(THINK) {
        return;
    }
    let (Some(s), Some(u), Some(team)) = (sim.seat_look(me), sim.unit(me), v.team) else {
        return;
    };
    let gun_reach = s
        .gun
        .weapons
        .iter()
        .take(2)
        .map(|w| reach(&sim.weapon_classes[w.class]))
        .fold(0.0f32, f32::max);
    let nose = s.rot * Vec3::Z;
    let tier = sim.unit_tier(u);
    let eyes = Eyes {
        eye: s.eye,
        nose,
        team,
        reach: gun_reach,
        view_mult: tier.view_mult,
    };
    ai.armed_target = armed_ahead(sim, h, v.instance, &eyes);
    let Some(target) = ai.armed_target.or(flyer_target) else {
        return;
    };
    let Some(t) = sim.vehicle(target) else {
        return;
    };
    let (centre, _) = sim.vehicle_blast_sphere(t);
    let foe = foe_of(target, t, (centre - s.eye).length_squared());
    pick_weapon(sim, pilot, ai, &foe, burst);
}

fn pick_weapon(sim: &Sim, pilot: &Pilot, ai: &mut FlyerAi, foe: &Foe, burst: &mut u8) {
    let coin = sim.tick & 1 == 0;
    let Some(pick) = choose(sim, pilot.vehicle, pilot.handle, pilot.seat, foe, coin) else {
        return;
    };
    if pick.claim {
        ai.heavy_claim = Some((foe.handle, sim.tick + CLAIM_TICKS));
    }
    if pick.index != ai.channel {
        ai.channel = pick.index;
        ai.fire_held = false;
        *burst = 0;
    }
}

pub(super) fn follow_threats(sim: &Sim, pilot: &Pilot, ai: &mut FlyerAi, burst: &mut u8) {
    let threats = &pilot.vehicle.flyer.threats;
    let object = threats.visible_object();
    ai.armed_target = None;
    ai.armed_station = None;
    match object {
        Some(ThreatObject::Vehicle(h)) => ai.armed_target = Some(h),
        Some(ThreatObject::Station(i)) => ai.armed_station = Some(i),
        None => {}
    }
    if ai.threat_thinks == threats.thinks {
        return;
    }
    ai.threat_thinks = threats.thinks;
    let Some(s) = sim.seat_look(pilot.me) else {
        return;
    };
    if let Some(foe) = object.and_then(|o| object_foe(sim, o, s.eye)) {
        pick_weapon(sim, pilot, ai, &foe, burst);
    }
}

pub fn armed_shot(sim: &Sim, me: UnitHandle, ai: &FlyerAi) -> bool {
    let Some(s) = sim.seat_look(me) else {
        return false;
    };
    let at = if ai.armed_station.is_some() {
        armed_point(sim, ai)
    } else {
        ai.armed_target
            .and_then(|t| sim.vehicle(t))
            .filter(|t| t.health.alive())
            .map(|t| {
                let centre = sim.vehicle_blast_sphere(t).0;
                if t.kind == VehicleKind::Flyer {
                    let speed = s.gun.weapons.get(usize::from(ai.channel)).map_or(0.0, |w| {
                        sim.weapon_classes[w.class].shot_ordnance().velocity
                    });
                    crate::collision::lead_point(centre - s.eye, t.vel, speed)
                        .map_or(centre, |rel| s.eye + rel)
                } else {
                    centre
                }
            })
    };
    let Some(at) = at else {
        return false;
    };
    let Some(w) = s.gun.weapons.get(usize::from(ai.channel)) else {
        return false;
    };
    let wc = &sim.weapon_classes[w.class];
    let to = at - s.muzzle;
    let d2 = to.length_squared();
    let [min, _, max] = wc.ai_range;
    min * min < d2 && d2 < max * max && super::gunner::flyer_on_target(s.rot * Vec3::Z, to)
}
