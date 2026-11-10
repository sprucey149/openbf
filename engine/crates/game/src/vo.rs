//! BF2's unit voice: the class's `VOSound` lines (card UVO2,
//! `analysis/re-unit-chatter.md` 3, 5 `[O]`). A line is an entry
//! `<sound> <event> [conditions]` on the soldier class (a child shares its
//! parent's chain); an event asks for it with a speaker, a SUBJECT (the local
//! player's unit when the caller names none) and an object. The speaker must
//! stand within its class's `VORadius` (50 m) of the subject - so BF2's
//! units talk only round the player - and an enemy of the subject speaks 31
//! times in 101; the speaker's entries of that event whose conditions all
//! match the context (where the object is from the subject, what it is, its
//! `VOUnitType`) are ranked by how many match (ties as the PS2's sort
//! leaves them, [`best`]), and the top one is asked through the battle's
//! voice queue (`announce::Commander::gate_calls`).
//!
//! Called from `Commander::calls` once per stepped tick on a BF2 battle; the
//! sim's events feed it (the tracker's take, a live grenade, hits, kills).
//! Its draws are its own: no sim state moves.

use crate::classdb::{fnv1a_lowercase, ClassDb};
use core::sim::{Event, Sim};
use glam::Vec3;

/// The PS2's event table (B2 ELF 0x624ed8, 40 words: the names' hashes; an
/// entry's event is its name's place in it, 0 for a name not in it - the
/// eight `...VO` requests - which no caller passes). Read from the ELF's
/// words 2026-10-16; indices 3, 0x13-0x15, 0x1e, 0x22, 0x26, 0x27 are
/// hashes of no known name.
pub const EVENTS: &[(u8, &str)] = &[
    (1, "AcquiredTarget"),
    (2, "FriendlyFire"),
    (4, "MissileLocked"),
    (5, "MissileIncoming"),
    (6, "PathFollowWait"),
    (7, "CollisionDamage"),
    (8, "Kill"),
    (9, "HeadshotKill"),
    (0xa, "Grenade"),
    (0xb, "RebelsShootDeadBody"),
    (0xc, "KillingSpree4"),
    (0xd, "KillingSpree8"),
    (0xe, "KillingSpree12"),
    (0xf, "KillingSpree16"),
    (0x10, "RepairStart"),
    (0x11, "RepairEnd"),
    (0x12, "GivePowerup"),
    (0x16, "SC_Follow"),
    (0x17, "SC_FollowResponse"),
    (0x18, "SC_StopFollow"),
    (0x19, "SC_StopFollowResponse"),
    (0x1a, "SC_VehicleWaitUp"),
    (0x1b, "SC_VehicleWaitUpResponse"),
    (0x1c, "SC_GetIn"),
    (0x1d, "SC_GetInResponse"),
    (0x1f, "SC_GetOut"),
    (0x20, "SC_GetOutResponse"),
    (0x21, "NotShootingCriticalHit"),
    (0x23, "NearbyFriendlySlaughter"),
    (0x24, "NearbyEnemySlaughter"),
    (0x25, "NearbyTeamKillSlaughter"),
];

pub const ACQUIRED_TARGET: u8 = 1;
pub const FRIENDLY_FIRE: u8 = 2;
pub const PATH_FOLLOW_WAIT: u8 = 6;
pub const GRENADE: u8 = 0xa;
pub const REBELS_SHOOT_DEAD_BODY: u8 = 0xb;
pub const KILLING_SPREE4: u8 = 0xc;
pub const KILLING_SPREE8: u8 = 0xd;
pub const FRIENDLY_SLAUGHTER: u8 = 0x23;
pub const ENEMY_SLAUGHTER: u8 = 0x24;
pub const TEAMKILL_SLAUGHTER: u8 = 0x25;

/// The events the port's triggers ask (`Vo::tick`): the PS2's call sites
/// that have a line and a port event. Never 0 (the `...VO` requests) or
/// MissileIncoming (5): no PS2 site passes them [receipt:e6dfb8fe].
/// HeadshotKill (9) has no voice trigger wired to the reported critical hits;
/// PathFollowWait, RebelsShootDeadBody, the repairs, GivePowerup,
/// NotShootingCriticalHit and the SC_ lines for their port triggers.
pub const ASKED: [u8; 8] = [
    ACQUIRED_TARGET,
    FRIENDLY_FIRE,
    GRENADE,
    KILLING_SPREE4,
    KILLING_SPREE8,
    FRIENDLY_SLAUGHTER,
    ENEMY_SLAUGHTER,
    TEAMKILL_SLAUGHTER,
];

/// An event's index by name (`FUN_002e57a8` B2:271488-271503; the hash is
/// the PROP hash): 0 when the table does not hold it.
pub fn event_index(name: &str) -> u8 {
    let h = fnv1a_lowercase(name);
    EVENTS
        .iter()
        .find(|(_, n)| fnv1a_lowercase(n) == h)
        .map_or(0, |(i, _)| *i)
}

/// `VORadius`: the class constructor's 50.0 (`lui 0x4248` at B2 0x269ec8);
/// no class on the disc authors the key (re-unit-chatter.md 3.1).
pub const VO_RADIUS: f32 = 50.0;

/// The chance in 101 a subject's ENEMY speaks (`slti 31` at 0x2e638c).
pub const ENEMY_SPEAKS: u32 = 31;
/// The entry point keeps an AcquiredTarget or PathFollowWait 71 times in 101
/// (`slti 71` at 0x2e67c8).
pub const ACQUIRED_KEPT: u32 = 71;
/// A human's kills each within this many seconds of the last are a spree
/// (`lui 0x4080` at 0x362948).
pub const SPREE_GAP: f32 = 4.0;
/// The slaughter counters: an AI unit within this many metres of a death
/// counts it, and four of one kind inside [`SLAUGHTER_WINDOW`] seconds ask
/// for the line (B2 `FUN_002b5240` 237348; `lui 0x4100` at 0x2b52e8).
pub const SLAUGHTER_REACH: f32 = 25.0;
pub const SLAUGHTER_WINDOW: f32 = 8.0;

// The condition word's fields (`FUN_002e5340` B2:271329, the context
// `FUN_002e5c08` 271654-271782): bits 0-1 the object above (1) / below (2);
// bits 2-3 right (1) / left (2) / behind (3) of the subject's facing; bits
// 4-5 under 10 m (1) / under 30 m (2) / over 100 m (3); bits 8-10 what the
// object is (flyer 1, hover 2, walker 3, turret 4, sniper 5, hero 7); bits
// 11-13 what the subject sits in (flyer 1, hover 2, walker 3, turret 4);
// byte 2 the object's `VOUnitType`. Bits 6, 7, 14, 15 are set but never
// compared (`FUN_002e5f90` 271787-271815).
const ABOVE: u32 = 1;
const BELOW: u32 = 2;
const RIGHT: u32 = 1 << 2;
const LEFT: u32 = 2 << 2;
const BEHIND: u32 = 3 << 2;
const UNDER_10: u32 = 1 << 4;
const UNDER_30: u32 = 2 << 4;
const OVER_100: u32 = 3 << 4;
pub const KIND_FLYER: u32 = 1;
pub const KIND_HOVER: u32 = 2;
pub const KIND_WALKER: u32 = 3;
pub const KIND_TURRET: u32 = 4;
pub const KIND_SNIPER: u32 = 5;

/// A condition token's (mask, value) by its hash (`FUN_002e5340`; the names
/// matched to the hashes 2026-10-16, eleven hashes of the parser name no
/// token the disc uses). `InVehicle`, which 184 entries carry, is not in it:
/// the parser skips it.
fn token(name: &str) -> Option<(u32, u32)> {
    let kind = |k: u32| (0x700, k << 8);
    let seat = |k: u32| (0x3800, k << 11);
    Some(match fnv1a_lowercase(name) {
        0xa421_3c0a => (3, ABOVE),
        0x9253_c0f2 => (3, BELOW),
        0x78e3_2de5 => (0xc, RIGHT),
        0x124a_ec70 => (0xc, LEFT),
        0xdced_8a09 => (0xc, BEHIND),
        0x54f3_570f => (0x30, UNDER_30),
        0xeedd_ae45 => kind(KIND_FLYER),
        0x5d84_7edd => kind(KIND_HOVER),
        0x9166_8bb7 => kind(KIND_WALKER),
        0x710f_3467 => kind(KIND_TURRET),
        0xb0c0_e926 => kind(KIND_SNIPER),
        0x6b46_8289 => kind(7),
        0x76eb_7f24 => seat(KIND_FLYER),
        0x5968_b570 => seat(KIND_HOVER),
        0x1ee5_dffc => seat(KIND_WALKER),
        0x1c99_3cf8 => seat(KIND_TURRET),
        _ => return None,
    })
}

/// One `VOSound` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub sound: String,
    pub event: u8,
    /// The condition word (the fields above; byte 2 `VOUnitNum<n>`).
    pub cond: u32,
}

/// `VOSound = <sound> <event> [conditions]`: tokens split on ` ,+-;`
/// (B2:186712-186735); a `VOUnitNum<n>` token sets byte 2 to n.
pub fn parse_entry(value: &str) -> Option<Entry> {
    let mut t = value
        .split([' ', ',', '+', '-', ';', '\t'])
        .filter(|s| !s.is_empty());
    let sound = t.next()?.to_owned();
    let event = event_index(t.next()?);
    let mut cond = 0u32;
    for c in t {
        if c.len() > 9
            && c.get(..9)
                .is_some_and(|p| p.eq_ignore_ascii_case("VOUnitNum"))
        {
            let n = c[9..].parse::<u32>().unwrap_or(0) & 0xff;
            cond = (cond & !0x00ff_0000) | (n << 16);
        } else if let Some((mask, v)) = token(c) {
            cond = (cond & !mask) | v;
        }
    }
    Some(Entry { sound, event, cond })
}

/// A class's voice: its entries (the chain's, parent first - the PS2 copies
/// the parent's list head and appends, `FUN_00269f38` 186600-186602), its
/// `VOUnitType` and `VORadius`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VoLines {
    pub entries: Vec<Entry>,
    pub unit_type: u8,
    pub radius: f32,
}

/// `class`'s voice from the baked class db.
pub fn class_lines(db: &ClassDb, class: &str) -> VoLines {
    let h = fnv1a_lowercase("VOSound");
    let entries = db
        .chain(class)
        .iter()
        .rev()
        .flat_map(|d| d.props.iter().filter(|p| p.0 == h))
        .filter_map(|p| parse_entry(&p.1))
        .collect();
    let num = |k: &str| db.prop(class, k).and_then(|v| v.trim().parse::<f32>().ok());
    VoLines {
        entries,
        unit_type: num("VOUnitType").map_or(0, |v| v.clamp(0.0, 255.0) as u8),
        radius: num("VORadius").map_or(VO_RADIUS, |v| v.max(0.0)),
    }
}

/// An entry passes when each of its fields is 0 or the context's
/// (`FUN_002e5f90`).
pub fn passes(cond: u32, ctx: u32) -> bool {
    [(0, 3), (2, 3), (4, 3), (8, 7), (11, 7), (16, 0xff)]
        .iter()
        .all(|&(s, m)| {
            let f = (cond >> s) & m;
            f == 0 || f == (ctx >> s) & m
        })
}

/// How many of an entry's fields are set and match (`FUN_002e6060`).
pub fn score(cond: u32, ctx: u32) -> u32 {
    [(0, 3), (2, 3), (4, 3), (8, 7), (11, 7), (16, 0xff)]
        .iter()
        .filter(|&&(s, m)| {
            let f = (cond >> s) & m;
            f != 0 && f == (ctx >> s) & m
        })
        .count() as u32
}

/// The entry an event plays (`FUN_002e6180` 272041-272079): the first 64
/// passing entries of the event in list order, sorted by score with the
/// PS2's C library quicksort (`FUN_005216b0`, the BSD one); slot 0 plays.
/// On equal scores the comparator (ELF 0x2e6130) puts the later array slot
/// first, so with fewer than 7 passing the last parsed of the best wins.
pub fn best(lines: &VoLines, event: u8, ctx: u32) -> Option<&Entry> {
    let pass: Vec<&Entry> = lines
        .entries
        .iter()
        .filter(|e| e.event == event && passes(e.cond, ctx))
        .take(64)
        .collect();
    let scores: Vec<u32> = pass.iter().map(|e| score(e.cond, ctx)).collect();
    ps2_rank(&scores).first().map(|&i| pass[i])
}

/// The ranking space battles keep: the first of the best among the first
/// 64 entries of the event.
fn best_space(lines: &VoLines, event: u8, ctx: u32) -> Option<&Entry> {
    let mut out: Option<(&Entry, u32)> = None;
    for e in lines
        .entries
        .iter()
        .filter(|e| e.event == event)
        .take(64)
        .filter(|e| passes(e.cond, ctx))
    {
        let s = score(e.cond, ctx);
        if out.is_none_or(|(_, b)| s > b) {
            out = Some((e, s));
        }
    }
    out.map(|(e, _)| e)
}

/// The order the PS2's library quicksort leaves `scores` in, as indices:
/// the 4.4BSD qsort (insertion sort under 7, median of three over 7,
/// ninther over 40, the swap-count fallback to insertion sort) with the VO
/// comparator, which ranks by score and then by the CURRENT slot, so the
/// result depends on the sort's moves and not only on the scores.
fn ps2_rank(scores: &[u32]) -> Vec<usize> {
    let mut v: Vec<usize> = (0..scores.len()).collect();
    if v.len() > 1 {
        qsort(&mut v, scores, 0, scores.len());
    }
    v
}

/// The comparator over slots `i` and `j`: higher score first, then the
/// higher slot first.
fn slot_cmp(v: &[usize], s: &[u32], i: usize, j: usize) -> i32 {
    let (a, b) = (s[v[i]], s[v[j]]);
    match (a.cmp(&b), i.cmp(&j)) {
        (std::cmp::Ordering::Less, _) => 1,
        (std::cmp::Ordering::Greater, _) => -1,
        (_, std::cmp::Ordering::Less) => 1,
        (_, std::cmp::Ordering::Greater) => -1,
        _ => 0,
    }
}

fn med3(v: &[usize], s: &[u32], a: usize, b: usize, c: usize) -> usize {
    let cmp = |i, j| slot_cmp(v, s, i, j);
    if cmp(a, b) < 0 {
        if cmp(b, c) < 0 {
            b
        } else if cmp(a, c) < 0 {
            c
        } else {
            a
        }
    } else if cmp(b, c) > 0 {
        b
    } else if cmp(a, c) < 0 {
        a
    } else {
        c
    }
}

fn insertion(v: &mut [usize], s: &[u32], a: usize, n: usize) {
    for m in a + 1..a + n {
        let mut l = m;
        while l > a && slot_cmp(v, s, l - 1, l) > 0 {
            v.swap(l, l - 1);
            l -= 1;
        }
    }
}

fn vecswap(v: &mut [usize], a: usize, b: usize, n: usize) {
    for k in 0..n {
        v.swap(a + k, b + k);
    }
}

/// Sorts the `n` slots from `a` (the whole array stays addressed, as the
/// comparator reads absolute slots).
fn qsort(v: &mut [usize], s: &[u32], mut a: usize, mut n: usize) {
    loop {
        if n < 7 {
            insertion(v, s, a, n);
            return;
        }
        let mut pm = a + n / 2;
        if n > 7 {
            let (mut pl, mut pn) = (a, a + n - 1);
            if n > 40 {
                let d = n / 8;
                pl = med3(v, s, pl, pl + d, pl + 2 * d);
                pm = med3(v, s, pm - d, pm, pm + d);
                pn = med3(v, s, pn - 2 * d, pn - d, pn);
            }
            pm = med3(v, s, pl, pm, pn);
        }
        v.swap(a, pm);
        let (mut pa, mut pb) = (a + 1, a + 1);
        let (mut pc, mut pd) = (a + n - 1, a + n - 1);
        let mut swapped = false;
        loop {
            while pb <= pc {
                let r = slot_cmp(v, s, pb, a);
                if r > 0 {
                    break;
                }
                if r == 0 {
                    swapped = true;
                    v.swap(pa, pb);
                    pa += 1;
                }
                pb += 1;
            }
            while pb <= pc {
                let r = slot_cmp(v, s, pc, a);
                if r < 0 {
                    break;
                }
                if r == 0 {
                    swapped = true;
                    v.swap(pc, pd);
                    pd -= 1;
                }
                pc -= 1;
            }
            if pb > pc {
                break;
            }
            v.swap(pb, pc);
            swapped = true;
            pb += 1;
            pc -= 1;
        }
        if !swapped {
            insertion(v, s, a, n);
            return;
        }
        let pn = a + n;
        let r = (pa - a).min(pb - pa);
        vecswap(v, a, pb - r, r);
        let r = (pd - pc).min(pn - pd - 1);
        vecswap(v, pb, pn - r, r);
        if pb - pa > 1 {
            qsort(v, s, a, pb - pa);
        }
        let right = pd - pc;
        if right <= 1 {
            return;
        }
        a = pn - right;
        n = right;
    }
}

/// One side of a context: where a unit stands, its facing, what it is.
#[derive(Debug, Clone, Copy, Default)]
pub struct Body {
    pub pos: Vec3,
    pub yaw: f32,
    /// What it is or sits in (the `KIND_*` values; 0 a soldier on foot).
    pub kind: u32,
    pub unit_type: u8,
}

/// The facing test's cosine: the PS2's own literal 0.7071 (271613, 271617),
/// not 1/sqrt 2.
#[allow(clippy::approx_constant)]
const SIDE_COS: f32 = 0.7071;

/// The context word of `subject` toward `object` (`FUN_002e5c08`,
/// `FUN_002e5920` 271546-271649). No object: 0.
pub fn context(subject: &Body, object: Option<&Body>) -> u32 {
    let Some(o) = object else { return 0 };
    let d = o.pos - subject.pos;
    let dist = d.length().max(0.1);
    let mut w = 0u32;
    if d.y > 3.0 && d.y / dist > 0.25 {
        w |= ABOVE;
    } else if d.y < -3.0 && d.y / dist < -0.25 {
        w |= BELOW;
    }
    // [D] The subject's own frame (`core::unit::Unit::aim_frame`: yaw 0
    // faces -Z, right (cos yaw, 0, -sin yaw)); the PS2 tests its facing row
    // at +-0.7071 and then the side row's sign.
    let n = d / dist;
    let (s, c) = core::fmath::sin_cos(subject.yaw);
    let ahead = -n.x * s - n.z * c;
    if ahead < -SIDE_COS {
        w |= BEHIND;
    } else if ahead < SIDE_COS {
        w |= if n.x * c - n.z * s > 0.0 { RIGHT } else { LEFT };
    }
    w |= if dist < 10.0 {
        UNDER_10
    } else if dist < 30.0 {
        UNDER_30
    } else if dist > 100.0 {
        OVER_100
    } else {
        0
    };
    w |= o.kind.min(7) << 8;
    // As read (271741-271760): the subject in a walker, flyer or hover
    // writes the OBJECT's kind field; only a turret seat writes its own.
    match subject.kind {
        KIND_FLYER | KIND_HOVER | KIND_WALKER => w = (w & !0x700) | (subject.kind << 8),
        KIND_TURRET => w |= KIND_TURRET << 11,
        _ => {}
    }
    w | (u32::from(o.unit_type) << 16)
}

/// One line asked: who says it and the entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub speaker: usize,
    pub event: u8,
    pub sound: String,
    /// The speaker IS the subject: played at once and 2-D (272041-272079).
    pub own: bool,
}

/// What the speaker rule needs of one unit.
#[derive(Debug, Clone)]
pub struct Who<'a> {
    pub alive: bool,
    pub team: u8,
    pub body: Body,
    pub lines: &'a VoLines,
}

/// The battle's VO state: its draws, the human's spree, each unit's
/// slaughter counters.
#[derive(Debug, Clone, Default)]
pub struct Vo {
    seed: u32,
    /// The human's kill count in the current spree, and the last kill's time.
    spree: (u32, f32),
    /// Per unit, per kind (enemy, friendly, team kill): four death times.
    slaughter: Vec<[[f32; 4]; 3]>,
    /// A space battle: lines keep the ranking they had before the PS2's
    /// sort was followed (space modes are tuned separately).
    pub space: bool,
}

impl Vo {
    pub fn new(seed: u32) -> Vo {
        Vo {
            seed,
            ..Vo::default()
        }
    }

    /// `FUN_002a3f08(0, 100)`: 0..=100.
    fn roll(&mut self) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(0x0019_660D)
            .wrapping_add(0x3C6E_F35F);
        (self.seed >> 8) % 101
    }

    /// The entry point and the speaker rule (`FUN_002e6778` 272148-272200,
    /// `FUN_002e6180` 271860-272084). `subject` None = the player's unit
    /// (`player`; dead or absent, nothing is said).
    pub fn ask(
        &mut self,
        units: &[Who],
        player: Option<usize>,
        event: u8,
        speaker: Option<usize>,
        subject: Option<usize>,
        object: Option<usize>,
    ) -> Option<Said> {
        if !ASKED.contains(&event) {
            return None;
        }
        if (event == ACQUIRED_TARGET || event == PATH_FOLLOW_WAIT) && self.roll() >= ACQUIRED_KEPT {
            return None;
        }
        let subject = subject.or(player).filter(|&i| units[i].alive)?;
        let sub = &units[subject];
        let ctx = context(&sub.body, object.map(|o| &units[o].body));
        let speaker = match speaker {
            Some(s) => s,
            None => {
                (0..units.len())
                    .filter(|&i| i != subject && units[i].alive && units[i].team == sub.team)
                    .map(|i| (i, (units[i].body.pos - sub.body.pos).length()))
                    .filter(|&(i, d)| d < units[i].lines.radius)
                    .min_by(|a, b| a.1.total_cmp(&b.1))?
                    .0
            }
        };
        let sp = &units[speaker];
        if !sp.alive || (sp.body.pos - sub.body.pos).length() > sp.lines.radius {
            return None;
        }
        if sp.team != sub.team && self.roll() >= ENEMY_SPEAKS && event != REBELS_SHOOT_DEAD_BODY {
            return None;
        }
        let e = if self.space {
            best_space(sp.lines, event, ctx)?
        } else {
            best(sp.lines, event, ctx)?
        };
        Some(Said {
            speaker,
            event,
            sound: e.sound.clone(),
            own: speaker == subject,
        })
    }

    /// The events of the tick `sim` just stepped, as lines (in the order
    /// asked). `units` is the sim's units, index for index; `player` the
    /// local player's unit, the subject when an event names none.
    pub fn tick(&mut self, sim: &Sim, units: &[Who], player: Option<usize>) -> Vec<Said> {
        let now = sim.tick as f32 * core::TICK_DT;
        let mut out = Vec::new();
        if self.slaughter.len() < units.len() {
            self.slaughter.resize(units.len(), [[-1000.0; 4]; 3]);
        }
        // The slot of `h`, dead or alive (a kill's victim is dead by now).
        let idx = |h: core::unit::UnitHandle| {
            let i = h.index as usize;
            sim.units
                .get(i)
                .is_some_and(|u| u.gen == h.gen)
                .then_some(i)
                .filter(|&i| i < units.len())
        };
        for e in &sim.events {
            match *e {
                Event::Call { unit, call } => {
                    let Some(i) = idx(unit) else { continue };
                    let (event, object) = match call {
                        core::sim::Call::Acquired => (
                            ACQUIRED_TARGET,
                            sim.units[i].ai.target.and_then(|t| t.unit()).and_then(idx),
                        ),
                        core::sim::Call::Flee => (GRENADE, None),
                        _ => continue,
                    };
                    out.extend(self.ask(units, player, event, Some(i), None, object));
                }
                Event::Hit {
                    shooter,
                    victim,
                    health_left,
                    ..
                } => {
                    // FriendlyFire (`FUN_002c25e0` 247644): the attacker's
                    // team holds the victim's as a friend; the victim says it.
                    let (Some(s), Some(v)) = (idx(shooter), idx(victim)) else {
                        continue;
                    };
                    if s != v && units[s].team == units[v].team && health_left > 0.0 {
                        out.extend(self.ask(units, player, FRIENDLY_FIRE, Some(v), None, Some(s)));
                    }
                }
                Event::Kill { shooter, victim } => {
                    let (Some(s), Some(v)) = (idx(shooter), idx(victim)) else {
                        continue;
                    };
                    out.extend(self.slaughter(sim, units, player, now, v, s));
                    // The sprees (`FUN_00362268` 359333-359344): a HUMAN's
                    // kills of enemies, each within 4 s of the last.
                    if sim.units[s].is_player && units[s].team != units[v].team {
                        let (n, last) = self.spree;
                        let n = if now - last <= SPREE_GAP { n + 1 } else { 1 };
                        self.spree = (n, now);
                        let event = match n {
                            4 => KILLING_SPREE4,
                            8 => KILLING_SPREE8,
                            _ => continue,
                        };
                        out.extend(self.ask(units, player, event, Some(s), Some(s), Some(v)));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// One death's slaughter counters (`FUN_002b5240` 237348): every AI
    /// unit within 25 m of the victim files it as an enemy's death, a
    /// friend's at an enemy's hand, or a team kill; four of one kind inside
    /// 8 s ask that kind's line - the subject that unit, no speaker named
    /// (its nearest team-mate says it) - and clear its counter.
    fn slaughter(
        &mut self,
        sim: &Sim,
        units: &[Who],
        player: Option<usize>,
        now: f32,
        victim: usize,
        killer: usize,
    ) -> Vec<Said> {
        let at = units[victim].body.pos;
        let mut out = Vec::new();
        for i in 0..units.len() {
            let u = &units[i];
            if !u.alive
                || sim.units[i].controller != core::unit::Controller::Ai
                || (u.body.pos - at).length() > SLAUGHTER_REACH
            {
                continue;
            }
            let (k, event) = if units[victim].team != u.team {
                (0, ENEMY_SLAUGHTER)
            } else if units[killer].team != u.team {
                (1, FRIENDLY_SLAUGHTER)
            } else {
                (2, TEAMKILL_SLAUGHTER)
            };
            let slots = &mut self.slaughter[i][k];
            let old = now - SLAUGHTER_WINDOW;
            if slots[0] < old {
                slots[0] = now;
            } else if let Some(s) = slots[1..].iter_mut().find(|t| **t < old) {
                *s = now;
            }
            let run = slots.iter().take_while(|t| **t >= old).count();
            if run == 4 {
                *slots = [-1000.0; 4];
                out.extend(self.ask(units, player, event, None, Some(i), None));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(entries: &[&str]) -> VoLines {
        VoLines {
            entries: entries.iter().filter_map(|v| parse_entry(v)).collect(),
            unit_type: 0,
            radius: VO_RADIUS,
        }
    }

    fn who(lines: &VoLines, team: u8, x: f32) -> Who<'_> {
        Who {
            alive: true,
            team,
            body: Body {
                pos: Vec3::new(x, 0.0, 0.0),
                ..Body::default()
            },
            lines,
        }
    }

    #[test]
    fn multibyte_unknown_condition_is_ignored() {
        assert_eq!(
            parse_entry("s FriendlyFire VOUnitNué1"),
            parse_entry("s FriendlyFire")
        );
    }

    #[test]
    fn the_event_table_names_the_ps2s_indices() {
        assert_eq!(event_index("AcquiredTarget"), 1);
        assert_eq!(event_index("acquiredtarget"), 1);
        assert_eq!(event_index("NearbyEnemySlaughter"), 0x24);
        assert_eq!(event_index("SC_GetOut"), 0x1f);
        // The `...VO` requests are not in the table: index 0, never asked.
        assert_eq!(event_index("SpottedVO"), 0);
        assert_eq!(event_index("NeedMedicVO"), 0);
    }

    #[test]
    fn an_entry_parses_its_event_and_conditions() {
        let e = parse_entry("rep1_x      AcquiredTarget +Above +VOUnitNum41").expect("entry");
        assert_eq!((e.sound.as_str(), e.event), ("rep1_x", 1));
        assert_eq!(e.cond, ABOVE | (41 << 16));
        let e = parse_entry("a AcquiredTarget +IsSniper +InHover +VOUnitNum084").expect("entry");
        assert_eq!(e.cond, (KIND_SNIPER << 8) | (KIND_HOVER << 11) | (84 << 16));
        // "left" and "Left" are one hash; `InVehicle` is no token.
        let e = parse_entry("a AcquiredTarget left InVehicle").expect("entry");
        assert_eq!(e.cond, LEFT);
    }

    #[test]
    fn the_entry_with_more_matching_conditions_wins_and_a_failing_one_never() {
        let l = lines(&[
            "plain AcquiredTarget",
            "type83 AcquiredTarget +VOUnitNum83",
            "above83 AcquiredTarget +Above +VOUnitNum83",
            "walker AcquiredTarget +IsWalker",
            "other FriendlyFire +VOUnitNum83",
        ]);
        let ctx = 83 << 16;
        assert_eq!(best(&l, 1, ctx).map(|e| e.sound.as_str()), Some("type83"));
        assert_eq!(
            best(&l, 1, ctx | ABOVE).map(|e| e.sound.as_str()),
            Some("above83")
        );
        assert_eq!(best(&l, 1, 0).map(|e| e.sound.as_str()), Some("plain"));
        // A walker object: the walker line, never one of type 83.
        assert_eq!(
            best(&l, 1, KIND_WALKER << 8).map(|e| e.sound.as_str()),
            Some("walker")
        );
        assert!(!passes(l.entries[1].cond, 84 << 16));
        assert!(best(&l, 9, ctx).is_none());
        // A tie goes to the later slot (the PS2's sort and comparator).
        let t = lines(&["a NearbyEnemySlaughter", "b NearbyEnemySlaughter"]);
        assert_eq!(best(&t, 0x24, 0).map(|e| e.sound.as_str()), Some("b"));
    }

    #[test]
    fn space_keeps_the_first_parsed_tie() {
        let t = lines(&["a NearbyEnemySlaughter", "b NearbyEnemySlaughter"]);
        assert_eq!(best_space(&t, 0x24, 0).map(|e| e.sound.as_str()), Some("a"));
        let units = [who(&t, 0, 0.0), who(&t, 0, 5.0)];
        let mut vo = Vo::new(1);
        vo.space = true;
        let s = vo.ask(&units, Some(0), ENEMY_SLAUGHTER, Some(1), None, None);
        assert_eq!(s.map(|s| s.sound), Some("a".to_owned()));
    }

    #[test]
    fn the_ps2_library_sort_orders_ties_by_slot() {
        let order = |s: &[u32]| ps2_rank(s);
        assert_eq!(order(&[1, 1, 0])[0], 1);
        assert_eq!(order(&[1; 6]), [5, 4, 3, 2, 1, 0]);
        assert_eq!(order(&[1; 7]), [6, 5, 4, 0, 2, 1, 3]);
        assert_eq!(order(&[1; 8]), [7, 6, 5, 0, 3, 2, 1, 4]);
        // Seven or more: the winner is not simply the last of the best.
        assert_eq!(order(&[0, 0, 0, 1, 1, 1, 1]), [5, 6, 4, 3, 1, 2, 0]);
        assert_eq!(order(&[0, 0, 1, 1, 0, 0, 0]), [2, 3, 6, 5, 4, 0, 1]);
        assert_eq!(order(&[0, 1, 1, 0, 2, 1, 1, 0]), [4, 6, 5, 2, 1, 0, 3, 7]);
        assert_eq!(
            order(&[1, 0, 1, 0, 1, 0, 1, 0, 1, 0]),
            [0, 8, 6, 4, 2, 7, 5, 3, 1, 9]
        );
        assert_eq!(order(&[2, 2, 1, 1, 2, 0, 1, 2, 1, 1])[0], 7);
        // Any length sorts by score, highest first.
        for n in [0usize, 1, 2, 40, 41, 64] {
            let s: Vec<u32> = (0..n as u32).map(|i| (i * 7 + 3) % 5).collect();
            let o = order(&s);
            let mut seen = o.clone();
            seen.sort_unstable();
            assert_eq!(seen, (0..n).collect::<Vec<_>>());
            assert!(o.windows(2).all(|w| s[w[0]] >= s[w[1]]), "{n}: {o:?}");
        }
    }

    #[test]
    fn the_cap_counts_passing_entries() {
        let mut v = vec!["x AcquiredTarget +VOUnitNum5"; 70];
        v.push("hit AcquiredTarget +VOUnitNum83");
        let l = lines(&v);
        assert_eq!(best(&l, 1, 83 << 16).map(|e| e.sound.as_str()), Some("hit"));
    }

    #[test]
    fn an_enemy_trooper_names_the_player_hero() {
        let l = lines(&[
            "l AcquiredTarget +left",
            "r AcquiredTarget +right",
            "maul AcquiredTarget +VOUnitNum196",
            "plain AcquiredTarget",
        ]);
        let hero = Body {
            unit_type: 196,
            ..Body::default()
        };
        // The subject is the player and so is the object: LEFT, under 10 m.
        let ctx = context(&hero, Some(&hero));
        assert_eq!(best(&l, 1, ctx).map(|e| e.sound.as_str()), Some("maul"));
        let mut player = who(&l, 0, 0.0);
        player.body.unit_type = 196;
        let units = [player, who(&l, 1, 10.0)];
        let mut said = 0;
        for seed in 0..50 {
            let mut vo = Vo::new(seed);
            if let Some(s) = vo.ask(&units, Some(0), ACQUIRED_TARGET, Some(1), None, Some(0)) {
                assert_eq!(s.sound, "maul");
                said += 1;
            }
        }
        assert!(said > 0);
    }

    #[test]
    fn the_context_reads_height_side_and_distance_from_the_subject() {
        let me = Body::default(); // at the origin facing -Z, +X its right
        let at = |x: f32, y: f32, z: f32| Body {
            pos: Vec3::new(x, y, z),
            unit_type: 9,
            ..Body::default()
        };
        assert_eq!(context(&me, None), 0);
        assert_eq!(
            context(&me, Some(&at(0.0, 0.0, -5.0))),
            UNDER_10 | (9 << 16)
        );
        assert_eq!(context(&me, Some(&at(0.0, 10.0, -20.0))) & 3, ABOVE);
        assert_eq!(context(&me, Some(&at(0.0, -10.0, -20.0))) & 3, BELOW);
        // 2 m up at 20 m: not above (3 m), and 5 m up at 30 m: under 0.25.
        assert_eq!(context(&me, Some(&at(0.0, 2.0, -20.0))) & 3, 0);
        assert_eq!(context(&me, Some(&at(0.0, 5.0, -30.0))) & 3, 0);
        assert_eq!(context(&me, Some(&at(0.0, 0.0, 20.0))) & 0xc, BEHIND);
        // 40 deg off straight behind (cos 0.766): behind; 50 deg: a side.
        assert_eq!(context(&me, Some(&at(12.86, 0.0, 15.32))) & 0xc, BEHIND);
        assert_eq!(context(&me, Some(&at(15.32, 0.0, 12.86))) & 0xc, RIGHT);
        assert_eq!(context(&me, Some(&at(20.0, 0.0, 0.0))) & 0xc, RIGHT);
        assert_eq!(context(&me, Some(&at(-20.0, 0.0, 0.0))) & 0xc, LEFT);
        assert_eq!(context(&me, Some(&at(0.0, 0.0, -50.0))) & 0x30, 0);
        assert_eq!(context(&me, Some(&at(0.0, 0.0, -150.0))) & 0x30, OVER_100);
        // Turned half round, the same spot is behind.
        let turned = Body {
            yaw: std::f32::consts::PI,
            ..Body::default()
        };
        assert_eq!(context(&turned, Some(&at(0.0, 0.0, -20.0))) & 0xc, BEHIND);
        // The object's kind; a subject in a turret sets its own seat field.
        let walker = Body {
            kind: KIND_WALKER,
            ..at(0.0, 0.0, -50.0)
        };
        assert_eq!(context(&me, Some(&walker)) & 0x700, KIND_WALKER << 8);
        let gunner = Body {
            kind: KIND_TURRET,
            ..Body::default()
        };
        assert_eq!(
            context(&gunner, Some(&walker)) & 0x3f00,
            (KIND_TURRET << 11) | (KIND_WALKER << 8)
        );
    }

    #[test]
    fn only_a_speaker_within_its_radius_of_the_subject_speaks() {
        let l = lines(&["line FriendlyFire"]);
        let units = [who(&l, 0, 0.0), who(&l, 0, 40.0), who(&l, 0, 60.0)];
        let mut vo = Vo::new(1);
        let said = vo.ask(&units, Some(0), FRIENDLY_FIRE, Some(1), None, None);
        assert_eq!(said.map(|s| s.speaker), Some(1));
        assert!(vo
            .ask(&units, Some(0), FRIENDLY_FIRE, Some(2), None, None)
            .is_none());
        // No player unit: nothing.
        assert!(vo
            .ask(&units, None, FRIENDLY_FIRE, Some(1), None, None)
            .is_none());
    }

    #[test]
    fn with_no_speaker_the_subjects_nearest_team_mate_in_reach_speaks() {
        let l = lines(&["line NearbyEnemySlaughter"]);
        let units = [
            who(&l, 0, 0.0),
            who(&l, 1, 5.0),
            who(&l, 0, 30.0),
            who(&l, 0, 20.0),
            who(&l, 0, 70.0),
        ];
        let mut vo = Vo::new(1);
        let s = vo.ask(&units, Some(0), ENEMY_SLAUGHTER, None, Some(0), None);
        assert_eq!(s.map(|s| (s.speaker, s.own)), Some((3, false)));
        // Alone in reach: no team-mate, no line.
        let alone = [who(&l, 0, 0.0), who(&l, 1, 5.0), who(&l, 0, 70.0)];
        assert!(vo
            .ask(&alone, Some(0), ENEMY_SLAUGHTER, None, Some(0), None)
            .is_none());
    }

    #[test]
    fn an_enemy_speaker_goes_on_31_times_in_101() {
        let l = lines(&["line FriendlyFire"]);
        let units = [who(&l, 0, 0.0), who(&l, 1, 10.0), who(&l, 0, 10.0)];
        let mut vo = Vo::new(7);
        let n = 2000;
        let enemy = (0..n)
            .filter(|_| {
                vo.ask(&units, Some(0), FRIENDLY_FIRE, Some(1), None, None)
                    .is_some()
            })
            .count();
        let friend = (0..n)
            .filter(|_| {
                vo.ask(&units, Some(0), FRIENDLY_FIRE, Some(2), None, None)
                    .is_some()
            })
            .count();
        let want = n as f64 * 31.0 / 101.0;
        assert!(
            (enemy as f64 - want).abs() < 60.0,
            "{enemy} of {n}, want ~{want:.0}"
        );
        assert_eq!(friend, n);
        // AcquiredTarget is kept 71 in 101 at the entry point.
        let l2 = lines(&["line AcquiredTarget"]);
        let units = [who(&l2, 0, 0.0), who(&l2, 0, 10.0)];
        let kept = (0..n)
            .filter(|_| {
                vo.ask(&units, Some(0), ACQUIRED_TARGET, Some(1), None, None)
                    .is_some()
            })
            .count();
        let want = n as f64 * 71.0 / 101.0;
        assert!(
            (kept as f64 - want).abs() < 60.0,
            "{kept} of {n}, want ~{want:.0}"
        );
    }

    /// A sim of `(team, human, x)` units on a line, and their handles.
    fn sim_of(units: &[(u8, bool, f32)]) -> (Sim, Vec<core::unit::UnitHandle>) {
        use core::unit::{Controller, UnitSpec};
        let mut sim = Sim::empty(5);
        let hs = units
            .iter()
            .map(|&(team, human, x)| {
                let pos = Vec3::new(x, 0.0, 0.0);
                let spec = if human {
                    UnitSpec::player(team, pos)
                } else {
                    UnitSpec {
                        controller: Controller::Ai,
                        ..UnitSpec::trooper(team, pos)
                    }
                };
                sim.spawn(&spec).expect("spawn")
            })
            .collect();
        (sim, hs)
    }

    fn whos<'a>(sim: &Sim, l: &'a VoLines) -> Vec<Who<'a>> {
        sim.units
            .iter()
            .map(|u| Who {
                alive: u.alive,
                team: u.team,
                body: Body {
                    pos: u.soldier.pos,
                    ..Body::default()
                },
                lines: l,
            })
            .collect()
    }

    /// Kills by `shooter` at the given seconds; the lines said, by second.
    fn kills(
        vo: &mut Vo,
        sim: &mut Sim,
        l: &VoLines,
        shooter: core::unit::UnitHandle,
        victims: &[core::unit::UnitHandle],
        at: &[f32],
    ) -> Vec<(f32, Said)> {
        let mut out = Vec::new();
        for (&t, &victim) in at.iter().zip(victims.iter().cycle()) {
            sim.tick = (t * core::TICK_HZ as f32).round() as u64;
            sim.events = vec![Event::Kill { shooter, victim }];
            sim.units[victim.index as usize].alive = false;
            let who = whos(sim, l);
            out.extend(vo.tick(sim, &who, Some(0)).into_iter().map(|s| (t, s)));
        }
        out
    }

    #[test]
    fn a_humans_fourth_kill_inside_4_s_steps_asks_his_spree_line() {
        let l = lines(&["s4 KillingSpree4", "s8 KillingSpree8"]);
        let (mut sim, h) = sim_of(&[
            (0, true, 0.0),
            (0, false, 5.0),
            (1, false, 60.0),
            (1, false, 70.0),
        ]);
        let (human, bot, victims) = (h[0], h[1], &h[2..]);
        let spree = |said: &[(f32, Said)]| -> Vec<(f32, u8, usize, bool)> {
            said.iter()
                .filter(|(_, s)| s.event == KILLING_SPREE4 || s.event == KILLING_SPREE8)
                .map(|(t, s)| (*t, s.event, s.speaker, s.own))
                .collect()
        };
        let mut vo = Vo::new(1);
        let said = kills(
            &mut vo,
            &mut sim,
            &l,
            human,
            victims,
            &[1.0, 4.0, 7.5, 11.0, 12.0, 13.0, 14.0, 17.9],
        );
        assert_eq!(
            spree(&said),
            vec![
                (11.0, KILLING_SPREE4, 0, true),
                (17.9, KILLING_SPREE8, 0, true)
            ]
        );
        // A gap over 4 s starts again: the fourth kill counted is no spree.
        let mut vo = Vo::new(1);
        let said = kills(
            &mut vo,
            &mut sim,
            &l,
            human,
            victims,
            &[30.0, 33.0, 37.5, 40.0],
        );
        assert!(spree(&said).is_empty(), "{said:?}");
        // An AI's kills never.
        let mut vo = Vo::new(1);
        let said = kills(
            &mut vo,
            &mut sim,
            &l,
            bot,
            victims,
            &[50.0, 51.0, 52.0, 53.0],
        );
        assert!(spree(&said).is_empty(), "{said:?}");
    }

    #[test]
    fn four_enemy_deaths_inside_8_s_near_a_bot_ask_its_team_mates_line() {
        let l = lines(&["slay NearbyEnemySlaughter", "fslay NearbyFriendlySlaughter"]);
        // Two bots 10 m apart, an enemy killer and its victims 3 m off, a
        // bot 40 m off (outside 25 m of the deaths).
        let (mut sim, h) = sim_of(&[
            (0, true, -200.0),
            (0, false, 0.0),
            (0, false, 10.0),
            (1, false, 3.0),
            (1, false, 3.0),
            (0, false, 43.0),
        ]);
        let mut vo = Vo::new(1);
        // Our side kills them: the enemy's deaths.
        let said = kills(&mut vo, &mut sim, &l, h[1], &h[3..5], &[1.0, 2.0, 3.0, 8.5]);
        let rows: Vec<_> = said
            .iter()
            .map(|(t, s)| (*t, s.speaker, s.sound.as_str()))
            .collect();
        // At the fourth (inside 8 s of the first) each bot within 25 m asks,
        // and its nearest team-mate speaks: bot 1's is bot 2, bot 2's bot 1.
        assert_eq!(rows, vec![(8.5, 2, "slay"), (8.5, 1, "slay")], "{said:?}");
        // The counters were cleared: three more are not four.
        let said = kills(&mut vo, &mut sim, &l, h[1], &h[3..5], &[9.0, 10.0, 11.0]);
        assert!(said.is_empty(), "{said:?}");
        // Spread over more than 8 s: never four inside the window.
        let mut vo = Vo::new(1);
        let said = kills(
            &mut vo,
            &mut sim,
            &l,
            h[1],
            &h[3..5],
            &[20.0, 23.0, 26.0, 29.0, 32.0],
        );
        assert!(said.is_empty(), "{said:?}");
        // Our own dead at an enemy's hand: the friendly line, from the two
        // bots within 25 m (each the other's nearest team-mate); deaths
        // 30 m from every bot count for none.
        let (mut sim, h) = sim_of(&[
            (0, true, -200.0),
            (0, false, 0.0),
            (0, false, 10.0),
            (0, false, 20.0),
            (0, false, 50.0),
            (1, false, 3.0),
        ]);
        let mut vo = Vo::new(1);
        let said = kills(
            &mut vo,
            &mut sim,
            &l,
            h[5],
            &[h[4]],
            &[40.0, 41.0, 42.0, 43.0],
        );
        assert!(said.is_empty(), "the dead 30 m off are not near: {said:?}");
        let said = kills(
            &mut vo,
            &mut sim,
            &l,
            h[5],
            &[h[3]],
            &[50.0, 51.0, 52.0, 53.0],
        );
        let rows: Vec<_> = said
            .iter()
            .map(|(_, s)| (s.speaker, s.sound.as_str()))
            .collect();
        assert_eq!(rows, vec![(2, "fslay"), (1, "fslay")], "{said:?}");
    }

    #[test]
    fn a_unit_hit_by_a_friend_says_so_and_one_hit_by_an_enemy_does_not() {
        let l = lines(&["ff FriendlyFire"]);
        let (mut sim, h) = sim_of(&[
            (0, true, 0.0),
            (0, false, 10.0),
            (0, false, 20.0),
            (1, false, 30.0),
        ]);
        let mut vo = Vo::new(1);
        let hit = |shooter, victim, health_left| Event::Hit {
            direct: true,
            shooter,
            victim,
            damage: 10.0,
            age: 0.1,
            health_left,
            dir: glam::Vec3::NEG_Z,
        };
        sim.events = vec![
            hit(h[2], h[1], 200.0),
            hit(h[3], h[2], 200.0),
            hit(h[2], h[1], 0.0),
        ];
        let who = whos(&sim, &l);
        let said: Vec<_> = vo
            .tick(&sim, &who, Some(0))
            .into_iter()
            .map(|s| s.speaker)
            .collect();
        // The victim of the friend's hit speaks; the enemy's hit and the
        // killing hit ask nothing.
        assert_eq!(said, vec![1]);
    }

    #[test]
    fn a_vo_request_name_and_missile_incoming_are_never_asked() {
        // Entries authored under the eight request names sit at index 0 and
        // MissileIncoming at 5: no trigger of the port passes either.
        let l = lines(&["s SpottedVO", "m MissileIncoming", "f FriendlyFire"]);
        assert_eq!(l.entries[0].event, 0);
        assert_eq!(l.entries[1].event, 5);
        let units = [who(&l, 0, 0.0), who(&l, 0, 10.0)];
        let mut vo = Vo::new(3);
        for event in [0, 5] {
            assert!(vo
                .ask(&units, Some(0), event, Some(1), None, None)
                .is_none());
            assert!(!ASKED.contains(&event));
        }
        assert!(vo
            .ask(&units, Some(0), FRIENDLY_FIRE, Some(1), None, None)
            .is_some());
    }
}
