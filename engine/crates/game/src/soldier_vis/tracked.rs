//! Controller-driven infantry hands, forearms and weapon presentation.

use super::{fire_point, SoldierAsset, SoldierVisuals, WeaponAsset};
use crate::assets::CpuSkinnedMesh;
use crate::walk::WalkSim;
use glam::{Mat4, Vec3};
use render::{DrawCmd, LightPath, MeshData, MeshHandle, Renderer};

#[derive(Default)]
pub(super) struct Arms {
    sides: [Option<Arm>; 2],
    front_hand_bones: bool,
    super_battle_droid: bool,
}

struct Arm {
    body_to_mount: Mat4,
    parts: Vec<(usize, MeshHandle)>,
}

pub use extensions::runtime::TrackedGun;

#[derive(Clone, Copy)]
enum TrackedWeapon<'a> {
    Held(&'a WeaponAsset, &'a str),
    Wrist,
}

impl TrackedWeapon<'_> {
    fn muzzle(self, mount: Mat4) -> Vec3 {
        match self {
            Self::Held(weapon, node) => fire_point(mount, &weapon.nodes, node),
            // The droid's weapon matrix starts at the body's hp_weapons.
            // The integrated weapons have no geometry node offset (re-muzzle-flash §4.1).
            Self::Wrist => mount.transform_point3(Vec3::ZERO),
        }
    }
}

impl SoldierVisuals {
    /// Upload only the new bodies' arm meshes. Called by the VR window when
    /// loading a battle or appending a class.
    pub fn load_tracked_arms(&mut self, renderer: &mut Renderer) {
        self.tracked_arms.extend(
            self.assets[self.tracked_arms.len()..]
                .iter()
                .map(|body| Arms::load(renderer, body)),
        );
    }

    pub fn tracked_draws(
        &self,
        walk: &WalkSim,
        mounts: [Option<Mat4>; 2],
        draws: &mut Vec<DrawCmd>,
    ) {
        let Some(unit) = walk.sim.unit(walk.player) else {
            return;
        };
        if let Some((body, arms)) = self
            .asset_index(unit.team, unit.spec.class)
            .and_then(|i| self.assets.get(i).zip(self.tracked_arms.get(i)))
        {
            let skin = self.body_skin(unit.team, unit.spec.class, false);
            let saber = arms
                .front_hand_bones
                .then(|| self.saber_visual(unit.team, unit.spec.class))
                .flatten();
            for (side, (arm, mount)) in arms.sides.iter().zip(mounts).enumerate() {
                let Some((arm, mount)) = arm.as_ref().zip(mount) else {
                    continue;
                };
                let grip = saber
                    .and_then(|saber| {
                        saber
                            .bindings
                            .iter()
                            .find(|b| b.blade.tracked_hand() == Some(side))
                    })
                    .map_or(Mat4::IDENTITY, |b| b.tracked_mount);
                let model = mount * grip * arm.body_to_mount;
                let lit = Some(crate::point_light::soldier_sphere(mount.w_axis.truncate()));
                for &(part, mesh) in &arm.parts {
                    let (_, texture, layer, spec) = body.meshes[part];
                    let texture = skin.get(part).copied().flatten().or(texture);
                    let base = DrawCmd {
                        texture,
                        layer,
                        ..DrawCmd::flat(mesh, [1.0; 4], model)
                    };
                    draws.push(base.with_light(LightPath::Character).with_lit(lit));
                    if let (Some(tex), Some((rgb, power))) = (texture, spec) {
                        draws.push(
                            DrawCmd::specular(mesh, tex, rgb, power, model)
                                .with_light(LightPath::Character)
                                .with_lit(lit),
                        );
                    }
                }
            }
        }
        if self.tracked_saber_hilts(walk, mounts, draws) {
            return;
        }
        if let Some((mount, TrackedWeapon::Held(weapon, _))) =
            mounts[1].zip(self.tracked_weapon(walk))
        {
            weapon.part_draws(mount, draws);
        }
    }

    pub fn tracked_grenade_draws(
        &self,
        walk: &WalkSim,
        class: usize,
        at: Mat4,
        draws: &mut Vec<DrawCmd>,
    ) {
        let Some(unit) = walk.sim.unit(walk.player) else {
            return;
        };
        let Some((selected, slot, true)) = walk.sim.tracked_grenade_slot(walk.player) else {
            return;
        };
        if selected == class {
            if let Some(asset) = self
                .ordnance_model(class)
                .filter(|asset| !asset.parts.is_empty())
                .or_else(|| self.weapon_for(unit.team, unit.spec.class, slot))
            {
                asset.part_draws(at, draws);
            }
        }
    }

    /// Visual origin shared by bolts and muzzle flashes. The simulation
    /// keeps its own shot origin and replayable aim.
    pub fn tracked_fire_point(&self, walk: &WalkSim, right_mount: Mat4) -> Option<Vec3> {
        Some(self.tracked_weapon(walk)?.muzzle(right_mount))
    }

    /// The muzzle and available offhand grip on each selected weapon channel.
    pub fn tracked_guns(&self, walk: &WalkSim) -> [Option<TrackedGun>; 2] {
        let Some(unit) = walk.sim.unit(walk.player) else {
            return [None; 2];
        };
        let channels = [
            Some((unit.weapon_class, unit.weapon_slot)),
            unit.cur1
                .and_then(|i| unit.held.get(usize::from(i)))
                .map(|held| (held.class, held.slot)),
        ];
        let row = self
            .native
            .get(&unit.team)
            .map_or(&self.anim[usize::from(unit.team.min(1))], |side| &side.anim);
        let anim = row.get(usize::from(unit.spec.class));
        channels.map(|channel| {
            let (class, slot) = channel?;
            let weapon_class = walk.sim.weapon_classes.get(class)?;
            let weapon = self.tracked_weapon_at(unit, &walk.sim.weapon_classes, class, slot);
            let muzzle = weapon.map_or(Vec3::ZERO, |w| w.muzzle(Mat4::IDENTITY));
            let bank = anim.and_then(|a| {
                a.banks
                    .iter()
                    .find_map(|(w, bank)| (w == &weapon_class.name).then_some(bank.as_str()))
            });
            let two_handed = match bank {
                Some(bank) => matches!(bank, "rifle" | "bazooka" | "lascannon" | "bdroid"),
                // Award weapons can inherit the slot without a named bank in
                // the soldier's animation table.
                None => {
                    weapon_class.rocket_launcher()
                        || ["rifle", "shotgun", "chaingun"]
                            .iter()
                            .any(|&suffix| weapon_class.name.ends_with(suffix))
                }
            };
            let foregrip = match (two_handed, weapon) {
                (true, Some(TrackedWeapon::Held(asset, _))) => asset.bounds.and_then(foregrip),
                _ => None,
            };
            Some(TrackedGun {
                class,
                slot,
                muzzle,
                foregrip,
            })
        })
    }

    fn tracked_weapon<'a>(&'a self, walk: &'a WalkSim) -> Option<TrackedWeapon<'a>> {
        let unit = walk.sim.unit(walk.player)?;
        let hand = self.hand_of(unit, &walk.sim.weapon_classes);
        if hand.hidden {
            return None;
        }
        self.tracked_weapon_at(unit, &walk.sim.weapon_classes, hand.class, hand.slot)
    }

    fn tracked_weapon_at<'a>(
        &'a self,
        unit: &core::unit::Unit,
        classes: &'a [core::weapon::WeaponClass],
        class: usize,
        slot: u8,
    ) -> Option<TrackedWeapon<'a>> {
        let class = classes.get(class)?;
        if let Some(weapon) = self.weapon_for(unit.team, unit.spec.class, slot) {
            let node = class
                .flash
                .as_ref()
                .map_or(core::flash::DEFAULT_FIRE_POINT, |f| f.fire_point.as_str());
            return Some(TrackedWeapon::Held(weapon, node));
        }
        let arms = self
            .tracked_arms
            .get(self.asset_index(unit.team, unit.spec.class)?)?;
        (arms.super_battle_droid
            && arms.sides[1].is_some()
            && (class.name.starts_with("cis_weap_inf_wrist_")
                || class.name == "cis_weap_award_wrist_blaster"))
            .then_some(TrackedWeapon::Wrist)
    }
}

/// OURS: place the support palm below the receiver on long-gun models.
/// measure: calibrate the grip against the fore-end of each model in a headset.
fn foregrip((min, max): (Vec3, Vec3)) -> Option<Vec3> {
    (max.z >= 0.35).then(|| {
        Vec3::new(
            (min.x + max.x) * 0.5,
            min.y + (max.y - min.y) * 0.3,
            max.z * 0.45,
        )
    })
}

impl Arms {
    fn load(renderer: &mut Renderer, body: &SoldierAsset) -> Self {
        let skel = &body.asset.skeleton;
        let standard = arm_mounts(body);
        // Two controllers drive the front pair on Grievous's four-arm rig.
        let Some((forearms, mounts)) = standard.or_else(|| {
            let forearms = [
                skel.index_of("bone_lf_forearm")?,
                skel.index_of("bone_rf_forearm")?,
            ];
            let hands = [
                skel.index_of("bone_lf_hand")?,
                skel.index_of("bone_rf_hand")?,
            ];
            Some((forearms, hands.map(|i| skel.global_bind[i])))
        }) else {
            return Self::default();
        };
        let mesh = &body.asset.mesh;
        let sides = std::array::from_fn(|side| {
            let mut under_forearm = vec![false; skel.len()];
            for (i, under) in under_forearm.iter_mut().enumerate() {
                let mut joint = Some(i);
                while let Some(bone) = joint {
                    if bone == forearms[side] {
                        *under = true;
                        break;
                    }
                    joint = skel.parents[bone];
                }
            }
            let on_arm: Vec<_> = mesh
                .joints
                .iter()
                .zip(&mesh.weights)
                .map(|(joints, weights)| {
                    joints
                        .iter()
                        .zip(weights)
                        .filter_map(|(&bone, &weight)| {
                            under_forearm[usize::from(bone)].then_some(weight)
                        })
                        .sum::<f32>()
                        > 0.5
                })
                .collect();
            let parts: Vec<_> = body
                .meshes
                .iter()
                .enumerate()
                .filter_map(|(part, _)| {
                    let (start, count, _, _) = mesh.submeshes[part];
                    let indices = &mesh.indices[start as usize..(start + count) as usize];
                    upload_arm_part(renderer, mesh, indices, &on_arm).map(|handle| (part, handle))
                })
                .collect();
            (!parts.is_empty()).then(|| Arm {
                body_to_mount: mounts[side].inverse(),
                parts,
            })
        });
        Self {
            sides,
            front_hand_bones: standard.is_none(),
            super_battle_droid: body.asset.name.eq_ignore_ascii_case("cis_inf_sbdroid"),
        }
    }
}

/// The right hand uses the authored weapon mount. Mirror its offset through
/// the hand bones for the left, whose local Z axis has opposite handedness
/// across the body. Reflect the mount's X as well to keep a proper rotation.
fn arm_mounts(body: &SoldierAsset) -> Option<([usize; 2], [Mat4; 2])> {
    let skel = &body.asset.skeleton;
    let forearms = [
        skel.index_of("bone_l_forearm")?,
        skel.index_of("bone_r_forearm")?,
    ];
    let left = skel.global_bind[skel.index_of("bone_l_hand")?];
    let right = skel.inverse_bind[skel.index_of("bone_r_hand")?];
    let weapon = skel.global_bind[body.hp_weapons?];
    let mirror_z = Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0));
    let mirror_x = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    Some((
        forearms,
        [left * mirror_z * right * weapon * mirror_x, weapon],
    ))
}

/// Keep the triangles whose majority is on the hand or forearm. Remap
/// their vertices so each uploaded part contains only the visible limb.
fn upload_arm_part(
    renderer: &mut Renderer,
    mesh: &CpuSkinnedMesh,
    indices: &[u32],
    on_arm: &[bool],
) -> Option<MeshHandle> {
    let indices: Vec<_> = indices
        .chunks_exact(3)
        .filter(|tri| {
            tri.iter().all(|&i| (i as usize) < on_arm.len())
                && tri.iter().filter(|&&i| on_arm[i as usize]).count() >= 2
        })
        .flatten()
        .copied()
        .collect();
    if indices.is_empty() {
        return None;
    }
    let limb = crate::viewer::compact_submesh(MeshData {
        positions: &mesh.positions,
        normals: &mesh.normals,
        uvs: &mesh.uvs,
        colors: Some(&mesh.colors),
        indices: &indices,
    });
    match renderer.add_mesh(&limb.mesh_data()) {
        Ok(mesh) => Some(mesh),
        Err(err) => {
            eprintln!("VR hand mesh upload: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soldier_vis::ClassDraw;
    use glam::Quat;
    use render::Camera;
    use std::collections::HashMap;

    #[test]
    #[ignore = "requires a GPU and local soldier bakes"]
    fn tracked_hands_draw_a_held_rifle_and_both_discs_wrist_weapons() {
        let root = crate::assets::baked_root();
        let cases = [
            ("bf2", "rep_inf_ep3trooper", Some("rep_weap_inf_rifle")),
            ("bf1", "cis_inf_sbdroid", None),
            ("bf2", "cis_inf_sbdroid", None),
        ];
        if cases.iter().any(|(game, body, _)| {
            !root
                .join(format!("side_{game}/soldiers/{body}.glb"))
                .is_file()
        }) {
            eprintln!("SKIP: tracked hand soldier bakes absent");
            return;
        }
        let mut renderer = Renderer::new_headless(256, 192).unwrap();
        let camera = Camera {
            near: 0.05,
            ..Default::default()
        };
        let clear = [0.0, 0.0, 0.0, 1.0];
        renderer
            .render_frame(&camera, &[], &[], clear, None)
            .unwrap();
        let (background, width, _) = renderer.capture_rgba().unwrap();
        let mounts = [-0.35, 0.35].map(|x| {
            Some(Mat4::from_rotation_translation(
                Quat::from_rotation_y(std::f32::consts::PI),
                Vec3::new(x, -0.08, -0.77),
            ))
        });
        let right_mount = mounts[1].unwrap();
        for (game, geometry, rifle) in cases {
            let mut cache = HashMap::new();
            let body =
                SoldierVisuals::load_body(Some(&mut renderer), &root, game, geometry, &mut cache)
                    .unwrap();
            let mut visuals = SoldierVisuals {
                assets: vec![body],
                ..Default::default()
            };
            let mut walk = WalkSim::new(7, Vec3::ZERO, None);
            let unit = walk.sim.unit(walk.player).unwrap();
            let (team, class, weapon_class) = (unit.team, unit.spec.class, unit.weapon_class);
            let row = &mut visuals.per_class[usize::from(team)];
            row.resize(
                usize::from(class) + 1,
                ClassDraw {
                    body: Some(0),
                    weapon: rifle.map(|_| 0),
                },
            );
            if let Some(rifle) = rifle {
                visuals.weapons.push(SoldierVisuals::load_weapon(
                    Some(&mut renderer),
                    &root,
                    game,
                    rifle,
                    &mut cache,
                ));
                assert!(!visuals.weapons[0].parts.is_empty());
            } else {
                walk.sim.weapon_classes[weapon_class].name = "cis_weap_inf_wrist_blaster".into();
            }
            visuals.load_tracked_arms(&mut renderer);
            if rifle.is_some() {
                let row = &mut visuals.anim[usize::from(team)];
                row.resize_with(usize::from(class) + 1, Default::default);
                let anim = &mut row[usize::from(class)];
                anim.banks.push((
                    walk.sim.weapon_classes[weapon_class].name.clone(),
                    "rifle".into(),
                ));
                let gun = visuals.tracked_guns(&walk)[0].unwrap();
                assert!(gun.foregrip.is_some());
                visuals.anim[usize::from(team)][usize::from(class)].banks[0].1 = "pistol".into();
                assert!(visuals.tracked_guns(&walk)[0].unwrap().foregrip.is_none());
            } else {
                let gun = visuals.tracked_guns(&walk)[0].unwrap();
                assert_eq!(gun.muzzle, Vec3::ZERO);
                assert!(gun.foregrip.is_none());
            }
            let body = &visuals.assets[0];
            let wrist = body.asset.skeleton.global_bind[body.hp_weapons.unwrap()]
                .w_axis
                .truncate();
            let right_arm = visuals.tracked_arms[0].sides[1].as_ref().unwrap();
            assert!((right_mount * right_arm.body_to_mount)
                .transform_point3(wrist)
                .abs_diff_eq(right_mount.w_axis.truncate(), 1e-5));

            let mut draws = Vec::new();
            visuals.tracked_draws(&walk, mounts, &mut draws);
            renderer
                .render_frame(&camera, &draws, &[], clear, None)
                .unwrap();
            let (rgba, _, _) = renderer.capture_rgba().unwrap();
            let mut halves = [0, 0];
            for (i, (pixel, clear)) in rgba
                .chunks_exact(4)
                .zip(background.chunks_exact(4))
                .enumerate()
            {
                if pixel != clear {
                    halves[usize::from(i % width as usize >= width as usize / 2)] += 1;
                }
            }
            assert!(
                halves.into_iter().all(|n| n > 100),
                "{game} {geometry}: {halves:?}"
            );
            let point = visuals.tracked_fire_point(&walk, right_mount).unwrap();
            if rifle.is_some() {
                assert!((point - right_mount.w_axis.truncate()).dot(Vec3::NEG_Z) > 0.0);
                visuals.per_class[usize::from(team)][usize::from(class)].weapon = None;
                let mut empty_hands = Vec::new();
                visuals.tracked_draws(&walk, mounts, &mut empty_hands);
                assert!(!empty_hands.is_empty());
                assert!(empty_hands.len() < draws.len());
                assert!(visuals.tracked_fire_point(&walk, right_mount).is_none());
            } else {
                for name in [
                    "cis_weap_inf_wrist_blaster",
                    "cis_weap_inf_wrist_rocket",
                    "cis_weap_inf_wrist_shrapnel",
                    "cis_weap_inf_wrist_trishot",
                    "cis_weap_award_wrist_blaster",
                ] {
                    walk.sim.weapon_classes[weapon_class].name = name.into();
                    assert_eq!(
                        visuals.tracked_fire_point(&walk, right_mount),
                        Some(right_mount.w_axis.truncate()),
                    );
                }
            }
        }
    }
}
