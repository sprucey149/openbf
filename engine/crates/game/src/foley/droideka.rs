//! Droideka feet use the three leg bones, not humanoid surface rows.
//! BF2 FUN_001f3b90 starts each authored sound when its bone drops through
//! 0.48 m. Cache model-space heights at load; ticks interpolate three floats.
use super::{Sound, View, Who, REACH};
use crate::{assets::SkinnedAsset, classdb::ClassDb, walk::WalkSim};
use core::droideka::{Form, STATES};
use std::{collections::HashMap, path::Path, sync::Arc};

const BONES: [&str; 3] = ["bone_l_leg2", "bone_r_leg2", "bone_b_leg2"];
const SOUND_KEYS: [u32; 3] = [0x21f5_6ed0, 0x937c_eaa5, 0x12b2_3362];
const STEP_HEIGHT: f32 = 0.48;

#[derive(Clone, Debug)]
struct Curve {
    fps: f32,
    heights: Vec<[f32; 3]>,
}
impl Curve {
    fn at(&self, time: f32) -> [f32; 3] {
        let f = (time * self.fps)
            .max(0.0)
            .min((self.heights.len() - 1) as f32);
        let a = f as usize;
        let b = (a + 1).min(self.heights.len() - 1);
        std::array::from_fn(|i| {
            self.heights[a][i] + (self.heights[b][i] - self.heights[a][i]) * f.fract()
        })
    }
}
#[derive(Clone, Debug)]
struct Steps {
    sounds: [u32; 3],
    curves: [Option<Curve>; STATES],
}
impl Steps {
    fn new(
        asset: &SkinnedAsset,
        record: &manifest::ManifestDroideka,
        db: &ClassDb,
    ) -> Option<Self> {
        let skel = &asset.skeleton;
        let bones = BONES.map(|name| skel.index_of(name));
        if bones.iter().any(Option::is_none) {
            return None;
        }
        let bones = bones.map(Option::unwrap);
        let sounds = SOUND_KEYS.map(|key| {
            db.prop_hash(&record.class, key)
                .map(|s| crate::classdb::fnv1a_lowercase(s.trim().trim_matches('"')))
                .unwrap_or(0)
        });
        let curves = std::array::from_fn(|state| {
            let name = &record.clips.get(state)?.as_ref()?.name;
            let clip = asset
                .clips
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(name) && c.frames > 0)?;
            let heights = (0..clip.frames)
                .map(|frame| {
                    let palette = skel.palette(&clip.sample_frame(skel, frame).locals());
                    bones.map(|bone| (palette[bone] * skel.global_bind[bone]).w_axis.y)
                })
                .collect();
            Some(Curve {
                fps: clip.fps,
                heights,
            })
        });
        Some(Self { sounds, curves })
    }
}
#[derive(Clone, Debug, Default)]
struct Track {
    generation: u32,
    heights: Option<[f32; 3]>,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Feet {
    classes: [Vec<Option<Arc<Steps>>>; 2],
    tracks: Vec<Track>,
}
impl Feet {
    pub fn load(map_dir: &Path, walk: &WalkSim) -> Self {
        let Some(root) = map_dir.parent() else {
            return Self::default();
        };
        let game = pak::read_to_string(map_dir.join("manifest.ron"))
            .ok()
            .and_then(|text| ron::from_str::<manifest::Manifest>(&text).ok())
            .map(|m| m.source_game.to_ascii_lowercase());
        let Some(game) = game else {
            return Self::default();
        };
        let factions = [
            walk.teams[0].faction.as_str(),
            walk.teams[1].faction.as_str(),
        ];
        let Ok(db) = crate::roster::load_classdb(&root.join("classes"), &game, &factions) else {
            return Self::default();
        };
        let mut by_name = HashMap::new();
        for (dir, side) in crate::vehicle_pads::side_vehicle_files(map_dir, &game) {
            for record in side.droidekas {
                if !walk.teams.iter().any(|t| {
                    t.classes
                        .iter()
                        .any(|c| c.name.eq_ignore_ascii_case(&record.class))
                }) {
                    continue;
                }
                let Some(skin) = &record.skin else {
                    continue;
                };
                let path = dir.join("walkers").join(format!("{skin}.glb"));
                match crate::assets::load_skinned_glb(&path) {
                    Ok(asset) => {
                        if let Some(steps) = Steps::new(&asset, &record, &db) {
                            by_name.insert(record.class.to_ascii_lowercase(), Arc::new(steps));
                        }
                    }
                    Err(e) => eprintln!("droideka footsteps {}: {e}", path.display()),
                }
            }
        }
        Self {
            classes: std::array::from_fn(|team| {
                walk.teams[team]
                    .classes
                    .iter()
                    .map(|c| by_name.get(&c.name.to_ascii_lowercase()).cloned())
                    .collect()
            }),
            tracks: Vec::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.classes.iter().flatten().all(Option::is_none)
    }
    pub fn tick(&mut self, walk: &WalkSim, view: &View, who: Who, play: &mut impl FnMut(Sound)) {
        self.tracks.resize(walk.sim.units.len(), Track::default());
        for (i, u) in walk.sim.units.iter().enumerate() {
            let track = &mut self.tracks[i];
            let class = self.classes[usize::from(u.team.min(1))]
                .get(usize::from(u.spec.class))
                .and_then(Option::as_deref);
            let active = u.alive
                && u.mount.is_none()
                && (u.soldier.pos - view.eye).length_squared() <= REACH * REACH
                && (who == Who::InReach || u.is_player || view.sees(u.soldier.pos));
            let Some((deka, steps)) = u
                .droideka
                .as_ref()
                .zip(class)
                .filter(|(d, _)| active && d.form() == Form::Walk)
            else {
                track.heights = None;
                continue;
            };
            let Some(curve) = steps
                .curves
                .get(usize::from(deka.state))
                .and_then(Option::as_ref)
            else {
                track.heights = None;
                continue;
            };
            let heights = curve.at(deka.clip_t);
            if track.generation == u.gen {
                if let Some(previous) = track.heights {
                    for foot in 0..3 {
                        if crosses(previous[foot], heights[foot]) && steps.sounds[foot] != 0 {
                            play(Sound {
                                unit: i as u32,
                                hash: steps.sounds[foot],
                                pos: u.soldier.pos,
                            });
                        }
                    }
                }
            }
            track.generation = u.gen;
            track.heights = Some(heights);
        }
    }
}
fn crosses(previous: f32, current: f32) -> bool {
    previous > STEP_HEIGHT && current < STEP_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foot_contact_is_a_downward_strict_crossing() {
        assert!(crosses(0.5, 0.4));
        for (a, b) in [(0.4, 0.5), (0.5, 0.5), (0.4, 0.4), (0.48, 0.4), (0.5, 0.48)] {
            assert!(!crosses(a, b));
        }
        let c = Curve {
            fps: 30.0,
            heights: vec![[0.6; 3], [0.2; 3]],
        };
        assert!(crosses(c.at(0.0)[0], c.at(1.0 / 60.0)[0]));
        assert_eq!(c.at(100.0), [0.2; 3]);
    }
    #[test]
    #[ignore = "requires native droideka side bakes"]
    fn both_native_droidekas_have_three_sounding_feet() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets_baked");
        for game in ["bf1", "bf2"] {
            let db = crate::roster::load_classdb(&root.join("classes"), game, &["cis"]).unwrap();
            let dir = root.join(format!("side_{game}/vehicles/cis"));
            let side: manifest::VehicleManifest =
                ron::from_str(&pak::read_to_string(dir.join("vehicles.ron")).unwrap()).unwrap();
            let record = side
                .droidekas
                .iter()
                .find(|d| d.class == "cis_inf_droideka")
                .unwrap();
            let asset = crate::assets::load_skinned_glb(
                &dir.join("walkers")
                    .join(format!("{}.glb", record.skin.as_ref().unwrap())),
            )
            .unwrap();
            let steps = Steps::new(&asset, record, &db).unwrap();
            assert!(steps.sounds.iter().all(|s| *s != 0));
            let mut contacts = [0; 3];
            for curve in steps.curves.iter().flatten() {
                for pair in curve.heights.windows(2) {
                    for (foot, count) in contacts.iter_mut().enumerate() {
                        *count += usize::from(crosses(pair[0][foot], pair[1][foot]));
                    }
                }
            }
            assert!(contacts.iter().all(|n| *n > 0), "{game}: {contacts:?}");
            eprintln!(
                "{game}: native contacts {contacts:?}, sounds {:?}",
                steps.sounds
            );
        }
    }
}
