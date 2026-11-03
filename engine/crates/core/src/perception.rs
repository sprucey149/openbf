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
    /// Tick the slot was created — the reaction timer's zero.
    pub first_seen: u64,
    /// Tick of the last sighting or hearing.
    pub last_seen: u64,
    pub last_pos: Vec3,
    /// Seen by THIS think (eyes on it right now).
    pub seen_now: bool,
    /// The last record came from hearing, not sight.
    pub heard: bool,
    /// Card TEAM: ...and from a FRIEND's sight ([`tell`]), not this unit's
    /// own eyes or ears. A told record is aimed at; it does not keep the
    /// unit on a position (`ai::behave`).
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
/// warning, which is what makes a heard contact useful at all.
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
    // a flyer target): for a flyer in the sky, and for a flyer pilot's eyes on
    // any vehicle. Ground bots and ground vehicles keep the soldier's sight.
    let pilot = u
        .mount
        .and_then(|m| sim.vehicle(m.vehicle))
        .is_some_and(|v| v.kind == crate::vehicle::VehicleKind::Flyer);
    for (_, v) in sim.alive_vehicles() {
        let flyer = v.kind == crate::vehicle::VehicleKind::Flyer;
        if (!flyer && !pilot) || v.is_empty() {
            continue;
        }
        let radius = sim
            .seat_setup
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .map_or(0.0, |s| s.radius);
        let double = flyer && sim.ground_rules == crate::soldier::GroundRules::Bf2;
        let reach = (20.0 * radius + 30.0) * tier.view_mult * if double { 2.0 } else { 1.0 };
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
    //    last think, and — at any range — whoever hit me.
    //    Card TEAM: the grid's units within HEAR_RADIUS, and whoever hit me.
    let mut heard = 0u8;
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
        if (shot || hit_me) && hear(st, Foe::Unit(h), tick, o.soldier.pos, false) {
            heard += 1;
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
    let held = u.held.iter().map(|h| range(h.class));
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
mod tests {
    use super::*;
    use crate::ai::{think, AiState, THINK_PERIOD};
    use crate::collision::{Body, Shape};
    use crate::heightfield::Heightfield;
    use crate::sim::{buttons, Sim};
    use crate::skill::Difficulty;
    use crate::unit::{Controller, UnitSpec};
    use glam::Mat4;

    /// Flat ground everywhere (without a heightfield units spawn at
    /// `NO_GROUND` and fall away from anything placed for them).
    fn ground() -> Heightfield {
        Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        }
    }

    /// An AI soldier at the origin facing −Z, on flat ground.
    fn watcher(seed: u32) -> (Sim, crate::unit::UnitHandle) {
        let mut s = Sim::empty(seed);
        s.heightfield = Some(ground());
        let a = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        (s, a)
    }

    fn enemy_at(s: &mut Sim, p: Vec3) -> crate::unit::UnitHandle {
        s.spawn(&UnitSpec::trooper(1, p)).unwrap()
    }

    /// Normal's columns (Easy's are the same): view ×1.0, 9 reaction ticks.
    fn normal() -> Tier {
        Difficulty::Normal.tier()
    }

    /// Line of sight is blocked by a WORLD model, not only by terrain: an
    /// enemy on the far side of a wall body is invisible, and visible the
    /// moment the wall is gone. Before this the AI shot through every
    /// building on the map. (Phase C moved this test with `ai::visible` →
    /// `perception::los`, brief D5.)
    #[test]
    fn a_wall_body_blocks_line_of_sight() {
        let build = |wall: bool| {
            let (mut s, a) = watcher(11);
            let b = enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
            if wall {
                // Halfway between them, y in (0, 10): taller and wider than
                // either soldier, so nothing sees over or round it.
                s.collision.push_body(Body::new(
                    Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
                    Shape::Box {
                        half: Vec3::new(5.0, 5.0, 0.5),
                    },
                ));
            }
            (s, a, b)
        };
        let eye_and_target = |s: &Sim, a, b| {
            (
                s.unit(a).unwrap().muzzle(),
                s.unit(b).unwrap().capsule().center(),
            )
        };

        let (s, a, b) = build(true);
        let (eye, tgt) = eye_and_target(&s, a, b);
        assert!(!los(&s, eye, tgt), "the wall blocks");
        assert!(!los(&s, tgt, eye), "and blocks looking back");
        // The terrain half alone still says clear — it is the world ray that
        // is doing the work here, and the two halves are what the sweep's
        // `BLOCKED terrain|world|both` column reads.
        assert!(los_terrain(&s, eye, tgt), "terrain is clear");
        assert!(!los_world(&s, eye, tgt), "the world body is not");

        let (s, a, b) = build(false);
        let (eye, tgt) = eye_and_target(&s, a, b);
        assert!(los(&s, eye, tgt), "no wall, clear");
        // A wall PAST the target does not block it.
        let (mut s, a, b) = build(false);
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(0.0, 5.0, -30.0)),
            Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
        ));
        let (eye, tgt) = eye_and_target(&s, a, b);
        assert!(los(&s, eye, tgt), "the wall is behind the target");

        // ... and the AI acts on it: it targets the enemy it can see and
        // never the one behind the wall.
        let targeted = |wall: bool| {
            let (s, a, _) = build(wall);
            let mut st = AiState::default();
            think(&s, a, &mut st);
            st.target
        };
        assert!(targeted(false).is_some(), "targets a visible enemy");
        assert_eq!(targeted(true), None, "never targets through a wall");
    }

    /// The cone is the PS2's (RE §9.1): a 77.5° half-angle about the aim
    /// direction, with BF2's 10 m omni bubble that BF1 does not have.
    #[test]
    fn the_view_cone_is_77_5_degrees_over_bf2s_ten_metre_omni() {
        let view = Vec3::new(0.0, 0.0, -1.0);
        let at = |deg: f32, dist: f32| {
            let r = deg.to_radians();
            Vec3::new(r.sin() * dist, 0.0, -r.cos() * dist)
        };
        for &omni in &[0.0f32, 10.0] {
            for &(deg, inside) in &[
                (0.0, true),
                (60.0, true),
                (77.0, true),
                (78.0, false),
                (120.0, false),
            ] {
                let off = at(deg, 20.0);
                assert_eq!(
                    in_cone(view, off, off.dot(off), omni),
                    inside,
                    "{deg}° at 20 m, omni {omni}"
                );
            }
        }
        // Behind the unit at 5 m: BF2 sees it (inside the bubble), BF1 does
        // not (RE §9.1 — BF1 has the same cone with NO omni).
        let back = at(180.0, 5.0);
        let d2 = back.dot(back);
        assert!(in_cone(view, back, d2, 10.0), "BF2's 10 m omni");
        assert!(!in_cone(view, back, d2, 0.0), "BF1 has no omni");
        // The bubble ends exactly at its radius.
        let just_out = at(180.0, 10.5);
        assert!(!in_cone(view, just_out, just_out.dot(just_out), 10.0));
    }

    /// The view distance is `(20·bound_radius + 30) × mult × vis` with the
    /// bound radius half the capsule height (RE §5.1; PROVISIONAL — the PS2's
    /// bound is the model's).
    #[test]
    fn view_distance_by_stance_and_tier() {
        let close = |a: f32, b: f32| assert!((a - b).abs() < 1e-3, "{a} vs {b}");
        close(view_distance(Stance::Stand, 1.0), 49.0);
        close(view_distance(Stance::Crouch, 1.0), 46.0);
        close(view_distance(Stance::Prone, 1.0), 35.5);
        // The tiers scale it: Easy and Normal 1.0, Hard 1.25, Elite 1.5.
        for (d, mult) in [
            (Difficulty::Easy, 1.0),
            (Difficulty::Normal, 1.0),
            (Difficulty::Hard, 1.25),
            (Difficulty::Elite, 1.5),
        ] {
            close(
                view_distance(Stance::Stand, d.tier().view_mult),
                49.0 * mult,
            );
        }
        // ... and an enemy just past it is not a contact.
        let (mut s, a) = watcher(3);
        enemy_at(&mut s, Vec3::new(0.0, 0.0, -50.0));
        let mut st = AiState::default();
        assert_eq!(scan(&s, a, &mut st, normal()), (0, 0), "50 m > 49 m");
        let (mut s, a) = watcher(3);
        enemy_at(&mut s, Vec3::new(0.0, 0.0, -48.0));
        let mut st = AiState::default();
        assert_eq!(scan(&s, a, &mut st, normal()).0, 1, "48 m < 49 m");
    }

    /// Card MAIS: the mission's `SetAIViewMultiplier` shortens every bot's
    /// sight, on top of its tier's: Dagobah's 0.35 at Normal is 17 m, so an
    /// enemy 30 m off is not seen; at Hard (x 1.25, 21 m) one 20 m off is.
    #[test]
    fn the_missions_view_multiplier_shortens_every_bots_sight() {
        let seen = |s: &Sim, a| {
            let mut st = AiState::default();
            think(s, a, &mut st);
            st.contacts.iter().flatten().any(|c| c.seen_now)
        };
        let (mut s, a) = watcher(3);
        enemy_at(&mut s, Vec3::new(0.0, 0.0, -30.0));
        assert!(seen(&s, a), "30 m < 49 m");
        s.mission_ai.set_view_mult(0.35);
        assert!((s.ai_tier(0).view_mult - 0.35).abs() < 1e-6);
        assert!(!seen(&s, a), "30 m > 17 m");
        let (mut s, a) = watcher(3);
        enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
        s.mission_ai.set_view_mult(0.35);
        assert!(!seen(&s, a), "20 m > 17 m at Normal");
        s.difficulty = Difficulty::Hard;
        assert!((s.ai_tier(0).view_mult - 1.25 * 0.35).abs() < 1e-6);
        assert!(seen(&s, a), "20 m < 21 m at Hard");
    }

    /// OURS (2026-10-29, long-range bot sniping, tied to the tiers by the
    /// maintainer): a bot standing still with a scoped rifle in hand sees a
    /// standing enemy out to its tier's share of the rifle's `MaxRange` (128
    /// m), times the mission's view multiplier: Easy none (the plain 49 m),
    /// Normal 0.75 (96 m), Hard and Elite all of it; a crouching one as much
    /// nearer as the plain sight is. Not with the rifle slung, not on the
    /// move; a proxy sees as Hard does on any tier.
    #[test]
    fn a_still_sniper_sees_out_to_its_tiers_share_of_its_rifles_range() {
        let rifle = |s: &mut Sim| {
            s.add_weapon_class(crate::weapon::WeaponClass {
                trigger_single: true,
                zoom_max: 8.0,
                yaw_spread: 0.0,
                ai_range: [20.0, 64.0, 128.0],
                ..crate::weapon::WeaponClass::bf2_rifle()
            })
        };
        let seen = |s: &Sim, a| {
            let mut st = AiState::default();
            scan(s, a, &mut st, s.unit_tier(s.unit(a).unwrap()));
            st.contacts.iter().flatten().any(|c| c.seen_now)
        };
        let scene = |d: Difficulty, at: f32, stance: Stance| {
            let (mut s, a) = watcher(3);
            s.difficulty = d;
            let w = rifle(&mut s);
            s.units[a.index as usize].weapon_class = w;
            let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -at));
            s.units[e.index as usize].soldier.stance = stance;
            (s, a)
        };
        let sees = |d, at| {
            let (s, a) = scene(d, at, Stance::Stand);
            seen(&s, a)
        };
        // Per tier, a standing enemy at 48, 80, 110 and 120 m.
        for (d, want) in [
            (Difficulty::Easy, [true, false, false, false]),
            (Difficulty::Normal, [true, true, false, false]),
            (Difficulty::Hard, [true, true, true, true]),
            (Difficulty::Elite, [true, true, true, true]),
        ] {
            let got = [48.0, 80.0, 110.0, 120.0].map(|at| sees(d, at));
            assert_eq!(got, want, "{d:?}");
        }
        assert!(sees(Difficulty::Normal, 94.0), "Normal: 94 m < 96 m");
        assert!(!sees(Difficulty::Hard, 130.0), "Hard: 130 m > 128 m");
        // Crouching at Hard: 128 x 46 / 49 = 120 m.
        let (s, a) = scene(Difficulty::Hard, 115.0, Stance::Crouch);
        assert!(seen(&s, a), "a crouching enemy at 115 m");
        let (s, a) = scene(Difficulty::Hard, 125.0, Stance::Crouch);
        assert!(!seen(&s, a), "but not at 125 m");
        // Walking, it sees as anyone does.
        let (mut s, a) = scene(Difficulty::Hard, 80.0, Stance::Stand);
        s.units[a.index as usize].soldier.vel = Vec3::new(3.0, 0.0, 0.0);
        assert!(!seen(&s, a), "not on the move");
        // The rifle slung (a plain rifle in hand).
        let (mut s, a) = scene(Difficulty::Hard, 80.0, Stance::Stand);
        s.units[a.index as usize].weapon_class = 0;
        assert!(!seen(&s, a), "not with a plain rifle");
        // The mission's view multiplier shortens the scope too: 128 x 0.5.
        let (mut s, a) = scene(Difficulty::Hard, 70.0, Stance::Stand);
        s.mission_ai.set_view_mult(0.5);
        assert!(!seen(&s, a), "70 m > 64 m at x 0.5");
        let (mut s, a) = scene(Difficulty::Hard, 60.0, Stance::Stand);
        s.mission_ai.set_view_mult(0.5);
        assert!(seen(&s, a), "60 m < 64 m at x 0.5");
        // A proxy at Easy sees as Hard does.
        let (mut s, a) = scene(Difficulty::Easy, 120.0, Stance::Stand);
        s.units[a.index as usize].proxy_human = Some(crate::ai::proxy::Strength::Average.profile());
        assert!(seen(&s, a), "a proxy's scope at Easy");
    }

    /// A contact is kept for MEMORY_TICKS after the last sighting and cleared
    /// on the think after that.
    #[test]
    fn memory_expires_three_seconds_after_the_last_sighting() {
        let (mut s, a) = watcher(5);
        let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
        let mut st = AiState::default();
        scan(&s, a, &mut st, normal());
        let c = contact_of(&st, e).expect("seen");
        assert!(c.seen_now && !c.heard && c.first_seen == 0);
        // Take the sight away (a wall between them) and step the clock.
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
            Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
        ));
        s.tick = MEMORY_TICKS;
        scan(&s, a, &mut st, normal());
        let c = contact_of(&st, e).expect("still remembered at 90 ticks");
        assert!(!c.seen_now, "not seen, only remembered");
        assert_eq!(c.last_pos, Vec3::new(0.0, 0.0, -20.0) + Vec3::Y * 0.95);
        s.tick = MEMORY_TICKS + 1;
        scan(&s, a, &mut st, normal());
        assert_eq!(contact_of(&st, e), None, "forgotten at 91 ticks");
    }

    /// Six slots: a seventh enemy takes the slot whose record is oldest.
    #[test]
    fn a_seventh_contact_evicts_the_oldest() {
        let (mut s, a) = watcher(9);
        let mut seen = Vec::new();
        for i in 0..CONTACTS {
            seen.push(enemy_at(&mut s, Vec3::new(i as f32 * 2.0, 0.0, -20.0)));
        }
        let mut st = AiState::default();
        assert_eq!(scan(&s, a, &mut st, normal()).0, CONTACTS as u8);
        // Age the first contact only: hide it behind a wall, keep the rest.
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
            Shape::Box {
                half: Vec3::new(0.9, 5.0, 0.5),
            },
        ));
        s.tick = 30;
        scan(&s, a, &mut st, normal());
        assert!(!contact_of(&st, seen[0]).unwrap().seen_now, "hidden");
        // A seventh enemy arrives; the memory is full, so the oldest goes.
        let newcomer = enemy_at(&mut s, Vec3::new(-4.0, 0.0, -18.0));
        s.tick = 60;
        scan(&s, a, &mut st, normal());
        assert_eq!(contact_of(&st, seen[0]), None, "the oldest was evicted");
        assert!(contact_of(&st, newcomer).is_some(), "the seventh is held");
        assert_eq!(st.contacts.iter().flatten().count(), CONTACTS);
    }

    /// A first sighting is not a shot: the tier's reaction time must pass
    /// first (`reaction_ticks`, PROVISIONAL). Easy and Normal are 9 ticks (0.3 s).
    #[test]
    fn a_first_sighting_only_fires_after_the_reaction_time() {
        let (mut s, a) = watcher(13);
        enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
        let r = normal().reaction_ticks;
        assert_eq!(r, 9);
        let mut st = AiState::default();
        // Tick 0: seen for the first time, no shot.
        let (pad, _) = think(&s, a, &mut st);
        assert!(st.target.is_some(), "target acquired");
        assert_eq!(pad.buttons & buttons::FIRE, 0, "no shot on sight");
        let c = contact_of(&st, st.target.unwrap()).unwrap();
        assert!(!fireable(&c, 0, r) && !fireable(&c, r - 1, r));
        assert!(fireable(&c, r, r), "fireable once the reaction has passed");
        assert!(THINK_PERIOD > r, "the reaction is inside one think period");
        // ... and the think agrees: nothing before tick r, and the trigger
        // does go down after it (the aim tracker's slew, brief D9, still costs
        // a few ticks of alignment).
        for t in 1..r {
            s.tick = t;
            let (pad, _) = think(&s, a, &mut st);
            assert_eq!(pad.buttons & buttons::FIRE, 0, "no shot at tick {t}");
        }
        let mut fired = None;
        for t in r..r + 60 {
            s.tick = t;
            let (pad, _) = think(&s, a, &mut st);
            if pad.buttons & buttons::FIRE != 0 {
                fired = Some(t);
                break;
            }
        }
        assert!(fired.is_some(), "fires once the reaction has passed");
    }

    /// Hearing (design decision 2): a shot inside HEAR_RADIUS is a contact
    /// with no line of sight at all; one outside it is not; and being hit
    /// records the shooter at any range.
    #[test]
    fn hearing_records_a_shooter_behind_a_wall() {
        let heard_at = |dist: f32| {
            let (mut s, a) = watcher(17);
            // Behind the listener AND behind a wall: neither the cone nor
            // the LOS ray can find this one.
            let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, dist));
            s.collision.push_body(Body::new(
                Mat4::from_translation(Vec3::new(0.0, 5.0, dist / 2.0)),
                Shape::Box {
                    half: Vec3::new(5.0, 5.0, 0.5),
                },
            ));
            s.units[e.index as usize].last_shot_tick = 0;
            s.tick = 1;
            let mut st = AiState::default();
            let counts = scan(&s, a, &mut st, normal());
            (counts, contact_of(&st, e))
        };
        let ((held, heard), c) = heard_at(25.0);
        assert_eq!((held, heard), (1, 1), "a shot at 25 m is heard");
        let c = c.expect("heard contact");
        assert!(c.heard && !c.seen_now, "heard, not seen");
        assert_eq!(c.last_pos, Vec3::new(0.0, 0.0, 25.0), "the shooter's feet");
        let ((held, heard), c) = heard_at(35.0);
        assert_eq!((held, heard), (0, 0), "a shot at 35 m is not");
        assert_eq!(c, None);

        // Being HIT records the shooter at any range.
        let (mut s, a) = watcher(19);
        let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, 80.0));
        s.units[a.index as usize].last_hit = Some((e, 0));
        s.tick = 1;
        let mut st = AiState::default();
        assert_eq!(scan(&s, a, &mut st, normal()), (1, 1), "hit from 80 m");
        assert!(contact_of(&st, e).unwrap().heard);
    }

    /// Card TEAM: a scene for passing contacts on - the watcher at the
    /// origin facing -Z, an enemy 30 m behind it beyond a wall (it can
    /// neither see nor hear it), and a friend at `friend` whose last think
    /// saw the enemy (`seen` false: only remembered it).
    fn told_scene(friend: Vec3, seen: bool) -> (Sim, UnitHandle, UnitHandle, UnitHandle) {
        let (mut s, a) = watcher(31);
        let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, 30.0));
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(0.0, 5.0, 15.0)),
            Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
        ));
        let f = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, friend)
            })
            .unwrap();
        s.units[f.index as usize].ai.contacts[0] = Some(Contact {
            handle: e.into(),
            first_seen: 0,
            last_seen: 0,
            last_pos: Vec3::new(0.0, 0.9, 30.0),
            seen_now: seen,
            heard: false,
            told: false,
        });
        s.tick = 5;
        (s, a, e, f)
    }

    /// Card TEAM (bot design 4.1): what a friend within 20 m sees is heard -
    /// at the friend's point, its reaction counted from now - and nothing is
    /// passed on from 20.5 m or from a memory.

    #[test]
    fn a_friend_within_twenty_metres_passes_on_what_it_sees() {
        let told = |friend: Vec3, seen: bool, tier: Tier| {
            let (s, a, e, _) = told_scene(friend, seen);
            let mut st = AiState::default();
            assert_eq!(scan(&s, a, &mut st, tier), (0, 0), "not seen, not heard");
            let n = tell(&s, a, &mut st);
            (n, contact_of(&st, e))
        };
        let (n, c) = told(Vec3::new(15.0, 0.0, 0.0), true, normal());
        let c = c.expect("told");
        assert_eq!(n, 1);
        assert!(c.heard && c.told && !c.seen_now, "heard, told, not seen");
        assert_eq!(c.last_pos, Vec3::new(0.0, 0.9, 30.0), "the friend's point");
        assert_eq!(c.first_seen, 5, "the reaction counts from the telling");
        assert_eq!(
            told(Vec3::new(19.5, 0.0, 0.0), true, normal()).0,
            1,
            "19.5 m"
        );
        assert_eq!(
            told(Vec3::new(20.5, 0.0, 0.0), true, normal()),
            (0, None),
            "20.5 m"
        );
        assert_eq!(
            told(Vec3::new(15.0, 0.0, 0.0), false, normal()),
            (0, None),
            "a memory"
        );
        // Every tier tells (the design's column was every tier but Easy).
        for d in Difficulty::ALL {
            assert_eq!(
                told(Vec3::new(15.0, 0.0, 0.0), true, d.tier()).0,
                1,
                "{d:?}"
            );
        }
    }

    /// Card TEAM: a friend's telling never overwrites the unit's own record
    /// of the enemy (its own memory lapses as it would untold), and sight
    /// clears the told mark.
    #[test]
    fn a_told_record_never_overwrites_the_units_own() {
        let (mut s, a, e, _) = told_scene(Vec3::new(15.0, 0.0, 0.0), true);
        let own = Contact {
            handle: e.into(),
            first_seen: 1,
            last_seen: 2,
            last_pos: Vec3::new(3.0, 0.0, 28.0),
            seen_now: false,
            heard: true,
            told: false,
        };
        let mut st = AiState::default();
        st.contacts[0] = Some(own);
        assert_eq!(tell(&s, a, &mut st), 0, "its own record stands");
        assert_eq!(contact_of(&st, e), Some(own));
        // Its own memory lapses; then the friend's sight is told.
        s.tick = 2 + MEMORY_TICKS + 1;
        scan(&s, a, &mut st, normal());
        assert_eq!(contact_of(&st, e), None, "forgotten");
        assert_eq!(tell(&s, a, &mut st), 1);
        assert!(contact_of(&st, e).is_some_and(|c| c.told));
        // Seen, it is the unit's own again.
        see(&mut st, e.into(), s.tick + 1, Vec3::new(0.0, 0.9, 30.0));
        assert!(contact_of(&st, e).is_some_and(|c| c.seen_now && !c.told && !c.heard));
    }

    /// Card TEAM: a told enemy keeps nobody on a position. A bot hiding at a
    /// covered position (`COVER`), or holding one with time left on it,
    /// whose target only a friend sees lets it go - after `HOLD_CLOSE`, or
    /// once it has been quiet for `HOLD_QUIET`; the same bot with a record
    /// of its OWN (the control: told by hand each tick, as its ears would)
    /// keeps it the whole 10 s.
    #[test]
    fn a_told_enemy_keeps_nobody_on_a_position() {
        use crate::ai::{FightState, HoldState, HOLD_CLOSE};
        let kept = |own: bool, cover: bool| {
            let (mut s, a, e, f) = told_scene(Vec3::new(15.0, 0.0, 0.0), true);
            let mut st = AiState {
                hold: Some(HoldState {
                    until: 5 + if cover {
                        HOLD_CLOSE
                    } else {
                        20 * crate::TICK_HZ as u64
                    },
                    last_contact: 5,
                    ..HoldState::default()
                }),
                ..AiState::default()
            };
            st.fight.mode = if cover { FightState::COVER } else { 0 };
            if cover {
                // (At COVER a bot holds a position of the table, where it
                // stands: card TPOSR runs the Tactician on its own record.)
                use crate::ai::tactic::{Position, PositionTable, Source};
                let q = Position {
                    pos: s.units[a.index as usize].soldier.pos,
                    source: Source::Edge,
                    facing: glam::Vec2::ZERO,
                    stance: 0,
                    stance2: 0,
                    hub: u16::MAX,
                    low: 0,
                    corner: 0,
                    shut: 0,
                    tall: 0,
                };
                s.positions = PositionTable::index(vec![q], Default::default());
                st.tpos.held = Some(0);
            }
            for t in 5..5 + 10 * crate::TICK_HZ as u64 {
                s.tick = t;
                if let Some(c) = s.units[f.index as usize].ai.contacts[0].as_mut() {
                    c.seen_now = true;
                    c.last_seen = t;
                }
                if own {
                    st.contacts[0] = Some(Contact {
                        handle: e.into(),
                        first_seen: 5,
                        last_seen: t,
                        last_pos: Vec3::new(0.0, 0.9, 30.0),
                        seen_now: false,
                        heard: true,
                        told: false,
                    });
                }
                think(&s, a, &mut st);
            }
            assert_eq!(
                st.target,
                Some(e.into()),
                "own {own}: the enemy is the target"
            );
            st.hold.is_some_and(|h| s.tick < h.until)
        };
        for cover in [true, false] {
            assert!(kept(true, cover), "cover {cover}: its own record keeps it");
            assert!(!kept(false, cover), "cover {cover}: a friend's does not");
        }
    }

    /// Card TEAM: a told enemy becomes the target and is aimed at, and is
    /// never fired at while the bot cannot see it, however long it is told.
    #[test]
    fn a_told_enemy_is_aimed_at_and_never_fired_at_unseen() {
        let (mut s, a, e, f) = told_scene(Vec3::new(15.0, 0.0, 0.0), true);
        let mut st = AiState::default();
        for t in 5..5 + 4 * THINK_PERIOD * 10 {
            s.tick = t;
            if let Some(c) = s.units[f.index as usize].ai.contacts[0].as_mut() {
                c.seen_now = true;
                c.last_seen = t;
            }
            let (pad, _) = think(&s, a, &mut st);
            assert_eq!(pad.buttons & buttons::FIRE, 0, "no shot at tick {t}");
        }
        assert_eq!(st.target, Some(e.into()), "the told enemy is the target");
        let c = contact_of(&st, e).expect("still held");
        assert!(c.heard && !c.seen_now);
    }

    /// Target choice (decision 3): the current target is kept until a rival
    /// beats it by more than HYSTERESIS, and never before RETARGET_TICKS.
    #[test]
    fn hysteresis_and_the_retarget_interval() {
        // Two enemies in view; the score is the distance term alone (no
        // humans, both seen), so the closer one wins by
        // 100·(d₂ − d₁)/view_dist points.
        let scale = view_distance(Stance::Stand, 1.0);
        let gap = |points: f32| points * scale / 100.0;
        let build = |second: f32| {
            let (mut s, a) = watcher(23);
            let far = enemy_at(&mut s, Vec3::new(0.0, 0.0, -30.0));
            let near = enemy_at(&mut s, Vec3::new(0.0, 0.0, -(30.0 - second)));
            (s, a, far, near)
        };
        // Start with the FAR one as the target, at tick 0.
        let run = |second: f32, tick: u64| {
            let (mut s, a, far, near) = build(second);
            s.tick = tick;
            let mut st = AiState {
                target: Some(far.into()),
                last_switch: 0,
                ..Default::default()
            };
            scan(&s, a, &mut st, normal());
            // The far one must still be a contact for the "keeps" case to
            // mean anything.
            assert!(contact_of(&st, far).is_some());
            choose_target(&s, a, &mut st, normal());
            (st.target == Some(near.into()), far, near)
        };
        let (switched, ..) = run(gap(9.0), RETARGET_TICKS);
        assert!(!switched, "+9 points does not beat +10 hysteresis");
        let (switched, ..) = run(gap(11.0), RETARGET_TICKS);
        assert!(switched, "+11 points does");
        let (switched, ..) = run(gap(11.0), RETARGET_TICKS - 1);
        assert!(!switched, "... but never before the 1.5 s interval");
        // A target that left the memory is dropped at once, interval or not.
        let (mut s, a, far, near) = build(gap(11.0));
        s.units[far.index as usize].alive = false;
        let mut st = AiState {
            target: Some(far.into()),
            last_switch: 0,
            ..Default::default()
        };
        scan(&s, a, &mut st, normal());
        choose_target(&s, a, &mut st, normal());
        assert_eq!(
            st.target,
            Some(near.into()),
            "a lost target is replaced at once"
        );
    }

    /// The PRESSURE weight (decision 3): at Hard a human is worth ×1.3, so a
    /// farther human outscores a nearer bot; at Normal (×1.0) it does not.
    #[test]
    fn the_pressure_weight_prefers_a_farther_human_at_elite() {
        let pick = |tier: Tier| {
            let mut s = Sim::empty(29);
            s.heightfield = Some(ground());
            let a = s
                .spawn(&UnitSpec {
                    controller: Controller::Ai,
                    ..UnitSpec::trooper(0, Vec3::ZERO)
                })
                .unwrap();
            let bot = s
                .spawn(&UnitSpec::trooper(1, Vec3::new(4.0, 0.0, -10.0)))
                .unwrap();
            let human = s
                .spawn(&UnitSpec::player_in_slot(0, 1, Vec3::new(0.0, 0.0, -30.0)))
                .unwrap();
            let mut st = AiState::default();
            scan(&s, a, &mut st, tier);
            assert_eq!(st.contacts.iter().flatten().count(), 2, "both in view");
            choose_target(&s, a, &mut st, tier);
            (st.target, bot, human)
        };
        let (t, bot, _) = pick(normal());
        assert_eq!(t, Some(bot.into()), "Normal takes the nearer bot");
        let (t, _, human) = pick(Difficulty::Hard.tier());
        assert_eq!(t, Some(human.into()), "Hard leans on the human");
    }

    /// `[ours]` (the maintainer 2026-10-29): no bot takes a human as its
    /// target in his first second alive; a bot still may.
    #[test]
    fn a_human_is_nobodys_target_in_his_first_second() {
        let mut s = Sim::empty(31);
        s.heightfield = Some(ground());
        let a = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        let human = s
            .spawn(&UnitSpec::player_in_slot(0, 1, Vec3::new(0.0, 0.0, -10.0)))
            .unwrap();
        crate::ai::dfocus::upkeep(&mut s);
        let born = s.tick;
        let mut st = AiState::default();
        scan(&s, a, &mut st, normal());
        assert_eq!(st.contacts.iter().flatten().count(), 1, "seen");
        choose_target(&s, a, &mut st, normal());
        assert_eq!(st.target, None, "just spawned");
        s.tick = born + crate::ai::dfocus::UNTARGETED - 1;
        choose_target(&s, a, &mut st, normal());
        assert_eq!(st.target, None, "still in his first second");
        s.tick = born + crate::ai::dfocus::UNTARGETED;
        choose_target(&s, a, &mut st, normal());
        assert_eq!(st.target, Some(human.into()), "fair game after a second");
        let bot = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -5.0)))
            .unwrap();
        crate::ai::dfocus::upkeep(&mut s);
        let mut st = AiState::default();
        scan(&s, a, &mut st, normal());
        choose_target(&s, a, &mut st, normal());
        assert_eq!(st.target, Some(bot.into()), "a new bot is a target at once");
    }

    /// A unit that LOSES sight aims at the last known position and does not
    /// fire at it (brief D7).
    #[test]
    fn a_remembered_target_is_aimed_at_but_never_shot_at() {
        let (mut s, a) = watcher(31);
        let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
        let mut st = AiState::default();
        let mut fired = false;
        for t in 0..60u64 {
            s.tick = t;
            let (pad, _) = think(&s, a, &mut st);
            fired |= pad.buttons & buttons::FIRE != 0;
        }
        assert!(fired, "shoots what it sees");
        // The enemy steps behind a wall: the memory keeps it, the trigger
        // does not.
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
            Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
        ));
        s.tick += THINK_PERIOD;
        let (pad, _) = think(&s, a, &mut st);
        assert_eq!(st.target, Some(e.into()), "still the target");
        let c = contact_of(&st, e).unwrap();
        assert!(!c.seen_now, "out of sight");
        assert_eq!(pad.buttons & buttons::FIRE, 0, "never fires at a memory");
    }

    /// `PadState` is not part of this module's contract, but the fairness
    /// rule is: every FIRE the AI presses is at a contact it holds.
    #[test]
    fn every_ai_shot_is_at_a_held_contact() {
        let (mut s, a) = watcher(37);
        enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
        let mut st = AiState::default();
        for t in 0..90u64 {
            s.tick = t;
            let (pad, _) = think(&s, a, &mut st);
            if pad.pressed(buttons::FIRE) {
                let target = st.target.expect("fired with no target");
                assert!(contact_of(&st, target).is_some(), "fired at tick {t}");
            }
        }
    }

    /// The whole state a scan writes is hashed: two sims whose units carry
    /// different memories must not hash alike.
    #[test]
    fn the_contact_memory_is_part_of_the_hash() {
        let (mut s, a) = watcher(41);
        let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
        let base = s.hash();
        let mut st = AiState::default();
        scan(&s, a, &mut st, normal());
        s.units[a.index as usize].ai = st;
        let with_contact = s.hash();
        assert_ne!(base, with_contact, "a contact is state");
        // Every field of the record, one at a time (`Sim` is not `Clone`, so
        // each probe puts the slot back).
        let probe = |s: &mut Sim, name: &str, f: &dyn Fn(&mut Contact)| {
            let before = s.units[a.index as usize].ai.contacts[0];
            f(s.units[a.index as usize].ai.contacts[0].as_mut().unwrap());
            assert_ne!(s.hash(), with_contact, "{name} is state");
            s.units[a.index as usize].ai.contacts[0] = before;
            assert_eq!(s.hash(), with_contact, "{name} restored");
        };
        probe(&mut s, "first_seen", &|c| c.first_seen += 1);
        probe(&mut s, "last_seen", &|c| c.last_seen += 1);
        probe(&mut s, "last_pos", &|c| c.last_pos.x += 1.0);
        probe(&mut s, "seen_now", &|c| c.seen_now = !c.seen_now);
        probe(&mut s, "heard", &|c| c.heard = !c.heard);
        probe(&mut s, "handle", &|c| {
            if let Foe::Unit(h) = &mut c.handle {
                h.gen += 1
            }
        });
        // ... the empty slot's sentinel: dropping a contact must move it too.
        let keep = s.units[a.index as usize].ai.contacts[0];
        s.units[a.index as usize].ai.contacts[0] = None;
        assert_ne!(s.hash(), with_contact, "an empty slot is state");
        s.units[a.index as usize].ai.contacts[0] = keep;
        // ... and the two `Unit` fields the hearing rule writes.
        s.units[a.index as usize].last_shot_tick = 3;
        assert_ne!(s.hash(), with_contact, "last_shot_tick is state");
        s.units[a.index as usize].last_shot_tick = u64::MAX;
        s.units[a.index as usize].last_hit = Some((e, 1));
        assert_ne!(s.hash(), with_contact, "last_hit is state");
        s.units[a.index as usize].last_hit = None;
        assert_eq!(s.hash(), with_contact, "both restored");
        // ... and the target / last_switch pair.
        s.units[a.index as usize].ai.target = Some(e.into());
        let with_target = s.hash();
        assert_ne!(with_target, with_contact, "the target is state");
        s.units[a.index as usize].ai.last_switch = 7;
        assert_ne!(s.hash(), with_target, "last_switch is state");
    }
    #[test]
    fn audit_whole_brain_preferences_respect_spawn_grace() {
        for tier in [
            Difficulty::Easy,
            Difficulty::Normal,
            Difficulty::Hard,
            Difficulty::Elite,
        ] {
            for proxy in [None, Some(crate::ai::proxy::Strength::Good.profile())] {
                let mut sim = Sim::empty(31);
                sim.heightfield = Some(crate::heightfield::Heightfield {
                    size: 2,
                    unit: 4000.0,
                    offset_x: 2000.0,
                    offset_z: 2000.0,
                    heights: vec![0.0; 4],
                });
                sim.team_difficulty[0] = Some(tier);
                let bot = sim
                    .spawn(&UnitSpec {
                        controller: Controller::Ai,
                        ..UnitSpec::trooper(0, Vec3::ZERO)
                    })
                    .unwrap();
                sim.unit_mut(bot).unwrap().proxy_human = proxy;
                let human = sim
                    .spawn(&UnitSpec::player_in_slot(0, 1, Vec3::new(0.0, 0.0, -15.0)))
                    .unwrap();
                crate::ai::dfocus::upkeep(&mut sim);
                for tick in [
                    0,
                    crate::ai::dfocus::UNTARGETED - 1,
                    crate::ai::dfocus::UNTARGETED,
                ] {
                    sim.tick = tick;
                    let mut st = AiState::default();
                    crate::ai::think(&sim, bot, &mut st);
                    assert_eq!(
                        st.target,
                        (tick == crate::ai::dfocus::UNTARGETED).then_some(human.into()),
                        "tier={tier:?} proxy={proxy:?} tick={tick}"
                    );
                }
            }
        }
    }
}
