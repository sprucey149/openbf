//! The animation rig: clip sets, the body and action slots, blend times, and the player that advances a clip.

use super::*;
use core::fmath::FloatExt;

/// A soldier's baked sets over his asset's skeleton and clips.
#[derive(Debug, Clone)]
pub struct Rig {
    pub game: Game,
    /// Per skeleton bone: posed by the upper / the lower player.
    pub(super) upper: Vec<bool>,
    pub(super) lower: Vec<bool>,
    /// `sets.clips` index → asset clip index.
    pub(super) map: Vec<Option<usize>>,
    pub(super) sets: SoldierSets,
    /// BF2: (family, weapon class) of each set, in [`Rig::weapon_set`]'s
    /// numbering.
    pub(super) bf2: Vec<(usize, String)>,
    /// Per asset clip: its root speed (`[x, z]` m/s, the bake's
    /// `root_speed`), for a walk, run or sprint.
    pub(super) root: Vec<Option<[f32; 2]>>,
    /// Per asset clip: a thrown flight (`thrown_flying*`), drawn over the
    /// soldier ([`Rig::root_bone`]).
    pub(super) flight: Vec<bool>,
    /// `bone_root` and its rest translation. The flight clips hold it 1.3 to
    /// 1.8 m off the soldier's position (BF1 and BF2 alike: front +1.8 m z,
    /// back -1.65, right -1.25 x), so the drawn body left his place (and
    /// his hit prims) until he landed: off the screen's bottom when thrown
    /// toward the camera (the maintainer 2026-10-27). Ours, PROVISIONAL (the PS2's
    /// draw of these clips is not measured): a flight clip keeps the root's
    /// rest x and z; its height and every rotation are the clip's.
    pub(super) root_bone: Option<(usize, glam::Vec3)>,
    /// BF2: the rifle's prone clips borrowed from BF1 (the bake's
    /// `BORROWED_PRONE`, issue #40; the PS2's BF2 has no prone): per
    /// [`bf2::MOVES`], then the shot and the reload, then the four
    /// [`PRONE_CHANGES`].
    pub(super) prone: Vec<Option<usize>>,
    /// BF1, fielded on a BF2 map ([`Rig::play_bf2_throw`]): BF2's throw (the
    /// bake's `BORROWED_THROW`, card GTHRW), `shoot_secondary` (underhand),
    /// then `shoot_secondary2` (overhand). `None` everywhere else: BF1's own
    /// grenade clip plays.
    pub(super) throw: [Option<usize>; 2],
    /// BF2, fielded on a BF1 map ([`Rig::play_bf1_throw`]): [`BF1_THROW_CLIPS`].
    pub(super) bf1_throw: [Option<usize>; 2],
}

/// BF1's throw, standing or crouched, then prone: every thermal, crack and
/// haywire plays these (`analysis/re-grenade-throw-anim.md` §1).
pub const BF1_THROW_CLIPS: [&str; 2] = ["human_grenade_stand_shoot3", "human_grenade_prone_shoot"];

/// BF2's rifle throw clips, which a BF1 soldier fielded on a BF2 map borrows
/// ([`Rig::play_bf2_throw`]).
pub const BF2_THROW_CLIPS: [&str; 2] = [
    "human_rifle_stand_shoot_secondary_full",
    "human_rifle_stand_shoot_secondary2_full",
];

impl Rig {
    pub fn new(sets: SoldierSets, skel: &Skeleton, clips: &[Clip]) -> Self {
        let mask = |names: &[String]| -> Vec<bool> {
            skel.names
                .iter()
                .map(|b| names.iter().any(|n| n.eq_ignore_ascii_case(b)))
                .collect()
        };
        let bf2 = sets
            .bf2
            .iter()
            .enumerate()
            .flat_map(|(f, s)| s.sets.keys().map(move |k| (f, k.clone())))
            .collect();
        let map: Vec<Option<usize>> = sets
            .clips
            .iter()
            .map(|c| find_clip(clips, &c.name))
            .collect();
        let mut root = vec![None; clips.len()];
        for (c, at) in sets.clips.iter().zip(&map) {
            if let (Some(v), Some(&k)) = (sets.root_speed.get(&c.name), at.as_ref()) {
                root[k] = Some(*v);
            }
        }
        Self {
            game: if sets.game == "bf2" {
                Game::Bf2
            } else {
                Game::Bf1
            },
            upper: mask(&sets.upper_bones),
            lower: mask(&sets.lower_bones),
            map,
            sets,
            bf2,
            root,
            flight: clips
                .iter()
                .map(|c| c.name.to_ascii_lowercase().contains("thrown_flying"))
                .collect(),
            root_bone: skel
                .index_of("bone_root")
                .map(|b| (b, skel.local_bind[b].w_axis.truncate())),
            prone: bf2::MOVES
                .iter()
                .map(|m| format!("human_rifle_prone_{m}"))
                .chain(["shoot", "reload"].map(|a| format!("human_rifle_prone_{a}")))
                .chain(PRONE_CHANGES.iter().map(|c| format!("human_rifle_{c}")))
                .map(|n| find_clip(clips, &n))
                .collect(),
            throw: [None; 2],
            bf1_throw: [None; 2],
        }
    }

    /// A BF1 soldier fielded on a BF2 map plays the BF2 throw clips the bake
    /// lent him. On a BF1 map nobody calls this: he plays BF1's own throw.
    pub fn play_bf2_throw(&mut self, clips: &[Clip]) {
        if self.game == Game::Bf1 {
            self.throw = BF2_THROW_CLIPS.map(|n| find_clip(clips, n));
        }
    }

    /// A BF2 soldier fielded on a BF1 map plays BF1's throw: `clips` are his
    /// clips with [`BF1_THROW_CLIPS`] lent at the end.
    pub fn play_bf1_throw(&mut self, clips: &[Clip]) {
        if self.game == Game::Bf2 {
            self.flight.resize(clips.len(), false);
            self.root.resize(clips.len(), None);
            self.bf1_throw = BF1_THROW_CLIPS.map(|n| find_clip(clips, n));
        }
    }

    /// Is asset clip `clip` a lent BF1 throw ([`Rig::play_bf1_throw`])?
    pub(super) fn is_lent_bf1_throw(&self, clip: usize) -> bool {
        self.bf1_throw.contains(&Some(clip))
    }

    /// A BF1 soldier on a BF2 map: the borrowed BF2 throw clip of `variant`
    /// (overhand above 0), standing or crouched (BF2's crouch plays the stand
    /// clips; it has no prone throw, so a prone soldier keeps BF1's).
    pub(super) fn borrowed_throw(&self, s: usize, variant: u8) -> Option<usize> {
        if self.game != Game::Bf1 || s == 2 {
            return None;
        }
        self.throw[usize::from(variant > 0)]
    }

    /// Does this BF1 soldier throw BF2's throw ([`Rig::play_bf2_throw`])? Then a
    /// thrown weapon is drawn as BF2's offhand one: the primary stays in
    /// his hand and keeps its set.
    pub fn throws_offhand(&self) -> bool {
        self.throw.iter().any(Option::is_some)
    }

    /// Is asset clip `clip` a borrowed BF2 throw ([`Rig::throw`])?
    pub fn is_borrowed_throw(&self, clip: usize) -> bool {
        self.throw.contains(&Some(clip))
    }

    /// BF2: the borrowed prone clip `k` of [`Rig::prone`] as a whole-body
    /// slot.
    pub(super) fn prone_slot(&self, k: usize, looping: bool) -> Option<Slot> {
        Some(Slot {
            clip: (*self.prone.get(k)?)?,
            looping,
            blend_in: 0.2,
            blend_out: 0.2,
            aim: aim::AimKind::None,
            pair: None,
        })
    }

    /// The set a weapon class puts in the soldier's hands: `bank` is the
    /// first word of its `AnimationBank` (BF2: or `CustomAnimationBank` /
    /// `ComboAnimationBank`), `family` the first word of the soldier class's
    /// `AnimationName` / `SkeletonName` (BF2 only; §4, §6.2). No match is the
    /// rifle's.
    pub fn weapon_set(&self, family: &str, bank: &str) -> usize {
        let bank = bank.trim().to_ascii_lowercase();
        if self.game == Game::Bf1 {
            return bf1::weapon_set(&bank).unwrap_or(0);
        }
        let fam = self
            .sets
            .bf2
            .iter()
            .position(|s| s.family.eq_ignore_ascii_case(family.trim()))
            .unwrap_or(0);
        let class = bank.split_once('_').map_or(bank.as_str(), |(_, n)| n);
        let find = |c: &str| self.bf2.iter().position(|(f, k)| *f == fam && k == c);
        find(class)
            .or_else(|| find(&bank))
            .or_else(|| find("rifle"))
            .unwrap_or(0)
    }

    /// The set's name, for a census.
    pub fn set_name(&self, set: usize) -> String {
        match self.game {
            Game::Bf1 => bf1::WEAPONS.get(set).copied().unwrap_or("?").to_owned(),
            Game::Bf2 => self.bf2.get(set).map_or("?".to_owned(), |(f, k)| {
                format!("{}_{k}", self.sets.bf2[*f].family)
            }),
        }
    }

    pub(super) fn slot1(&self, i: Option<u16>) -> Option<Slot> {
        let c = self.sets.clips.get(usize::from(i?))?;
        let blend = c.blend.unwrap_or(0.15);
        Some(Slot {
            clip: self.map[usize::from(i?)]?,
            looping: c.looping.unwrap_or(false),
            blend_in: blend,
            blend_out: blend,
            aim: aim::AimKind::None,
            pair: None,
        })
    }

    /// BF2: layer 0 the upper slot, 1 the lower.
    pub(super) fn slot2(&self, set: usize, pair: usize, layer: usize) -> Option<Slot> {
        let (f, k) = self.bf2.get(set)?;
        let i = usize::from((*self.sets.bf2[*f].sets[k].get(2 * pair + layer)?)?);
        let a = self.sets.bf2_animations.get(pair);
        let l = a.and_then(|a| if layer == 0 { &a.upper } else { &a.lower }.as_ref());
        Some(Slot {
            clip: self.map[i]?,
            looping: l.is_some_and(|l| l.looping),
            blend_in: l.map_or(0.0, |l| l.blend_in),
            blend_out: l.map_or(0.0, |l| l.blend_out),
            aim: l.map_or(aim::AimKind::None, |l| aim::AimKind::of(&l.aim)),
            pair: Some(pair),
        })
    }

    /// Has stance `s` strafe clips - a left and a right - in this set (BF1
    /// the base table's, `FUN_0022e990` B1:145010-145018; BF2 the set's own
    /// in the block `alert` picks, `FUN_0027ed18` B2:202195-202230)? The
    /// disc's stand and crouch have none.
    pub fn strafes(&self, set: usize, s: usize, alert: bool) -> bool {
        let has = |m: Move| match (self.game, &self.sets.bf1) {
            (Game::Bf1, Some(t)) => t
                .base
                .get(s)
                .and_then(|r| r.get(m.ids().0).copied().flatten())
                .is_some(),
            (Game::Bf2, _) => {
                let b = if alert { s + 3 } else { s };
                self.slot2(set, bf2::move_pair(b, m.ids().1), 1).is_some()
            }
            _ => false,
        };
        (has(Move::WalkLeft) || has(Move::RunLeft)) && (has(Move::WalkRight) || has(Move::RunRight))
    }

    /// The root speed of a movement clip along its axis (m/s), if baked.
    pub(super) fn root_speed(&self, clip: usize, m: Move) -> Option<f32> {
        Some(self.clip_speed(clip)?[m.axis()])
    }

    /// An asset clip's baked root speed (`[x, z]` m/s), for a walk, run or
    /// sprint.
    pub fn clip_speed(&self, clip: usize) -> Option<[f32; 2]> {
        self.root.get(clip).copied().flatten()
    }

    /// Does the set take the alert timer? Not BF1's tool and sabre sets
    /// (`FUN_0022d150`), not BF2's tool and melee classes (§7).
    pub fn alerts(&self, set: usize) -> bool {
        match self.game {
            Game::Bf1 => !matches!(set, 4 | 5),
            Game::Bf2 => self
                .bf2
                .get(set)
                .is_none_or(|(_, k)| k != "tool" && k != "melee"),
        }
    }

    /// Last contact direction, kept by the visual animator even for a
    /// killing hit. Missing directional clips fall back to forward death.
    pub fn death_slots(
        &self,
        set: usize,
        direction: hit::Direction,
    ) -> (Option<Slot>, Option<Slot>) {
        let slot = |name: &str, layer: usize| match self.game {
            Game::Bf1 => {
                let t = self.sets.bf1.as_ref()?;
                let id = bf1::SPECIAL.iter().position(|n| *n == name)?;
                let lower = self.slot1(*t.special.get(id)?);
                if layer == 0 {
                    self.slot1(*t.s.get(set)?.get(id)?).or(lower)
                } else {
                    lower
                }
            }
            Game::Bf2 => self.slot2(set, bf2::simple_pair(name)?, layer),
        };
        let at = |layer| {
            slot(direction.death_name(), layer).or_else(|| slot("stand_death_forward", layer))
        };
        (at(0), at(1))
    }

    /// (upper, lower) clips of a body state with no action running; `alert`
    /// takes the alert rows where the set has them (§3.2, §7).
    pub fn body_slots(&self, set: usize, body: Body, alert: bool) -> (Option<Slot>, Option<Slot>) {
        match (self.game, &self.sets.bf1, body) {
            (Game::Bf1, Some(t), Body::Special(sp)) => {
                let Some(id) = bf1::SPECIAL.iter().position(|n| *n == sp.name()) else {
                    return (None, None);
                };
                let lower = self.slot1(t.special[id]);
                (self.slot1(t.s[set][id]).or(lower), lower)
            }
            (Game::Bf1, Some(t), Body::Feet(s, m)) => {
                // A slot the tables leave empty: the other gait, then idle.
                let ids = [m.ids().0, m.other_gait().ids().0, 0];
                // Alert rows are the standing soldier's: the legs take the
                // rifle's for idle and the two runs, the upper body the
                // weapon's.
                let row = |w: usize, id: usize| {
                    let id = (alert && s == 0).then_some(id)?;
                    self.slot1(*t.alert.get(w)?.get(id)?)
                };
                let lower = row(0, ids[0])
                    .filter(|_| matches!(ids[0], 0 | 7 | 8))
                    .or_else(|| ids.iter().find_map(|&id| self.slot1(t.base[s][id])));
                // The upper body idles through a turn (`FUN_0022f1c8`).
                let up_ids = if matches!(m, Move::TurnLeft | Move::TurnRight) {
                    [0; 3]
                } else {
                    ids
                };
                let upper = row(set, up_ids[0])
                    .or_else(|| up_ids.iter().find_map(|&id| self.slot1(t.w[s][set][id])));
                (upper.or(lower), lower)
            }
            (Game::Bf2, _, body) => {
                let pairs = match body {
                    Body::Special(sp) => [bf2::simple_pair(sp.name()), None, None, None],
                    Body::Feet(s, m) => [
                        // The alert blocks stand three after the plain.
                        alert.then(|| bf2::move_pair(s + 3, m.ids().1)),
                        Some(bf2::move_pair(s, m.ids().1)),
                        Some(bf2::move_pair(s, m.other_gait().ids().1)),
                        Some(bf2::move_pair(s, 0)),
                    ],
                };
                // A BF2 set's prone rows hold the stand idle (the resolver's
                // last fallback: the disc has no prone), so a prone soldier
                // plays the borrowed clips first: the move, its other gait,
                // the idle.
                let prone = match body {
                    Body::Feet(2, m) => [m.ids().1, m.other_gait().ids().1, 0]
                        .into_iter()
                        .find_map(|k| self.prone_slot(k, true)),
                    Body::Special(sp @ (Special::ProneDown(_) | Special::ProneUp(_))) => {
                        PRONE_CHANGES
                            .iter()
                            .position(|n| *n == sp.name())
                            .and_then(|k| self.prone_slot(bf2::MOVES.len() + 2 + k, false))
                    }
                    _ => None,
                };
                let at = |layer| {
                    prone.or_else(|| {
                        pairs
                            .iter()
                            .flatten()
                            .find_map(|&p| self.slot2(set, p, layer))
                            .or_else(|| self.slot2(set, bf2::move_pair(0, 0), layer))
                    })
                };
                (at(0), at(1))
            }
            _ => (None, None),
        }
    }

    /// (upper, lower) clips of a weapon action in stance `s`; `None` when
    /// the set has no clip for it (the action then does not run).
    pub fn action_slots(
        &self,
        set: usize,
        s: usize,
        kind: ActionKind,
    ) -> Option<(Slot, Option<Slot>)> {
        match (self.game, &self.sets.bf1) {
            (Game::Bf1, Some(t)) => {
                // A BF1 soldier on a BF2 map throws BF2's throw (card GTHRW):
                // its slots' blends (upper 0.15, lower 0.5; the bf2
                // `shoot_secondary2` pair).
                if let ActionKind::Throw(variant) = kind {
                    if let Some(clip) = self.borrowed_throw(s, variant) {
                        let up = Slot {
                            clip,
                            looping: false,
                            blend_in: 0.15,
                            blend_out: 0.15,
                            aim: aim::AimKind::None,
                            pair: None,
                        };
                        let legs = Slot {
                            blend_in: 0.5,
                            blend_out: 0.5,
                            ..up
                        };
                        return Some((up, Some(legs)));
                    }
                }
                let k = match kind {
                    ActionKind::Shoot => 1,
                    ActionKind::Throw(variant) => usize::from(variant.min(2)) + 1,
                    ActionKind::Reload => 5,
                };
                // A soldier who borrows BF2's throw keeps his primary's set
                // through it (BF2's offhand rule); a prone throw, which BF2
                // has none of, takes the grenade set's.
                let set = match kind {
                    ActionKind::Throw(_) if self.throw.iter().any(Option::is_some) => {
                        bf1::weapon_set("grenade").unwrap_or(set)
                    }
                    _ => set,
                };
                // The legs play the same clip, its blend 0.5 s longer (§3.2).
                let up = self.slot1(t.u[s][set][k])?;
                let legs = Slot {
                    blend_in: up.blend_in + 0.5,
                    ..up
                };
                Some((up, Some(legs)))
            }
            (Game::Bf2, _) => {
                // A BF2 soldier on a BF1 map throws BF1's throw: BF1's blends
                // (the clip's 0.02, the legs 0.5 s more; §1).
                if kind == ActionKind::Throw(2) {
                    if let Some(clip) = self.bf1_throw[usize::from(s == 2)] {
                        let up = Slot {
                            clip,
                            looping: false,
                            blend_in: 0.02,
                            blend_out: 0.02,
                            aim: aim::AimKind::None,
                            pair: None,
                        };
                        let legs = Slot {
                            blend_in: 0.52,
                            ..up
                        };
                        return Some((up, Some(legs)));
                    }
                }
                let k = match kind {
                    ActionKind::Shoot => 0,
                    ActionKind::Throw(variant) => 2 + usize::from(variant > 0),
                    ActionKind::Reload => 5,
                };
                let p = bf2::action_pair(s, k);
                // A prone shot or reload: the borrowed clip (see `Rig::prone`).
                let n = bf2::MOVES.len();
                let prone = match kind {
                    ActionKind::Shoot if s == 2 => self.prone_slot(n, false),
                    ActionKind::Reload if s == 2 => self.prone_slot(n + 1, false),
                    _ => None,
                };
                if let Some(up) = prone {
                    return Some((up, Some(up)));
                }
                Some((self.slot2(set, p, 0)?, self.slot2(set, p, 1)))
            }
            _ => None,
        }
    }

    /// A still of a body state `frame` frames in, with a weapon action over
    /// it when one is named (the legs joined, as standing still): what a
    /// gallery shows of a set.
    pub fn still(
        &self,
        (clips, skel): (&[Clip], &Skeleton),
        set: usize,
        body: Body,
        action: Option<ActionKind>,
        frame: f32,
    ) -> Pose {
        let (mut up, mut low) = self.body_slots(set, body, false);
        if let (Body::Feet(s, _), Some(kind)) = (body, action) {
            if let Some((a, legs)) = self.action_slots(set, s, kind) {
                (up, low) = (Some(a), legs.or(low));
            }
        }
        let mut pose = Pose::bind(skel);
        for (slot, mask) in [(low, &self.lower), (up, &self.upper)] {
            let Some(slot) = slot else { continue };
            let sampled = clips[slot.clip].sample(skel, frame, true);
            for (b, x) in pose.0.iter_mut().enumerate() {
                if mask[b] {
                    *x = sampled.0[b];
                }
            }
        }
        pose
    }

    /// Seconds a change from `old` to `new` blends over.
    pub(super) fn blend_secs(&self, old: Option<&Slot>, new: &Slot, upper: bool) -> f32 {
        if self.game == Game::Bf1 {
            return new.blend_in;
        }
        let Some(old) = old else { return 0.0 };
        let layer = |p: usize| {
            let a = self.sets.bf2_animations.get(p)?;
            if upper { &a.upper } else { &a.lower }.as_ref()
        };
        let to = old.pair.zip(new.pair).and_then(|(o, n)| {
            let name = &self.sets.bf2_animations.get(n)?.name;
            let l = layer(o)?;
            l.blend_to
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
        });
        to.map_or(new.blend_in.fmin(old.blend_out), |(_, t)| *t)
    }
}

/// What an update poses with.
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub(super) rig: &'a Rig,
    pub(super) clips: &'a [Clip],
    pub(super) skel: &'a Skeleton,
    pub(super) dt: f32,
    /// BF1: the alert's factor on both layers' weight (§3.4).
    pub(super) factor: f32,
    pub(super) blend_updates: f32,
}

/// One clip player.
#[derive(Debug, Clone, Default)]
pub(super) struct Player {
    pub(super) slot: Option<Slot>,
    /// Phase in [0, 1].
    pub(super) t: f32,
    /// Seconds on this clip.
    pub(super) secs: f32,
    pub(super) finished: bool,
    /// Seconds the change to this clip blends over.
    pub(super) blend: f32,
    /// BF2: the crossfade weight already applied.
    pub(super) weight: f32,
}

impl Player {
    /// Put `slot` on the player; `restart` replays a clip it already holds.
    pub(super) fn set(
        &mut self,
        rig: &Rig,
        slot: Slot,
        upper: bool,
        restart: bool,
        phase: Option<f32>,
    ) {
        if !restart && self.slot.is_some_and(|s| s.clip == slot.clip) {
            return;
        }
        self.blend = rig.blend_secs(self.slot.as_ref(), &slot, upper);
        self.slot = Some(slot);
        self.t = phase.unwrap_or(0.0);
        self.secs = 0.0;
        self.finished = false;
        self.weight = 0.0;
    }

    pub(super) fn advance(&mut self, clips: &[Clip], dt: f32, game: Game) {
        let Some(s) = self.slot else { return };
        let f = clips[s.clip].frames.max(2) as f32;
        let denom = match game {
            Game::Bf1 => f,
            Game::Bf2 => f - 1.0,
        };
        self.t += dt * 30.0 / denom;
        self.secs += dt.abs();
        if s.looping {
            self.t = self.t.rem_euclid(1.0);
        } else if self.t >= 1.0 {
            self.t = 1.0;
            self.finished = true;
        }
    }

    /// Write this player's clip into the masked bones of the kept pose.
    pub(super) fn apply(&mut self, pose: &mut Pose, cx: &Ctx<'_>, upper: bool, snap: bool) {
        let Ctx {
            rig,
            clips,
            skel,
            dt,
            factor,
            blend_updates,
        } = *cx;
        let (mask, game) = (if upper { &rig.upper } else { &rig.lower }, rig.game);
        let Some(s) = self.slot else { return };
        let c = &clips[s.clip];
        let mut sampled = c.sample(skel, self.t * c.frames.saturating_sub(1) as f32, s.looping);
        if let (Some(true), Some((b, rest))) = (rig.flight.get(s.clip), rig.root_bone) {
            let x = &mut sampled.0[b].translation;
            (x.x, x.z) = (rest.x, rest.z);
        }
        let cur = factor
            * if self.blend > 0.0 {
                (self.secs / self.blend).fmin(1.0)
            } else {
                1.0
            };
        // The weight the kept pose moves toward the clip by, this update.
        let k = match game {
            _ if snap || cur >= 1.0 => 1.0,
            Game::Bf1 => 1.0 - (1.0 - cur).powf(dt * blend_updates),
            Game::Bf2 => ((cur - self.weight) / (1.0 - self.weight)).clamp(0.0, 1.0),
        };
        self.weight = cur;
        for (b, x) in pose.0.iter_mut().enumerate() {
            if !mask.get(b).copied().unwrap_or(false) {
                continue;
            }
            if k >= 1.0 {
                // An overwrite gives a bone the clip has no track for its
                // rest pose (`Clip::sample`'s).
                *x = sampled.0[b];
            } else if c.tracks.get(b).is_some_and(Option::is_some) {
                *x = x.blend(sampled.0[b], k);
            }
        }
    }
}
