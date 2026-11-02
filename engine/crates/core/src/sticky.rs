//! The thrown ordnance — the PS2's `sticky` class (card GREN,
//! `analysis/re-grenades.md` §4 [O]; B1 `FUN_0027f9d8` / B2 `FUN_003217c0`):
//! a shell under gravity that is swept one ray a tick, sticks to what its
//! class's `Stick*` mask names, bounces off the rest with the code's own
//! constants, comes to rest, and only THEN counts its `LifeSpan` down to its
//! `ExplosionExpire` class. This file is the class and one instance's rules;
//! the world sweep and the blast are `Sim`'s (`sim/ordnance.rs`). A class's
//! `bf2` follows its disc; a BF1 class fielded on a BF2 map is set to it by
//! [`crate::weapon::Thrown::throw_as_bf2`].

use crate::unit::UnitHandle;
use crate::vehicle::VehicleHandle;
use crate::weapon::ExplosionClass;
use glam::Vec3;

/// The sweep ray starts this far behind the previous position and runs the
/// step's length plus [`SWEEP_EXTRA`] from there (§4.2, B1 `FUN_0027eff8`,
/// B2 `FUN_00320de8`): it ends 0.1 m past the new position.
pub const SWEEP_BACK: f32 = 0.1;
pub const SWEEP_EXTRA: f32 = 0.2;
/// The bounce's constants (§4.3; the `lui` / `ori` pairs at B1 0x27f7f0 -
/// 0x27f8c8). The disc's `Rebound` / `Friction` are never read by a sticky.
pub const RESTITUTION: f32 = 0.37;
pub const FRICTION: f32 = 1.15;
pub const FRICTION_GAIN: f32 = 1.37;
/// A bounce leaves the grenade this far off the surface; a stick, [`STICK_OFF`].
pub const PUSH_OFF: f32 = 0.1;
pub const STICK_OFF: f32 = 0.01;
/// Where the push-off's room check starts along the normal: off the surface
/// it bounced on (`Sim::push_off_room`, issue #37).
pub const PUSH_ROOM_START: f32 = 0.001;
/// BF1 counts a bounce on a floor: normal.y above this.
pub const FLOOR_NY: f32 = 0.7;
/// BF2 counts a bounce that leaves it slower than this (speed², m²/s²).
pub const SLOW_SPEED_SQ: f32 = 1.0;
/// At this count the grenade is at rest.
pub const REST_BOUNCES: u8 = 4;
/// It dies below this height (§4.1; BF2 −2000).
pub const WORLD_FLOOR: f32 = -500.0;
/// Under water the shell's update drags instead of falling (§4.1).
pub const WATER_DRAG: f32 = 0.7;

/// The `Stick*` mask bits (§4.2; B1 class+0x100, B2 class+0x13c): the low
/// bits are the object kind (`Unit::kind`), bit 31 the terrain.
pub mod stick {
    pub const PERSON: u32 = 1;
    pub const ANIMAL: u32 = 1 << 1;
    pub const DROID: u32 = 1 << 2;
    pub const VEHICLE: u32 = 1 << 3;
    pub const BUILDING: u32 = 1 << 4;
    pub const BUILDING_DEAD: u32 = 1 << 5;
    pub const BUILDING_UNBUILT: u32 = 1 << 6;
    pub const TERRAIN: u32 = 1 << 31;
}

/// `StickPerson` … `StickTerrain`, in mask-bit order, and the set-all key.
const STICK_KEYS: [(u32, u32); 8] = [
    (0x01E9_CAEA, stick::PERSON),
    (0xD1B4_9B73, stick::ANIMAL),
    (0x3B3F_45B7, stick::DROID),
    (0x9B4A_F9ED, stick::VEHICLE),
    (0x7449_C0B3, stick::BUILDING),
    (0xAD9C_5FA5, stick::BUILDING_DEAD),
    (0xA8A2_AFF4, stick::BUILDING_UNBUILT),
    (0x9570_6D30, stick::TERRAIN),
];
const HASH_STICK_ALL: u32 = 0xD391_3CC2;
const HASH_GRAVITY: u32 = 0xD209_FF45;
const HASH_COLLISION_OTHER_SOUND: u32 = 0xEDC6_251A;

/// What a `sticky` ordnance class adds to [`crate::weapon::OrdnanceClass`]
/// (whose `velocity` is the launch speed and `life_span` the fuse).
#[derive(Clone, Debug, PartialEq)]
pub struct StickyClass {
    /// m/s² on y: `Gravity` × −9.8 (the shell setter, §4.1; `Gravity`
    /// defaults to 1.0).
    pub gravity: f32,
    /// [`stick`] bits; the constructor's default is every bit.
    pub stick_mask: u32,
    /// BF2's rules where the games differ: the rest count ([`SLOW_SPEED_SQ`]
    /// against [`FLOOR_NY`]) and the water contact's sink speed.
    pub bf2: bool,
    /// The `ExplosionExpire` class, resolved by the class loader.
    pub explosion: Option<ExplosionClass>,
    /// `CollisionOtherSound`: what a bounce or a stick plays.
    pub bounce_sound: Option<String>,
}

impl StickyClass {
    /// The constructor's defaults (B1 `FUN_0027f9d8` 204643, B2 `FUN_003217c0`).
    pub fn new(bf2: bool) -> Self {
        Self {
            gravity: -9.8,
            stick_mask: u32::MAX,
            bf2,
            explosion: None,
            bounce_sound: None,
        }
    }

    /// Replay PROPs in order (the class chain parent first, each class's in
    /// file order) through the setter (B1 `FUN_0027fb78`, B2 `FUN_00321960`):
    /// the set-all key and the per-kind keys write one mask, so the order is
    /// the value.
    pub fn replay<'a>(props: impl IntoIterator<Item = (u32, &'a str)>, bf2: bool) -> Self {
        let mut c = Self::new(bf2);
        let num = |v: &str| {
            v.split_whitespace()
                .next()
                .and_then(|t| t.parse::<f32>().ok())
        };
        for (h, v) in props {
            if h == HASH_GRAVITY {
                if let Some(g) = num(v) {
                    c.gravity = g * -9.8;
                }
            } else if h == HASH_STICK_ALL {
                if let Some(f) = num(v) {
                    c.stick_mask = if f != 0.0 { u32::MAX } else { 0 };
                }
            } else if h == HASH_COLLISION_OTHER_SOUND {
                c.bounce_sound = Some(v.trim().to_owned());
            } else if let Some((_, bit)) = STICK_KEYS.iter().find(|(k, _)| *k == h) {
                match num(v) {
                    Some(f) if f != 0.0 => c.stick_mask |= bit,
                    Some(_) => c.stick_mask &= !bit,
                    None => {}
                }
            }
        }
        c
    }
}

/// What the sweep ray met.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surface {
    Water,
    Terrain,
    /// A static body of the map. PROVISIONAL: it never holds a grenade — the
    /// PS2 asks the object's kind bit, and which kind a plain prop carries
    /// was not read; the destructible buildings the port has are its turrets
    /// ([`Surface::Vehicle`] with `building`).
    World,
    /// A soldier; `kind` is [`crate::unit::Unit::kind`].
    Unit {
        handle: UnitHandle,
        kind: u32,
    },
    /// A vehicle's hull; `building` for a building turret (the PS2's kind 4).
    Vehicle {
        handle: VehicleHandle,
        building: bool,
    },
}

/// What a grenade is held to once it has stuck.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// The world: it stays where it is.
    World,
    /// Rides a soldier: `local` is the offset from its feet (PROVISIONAL —
    /// the PS2's record holds a bone; no shipped grenade sticks to a person).
    Unit { handle: UnitHandle, local: Vec3 },
    /// Rides a hull: `local` is in the vehicle's frame.
    Vehicle { handle: VehicleHandle, local: Vec3 },
}

/// What a contact did ([`Sticky::contact`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contact {
    Water,
    Stuck,
    Bounced,
    Rest,
}

/// The bounce (§4.3; B1 204520-204595, B2 314270-314345): `None` when the
/// velocity does not point into the surface (the grenade rests instead).
pub fn bounce(v: Vec3, n: Vec3) -> Option<Vec3> {
    let vn = v.dot(n);
    if vn >= 0.0 {
        return None;
    }
    let vt = v - n * vn;
    let f = (-vn * FRICTION * FRICTION_GAIN / vt.length()).min(1.0);
    Some(vt * (1.0 - f) - n * (vn * RESTITUTION))
}

/// A human's throw direction (§3.3, B1 215847-215862): the aim with
/// `ShotElevate` added to y, renormalised. A bot throws along its aim.
pub fn throw_dir(aim: Vec3, shot_elevate: f32) -> Vec3 {
    (aim + Vec3::Y * shot_elevate).normalize()
}

/// One thrown grenade.
#[derive(Clone, Debug)]
pub struct Sticky {
    pub pos: Vec3,
    pub vel: Vec3,
    /// The instance's gravity (s+0x110): the class's, zeroed at rest.
    pub gravity: f32,
    /// Seconds of fuse left (s+0x3c); counts only while [`Self::armed`].
    pub life: f32,
    /// The life timer runs (s+0x39): set by rest, a stick or water — never
    /// by the throw or a bounce (§4.4).
    pub armed: bool,
    /// Counted bounces (§4.3).
    pub bounces: u8,
    /// Still in the sweep list: flying. Cleared by rest or a stick.
    pub moving: bool,
    pub anchor: Option<Anchor>,
    /// The tick of its first contact (s+0x12c): the AI warning's clock.
    pub touched: Option<u64>,
    /// Stable identity of this grenade's AI warning, independent of contact tick.
    pub warning_id: Option<u64>,
    pub owner: UnitHandle,
    pub team: u8,
    /// Index into `Sim::weapon_classes` of the weapon that threw it.
    pub weapon_class: usize,
}

impl Sticky {
    pub fn new(
        pos: Vec3,
        vel: Vec3,
        c: &StickyClass,
        life_span: f32,
        owner: UnitHandle,
        team: u8,
        weapon_class: usize,
    ) -> Self {
        Self {
            pos,
            vel,
            gravity: c.gravity,
            life: life_span,
            armed: false,
            bounces: 0,
            moving: true,
            anchor: None,
            touched: None,
            warning_id: None,
            owner,
            team,
            weapon_class,
        }
    }

    /// The base ordnance's update (§4.1, B1 `FUN_00272e98`): the armed fuse
    /// counts down; `true` when it has run out (or the grenade fell out of
    /// the world, which makes no explosion — see [`Self::lost`]).
    pub fn burn(&mut self, dt: f32) -> bool {
        if self.armed {
            self.life -= dt;
        }
        self.life <= 0.0 || self.lost()
    }

    /// Below the world's floor: gone without a blast.
    pub fn lost(&self) -> bool {
        self.pos.y < WORLD_FLOOR
    }

    /// The shell's update (§4.1, B1 `FUN_0027e478`): gravity (or the water's
    /// drag) onto the velocity, then the move.
    pub fn fly(&mut self, dt: f32, under_water: bool) {
        if !self.moving {
            return;
        }
        if under_water {
            let k = WATER_DRAG * dt / (WATER_DRAG * dt + 1.0);
            self.vel -= Vec3::new(self.vel.x, self.vel.y + 0.5, self.vel.z) * k;
        } else {
            self.vel.y += self.gravity * dt;
        }
        self.pos += self.vel * dt.min(self.life);
    }

    fn rest(&mut self) {
        self.moving = false;
        self.gravity = 0.0;
        self.vel = Vec3::ZERO;
        self.armed = true;
    }

    /// The contact handler (§4.2 steps 2-5, B1 `FUN_0027f1e8` / B2
    /// `FUN_00320fe8`). `point` and `n` are the sweep's hit; the caller
    /// records the first contact and fills [`Self::anchor`] after a
    /// [`Contact::Stuck`]. A bounce pushes it `push_off` along `n`: the
    /// PS2's [`PUSH_OFF`], less where a surface is nearer.
    pub fn contact(
        &mut self,
        c: &StickyClass,
        point: Vec3,
        n: Vec3,
        what: Surface,
        push_off: f32,
    ) -> Contact {
        let bit = match what {
            Surface::Water => {
                self.vel = Vec3::new(0.0, if c.bf2 { -2.0 } else { -0.2 }, 0.0);
                self.armed = true;
                return Contact::Water;
            }
            Surface::Terrain => stick::TERRAIN,
            Surface::World => 0,
            Surface::Unit { kind, .. } => 1 << (kind & 7),
            Surface::Vehicle { building, .. } => {
                if building {
                    stick::BUILDING
                } else {
                    stick::VEHICLE
                }
            }
        };
        if c.stick_mask & bit != 0 {
            self.pos = point + n * STICK_OFF;
            self.moving = false;
            self.vel = Vec3::ZERO;
            self.armed = true;
            return Contact::Stuck;
        }
        if self.bounces < REST_BOUNCES {
            match bounce(self.vel, n) {
                None => self.bounces = REST_BOUNCES,
                Some(v) => {
                    self.pos = point + n * push_off;
                    self.vel = v;
                    let counted = if c.bf2 {
                        v.length_squared() < SLOW_SPEED_SQ
                    } else {
                        n.y > FLOOR_NY
                    };
                    self.bounces += u8::from(counted);
                }
            }
        }
        if self.bounces >= REST_BOUNCES {
            self.rest();
            Contact::Rest
        } else {
            Contact::Bounced
        }
    }

    /// Feed into the sim hash.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for v in [
            self.pos.x, self.pos.y, self.pos.z, self.vel.x, self.vel.y, self.vel.z, self.life,
        ] {
            eat(&v.to_bits().to_le_bytes());
        }
        eat(&[self.armed as u8, self.bounces, self.moving as u8]);
        eat(&self.owner.index.to_le_bytes());
        // The launch identity survives switches, deaths and slot reuse.
        eat(b"STICKY_IDENTITY");
        eat(&(self.weapon_class as u64).to_le_bytes());
        eat(&[self.team]);
        eat(&self.owner.index.to_le_bytes());
        eat(&self.owner.gen.to_le_bytes());
        eat(&[u8::from(self.touched.is_some())]);
        if let Some(tick) = self.touched {
            eat(&tick.to_le_bytes());
        }
        // None/World are inert; a dynamic frame determines the next position.
        let dynamic = match self.anchor {
            Some(Anchor::Unit { handle, local }) => Some((1, handle.index, handle.gen, local)),
            Some(Anchor::Vehicle { handle, local }) => Some((2, handle.index, handle.gen, local)),
            _ => None,
        };
        if let Some((kind, index, generation, local)) = dynamic {
            eat(b"STICKY_ANCHOR");
            eat(&[kind]);
            eat(&index.to_le_bytes());
            eat(&generation.to_le_bytes());
            for v in [local.x, local.y, local.z] {
                eat(&v.to_bits().to_le_bytes());
            }
        }
        if let Some(id) = self.warning_id {
            eat(b"STICKY_WARNING");
            eat(&id.to_le_bytes());
        }
        // Moving gravity is class-derived; a stopped grenade never reads it.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = crate::TICK_DT;

    fn near(a: Vec3, b: Vec3, tol: f32) -> bool {
        (a - b).abs().max_element() <= tol
    }

    fn class(bf2: bool, mask: u32) -> StickyClass {
        StickyClass {
            gravity: 1.25 * -9.8,
            stick_mask: mask,
            ..StickyClass::new(bf2)
        }
    }

    fn thrown(c: &StickyClass, pos: Vec3, vel: Vec3, life: f32) -> Sticky {
        let owner = UnitHandle { index: 0, gen: 0 };
        Sticky::new(pos, vel, c, life, owner, 0, 0)
    }

    /// The PS2's own launch rows (re-grenades §3.3 [M], `lead_tmp/gren/
    /// bf{1,2}_throw_a.json`): aim -> velocity = normalise(aim + 0.21 y) x 25.
    #[test]
    fn sticky_ps2_launch_is_the_elevated_aim_times_velocity() {
        for (aim, want) in [
            (
                Vec3::new(-0.1205, -0.0622, 0.9908),
                Vec3::new(-2.9858, 3.6617, 24.5495),
            ),
            (
                Vec3::new(0.0064, 0.1857, -0.9826),
                Vec3::new(0.1513, 9.3390, -23.1896),
            ),
        ] {
            let v = throw_dir(aim, 0.21) * 25.0;
            // The aims are the RAM rows to 4 decimals; 2e-3 covers that.
            assert!(near(v, want, 2e-3), "{aim} -> {v}, PS2 {want}");
        }
    }

    /// The PS2's first bounce (§8): the velocity, the fitted terrain normal
    /// and the velocity before the next gravity step.
    #[test]
    fn sticky_ps2_bounce_matches_the_sampled_contact() {
        let v = Vec3::new(-2.986, -14.323, 24.549);
        let n = Vec3::new(-0.134, 0.991, 0.022).normalize();
        let out = bounce(v, n).expect("into the surface");
        let want = Vec3::new(-1.493, 4.644, 4.484);
        assert!(near(out, want, 0.02), "{out} vs PS2 {want}");
        // Leaving the surface is no bounce: the grenade rests.
        assert_eq!(bounce(Vec3::Y, Vec3::Y), None);
    }

    /// Fly `s` over a flat floor at y 0 with the sim's sweep order (update,
    /// then the contact); returns the ticks of every contact and of the
    /// fuse's end.
    fn flat_floor(c: &StickyClass, s: &mut Sticky) -> (Vec<(u32, Contact, f32)>, u32) {
        let mut contacts = Vec::new();
        for tick in 0..2000 {
            if s.burn(DT) {
                return (contacts, tick);
            }
            let prev = s.pos;
            s.fly(DT, false);
            if s.moving && prev.y > 0.0 && s.pos.y <= 0.0 {
                let t = prev.y / (prev.y - s.pos.y);
                let point = prev + (s.pos - prev) * t;
                let r = s.contact(c, point, Vec3::Y, Surface::Terrain, PUSH_OFF);
                contacts.push((tick, r, s.vel.length_squared()));
            }
        }
        panic!("the grenade never went off: {s:?}");
    }

    /// BF1 on a flat floor: four counted floor bounces, rest at the fourth,
    /// the blast `LifeSpan` 0.9 s after the rest — not after the throw.
    #[test]
    fn sticky_ps2_bf1_rests_after_four_floor_bounces_then_burns_its_fuse() {
        let c = class(false, 0x7fff_ff80); // the thermal's RAM mask
        let v = throw_dir(Vec3::new(0.0, 0.0, -1.0), 0.21) * 25.0;
        let mut s = thrown(&c, Vec3::new(0.0, 1.4, 0.0), v, 0.9);
        let (contacts, gone) = flat_floor(&c, &mut s);
        let kinds: Vec<Contact> = contacts.iter().map(|c| c.1).collect();
        assert_eq!(
            kinds,
            [
                Contact::Bounced,
                Contact::Bounced,
                Contact::Bounced,
                Contact::Rest
            ]
        );
        assert_eq!(s.vel, Vec3::ZERO);
        // Not armed in flight: the fuse still held 0.9 s at the rest.
        let rest = contacts[3].0;
        let fuse = (gone - rest) as f32 * DT;
        assert!((fuse - 0.9).abs() <= DT + 1e-4, "fuse {fuse} s after rest");
        assert!(rest as f32 * DT > 1.0, "rest at {} s", rest as f32 * DT);
    }

    /// BF2 counts only the bounces that leave it slower than 1 m/s: the fast
    /// first ones do not count, the grenade rests after four that do, and
    /// the blast comes `LifeSpan` 0.6 s after the rest.
    #[test]
    fn sticky_ps2_bf2_counts_only_slow_bounces() {
        let c = class(true, 0x7fff_ffd8); // the BF2 thermal's RAM mask
        let v = throw_dir(Vec3::new(0.0, 0.0, -1.0), 0.21) * 25.0;
        let mut s = thrown(&c, Vec3::new(0.0, 1.8183, -0.7433), v, 0.6);
        let (contacts, gone) = flat_floor(&c, &mut s);
        let fast = contacts.iter().filter(|c| c.2 >= SLOW_SPEED_SQ).count();
        let slow = contacts.iter().filter(|c| c.2 < SLOW_SPEED_SQ).count();
        assert!(fast >= 1, "the first bounces are fast: {contacts:?}");
        assert!(
            contacts[..fast]
                .iter()
                .all(|c| c.1 == Contact::Bounced && c.2 >= SLOW_SPEED_SQ),
            "fast bounces come first and do not count: {contacts:?}"
        );
        // The fourth slow bounce rests it (its velocity is zeroed, so it
        // reads as speed 0 here).
        assert_eq!(slow, 4, "{contacts:?}");
        assert_eq!(contacts.last().unwrap().1, Contact::Rest);
        let rest = contacts.last().unwrap().0;
        let fuse = (gone - rest) as f32 * DT;
        assert!((fuse - 0.6).abs() <= DT + 1e-4, "fuse {fuse} s after rest");
    }

    /// The `Stick*` mask: a BF2 thermal sticks to a vehicle and bounces off
    /// one in BF1; BF1's crack grenade sticks; nothing sticks to a soldier
    /// or the ground.
    #[test]
    fn sticky_ps2_stick_follows_the_class_mask() {
        let hull = Surface::Vehicle {
            handle: VehicleHandle { index: 0, gen: 0 },
            building: false,
        };
        let v = Vec3::new(0.0, -3.0, -10.0);
        let n = Vec3::Z;
        let p = Vec3::new(0.0, 1.0, -5.0);
        let crack = stick::VEHICLE | stick::BUILDING | stick::BUILDING_DEAD;
        for (bf2, mask, want) in [
            (true, 0x7fff_ffd8, Contact::Stuck),
            (false, 0x7fff_ff80, Contact::Bounced),
            (false, 0x7fff_ff80 | crack, Contact::Stuck),
        ] {
            let c = class(bf2, mask);
            let mut s = thrown(&c, p - v * DT, v, 1.0);
            assert_eq!(
                s.contact(&c, p, n, hull, PUSH_OFF),
                want,
                "bf2 {bf2} mask {mask:#x}"
            );
            if want == Contact::Stuck {
                assert!(s.armed && !s.moving && s.vel == Vec3::ZERO);
                assert!(near(s.pos, p + n * STICK_OFF, 1e-6));
            } else {
                assert!(!s.armed && s.moving, "a bounce does not arm the fuse");
            }
            let mut s = thrown(&c, p - v * DT, v, 1.0);
            let man = Surface::Unit {
                handle: UnitHandle { index: 1, gen: 0 },
                kind: 0,
            };
            assert_eq!(s.contact(&c, p, n, man, PUSH_OFF), Contact::Bounced);
            let mut s = thrown(&c, p - v * DT, v, 1.0);
            assert_eq!(
                s.contact(&c, p, Vec3::Y, Surface::Terrain, PUSH_OFF),
                Contact::Bounced
            );
        }
    }

    /// Water sinks it slowly and arms the fuse at once.
    #[test]
    fn water_arms_the_fuse() {
        let c = class(false, 0);
        let mut s = thrown(&c, Vec3::Y, Vec3::new(3.0, -9.0, 0.0), 0.9);
        assert_eq!(
            s.contact(&c, Vec3::ZERO, Vec3::Y, Surface::Water, PUSH_OFF),
            Contact::Water
        );
        assert!(s.armed && s.moving);
        assert_eq!(s.vel, Vec3::new(0.0, -0.2, 0.0));
    }

    /// The setter replays in order: the thermal's seven `Stick* 0` lines
    /// leave the PS2's RAM mask.
    #[test]
    fn replay_builds_the_ram_masks() {
        let keys = |vals: [&'static str; 8]| {
            STICK_KEYS
                .iter()
                .zip(vals)
                .map(|((h, _), v)| (*h, v))
                .collect::<Vec<_>>()
        };
        let mut bf1 = keys(["0"; 8]);
        bf1.push((HASH_GRAVITY, "1.25"));
        let c = StickyClass::replay(bf1, false);
        assert_eq!(c.stick_mask, 0x7fff_ff80);
        assert_eq!(c.gravity, -12.25);
        let c = StickyClass::replay(keys(["0", "0", "0", "1", "1", "0", "1", "0"]), true);
        assert_eq!(c.stick_mask, 0x7fff_ffd8);
        // Set-all then one key: the order is the value.
        let c = StickyClass::replay([(HASH_STICK_ALL, "0"), (STICK_KEYS[3].0, "1")], false);
        assert_eq!(c.stick_mask, stick::VEHICLE);
    }
}
