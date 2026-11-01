//! Animated soldier visuals: per-team skinned assets (baked by `bake
//! soldier`), per-unit animators driven by the sim's soldier state, and the
//! skinned draw list. Visual only — nothing here feeds back into the sim.
//!
//! **A rider is not drawn — but in a building turret that shows him** (card
//! VTURG, `analysis/re-turret-draw.md` 3 [O]): a soldier seated in a turret
//! whose seat has a pose key (`vehicle_class::SeatSpec::gunner_draw`) is
//! drawn at `L x node x O` (`viewer::turret_pose::GunnerHang`: the
//! `PilotPosition` hardpoint on its parent node as the turret is posed now -
//! an aimer node as the update before left it - in the building), posed by
//! the seat's clip (`Animator::seat`, the nine-pose numbers of
//! `turret_pose::nine_pose`), in every view, his own too, and without his
//! hand weapon. Hoth's laser towers have no pose key: nobody is drawn.
//!
//! **A vehicle's rider likewise** (card VRIDE, `analysis/re-vehicle-riders.md`
//! [O]): a seat with a pose key (BF2: and exposed) draws its soldier at
//! `O x hang` - the vehicle as drawn, the seat's
//! `vehicle_class::RiderHang` (the hardpoint's bind, or an
//! `AnimatedPilotPosition` node of the posed walker skin now) - posed by the
//! seat's clip: a hover's nine-pose pilot by the drive's smoothed throttle
//! and turn (`turret_pose::HoverNine`, kept per vehicle here), a vehicle
//! turret's nine-pose gunner by his gun's angles, every other seat's clip
//! played and looped. The speeder bikes, STAP, BARC, tauntaun, kaadu,
//! one-man AT-ST and the fighter tanks' gunners show their riders; the hover
//! tanks', AT-STs' and AATs' pilots have no pose key.
//!
//! **The muzzle flash** (card FXMF, `analysis/re-muzzle-flash.md` [O]) is
//! drawn here too, because it rides the held weapon: each `Shot` restarts
//! the unit's timer, and while t = (end − now) / `FlashLength` > 0 the
//! game's flash MODEL (`muzzleflash` / `com_sfx_muzzleflash`) is drawn at
//! `hp_weapons` × the weapon's fire point (its `FirePointName` node, baked
//! beside the weapon GLB; the `hp_weapons` origin when it has none), +Z along
//! the aim, scaled `FlashRadius`, tinted `FlashColor` with alpha
//! min(A, 2A·t) ([`core::flash`]), ADDITIVE in the particle list. Time runs
//! on the SIM clock (the 30 Hz tick), not the render's: every blaster's
//! 0.025-s flash is drawn for exactly the tick of its shot at full alpha, as
//! on the PS2 (§3; FEEL_NOTES). Only the guns a soldier holds flash — the
//! weapon loader gives none to sabres and thrown weapons (§5), and vehicle
//! and turret guns never reach this draw.
//!
//! **The weapon in the hand is the one in hand** (card WHELD): the soldier
//! render draws the ACTIVE weapon at `hp_weapons` (`re-muzzle-flash.md` §5,
//! `re-grenades.md` §2.3 item 4) - channel 1's current weapon while that
//! channel has the soldier, else channel 0's current one, which a switch
//! swaps ([`weapon_in_hand`]) - as its own `GeometryName` (none = nothing
//! drawn), and a thrown weapon whose `HideOnFire` is set is not drawn from
//! its throw's fire state through its reload (§3.2). The fire point and the
//! flash follow that weapon too.

use crate::animator::{Animator, ClassAnim, Rig};
use crate::assets::SkinnedAsset;
use crate::viewer::turret_pose as tp;
use crate::walk::{TeamTemplate, WalkSim};
use glam::{Mat4, Vec3};
use render::{
    ParticleDraw, ParticleShape, Renderer, SkinnedDrawCmd, SkinnedMeshHandle, TextureHandle,
};
use std::path::Path;

/// A part's specular second pass, resolved for the loaded map: its flat
/// colour (`crate::assets::SpecPass::rgb_under` the map's sun) and its power
/// — `None` for a part that draws none (queue SPEC2).
pub type PartSpec = Option<([f32; 3], u32)>;

/// A part's [`PartSpec`] from the pass its GLB material carries and the sun
/// colour on the renderer — which must already be the map's
/// (`crate::viewer::upload_scene` puts it there).
pub fn part_spec(r: &Renderer, pass: Option<crate::assets::SpecPass>) -> PartSpec {
    pass.map(|p| (p.rgb_under(r.lighting.sun_color), p.power))
}

/// A skinned part's draws: `base`, then its specular second pass when it has
/// one and is textured — the add is weighted by the base texel's alpha, so an
/// untextured part draws none (as the world's, `crate::viewer::upload_scene`).
pub fn with_spec(base: SkinnedDrawCmd, spec: PartSpec) -> Vec<SkinnedDrawCmd> {
    let second = match (base.texture, spec) {
        (Some(_), Some((rgb, power))) => Some(base.specular(rgb, power)),
        _ => None,
    };
    std::iter::once(base).chain(second).collect()
}

pub struct SoldierAsset {
    pub asset: SkinnedAsset,
    /// Per skinned submesh: mesh, texture, the pass its GLB material asks
    /// for — `Opaque`, or `Blend` for a segment the disc composites
    /// (`render::SkinnedDrawCmd::layer`) — and its specular second pass.
    pub meshes: Vec<(
        SkinnedMeshHandle,
        Option<TextureHandle>,
        render::DrawLayer,
        PartSpec,
    )>,
    /// The soldier's animation sets over `asset` (`soldiers/<model>.sets.ron`);
    /// `None` when they are not baked: the soldier stands in his bind pose.
    pub rig: Option<Rig>,
    /// Skeleton index of `hp_weapons`: where the class's [`WeaponAsset`] is
    /// drawn.
    pub hp_weapons: Option<usize>,
    /// Per part of `meshes`: the model group its texture is drawn in
    /// (`soldiers/<model>.skins.ron`, [`part_groups`]); a class's override
    /// of that group replaces the part's texture (card USKN).
    pub groups: Vec<Option<String>>,
    /// Optional live cloth companion; older GLBs retain their skinned fallback.
    pub cloths: Vec<crate::cloth::Definition>,
}

/// The model group of each part, by its texture: the groups file lists
/// the textures of the model's segments in each override group (the bake's
/// `bake::soldier::ModelSkins`). A part with no texture, or one no group
/// lists, has none.
pub fn part_groups(
    textures: &[Option<&str>],
    groups: &std::collections::BTreeMap<String, Vec<String>>,
) -> Vec<Option<String>> {
    textures
        .iter()
        .map(|t| {
            let t = (*t)?.to_ascii_lowercase();
            groups
                .iter()
                .find(|(_, list)| list.contains(&t))
                .map(|(g, _)| g.clone())
        })
        .collect()
}

/// A hand weapon model (static, drawn at the body's `hp_weapons`), cached by
/// its own `GeometryName` — apart from the body, because classes that share
/// a body hold different guns (card UWPN: BF1's clone trooper and
/// sharpshooter are both `rep_inf_trooper`).
#[derive(Default)]
pub struct WeaponAsset {
    /// Per weapon submesh: mesh, texture, and the pass its GLB material
    /// asks for. The weapon bake is NOT on ALPHA RULE v2 (design D6) — it
    /// still ORs the material flags per texture — but it writes that decision
    /// on the material, and the part is routed from it. A MASK part draws
    /// through the WORLD's mask pipeline (the disc's cutoff since queue AT);
    /// none of the 31 baked weapon GLBs has a MASK material. The last field
    /// is the part's specular second pass, as [`SoldierAsset::meshes`]'.
    /// Empty when the GLB is not baked (no weapon drawn).
    pub parts: Vec<(
        render::MeshHandle,
        Option<TextureHandle>,
        render::DrawLayer,
        PartSpec,
    )>,
    /// The weapon's `skel` nodes (`weapons/<model>.nodes.ron`, card FXMF):
    /// where its `FirePointName` puts the muzzle flash. Empty when the bake
    /// predates them or the model has none.
    pub nodes: manifest::WeaponNodes,
}

/// What one class draws: its body and the gun in its hand, each an index
/// into its own list. `None` = no model / no hand weapon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClassDraw {
    pub body: Option<usize>,
    pub weapon: Option<usize>,
}

/// Which models two teams' classes draw, before anything is loaded: every
/// distinct body and every distinct hand weapon once, and each class's pair.
/// The body and the weapon are keyed apart (card UWPN) — a class sharing its
/// body with an earlier class still holds its OWN primary's model.
#[derive(Debug, Default)]
pub struct DrawPlan {
    /// Distinct soldier `GeometryName`s, in first-use order.
    pub bodies: Vec<String>,
    /// Distinct primary-weapon `GeometryName`s, in first-use order.
    pub weapons: Vec<String>,
    /// `per_class[team][class]`, index-aligned with `TeamTemplate::classes`.
    pub per_class: [Vec<ClassDraw>; 2],
    /// `held[team][class]`: the class's other slots' models (card WHELD),
    /// `(slot, index into weapons)`; a slot not listed holds the primary's.
    pub held: [Vec<HeldDraws>; 2],
}

/// A class's other slots and the hand model each draws (`None` = none).
pub type HeldDraws = Vec<(u8, Option<usize>)>;

pub(super) fn intern_name(names: &mut Vec<String>, name: Option<&str>) -> Option<usize> {
    let name = name?;
    Some(names.iter().position(|x| x == name).unwrap_or_else(|| {
        names.push(name.to_owned());
        names.len() - 1
    }))
}

impl DrawPlan {
    pub fn of(teams: &[TeamTemplate; 2]) -> Self {
        let mut plan = Self::default();
        for (i, t) in teams.iter().enumerate() {
            for c in &t.classes {
                let draw = ClassDraw {
                    body: intern_name(&mut plan.bodies, c.geometry.as_deref()),
                    weapon: intern_name(&mut plan.weapons, c.weapon_geometry.as_deref()),
                };
                plan.per_class[i].push(draw);
                let held = c
                    .held_models
                    .iter()
                    .map(|(s, g)| (*s, intern_name(&mut plan.weapons, g.as_deref())))
                    .collect();
                plan.held[i].push(held);
            }
        }
        plan
    }

    /// The weapon model a class is drawn holding, if any.
    pub fn weapon_of(&self, team: usize, class: usize) -> Option<&str> {
        let w = self.per_class.get(team)?.get(class)?.weapon?;
        self.weapons.get(w).map(String::as_str)
    }

    /// The weapon model a class draws while its class slot `slot` is in hand
    /// (card WHELD): a listed held slot's own, else the primary's.
    pub fn weapon_in_slot(&self, team: usize, class: usize, slot: u8) -> Option<&str> {
        let w = held_or_primary(
            self.held.get(team)?.get(class)?,
            self.per_class.get(team)?.get(class)?.weapon,
            slot,
        )?;
        self.weapons.get(w).map(String::as_str)
    }
}

/// The model index of class slot `slot`: a held slot's own entry (which may
/// be no model), else the primary's.
fn held_or_primary(
    held: &[(u8, Option<usize>)],
    primary: Option<usize>,
    slot: u8,
) -> Option<usize> {
    held.iter()
        .find(|(s, _)| *s == slot)
        .map_or(primary, |(_, w)| *w)
}

/// What a soldier holds now (card WHELD, `re-grenades.md` §2.3): channel 1's
/// current weapon while that channel has him, else channel 0's current one
/// (`Unit::weapon`, which a switch swaps with a held slot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InHand {
    /// Index into `Sim::weapon_classes`.
    pub class: usize,
    /// The class slot it came from.
    pub slot: u8,
    /// A thrown weapon with `HideOnFire` in its throw or its reload: the
    /// soldier render draws no model (weapon+0xbc, §3.2).
    pub hidden: bool,
}

/// [`InHand`] for `u`, its weapon classes `classes`.
pub fn weapon_in_hand(u: &core::unit::Unit, classes: &[core::weapon::WeaponClass]) -> InHand {
    in_hand(classes, u.active_weapon())
}

fn in_hand(
    classes: &[core::weapon::WeaponClass],
    (class, slot, state): (usize, u8, &core::weapon::WeaponState),
) -> InHand {
    let hides = classes
        .get(class)
        .and_then(|c| c.thrown.as_ref())
        .is_some_and(|t| t.hide_on_fire);
    let fired = state.state == core::weapon::FireState::Firing
        && (classes.get(class).is_none_or(|c| c.mine.is_none()) || state.salvo_counter == 0);
    InHand {
        class,
        slot,
        hidden: hides && (fired || state.reloading()),
    }
}

/// Triangles as (position, UV), three per triangle.
pub type Tris = Vec<(Vec3, [f32; 2])>;

/// The game's muzzle flash model (card FXMF), loaded once: its triangles in
/// model space, one list per sheet.
pub struct FlashModel {
    /// The model's name (`core::flash::BF1_MODEL` / `BF2_MODEL`).
    pub name: String,
    pub parts: Vec<(Option<TextureHandle>, Tris)>,
}

impl FlashModel {
    /// `common_<game>/models/<name>.glb` and its sheets beside it, or `None`
    /// (printed) when the common bake lacks it.
    pub fn load(r: &mut Renderer, assets_dir: &Path, game: &str) -> Option<Self> {
        let name = core::flash::MuzzleFlashLook::default_model(game == "bf2");
        let common = assets_dir.join(format!("common_{game}"));
        let glb = common.join("models").join(format!("{name}.glb"));
        let cpu = match crate::assets::load_glb(&glb) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("muzzle flash {name}: {e:#} (no flash drawn)");
                return None;
            }
        };
        let mut parts = Vec::new();
        for (start, count, material, _, _) in &cpu.submeshes {
            let tex = crate::viewer::upload_texture_file(
                r,
                &common.join("textures"),
                &material.to_ascii_lowercase(),
            )
            .unwrap_or_else(|e| {
                eprintln!("muzzle flash {name}: {e:#}");
                None
            });
            let tris = cpu.indices[*start as usize..(*start + *count) as usize]
                .iter()
                .map(|&i| {
                    let i = i as usize;
                    (
                        Vec3::from(cpu.positions[i]),
                        cpu.uvs.get(i).copied().unwrap_or([0.0; 2]),
                    )
                })
                .collect();
            parts.push((tex, tris));
        }
        Some(Self {
            name: name.to_owned(),
            parts,
        })
    }

    /// This model placed by `m` (`core::flash::matrix`), coloured `color`:
    /// one additive particle per sheet, keyed at `key`.
    pub fn draws(&self, m: Mat4, color: [u8; 4], key: Vec3) -> Vec<ParticleDraw> {
        self.parts
            .iter()
            .map(|(tex, tris)| ParticleDraw {
                shape: ParticleShape::Mesh {
                    tris: tris
                        .iter()
                        .map(|(p, uv)| (m.transform_point3(*p), *uv))
                        .collect(),
                },
                key,
                color,
                texture: *tex,
                additive: true,
            })
            .collect()
    }
}

#[derive(Default)]
pub struct UnitVisual {
    pub gen: u32,
    pub animator: Animator,
    /// Seconds since the unit died (death clip holds its last frame).
    pub dead_for: f32,
    /// The sim tick of the unit's last shot: its muzzle flash's clock.
    pub shot_tick: Option<u64>,
    cast: cast::CastClock,
    pub shot_class: Option<usize>,
    shot_ordinal: u64,
    repair_shot: Option<usize>,
    repair_tick: Option<u64>,
    /// The weapon class last seen in the unit's hands and the set it picks.
    set: Option<(usize, usize)>,
    throwing: Option<usize>,
    /// A melee throw's weapon was firing at the last look: its rising edge
    /// starts the throw clip ([`cast::CastClock`]).
    saber_throwing: bool,
    /// The unit left the field with no death ([`core::sim::Event::Despawn`],
    /// a hero accept or a class change): nothing to draw until it respawns.
    gone: bool,
    pending_throw: Option<u8>,
    throw_windup: f32,
    /// Card VTURG: a seated gunner's world matrix this frame; `None` when he
    /// is not drawn in a seat.
    seat_model: Option<Mat4>,
    seat_clock: Option<SeatClock>,
    cloth_body: Option<usize>,
    cloth_alive: Option<bool>,
    cloth_states: Vec<crate::cloth::State>,
}

/// What a seated gunner keeps from one sim tick to the next: the tick, the
/// look's yaw then, the turn's rate over its speed (what a full-circle
/// nine-pose seat's yaw number is made of, `turret_pose::nine_pose`) and the hang parent's matrix
/// of the update before and of this one (an aimer-node seat hangs from the
/// update before's, `analysis/re-turret-draw.md` 3.2).
#[derive(Clone, Copy, Debug)]
struct SeatClock {
    tick: u64,
    yaw: f32,
    stick: f32,
    prev: Mat4,
    cur: Mat4,
}

impl SeatClock {
    /// The clock of `slot` brought to `tick`: the look's yaw now, the turn
    /// speed `yaw_rate`, the hang parent's matrix `held`.
    fn advance(
        slot: &mut Option<SeatClock>,
        tick: u64,
        yaw: f32,
        yaw_rate: f32,
        held: Mat4,
    ) -> SeatClock {
        let c = slot.get_or_insert(SeatClock {
            tick,
            yaw,
            stick: 0.0,
            prev: held,
            cur: held,
        });
        if c.tick != tick {
            let turned = (yaw - c.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let secs = tick.saturating_sub(c.tick).max(1) as f32 * core::TICK_DT;
            c.stick = (turned / (secs * yaw_rate.max(1e-3))).clamp(-1.0, 1.0);
            (c.tick, c.yaw, c.prev, c.cur) = (tick, yaw, c.cur, held);
        }
        *c
    }
}

/// A seat its gunner is drawn in, as it stands this tick.
struct SeatNow<'a> {
    hang: tp::GunnerHang,
    model: Mat4,
    /// The rig's nodes now.
    now: Vec<Mat4>,
    /// The hang's parent is the seat's aimer node.
    lags: bool,
    seat: &'a crate::vehicle_class::SeatSpec,
    look: (f32, f32),
    yaw_rate: f32,
}

/// The seat of mount `m` when the PS2 draws its soldier there (card VTURG):
/// a building turret ([`crate::vehicle_class::SeatSpec::gunner_draw`]) whose
/// placed building has a rig holding the seat's `PilotPosition`. A
/// vehicle's seat is [`rider_now`]'s.
fn seat_now<'a>(
    turrets: &[tp::RiggedTurret],
    walk: &'a WalkSim,
    m: core::vehicle::Mount,
    bf2: bool,
) -> Option<SeatNow<'a>> {
    let v = walk.sim.vehicle(m.vehicle)?;
    if v.kind != core::vehicle::VehicleKind::Turret {
        return None;
    }
    let s = usize::from(m.seat);
    let seat = walk.vehicle_seats.get(v.class as usize)?.get(s)?;
    if seat.gunner_draw(bf2) != crate::vehicle_class::GunnerDraw::Drawn {
        return None;
    }
    let t = turrets.iter().find(|t| t.pos == v.pos)?;
    let hang = tp::GunnerHang::of(&t.rig, seat)?;
    let setup = walk
        .sim
        .seat_setup
        .get(v.class as usize)
        .and_then(Option::as_ref)
        .and_then(|x| x.guns.get(s))
        .and_then(Option::as_ref);
    // A bot's seat is armed on its first aim: until then the gun rests.
    let (look, _, aim) = setup.map_or(((0.0, 0.0), (0.0, 0.0), (0.0, 0.0)), |g| g.rest());
    let (look, aim) = v
        .guns
        .get(s)
        .and_then(Option::as_ref)
        .map_or((look, aim), |g| (g.look, g.aim));
    let spec = tp::PoseSpec::of(&t.rig, seat);
    Some(SeatNow {
        hang,
        model: t.model,
        now: tp::pose(&t.rig, &spec, look, aim, &[]),
        lags: hang.parent.is_some() && hang.parent == spec.aimer,
        seat,
        look,
        yaw_rate: setup.map_or(2.0, |g| g.yaw_speed()),
    })
}

/// A vehicle seat its rider is drawn in, as it stands this tick (card VRIDE).
struct RiderNow<'a> {
    /// His world matrix.
    model: Mat4,
    seat: &'a crate::vehicle_class::SeatSpec,
    look: (f32, f32),
    yaw_rate: f32,
    /// A hover pilot's numbers.
    hover: Option<tp::HoverNine>,
}

/// The seat of mount `m` on a vehicle (not a building turret) when the PS2
/// draws its soldier there (card VRIDE, `analysis/re-vehicle-riders.md` 1,
/// 2.1): `O x hang`, O the vehicle as `viewer::VehicleVisuals` draws it (a
/// walker with no rig turns with its legs).
fn rider_now<'a>(
    rigs: &[Option<std::sync::Arc<crate::viewer::walker_pose::WalkerRig>>],
    nines: &[Option<HoverClock>],
    walk: &'a WalkSim,
    m: core::vehicle::Mount,
    bf2: bool,
) -> Option<RiderNow<'a>> {
    use core::vehicle::VehicleKind;
    let v = walk.sim.vehicle(m.vehicle)?;
    if v.kind == VehicleKind::Turret {
        return None;
    }
    let s = usize::from(m.seat);
    let class = v.class as usize;
    let seat = walk.vehicle_seats.get(class)?.get(s)?;
    if seat.gunner_draw(bf2) != crate::vehicle_class::GunnerDraw::Drawn {
        return None;
    }
    let hang = walk.vehicle_riders.get(class)?.get(s)?;
    let walker = v.kind == VehicleKind::Walker;
    let rig = rigs
        .get(class)
        .and_then(Option::as_deref)
        .filter(|_| walker);
    let rot = if walker && rig.is_none() {
        core::vehicle::walker::legs_rot(v)
    } else {
        v.rot
    };
    let local = match hang {
        crate::vehicle_class::RiderHang::Bind(b) => rig
            .and_then(|r| {
                let scale = walk
                    .sim
                    .walker_params
                    .get(class)
                    .and_then(Option::as_ref)
                    .map_or(1.0, |p| p.root_scale);
                r.node_now(v, scale, seat.pilot_position.as_deref()?)
            })
            .unwrap_or(*b),
        crate::vehicle_class::RiderHang::Node(name, b) => rig
            .and_then(|r| {
                let scale = walk
                    .sim
                    .walker_params
                    .get(class)
                    .and_then(Option::as_ref)
                    .map_or(1.0, |p| p.root_scale);
                r.node_now(v, scale, name)
            })
            .unwrap_or(*b),
    };
    let setup = walk
        .sim
        .seat_setup
        .get(class)
        .and_then(Option::as_ref)
        .and_then(|x| x.guns.get(s))
        .and_then(Option::as_ref);
    let look = v
        .guns
        .get(s)
        .and_then(Option::as_ref)
        .map(|g| g.look)
        .or_else(|| setup.map(|g| g.rest().0))
        .unwrap_or((0.0, 0.0));
    let pilot = seat.kind == crate::vehicle_class::SeatKind::Pilot;
    Some(RiderNow {
        model: Mat4::from_rotation_translation(rot, v.pos) * local,
        seat,
        look,
        yaw_rate: setup.map_or(2.0, |g| g.yaw_speed()),
        hover: nines
            .get(m.vehicle.index as usize)
            .copied()
            .flatten()
            .filter(|c| pilot && v.kind == VehicleKind::Hover && c.gen == v.gen)
            .map(|c| c.nine),
    })
}

/// Per-class soldier assets. `assets` is the BODY cache, one upload per
/// soldier geometry, and `weapons` the hand-weapon cache, one per weapon
/// geometry — so two classes (or two teams) that share a model share one
/// upload, and two classes that share a body still hold their own guns.
/// `per_class[team][class]` is the pair of indices `UnitSpec::class` resolves
/// to.
///
/// A class whose GLB is not baked falls back to its own team's FIRST class's
/// body (the fill trooper) rather than to the marker box — so a mixed roster
/// looks like a team, not like a team with holes — and keeps its own weapon.
/// The fallback is printed once per missing model; baking the rest
/// (`scripts/rebake.sh`) makes it stop.
#[derive(Default)]
pub struct SoldierVisuals {
    pub assets: Vec<SoldierAsset>,
    pub weapons: Vec<WeaponAsset>,
    sabers: [Vec<Option<saber::SaberVisual>>; 2],
    saber_trails: Vec<saber_trail::State>,
    grenades: Vec<Option<usize>>,
    /// `per_class[team][class]` -> indices into `assets` and `weapons`; a
    /// `None` body = marker box.
    pub per_class: [Vec<ClassDraw>; 2],
    /// `held[team][class]`: the other slots' models ([`DrawPlan::held`]).
    pub held: [Vec<HeldDraws>; 2],
    /// `anim[team][class]`: what picks each class's animation sets.
    pub anim: [Vec<ClassAnim>; 2],
    /// `skins[team][class][part]`: the class's override texture for each
    /// part of its body ([`SoldierAsset::groups`], card USKN); empty = the
    /// model's own textures.
    pub skins: [Vec<Vec<Option<TextureHandle>>>; 2],
    pub units: Vec<UnitVisual>,
    /// The map's game's muzzle flash model (card FXMF); `None` = none baked.
    pub flash: Option<FlashModel>,
    /// The map's rigged armed buildings: where a seated gunner hangs (card
    /// VTURG; [`SoldierVisuals::set_turrets`]). Empty = no gunner is drawn.
    pub turrets: Vec<tp::RiggedTurret>,
    /// The map is a BF2 disc's (BF2 also hides a gunner on
    /// `IsPilotExposed = 0`).
    pub bf2: bool,
    /// Each vehicle class's walker rig, by `Vehicle::class` (card VRIDE;
    /// [`SoldierVisuals::set_walker_rigs`]): an `AnimatedPilotPosition`
    /// rider hangs from its node now. Empty = every walker drawn rigid.
    pub walker_rigs: Vec<Option<std::sync::Arc<crate::viewer::walker_pose::WalkerRig>>>,
    /// Per `Sim::vehicles` slot: a hover's rider numbers (card VRIDE).
    hover_nine: Vec<Option<HoverClock>>,
    /// Reused across generations; respawning resets CPU state, not GPU buffers.
    cloth_meshes: std::collections::HashMap<(usize, usize, usize), render::MeshHandle>,
    pub fp: first_person::FirstPersonVisuals,
}

/// A hover's [`tp::HoverNine`] and the vehicle and tick it was stepped for.
#[derive(Clone, Copy, Debug)]
struct HoverClock {
    gen: u32,
    tick: u64,
    nine: tp::HoverNine,
}

/// Seconds a corpse stays drawn, counted from the death (`dead_for`), unless
/// the slot respawns first: the sim's own [`core::soldier::CORPSE_SECS`], so
/// the body is not drawn after it left the world.
const CORPSE_SECS: f32 = core::soldier::CORPSE_SECS;

impl WeaponAsset {
    /// The weapon's parts at `m` (the hand's `hp_weapons` in the world),
    /// each textured one with its specular second pass.
    fn part_draws(&self, m: Mat4, out: &mut Vec<render::DrawCmd>) {
        for (mesh, tex, layer, spec) in &self.parts {
            // The weapon is a non-static draw with no shadow volume: the
            // soldier's own character ambient on BF1 (re-model-lighting §12).
            let light = render::LightPath::Character;
            out.push(
                match tex {
                    Some(t) if *layer == render::DrawLayer::Mask => {
                        render::DrawCmd::textured_masked(*mesh, *t, m)
                    }
                    Some(t) => render::DrawCmd::textured(*mesh, *t, m),
                    None => render::DrawCmd::flat(*mesh, [0.3, 0.3, 0.32, 1.0], m),
                }
                .with_light(light),
            );
            // The second pass rides only a textured draw: its add is
            // weighted by the base texel's alpha (as the world's).
            if let (Some(t), Some((rgb, power))) = (tex, spec) {
                out.push(render::DrawCmd::specular(*mesh, *t, *rgb, *power, m).with_light(light));
            }
        }
    }
}

/// The world point of the weapon node `name` under the hand matrix `hand`
/// (`hp_weapons` in the world); the hand's origin when the weapon has no
/// such node — what the PS2's class+0x30 is then (§4.1).
pub fn fire_point(hand: Mat4, nodes: &manifest::WeaponNodes, name: &str) -> Vec3 {
    let local = nodes
        .iter()
        .find(|n| n.name.eq_ignore_ascii_case(name))
        .map_or(Vec3::ZERO, |n| Vec3::from(n.pos));
    hand.transform_point3(local)
}

/// A soldier's model matrix: at his feet, turned to his yaw (the model faces
/// +Z, sim yaw 0 faces -Z; `animator::travel` is measured in this frame).
fn model_of(s: &core::soldier::Soldier) -> Mat4 {
    Mat4::from_translation(s.pos) * Mat4::from_rotation_y(s.yaw + std::f32::consts::PI)
}

mod cast;
mod cloth;
pub mod first_person;
mod grenade;
use grenade::drawn;
mod runtime;
mod saber;
mod saber_trail;

#[cfg(test)]
mod tests;
