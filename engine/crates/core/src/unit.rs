//! A combat unit: the P3 soldier plus health, its weapons (the one in its
//! hands and the class's other slots, card GREN), a team and a controller. Units live in `Sim::units` slots addressed by generation-
//! checked handles (the originals re-validate stale entity pointers by id —
//! e.g. bolt hit target `+0xb4` guard; we do the same with `gen`).

use crate::ai::AiState;
use crate::collision::Capsule;
use crate::fmath;
use crate::soldier::{GroundRules, Soldier, SoldierConstants, Stance};
use crate::weapon::{Health, HealthType, WeaponClass, WeaponState};
use glam::Vec3;

mod melee;

/// Where a bot's next life starts (card BSPN; re-command-walker-post §6
/// [O]): one of the sim's command posts by index, or a live command walker —
/// a post like any other to the PS2's draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnPost {
    Post(u16),
    Walker(crate::vehicle::VehicleHandle),
}

impl SpawnPost {
    /// A kind byte (1 post, 2 walker), then the index and generation.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        let (kind, index, gen) = match *self {
            SpawnPost::Post(i) => (1u8, u32::from(i), 0),
            SpawnPost::Walker(h) => (2, h.index, h.gen),
        };
        eat(&[kind]);
        eat(&index.to_le_bytes());
        eat(&gen.to_le_bytes());
    }
}

/// Generation-checked unit reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnitHandle {
    pub index: u32,
    pub gen: u32,
}

/// Who drives a unit's pad each tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Controller {
    /// Driven by the pad in player slot `slot` (`Sim::step`'s `pads[slot]`);
    /// a slot with no pad this tick idles.
    Player { slot: u8 },
    /// Stands still (dummy target).
    Idle,
    /// Dummy pacer: constant planar velocity, reversed every `period`
    /// ticks (the P3 aim-assist rig).
    Pacer { vel: Vec3, period: u32 },
    /// AI synthesizes pad input; state lives in `crate::ai`.
    Ai,
}

/// PROVISIONAL soldier collision radius (the BF1 aim-assist rig used 0.5;
/// the real prim radius comes from the soldier ODF / collision RE).
pub const SOLDIER_RADIUS: f32 = 0.5;

/// BF2 per-stance capsule heights (DAT_0061ed50: 1.9 / 1.6 / 0.55).
pub fn capsule_height(stance: Stance) -> f32 {
    match stance {
        // The tumble and get-up take the crouch's posture, a corpse the
        // prone's (card KNOCK, `re-knockback.md` §2 / §3: posture 1 / 2).
        Stance::Crouch | Stance::Tumble | Stance::GetUp => 1.6,
        Stance::Prone | Stance::Roll | Stance::Dead => 0.55,
        _ => 1.9,
    }
}

/// PROVISIONAL muzzle height: 75% of the capsule (the real fire point is
/// the weapon model's hp_fire hardpoint — needs skinned soldiers).
pub fn muzzle_height(stance: Stance) -> f32 {
    capsule_height(stance) * 0.75
}

/// The soldier's acquisition box, local to the feet (the model box the
/// PS2's aim acquisition reads, `analysis/re-aim-assist.md` §3.6: [V-RAM]
/// one BF1 soldier's box, which does not change with stance). PROVISIONAL
/// for every class: the RAM showed seven shapes (r 1.387 to 1.704) and the
/// port carries none per class — the modl vis box is not it (y max 2.0877,
/// z max 0.215 on the stormtrooper).
pub const AIM_BOX_MIN: Vec3 = Vec3::new(-0.929, -0.001, -0.196);
pub const AIM_BOX_MAX: Vec3 = Vec3::new(0.929, 2.099, 0.204);

/// The pitch every PS2 soldier spawns with (§10.3: 0xbd8f5c29, both games).
/// The port gives it to a human; a bot's is card FPNT's.
pub const SPAWN_PITCH: f32 = -0.07;

/// One camera section of a soldier class (`analysis/re-aim-assist.md`
/// §10.1): `EyePointOffset` = `TrackCenter` is `eye_y` above the feet, the
/// camera sits `offset` from it in its own frame (x right, y up, z back) and
/// looks `tilt` radians BELOW the aim.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSection {
    pub eye_y: f32,
    pub offset: Vec3,
    pub tilt: f32,
}

/// In split screen the tilt is 0.4 of the authored one (§10.1, the tilt
/// getter B1 FUN_001b58c0 / B2 FUN_001ce9b8).
pub const SPLIT_TILT: f32 = 0.4;

/// The chase camera's section by stance and zoom: BF2's on every map, BF1's
/// included - OURS, a named deviation (playtest 2026-10-28: "let's
/// standardize the bf2 camera angles and zoom and everything for default.
/// All maps should appear the same way"). BF1's own section (stand 3.5 m
/// back, tilt 5 deg) under its 48 deg FOV leaves the soldier's shins off the
/// bottom of the frame (`analysis/camera-occlusion.md` §3).
pub fn camera_section(stance: Stance, zoomed: bool) -> CameraSection {
    ps2_camera_section(GroundRules::Bf2, stance, zoomed)
}

/// A soldier's camera section on the PS2 by game, stance and zoom, [V-RAM]
/// §10.1's table (all player classes alike; the per-class exceptions - the
/// BF1 snowtrooper's zoom - are card CAMS's): the record; the port plays
/// [`camera_section`]. A zoomed soldier takes the
/// stance's ZOOM section: over the right shoulder, the same in both games
/// (`analysis/re-zoom.md` §5). Sprint is a section of its own on BF2; BF1 has no sprint, so
/// its sprint section is OURS (the maintainer, 2026-10-04, FEEL_NOTES): the stand
/// section moved back by BF2's own step, 0.8 m, so a sprint shows. Jump, roll,
/// fall and the jets keep the stand section.
pub fn ps2_camera_section(rules: GroundRules, stance: Stance, zoomed: bool) -> CameraSection {
    let bf1 = rules == GroundRules::Bf1;
    if zoomed {
        let (eye_y, y) = match stance {
            Stance::Crouch => (1.3, 0.2),
            Stance::Prone => (0.5, 0.2),
            _ => (1.8, 0.05),
        };
        return CameraSection {
            eye_y,
            offset: Vec3::new(0.4, y, 2.8),
            tilt: f32::to_radians(3.5),
        };
    }
    let (eye_y, y, z, deg) = match stance {
        Stance::Crouch => (
            1.3,
            0.15,
            if bf1 { 3.5 } else { 3.0 },
            if bf1 { 5.0 } else { 10.0 },
        ),
        Stance::Prone => (0.5, 0.0, 3.0, 5.0),
        // The jets keep the stand's section: measured live, a hovering jet trooper's
        // camera sits where it stood, and only trails his speed ([`jet_camera_lag`],
        // `analysis/re-jetpack.md` §16).
        Stance::Sprint if !bf1 => (1.8, 0.0, 4.0, 10.0),
        Stance::Sprint => (1.8, 0.0, 4.3, 5.0),
        _ if bf1 => (1.8, 0.0, 3.5, 5.0),
        _ => (1.8, 0.0, 3.2, 10.0),
    };
    CameraSection {
        eye_y,
        offset: Vec3::new(0.0, y, z),
        tilt: f32::to_radians(deg),
    }
}

/// One game update of the chase camera's lag behind a jetting soldier, `vel` his
/// velocity while he jets and `None` once he does not. On the PS2 the camera eases at
/// 15 per second, run once per 30 Hz update, so the lag settles at 0.0514 s of his
/// velocity on all three axes (measured live: 0.54 m at 10.5 m/s, 1.83 m at 35.9 m/s;
/// `analysis/re-jetpack.md` §16). Being an ease, it grows and dies away over a few
/// updates, so starting or ending a jet at speed does not jump the camera.
pub fn jet_camera_lag(lag: Vec3, vel: Option<Vec3>) -> Vec3 {
    const EASE: f32 = 0.606_530_66; // e^-0.5: 15 per second over one 30 Hz update
    (lag + vel.unwrap_or(Vec3::ZERO) * crate::TICK_DT) * EASE
}

/// A human's zoom (`analysis/re-zoom.md` §3-§4): the PS2's zoom flag with
/// the weapon's zoom value and turn divisor at the step it is on, and
/// whether the weapon's zoom is the scope view (`ZoomFirstPerson`). Written
/// only by `Sim::tick_zoom` and the weapon switch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zoom {
    /// 0 = `ZoomMin`, 1 = `ZoomMax`.
    pub step: u8,
    pub value: f32,
    pub divisor: f32,
    pub scope: bool,
}

pub fn eye_back(rules: GroundRules, zoomed: bool) -> f32 {
    match rules {
        GroundRules::Bf2 if zoomed => 0.6,
        _ => 0.3,
    }
}

const BF1_PRONE_EYE_LEFT: f32 = -0.12;

/// The fire point's two tables by stance (§10.2 step 2, the same numbers in
/// both games): T0 in the body's frame (x to the soldier's LEFT, y up, z
/// forward), T2 in the aim's (x left, y the aim's up, z along the aim).
fn fire_point_tables(stance: Stance) -> (Vec3, Vec3) {
    match stance {
        Stance::Crouch => (Vec3::new(-0.1, 1.2, 0.0), Vec3::new(-0.12, -0.05, 0.25)),
        Stance::Prone => (Vec3::new(0.06, 0.6, 0.4), Vec3::new(-0.12, -0.20, 0.15)),
        _ => (Vec3::new(0.06, 1.7, 0.0), Vec3::new(-0.12, -0.10, 0.25)),
    }
}

/// The camera the PS2 is about to draw, without its smoothing and collision
/// (§10.2 step 4): where it stands and its forward and up rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IdealCamera {
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    /// The track centre (= the eye point): what the camera's collision pulls
    /// toward.
    pub center: Vec3,
}

/// The point the acquisition's LOS ray aims at, above the feet by stance
/// (FUN_0020c680's table 0x586350: 1.4 / 0.6 / 0.2, `re-aim-assist.md`
/// §3.6). Roll takes prone's, as the capsule does (PROVISIONAL).
pub fn aim_point_height(stance: Stance) -> f32 {
    match stance {
        Stance::Crouch | Stance::Tumble | Stance::GetUp => 0.6,
        Stance::Prone | Stance::Roll | Stance::Dead => 0.2,
        _ => 1.4,
    }
}

/// One class weapon slot past the primary, as a unit is built with it
/// (card GREN, `analysis/re-grenades.md` §2.1): the PS2's slot index, its
/// channel (0 = the primary trigger's, 1 = the secondary's), the weapon and
/// its `WeaponAmmo` in clips (0 = infinite).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotSpec {
    pub slot: u8,
    pub channel: u8,
    /// Index into `Sim::weapon_classes`.
    pub weapon_class: usize,
    pub clips: u32,
}

/// A weapon a unit holds besides the one in its hands ([`Unit::held`]).
#[derive(Clone, Debug)]
pub struct HeldWeapon {
    pub slot: u8,
    pub channel: u8,
    /// Index into `Sim::weapon_classes`.
    pub class: usize,
    pub state: WeaponState,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub choked: Option<crate::hero_weapon::Choked>,
    pub burn: Option<crate::hero_weapon::Burn>,
    pub gen: u32,
    pub alive: bool,
    pub team: u8,
    pub is_player: bool,
    pub controller: Controller,
    pub soldier: Soldier,
    pub consts: SoldierConstants,
    pub health: Health,
    pub weapon: WeaponState,
    /// Index into `Sim::weapon_classes`.
    pub weapon_class: usize,
    /// The class slot `weapon` came from: channel 0's CURRENT slot (the
    /// PS2's cur[0], `re-grenades.md` §2.2).
    pub weapon_slot: u8,
    /// Every other slot the class gives it, each a live weapon (§2.2), in
    /// slot order. Switching channel 0 swaps one of these with `weapon`, so
    /// everything that reads `weapon` / `weapon_class` follows the switch.
    pub held: Vec<HeldWeapon>,
    /// Channel 1's current weapon: an index into `held` (the PS2's cur[1]).
    pub cur1: Option<u8>,
    /// The active channel (§2.3): 1 while the secondary trigger or its
    /// weapon's fire state has the soldier, else 0.
    pub active_channel: u8,
    /// The two switch buttons as they were last tick (bit 0 channel 0's,
    /// bit 1 channel 1's): the switch is an edge.
    pub switch_held: u8,
    /// The originals' integer AI skill 1..=20 (`core::skill`), 0 for a player
    /// or a dummy. Written at spawn/respawn from `Sim::difficulty`
    /// (PLAN "PHASE C DESIGN" decision 4: "every bot receives the tier's
    /// skill at spawn"). HASHED: a round of skill-8 bots is a different world
    /// from a round of skill-20 ones. The VECTOR is derived per think
    /// (`Skill::from_level`), never stored.
    pub skill: u8,
    /// The unit's own aim-error block (card CLSE2). HASHED.
    pub aim: AimBlock,
    /// Card DPRX: a bot the sim treats as a HUMAN for every human-only rule,
    /// driven by the bot brain with this fixed profile (`crate::ai::proxy`).
    /// A measuring tool (sweep, arena); `None` for every unit in the game.
    /// Kept through respawns. HASHED only when set.
    pub proxy_human: Option<crate::ai::proxy::Profile>,
    /// Bookkeeping for traces/awards.
    pub kills: u32,
    pub deaths: u32,
    /// Minimal-AI scratch state (Controller::Ai).
    pub ai: AiState,
    /// Tick at which a dead unit may respawn (respawn timer, BF2 4.0 s).
    pub respawn_at: Option<u64>,
    /// Spawn-wave slot assigned at death (wave, slot) — see `SpawnWaves`.
    pub spawn_slot: Option<(u32, u32)>,
    /// The post a dead BOT will spawn at (card BSPN): drawn when its respawn
    /// timer runs out, kept until the spawn while the team may still spawn
    /// there (`Sim::keep_or_draw_spawn_post`). `None` alive, and for a human.
    pub spawn_post: Option<SpawnPost>,
    /// How to rebuild this unit on respawn.
    pub spec: UnitSpec,
    /// Soldier-class `CapturePosts` (re-cp-capture.md): soldiers/walkerdroids
    /// count toward captures, vehicles and BF1 heroes don't.
    pub capture_posts: bool,
    /// Card KNOCK (`analysis/re-knockback.md` §3): a dead unit whose body is
    /// still in the world - it flies, tumbles and slides
    /// ([`crate::soldier::Stance::Dead`]), blasts kick and push it, and it
    /// leaves after [`crate::soldier::CORPSE_SECS`] dead or at the respawn.
    /// A unit killed in a seat, or by the world, has none.
    pub body: bool,
    /// The kill's item drop waits for the dead state (B1 FUN_001ffec0 runs
    /// from the death block: a thrown soldier drops where he lands).
    pub drop_due: bool,
    /// The chunk mark (card DROIDDEATH, `analysis/re-droid-death.md` §1; B1
    /// S+0xbfd / B2 E+0xa74 bit 3): a blast's push rolled the class's
    /// `ChunkFrequency` and won. Sticky for the life: the unit bursts at its
    /// death whatever the death roll draws.
    pub chunk_mark: bool,
    /// This life ended in the chunk death ([`Event::DroidBurst`]): the body
    /// left the world at once and is never drawn.
    ///
    /// [`Event::DroidBurst`]: crate::sim::Event::DroidBurst
    pub burst: bool,
    /// The total push the WALL PASS applied to this unit this tick (`Sim::step`
    /// phase 3a, `CollisionWorld::push_out`), zero when it touched nothing.
    /// Horizontal by construction.
    ///
    /// This is the port's stand-in for the PS2's side-contact accumulator
    /// (`soldier+0x970`, RE_LOG "Local obstacle avoidance"): the steering
    /// reads it as "I am in contact with something on this side" even when the
    /// wall sweep ahead of me is clear, and takes the side-step's side from it
    /// (PLAN "PHASE A.2 DESIGN" decision 2). It is RECORDED, never re-applied
    /// — the push itself is already in `soldier.pos` — so no unit's motion,
    /// the player's least of all, depends on it.
    pub wall_push: Vec3,
    /// Tick this unit last FIRED, `u64::MAX` for never. Perception's hearing
    /// reads it (`crate::perception::scan`): a shot inside `HEAR_RADIUS`
    /// since the listener's last think puts the shooter in its memory with no
    /// line of sight at all. Written where `Event::Shot` is pushed
    /// (`Sim::step` phase 4) and HASHED — who has fired recently changes what
    /// everyone around them knows.
    pub last_shot_tick: u64,
    /// The squad order a player has on this unit (Phase D, `core::squad`):
    /// `None` for every unit that has none, which is every unit in a round
    /// nobody commands. HASHED only when `Some` — and NOTHING at all when it is
    /// `None`, not even a 0 byte (see `hash_into` below) — so a
    /// command-free sim hashes exactly as it did before Phase D — that is what
    /// keeps the replay gates' three values where they are. The only writers
    /// are `Sim::take_order` (phase 0b), `Sim::issue_command`, `Sim::clear_all`,
    /// `squad::on_unit_died` and the release in
    /// `Sim::step`'s AI write-back, all in `Sim::step`'s fixed order.
    pub command: Option<crate::squad::Command>,
    /// The order this unit was given and has not thought about yet (card
    /// SQACK): taken at its next think. HASHED only when `Some`.
    pub order: Option<crate::squad::Pending>,
    /// `(shooter, tick)` of the last bolt that HIT this unit, written next to
    /// `Event::Hit` in `Sim::resolve_bolt`. Being shot records the shooter at
    /// any range (design decision 2's hearing rule), which is what stops a
    /// unit standing still while it is killed from behind. Hashed.
    pub last_hit: Option<(UnitHandle, u64)>,
    /// The seat this unit rides in (card VENT, `crate::vehicle`), the unit's
    /// half of the two-sided occupancy; written only by `Sim::seat_unit` /
    /// `Sim::unseat_unit`. HASHED only when `Some`, like `command`, so an
    /// unmounted sim hashes as before.
    pub mount: Option<crate::vehicle::Mount>,
    /// BF2's timed offense / defense buffs (card PWUP, `sim/pickups.rs`).
    /// HASHED only while one runs.
    pub buffs: crate::sim::Buffs,
    pub cloak: crate::cloak::CloakState,
    /// The zoom of the weapon in hand, `None` when not zoomed: every bot,
    /// always. HASHED only when `Some`, so a round nobody zooms in hashes as
    /// it did.
    pub zoom: Option<Zoom>,
    pub first_person: bool,
    pub droideka: Option<Box<crate::droideka::Droideka>>,
}

impl Unit {
    pub fn active_weapon(&self) -> (usize, u8, &WeaponState) {
        (self.active_channel == 1)
            .then(|| self.held.get(usize::from(self.cur1?)))
            .flatten()
            .map_or((self.weapon_class, self.weapon_slot, &self.weapon), |h| {
                (h.class, h.slot, &h.state)
            })
    }

    pub fn capsule(&self) -> Capsule {
        Capsule {
            base: self.soldier.pos,
            height: capsule_height(self.soldier.stance),
            radius: SOLDIER_RADIUS,
        }
    }

    /// Issue #41: what a bolt's flight `from` along the unit `dir` meets of
    /// this soldier within `max_t`, the nearest of his bolt prims - the
    /// PS2's sweep keeps the nearest (B2 FUN_00168318, B1 FUN_00167b90) and
    /// tests only these (BF2 mask 0x400: coll_head, coll_torso, coll_body;
    /// FUN_002517e8 B2:170187-170556, analysis/re-hitzones.md). BF2's posture
    /// lists on both games' maps (BF1 has no torso and a taller body; one
    /// gameplay), placed in the soldier's frame (+z his facing) and scaled
    /// by `CollisionRootScale` `s` (the head's radius by max(s, 1)). The
    /// head's place is the posture list's; BF2 moves it each frame to a
    /// point of the animation (FUN_0024f350), not ported (PROVISIONAL).
    pub fn bolt_hit(&self, from: Vec3, dir: Vec3, max_t: f32) -> Option<(f32, HitZone)> {
        self.bolt_prims().hit(from, dir, max_t)
    }

    /// What [`Unit::bolt_hit`] reads of him, taken once a tick by the sweep.
    pub fn bolt_prims(&self) -> BoltPrims {
        BoltPrims {
            pos: self.soldier.pos,
            yaw: self.soldier.yaw,
            stance: self.soldier.stance,
            scale: self.consts.collision_scale,
        }
    }

    /// Aim frame from yaw/pitch (render::Camera convention: yaw 0 = -Z,
    /// positive yaw left): (forward, right, up).
    pub fn aim_frame(&self) -> (Vec3, Vec3, Vec3) {
        let (sy, cy) = fmath::sin_cos(self.soldier.yaw);
        let (sp, cp) = fmath::sin_cos(self.soldier.pitch);
        let fwd = Vec3::new(-sy * cp, sp, -cy * cp);
        let right = Vec3::new(cy, 0.0, -sy);
        let up = right.cross(fwd).normalize();
        (fwd, right, up)
    }

    pub fn muzzle(&self) -> Vec3 {
        self.soldier.pos + Vec3::Y * muzzle_height(self.soldier.stance)
    }

    pub fn launch_origin(&self, hand: Option<(f32, f32)>) -> Vec3 {
        match hand {
            Some((up, forward)) => {
                let (sy, cy) = fmath::sin_cos(self.soldier.yaw);
                self.soldier.pos + Vec3::Y * up + Vec3::new(-sy, 0.0, -cy) * forward
            }
            None => self.muzzle(),
        }
    }

    /// The acquisition's bounds (`re-aim-assist.md` §3.6): the box centre
    /// and half its diagonal. The box's 4 mm horizontal offset is dropped,
    /// so the centre is straight above the feet.
    pub fn aim_bounds(&self) -> (Vec3, f32) {
        let mid = (AIM_BOX_MIN + AIM_BOX_MAX) * 0.5;
        let r = (AIM_BOX_MAX - AIM_BOX_MIN).length() * 0.5;
        (self.soldier.pos + Vec3::Y * mid.y, r)
    }

    /// The camera section this unit takes now: the stance's, or its ZOOM
    /// section while zoomed ([`camera_section`], one table on every map).
    pub fn section(&self) -> CameraSection {
        let own = self
            .droideka
            .as_deref()
            .zip(self.droideka_class())
            .and_then(|(d, c)| c.camera_section(d.form(), self.zoom.is_some()));
        let index = match (self.soldier.stance, self.zoom.is_some()) {
            (Stance::Crouch, false) => 2,
            (Stance::Crouch, true) => 3,
            (Stance::Prone, false) => 4,
            (Stance::Prone, true) => 5,
            (_, true) => 1,
            (Stance::Sprint, false) => 6,
            _ => 0,
        };
        own.or(self.consts.hero_cameras[index])
            .unwrap_or_else(|| camera_section(self.soldier.stance, self.zoom.is_some()))
    }

    /// Is the view the scope's (zoomed on a `ZoomFirstPerson` weapon)?
    pub fn scoped(&self) -> bool {
        self.zoom.is_some_and(|z| z.scope)
    }

    pub fn first_person_view(&self) -> bool {
        self.first_person
            && self.alive
            && self.mount.is_none()
            && self.droideka.is_none()
            && !matches!(
                self.soldier.stance,
                Stance::Thrown | Stance::Tumble | Stance::GetUp | Stance::Dead
            )
    }

    pub fn eye_view(&self) -> bool {
        self.scoped() || self.first_person_view()
    }

    pub fn scope_path(&self) -> bool {
        self.zoom.is_some() && (self.scoped() || self.first_person_view())
    }

    pub fn look_scale(&self) -> f32 {
        self.zoom.map_or(1.0, |z| 1.0 / z.value.max(1.0))
    }

    /// The point the scope looks from and the scoped shot leaves (§10.2
    /// step 7): BF1 the fire point; BF2 the shoulder pivot plus the gun's
    /// reach along the aim, without the fire point's two sideways terms.
    pub fn scope_point(&self, rules: GroundRules) -> Vec3 {
        match rules {
            GroundRules::Bf1 => self.fire_point(),
            GroundRules::Bf2 => {
                let (_, t2) = fire_point_tables(self.soldier.stance);
                self.shoulder_pivot() + self.aim_frame().0 * t2.z
            }
        }
    }

    pub fn shoulder_pivot(&self) -> Vec3 {
        let (t0, _) = fire_point_tables(self.soldier.stance);
        let (_, right, _) = self.aim_frame();
        let ahead = Vec3::new(right.z, 0.0, -right.x);
        self.soldier.pos + Vec3::Y * t0.y + ahead * t0.z - right * t0.x
    }

    pub fn eye_point(&self, rules: GroundRules) -> Vec3 {
        match rules {
            GroundRules::Bf1 if self.scope_path() => {
                if self.soldier.stance == Stance::Prone {
                    self.shoulder_pivot() - self.aim_frame().1 * BF1_PRONE_EYE_LEFT
                } else {
                    self.fire_point()
                }
            }
            _ => self.shoulder_pivot(),
        }
    }

    /// The ideal chase camera (§10.2 step 4): the frame of the aim turned
    /// down by the section's tilt, standing at the track centre plus the
    /// section's offset in that frame. `views` is the number of views drawn
    /// (split screen takes [`SPLIT_TILT`] of the tilt). In the scope view
    /// (`re-zoom.md` §5) and in first person it is the eye [`eye_back`]
    /// behind [`Unit::eye_point`], looking along the aim with no tilt.
    pub fn ideal_camera(&self, rules: GroundRules, views: usize) -> IdealCamera {
        let (d, right, up) = self.aim_frame();
        if self.eye_view() {
            let position = self.eye_point(rules) - d * eye_back(rules, self.zoom.is_some());
            return IdealCamera {
                position,
                forward: d,
                up,
                center: position,
            };
        }
        let sec = self.section();
        let tilt = sec.tilt * if views > 1 { SPLIT_TILT } else { 1.0 };
        let (st, ct) = fmath::sin_cos(tilt);
        let forward = d * ct - up * st;
        let cam_up = up * ct + d * st;
        let center = self.soldier.pos + Vec3::Y * sec.eye_y;
        IdealCamera {
            position: center + right * sec.offset.x + cam_up * sec.offset.y
                - forward * sec.offset.z,
            forward,
            up: cam_up,
            center,
        }
    }

    /// A human's aim ray (§10.2 step 4): it leaves the ideal camera's
    /// position along the aim, its origin A slid forward until it is abreast
    /// of the eye point. The reticle is where this ray goes on screen and the
    /// acquisition looks along it. In the scope view it leaves the scope
    /// point (step 7).
    pub fn aim_ray(&self, rules: GroundRules, views: usize) -> (Vec3, Vec3) {
        let d = self.aim_frame().0;
        if self.scope_path() {
            return (self.scope_point(rules), d);
        }
        if self.first_person_view() {
            return (self.fire_point(), d);
        }
        let cam = self.ideal_camera(rules, views);
        (cam.position + d * (cam.center - cam.position).dot(d), d)
    }

    /// The fire point F (§10.2 step 2): a shoulder pivot by stance with the
    /// gun ahead along the aim, below it and to the right. A table, not the
    /// weapon model's hardpoint. Only a human's shot leaves it today (bots
    /// keep [`Unit::muzzle`], card FPNT).
    pub fn fire_point(&self) -> Vec3 {
        let (t0, t2) = fire_point_tables(self.soldier.stance);
        let (d, right, up) = self.aim_frame();
        let ahead = Vec3::new(right.z, 0.0, -right.x);
        self.soldier.pos + Vec3::Y * t0.y + ahead * t0.z - right * (t0.x + t2.x)
            + up * t2.y
            + d * t2.z
    }

    /// Where the acquisition's LOS ray aims ([`aim_point_height`]).
    pub fn aim_point(&self) -> Vec3 {
        self.soldier.pos + Vec3::Y * aim_point_height(self.soldier.stance)
    }

    /// Is `p` inside this unit's acquisition box, taken in its model frame
    /// (the self-box skip, `re-aim-assist.md` §3.6)? The frame is the
    /// unit's heading: x along its right, z along its facing (the box is
    /// symmetric in z to 4 mm, so that sign hardly matters).
    pub fn in_aim_box(&self, p: Vec3) -> bool {
        let (sy, cy) = fmath::sin_cos(self.soldier.yaw);
        let d = p - self.soldier.pos;
        let local = Vec3::new(d.x * cy - d.z * sy, d.y, -d.x * sy - d.z * cy);
        local.cmpge(AIM_BOX_MIN).all() && local.cmple(AIM_BOX_MAX).all()
    }

    /// The PS2's object kind (`+0xac & 7`, `re-aim-assist.md` §3.6), the
    /// bit the weapon's `target_kind` tests. The PS2 copies it from the
    /// class (class+0x48); the port takes the unit's HealthType, which uses
    /// the same numbering (person 0, animal 1, droid 2 …) — [D], PROVISIONAL.
    pub fn kind(&self) -> u32 {
        self.health.health_type as u32
    }

    /// Feed into the sim hash.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&self.gen.to_le_bytes());
        if self.consts.collision_damage_immune || self.spec.consts.collision_damage_immune {
            eat(b"COLLISION_IMMUNE");
            eat(&[
                u8::from(self.consts.collision_damage_immune),
                u8::from(self.spec.consts.collision_damage_immune),
            ]);
        }
        eat(&[
            self.alive as u8,
            self.team,
            self.is_player as u8,
            self.skill,
        ]);
        self.soldier.hash_into(eat);
        eat(&self.health.cur.to_bits().to_le_bytes());
        self.weapon.hash_into(eat);
        self.ai.hash_into(eat);
        self.aim.hash_into(eat);
        // Humans (including the proxy) use the current error direction;
        // only bots interpolate from the previous one (gunner_foot::error_dir).
        if !self.is_human() {
            self.aim.hash_history_into(eat);
        }
        eat(&self.respawn_at.unwrap_or(u64::MAX).to_le_bytes());
        let (w, s) = self.spawn_slot.unwrap_or((u32::MAX, u32::MAX));
        eat(&w.to_le_bytes());
        eat(&s.to_le_bytes());
        // Eaten only while a dead bot holds a choice, so a sim nobody has
        // died in keeps its hash (the pinned command-free format).
        if let Some(post) = self.spawn_post {
            post.hash_into(eat);
        }
        for v in [self.wall_push.x, self.wall_push.y, self.wall_push.z] {
            eat(&v.to_bits().to_le_bytes());
        }
        if self.body {
            eat(&[1, self.drop_due as u8]);
        }
        // Card DROIDDEATH: only once set, so a sim without droids keeps its hash.
        if self.chunk_mark || self.burst {
            eat(&[2, self.chunk_mark as u8, self.burst as u8]);
        }
        // Phase D: the squad order, and NOTHING AT ALL when there is none (not
        // even a 0 byte — brief addendum 3). That is what keeps a command-free
        // sim's hash byte-identical to the pre-Phase-D one, which is the whole
        // reason the three replay gate values do not move; the cost is that the
        // stream is only self-delimiting because `Command::hash_into` leads
        // with a 1 tag (`squad.rs`).
        if let Some(c) = self.command {
            c.hash_into(eat);
        }
        if let Some(p) = self.order {
            p.hash_into(eat);
        }
        eat(&self.last_shot_tick.to_le_bytes());
        match self.last_hit {
            Some((h, t)) => {
                eat(&[1]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
                eat(&t.to_le_bytes());
            }
            None => eat(&[0]),
        }
        // Which CLASS this unit came out of its team's table as. A class
        // change is state: the same slot respawned as a sniper instead of a
        // trooper is a different world, and the weapon/health/constants it
        // carries are already hashed through `weapon`/`health`/`soldier`
        // only indirectly (two classes can share a rifle).
        eat(&[self.spec.class]);
        // The other slots (GREN): nothing at all for a unit that holds one
        // weapon, so a sim with no class slots hashes as it did.
        if !self.held.is_empty() {
            eat(&[
                self.held.len() as u8,
                self.weapon_slot,
                self.cur1.unwrap_or(u8::MAX),
                self.active_channel,
                self.switch_held,
            ]);
            for w in &self.held {
                eat(&[w.slot]);
                w.state.hash_into(eat);
            }
        }
        if let Some(m) = self.mount {
            m.hash_into(eat);
        }
        if let Some(choked) = self.choked {
            eat(b"CHOKED");
            eat(&choked.left.to_bits().to_le_bytes());
            eat(&choked.age.to_bits().to_le_bytes());
        }
        if let Some(burn) = self.burn {
            burn.hash_into(eat);
        }
        self.cloak.hash_into(eat);
        if self.buffs.active() {
            self.buffs.hash_into(eat);
        }
        if let Some(z) = self.zoom {
            eat(&[0x5a, z.step, z.scope as u8]);
            eat(&z.value.to_bits().to_le_bytes());
            eat(&z.divisor.to_bits().to_le_bytes());
        }
        if self.first_person {
            eat(b"FP");
        }
        // Card DPRX: nothing at all for a unit that is not a proxy.
        if let Some(p) = &self.proxy_human {
            p.hash_into(eat);
        }
        if let Some(d) = &self.droideka {
            d.hash_into(eat);
        }
    }

    /// Is this unit a human for the human-only rules: a player, or a bot
    /// standing in for one (card DPRX, [`Unit::proxy_human`]). The cheats and
    /// the rules about the unit as a SHOOTER keep asking `is_player`.
    pub fn is_human(&self) -> bool {
        self.is_player || self.proxy_human.is_some()
    }
}

/// A soldier's place, turn, stance and `CollisionRootScale`: all a bolt's
/// test of his prims reads (`Unit::bolt_hit`).
#[derive(Clone, Copy, Debug)]
pub struct BoltPrims {
    pub pos: Vec3,
    pub yaw: f32,
    pub stance: Stance,
    pub scale: f32,
}

/// OURS (playtest 2026-10-28, BF1 Yavin 4: "I'd see the bullet hit the guy
/// but nothing would happen"): how far behind the sim a human sees the
/// soldiers he shoots at. The window draws the world blended between the
/// last two ticks (`game::render_blend`) and his press is taken by the next
/// tick, after everyone has moved: the body he aimed at is 1 to 2 ticks
/// (60 fps: 1.5 to 2) behind the one the bolt meets - at a run 0.3 to 0.6 m,
/// past the 0.35-0.4 m half width of the torso prims. The sweep takes back
/// part of it: a tick's flight meets the soldiers where that tick left
/// them, on average half a tick before the bolt gets there. A human's bolt
/// (and his reticle's ray, `Sim::aim_hit`) meets each soldier this long ago
/// along his velocity: the middle of what is left (0.5 to 2 ticks at
/// 60 fps). The PS2 drew its last update with no blend. Bots' bolts meet
/// the soldiers where they are.
pub const HUMAN_SHOT_LAG: f32 = 1.25 * crate::TICK_DT;

/// OURS (the maintainer, 2026-10-29): a round covering this much in one
/// tick is instant to a player: he aims at the body, not ahead of it. The
/// discs' rounds are 600 m/s (20 m a tick) or less, or 2000 m/s (66.7 m:
/// the sniper rifles, BF2's shotgun pellets) and over.
pub const FAST_ROUND_TICK_M: f32 = 50.0;

/// Is a round of `speed` (m/s) instant to a player ([`FAST_ROUND_TICK_M`])?
pub fn fast_round(speed: f32) -> bool {
    speed * crate::TICK_DT >= FAST_ROUND_TICK_M
}

/// How far back in time a human's bolt of `speed` that has flown `age`
/// meets the soldiers: [`HUMAN_SHOT_LAG`], and for a [`fast_round`] its
/// age too (OURS, 2026-10-29), so it meets a soldier where the player saw
/// him at the press however many ticks it flies (2000 m/s: past 66.7 m it
/// meets him in its second tick, a tick further on than drawn).
pub fn human_meet_lag(speed: f32, age: f32) -> f32 {
    if fast_round(speed) {
        HUMAN_SHOT_LAG + age
    } else {
        HUMAN_SHOT_LAG
    }
}

impl BoltPrims {
    /// The same prims `by` away (a human's bolt, [`HUMAN_SHOT_LAG`]).
    pub fn moved(self, by: Vec3) -> Self {
        Self {
            pos: self.pos + by,
            ..self
        }
    }

    /// The prims' reach around `pos`: (lower corner, upper corner).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let r = 1.2 * self.scale.max(1.0);
        (
            self.pos - Vec3::new(r, 0.0, r),
            self.pos + Vec3::new(r, 2.1 * self.scale.max(1.0), r),
        )
    }

    /// See [`Unit::bolt_hit`].
    pub fn hit(&self, from: Vec3, dir: Vec3, max_t: f32) -> Option<(f32, HitZone)> {
        let s = self.scale;
        let (sy, cy) = fmath::sin_cos(self.yaw);
        let fwd = Vec3::new(-sy, 0.0, -cy);
        let right = Vec3::new(cy, 0.0, -sy);
        let at = |x: f32, y: f32, z: f32| self.pos + right * x + Vec3::Y * y + fwd * z;
        let head_r = 0.17 * s.max(1.0);
        let mut best: Option<(f32, HitZone)> = None;
        let mut take = |t: Option<f32>, z: HitZone| {
            if let Some(t) = t.filter(|t| *t <= max_t && best.is_none_or(|b| *t < b.0)) {
                best = Some((t, z));
            }
        };
        match self.stance {
            Stance::Prone | Stance::Roll | Stance::Dead => {
                take(
                    ray_sphere(from, dir, at(0.0, 0.28 * s, 0.75 * s), head_r),
                    HitZone::Head,
                );
                let half = Vec3::new(0.25, 0.2, 0.87 * s);
                let c = at(0.0, 0.2, -0.25 * s);
                take(ray_yawed_box(from, dir, c, right, fwd, half), HitZone::Body);
            }
            Stance::Crouch => {
                take(
                    ray_sphere(from, dir, at(0.0, 1.2 * s, 0.25), head_r),
                    HitZone::Head,
                );
                take(
                    ray_sphere(from, dir, at(0.0, 0.7 * s, 0.0), 0.4 * s),
                    HitZone::Torso,
                );
                let c = at(0.0, 0.35 * s, 0.0);
                take(
                    ray_upright_cylinder(from, dir, c, 0.35, 0.35 * s),
                    HitZone::Body,
                );
            }
            _ => {
                take(
                    ray_sphere(from, dir, at(0.0, 1.65 * s, 0.16), head_r),
                    HitZone::Head,
                );
                take(
                    ray_sphere(from, dir, at(0.0, 1.15 * s, 0.0), 0.4 * s),
                    HitZone::Torso,
                );
                let c = at(0.0, 0.55 * s, 0.0);
                take(
                    ray_upright_cylinder(from, dir, c, 0.35, 0.55 * s),
                    HitZone::Body,
                );
            }
        }
        best
    }
}

/// The bolt prim a hit met (`Unit::bolt_hit`): the head takes the PS2's
/// default `HitLocation` x3 (B2:170444-170450, B1:122725; issue #41).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitZone {
    Head,
    Torso,
    Body,
}

/// First `t >= 0` where the ray meets the sphere (0 when it starts inside).
pub(crate) fn ray_sphere(o: Vec3, d: Vec3, c: Vec3, r: f32) -> Option<f32> {
    let m = o - c;
    let b = m.dot(d);
    let cc = m.length_squared() - r * r;
    if cc <= 0.0 {
        return Some(0.0);
    }
    if b > 0.0 {
        return None;
    }
    let disc = b * b - cc;
    (disc >= 0.0).then(|| -b - disc.sqrt())
}

/// An upright cylinder of radius `r` and half height `hh` centred `c`.
pub(crate) fn ray_upright_cylinder(o: Vec3, d: Vec3, c: Vec3, r: f32, hh: f32) -> Option<f32> {
    let (lo, hi) = (c.y - hh, c.y + hh);
    let (mx, mz) = (o.x - c.x, o.z - c.z);
    let a = d.x * d.x + d.z * d.z;
    let inside = mx * mx + mz * mz <= r * r;
    // The side's interval [t0, t1] (all t when the ray runs straight up).
    let (t0, t1) = if a < 1e-12 {
        if !inside {
            return None;
        }
        (f32::NEG_INFINITY, f32::INFINITY)
    } else {
        let b = mx * d.x + mz * d.z;
        let cc = mx * mx + mz * mz - r * r;
        let disc = b * b - a * cc;
        if disc < 0.0 {
            return None;
        }
        let q = disc.sqrt();
        ((-b - q) / a, (-b + q) / a)
    };
    // The caps' interval.
    let (s0, s1) = if d.y.abs() < 1e-12 {
        if o.y < lo || o.y > hi {
            return None;
        }
        (f32::NEG_INFINITY, f32::INFINITY)
    } else {
        let (a0, a1) = ((lo - o.y) / d.y, (hi - o.y) / d.y);
        (a0.min(a1), a0.max(a1))
    };
    let (enter, exit) = (t0.max(s0), t1.min(s1));
    (enter <= exit && exit >= 0.0).then(|| enter.max(0.0))
}

/// A box of half extents `half` (along `right`, up, `fwd`) centred `c`.
pub(crate) fn ray_yawed_box(
    o: Vec3,
    d: Vec3,
    c: Vec3,
    right: Vec3,
    fwd: Vec3,
    half: Vec3,
) -> Option<f32> {
    let m = o - c;
    let (lo, ld) = (
        Vec3::new(m.dot(right), m.y, m.dot(fwd)),
        Vec3::new(d.dot(right), d.y, d.dot(fwd)),
    );
    let (mut enter, mut exit) = (f32::NEG_INFINITY, f32::INFINITY);
    for k in 0..3 {
        if ld[k].abs() < 1e-12 {
            if lo[k].abs() > half[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((-half[k] - lo[k]) / ld[k], (half[k] - lo[k]) / ld[k]);
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
    }
    (enter <= exit && exit >= 0.0).then(|| enter.max(0.0))
}

/// A droid class's chunk death (card DROIDDEATH, `analysis/re-droid-death.md`;
/// B1 B1:116889-116968, B2 B2:159605-159669 [V]): at the death block one
/// draw U, and `U <= frequency` or the unit's [`Unit::chunk_mark`] bursts it -
/// the `explosion` at the body, the body out of the world, no corpse, the
/// item drop as on any death. Only a class with chunk geometry
/// (`ChunkGeometryName`) has one: the CIS soldier droids of both games.
#[derive(Clone, Debug)]
pub struct ChunkDeath {
    /// `ChunkFrequency` (B1 class+0x1e58, B2 class+0x121c; BF2's ctor -1.0).
    pub frequency: f32,
    /// The class's `ExplosionName` class (B1 class+0x1e6c, B2 +0x1238).
    pub explosion: Option<crate::weapon::ExplosionClass>,
}

/// The chunks' launch scale at the burst (B1:116957, B2:159656): 1.0 at
/// health -50 or below, else 0.1 + 0.018 x the overkill.
pub fn chunk_scale(health: f32) -> f32 {
    if health <= -50.0 {
        1.0
    } else {
        -health * 0.018 + 0.1
    }
}

/// One entry of a team's class table (`Sim::classes[team]`): everything the
/// sim needs to BUILD a unit of that class, plus the mission's per-class
/// counts. Built by the game layer from the ODF chain
/// (`game::roster::ClassRoster`); `core` never reads a class file.
///
/// SETUP data, like `Sim::weapon_classes`: written before the round starts,
/// never mutated by `step`, and NOT part of [`crate::sim::Sim::hash`] — what
/// each unit took out of it (`UnitSpec::class`) is.
#[derive(Clone, Debug)]
pub struct UnitClass {
    pub consts: SoldierConstants,
    pub max_health: f32,
    pub health_type: HealthType,
    /// Index into `Sim::weapon_classes` (the class's PRIMARY weapon).
    pub weapon_class: usize,
    /// Authored WeaponAmmo: total clips including a loaded clip; zero is unlimited.
    pub total_clips: u32,
    /// The primary's class slot and the class's other slots (GREN).
    pub weapon_slot: u8,
    pub slots: std::sync::Arc<[SlotSpec]>,
    pub jet: bool,
    pub capture_posts: bool,
    /// What a unit of this class drops when it dies (card PWUP).
    pub drops: crate::sim::DropTable,
    /// The droid's chunk death (card DROIDDEATH); `None` for a class without
    /// chunk geometry, which never bursts.
    pub chunk: Option<ChunkDeath>,
    /// Mission `AddUnitClass` count / `SetupTeams` slot minimum — how many of
    /// this class the team fields before the fill class takes over
    /// (`Sim::ai_class`).
    pub min: u32,
    /// BF2 `SetupTeams` slot maximum; BF1 authors none.
    pub max: Option<u32>,
    /// The ODF's `PointsToUnlock` (BF2: every officer 8, the specials 12; BF1
    /// none = 0): the points a HUMAN needs to take this class, and with a
    /// cost the class's `max` caps the humans' pick too
    /// (`analysis/re-spawn-flow.md` §4.3, [`crate::sim::Sim::class_refusal`]).
    /// Bots are never refused.
    pub unlock_points: u32,
    pub droideka: Option<std::sync::Arc<crate::droideka::DroidekaClass>>,
}

impl UnitClass {
    /// A spawn spec for this class. `class` is the class's own index in the
    /// team's table — the caller passes it because the table does not know
    /// where it lives.
    /// `skill` is the difficulty tier's base skill; it reaches the spec only
    /// on an `Ai` controller (a player and a dummy carry 0). `Sim::spawn`
    /// enforces the same rule on every other spawn path.
    pub fn spec(
        &self,
        team: u8,
        pos: Vec3,
        yaw: f32,
        controller: Controller,
        class: u8,
        skill: u8,
    ) -> UnitSpec {
        UnitSpec {
            team,
            player_slot: match controller {
                Controller::Player { slot } => Some(slot),
                _ => None,
            },
            controller,
            pos,
            yaw,
            consts: self.consts,
            max_health: self.max_health,
            health_type: self.health_type,
            weapon_class: self.weapon_class,
            total_clips: self.total_clips,
            weapon_slot: self.weapon_slot,
            slots: self.slots.clone(),
            capture_posts: self.capture_posts,
            jet: self.jet,
            drops: self.drops,
            class,
            skill: crate::skill_for(controller, skill),
            droideka: self.droideka.clone(),
        }
    }
}

/// The block every BF2 unit carries about ITSELF as a target, and the
/// direction its own aim error points (card CLSE2, `analysis/re-soldier-ai.md`
/// 18.1: B2 `FUN_002ae010` / `FUN_002ae7b8`). Every bot that shoots at the
/// unit takes the unit's `err` (18.6: knocked in on the running PS2); `dir`
/// is the unit's own, for the shots IT fires. `Sim::step` thinks it: a bot's
/// every 5th tick, a human's every tick. A BF1 round has no block (18.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimBlock {
    /// The error, metres before the radius's factors
    /// (`ai::gunner_foot::radius`): it decays to the skill's floor and grows
    /// with the unit's CHANGE of velocity, at most [`crate::ai::ERR_MAX`].
    pub err: f32,
    /// What growth is paid from: it fills at half the decay, at most
    /// [`AimBlock::BUDGET_MAX`], so the error can rise by no more than half
    /// of what it decays plus what was saved up.
    pub budget: f32,
    /// The unit's velocity at the last think.
    pub vel: Vec3,
    /// The axis the direction turns about; it walks (a U(-1, 1) cube is
    /// added every think and the sum normalised).
    pub axis: Vec3,
    /// The turn so far: 2 x dt radians about `axis` every think.
    pub quat: glam::Quat,
    /// `quat` applied to +Y: where this unit's error vector points. Zero
    /// until the first think.
    pub dir: Vec3,
    /// `dir` before the last think (zero until the second): a bot's aim
    /// turns from it to `dir` over the think's ticks
    /// (`ai::gunner_foot::error_dir`, ours).
    pub from: Vec3,
}

impl Default for AimBlock {
    /// The PS2 constructor's values (B2:232960-232990): no error, an empty
    /// budget, the identity turn.
    fn default() -> Self {
        Self {
            err: 0.0,
            budget: 0.0,
            vel: Vec3::ZERO,
            axis: Vec3::ZERO,
            quat: glam::Quat::IDENTITY,
            dir: Vec3::ZERO,
            from: Vec3::ZERO,
        }
    }
}

impl AimBlock {
    /// The budget's cap (B2:233051-233056).
    pub const BUDGET_MAX: f32 = 5.0;
    /// A bot's block thinks every this many ticks (the PS2's every 5th
    /// update at its level of detail 0 / 1; the port has one level).
    pub const PERIOD: u64 = 5;

    /// One think `dt` seconds after the last: `vel` and `top` are the unit's
    /// velocity and MaxSpeed, `decay` (m/s) and `floor` (m) the skill's of
    /// who shoots at it, `jolt` the U(-1, 1) cube the axis walks by.
    pub fn think(&mut self, dt: f32, vel: Vec3, top: f32, decay: f32, floor: f32, jolt: Vec3) {
        self.think_scaled(dt, vel, top, decay, 1.0, floor, jolt);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn think_scaled(
        &mut self,
        dt: f32,
        vel: Vec3,
        top: f32,
        decay: f32,
        own_decay: f32,
        floor: f32,
        jolt: Vec3,
    ) {
        let d = dt * decay;
        self.add(-d * own_decay, floor);
        self.budget = (self.budget + 0.5 * d).clamp(0.0, Self::BUDGET_MAX);
        let delta = (self.vel - vel).length();
        self.vel = vel;
        self.add(crate::ai::ERR_GROWTH * delta / top.max(0.1), floor);
        self.axis = (self.axis + jolt).normalize_or_zero();
        let (s, c) = fmath::sin_cos(dt);
        let turn = glam::Quat::from_xyzw(self.axis.x * s, self.axis.y * s, self.axis.z * s, c);
        // OURS: normalised, so 60,000 ticks of products do not drift.
        self.quat = (self.quat * turn).normalize();
        self.from = self.dir;
        self.dir = self.quat * Vec3::Y;
    }

    pub fn trick(&mut self, floor: f32) {
        self.add(5.0, floor);
    }

    /// B2 `FUN_002ae7b8`: growth is cut to the budget and taken off it; then
    /// the floor, and the 2 m cap.
    fn add(&mut self, mut x: f32, floor: f32) {
        if x > 0.0 {
            x = x.min(self.budget);
            self.budget -= x;
        }
        self.err += x;
        self.err = if self.err < floor {
            floor
        } else {
            self.err.min(crate::ai::ERR_MAX)
        };
    }

    /// Feed into the sim hash.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        let q = self.quat;
        for v in [
            self.err,
            self.budget,
            self.vel.x,
            self.vel.y,
            self.vel.z,
            self.axis.x,
            self.axis.y,
            self.axis.z,
            q.x,
            q.y,
            q.z,
            q.w,
        ] {
            eat(&v.to_bits().to_le_bytes());
        }
    }

    /// Extra state only for units whose next aim interpolates this history.
    /// Keeping it separate preserves the legacy human/default hash format.
    fn hash_history_into(&self, eat: &mut impl FnMut(&[u8])) {
        if self.from != Vec3::ZERO {
            eat(b"AIM_FROM");
            for v in self.from.to_array() {
                eat(&v.to_bits().to_le_bytes());
            }
        }
    }
}

/// How to build a unit (class constants resolved by the game layer).
#[derive(Clone, Debug)]
pub struct UnitSpec {
    pub team: u8,
    /// Player slot this unit belongs to (`None` = AI/dummy). The slot is what
    /// `Sim::players` indexes; [`UnitSpec::is_player`] is derived from it.
    pub player_slot: Option<u8>,
    pub controller: Controller,
    pub pos: Vec3,
    pub yaw: f32,
    pub consts: SoldierConstants,
    pub max_health: f32,
    pub health_type: HealthType,
    pub weapon_class: usize,
    /// Authored WeaponAmmo: total clips including a loaded clip; zero is unlimited.
    pub total_clips: u32,
    /// The primary's class slot (0 on every built-in spec).
    pub weapon_slot: u8,
    /// The class's other weapon slots (empty on every built-in spec).
    pub slots: std::sync::Arc<[SlotSpec]>,
    pub capture_posts: bool,
    /// Authored jet/jump-pack eligibility, loaded from `JetJump`. The planning
    /// graph keeps `jet_jump` arcs (`Arc::JET_JUMP`) off jetless routes (PLAN
    /// "PHASE A.2 DESIGN" decision 4); this flag does not implement jet flight.
    pub jet: bool,
    /// The class's drop table (card PWUP; empty on every built-in spec: a
    /// death spends no roll and drops nothing).
    pub drops: crate::sim::DropTable,
    /// The integer AI skill this unit spawns with (0 = not a bot). Filled by
    /// `Sim::spawn`/`Sim::respawn_slot` from `Sim::difficulty`; the default on
    /// every hand-built spec is 0.
    pub skill: u8,
    /// Index into the TEAM'S class table (`Sim::classes[team]`) this unit was
    /// built from. `0` on every built-in spec ([`UnitSpec::trooper`], the
    /// player constructors) — a sim with no class tables has one implied
    /// class per team and every unit is it.
    pub class: u8,
    pub droideka: Option<std::sync::Arc<crate::droideka::DroidekaClass>>,
}

impl UnitSpec {
    /// Is this a PLAYER unit (tickets, cheats, the wall pass, the alive-AI
    /// cap)? Derived: a unit is a player exactly when it owns a slot.
    pub fn is_player(&self) -> bool {
        self.player_slot.is_some()
    }

    /// Dummy target / default trooper: BF1 all_inf_soldier health 300.
    pub fn trooper(team: u8, pos: Vec3) -> Self {
        Self {
            team,
            player_slot: None,
            controller: Controller::Idle,
            pos,
            yaw: 0.0,
            consts: SoldierConstants::default(),
            max_health: 300.0,
            health_type: HealthType::Person,
            weapon_class: 0,
            total_clips: 6,
            weapon_slot: 0,
            slots: std::sync::Arc::new([]),
            jet: false,
            capture_posts: true,
            drops: crate::sim::DropTable::default(),
            class: 0,
            skill: 0,
            droideka: None,
        }
    }

    /// A player driven by pad slot `slot`.
    pub fn player_in_slot(slot: u8, team: u8, pos: Vec3) -> Self {
        Self {
            player_slot: Some(slot),
            controller: Controller::Player { slot },
            ..Self::trooper(team, pos)
        }
    }

    /// The local player: pad slot 0.
    pub fn player(team: u8, pos: Vec3) -> Self {
        Self::player_in_slot(0, team, pos)
    }

    /// `classes` is `Sim::weapon_classes`.
    pub fn build(&self, gen: u32, classes: &[WeaponClass]) -> Unit {
        let class = &classes[self.weapon_class];
        let mut soldier = Soldier::new(self.pos);
        soldier.yaw = self.yaw;
        soldier.fuel = Soldier::spawn_fuel(&self.consts);
        if self.is_player() {
            soldier.pitch = SPAWN_PITCH;
        }
        let mut held: Vec<HeldWeapon> = self
            .slots
            .iter()
            .map(|s| HeldWeapon {
                slot: s.slot,
                channel: s.channel,
                class: s.weapon_class,
                state: WeaponState::with_clips(&classes[s.weapon_class], self.is_player(), s.clips),
            })
            .collect();
        // Channel 1's current weapon is its first slot (§2.2); only the two
        // current weapons are selected.
        let cur1 = held.iter().position(|w| w.channel == 1);
        for (i, w) in held.iter_mut().enumerate() {
            w.state.active = Some(i) == cur1;
        }
        Unit {
            choked: None,
            burn: None,
            gen,
            alive: true,
            team: self.team,
            is_player: self.is_player(),
            controller: self.controller,
            soldier,
            consts: self.consts,
            health: Health::new(self.max_health, self.health_type),
            weapon: WeaponState::with_clips(class, self.is_player(), self.total_clips),
            weapon_class: self.weapon_class,
            weapon_slot: self.weapon_slot,
            held,
            cur1: cur1.map(|i| i as u8),
            active_channel: 0,
            switch_held: 0,
            skill: self.skill,
            aim: AimBlock::default(),
            proxy_human: None,
            kills: 0,
            deaths: 0,
            ai: AiState::default(),
            respawn_at: None,
            spawn_slot: None,
            spawn_post: None,
            spec: self.clone(),
            capture_posts: self.capture_posts,
            wall_push: Vec3::ZERO,
            body: false,
            drop_due: false,
            chunk_mark: false,
            burst: false,
            last_shot_tick: u64::MAX,
            last_hit: None,
            command: None,
            order: None,
            mount: None,
            buffs: crate::sim::Buffs::default(),
            cloak: crate::cloak::CloakState::default(),
            zoom: None,
            first_person: false,
            droideka: self.droideka.as_ref().map(|c| {
                let order = if self.is_player() {
                    crate::droideka::Order::Deploy
                } else {
                    crate::droideka::Order::Roll
                };
                Box::new(crate::droideka::Droideka::new(c, order))
            }),
        }
    }
}

#[cfg(test)]
mod tests;
