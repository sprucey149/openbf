//! A vehicle's or armed building's `skel` nodes in model space (card VTUR,
//! `analysis/re-vehicles.md` §5, §8): the seats name them — `FirePointName`
//! (where a seat weapon's bolt starts), `TurretNodeName` (the aimer's
//! pivot), `PilotPosition` (where the rider sits). The model is drawn rigid
//! in its bind pose (`crate::vehicle`), so each node's bind-pose origin is
//! where the drawn gun is. A vehicle's nodes keep their bind AXES too: its
//! rider hangs by the whole bind matrix (card VRIDE).
//!
//! The skeleton is found the way `crate::door::rigs` finds a door's: named
//! as the class's `GeometryName`, else as its `gmod`'s best LOD.

use manifest::{ManifestClass, ManifestHardpoint};
use mungefs::entity::GeomModel;
use mungefs::skel::Skeleton;

/// The BASEs whose classes carry seats: every vehicle label, and the
/// armed buildings (§8; BF2's `portableturret` / `defensegridturret` are
/// `armedbuildingdynamic`).
pub fn wants(base: &str) -> bool {
    crate::vehicle::is_vehicle_base(base)
        || ["armedbuilding", "armedbuildingdynamic"]
            .iter()
            .any(|b| b.eq_ignore_ascii_case(base))
}

/// Every node's bind-pose origin (row 3 of its model-space bind matrix, the
/// matrix `mungefs::model::to_model_space` moves a bone-space vertex by) and
/// axes (rows 0-2).
pub fn of(skel: &Skeleton) -> Vec<ManifestHardpoint> {
    let Ok(m) = skel.bind_matrices() else {
        return Vec::new();
    };
    skel.bones
        .iter()
        .zip(m)
        .map(|(b, m)| ManifestHardpoint {
            name: b.name.clone(),
            pos: [m[3][0], m[3][1], m[3][2]],
            axes: Some([0, 1, 2].map(|r| [m[r][0], m[r][1], m[r][2]])),
        })
        .collect()
}

/// The LVL's skeletons, found for a class the way `crate::door::rigs`
/// finds a door's: named as the class's `GeometryName`, else as its `gmod`'s
/// best LOD (first of a name wins, the walk's own rule).
pub struct Skeletons {
    skels: Vec<Skeleton>,
    gmods: Vec<GeomModel>,
}

impl Skeletons {
    pub fn of(chunks: &[mungefs::Chunk<'_>]) -> Skeletons {
        let mut skels: Vec<Skeleton> = Vec::new();
        let mut gmods: Vec<GeomModel> = Vec::new();
        for c in chunks {
            match &c.tag {
                b"skel" => {
                    if let Ok(s) = mungefs::skel::parse_skel(c) {
                        if !skels.iter().any(|k| k.name.eq_ignore_ascii_case(&s.name)) {
                            skels.push(s);
                        }
                    }
                }
                b"gmod" => {
                    if let Ok(g) = GeomModel::parse(c) {
                        if !gmods.iter().any(|k| k.name.eq_ignore_ascii_case(&g.name)) {
                            gmods.push(g);
                        }
                    }
                }
                _ => {}
            }
        }
        Skeletons { skels, gmods }
    }

    #[cfg(test)]
    pub(crate) fn from_skeleton(skel: Skeleton) -> Self {
        Self {
            skels: vec![skel],
            gmods: Vec::new(),
        }
    }

    fn named(&self, name: &str) -> Option<&Skeleton> {
        self.skels
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub(crate) fn geometries(&self) -> &[GeomModel] {
        &self.gmods
    }

    pub(crate) fn geometry(&self, name: &str) -> Option<&GeomModel> {
        self.gmods
            .iter()
            .find(|g| g.name.eq_ignore_ascii_case(name))
    }

    /// The skeleton of a class whose `GeometryName` is `geom`.
    pub fn find(&self, geom: &str) -> Option<&Skeleton> {
        self.named(geom).or_else(|| {
            self.geometry(geom)
                .and_then(|g| g.best_model())
                .and_then(|n| self.named(n))
        })
    }

    /// A named skeleton, else the first geometry containing this LOD's
    /// skeleton (geometry name, then best LOD). `geometry_matches` also
    /// accepts the geometry's own name, as required by turret rigs.
    pub(crate) fn find_model(&self, model: &str, geometry_matches: bool) -> Option<&Skeleton> {
        self.named(model).or_else(|| {
            self.gmods
                .iter()
                .find(|g| {
                    (geometry_matches && g.name.eq_ignore_ascii_case(model))
                        || g.lods.iter().any(|l| l.model.eq_ignore_ascii_case(model))
                })
                .and_then(|g| {
                    self.named(&g.name)
                        .or_else(|| g.best_model().and_then(|n| self.named(n)))
                })
        })
    }
}

/// Fill [`ManifestClass::hardpoints`] for every class [`wants`] picks whose
/// skeleton `chunks` holds.
pub fn attach<'a>(
    chunks: &[mungefs::Chunk<'_>],
    classes: impl Iterator<Item = &'a mut ManifestClass>,
) {
    let skels = Skeletons::of(chunks);
    for k in classes {
        let destructible = k.base.eq_ignore_ascii_case("destructablebuilding");
        if !wants(&k.base) && !destructible {
            continue;
        }
        let Some(geom) = k.geometry.as_deref() else {
            continue;
        };
        if let Some(s) = skels.find(geom) {
            k.hardpoints = of(s);
            // Only a vehicle's rider hangs by the axes (card VRIDE): an armed
            // building's nodes stay positions, as its rig carries them.
            if !crate::vehicle::is_vehicle_base(&k.base) && !destructible {
                k.hardpoints.iter_mut().for_each(|h| h.axes = None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Data-gated: TAT1's skiff (a map-local class). Its three gunner
    /// sections fire from `hp_firel`, `hp_fire` and `hp_firer`; the left and
    /// right guns sit on opposite sides of the hull, the middle one on it.
    #[test]
    fn tat1_skiff_names_its_three_fire_points() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/TAT/TAT1.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: TAT1.LVL not present");
            return;
        };
        let inner = mungefs::open(&buf).unwrap();
        let chunks = mungefs::content_chunks(inner).unwrap();
        let mut k = ManifestClass {
            effects: Vec::new(),
            name: "tat_hover_skiff".into(),
            base: "hover".into(),
            geometry: Some("all_hover_skiff".into()),
            collision: vec![],
            door: None,
            hardpoints: vec![],
            attach: vec![],
            build_zone: None,
            lights: vec![],
            turret_rig: None,
        };
        attach(&chunks, std::iter::once(&mut k));
        let at = |n: &str| {
            k.hardpoints
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case(n))
                .unwrap_or_else(|| panic!("{n} missing: {:?}", k.hardpoints))
                .pos
        };
        let (l, m, r) = (at("hp_firel"), at("hp_fire"), at("hp_firer"));
        eprintln!("{:?}", k.hardpoints);
        assert!(l[0] * r[0] < 0.0, "the side guns straddle the hull");
        assert!(m[0].abs() < l[0].abs().min(r[0].abs()));
    }
}
