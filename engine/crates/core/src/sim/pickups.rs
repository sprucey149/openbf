//! Pickups (card PWUP, `analysis/re-powerups.md`): what a dead soldier drops,
//! the item on the ground and who takes it. Step 5c of the tick.
//!
//! - The drop (§3.2, §3.4): a class with a drop table spends ONE draw of the
//!   game's LCG at its death; the table is walked subtracting each
//!   probability and the first entry that takes it below zero is the item —
//!   unless the pool is full. It starts 1.0 m over the feet, upright, facing
//!   the soldier's heading, with the soldier's velocity (BF1), or (BF2) with
//!   it when it is faster than 3 m/s, else 3 m/s to the soldier's left.
//! - The item (§4): gravity -9.8, lands on the first floor it reaches
//!   (`Rebound` 0, `Friction` 1), life `LifeSpan` -> -1 s, then gone.
//! - The pickup (§5.2): any living soldier or droid of EITHER team whose
//!   body touches the item's box, once it has landed or is 0.5 s old; health
//!   to the cap, clips to every weapon's cap, BF2 energy and the offense /
//!   defense timers; taken only when it gave something.
//!
//! PROVISIONAL (ours): the port's death is immediate, so the drop is too (the
//! PS2 waits out a thrown or tumbling body, §3.3); the item lands by a floor
//! ray, with no wall contact; the touch is tested every tick for every unit
//! (the PS2 needs one of the two to move, §5.7); a weapon's reserve cap is
//! the reserve it spawned with. Supply dispensers cannot be refilled (§5.4).

use super::state::{DeathCause, Event, Sim};
use crate::fmath;
use crate::fmath::FloatExt;
use crate::soldier::GroundRules;
use crate::unit::{Unit, UnitHandle, UnitSpec};
use crate::weapon::{HealthType, ShotSpec, WeaponClass, WeaponState, SPARE_INFINITE};
use glam::Vec3;

/// The slots of a soldier class's drop table (both discs, §3.1).
pub const MAX_DROPS: usize = 8;
/// The `PowerupItem` pool's default capacity (both games, §2.4).
pub const ITEM_POOL_DEFAULT: usize = 25;
/// How far over the feet an item starts (§3.4).
pub const DROP_HEIGHT: f32 = 1.0;
/// BF2: a victim slower than this sends its item to its left at this speed
/// (m/s; the PS2 compares the squared speed with 9.0, §3.4).
pub const DROP_SIDE_SPEED: f32 = 3.0;
/// An item's gravity (m/s2 x its class's `Gravity` 1.0, §3.4).
pub const ITEM_GRAVITY: f32 = -9.8;
/// An item in flight is taken only once it is older than this (s, §5.2).
pub const TAKE_AGE: f32 = 0.5;
/// The item is removed when its life reaches this (s, §4).
pub const LIFE_END: f32 = -1.0;
/// The taker's body: a soldier's collision sphere (`re-soldier-walls.md`),
/// its radius and how far ahead of the feet its centre is (§5.7).
pub const BODY_RADIUS: f32 = 0.57;
pub const BODY_AHEAD: f32 = 0.15;
/// The height of the body the box is tested against (m; PROVISIONAL, the
/// standing soldier's).
const BODY_HEIGHT: f32 = 1.8;

/// One `powerupitem` class (§2.1), built by `game::roster` from the disc.
/// SETUP data like `Sim::weapon_classes`: not hashed.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemClass {
    pub name: String,
    /// `SoldierHealth`: points given to a person or a droid, up to its cap.
    pub health: f32,
    /// `SoldierAmmo`: clips added to every weapon's reserve.
    pub ammo: u32,
    /// BF2 `SoldierEnergy`.
    pub energy: f32,
    /// BF2 `BuffOffenseTimer` (s) and `BuffOffenseMult`.
    pub offense: (f32, f32),
    /// BF2 `BuffDefenseTimer` (s) and `BuffDefenseMult`.
    pub defense: (f32, f32),
    /// `LifeSpan` (s; 30 on every disc item).
    pub life_span: f32,
    /// The collision box, from the model's bounds (§2.3): half extents and
    /// centre in the item's frame.
    pub half: Vec3,
    pub centre: Vec3,
    /// What the game layer draws and plays (the sim reads none of it):
    /// `GeometryName`, `PowerupSound` (at the item when it is taken) and
    /// BF1's `CollisionOtherSound` (when it lands).
    pub geometry: String,
    pub pickup_sound: Option<String>,
    pub land_sound: Option<String>,
}

impl ItemClass {
    /// An item that gives `health` and `ammo`, with the smallest disc box.
    pub fn basic(name: &str, health: f32, ammo: u32) -> Self {
        ItemClass {
            name: name.to_owned(),
            health,
            ammo,
            energy: 0.0,
            offense: (0.0, 1.0),
            defense: (0.0, 1.0),
            life_span: 30.0,
            half: Vec3::new(0.0965, 0.2381, 0.0965),
            centre: Vec3::new(0.0, 0.237, 0.0),
            geometry: String::new(),
            pickup_sound: None,
            land_sound: None,
        }
    }
}

/// A soldier class's drop table (§3.1): up to [`MAX_DROPS`] (item class,
/// probability) pairs in slot order. `item` indexes `Sim::item_classes`;
/// `u8::MAX` is a slot with a probability and no class (it spends its share
/// and drops nothing).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DropTable {
    pub count: u8,
    pub item: [u8; MAX_DROPS],
    pub prob: [f32; MAX_DROPS],
}

impl DropTable {
    /// No drops (the `Default`, as a constant: a golden test that builds a
    /// template links no `core::sim` function, `engine/goldens/deps.toml`).
    pub const NONE: Self = Self {
        count: 0,
        item: [0; MAX_DROPS],
        prob: [0.0; MAX_DROPS],
    };

    pub fn from_pairs(pairs: &[(u8, f32)]) -> Self {
        let mut t = DropTable::default();
        for (i, (item, prob)) in pairs.iter().take(MAX_DROPS).enumerate() {
            t.item[i] = *item;
            t.prob[i] = *prob;
            t.count = i as u8 + 1;
        }
        t
    }

    /// The walk (§3.2): the slot a roll `r` in [0, 1) picks, if any.
    pub fn pick(&self, mut r: f32) -> Option<u8> {
        for i in 0..usize::from(self.count) {
            r -= self.prob[i];
            if r < 0.0 {
                return (self.item[i] != u8::MAX).then_some(self.item[i]);
            }
        }
        None
    }
}

/// An item on the field (`Sim::pickups`). HASHED.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pickup {
    pub source: Option<(UnitHandle, usize)>,
    /// Index into `Sim::item_classes`.
    pub class: u8,
    /// The item's origin (the bottom of its model).
    pub pos: Vec3,
    pub vel: Vec3,
    /// Its heading (the dead soldier's).
    pub yaw: f32,
    /// Runs from the class's `LifeSpan` down to [`LIFE_END`].
    pub life: f32,
    pub landed: bool,
}

impl Pickup {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if let Some((owner, weapon)) = self.source {
            eat(b"dispenser");
            eat(&owner.index.to_le_bytes());
            eat(&owner.gen.to_le_bytes());
            eat(&(weapon as u32).to_le_bytes());
        }
        eat(&[self.class, self.landed as u8]);
        for v in [
            self.pos.x, self.pos.y, self.pos.z, self.vel.x, self.vel.y, self.vel.z, self.yaw,
            self.life,
        ] {
            eat(&v.hash_bits().to_le_bytes());
        }
    }
}

/// AMCAP: total authored clips minus one whenever rounds remain loaded, using
/// BF1's rule on every map (FEEL_NOTES; re-ammo-reload.md).
fn reserve_cap(spec: &UnitSpec, slot: u8, class: &WeaponClass, state: &WeaponState) -> u32 {
    let clips = if slot == spec.weapon_slot {
        spec.total_clips
    } else {
        spec.slots
            .iter()
            .find(|s| s.slot == slot)
            .map_or(0, |s| s.clips)
    };
    clips
        .saturating_sub(u32::from(state.clip > 0))
        .saturating_mul(class.rounds_per_clip)
}

fn needs_clips(w: &WeaponState, class: &WeaponClass, cap: u32) -> bool {
    class.dispenser.is_none()
        && !class.infinite_ammo()
        && w.spare != SPARE_INFINITE
        && w.spare < cap
}

/// §5.4 for one weapon: `clips` more in reserve, up to `cap` rounds. A
/// weapon with an endless reserve is unchanged.
fn give_clips(w: &mut WeaponState, class: &WeaponClass, clips: u32, cap: u32) -> bool {
    if clips == 0 || !needs_clips(w, class, cap) {
        return false;
    }
    let new = w
        .spare
        .saturating_add(clips.saturating_mul(class.rounds_per_clip))
        .min(cap.max(w.spare));
    let changed = new != w.spare;
    w.spare = new;
    changed
}

/// §5.4 for a unit: `clips` more to every weapon it holds, each to its cap
/// (the items and the stations, card PWST, share it).
pub(super) fn give_ammo(classes: &[WeaponClass], u: &mut Unit, clips: u32) -> bool {
    if clips == 0 {
        return false;
    }
    let wc = &classes[u.weapon_class];
    let cap = reserve_cap(&u.spec, u.weapon_slot, wc, &u.weapon);
    let mut ammo = give_clips(&mut u.weapon, wc, clips, cap);
    for w in &mut u.held {
        let wc = &classes[w.class];
        let cap = reserve_cap(&u.spec, w.slot, wc, &w.state);
        ammo |= give_clips(&mut w.state, wc, clips, cap);
    }
    ammo
}

/// Does `u`'s body touch `p`'s box (§5.7)? The body is an upright capsule of
/// [`BODY_RADIUS`] whose axis stands [`BODY_AHEAD`] in front of the feet.
fn touches(u: &Unit, p: &Pickup, c: &ItemClass) -> bool {
    let (sy, cy) = fmath::sin_cos(u.soldier.yaw);
    let axis = u.soldier.pos + Vec3::new(-sy, 0.0, -cy) * BODY_AHEAD;
    // Into the item's frame (yaw about +Y, forward = -Z at yaw 0).
    let (si, ci) = fmath::sin_cos(p.yaw);
    let d = axis - p.pos;
    let local = Vec3::new(d.x * ci - d.z * si, d.y, d.x * si + d.z * ci) - c.centre;
    let dx = (local.x.abs() - c.half.x).fmax(0.0);
    let dz = (local.z.abs() - c.half.z).fmax(0.0);
    // The capsule's axis runs from one radius over the feet to one under the
    // head; the vertical gap between that span and the box.
    let (lo, hi) = (local.y + BODY_RADIUS, local.y + BODY_HEIGHT - BODY_RADIUS);
    let dy = (lo - c.half.y).fmax(-c.half.y - hi).fmax(0.0);
    dx * dx + dy * dy + dz * dz <= BODY_RADIUS * BODY_RADIUS
}

impl Sim {
    pub(super) fn dispense_shots(
        &mut self,
        i: usize,
        held_slot: Option<usize>,
        class: usize,
        shots: &[ShotSpec],
    ) {
        for shot in shots {
            if self.dispense_pickup(i, class, shot.charge) {
                continue;
            }
            // Both ammo cheats: the use was never drawn, so nothing to give back.
            let (infinite, no_reload) = self.ammo_cheats(i);
            let u = &mut self.units[i];
            let wc = &self.weapon_classes[class];
            if wc.infinite_ammo() || (infinite && no_reload) {
                continue;
            }
            let state = match held_slot {
                Some(k) => &mut u.held[k].state,
                None => &mut u.weapon,
            };
            let cancel_reload = state.reloading();
            state.refund_dispense(wc);
            if cancel_reload {
                let owner = self.handle(i);
                if let Some(event) = self.events.iter().rposition(|e| match e {
                    Event::Reload { unit, weapon_class } => {
                        *unit == owner && *weapon_class as usize == class
                    }
                    _ => false,
                }) {
                    self.events.remove(event);
                }
            }
        }
    }

    pub(super) fn dispense_pickup(&mut self, i: usize, weapon: usize, charge: f32) -> bool {
        let d = self.weapon_classes[weapon].dispenser.as_ref().unwrap();
        let owner = self.handle(i);
        let class = self
            .item_classes
            .iter()
            .position(|c| c.name == d.item.name)
            .unwrap() as u8;
        let (pos, vel) = self.throw_from(i, weapon, d.speed(charge));
        self.spawn_pickup(
            class,
            pos,
            vel,
            self.units[i].soldier.yaw,
            Some((owner, weapon)),
        )
    }

    fn spawn_pickup(
        &mut self,
        class: u8,
        pos: Vec3,
        vel: Vec3,
        yaw: f32,
        source: Option<(UnitHandle, usize)>,
    ) -> bool {
        if self.ground_rules == GroundRules::Bf2 && self.outcome.is_some() {
            return false;
        }
        let Some(ic) = self.item_classes.get(usize::from(class)) else {
            return false;
        };
        let replace = source.and_then(|(_, weapon)| {
            let max = self.weapon_classes[weapon].dispenser.as_ref()?.max_items as usize;
            if max == 0 {
                return None;
            }
            let mut owned = self
                .pickups
                .iter()
                .enumerate()
                .filter(|(_, p)| p.source == source);
            let (oldest, _) = owned.next()?;
            (owned.count() + 1 >= max).then_some(oldest)
        });
        if self.pickups.len() - usize::from(replace.is_some()) >= self.item_pool {
            return false;
        }
        if let Some(oldest) = replace {
            self.pickups.remove(oldest);
        }
        self.pickups.push(Pickup {
            source,
            class,
            pos,
            vel,
            yaw,
            life: ic.life_span,
            landed: false,
        });
        if let Some((unit, weapon)) = source {
            self.events.push(Event::Throw {
                unit,
                weapon_class: weapon as u32,
                origin: pos,
            });
        }
        self.events.push(Event::ItemDropped { item: class, pos });
        true
    }

    pub(crate) fn item_needed(&self, u: &Unit, c: &ItemClass) -> bool {
        let (health, ammo) = self.item_needs(u, c);
        health || ammo
    }

    /// Separate gains so support AI can suppress health and ammo independently.
    pub(crate) fn item_needs(&self, u: &Unit, c: &ItemClass) -> (bool, bool) {
        if !matches!(u.health.health_type, HealthType::Person | HealthType::Droid) {
            return (false, false);
        }
        let hero = self.hero_class(u.team) == Some(u.spec.class);
        let health = !hero && c.health > 0.0 && u.health.cur < u.health.max;
        let lacks = |slot, class: usize, state: &WeaponState| {
            let wc = &self.weapon_classes[class];
            needs_clips(state, wc, reserve_cap(&u.spec, slot, wc, state))
        };
        let ammo = c.ammo > 0
            && (lacks(u.weapon_slot, u.weapon_class, &u.weapon)
                || u.held.iter().any(|w| lacks(w.slot, w.class, &w.state)));
        (health, ammo)
    }

    /// Register an item class (one entry per name) and return its index.
    pub fn add_item_class(&mut self, c: ItemClass) -> u8 {
        if let Some(i) = self.item_classes.iter().position(|x| x.name == c.name) {
            return i as u8;
        }
        self.item_classes.push(c);
        (self.item_classes.len() - 1) as u8
    }

    /// A soldier died (§3.2): its class's roll and, with room in the pool,
    /// the item. Called before the unit leaves its seat, with the body where
    /// the death left it. `cause` is `None` for damage.
    pub(super) fn drop_item(&mut self, h: UnitHandle, cause: Option<DeathCause>) {
        // The port's recycles remove a bot; they are not deaths.
        if matches!(cause, Some(DeathCause::Lost | DeathCause::Stalled)) {
            return;
        }
        let u = &self.units[h.index as usize];
        let table = u.spec.drops;
        if table.count == 0 {
            return;
        }
        let (feet, vel, yaw) = (u.soldier.pos, u.soldier.vel, u.soldier.yaw);
        // One roll, spent whether or not anything comes of it.
        let r = self.lcg.unit();
        let Some(class) = table.pick(r) else {
            return;
        };
        let bf2 = self.ground_rules == GroundRules::Bf2;
        let vel = if bf2 && vel.length_squared() <= DROP_SIDE_SPEED * DROP_SIDE_SPEED {
            // Row 0 of the soldier's matrix = up x forward: its left.
            let (sy, cy) = fmath::sin_cos(yaw);
            Vec3::new(-cy, 0.0, sy) * DROP_SIDE_SPEED
        } else {
            vel
        };
        let pos = feet + Vec3::Y * DROP_HEIGHT;
        self.spawn_pickup(class, pos, vel, yaw, None);
    }

    /// Step 5c: every item's flight and life, then the pickups.
    pub(super) fn tick_pickups(&mut self, dt: f32) {
        if self.pickups.is_empty() {
            return;
        }
        for i in 0..self.pickups.len() {
            let mut p = self.pickups[i];
            p.life -= dt;
            if !p.landed {
                let from_y = p.pos.y;
                p.vel.y += ITEM_GRAVITY * dt;
                p.pos += p.vel * dt;
                let floor = self.floor_point_under(p.pos, from_y);
                if p.pos.y <= floor {
                    p.pos.y = floor;
                    p.vel = Vec3::ZERO;
                    p.landed = true;
                    self.events.push(Event::ItemLanded {
                        item: p.class,
                        pos: p.pos,
                    });
                }
            }
            self.pickups[i] = p;
        }
        // Gone: the life ran out, or it fell out of the world (§4).
        self.pickups
            .retain(|p| p.life > LIFE_END && p.pos.y > -500.0);
        let mut i = 0;
        while i < self.pickups.len() {
            if self.take_item(i) {
                self.pickups.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// §5.2 for item `i`: the first living soldier (in slot order) that
    /// touches it and gains something takes it.
    fn take_item(&mut self, i: usize) -> bool {
        let p = self.pickups[i];
        let Some(c) = self.item_classes.get(usize::from(p.class)) else {
            return false;
        };
        if !p.landed && c.life_span - p.life <= TAKE_AGE {
            return false;
        }
        for ui in 0..self.units.len() {
            let u = &self.units[ui];
            // A rider is not in the world; an animal, a vehicle and a
            // building get nothing from any disc item (§2.2).
            if !u.alive
                || u.mount.is_some()
                || !matches!(u.health.health_type, HealthType::Person | HealthType::Droid)
                || !touches(u, &p, c)
            {
                continue;
            }
            let hero = self.hero_class(u.team) == Some(u.spec.class);
            let classes = &self.weapon_classes;
            let u = &mut self.units[ui];
            let health = !hero && c.health != 0.0 && u.health.cur < u.health.max;
            if health {
                u.health.cur = (u.health.cur + c.health).fmin(u.health.max);
            }
            let ammo = give_ammo(classes, u, c.ammo);
            let energy = c.energy > 0.0 && u.soldier.energy < u.consts.energy_bar;
            if energy {
                u.soldier.energy = (u.soldier.energy + c.energy).fmin(u.consts.energy_bar);
            }
            // A buff item is always taken (§5.5); the timer and its number
            // are replaced only by a longer timer.
            let buff = c.offense.0 > 0.0 || c.defense.0 > 0.0;
            if !hero {
                u.buffs.apply_multipliers(c.offense, c.defense);
            }
            if health || ammo || energy || buff {
                let unit = UnitHandle {
                    index: ui as u32,
                    gen: u.gen,
                };
                self.events.push(Event::ItemTaken {
                    unit,
                    item: p.class,
                    pos: p.pos,
                    health,
                    ammo,
                    energy,
                });
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
