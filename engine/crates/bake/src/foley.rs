//! A SOUND LVL's `ffx_` foley tables → `<map>/sounds/foley.ron` (card FOLY,
//! `analysis/re-foley.md` §1, `[O]`): per sub-level `<map><era>` its surface
//! groups, each a list of rows (a soldier family's `FoleyFXClass` hash → the
//! sound property hash of each slot). The properties themselves are in the
//! same LVL's `sounds.ron`.

use anyhow::Result;
use manifest::{FoleyGroup, FoleyLevel, FoleyRon, FoleyRow};
use std::path::Path;

fn row(r: &mungefs::foley::FoleySoldier) -> FoleyRow {
    FoleyRow {
        class: r.class,
        walk_left: r.walk_left,
        walk_right: r.walk_right,
        run_left: r.run_left,
        run_right: r.run_right,
        jump: r.jump,
        land: r.land,
        roll: r.roll,
        prone: r.prone,
        squat: r.squat,
        stand: r.stand,
        body_fall: r.body_fall,
    }
}

/// Every foley sub-level of an opened SOUND LVL payload.
pub fn table(inner: &[u8]) -> FoleyRon {
    FoleyRon {
        levels: mungefs::foley::find_all(inner)
            .iter()
            .map(|l| FoleyLevel {
                name: l.name,
                groups: l
                    .groups
                    .iter()
                    .map(|g| FoleyGroup {
                        name: g.name,
                        rows: g.rows.iter().map(row).collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Write `<sounds dir>/foley.ron` when the LVL has foley; returns the number
/// of sub-levels. Removes the prior table when the current LVL has none.
pub fn write(inner: &[u8], sounds_dir: &Path) -> Result<usize> {
    let t = table(inner);
    if !t.levels.is_empty() {
        crate::write_ron(&sounds_dir.join("foley.ron"), &t)?;
    } else {
        crate::remove_optional_file(&sounds_dir.join("foley.ron"))?;
    }
    Ok(t.levels.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Card FOLY's bake acceptance, from the discs: `nab1cw` = 4 groups x 3
    /// rows, `uta1cw` = 3 x 5 - what the PS2's RAM holds (re-foley §8.3).
    #[test]
    fn the_baked_table_matches_the_ps2s_ram() {
        let e = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
        for (lvl, sub, groups, rows) in [
            ("bf1/sound/DATA/_LVL_PS2/SOUND/NAB.LVL", "nab1cw", 4, 3),
            ("bf2/sound/DATA/_LVL_PS2/SOUND/UTA.LVL", "uta1cw", 3, 5),
        ] {
            let Ok(buf) = std::fs::read(format!("{e}/{lvl}")) else {
                eprintln!("SKIP: {lvl} not present");
                continue;
            };
            let t = table(mungefs::open(&buf).unwrap());
            let h = mungefs::fnv1a_lowercase(sub);
            let l = t.levels.iter().find(|l| l.name == h).expect(sub);
            let shape: Vec<usize> = l.groups.iter().map(|g| g.rows.len()).collect();
            assert_eq!(shape, vec![rows; groups], "{sub}");
            // The file survives a round trip through its reader's type.
            let text = ron::ser::to_string_pretty(&t, crate::pretty()).unwrap();
            assert_eq!(ron::from_str::<FoleyRon>(&text).unwrap(), t);
        }
    }
}
