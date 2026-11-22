//! `HERO_CENSUS=1` on a timed headless battle (`--headless --seconds N`):
//! what every AI hero life did, from the sim's own state and events. It only
//! reads the sim, so the round it describes is the round that was played.
//!
//! One stdout line per hero class and one `herototal` line:
//!
//! ```text
//! hero <class>: lives L kills K deaths D kpl K/L kd K/D life_s S | target T % dist M m close C % |
//!   swings W/min saber_hits H force F/min throw R/min gun G/min | block B % blocked X |
//!   stuck P % | env E [cause n, ...]
//! ```
//!
//! - `target`: share of the life with a unit target; `dist`: mean planar
//!   distance to it; `close`: share of target time inside 4 m.
//! - `swings`: rising edges of an authored attack window; `saber_hits`: the
//!   hero's saber hits on units; `force` / `throw`: Force and saber-throw
//!   uses (`Event::Ability`); `gun`: shots of a gun in hand.
//! - `block`: share of target time in a Deflect state; `blocked`: blocks
//!   accepted (the deflect sound at the hero).
//! - `stuck`: share of target time, sampled each second, with the target
//!   over 6 m away and under 1 m moved in the last 3 s.
//! - `env`: deaths the world caused (falls, hero exhaustion, ...).

use core::sim::{DeathCause, Event};
use core::unit::Controller;
use core::TICK_HZ;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
struct Life {
    class: String,
    start: u64,
    end: u64,
    kills: u32,
    died: bool,
    env: Option<DeathCause>,
    target_ticks: u64,
    dist_sum: f64,
    close_ticks: u64,
    deflect_ticks: u64,
    blocked: u32,
    swings: u32,
    saber_hits: u32,
    force: u32,
    throws: u32,
    gun: u32,
    stuck_samples: u32,
    target_samples: u32,
    in_attack: bool,
    /// Target time by planar distance: under 4, 4-10, 10-25, 25 m and over.
    bands: [u64; 4],
    /// Target time inside 4 m with an attack window open.
    close_attack: u64,
    trail: Vec<glam::Vec3>,
}

#[derive(Default)]
pub(super) struct HeroCensus {
    kits: BTreeMap<String, String>,
    live: BTreeMap<usize, (u32, Life)>,
    done: Vec<Life>,
}

pub(super) fn wanted() -> bool {
    std::env::var("HERO_CENSUS").is_ok_and(|v| v != "0")
}

impl HeroCensus {
    pub(super) fn tick(&mut self, walk: &game::walk::WalkSim) {
        let sim = &walk.sim;
        let hz = u64::from(TICK_HZ);
        // Events first: they belong to the lives that were open this tick.
        for e in &sim.events {
            match *e {
                Event::Kill { shooter, victim } => {
                    if let Some((g, l)) = self.live.get_mut(&(shooter.index as usize)) {
                        if *g == shooter.gen && shooter != victim {
                            l.kills += 1;
                        }
                    }
                }
                Event::EnvDeath { unit, cause } => {
                    if let Some((g, l)) = self.live.get_mut(&(unit.index as usize)) {
                        if *g == unit.gen {
                            l.env = Some(cause);
                        }
                    }
                }
                Event::Ability {
                    unit, weapon_class, ..
                } => {
                    if let Some((g, l)) = self.live.get_mut(&(unit.index as usize)) {
                        if *g == unit.gen {
                            let wc = &sim.weapon_classes[weapon_class as usize];
                            if wc.force.is_some() {
                                l.force += 1;
                            } else if wc.saber_throw.is_some() {
                                l.throws += 1;
                            }
                        }
                    }
                }
                Event::Shot {
                    unit, weapon_class, ..
                } => {
                    if let Some((g, l)) = self.live.get_mut(&(unit.index as usize)) {
                        let wc = &sim.weapon_classes[weapon_class as usize];
                        if *g == unit.gen && wc.force.is_none() && wc.saber.is_none() {
                            l.gun += 1;
                        }
                    }
                }
                Event::Hit {
                    shooter,
                    victim,
                    direct: true,
                    ..
                } if shooter != victim => {
                    if let Some((g, l)) = self.live.get_mut(&(shooter.index as usize)) {
                        let saber = sim
                            .unit(shooter)
                            .is_some_and(|u| sim.weapon_classes[u.weapon_class].saber.is_some());
                        if *g == shooter.gen && saber {
                            l.saber_hits += 1;
                        }
                    }
                }
                Event::ComboSound {
                    unit,
                    weapon_class,
                    sound_hash,
                } => {
                    if let Some((g, l)) = self.live.get_mut(&(unit.index as usize)) {
                        let deflect = sim.weapon_classes[weapon_class as usize]
                            .saber
                            .as_ref()
                            .and_then(|s| s.deflect_sound.as_deref())
                            .map(core::hero_weapon::sound_hash);
                        if *g == unit.gen && deflect == Some(sound_hash) {
                            l.blocked += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        for (i, u) in sim.units.iter().enumerate() {
            // A hero class by its name: Hero Assault has no hero manager, so
            // its classes never take the manager's runtime hero bit.
            let hero = u.alive
                && u.controller == Controller::Ai
                && !u.is_player
                && (u.consts.hero
                    || walk
                        .team_template(u.team)
                        .class(u.spec.class)
                        .name
                        .contains("_hero_"));
            let open = self
                .live
                .get(&i)
                .is_some_and(|(g, _)| *g == u.gen && u.alive);
            if !open {
                if let Some((_, mut l)) = self.live.remove(&i) {
                    l.end = sim.tick;
                    l.died = !u.alive || l.env.is_some();
                    self.done.push(l);
                }
                if hero {
                    let class = walk.team_template(u.team).class(u.spec.class).name.clone();
                    self.kits.entry(class.clone()).or_insert_with(|| {
                        let kit = |c: usize| {
                            let w = &sim.weapon_classes[c];
                            let kind = if w.saber.is_some() {
                                "saber".to_owned()
                            } else if let Some(f) = &w.force {
                                format!("force[{:?} e{} r{}]", std::mem::discriminant(&f.kind), f.energy, f.range)
                                    .replace("Discriminant", "")
                            } else if let Some(t) = &w.saber_throw {
                                format!("throw[e{}]", t.energy)
                            } else {
                                "gun".to_owned()
                            };
                            format!(
                                "{} {kind} ai {:?} mask {:#x} dmg {} delay {:?} salvo {} clip {} reload {}",
                                w.name,
                                w.ai_range,
                                w.ai_target_kind,
                                w.ordnance.max_damage * w.ordnance.scales[0],
                                w.shot_delay,
                                w.shots_per_salvo,
                                w.rounds_per_clip,
                                w.reload_time
                            )
                        };
                        std::iter::once(kit(u.weapon_class))
                            .chain(u.held.iter().map(|h| format!("s{}c{} {}", h.slot, h.channel, kit(h.class))))
                            .collect::<Vec<_>>()
                            .join(" ; ")
                    });
                    self.live.insert(
                        i,
                        (
                            u.gen,
                            Life {
                                class,
                                start: sim.tick,
                                ..Life::default()
                            },
                        ),
                    );
                } else {
                    continue;
                }
            }
            let Some((_, l)) = self.live.get_mut(&i) else {
                continue;
            };
            let melee = u.weapon.melee.as_ref();
            let attacking = melee.is_some_and(|m| !m.attacks.is_empty());
            if attacking && !l.in_attack {
                l.swings += 1;
            }
            l.in_attack = attacking;
            let target =
                u.ai.target
                    .and_then(|f| f.unit())
                    .and_then(|h| sim.unit(h))
                    .filter(|t| t.alive);
            let second = sim.tick.is_multiple_of(hz);
            if second {
                l.trail.push(u.soldier.pos);
            }
            if let Some(t) = target {
                let d = t.soldier.pos - u.soldier.pos;
                let planar = glam::Vec2::new(d.x, d.z).length();
                l.target_ticks += 1;
                l.dist_sum += f64::from(planar);
                if planar < 4.0 {
                    l.close_ticks += 1;
                    if attacking {
                        l.close_attack += 1;
                    }
                }
                l.bands[match planar {
                    p if p < 4.0 => 0,
                    p if p < 10.0 => 1,
                    p if p < 25.0 => 2,
                    _ => 3,
                }] += 1;
                if melee.is_some_and(|m| m.deflect.is_some()) {
                    l.deflect_ticks += 1;
                }
                if second {
                    l.target_samples += 1;
                    let n = l.trail.len();
                    if planar > 6.0 && n > 3 && (l.trail[n - 1] - l.trail[n - 4]).length() < 1.0 {
                        l.stuck_samples += 1;
                    }
                }
            }
        }
    }

    pub(super) fn report(mut self, end: u64) -> String {
        for (_, (_, mut l)) in std::mem::take(&mut self.live) {
            l.end = end;
            self.done.push(l);
        }
        let mut by: BTreeMap<String, Vec<&Life>> = BTreeMap::new();
        for l in &self.done {
            by.entry(l.class.clone()).or_default().push(l);
            by.entry("~all".to_owned()).or_default().push(l);
        }
        let mut out = String::new();
        for (class, kit) in &self.kits {
            out += &format!("herokit {class}: {kit}\n");
        }
        let hz = f64::from(TICK_HZ);
        for (class, lives) in by {
            let n = lives.len() as f64;
            let kills: u32 = lives.iter().map(|l| l.kills).sum();
            let deaths = lives.iter().filter(|l| l.died).count();
            let ticks: u64 = lives.iter().map(|l| l.end - l.start).sum();
            let mins = (ticks as f64 / hz / 60.0).max(1e-9);
            let target: u64 = lives.iter().map(|l| l.target_ticks).sum();
            let pct = |a: u64, b: u64| 100.0 * a as f64 / (b as f64).max(1.0);
            let sum = |f: fn(&Life) -> u32| lives.iter().map(|l| f(l)).sum::<u32>();
            let mut env: BTreeMap<String, u32> = BTreeMap::new();
            for l in &lives {
                if let Some(c) = l.env {
                    *env.entry(format!("{c:?}")).or_default() += 1;
                }
            }
            out += &format!(
                "{} {class}: lives {} kills {kills} deaths {deaths} kpl {:.2} kd {:.2} life_s {:.0} | target {:.0} % dist {:.1} m close {:.0} % bands {} swing_close {:.0} % | swings {:.1}/min saber_hits {} force {:.1}/min throw {:.1}/min gun {:.1}/min | block {:.0} % blocked {} | stuck {:.0} % | env {} [{}]\n",
                if class == "~all" { "herototal" } else { "hero" },
                lives.len(),
                kills as f64 / n.max(1.0),
                kills as f64 / (deaths as f64).max(1.0),
                ticks as f64 / hz / n.max(1.0),
                pct(target, ticks),
                lives.iter().map(|l| l.dist_sum).sum::<f64>() / (target as f64).max(1.0),
                pct(lives.iter().map(|l| l.close_ticks).sum(), target),
                (0..4)
                    .map(|b| format!("{:.0}", pct(lives.iter().map(|l| l.bands[b]).sum(), target)))
                    .collect::<Vec<_>>()
                    .join("/"),
                pct(
                    lives.iter().map(|l| l.close_attack).sum(),
                    lives.iter().map(|l| l.close_ticks).sum()
                ),
                sum(|l| l.swings) as f64 / mins,
                sum(|l| l.saber_hits),
                sum(|l| l.force) as f64 / mins,
                sum(|l| l.throws) as f64 / mins,
                sum(|l| l.gun) as f64 / mins,
                pct(lives.iter().map(|l| l.deflect_ticks).sum(), target),
                sum(|l| l.blocked),
                100.0 * f64::from(sum(|l| l.stuck_samples))
                    / f64::from(sum(|l| l.target_samples)).max(1.0),
                env.values().sum::<u32>(),
                env.iter()
                    .map(|(c, n)| format!("{c} {n}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        out
    }
}
