//! A sim tick's events as sounds: which sound property each one starts, and
//! where (fire / reload / impact / explosion / weapon change / empty click /
//! door / the squad's answer to an order / a station's chatter).
//! Called once per stepped tick (`WalkSim::advance_with`, card EVTK):
//! `Sim::events` keeps a tick's events until the next step, so a render frame
//! that steps no tick would start every sound again, and one that steps two
//! would lose the first tick's.

pub(crate) mod ordnance;

use crate::sounds::Listener;
use crate::walk::WalkSim;
use core::sim::Event;
use glam::Vec3;

fn saber_sound(class: &core::hero_weapon::SaberClass, hash: u32) -> Option<&str> {
    fn find_fields(nodes: &[manifest::combo::ComboField], hash: u32) -> Option<&str> {
        for field in nodes {
            if field.keyword == manifest::combo::ComboKeyword::Sound {
                if let Some(name) = field
                    .text(0)
                    .filter(|n| crate::classdb::fnv1a_lowercase(n) == hash)
                {
                    return Some(name);
                }
            }
            if let Some(name) = find_fields(&field.children, hash) {
                return Some(name);
            }
        }
        None
    }
    [&class.hit_sound, &class.deflect_sound]
        .into_iter()
        .flatten()
        .find(|n| crate::classdb::fnv1a_lowercase(n) == hash)
        .map(String::as_str)
        .or_else(|| find_fields(&class.combo.fields, hash))
}

/// The local player's ears.
pub fn listener(walk: &WalkSim) -> Listener {
    let me = &walk.sim.units[walk.player.index as usize];
    if me.mount.is_some() || (!me.alive && walk.seat_view_timer > 0.0) {
        let cam = walk.camera();
        return Listener {
            pos: cam.position,
            yaw: cam.yaw,
        };
    }
    Listener {
        pos: me.soldier.pos + Vec3::Y * 1.6,
        yaw: me.soldier.yaw,
    }
}

/// `play(name, pos)` for each sound this tick's events start, heard from
/// `ears`.
pub fn each(walk: &WalkSim, ears: Vec3, play: impl FnMut(&str, Vec3)) {
    each_with(walk, ears, true, play);
}

/// [`each`] without what starts on a human's weapon handle ([`weapon_asks`]):
/// the mixer's path, which has to hold those voices to cut them.
pub fn each_off_handle(walk: &WalkSim, ears: Vec3, play: impl FnMut(&str, Vec3)) {
    each_with(walk, ears, false, play);
}

/// The weapon that fired starts a sound once per trigger pull, not per bolt
/// (card SNDX, re-weapon-sounds.md 7.7): a shot after the pull's first starts
/// it again only on a class with a `SalvoDelay`. Salvos without a delay all
/// leave in one update, so "the pull's first" is this tick's first of the
/// unit's weapon, which `fired` keeps.
fn shot_sound<'a>(
    walk: &'a WalkSim,
    fired: &mut Vec<(core::unit::UnitHandle, u32)>,
    unit: core::unit::UnitHandle,
    weapon_class: u32,
) -> Option<&'a str> {
    let wc = &walk.sim.weapon_classes[weapon_class as usize];
    let first = !fired.contains(&(unit, weapon_class));
    if first {
        fired.push((unit, weapon_class));
    }
    wc.fire_sound
        .as_deref()
        .filter(|_| fire_sound_starts(first, wc))
}

fn human(walk: &WalkSim, unit: core::unit::UnitHandle) -> bool {
    walk.sim.unit(unit).is_some_and(|u| u.is_player)
}

/// This tick's starts on a human's weapon handle (B1 +0xf4, B2 +0xd8), in
/// event order: its `FireEmptySound` and its `FireSound`. The empty click
/// stops what the handle still sounds (`FUN_00355768` 1 -> `FUN_00355968`;
/// re-weapon-sounds.md 7.3), so a pistol's 1 s tail cannot hide it, and one
/// handle holds one click however fast the trigger is mashed.
pub fn weapon_asks(walk: &WalkSim, mut ask: impl FnMut(crate::sounds::WeaponAsk)) {
    let hash = crate::classdb::fnv1a_lowercase;
    let mut fired = Vec::new();
    for e in &walk.sim.events {
        match *e {
            Event::FireEmpty {
                unit,
                weapon_class,
                pos,
            } => {
                if let Some(n) = &walk.sim.weapon_classes[weapon_class as usize].empty_sound {
                    ask(crate::sounds::WeaponAsk {
                        unit: unit.index,
                        weapon_class,
                        hash: hash(n),
                        pos,
                        click: true,
                    });
                }
            }
            Event::Shot {
                unit,
                weapon_class,
                origin,
                ..
            } if human(walk, unit) => {
                if let Some(n) = shot_sound(walk, &mut fired, unit, weapon_class) {
                    ask(crate::sounds::WeaponAsk {
                        unit: unit.index,
                        weapon_class,
                        hash: hash(n),
                        pos: origin,
                        click: false,
                    });
                }
            }
            _ => {}
        }
    }
}

fn each_with(walk: &WalkSim, ears: Vec3, handles: bool, mut play: impl FnMut(&str, Vec3)) {
    // A selected weapon's `WeaponChangeSound`, at its unit (card WSND,
    // re-weapon-sounds.md §7.1: every unit, no owner test).
    let equip = |class: usize, pos: Vec3, play: &mut dyn FnMut(&str, Vec3)| {
        if let Some(n) = &walk.sim.weapon_classes[class].change_sound {
            play(n, pos);
        }
    };
    // A spawn selects each channel's weapon (B1 `FUN_0020cbd0` 121397: the
    // PS2 logged two equip sounds at one instant on the player's spawn).
    let spawned = |u: &core::unit::Unit, play: &mut dyn FnMut(&str, Vec3)| {
        equip(u.weapon_class, u.soldier.pos, play);
        if let Some(w) = u.cur1.and_then(|k| u.held.get(usize::from(k))) {
            equip(w.class, u.soldier.pos, play);
        }
    };
    // The battle's first tick: everyone on the map has just spawned.
    if walk.sim.tick == 1 {
        for (_, u) in walk.sim.alive_units() {
            spawned(u, &mut play);
        }
    }
    // An explosion class's `SoundProperty`: once per explosion, at the blast
    // (§4.1), at each of the three places `Fx::ingest` detonates one.
    let boom =
        |x: Option<&core::impact::ExplosionLook>, pos: Vec3, play: &mut dyn FnMut(&str, Vec3)| {
            if let Some(n) = x.and_then(|x| x.sound.as_ref()) {
                play(n, pos);
            }
        };
    // The weapons that fired this tick (see `Event::Shot`).
    let mut fired: Vec<(core::unit::UnitHandle, u32)> = Vec::new();
    for e in &walk.sim.events {
        match e {
            Event::ComboSound {
                unit,
                weapon_class,
                sound_hash,
            } => {
                let sound = walk
                    .sim
                    .weapon_classes
                    .get(*weapon_class as usize)
                    .and_then(|w| w.saber.as_ref())
                    .and_then(|s| saber_sound(s, *sound_hash));
                if let (Some(u), Some(sound)) = (
                    walk.sim
                        .units
                        .get(unit.index as usize)
                        .filter(|u| u.gen == unit.gen),
                    sound,
                ) {
                    play(sound, u.soldier.pos);
                }
            }
            Event::SaberThrowHit { pos, weapon_class } => {
                if let Some(sound) = walk
                    .sim
                    .weapon_classes
                    .get(*weapon_class as usize)
                    .and_then(|w| w.saber.as_ref())
                    .and_then(|s| s.hit_sound.as_deref())
                {
                    play(sound, *pos);
                }
            }
            Event::Impact {
                pos, weapon_class, ..
            } => {
                let look = walk.sim.weapon_classes[*weapon_class as usize]
                    .ordnance
                    .impact
                    .as_ref();
                boom(look.and_then(|l| l.explosion.as_ref()), *pos, &mut play);
                // The contact owns its weapon and position, even after the
                // shooter switches weapons or its splash damages other units
                // (re-weapon-sounds.md §5.5; card HITSND).
                if let Some(n) = &walk.sim.weapon_classes[*weapon_class as usize]
                    .ordnance
                    .collision_sound
                {
                    play(n, *pos);
                }
            }
            Event::Droideka {
                unit,
                pos,
                cue: core::droideka::Cue::ShieldOff,
            } => {
                if let Some(n) = walk
                    .sim
                    .unit(*unit)
                    .and_then(|u| u.droideka_class())
                    .and_then(|c| c.shield.off_sound.as_deref())
                {
                    play(n, *pos);
                }
            }
            Event::MineTick { pos, weapon_class } => {
                if let Some(sound) = walk.sim.weapon_classes[*weapon_class as usize]
                    .mine
                    .as_ref()
                    .and_then(|d| d.item.tick_sound.as_deref())
                {
                    play(sound, *pos);
                }
            }
            Event::MineBlast {
                pos,
                weapon_class,
                cause,
            } => {
                let e = walk.sim.weapon_classes[*weapon_class as usize]
                    .mine
                    .as_ref()
                    .and_then(|d| d.item.explosion(*cause));
                boom(e.map(|e| &e.look), *pos, &mut play);
            }
            Event::GuidedGone {
                pos,
                weapon_class,
                end,
            } => {
                if let Some(g) = &walk.sim.weapon_classes[*weapon_class as usize].guided {
                    let look = g.explosion(*end).map(|e| &e.look);
                    boom(look, *pos, &mut play);
                    if let Some(name) = g
                        .class
                        .death_sound
                        .as_ref()
                        .filter(|&name| look.and_then(|l| l.sound.as_ref()) != Some(name))
                    {
                        play(name, *pos);
                    }
                }
            }
            Event::DroidCharge { pos, weapon_class } => {
                let r = walk.sim.weapon_classes[*weapon_class as usize]
                    .remote
                    .as_ref();
                if let Some(n) = r
                    .and_then(|r| r.destruct.as_ref())
                    .and_then(|x| x.charge_sound.as_deref())
                {
                    play(n, *pos);
                }
            }
            Event::DroidGone {
                pos,
                weapon_class,
                end,
            } => {
                if let Some(r) = &walk.sim.weapon_classes[*weapon_class as usize].remote {
                    for x in r.end_explosions(*end) {
                        boom(Some(&x.look), *pos, &mut play);
                    }
                    if let Some(n) = r.end_burst(*end).and_then(|x| x.burst_sound.as_deref()) {
                        play(n, *pos);
                    }
                }
            }
            Event::TurretGone {
                pos,
                weapon_class,
                end,
            } => {
                let e = walk.sim.weapon_classes[*weapon_class as usize]
                    .turret
                    .as_ref()
                    .and_then(|d| d.item.end_explosion(*end));
                if let Some(e) = e {
                    boom(Some(&e.look), *pos, &mut play);
                }
            }
            Event::WeaponBlast { pos, weapon_class } => {
                boom(
                    walk.sim.weapon_classes[*weapon_class as usize]
                        .ordnance
                        .explosion
                        .as_ref()
                        .map(|e| &e.look),
                    *pos,
                    &mut play,
                );
            }
            Event::OrdnanceExpired { pos, weapon_class } => {
                let x = walk.sim.weapon_classes[*weapon_class as usize]
                    .ordnance
                    .expire
                    .as_ref();
                boom(x.map(|x| &x.look), *pos, &mut play);
            }
            Event::StickyBlast { pos, weapon_class } => {
                let sticky = &walk.sim.weapon_classes[*weapon_class as usize]
                    .ordnance
                    .sticky;
                let x = sticky.as_ref().and_then(|s| s.explosion.as_ref());
                boom(x.map(|x| &x.look), *pos, &mut play);
            }
            Event::VehicleDestroyed { class, pos, .. } => {
                let setup = walk.sim.blast_setup.get(*class as usize);
                let x = setup.and_then(|b| b.as_ref()?.explosion.as_ref());
                boom(x.map(|x| &x.look), *pos, &mut play);
                if let Some(s) = walk
                    .vehicle_sounds
                    .get(*class as usize)
                    .and_then(|s| s.death_sound.as_deref())
                {
                    play(s, *pos);
                }
            }
            Event::FlyerImpact { vehicle, pos, .. } => {
                let sound = walk
                    .sim
                    .vehicle(*vehicle)
                    .and_then(|v| walk.vehicle_sounds.get(v.class as usize))
                    .and_then(|s| s.collision_sound.as_deref())
                    .unwrap_or("com_veh_collision_lg");
                play(sound, *pos);
            }
            Event::VehicleHit {
                vehicle, damage, ..
            } if *damage > 0.0 => {
                if let Some(v) = walk.sim.vehicle(*vehicle) {
                    if let Some(s) = walk
                        .vehicle_sounds
                        .get(v.class as usize)
                        .and_then(|s| s.hurt_sound.as_deref())
                    {
                        play(s, v.pos);
                    }
                }
            }
            // Card DROIDDEATH: the burst's explosion class authors no
            // SoundProperty on either disc; the noise heard is the droid's
            // DeathSound, started by the kill (`cries`).
            Event::DroidBurst {
                pos, team, class, ..
            } => {
                let x = walk.sim.burst_explosion(*team, *class);
                boom(x.map(|x| &x.look), *pos, &mut play);
            }
            Event::StationDestroyed { class, pos, .. } => {
                let x = walk
                    .sim
                    .station_classes
                    .get(usize::from(*class))
                    .and_then(|c| c.explosion.as_ref());
                boom(x.map(|x| &x.look), *pos, &mut play);
            }
            Event::WeaponSelect { unit, weapon_class } => {
                let pos = walk.sim.units[unit.index as usize].soldier.pos;
                equip(*weapon_class as usize, pos, &mut play);
            }
            Event::Respawn { unit } => {
                spawned(&walk.sim.units[unit.index as usize], &mut play);
            }
            // Pushed for a human only (§7.3).
            Event::FireEmpty {
                weapon_class, pos, ..
            } if handles => {
                if let Some(n) = &walk.sim.weapon_classes[*weapon_class as usize].empty_sound {
                    play(n, *pos);
                }
            }
            // The weapon that fired: a rider's shot is the seat gun's
            // `FireSound` (card VSND, re-vehicle-sounds.md 0.9), a shotgun's
            // 8 pellets are one sound ([`shot_sound`]).
            Event::Shot {
                unit,
                weapon_class,
                origin,
                ..
            } if handles || !human(walk, *unit) => {
                if let Some(n) = shot_sound(walk, &mut fired, *unit, *weapon_class) {
                    play(n, *origin);
                }
            }
            // GREN: the throw is the grenade weapon's FireSound, a bounce
            // or a stick its ordnance's CollisionOtherSound.
            Event::Throw {
                weapon_class,
                origin,
                ..
            }
            | Event::Cloak {
                weapon_class,
                pos: origin,
                ..
            }
            | Event::Ability {
                weapon_class,
                pos: origin,
                ..
            }
            | Event::Repair {
                weapon_class,
                pos: origin,
                ..
            } => {
                if let Some(n) = &walk.sim.weapon_classes[*weapon_class as usize].fire_sound {
                    play(n, *origin);
                }
            }
            // An item's `PowerupSound` where it lay, and BF1's
            // `CollisionOtherSound` where it lands (PWUP, re-powerups §5.2).
            Event::ItemTaken { item, pos, .. } => {
                let c = walk.sim.item_classes.get(usize::from(*item));
                if let Some(n) = c.and_then(|c| c.pickup_sound.as_ref()) {
                    play(n, *pos);
                }
            }
            // A station's `PowerupSound` at the station, once per unit it
            // served (PWST, re-powerup-station §5.1).
            Event::StationServed { station, pos, .. } => {
                let s = walk.sim.stations.get(usize::from(*station));
                let c = s.and_then(|s| walk.sim.station_classes.get(usize::from(s.class)));
                if let Some(n) = c.and_then(|c| c.powerup_sound.as_ref()) {
                    play(n, *pos);
                }
            }
            Event::ItemLanded { item, pos } => {
                let c = walk.sim.item_classes.get(usize::from(*item));
                if let Some(n) = c.and_then(|c| c.land_sound.as_ref()) {
                    play(n, *pos);
                }
            }
            Event::StickyContact {
                pos,
                weapon_class,
                water: false,
            } => {
                let sticky = &walk.sim.weapon_classes[*weapon_class as usize]
                    .ordnance
                    .sticky;
                if let Some(n) = sticky
                    .as_ref()
                    .and_then(|s| s.bounce_sound.as_ref())
                    .or_else(|| {
                        walk.sim.weapon_classes[*weapon_class as usize]
                            .mine
                            .as_ref()
                            .and_then(|d| d.item.contact_sound.as_ref())
                    })
                {
                    play(n, *pos);
                }
            }
            Event::Reload { unit, weapon_class } => {
                if let Some(n) = &walk.sim.weapon_classes[*weapon_class as usize].reload_sound {
                    play(n, walk.sim.units[unit.index as usize].soldier.pos);
                }
            }
            // DRV: the door's ODF sounds on its trigger's edges, at the door
            // (`re-bf2-doors.md` §2 hop 5).
            Event::DoorOpened { door }
            | Event::DoorClosed { door }
            | Event::DoorLocked { door } => {
                let Some(d) = walk.sim.doors.get(*door as usize) else {
                    continue;
                };
                let name = match e {
                    Event::DoorOpened { .. } => &d.open_sound,
                    Event::DoorClosed { .. } => &d.close_sound,
                    _ => &d.locked_sound,
                };
                if let Some(n) = name {
                    play(n, d.origin);
                }
            }
            // The player's own line (card SQACK, re-squad-orders.md section 2):
            // at the press, whether or not anyone is in reach; MOVE OUT and AT
            // EASE say the same one. The side's names, PROVISIONAL: the PS2
            // reads the issuer's CLASS (`SCField*Sound` / `SCDriverGetInSound`; the same names on
            // every class but the two smugglers'). It does not cut the line
            // before it off as the PS2's own slot does. The squad's answers
            // are the voice queue's (`announce::Commander::orders`).
            // BF1's names: BF2 has no `*_com_chatter_tac_*`, so this is
            // SILENT on BF2 maps (`SoundLib::play` is silent for a name it
            // does not hold). On BF1 maps it plays a streamed take: the
            // per-map `snd_` banks bake `samples: []` for these names and the
            // lookup falls through to the GLOBAL streams
            // (`analysis/re-sound-global.md`; `tests/global_sound.rs` pins it).
            Event::Command { slot, kind, .. } => {
                let team = walk
                    .sim
                    .players
                    .get(usize::from(*slot))
                    .copied()
                    .flatten()
                    .and_then(|h| walk.sim.unit(h))
                    .map_or(0, |u| u.team);
                let side = &walk.teams[usize::from(team).min(1)].faction;
                // The bank keys on the faction; `default_trooper`'s "team" has
                // no chatter of its own, so a map with no faction says nothing.
                if matches!(side.as_str(), "all" | "imp" | "rep" | "cis") {
                    play(&order_line_sound(side, *kind), ears);
                }
            }
            _ => {}
        }
    }

    // Incoming missile warning beep and target lock audio loop (card UORD / issue #123)
    if let Some(player_unit) = walk.sim.units.get(walk.player.index as usize) {
        if let core::unit::Controller::Player { slot } = player_unit.controller {
            let side = walk
                .teams
                .get(player_unit.team as usize)
                .map(|t| t.faction.as_str())
                .unwrap_or("");
            let slot_idx = usize::from(slot);
            if let Some(dist) = walk.sim.incoming_missile(slot_idx) {
                // Incoming missile warning beep: period accelerates as the missile closes in
                let interval = if dist < 100.0 {
                    4
                } else if dist < 250.0 {
                    8
                } else {
                    15
                };
                if walk.sim.tick.is_multiple_of(interval) {
                    let prop = if matches!(side, "all" | "imp" | "rep" | "cis") {
                        format!("{side}_weap_inf_rocket_launcher_tracking")
                    } else {
                        "com_weap_inf_rocket_launcher_tracking".to_owned()
                    };
                    play(&prop, ears);
                }
            }
        }
    }
}

/// The hurt cry's share (card UCRY): a hit asks it only when it took more
/// than this share of the health it LEFT (0x3ca3d70a at B1 0x1bc498, B2
/// 0x1e0010; `analysis/re-unit-chatter.md` 2.7, 3.6).
pub const HURT_SHARE: f32 = 0.02;

/// Whether a hit asks the victim's `HurtSound` (B1 `FUN_001bc190`
/// 64384-64436, B2 `FUN_001dfb38` 91408-91449): it left the unit alive and
/// took more than [`HURT_SHARE`] of the health left. A hit the shield
/// swallowed or one of no health damage is "tiny" by the same test. BF2's
/// extra gate, damage-flag bit 2, is taken as set for every hit the port
/// makes ([H]: what sets it is not read; the PS2 cries on bolt hits live,
/// section 5). A hit with no attacker pushes no `Event::Hit` (BF1 would
/// cry, BF2 asks the silent `HurtFallSound`).
pub fn hurts(damage: f32, health_left: f32) -> bool {
    health_left > 0.0 && damage > health_left * HURT_SHARE
}

/// One cry a tick's events ask (card UCRY): unit slot `unit`'s voice is cut,
/// then `name` (None: its class has no such key) is asked at `pos`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cry<'a> {
    pub unit: u32,
    pub name: Option<&'a str>,
    pub pos: Vec3,
    pub death: bool,
}

/// The hurt and death cries this tick's events ask, in event order (card
/// UCRY): a surviving hit that [`hurts`] asks the victim's `HurtSound`, a
/// kill (`Event::Kill`, `Event::EnvDeath`: the PS2's kill routine asks
/// `DeathSound` of any living unit it kills, B1 `FUN_001bbc98`) its
/// `DeathSound`; each at the unit, on its one voice (`SoundLib::cry`).
pub fn cries<'a>(walk: &'a WalkSim, mut cry: impl FnMut(Cry<'a>)) {
    let mut ask = |h: core::unit::UnitHandle, death: bool| {
        let Some(u) = walk.sim.units.get(h.index as usize) else {
            return;
        };
        let c = &walk.team_template(u.team).class(u.spec.class).cries;
        let name = if death { &c.death } else { &c.hurt };
        cry(Cry {
            unit: h.index,
            name: name.as_deref(),
            pos: u.soldier.pos,
            death,
        });
    };
    for e in &walk.sim.events {
        match *e {
            Event::Hit {
                victim,
                damage,
                health_left,
                ..
            } if hurts(damage, health_left) => ask(victim, false),
            Event::Kill { victim, .. } => ask(victim, true),
            Event::EnvDeath { unit, .. } => ask(unit, true),
            _ => {}
        }
    }
}

pub fn station_asks(walk: &WalkSim) -> Vec<(u16, u32, Vec3)> {
    let sim = &walk.sim;
    sim.stations
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            if s.destroyed {
                return None;
            }
            let c = sim.station_classes.get(usize::from(s.class))?;
            let name = c.ambient_sound.as_deref()?;
            let i = u16::try_from(i).ok()?;
            Some((i, crate::classdb::fnv1a_lowercase(name), s.pos))
        })
        .collect()
}

/// Whether a shot starts its weapon's `FireSound` (card SNDX; B1
/// `FUN_00288d10` 211389 / 211421, B2 `FUN_0032e8c0` 323541 / 323581): the
/// trigger pull starts one, and a later shot of the pull another only when the
/// class's `SalvoDelay` (BF2: its lower bound) is over 0.
pub fn fire_sound_starts(first_of_pull: bool, wc: &core::weapon::WeaponClass) -> bool {
    first_of_pull || wc.salvo_delay[0] > 0.0
}

/// The sound property of the player's own line for an order (`kind` is
/// `Event::Command`'s byte): 2-D, on the tac stream.
pub fn order_line_sound(side: &str, kind: u8) -> String {
    let suffix = match kind {
        0 => "follow",
        1 => "hold",
        4 => "needgunner",
        _ => "moveout",
    };
    format!("{side}_inf_com_chatter_tac_{suffix}")
}

/// Dev referee (cards SNDC, FOLY): every sound a headless replay's ticks
/// would start, as CSV rows `tick,src,name,unit,dist` - `src` is `event`
/// ([`each`]), `effect` (an explosion's effect's `SoundName`) or `foley`
/// (`crate::foley`), or `start` for each take the library starts for an
/// `event` / `effect` row (card WSND: in range, past the
/// property's gates, a layered record's layers on the ticks their delays
/// end; `SoundLib::start` / `tick`) with a `take` row after it, `name`
/// `<file under the baked root>|<gain>|<pitch>|<left>|<right>` (card SNDX:
/// what the mixer is handed; card MIXH: what each ear gets, the bus and
/// the PS2's per-ear split in, `Start::ears`), `name` the property's name where
/// the map's `sounds/names.ron` has it (else its hash), `unit` the foley
/// sound's unit (-1 for an event), `dist` metres from the listener.
/// Card JETP2: a `jet` row per jet one-shot (`name|TurnOn` / `TurnOff`,
/// `unit` the soldier) and the jets' loops among the `engine` rows.
/// Card VSND, once a second: an `engine` row per playing engine layer -
/// `name` is `<record>/<layer>/<gain factor>/<pitch factor>/<speed sent>`,
/// `unit` the vehicle - and one `engines` row, `name` `<pilot seats occupied>/<engines
/// on>/<of them running down>`.
/// `SOUND_LOG=<csv>` on `game --headless --walk-replay` writes it;
/// `SOUND_LOG_WHO=reach` makes every soldier in earshot step.
pub struct SoundLog {
    csv: String,
    names: std::collections::HashMap<u32, String>,
    foley: Option<crate::foley::Foley>,
    lib: Option<crate::sounds::SoundLib>,
    engines: crate::engine_sound::EngineSounds,
    /// The world's emitters (card AMBS): listed at tick 0 and once a second
    /// with their level at the listener (`ambience` rows).
    pub ambience: crate::ambience::Ambience,
    /// The baked root (the map dir's parent): `take` rows name files under it.
    root: std::path::PathBuf,
}

/// The living hero's own low-health voice and the saber weapon's ambient.
pub(crate) fn saber_visible(u: &core::unit::Unit, classes: &[core::weapon::WeaponClass]) -> bool {
    let Some(saber) = classes.get(u.weapon_class).and_then(|c| c.saber.as_ref()) else {
        return false;
    };
    let hidden = u.weapon.melee.as_ref().map_or(0, |s| s.hidden_edges);
    u.alive
        && u.mount.is_none()
        && saber.blades.iter().enumerate().any(|(edge, blade)| {
            core::hero_weapon::blade_visible(hidden, edge) && !blade.texture.is_empty()
        })
}

/// Whether `u` gets its class's hero sounds: the team's hero class, or any
/// unit in Hero Assault, where every class is a hero.
pub(crate) fn sounds_as_hero(sim: &core::sim::Sim, u: &core::unit::Unit) -> bool {
    sim.deathmatch.is_some() || sim.hero_class(u.team) == Some(u.spec.class)
}

pub fn hero_sample_asks(walk: &WalkSim) -> Vec<(core::unit::UnitHandle, String, Vec3)> {
    let mut asks = Vec::new();
    for (i, u) in walk.sim.units.iter().enumerate() {
        let handle = core::unit::UnitHandle {
            index: i as u32,
            gen: u.gen,
        };
        if !sounds_as_hero(&walk.sim, u) {
            continue;
        }
        let class = walk.team_template(u.team).class(u.spec.class);
        if u.alive && u.health.cur <= u.health.max * class.cries.low_threshold {
            if let Some(name) = &class.cries.low_health {
                asks.push((handle, name.clone(), u.soldier.pos));
            }
        }
        if saber_visible(u, &walk.sim.weapon_classes) {
            if let Some(name) = &class.cries.hero.saber_ambient {
                asks.push((handle, name.clone(), u.soldier.pos));
            }
        }
        if let Some(name) = &class.cries.hero.saber_flight {
            if let Some(flight) = std::iter::once(&u.weapon)
                .chain(u.held.iter().map(|w| &w.state))
                .find_map(|state| state.saber_flight.as_ref())
            {
                asks.push((handle, name.clone(), flight.pos));
            }
        }
    }
    asks
}

/// Names for the hashes the discs hold, including every authored combo sound.
pub fn sound_names(
    map_dir: &std::path::Path,
    walk: &WalkSim,
) -> std::collections::HashMap<u32, String> {
    // `(hash, "name", source),` lines; the type is the bake's.
    let text = pak::read_to_string(map_dir.join("sounds").join("names.ron")).unwrap_or_default();
    let mut names = crate::foley::names();
    for weapon in &walk.sim.weapon_classes {
        if let Some(saber) = &weapon.saber {
            let mut pending: Vec<_> = saber.combo.fields.iter().collect();
            while let Some(field) = pending.pop() {
                if field.keyword == manifest::combo::ComboKeyword::Sound {
                    if let Some(name) = field.text(0) {
                        names
                            .entry(crate::classdb::fnv1a_lowercase(name))
                            .or_insert_with(|| name.to_owned());
                    }
                }
                pending.extend(&field.children);
            }
            for name in [&saber.hit_sound, &saber.deflect_sound]
                .into_iter()
                .flatten()
            {
                names
                    .entry(crate::classdb::fnv1a_lowercase(name))
                    .or_insert_with(|| name.clone());
            }
        }
        for name in [
            &weapon.fire_sound,
            &weapon.reload_sound,
            &weapon.change_sound,
            &weapon.empty_sound,
        ]
        .into_iter()
        .flatten()
        {
            names
                .entry(crate::classdb::fnv1a_lowercase(name))
                .or_insert_with(|| name.clone());
        }
    }
    names.extend(text.lines().filter_map(|l| {
        let mut f = l.trim().trim_start_matches('(').splitn(3, ',');
        let hash = f.next()?.trim().parse().ok()?;
        Some((hash, f.next()?.trim().trim_matches('"').to_owned()))
    }));
    // The engine records are named by the classes alone.
    for e in walk.vehicle_engines.iter().flatten() {
        let hash = crate::classdb::fnv1a_lowercase(&e.record);
        names.entry(hash).or_insert_with(|| e.record.clone());
    }
    // And the jet classes' (card JETP2).
    for t in &walk.teams {
        for c in &t.classes {
            for n in [
                &c.cries.low_health,
                &c.cries.hero.offer,
                &c.cries.hero.spawned,
                &c.cries.hero.defeated,
                &c.cries.hero.saber_ambient,
                &c.cries.hero.saber_on,
                &c.cries.hero.saber_off,
                &c.cries.hero.saber_flight,
            ]
            .into_iter()
            .flatten()
            {
                names
                    .entry(crate::classdb::fnv1a_lowercase(n))
                    .or_insert_with(|| n.clone());
            }
            let j = &c.jet_look;
            for n in [&j.engine, &j.turn_on, &j.turning_off, &j.turn_off]
                .into_iter()
                .flatten()
            {
                names
                    .entry(crate::classdb::fnv1a_lowercase(n))
                    .or_insert_with(|| n.clone());
            }
        }
    }
    // So are the stations' (card PWST).
    for c in &walk.sim.station_classes {
        for n in [&c.powerup_sound, &c.ambient_sound].into_iter().flatten() {
            names
                .entry(crate::classdb::fnv1a_lowercase(n))
                .or_insert_with(|| n.clone());
        }
    }
    names
}

impl SoundLog {
    pub fn new(map_dir: &std::path::Path, walk: &WalkSim) -> SoundLog {
        let names = sound_names(map_dir, walk);
        let mut foley = crate::foley::Foley::load(map_dir, walk);
        if std::env::var("SOUND_LOG_WHO").as_deref() == Ok("reach") {
            if let Some(f) = foley.as_mut() {
                f.who = crate::foley::Who::InReach;
            }
        }
        let mut lib = crate::sounds::SoundLib::load(map_dir);
        if let Some(sl) = lib.as_mut() {
            sl.for_walk(map_dir, walk);
        }
        SoundLog {
            csv: String::from("tick,src,name,unit,dist\n"),
            names,
            foley,
            lib,
            engines: crate::engine_sound::EngineSounds::new(map_dir, walk),
            ambience: Default::default(),
            root: map_dir.parent().unwrap_or(map_dir).to_path_buf(),
        }
    }

    /// The tick the sim just stepped; `effects` are the sounds its
    /// explosions' effects carry (`Fx::take_sounds`).
    pub fn tick(&mut self, tick: usize, walk: &WalkSim, effects: &[(String, Vec3)]) {
        use std::fmt::Write as _;
        let heard = listener(walk);
        let ears = heard.pos;
        let csv = &mut self.csv;
        let mut starts = Vec::new();
        let lib = &mut self.lib;
        let mut row = |src: &str, name: &str, pos: Vec3| {
            let d = (pos - ears).length();
            let _ = writeln!(csv, "{tick},{src},{name},-1,{d:.1}");
            if let Some(lib) = lib.as_mut() {
                let hash = crate::classdb::fnv1a_lowercase(name);
                lib.start(hash, pos, ears, &mut starts);
            }
        };
        each(walk, ears, |name, pos| row("event", name, pos));
        for (name, pos) in effects {
            row("effect", name, *pos);
        }
        // Card UCRY: a `hurt` / `death` row per cry asked (`name` `-` for a
        // class without the key, `unit` the slot); what starts is the tick's
        // last ask per unit (`SoundLib::cries`), in the `start` rows.
        let mut asks = Vec::new();
        cries(walk, |c| {
            let d = (c.pos - ears).length();
            let src = if c.death { "death" } else { "hurt" };
            let name = c.name.unwrap_or("-");
            let _ = writeln!(csv, "{tick},{src},{name},{},{d:.1}", c.unit);
            let hash = c.name.map_or(0, crate::classdb::fnv1a_lowercase);
            asks.push((c.unit, hash, c.pos));
        });
        if let Some(lib) = lib.as_mut() {
            starts.extend(lib.cries(&asks, ears).into_iter().map(|(_, s)| s));
        }
        // Card PWST: the stations' ambient asks start in the `start` rows.
        if let Some(lib) = lib.as_mut() {
            starts.extend(lib.station_ambient(&station_asks(walk), ears));
            for (name, pos) in lib.hero_blade_changes(walk) {
                lib.start(
                    crate::classdb::fnv1a_lowercase(&name),
                    pos,
                    ears,
                    &mut starts,
                );
            }
            starts.extend(
                lib.hero_samples(&hero_sample_asks(walk), ears)
                    .into_iter()
                    .map(|(_, s)| s),
            );
        }
        if let Some(lib) = lib.as_mut() {
            lib.tick(core::TICK_DT, ears, &mut starts);
        }
        for s in &starts {
            let d = (s.pos - ears).length();
            let name = self
                .names
                .get(&s.hash)
                .cloned()
                .unwrap_or_else(|| format!("{:08x}", s.hash));
            let _ = writeln!(csv, "{tick},start,{name},-1,{d:.1}");
            // What the mixer is handed (card SNDX): a mix can be rendered
            // from these rows and its level measured.
            let file = s.path.strip_prefix(&self.root).unwrap_or(&s.path);
            let (l, r) = s.ears(&heard);
            let _ = writeln!(
                csv,
                "{tick},take,{}|{:.4}|{:.4}|{l:.4}|{r:.4},-1,{d:.1}",
                file.display(),
                s.gain,
                s.pitch
            );
        }
        if let Some(lib) = self.lib.as_ref() {
            // The world's emitters the listener hears: `sound/kind/level/
            // gain` (the level by distance with the emitter's distances, the
            // gain with the property's and the bus's).
            if tick.is_multiple_of(core::TICK_HZ as usize) {
                for h in lib.ambient_heard(&self.ambience, ears) {
                    let e = &self.ambience.emitters[h.index];
                    if h.level > 0.0 {
                        let _ = writeln!(
                            csv,
                            "{tick},ambience,{}/{}/{:.4}/{:.4},{},{:.1}",
                            e.sound,
                            e.kind.word(),
                            h.level,
                            h.gain,
                            h.index,
                            h.dist
                        );
                    }
                }
            }
            let mut voices = Vec::new();
            let n = self
                .engines
                .tick(walk, |h, l| lib.engine(h, l), &mut voices);
            // Card JETP2: a `jet` row per jet one-shot, `name|TurnOn` /
            // `TurningOff` / `TurnOff`, `unit` the soldier.
            for j in self.engines.take_jet_shots() {
                let d = (j.pos - ears).length();
                let _ = writeln!(csv, "{tick},jet,{}|{:?},{},{d:.1}", j.name, j.kind, j.unit);
            }
            if tick.is_multiple_of(core::TICK_HZ as usize) {
                let name = |h: u32| {
                    let n = self.names.get(&h).cloned();
                    n.unwrap_or_else(|| format!("{h:08x}"))
                };
                for v in &voices {
                    let d = (v.pos - ears).length();
                    let _ = writeln!(
                        csv,
                        "{tick},engine,{}/{}/{:.3}/{:.3}/{:.3},{},{d:.1}",
                        name(v.record),
                        v.layer,
                        v.gain_factor,
                        v.pitch_factor,
                        v.speed,
                        v.vehicle
                    );
                }
                let _ = writeln!(
                    csv,
                    "{tick},engines,{}/{}/{},-1,0.0",
                    n.piloted, n.on, n.turning_off
                );
            }
        }
        if let Some(foley) = self.foley.as_mut() {
            let view = crate::foley::View::of(&walk.camera(), 16.0 / 9.0);
            let names = &self.names;
            foley.tick(walk, &view, |s| {
                let d = (s.pos - ears).length();
                let name = names
                    .get(&s.hash)
                    .cloned()
                    .unwrap_or_else(|| format!("{:08x}", s.hash));
                let _ = writeln!(csv, "{tick},foley,{name},{},{d:.1}", s.unit);
            });
        }
    }

    /// Card UCALL: the battle's voice after the tick - the units' combat
    /// calls through the bank's gates (`Commander::gate_calls`), a `call`
    /// row each (`name|call|outcome`: `queued`, or why not - `take_locked`,
    /// `interval`, `probability`, `far`, `unknown`; `name` `-` for a class
    /// without the key; card UTALK's squad talk alike, a `LowHealth` ask
    /// only when queued; card UVO2's BF2 lines as `Vo(<event index>)`, the
    /// index in `crate::vo::EVENTS`), then the voice queue's frame: a `voice` row per
    /// line started (commander lines, squad answers and calls alike).
    pub fn voice(&mut self, tick: usize, walk: &mut WalkSim) {
        use std::fmt::Write as _;
        let ears = listener(walk).pos;
        let (Some(lib), Some(v)) = (self.lib.as_mut(), walk.voice.as_mut()) else {
            return;
        };
        for (a, r) in v.gate_calls(lib, ears) {
            // A low-health voice asks every tick its handle is idle (card
            // UTALK): only the lines it queues are rows.
            let every_tick = matches!(
                a.call,
                core::sim::Call::LowHealth
                    | core::sim::Call::Cockpit
                    | core::sim::Call::Loudspeaker
            );
            if every_tick && r.is_err() {
                continue;
            }
            let outcome = match r {
                Ok(_) => "queued",
                Err(crate::sounds::Refused::TakeLocked) => "take_locked",
                Err(crate::sounds::Refused::Interval) => "interval",
                Err(crate::sounds::Refused::Probability) => "probability",
                Err(crate::sounds::Refused::Far) => "far",
                Err(crate::sounds::Refused::Unknown) => "unknown",
            };
            let d = (a.pos - ears).length();
            let _ = writeln!(
                self.csv,
                "{tick},call,{}|{:?}|{outcome},{},{d:.1}",
                a.name.as_deref().unwrap_or("-"),
                a.call,
                a.unit
            );
        }
        let started = v.frame(&walk.sim, walk.player, core::TICK_DT, |l| lib.line_secs(l));
        if let Some(l) = started {
            let d = l.at.map_or(0.0, |p| (p - ears).length());
            let _ = writeln!(self.csv, "{tick},voice,{},-1,{d:.1}", l.name);
        }
    }

    pub fn csv(&self) -> &str {
        &self.csv
    }
}

#[cfg(test)]
mod tick_tests {
    use super::*;
    use crate::bolt_vis::BoltVis;
    use crate::fx::Fx;
    use core::sim::{buttons, PadState};

    /// What a run started, and what its ticks held.
    #[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
    struct Count {
        ticks: u32,
        /// `Shot` + `Reload` + struck `BoltExpired` over the ticks stepped.
        events: usize,
        /// Struck `BoltExpired` alone (one placeholder puff and one bolt
        /// remnant each).
        struck: usize,
        sounds: usize,
        effects: usize,
        remnants: usize,
    }

    /// How a run reads the events.
    #[derive(Clone, Copy)]
    enum Read {
        /// After each stepped tick (`advance_with`'s hook).
        PerTick,
        /// Once per render frame after `advance` — the windowed loop before
        /// card EVTK.
        PerFrame,
    }

    /// A soldier holding FIRE at the flat ground under his feet (each bolt
    /// spawns and strikes inside one tick), driven for `secs` seconds by
    /// render frames of `dt`.
    fn run(dt: f32, secs: f32, read: Read) -> Count {
        let flat = core::heightfield::Heightfield {
            size: 33,
            unit: 8.0,
            offset_x: 128.0,
            offset_z: 128.0,
            heights: vec![0.0; 33 * 33],
        };
        let mut walk = WalkSim::new(42, Vec3::ZERO, Some(flat));
        let down = PadState {
            ry: -32767,
            ..PadState::default()
        };
        for _ in 0..40 {
            walk.step_tick(&[down]);
        }
        let fire = [PadState {
            buttons: buttons::FIRE,
            ..PadState::default()
        }];
        let mut c = Count::default();
        // No effect data: a struck bolt is one placeholder puff. Never
        // `update`d, so every puff and remnant started is still counted.
        let mut fx = Fx::default();
        let mut bolts = BoltVis::default();
        let tick = |walk: &WalkSim, c: &mut Count| {
            c.ticks += 1;
            for e in &walk.sim.events {
                let struck = matches!(e, Event::BoltExpired { struck: true, .. });
                c.struck += usize::from(struck);
                c.events +=
                    usize::from(struck || matches!(e, Event::Shot { .. } | Event::Reload { .. }));
            }
        };
        let mut present = |walk: &WalkSim, c: &mut Count| {
            fx.ingest(walk);
            bolts.track(walk);
            // The spawn's equip sounds (the first tick) are not events.
            each(walk, listener(walk).pos, |n, _| {
                c.sounds += usize::from(!n.ends_with("_equip_med"));
            });
            c.effects = fx.len();
            c.remnants = bolts.remnant_count();
        };
        for _ in 0..(secs / dt).round() as u32 {
            match read {
                Read::PerTick => {
                    walk.advance_with(dt, &fire, |w| {
                        tick(w, &mut c);
                        present(w, &mut c);
                    });
                }
                Read::PerFrame => {
                    walk.advance_with(dt, &fire, |w| tick(w, &mut c));
                    present(&walk, &mut c);
                }
            }
        }
        c
    }

    /// Card WSND: an explosion's class sound once per explosion (an impact
    /// of an ordnance whose explosion names one), the selected weapon's
    /// change sound, the empty click.
    #[test]
    fn explosions_weapon_changes_and_empty_clicks_start_their_sounds() {
        use core::impact::ExplosionLook;
        use core::weapon::WeaponClass;
        let mut walk = WalkSim::new(42, Vec3::ZERO, None);
        let mut w = WeaponClass::bf1_rifle();
        w.change_sound = Some("equip".into());
        w.ordnance.collision_sound = None;
        w.ordnance.impact.as_mut().unwrap().explosion = Some(ExplosionLook {
            sound: Some("a_exp".into()),
            ..ExplosionLook::default()
        });
        let loud = walk.sim.add_weapon_class(w) as u32;
        let mut w = WeaponClass::bf1_rifle();
        w.change_sound = None;
        w.ordnance.collision_sound = None;
        w.ordnance.impact.as_mut().unwrap().explosion = Some(ExplosionLook::default());
        let quiet = walk.sim.add_weapon_class(w) as u32;
        let at = Vec3::new(3.0, 0.0, 4.0);
        let impact = |weapon_class| Event::Impact {
            pos: at,
            dir: Vec3::NEG_Z,
            weapon_class,
            surface: core::impact::Surface::Terrain,
            vehicle: None,
        };
        let unit = walk.player;
        walk.sim.events = vec![
            impact(loud),
            impact(quiet),
            impact(loud),
            Event::WeaponSelect {
                unit,
                weapon_class: loud,
            },
            Event::WeaponSelect {
                unit,
                weapon_class: quiet,
            },
            Event::FireEmpty {
                unit,
                weapon_class: walk.sim.units[unit.index as usize].weapon_class as u32,
                pos: walk.sim.units[unit.index as usize].soldier.pos,
            },
        ];
        let mut got = Vec::new();
        each(&walk, Vec3::ZERO, |n, pos| got.push((n.to_owned(), pos)));
        let me = walk.sim.units[unit.index as usize].soldier.pos;
        let want = [
            ("a_exp", at),
            ("a_exp", at),
            ("equip", me),
            ("com_weap_inf_ammo_empty", me),
        ];
        assert_eq!(got.len(), want.len(), "{got:?}");
        for (g, w) in got.iter().zip(want) {
            assert_eq!((g.0.as_str(), g.1), w);
        }
    }

    /// The empty click is the take of the weapon that refused, at the place
    /// the event names, on the handle a human's shots share: a class with no
    /// `FireEmptySound` is silent, and a bot's shot stays off the handles.
    #[test]
    fn a_click_asks_its_weapons_own_take_on_the_shooters_handle() {
        use crate::sounds::WeaponAsk;
        use core::weapon::WeaponClass;
        let mut walk = WalkSim::new(42, Vec3::ZERO, None);
        let class = |walk: &mut WalkSim, empty: Option<&str>, fire: &str| {
            let mut w = WeaponClass::bf1_rifle();
            w.empty_sound = empty.map(str::to_owned);
            w.fire_sound = Some(fire.into());
            walk.sim.add_weapon_class(w) as u32
        };
        let ammo = class(&mut walk, Some("com_weap_inf_ammo_empty"), "a_fire");
        let weapon = class(&mut walk, Some("com_weap_inf_weapon_empty"), "b_fire");
        let silent = class(&mut walk, None, "c_fire");
        let unit = walk.player;
        let at = Vec3::new(1.0, 2.0, 3.0);
        let click = |weapon_class| Event::FireEmpty {
            unit,
            weapon_class,
            pos: at,
        };
        let shot = |unit, weapon_class| Event::Shot {
            unit,
            weapon_class,
            origin: at,
            dir: Vec3::NEG_Z,
        };
        walk.sim.events = vec![
            click(ammo),
            click(weapon),
            click(silent),
            shot(unit, silent),
        ];
        let mut asks = Vec::new();
        weapon_asks(&walk, |a| asks.push(a));
        let h = crate::classdb::fnv1a_lowercase;
        let ask = |weapon_class, hash, click| WeaponAsk {
            unit: unit.index,
            weapon_class,
            hash,
            pos: at,
            click,
        };
        assert_eq!(
            asks,
            [
                ask(ammo, h("com_weap_inf_ammo_empty"), true),
                ask(weapon, h("com_weap_inf_weapon_empty"), true),
                ask(silent, h("c_fire"), false),
            ]
        );
        let mut off = Vec::new();
        each_off_handle(&walk, Vec3::ZERO, |n, _| off.push(n.to_owned()));
        assert!(off.is_empty(), "{off:?}");
        let mut all = Vec::new();
        each(&walk, Vec3::ZERO, |n, pos| all.push((n.to_owned(), pos)));
        assert_eq!(
            all,
            [
                ("com_weap_inf_ammo_empty".to_owned(), at),
                ("com_weap_inf_weapon_empty".to_owned(), at),
                ("c_fire".to_owned(), at),
            ]
        );

        walk.sim.units[unit.index as usize].is_player = false;
        walk.sim.events = vec![shot(unit, silent)];
        let mut asks = Vec::new();
        weapon_asks(&walk, |a| asks.push(a));
        assert!(asks.is_empty(), "{asks:?}");
        let mut off = Vec::new();
        each_off_handle(&walk, Vec3::ZERO, |n, _| off.push(n.to_owned()));
        assert_eq!(off, ["c_fire"]);
    }

    /// Card VSND: a shot plays the `FireSound` of the weapon that fired -
    /// a rider's seat gun, not the rider's own rifle.
    #[test]
    fn a_shot_plays_the_firing_weapons_sound() {
        use core::weapon::WeaponClass;
        let mut walk = WalkSim::new(42, Vec3::ZERO, None);
        let mut w = WeaponClass::bf1_rifle();
        w.fire_sound = Some("cis_weap_hover_aat_cannon_fire".into());
        let cannon = walk.sim.add_weapon_class(w) as u32;
        let unit = walk.player;
        let own = walk.sim.units[unit.index as usize].weapon_class;
        let own_sound = walk.sim.weapon_classes[own].fire_sound.clone().unwrap();
        assert_ne!(own_sound, "cis_weap_hover_aat_cannon_fire");
        let at = Vec3::new(1.0, 2.0, 3.0);
        let shot = |weapon_class| Event::Shot {
            unit,
            weapon_class,
            origin: at,
            dir: Vec3::NEG_Z,
        };
        walk.sim.tick = 2;
        walk.sim.events = vec![shot(cannon), shot(own as u32)];
        let mut got = Vec::new();
        each(&walk, Vec3::ZERO, |n, pos| got.push((n.to_owned(), pos)));
        assert_eq!(
            got,
            [
                ("cis_weap_hover_aat_cannon_fire".to_owned(), at),
                (own_sound, at)
            ]
        );
    }

    /// Card SNDX: the fire sound is the trigger pull's, not each bolt's. A
    /// shotgun (8 shots in a salvo, no `SalvoDelay`) held on FIRE through
    /// the real fire cycle starts one sound per 8 bolts; a class with a
    /// `SalvoDelay` starts one per shot, as the PS2's cycle does.
    #[test]
    fn a_salvo_of_bolts_is_one_fire_sound() {
        use core::weapon::WeaponClass;
        let run = |salvo_delay: f32| {
            let mut walk = WalkSim::new(42, Vec3::ZERO, None);
            let mut w = WeaponClass::bf1_rifle();
            w.shots_per_salvo = 8;
            w.salvo_delay = [salvo_delay; 2];
            w.fire_sound = Some("rep_weap_inf_shotgun_fire".into());
            let shotgun = walk.sim.add_weapon_class(w);
            let me = walk.player.index as usize;
            walk.sim.units[me].weapon_class = shotgun;
            let fire = [PadState {
                buttons: buttons::FIRE,
                ..PadState::default()
            }];
            let (mut shots, mut pulls, mut sounds) = (0, 0, 0);
            for _ in 0..120 {
                walk.step_tick(&fire);
                let n = walk.sim.events.iter();
                let n = n.filter(|e| matches!(e, Event::Shot { .. })).count();
                shots += n;
                pulls += usize::from(n > 0);
                each(&walk, Vec3::ZERO, |n, _| {
                    sounds += usize::from(n == "rep_weap_inf_shotgun_fire");
                });
            }
            (shots, pulls, sounds)
        };
        let (shots, pulls, sounds) = run(0.0);
        assert!(pulls >= 3, "the cycle ran: {pulls} pulls");
        // 8 bolts a pull; a clip's last pull is a short one.
        assert!(shots > 6 * pulls, "{shots} bolts in {pulls} pulls");
        assert_eq!(sounds, pulls, "one fire sound a pull");
        let (shots, _, sounds) = run(0.01);
        assert!(shots >= 24);
        assert_eq!(sounds, shots, "with a SalvoDelay each shot sounds");
    }

    /// Card EVTK: read in the tick hook, K events start K sounds, K effects
    /// and K remnants at any frame rate; read once per render frame they
    /// started twice at 60 Hz and went missing at 15 Hz.
    #[test]
    fn a_ticks_events_start_their_sounds_and_effects_once() {
        for dt in [1.0 / 144.0, 1.0 / 60.0, 1.0 / 30.0, 1.0 / 15.0] {
            let c = run(dt, 8.0, Read::PerTick);
            assert!(c.struck > 20 && c.events > 2 * c.struck - 2, "{dt}: {c:?}");
            assert_eq!(
                (c.sounds, c.effects, c.remnants),
                (c.events, c.struck, c.struck),
                "{dt}: {c:?}"
            );
        }
        // The frame-rate dependence the hook removes.
        let c = run(1.0 / 60.0, 8.0, Read::PerFrame);
        assert_eq!(
            (c.sounds, c.effects, c.remnants),
            (2 * c.events, 2 * c.struck, 2 * c.struck),
            "60 Hz, read per frame: {c:?}"
        );
        let c = run(1.0 / 15.0, 8.0, Read::PerFrame);
        assert!(
            c.sounds < c.events && c.effects < c.struck && c.remnants < c.struck,
            "15 Hz, read per frame: {c:?}"
        );
    }
}

#[cfg(test)]
mod tests;
