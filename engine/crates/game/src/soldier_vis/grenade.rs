//! The grenade on the soldier: strength variants (underhand / overhand) and the
//! pack drop action, including instant-release dispensers. Follows
//! `analysis/re-grenade-throw-anim.md` (card GTHRW).

use super::{in_hand, InHand, SoldierVisuals};
use crate::animator::BF1_THROW_CLIPS;
use crate::assets::SkinnedAsset;
use crate::skin::{BoneXform, Clip};
use crate::walk::WalkSim;
use core::unit::Unit;
use core::weapon::{FireState, WeaponClass, WeaponState};
use std::path::Path;

const UNDERHAND_MAX: f32 = 0.4;
const BF1_SHOOT3_ABOVE: f32 = 0.7;

/// The weapon drawn in the hand and whose set the animator takes: an
/// offhand weapon leaves the primary there (BF2). `bf2_throw`: a BF1 soldier
/// throws BF2's throw on a BF2 map ([`crate::animator::Rig::throws_offhand`],
/// card GTHRW), so a thrown grenade is offhand too (a dispenser keeps BF1's
/// drop).
pub(super) fn drawn<'u>(
    u: &'u Unit,
    classes: &[WeaponClass],
    bf2_throw: bool,
) -> (usize, u8, &'u WeaponState) {
    let hand = u.active_weapon();
    let offhand = classes
        .get(hand.0)
        .is_some_and(|c| c.offhand_weapon || (bf2_throw && c.thrown.is_some() && !c.dispenses()));
    if offhand {
        (u.weapon_class, u.weapon_slot, &u.weapon)
    } else {
        hand
    }
}

fn drawn_weapon(u: &Unit, classes: &[WeaponClass], bf2_throw: bool) -> InHand {
    in_hand(classes, drawn(u, classes, bf2_throw))
}

/// A baked BF1 soldier, who carries [`BF1_THROW_CLIPS`].
const BF1_THROW_LENDER: &str = "side_bf1/soldiers/rep_inf_trooper.glb";

/// `lender`'s clips `names` on `to`'s skeleton by bone name, every bone but
/// the root and the pelvis at `to`'s rest offset (the bake's own lending
/// rule, `bake::soldier`). A clip `to` already holds is left alone.
pub(super) fn lend_clips(lender: &SkinnedAsset, names: &[&str], to: &mut SkinnedAsset) {
    for name in names {
        let Some(src) = lender.clips.iter().find(|c| c.name == *name) else {
            continue;
        };
        if to.clips.iter().any(|c| c.name == *name) {
            continue;
        }
        let skel = &to.skeleton;
        let tracks = (0..skel.len())
            .map(|b| {
                let from = lender.skeleton.index_of(&skel.names[b])?;
                let keys = src.tracks.get(from)?.as_ref()?;
                let height = ["dummyroot", "bone_root", "bone_pelvis"]
                    .iter()
                    .any(|n| skel.names[b].eq_ignore_ascii_case(n));
                let rest = skel.local_bind[b].w_axis.truncate();
                Some(
                    keys.iter()
                        .map(|k| BoneXform {
                            rotation: k.rotation,
                            translation: if height { k.translation } else { rest },
                        })
                        .collect(),
                )
            })
            .collect();
        to.clips.push(Clip {
            name: (*name).to_owned(),
            fps: src.fps,
            frames: src.frames,
            tracks,
        });
    }
}

fn throw_variant(bf2: bool, strength: f32) -> u8 {
    if !bf2 && strength > BF1_SHOOT3_ABOVE {
        2
    } else {
        u8::from(strength > UNDERHAND_MAX)
    }
}

impl SoldierVisuals {
    /// The rigs of a class picked from the other disc throw as the map does:
    /// a BF1 soldier on a BF2 map plays BF2's throw the bake lent him, and a
    /// BF2 soldier on a BF1 map plays BF1's, lent here. `self` holds only that
    /// class's assets; `self.bf2` is its disc.
    pub(super) fn fit_throw_to_map(&mut self, root: &Path, map_bf2: bool) {
        if self.bf2 == map_bf2 {
            return;
        }
        if map_bf2 {
            for sa in &mut self.assets {
                if let Some(rig) = sa.rig.as_mut() {
                    rig.play_bf2_throw(&sa.asset.clips);
                }
            }
            return;
        }
        let lender = match crate::assets::load_skinned_glb(&root.join(BF1_THROW_LENDER)) {
            Ok(lender) => lender,
            Err(error) => {
                eprintln!("BF1's throw: {error:#}");
                return;
            }
        };
        for sa in &mut self.assets {
            lend_clips(&lender, &BF1_THROW_CLIPS, &mut sa.asset);
            if let Some(rig) = sa.rig.as_mut() {
                rig.play_bf1_throw(&sa.asset.clips);
            }
        }
    }

    /// Does unit `u`'s rig throw BF2's throw on a BF2 map
    /// ([`crate::animator::Rig::throws_offhand`])?
    pub(super) fn bf2_throw(&self, u: &Unit) -> bool {
        self.asset_for(u.team, u.spec.class)
            .and_then(|sa| sa.rig.as_ref())
            .is_some_and(|r| r.throws_offhand())
    }

    /// The weapon drawn in unit `u`'s hand ([`drawn_weapon`]).
    pub(super) fn hand_of(&self, u: &Unit, classes: &[WeaponClass]) -> InHand {
        drawn_weapon(u, classes, self.bf2_throw(u))
    }

    pub(super) fn note_throws(&mut self, walk: &WalkSim) {
        self.sync_units(walk);
        for (i, (v, u)) in self.units.iter_mut().zip(&walk.sim.units).enumerate() {
            let (wc, _, state) = u.active_weapon();
            let class = &walk.sim.weapon_classes[wc];
            let free = u.alive && u.mount.is_none();
            let released = class.dispenses() && walk.sim.events.iter().any(|e| {
                matches!(e,
                    core::sim::Event::Throw { unit, weapon_class, .. }
                    if unit.index as usize == i && unit.gen == u.gen && *weapon_class as usize == wc
                )
            });
            let throwing =
                (free && class.thrown.is_some() && (state.state == FireState::Firing || released))
                    .then_some(wc);
            if throwing.is_some() && throwing != v.throwing {
                v.pending_throw = Some(if class.dispenses() {
                    0
                } else {
                    throw_variant(self.bf2, state.charge)
                });
            }
            v.throwing = throwing;
            if !free {
                v.pending_throw = None;
            }
        }
    }
}

#[cfg(test)]
mod tests;
