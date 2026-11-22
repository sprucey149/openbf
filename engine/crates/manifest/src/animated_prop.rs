//! A BF2 map's animated props, beside its manifest as [`FILE`]: every class
//! whose BASE is `animatedprop` (BF2's EntityPropAnimated, B2 setter
//! `FUN_002321a0`, update `FUN_002312c0`; `bake::animated_prop`), and the
//! animated props other classes hang on their hardpoints with `AttachOdf` +
//! `AttachToHardpoint` (B2 `FUN_0022f6e8`, `analysis/re-leafpatch.md` §2.1),
//! such as Jabba's Palace's rancor in its pit and Jabba on his dais.
//!
//! A map with none has no file; the bake writes nothing then, so every other
//! map's bake is unchanged.

use serde::{Deserialize, Serialize};

pub const FILE: &str = "animated_props.ron";

/// One `Animation` line, in ODF order (at most five; the first is the idle
/// loop the prop returns to).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropClip {
    pub name: String,
    /// The clip's frame count; it lasts `frames / 30` s (the disc's
    /// `frames * 0.033333335`).
    pub frames: u16,
}

/// One `AnimationTrigger "<clip> <hardpoint> <radius>"`: something within
/// `radius` of the hardpoint, at the model's BIND pose (the update reads the
/// model's static hardpoint table, `FUN_0057dc60`), starts `clip` once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropTrigger {
    /// Index into [`AnimatedRig::clips`].
    pub clip: u8,
    pub hardpoint: String,
    pub radius: f32,
    /// Model space.
    pub position: [f32; 3],
}

/// One `AttachTrigger "<hardpoint> <radius> <clip> <soldier anim> <time> …"`:
/// while `clip` plays and is no further than `time` s in, a living soldier
/// within `radius` of the hardpoint, at the ANIMATED pose, is caught and
/// rides it until the clip ends, when he dies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropAttach {
    pub hardpoint: String,
    pub radius: f32,
    pub clip: u8,
    /// The soldier animation he plays while held (`thrown_flail`).
    pub soldier_animation: String,
    pub time: f32,
    /// The hardpoint's MODEL-space position at every frame of every clip,
    /// indexed like [`AnimatedRig::clips`].
    pub track: Vec<Vec<[f32; 3]>>,
}

/// One collision primitive whose `PRNT` bone the clips move: the bone's
/// MODEL-space transform at every frame of every clip, as (rotation xyzw,
/// translation). The prim's world transform is `instance · B(f) · XFRM`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropPrim {
    pub name: String,
    pub bone: String,
    pub frames: Vec<Vec<([f32; 4], [f32; 3])>>,
}

/// An `animatedprop` class's rig.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimatedRig {
    pub class: String,
    pub geometry: String,
    /// The drawn model's stem: `animated/<model>.glb` (its mesh at the bind
    /// pose with `JOINTS_0`) and `animated/<model>.skin.glb` (its skeleton
    /// and clips). `None` when the bake could not write them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// `AnimationName`: the `zaa_` bank.
    pub bank: String,
    pub clips: Vec<PropClip>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<PropTrigger>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attaches: Vec<PropAttach>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prims: Vec<PropPrim>,
    /// `KillSoldierSound`: played where the prop stands when a held soldier dies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kill_sound: Option<String>,
    /// `TrackDeathOnAttach`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub track_death: bool,
    /// `DisableForCloneWars`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_for_clone_wars: bool,
    /// `IdleDelay`, seconds the idle loop waits at its end.
    #[serde(default)]
    pub idle_delay: f32,
}

/// One animated prop another class hangs on a hardpoint of its model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachedProp {
    /// The class that carries it (its instances get one each).
    pub parent: String,
    /// The `animatedprop` class.
    pub class: String,
    pub hardpoint: String,
    /// The hardpoint's bind matrix in the parent's model space, rows as the
    /// manifest's instances store them (row-vector: world = this × instance).
    pub rotation: [[f32; 3]; 3],
    pub translation: [f32; 3],
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimatedProps {
    pub rigs: Vec<AnimatedRig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attached: Vec<AttachedProp>,
}

impl AnimatedProps {
    pub fn rig(&self, class: &str) -> Option<&AnimatedRig> {
        self.rigs
            .iter()
            .find(|r| r.class.eq_ignore_ascii_case(class))
    }

    pub fn is_empty(&self) -> bool {
        self.rigs.is_empty()
    }
}
