//! Perception (Phase C decision 2): what an AI unit KNOWS about its enemies,
//! and the rule that makes the top tiers fair — a bot may aim or fire only at
//! a contact its perception holds.
//!
//! The shapes are the PS2's (`analysis/re-ai-difficulty.md` §5.1/§5.2/§9.1,
//! quoted by the lead in `engine/artifacts/briefs/brief_phase_c.md` D6): a
//! bounded memory of six contacts per unit, a 77.5° half-angle view cone with
//! BF2's 10 m omni-directional bubble, a view distance of the shape
//! `(20·bound_radius + 30) × mult × visibility`, and a line-of-sight ray that
//! is the SAME one the AI's shots have to fly through. The tier columns that
//! scale them (`view_mult`, `reaction_ticks`, `pressure`) are the lead's
//! PROVISIONAL placeholders in [`crate::skill`], not RE. OURS: a bot standing
//! still with a scoped rifle sees out to the rifle's range ([`sight`]).
//! OURS (playtest C1): a bot on foot hit by a shooter it does not see must
//! find him before its reaction time starts ([`search_ticks`]).
//!
//! Nothing here reads wall time or the LCG: a scan is a pure function of the
//! sim state, so it replays bit-exactly.

use crate::ai::{AiState, THINK_PERIOD};
use crate::collision::HullFilter;
use crate::sim::Sim;
use crate::skill::Tier;
use crate::soldier::Stance;
use crate::unit::{capsule_height, UnitHandle};
use glam::Vec3;

/// Threat slots per unit (the PS2's six, RE §5.2). The array keeps
/// [`AiState`] `Copy`.
pub const CONTACTS: usize = 6;

/// cos 77.5° — the view cone's half-angle in BOTH games (RE §9.1). The
/// constant is cited, not computed from a degree value at runtime.
pub const COS_CONE: f32 = 0.21644;

/// A contact is forgotten this many ticks after it was last seen or heard
/// (3 s, design decision 2).
pub const MEMORY_TICKS: u64 = 90;

/// A shot fired within this radius is heard (m). PROVISIONAL (lead
/// placeholder, no RE).
pub const HEAR_RADIUS: f32 = 30.0;

/// OURS (playtest C1): a bot on foot hit by a shooter it does not see must
/// find him before its reaction time starts ([`search_ticks`]). Neither
/// original delays the answer to a hit (`analysis/re-ai-difficulty.md` 9.4);
/// without this a bot hit from 40 m just outside its view cone fired back
/// 0.6 s later. The search's fixed part (ticks, 0.3 s).
/// measure: PCSX2 T5/T6 (issue #104 brief): time from a hit by a far human
/// to the bot's first shot, engaged vs idle, within vs beyond view distance.
pub const SEARCH_BASE: f32 = 9.0;
/// OURS: ...plus this many ticks per metre to the shooter.
/// measure: as [`SEARCH_BASE`].
pub const SEARCH_PER_M: f32 = 0.6;
/// OURS: ...up to this many (1.8 s). Below [`MEMORY_TICKS`], so the record
/// outlives its search. measure: as [`SEARCH_BASE`].
pub const SEARCH_CAP: u64 = 54;
/// OURS: the share of the search paid for a shooter the bot already held a
/// record of (heard, told, or seen before). measure: as [`SEARCH_BASE`].
pub const SEARCH_KNOWN: f32 = 0.5;
/// OURS: a shooter the bot's own eyes held this recently (ticks, one think)
/// costs no search: it is where the bot was looking.
/// measure: as [`SEARCH_BASE`].
pub const SEARCH_FRESH: u64 = THINK_PERIOD;

/// The unit grid's query is this much wider than the test it feeds (m): the
/// grid measures the feet in XZ, the tests a body's centre in 3D (card TEAM).
const NEAR_MARGIN: f32 = 1.0;

/// The visibility factor of the view-distance shape (RE §5.1). The port has
/// no concealment byte (RE §5.3), so it is 1.0 — PROVISIONAL.
pub const VIS: f32 = 1.0;

/// Minimum ticks between target switches (1.5 s, RE §5.2).
pub const RETARGET_TICKS: u64 = 45;

/// A new target must beat the current one by this much to take it over
/// (RE §5.2).
pub const HYSTERESIS: f32 = 10.0;

/// What a contact and a target are: an enemy unit, or an enemy remote
/// droid by its id (`analysis/re-remote-droid.md` 6: the PS2's perception
/// holds entities, and the droid is one).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Foe {
    Unit(UnitHandle),
    Droid(u64),
    /// A dispensed auto-turret by its id (an armed building: the scan's
    /// `FUN_002e3aa0` asks `FUN_002a7c30` alive, finite health, hostile, then
    /// the weapon choice; `analysis/re-portable-turret.md` 10).
    Turret(u64),
}

impl Foe {
    pub fn unit(self) -> Option<UnitHandle> {
        match self {
            Foe::Unit(h) => Some(h),
            Foe::Droid(_) | Foe::Turret(_) => None,
        }
    }

    pub fn turret(self) -> Option<u64> {
        match self {
            Foe::Turret(id) => Some(id),
            Foe::Unit(_) | Foe::Droid(_) => None,
        }
    }

    pub fn droid(self) -> Option<u64> {
        match self {
            Foe::Droid(id) => Some(id),
            Foe::Unit(_) | Foe::Turret(_) => None,
        }
    }

    /// The live unit this foe is, if it is one.
    pub fn live_unit(self, sim: &Sim) -> Option<&crate::unit::Unit> {
        self.unit().and_then(|h| sim.unit(h))
    }
}

impl From<UnitHandle> for Foe {
    fn from(h: UnitHandle) -> Self {
        Foe::Unit(h)
    }
}

impl PartialEq<UnitHandle> for Foe {
    fn eq(&self, h: &UnitHandle) -> bool {
        *self == Foe::Unit(*h)
    }
}

/// One remembered enemy. `last_pos` is where it was when it was last SEEN or
/// HEARD — the point a unit that lost sight aims and walks at.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Contact {
    pub handle: Foe,
    /// Tick the slot was created — the reaction timer's zero. A hit by a
    /// shooter the bot cannot see moves it on by the search
    /// ([`search_ticks`]), so it can lie in the future.
    pub first_seen: u64,
    /// Tick of the last sighting or hearing.
    pub last_seen: u64,
    pub last_pos: Vec3,
    /// Seen by THIS think (eyes on it right now).
    pub seen_now: bool,
    /// The last record came from hearing, not sight.
    pub heard: bool,
    /// Card TEAM: ...and from a FRIEND's sight ([`tell`]), not this unit's
    /// own eyes or ears. A told record is aimed at, but not by a unit
    /// walking a leg (playtest C3); it does not keep the unit on a position
    /// (`ai::behave`).
    pub told: bool,
}

impl Contact {
    /// Feed into the sim hash. Every field: two units whose memories differ
    /// are different worlds even when they are looking at the same enemy.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        match self.handle {
            Foe::Unit(h) => {
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
            Foe::Turret(id) => {
                eat(b"T");
                eat(&id.to_le_bytes());
            }
            Foe::Droid(id) => {
                eat(b"DROID");
                eat(&id.to_le_bytes());
            }
        }
        eat(&self.first_seen.to_le_bytes());
        eat(&self.last_seen.to_le_bytes());
        for v in [self.last_pos.x, self.last_pos.y, self.last_pos.z] {
            eat(&v.to_bits().to_le_bytes());
        }
        eat(&[
            self.seen_now as u8,
            self.heard as u8 | (self.told as u8) << 1,
        ]);
    }

    /// Age in seconds since the last record.
    pub fn age_s(&self, tick: u64) -> f32 {
        tick.saturating_sub(self.last_seen) as f32 / crate::TICK_HZ as f32
    }
}

/// The AI's line of sight: the terrain sheet first, then the world's
/// `coll`/`prim` bodies on the ORDNANCE layer — the same two tests a bolt
/// flies through (`analysis/re-collision-layers.md` §4, `FUN_00162E88`).
///
/// This was `ai::visible`; Phase C moved it here so the sweep's diagnostic
/// column and the AI cannot drift apart again (brief D5).
///
/// Every vehicle hull blocks it (card VHULL); [`los_past`] skips some.
pub fn los(sim: &Sim, from: Vec3, to: Vec3) -> bool {
    los_past(sim, from, to, HullFilter::All)
}

/// [`los`] through the hulls `hulls` does not admit — a rider's own vehicle
/// and its target's, whose centres sit inside them.
pub fn los_past(sim: &Sim, from: Vec3, to: Vec3, hulls: HullFilter) -> bool {
    los_seat(sim, from, to, hulls, [None; 2])
}

/// [`los_past`] that also passes the map bodies of the instances in
/// `buildings` ([`past_buildings`]).
pub fn los_seat(
    sim: &Sim,
    from: Vec3,
    to: Vec3,
    hulls: HullFilter,
    buildings: [Option<u32>; 2],
) -> bool {
    los_terrain(sim, from, to)
        && match ray(from, to) {
            Some((dir, len)) => !sim.collision.ray_blocked(
                from,
                dir,
                len,
                crate::collision::Layers::ORDNANCE,
                hulls,
                buildings,
            ),
            None => true,
        }
}

/// May bot `me`, its eye at `eye`, PITCH its gun at `p`? Only if the sight
/// ray is clear: the same terrain + ORDNANCE test as [`scan`] and the bolt
/// ([`los_seat`], past its own mount). The foot brain asks this before it
/// aims at a point it does not see — a heard shot, a friend's report, a
/// lost contact, the holder's nearest enemy — so nobody stares through a
/// floor or a wall (playtest C4, BF1 Kashyyyk Docks: a platform sniper aimed
/// straight down through its floor at the fight below). OURS: the PS2 has
/// neither hearing nor a nearest-enemy stare to match.
pub fn can_aim(sim: &Sim, me: UnitHandle, eye: Vec3, p: Vec3) -> bool {
    los_seat(
        sim,
        eye,
        p,
        past_mounts(sim, me, me),
        past_buildings(sim, me, me),
    )
}

/// The point `p` brought to the eye's height: the yaw-only look a bot takes
/// toward something it cannot see ([`can_aim`] false) — it faces that way
/// with the gun level.
pub fn level(eye: Vec3, p: Vec3) -> Vec3 {
    Vec3::new(p.x, eye.y, p.z)
}

/// Both vehicles `a` and `b` ride in, skipped ([`HullFilter::Except`]).
pub fn past_mounts(sim: &Sim, a: UnitHandle, b: UnitHandle) -> HullFilter {
    let m = |h| sim.unit(h).and_then(|u| u.mount).map(|m| m.vehicle);
    HullFilter::Except([m(a), m(b)])
}

/// The buildings of the building turrets `a` and `b` ride in
/// (`Vehicle::instance`), whose map bodies their sight passes (card TURR):
/// a gunner's eye sits inside his building's mesh (BF1 Geonosis'
/// geoturret, `EyePointOffset 0 0.5 0`, saw no enemy in 15 seated spells),
/// as a seated target does. OURS: the PS2's camera ray passes the turret's
/// owner entity (`CollisionWorld::ray_past`); its AI sight ray from a seat
/// is not traced.
pub fn past_buildings(sim: &Sim, a: UnitHandle, b: UnitHandle) -> [Option<u32>; 2] {
    let m = |h| {
        let vh = sim.unit(h)?.mount?.vehicle;
        sim.vehicle(vh)?.instance
    };
    [m(a), m(b)]
}

/// The terrain half of [`los`] — `true` when the heightfield does not block.
pub fn los_terrain(sim: &Sim, from: Vec3, to: Vec3) -> bool {
    match ray(from, to) {
        Some((dir, len)) => sim.terrain_ray(from, dir, len, 1.0).is_none(),
        None => true,
    }
}

pub fn reach(sim: &Sim, from: Vec3, dir: Vec3, len: f32) -> f32 {
    let t = sim.terrain_ray(from, dir, len, 1.0).unwrap_or(len);
    sim.collision
        .ray_with(
            from,
            dir,
            t,
            crate::collision::Layers::ORDNANCE,
            HullFilter::All,
        )
        .map_or(t, |h| h.t)
}

/// The model half of [`los`] — `true` when no `coll`/`prim` body blocks.
pub fn los_world(sim: &Sim, from: Vec3, to: Vec3) -> bool {
    los_world_past(sim, from, to, HullFilter::All)
}

/// [`los_world`] seeing only the hulls `hulls` admits.
pub fn los_world_past(sim: &Sim, from: Vec3, to: Vec3, hulls: HullFilter) -> bool {
    match ray(from, to) {
        Some((dir, len)) => !sim.collision.ray_blocked(
            from,
            dir,
            len,
            crate::collision::Layers::ORDNANCE,
            hulls,
            [None; 2],
        ),
        None => true,
    }
}

/// `(direction, length)`, or `None` for a degenerate ray (which is clear).
fn ray(from: Vec3, to: Vec3) -> Option<(Vec3, f32)> {
    let d = to - from;
    let len = d.length();
    (len >= 1e-3).then(|| (d / len, len))
}

/// The PS2's view distance (RE §5.1): `(20·bound_radius + 30) × mult × vis`.
///
/// PROVISIONAL in one respect: the PS2's bound radius is the target MODEL's,
/// ours is half the capsule height (unit.rs), so a standing soldier is seen
/// at 49 m at mult 1.0, a crouching one at 46 m and a prone one at 35.5 m —
/// about 14 m less than the PS2 would give a prone target.
pub fn view_distance(stance: Stance, view_mult: f32) -> f32 {
    (20.0 * (capsule_height(stance) * 0.5) + 30.0) * view_mult * VIS
}

/// A scoped rifle's sight is used only below this speed (m/s, XZ): a
/// sniper looks through its scope standing, not walking. OURS.
pub const SCOPE_STILL: f32 = 1.0;

/// OURS (2026-10-29, long-range bot sniping; FEEL_NOTES): how far a bot
/// with weapon class `wc` in hand sees a standing soldier through its scope -
/// `share` (the tier's [`Tier::scope_sight`], [`scope_share`]) of the rifle's
/// `MaxRange` (128 m on every disc sniper) times the mission's view
/// multiplier - for a scoped rifle (the AI sniper table's class,
/// `WeaponClass::ai_distance_table`) and a share over 0, else `None`. The
/// PS2's sight stops at [`view_distance`] (49 m standing at Normal), so its
/// snipers never fired past it (re-soldier-ai 20.1) and a player in the open
/// across a map was safe from them.
pub fn scope_reach(sim: &Sim, wc: &crate::weapon::WeaponClass, share: f32) -> Option<f32> {
    (share > 0.0 && wc.ai_distance_table())
        .then(|| wc.ai_range[2] * share * sim.mission_ai.view_mult * VIS)
}

/// The share of a scope's reach unit `u` sees with under `tier`: the tier's
/// [`Tier::scope_sight`]; a proxy (a stand-in for a strong human) Hard's.
pub fn scope_share(u: &crate::unit::Unit, tier: &Tier) -> f32 {
    if u.proxy_human.is_some() {
        crate::skill::Difficulty::Hard.tier().scope_sight
    } else {
        tier.scope_sight
    }
}

/// How far unit `u` sees a soldier in `stance` under `tier`:
/// [`view_distance`], or on foot, standing still ([`SCOPE_STILL`]) with a
/// scoped rifle in hand, its [`scope_reach`] scaled by stance as the plain
/// sight is, if farther.
pub fn sight(sim: &Sim, u: &crate::unit::Unit, stance: Stance, tier: &Tier) -> f32 {
    let plain = view_distance(stance, tier.view_mult);
    let v = u.soldier.vel;
    if u.mount.is_some() || v.x * v.x + v.z * v.z >= SCOPE_STILL * SCOPE_STILL {
        return plain;
    }
    let Some(r) = sim
        .weapon_classes
        .get(u.weapon_class)
        .and_then(|wc| scope_reach(sim, wc, scope_share(u, tier)))
    else {
        return plain;
    };
    plain.max(r * view_distance(stance, 1.0) / view_distance(Stance::Stand, 1.0))
}

/// The view cone (RE §9.1, exactly): everything inside `omni_radius` is seen
/// regardless of facing (BF2's 10 m bubble; BF1 has none), then the 77.5°
/// half-angle test on the squared distance. `off` is target − eye.
pub fn in_cone(view: Vec3, off: Vec3, d2: f32, omni_radius: f32) -> bool {
    if d2 < omni_radius * omni_radius || d2 <= 1e-4 {
        return true;
    }
    let dot = view.dot(off);
    dot > 0.0 && d2 * COS_CONE * COS_CONE < dot * dot
}

/// A contact may be FIRED at once the tier's reaction time has passed since
/// the slot was created, and only while it is seen right now. A contact that
/// was HEARD first counts its reaction from the hearing — a shot you heard is
/// warning, which is what makes a heard contact useful at all. A shooter
/// that hit the bot unseen counts it from the end of the search.
pub fn fireable(c: &Contact, tick: u64, reaction_ticks: u64) -> bool {
    c.seen_now && tick.saturating_sub(c.first_seen) >= reaction_ticks
}

/// This unit's record of `h`, if it holds one.
pub fn contact_of(st: &AiState, h: impl Into<Foe>) -> Option<Contact> {
    let h = h.into();
    st.contacts
        .iter()
        .flatten()
        .find(|c| c.handle == h)
        .copied()
}

/// One perception pass for unit `me`: age the memory, look, listen.
/// Returns `(contacts held, records made by hearing)` for the diagnostics.
///
/// Runs inside the think's `next_think` block, so at most once every
/// [`THINK_PERIOD`] ticks — detection is quantised by that cadence (a unit
/// that walks into view right after a think is seen up to 9 ticks later), and
/// the measured time-to-first-shot is `U(0, 0.33 s) + reaction_s`.
pub fn scan(sim: &Sim, me: UnitHandle, st: &mut AiState, tier: Tier) -> (u8, u8) {
    let Some(u) = sim.unit(me) else {
        return (0, 0);
    };
    let tick = sim.tick;
    // A bot in an armed seat looks from the seat's eye along its gun's look
    // (card VTAI); any other seat, and on foot, the soldier's own. A building
    // turret's gunner looks from its muzzle (card TURR, OURS: the PS2's sight
    // origin for a seat is not traced): the camera's eye stands 8.3 m behind
    // Kashyyyk's recoilless turret, over its tower, and saw 180 of 913
    // enemies within 60 m where the muzzle saw 530.
    let (eye, view) = sim.seat_look(me).map_or_else(
        || (u.ai_eye(), u.aim_frame().0),
        |s| {
            (
                if s.gun.turn.is_some() {
                    s.muzzle
                } else {
                    s.eye
                },
                s.view,
            )
        },
    );

    // 1. Age the memory. Nothing is seen until this think proves it, a record
    //    older than MEMORY_TICKS is forgotten, and a dead or recycled handle
    //    is dropped (a respawn is a new generation, so the slot cannot follow
    //    a unit through death).
    for slot in st.contacts.iter_mut() {
        let gone = match slot {
            Some(c) => {
                c.seen_now = false;
                tick.saturating_sub(c.last_seen) > MEMORY_TICKS || !alive(sim, c.handle)
            }
            None => false,
        };
        if gone {
            *slot = None;
        }
    }

    // 2. Look. The cone and the view distance are cheap and cull first; the
    //    LOS ray is the cost driver, so it runs in ascending distance (unit
    //    index order on ties) and stops once six enemies are seen.
    //    Card TEAM: the candidates are the unit grid's within the farthest
    //    view distance (a standing body's), in unit-index order as before.
    let mut cand: Vec<(f32, (u8, u64), Foe, Vec3)> = Vec::new();
    let mut near = crate::ai::team::NearList::new();
    sim.units_within(
        eye,
        sight(sim, u, Stance::Stand, &tier) + NEAR_MARGIN,
        &mut near,
    );
    for &i in &near {
        let (h, o) = (sim.handle(i as usize), &sim.units[i as usize]);
        if h == me || !sim.hostile(u.team, o.team) {
            continue;
        }
        let p = o.capsule().center();
        let off = p - eye;
        let d2 = off.dot(off);
        if !in_cone(view, off, d2, sim.omni_radius) {
            continue;
        }
        let vd = sight(sim, u, o.soldier.stance, &tier) * o.cloak.visibility();
        if d2 >= vd * vd {
            continue;
        }
        cand.push((d2, (0, u64::from(h.index)), Foe::Unit(h), p));
    }
    // A rider is seen by his craft's body (RE §5.1's bound radius; BF2 doubles
    // a flyer observer and a human target): for a flyer in the sky, and for a
    // flyer pilot's eyes on any vehicle. Ground bots and ground vehicles keep
    // the soldier's sight.
    let pilot = u
        .mount
        .and_then(|m| sim.vehicle(m.vehicle))
        .is_some_and(|v| v.kind == crate::vehicle::VehicleKind::Flyer);
    for (_, v) in sim.alive_vehicles() {
        let flyer = v.kind == crate::vehicle::VehicleKind::Flyer;
        if (!flyer && !pilot) || v.is_empty() || !v.health.alive() {
            continue;
        }
        let radius = sim
            .seat_setup
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .map_or(0.0, |s| s.radius);
        let bf2 = sim.ground_rules == crate::soldier::GroundRules::Bf2;
        let double = pilot && bf2;
        let base = (20.0 * radius + 30.0) * tier.view_mult * if double { 2.0 } else { 1.0 };
        for o in v.seats.iter().filter_map(|s| s.occupant) {
            let Some(rider) = sim
                .unit(o)
                .filter(|r| r.alive && sim.hostile(u.team, r.team))
            else {
                continue;
            };
            let at = rider.capsule().center();
            let off = at - eye;
            let d2 = off.dot(off);
            let human_target = bf2 && rider.is_human();
            let reach = base * if human_target { 2.0 } else { 1.0 };
            if d2 < reach * reach
                && in_cone(view, off, d2, sim.omni_radius)
                && !cand.iter().any(|c| c.2 == Foe::Unit(o))
            {
                cand.push((d2, (0, u64::from(o.index)), Foe::Unit(o), at));
            }
        }
    }
    let droid_reach = droid_view_distance(tier.view_mult);
    for d in &sim.droids {
        if !sim.hostile(u.team, d.team) || !d.alive() || !droid_admitted(sim, u, d) {
            continue;
        }
        let off = d.pos - eye;
        let d2 = off.dot(off);
        if in_cone(view, off, d2, sim.omni_radius) && d2 < droid_reach * droid_reach {
            cand.push((d2, (1, d.id), Foe::Droid(d.id), d.pos));
        }
    }
    let turret_reach = turret_view_distance(tier.view_mult).min(scan_radius(sim, u));
    for t in &sim.turrets {
        if !sim.hostile(u.team, t.team) || !t.alive() || !turret_admitted(sim, u, t) {
            continue;
        }
        let at = t.centre();
        let off = at - eye;
        let d2 = off.dot(off);
        if in_cone(view, off, d2, sim.omni_radius) && d2 < turret_reach * turret_reach {
            cand.push((d2, (2, t.id), Foe::Turret(t.id), at));
        }
    }
    cand.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    // The unit's own target is always ray-tested first, so a firefight never
    // loses its target to five closer bodies it has not shot at yet.
    if let Some(t) = st.target {
        if let Some(i) = cand.iter().position(|c| c.2 == t) {
            let c = cand.remove(i);
            cand.insert(0, c);
        }
    }
    let mut seen = 0usize;
    for &(_, _, h, p) in &cand {
        if seen >= CONTACTS {
            break;
        }
        let past = h.unit().unwrap_or(me);
        if los_seat(
            sim,
            eye,
            p,
            past_mounts(sim, me, past),
            past_buildings(sim, me, past),
        ) {
            see(st, h, tick, p);
            seen += 1;
        }
    }

    // 3. Listen (no cone, no LOS): a shot fired inside HEAR_RADIUS since the
    //    last think, and — at any range — whoever hit me. A shooter that hit
    //    me unseen must be found first ([`search_ticks`], playtest C1).
    //    Card TEAM: the grid's units within HEAR_RADIUS, and whoever hit me.
    let mut heard = 0u8;
    let searches = hit_reaction_applies(sim, u);
    sim.units_within(eye, HEAR_RADIUS + NEAR_MARGIN, &mut near);
    if let Some((s, _)) = u.last_hit {
        if let Err(k) = near.binary_search(&s.index) {
            if sim.units.get(s.index as usize).is_some_and(|o| o.alive) {
                near.insert(k, s.index);
            }
        }
    }
    for &i in &near {
        let (h, o) = (sim.handle(i as usize), &sim.units[i as usize]);
        if h == me || !sim.hostile(u.team, o.team) {
            continue;
        }
        let shot = tick
            .checked_sub(o.last_shot_tick)
            .is_some_and(|d| d < THINK_PERIOD)
            && (o.soldier.pos - eye).length_squared() < HEAR_RADIUS * HEAR_RADIUS;
        let hit_me = u
            .last_hit
            .is_some_and(|(s, t)| s == h && tick.checked_sub(t).is_some_and(|d| d < THINK_PERIOD));
        let prior = if hit_me && searches {
            contact_of(st, h)
        } else {
            None
        };
        if (shot || hit_me) && hear(st, Foe::Unit(h), tick, o.soldier.pos, false) {
            heard += 1;
            if hit_me && searches {
                search(
                    st,
                    Foe::Unit(h),
                    tick,
                    (o.soldier.pos - eye).length(),
                    prior,
                );
            }
        }
    }

    let held = st.contacts.iter().flatten().count() as u8;
    (held, heard)
}

/// [`view_distance`] for a remote droid, by the same `(20·bound_radius + 30)
/// × mult × vis` (B2 `FUN_002e3910`). PROVISIONAL as the soldier's is: the
/// PS2's bound radius is the droid model's, ours the droid's collision sphere
/// ([`crate::remote::RADIUS`]).
pub fn droid_view_distance(view_mult: f32) -> f32 {
    (20.0 * crate::remote::RADIUS + 30.0) * view_mult * VIS
}

/// [`view_distance`] for an auto-turret: `(20·bound_radius + 30) × mult × vis`
/// × 10 for an armed building (B2 `FUN_002e3910`, `re-ai-difficulty.md` 5.1).
pub fn turret_view_distance(view_mult: f32) -> f32 {
    (20.0 * crate::autoturret::RADIUS + 30.0) * view_mult * VIS * 10.0
}

/// The PS2's scan radius (B2 `FUN_002a9d90`): the longest range of the unit's
/// weapons + 5 × its top speed. The soldier's own sight is the port's and
/// stays as it is; this only keeps [`turret_view_distance`]'s long reach.
fn scan_radius(sim: &Sim, u: &crate::unit::Unit) -> f32 {
    let range = |c: usize| sim.weapon_classes.get(c).map_or(0.0, |w| w.ai_range[2]);
    // An award weapon out of the unit's cycle does not count.
    let held = u
        .held
        .iter()
        .filter(|h| !sim.weapon_locked(u, h.class))
        .map(|h| range(h.class));
    held.fold(range(u.weapon_class), f32::max) + 5.0 * u.consts.max_speed
}

/// Whether `h` is still there to be remembered: a live unit, droid or turret.
pub fn alive(sim: &Sim, h: Foe) -> bool {
    match h {
        Foe::Unit(h) => sim.unit(h).is_some(),
        Foe::Droid(id) => sim.live_droid(id).is_some(),
        Foe::Turret(id) => sim.live_turret(id).is_some(),
    }
}

/// Where `h` is and how fast it moves: a unit's body centre, a droid's
/// position.
pub fn foe_motion(sim: &Sim, h: Foe) -> Option<(Vec3, Vec3)> {
    match h {
        Foe::Unit(h) => sim.unit(h).map(|o| (o.capsule().center(), o.soldier.vel)),
        Foe::Droid(id) => sim.live_droid(id).map(|d| (d.pos, d.vel)),
        Foe::Turret(id) => sim.live_turret(id).map(|t| (t.centre(), t.vel)),
    }
}

/// The PS2's suitability test at the scan (B2 `FUN_002e3aa0` ->
/// `FUN_002ac8f8`): a soldier takes a droid (kind 1, `EntityDroid`) by the
/// soldier-against-soldier rule (`FUN_002abe18` = `FUN_002abd38`), which asks
/// the weapon choice; a candidate no weapon it holds is suited to is not one.
/// A seated unit is not asked, as it is not of a soldier.
fn droid_admitted(sim: &Sim, u: &crate::unit::Unit, d: &crate::remote::Droid) -> bool {
    u.mount.is_some()
        || sim
            .droid_class(d)
            .is_some_and(|c| crate::ai::droid_suited(sim, u, c))
}

/// [`droid_admitted`] for an auto-turret: a soldier takes an armed building
/// through the weapon choice too (B2 `FUN_002ac8f8` kinds 8 and 9 ->
/// `FUN_002ac200` -> `FUN_002abee0` -> `FUN_002ab590`).
fn turret_admitted(sim: &Sim, u: &crate::unit::Unit, t: &crate::autoturret::Turret) -> bool {
    u.mount.is_some()
        || sim.turret_class(t).is_some_and(|c| {
            crate::ai::item_suited(sim, u, c.body.max_health, c.health_type as u32)
        })
}

/// Card TEAM: a friend this near passes on what it sees (m; the bot
/// design's 4.1). OURS and PROVISIONAL.
pub const TELL_RADIUS: f32 = 20.0;

/// Card TEAM (`analysis/bot-design.md` 4.1, 4.3, section 7's column): an
/// enemy that a friend within [`TELL_RADIUS`] saw at its last think is
/// HEARD by unit `me` — recorded at the friend's last known point, its
/// reaction counted from now, marked [`Contact::told`] — so it is aimed at
/// while the bot walks its own way (a told record holds nobody on a
/// position), and never fired at until `me` sees it ([`fireable`]). The
/// bot's own record of the same enemy is never overwritten.
/// Friends in unit-index order, their records in slot order. Every tier
/// tells (the design's column was every tier but Easy, removed 2026-10-28).
/// A told record keeps no bot on a position (`ai::behave`): before that
/// rule a told enemy held a bot standing at its fight spot while its friends
/// saw him - BF2 Coruscant's foot time in 20 s spells within 2 m 11.7 ->
/// 21.1 % (8 seeds), with it 9.4 %. Runs after [`scan`] in the same think.
/// Returns the records made or refreshed.
pub fn tell(sim: &Sim, me: UnitHandle, st: &mut AiState) -> u8 {
    let Some(u) = sim.unit(me) else {
        return 0;
    };
    let feet = u.soldier.pos;
    let mut near = crate::ai::team::NearList::new();
    sim.units_within(feet, TELL_RADIUS + NEAR_MARGIN, &mut near);
    let mut told = 0u8;
    for &i in &near {
        let (h, o) = (sim.handle(i as usize), &sim.units[i as usize]);
        if h == me
            || o.team != u.team
            || (o.soldier.pos - feet).length_squared() >= TELL_RADIUS * TELL_RADIUS
        {
            continue;
        }
        for c in o.ai.contacts.iter().flatten().filter(|c| c.seen_now) {
            if c.handle
                .live_unit(sim)
                .is_some_and(|e| sim.hostile(u.team, e.team) && e.cloak.visible())
                && hear(st, c.handle, sim.tick, c.last_pos, true)
            {
                told = told.saturating_add(1);
            }
        }
    }
    told
}

/// A NEW target restarts the aim tracker at the unit's own facing (brief D9)
/// and drops the tracked position, so the error's growth term is not paid for
/// a jump between two different enemies. The error RADIUS itself persists —
/// it is a property of the shooter, not of the target (LEAD ADDENDUM 4a).
fn restart_tracker(st: &mut AiState) {
    st.aim_dir = Vec3::ZERO;
    st.aim_vel = Vec3::ZERO;
    st.err_pos = Vec3::ZERO;
}

/// Card DFOCUS: make `h` (a contact the unit holds) its target now, past
/// the interval and the hysteresis, as a new target is taken. Returns true
/// when it was another target before.
pub fn take_target(st: &mut AiState, h: UnitHandle, tick: u64) -> bool {
    if st.target == Some(h.into()) || contact_of(st, h).is_none() {
        return false;
    }
    st.target = Some(h.into());
    st.last_switch = tick;
    restart_tracker(st);
    true
}

/// The slot for `h`: its own, else a free one, else the one whose record is
/// oldest (lowest slot index on ties). A new slot starts the reaction timer.
fn slot_for(st: &mut AiState, h: Foe, tick: u64, pos: Vec3) -> usize {
    if let Some(i) = st
        .contacts
        .iter()
        .position(|c| c.is_some_and(|c| c.handle == h))
    {
        return i;
    }
    let fresh = Contact {
        handle: h,
        first_seen: tick,
        last_seen: tick,
        last_pos: pos,
        seen_now: false,
        heard: false,
        told: false,
    };
    let i = match st.contacts.iter().position(Option::is_none) {
        Some(i) => i,
        None => (0..CONTACTS)
            .min_by_key(|&i| st.contacts[i].map_or(u64::MAX, |c| c.last_seen))
            .unwrap_or(0),
    };
    st.contacts[i] = Some(fresh);
    i
}

fn see(st: &mut AiState, h: Foe, tick: u64, pos: Vec3) {
    let i = slot_for(st, h, tick, pos);
    if let Some(c) = st.contacts[i].as_mut() {
        c.last_seen = tick;
        c.last_pos = pos;
        c.seen_now = true;
        c.heard = false;
        c.told = false;
    }
}

/// Record a heard enemy. Returns `false` when the unit already has EYES on it
/// this think — sight is the better record and hearing must not overwrite it
/// (the design's "seen_now stays false" for a heard record). Card TEAM: a
/// `told` record (a friend's, [`tell`]) never overwrites one of the unit's
/// own: its own memory of a target lapses as it would untold.
fn hear(st: &mut AiState, h: Foe, tick: u64, pos: Vec3, told: bool) -> bool {
    if contact_of(st, h).is_some_and(|c| c.seen_now || told && !c.told) {
        return false;
    }
    let i = slot_for(st, h, tick, pos);
    if let Some(c) = st.contacts[i].as_mut() {
        c.last_seen = tick;
        c.last_pos = pos;
        c.heard = true;
        c.told = told;
    }
    true
}

/// OURS (playtest C1): ticks a bot spends finding a shooter `dist` metres
/// away that hit it unseen: [`SEARCH_BASE`] + [`SEARCH_PER_M`] per metre, up
/// to [`SEARCH_CAP`], times [`SEARCH_KNOWN`] when it already held a record
/// of him. 20 m is 21 ticks, 40 m 33, 60 m 45, 75 m and beyond 54.
pub fn search_ticks(dist: f32, known: bool) -> u64 {
    let full = (SEARCH_BASE + SEARCH_PER_M * dist.max(0.0))
        .round()
        .min(SEARCH_CAP as f32);
    let t = if known {
        (full * SEARCH_KNOWN).round()
    } else {
        full
    };
    t as u64
}

/// Whether unit `u` pays the hit search and the flinch (playtest C1): a bot
/// on foot on a ground map. A seated bot keeps its seat's rules, a proxy
/// (a stand-in for a human, card DPRX) a human's, and a space map its own.
pub fn hit_reaction_applies(sim: &Sim, u: &crate::unit::Unit) -> bool {
    u.mount.is_none() && u.proxy_human.is_none() && !sim.space_round()
}

/// A hit by `h`, heard this think and not seen, moves the record's reaction
/// zero on by the search. `prior` is the record before this think's hearing.
/// A shooter the bot's own eyes held within [`SEARCH_FRESH`] costs nothing,
/// and a search already under way is not lengthened by another hit.
fn search(st: &mut AiState, h: Foe, tick: u64, dist: f32, prior: Option<Contact>) {
    let fresh = prior
        .is_some_and(|c| !c.heard && !c.told && tick.saturating_sub(c.last_seen) <= SEARCH_FRESH);
    if fresh {
        return;
    }
    if let Some(c) = st.contacts.iter_mut().flatten().find(|c| c.handle == h) {
        if c.first_seen <= tick {
            c.first_seen = tick + search_ticks(dist, prior.is_some());
        }
    }
}

/// Common admission for every target choice, including focus and threat
/// preferences: the one-second human spawn grace and the current turret arc.
pub(crate) fn target_admitted(sim: &Sim, me: UnitHandle, contact: &Contact) -> bool {
    contact
        .handle
        .unit()
        .is_none_or(|h| !crate::ai::dfocus::just_spawned(sim, h))
        && sim.unit(me).is_some_and(|u| {
            crate::ai::man::arc_of(sim, u).is_none_or(|holds| holds(contact.last_pos))
        })
}

/// Target choice over the memory (design decision 3): nearest-first with the
/// originals' +10 hysteresis and 1.5 s minimum interval, plus the tier's
/// PRESSURE weight for human targets.
///
/// The SCALE (200 / 100 over the standing view distance) is the port's — the
/// PS2's `120000 − metric` is in untraced units (RE §5.1) — PROVISIONAL.
///
/// Returns true when the tracker TOOK a target this call (the first, or
/// another in place of the held one): B1's tracker asks the soldier's
/// `AcquiredTargetSound` then, with no test that it differs from the last
/// (`FUN_0024d6d0` 167335; card UCALL, re-unit-chatter.md 2.2).
pub fn choose_target(sim: &Sim, me: UnitHandle, st: &mut AiState, tier: Tier) -> bool {
    choose_target_with(sim, me, st, tier, |_| true)
}

/// [`choose_target`] among the contacts whose last known point `keep`
/// admits: a held target it does not admit is dropped as one that left the
/// memory is (card TURR: a building turret's gunner keeps to its arc,
/// `ai::man::arc_holds`).
pub fn choose_target_with(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    tier: Tier,
    keep: impl Fn(Vec3) -> bool,
) -> bool {
    let Some(u) = sim.unit(me) else {
        return false;
    };
    let eye = u.ai_eye();
    let tick = sim.tick;
    let scale = view_distance(Stance::Stand, tier.view_mult);
    let score = |c: &Contact| -> f32 {
        let d = (c.last_pos - eye).length();
        let human = c.handle.live_unit(sim).is_some_and(|o| o.is_human());
        let base = (200.0 - 100.0 * d / scale) * if human { tier.pressure } else { 1.0 };
        if c.seen_now {
            // Card HVYW (OURS): a bot with a launcher prefers the vehicle.
            base + c
                .handle
                .live_unit(sim)
                .map_or(0.0, |o| crate::ai::vehicle_bonus(sim, u, o))
        } else {
            base - 10.0 * c.age_s(tick)
        }
    };
    let open = |c: &Contact| keep(c.last_pos) && target_admitted(sim, me, c);
    let mut best: Option<(f32, Foe)> = None;
    for c in st.contacts.iter().flatten().filter(|c| open(c)) {
        let s = score(c);
        if best.is_none_or(|(bs, _)| s > bs) {
            best = Some((s, c.handle));
        }
    }
    let held = st
        .target
        .and_then(|h| contact_of(st, h))
        .filter(|c| open(c));
    match (held, best) {
        // A target that left the memory is dropped at once — no interval, no
        // hysteresis: the unit has nothing to aim at any more.
        (None, best) => {
            if let Some((_, h)) = best {
                st.target = Some(h);
                st.last_switch = tick;
                restart_tracker(st);
                return true;
            }
            st.target = None;
        }
        (Some(cur), Some((bs, bh))) => {
            if bh != cur.handle
                && tick.saturating_sub(st.last_switch) >= RETARGET_TICKS
                && bs > score(&cur) + HYSTERESIS
            {
                st.target = Some(bh);
                st.last_switch = tick;
                restart_tracker(st);
                return true;
            }
        }
        (Some(_), None) => {}
    }
    false
}

#[cfg(test)]
mod tests;
