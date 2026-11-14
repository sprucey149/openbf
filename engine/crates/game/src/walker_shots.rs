use crate::walk::WalkSim;
use core::sim::Event;
use core::unit::UnitHandle;
use core::vehicle::VehicleKind;
use core::weapon::FireState;
use glam::Mat4;

/// A walker or flyer gun's fire loop (the class table's `0x2298cae8`). Its
/// rounds draw as their own ordnance and its `SecondaryOrdnanceName` round
/// as its own (a chaingun's tracer is a `beam`, `Sim::beams`), as the PS2
/// makes each through its class's create slot (`analysis/re-ordnance-kinds.md`
/// §7).
#[derive(Clone, Debug, Default)]
pub struct Style {
    pub fire_sound: Option<String>,
    /// The fire loop's fade in and out, seconds (class +0x2d0 `0x674fc158`,
    /// +0x2d4 `0x0f1ca143`; B2 `FUN_0032ffd8`).
    pub fire_fade: [f32; 2],
    pub spin: Option<SpinSound>,
}

/// A chaingun's spin motor (class +0x2b4, `0xca333e94`): started at the
/// press when the class winds up (B2 `FUN_0032fa98`), it rises from `pitch`
/// to the sound's own as the gun winds up and falls back as it winds down,
/// fading in over `fade[0]` and out over the last `fade[1]` of the wind-down
/// (B2 `FUN_0032fbc8`).
#[derive(Clone, Debug)]
pub struct SpinSound {
    pub name: String,
    /// Class +0x2bc (`0x1a2034be`), the ctor's 1.0 unless authored.
    pub pitch: f32,
    /// Class +0x2c0 (`0x22e5153c`) and +0x2c4 (`0x25820dbf`), seconds.
    pub fade: [f32; 2],
}

/// Where one of a gun's loop voices is heading this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ramp {
    /// Fading in over these seconds; a stopped voice may start.
    Rise(f32),
    /// Holding its level.
    Hold,
    /// Fading out over these seconds; a stopped voice stays stopped.
    Fall(f32),
}

/// One loop voice of a seated walker or flyer gun.
#[derive(Clone, Debug)]
pub struct Voice<'a> {
    pub unit: UnitHandle,
    pub class: usize,
    /// The spin motor rather than the fire loop.
    pub spin: bool,
    pub name: &'a str,
    pub pos: glam::Vec3,
    pub pitch: f32,
    pub ramp: Ramp,
}

/// The fire ring's turn `event` fired on, a blocked barrel's passed turns
/// included ([`core::weapon::WeaponState::last_turn`]): the barrel it left.
/// Exact while a tick's rounds pass no turn between them.
pub fn turn(walk: &WalkSim, unit: UnitHandle, class: usize, event: usize) -> u64 {
    let count = walk
        .sim
        .unit(unit)
        .and_then(|u| u.mount)
        .and_then(|m| {
            let v = walk.sim.vehicle(m.vehicle)?;
            let setup = walk
                .sim
                .seat_setup
                .get(v.class as usize)?
                .as_ref()?
                .guns
                .get(m.seat as usize)?
                .as_ref()?;
            let slot = setup.weapons.iter().position(|w| w.class == class)?;
            Some(
                v.guns
                    .get(m.seat as usize)?
                    .as_ref()?
                    .weapons
                    .get(slot)
                    .map(|w| w.last_turn + 1)?,
            )
        })
        .unwrap_or(1);
    let after=walk.sim.events.iter().skip(event+1).filter(|e|matches!(e,Event::Shot {unit:u,weapon_class,..} if *u==unit && *weapon_class as usize==class)).count() as u64;
    count.saturating_sub(after + 1)
}

pub fn style(walk: &WalkSim, class: usize) -> Option<&Style> {
    walk.walker_shot_styles.get(&class)
}

pub fn frame(walk: &WalkSim, unit: UnitHandle, class: usize, shot: u64) -> Option<Mat4> {
    let m = walk.sim.unit(unit)?.mount?;
    let v = walk.sim.vehicle(m.vehicle)?;
    if v.kind != VehicleKind::Walker {
        return None;
    }
    let seat = walk
        .vehicle_seats
        .get(v.class as usize)?
        .get(m.seat as usize)?;
    let name = &walk.sim.weapon_classes.get(class)?.name;
    let weapon = seat
        .weapons
        .iter()
        .position(|w| w.class.eq_ignore_ascii_case(name))?;
    let points: Vec<_> = seat
        .aimers
        .iter()
        .filter(|a| a.weapon == weapon && !a.no_fire)
        .flat_map(|a| a.muzzles())
        .collect();
    let name = points
        .get(shot as usize % points.len().max(1))
        .copied()
        .or(seat.fire_point.as_deref())?;
    let rig = walk.walker_rigs.get(v.class as usize)?.as_ref()?;
    let scale = walk
        .sim
        .walker_params
        .get(v.class as usize)?
        .as_ref()?
        .root_scale;
    Some(Mat4::from_rotation_translation(v.rot, v.pos) * rig.node_now(v, scale, name)?)
}

/// The fire loops sounding at full or rising level this tick, by unit,
/// weapon class, sound and place ([`voices`] without the spin motors and
/// the fading loops).
pub fn loops(walk: &WalkSim) -> Vec<(UnitHandle, usize, &str, glam::Vec3)> {
    voices(walk)
        .into_iter()
        .filter(|v| !v.spin && !matches!(v.ramp, Ramp::Fall(_)))
        .map(|v| (v.unit, v.class, v.name, v.pos))
        .collect()
}

/// Every seated walker and flyer gun's loop voices this tick. A gun that
/// winds up keeps its fire loop to the ticks it fires on (bit 0 of its
/// weapon +0x144 set and the spin timer +0x140 run out, B2 `FUN_0032e8c0`
/// starting it through `FUN_0032fec8`) and fades it out once released; the
/// spin motor runs from the press to the end of the wind-down. A gun that
/// does not wind up loops while its trigger holds a firing weapon.
pub fn voices(walk: &WalkSim) -> Vec<Voice<'_>> {
    let mut out = Vec::new();
    for (h, u) in walk.sim.alive_units() {
        let Some(m) = u.mount else { continue };
        let Some(v) = walk.sim.vehicle(m.vehicle).filter(|v| {
            matches!(
                v.kind,
                VehicleKind::Walker | VehicleKind::Flyer | VehicleKind::Hover
            )
        }) else {
            continue;
        };
        let Some(gun) = walk
            .sim
            .seat_setup
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .and_then(|s| s.guns.get(m.seat as usize))
            .and_then(Option::as_ref)
        else {
            continue;
        };
        let Some(state) = v.guns.get(m.seat as usize).and_then(Option::as_ref) else {
            continue;
        };
        for (w, s) in gun.weapons.iter().zip(&state.weapons) {
            let Some(wc) = walk.sim.weapon_classes.get(w.class) else {
                continue;
            };
            let style = style(walk, w.class);
            let Some(name) = style
                .and_then(|s| s.fire_sound.as_deref())
                .or(wc.fire_loop_sound.as_deref())
            else {
                continue;
            };
            let pos = frame(walk, h, w.class, s.last_turn).map_or(v.pos, |m| m.w_axis.truncate());
            let fire = |ramp: Ramp| Voice {
                unit: h,
                class: w.class,
                spin: false,
                name,
                pos,
                pitch: 1.0,
                ramp,
            };
            if wc.wind_up <= 0.0 {
                let live = s.fire_cmd > 0
                    && s.shots_fired > 0
                    && !s.overheated
                    && s.reload_timer <= 0.0
                    && s.active;
                if live {
                    out.push(fire(Ramp::Rise(0.0)));
                }
                continue;
            }
            if s.state != FireState::Firing || !s.active {
                continue;
            }
            let fade = style.map_or([0.0; 2], |s| s.fire_fade);
            let engaged = !s.spin_down;
            if engaged && s.spin <= 0.0 {
                out.push(fire(Ramp::Rise(fade[0])));
            } else if !engaged {
                out.push(fire(Ramp::Fall(fade[1])));
            }
            let Some(spin) = style.and_then(|s| s.spin.as_ref()) else {
                continue;
            };
            let (span, from, to) = if engaged {
                (wc.wind_up, spin.pitch, 1.0)
            } else {
                (wc.wind_down, 1.0, spin.pitch)
            };
            let f = if span > 0.0 && s.spin > 0.0 {
                ((span - s.spin) / span).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let ramp = if engaged {
                Ramp::Rise(spin.fade[0])
            } else if s.spin <= spin.fade[1] {
                Ramp::Fall(spin.fade[1])
            } else {
                Ramp::Hold
            };
            out.push(Voice {
                spin: true,
                name: &spin.name,
                pitch: (1.0 - f) * from + f * to,
                ..fire(ramp)
            });
        }
    }
    out
}
