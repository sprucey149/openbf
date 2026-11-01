//! The building-turret classes a BF2 map's worlds place without carrying
//! them (card VTUR2): BF2's `SIDE/TUR.LVL`, one `lvl_` sublevel per class,
//! read by the mission lua (`ReadDataFile("SIDE\\tur.lvl", "tur_bldg_laser",
//! …)`). `crate::map` joins the pulled sublevels to its walk as layer content.

use anyhow::{anyhow, Result};
use std::path::Path;

/// `SIDE/TUR.LVL` beside the map's directory — the conquest missions of 11
/// of the 17 BF2 ground maps read it. BF1 ships none (its turrets are the
/// maps' own classes). `None` when it is absent.
pub(crate) fn side_classes_lvl(lvl_path: &str) -> Option<std::path::PathBuf> {
    Path::new(lvl_path)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| crate::join_ci(p, "SIDE/TUR.LVL"))
        .filter(|p| p.is_file())
}

/// The content of every `lvl_` sublevel of `side` (a [`side_classes_lvl`])
/// whose `entc` is a class the map's worlds place (`instance_classes`) and
/// the map does not define (`defined`): the whole sublevel — its models,
/// `skel`s, `coll`/`prim`, textures and weapon classes, the unit the
/// mission's `ReadDataFile` loads — minus any `wrld`/`plan`/`plnp`/`lght`
/// (a TUR sublevel carries none). Which sublevel holds a class is read from
/// its `entc` TYPE, not from the sublevel's hashed tag. Every TUR.LVL
/// sublevel ends with its one turret `entc`.
pub(crate) fn pull_side_classes<'a>(
    side: &'a [u8],
    instance_classes: &std::collections::BTreeSet<String>,
    defined: &std::collections::HashSet<String>,
) -> Result<(Vec<mungefs::Chunk<'a>>, Vec<String>)> {
    let inner = mungefs::open(side).map_err(|e| anyhow!("{e}"))?;
    let mut out = Vec::new();
    let mut taken = Vec::new();
    for c in mungefs::chunks(inner) {
        let c = c.map_err(|e| anyhow!("{e}"))?;
        if &c.tag != b"lvl_" {
            continue;
        }
        for sub in c.children().flatten() {
            let items: Vec<_> = sub.children().flatten().collect();
            let wants = items.iter().any(|i| {
                &i.tag == b"entc"
                    && mungefs::entity::EntityClass::parse(i).is_ok_and(|e| {
                        let n = e.name.to_ascii_lowercase();
                        let hit = instance_classes.contains(&n)
                            && !defined.contains(&n)
                            && !taken.contains(&n);
                        if hit {
                            taken.push(n);
                        }
                        hit
                    })
            });
            if wants {
                out.extend(
                    items
                        .into_iter()
                        .filter(|i| !matches!(&i.tag, b"wrld" | b"plan" | b"plnp" | b"lght")),
                );
            }
        }
    }
    Ok((out, taken))
}
