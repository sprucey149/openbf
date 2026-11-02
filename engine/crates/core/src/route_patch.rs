//! BF2's `plnp` route patches applied to the route table: what
//! `BlockPlanningGraphArcs` / `UnblockPlanningGraphArcs` do to the router.
//!
//! The disc (RE_LOG 2026-09-01 "`plan` / `plnp` / `Hint` / `BARR`" [V], the
//! hops re-read for DR3): a `plnp` names up to eight blockable GROUPS of arcs
//! (fnv1a of the arc names) and ships one XOR patch per blocked-group bitmask,
//! record 0 being "all open" (`hubCount` zero bytes). The planning object keeps
//! the current mask at +0x2c; its constructor `FUN_002ead18` zeroes it,
//! so every map starts on the table as authored. The
//! three Lua verbs mask the group bits in (`FUN_002ebe28`, `|`), out
//! (`FUN_002ebea8`, `& ~`) or toggle them (`FUN_002ebda8`, `^`); a name's
//! group is the first group whose hash list holds it (`FUN_002ebcc0`), an
//! unknown name is ignored. `FUN_002ec5f8` clamps the new mask to the group
//! count, and when it differs XORs the CURRENT record back off (restoring the
//! authored table), stores the mask and XORs the new record on
//! (`FUN_002ec818`: per hub `u8 has`; if set, for t = 0..4 `u8 rows` then
//! `rows × hubCount` bytes onto that hub's row k of type t).
//!
//! Not modelled [H]: `FUN_002ec5f8` goes on to post an event (`FUN_002b6258`,
//! type 10) after the swap — presumably the units' re-plan; the port's units
//! re-read the table at their next chooser step anyway.
//!
//! The patches are validated against the table ONCE, when attached, so a
//! swap can never half-apply: a record that does not parse, or names more
//! rows than the base hub has, rejects the whole set.

use crate::nav::{NavGraph, RouteTable, TYPE_COUNT};

/// One `plnp` patch set as the bake writes it, plus the mask in force.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoutePatches {
    /// Per blockable group, the fnv1a (lower-case) hashes of its arc names.
    groups: Vec<Vec<u32>>,
    /// `1 << groups.len()` inflated XOR patches, indexed by the blocked-group
    /// mask; record 0 is all open.
    records: Vec<Vec<u8>>,
    /// The blocked-group mask now XORed onto the table (the PS2's +0x2c).
    mask: u32,
}

impl RoutePatches {
    /// Parse the bake's `nav/plnp_<key>.bin` (`u16 recordCount`, then per
    /// record `u32 len` + `len` bytes) against the manifest's group hashes.
    pub fn parse(groups: Vec<Vec<u32>>, bytes: &[u8]) -> Result<RoutePatches, String> {
        if groups.len() >= 9 {
            return Err(format!(
                "plnp: {} groups (the loader refuses 9+)",
                groups.len()
            ));
        }
        let count = bytes
            .get(..2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
            .ok_or("plnp sidecar has no record count")?;
        if count != 1 << groups.len() {
            return Err(format!("plnp: {count} records for {} groups", groups.len()));
        }
        let mut at = 2usize;
        let mut records = Vec::with_capacity(count);
        for i in 0..count {
            let len = bytes
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
                .ok_or_else(|| format!("plnp: record {i} header truncated"))?;
            at += 4;
            let body = bytes
                .get(at..at + len)
                .ok_or_else(|| format!("plnp: record {i} wants {len} bytes"))?;
            records.push(body.to_vec());
            at += len;
        }
        if at != bytes.len() {
            return Err(format!("plnp: {} trailing bytes", bytes.len() - at));
        }
        Ok(RoutePatches {
            groups,
            records,
            mask: 0,
        })
    }

    /// The blocked-group mask in force.
    pub fn mask(&self) -> u32 {
        self.mask
    }

    /// The group bits `hashes` name (`FUN_002ebd28`): each arc-name hash sets
    /// the bit of the first group that lists it; an unknown hash sets nothing.
    pub fn group_bits(&self, hashes: &[u32]) -> u32 {
        hashes
            .iter()
            .filter_map(|h| self.groups.iter().position(|g| g.contains(h)))
            .fold(0, |m, g| m | 1 << g)
    }

    /// Walk one record as `FUN_002ec818` does, handing every XOR run to `f`
    /// as `(type, hub, row, bytes)`. `Err` if the record does not tile.
    fn walk(
        rec: &[u8],
        hub_count: usize,
        mut f: impl FnMut(usize, usize, usize, &[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut at = 0usize;
        let byte = |at: &mut usize| -> Result<usize, String> {
            let b = *rec.get(*at).ok_or("plnp record truncated")?;
            *at += 1;
            Ok(b as usize)
        };
        for hub in 0..hub_count {
            if byte(&mut at)? == 0 {
                continue;
            }
            for t in 0..TYPE_COUNT {
                let rows = byte(&mut at)?;
                for k in 0..rows {
                    let run = rec
                        .get(at..at + hub_count)
                        .ok_or("plnp record truncated in a row")?;
                    f(t, hub, k, run)?;
                    at += hub_count;
                }
            }
        }
        if at != rec.len() {
            return Err(format!("plnp record: {} trailing bytes", rec.len() - at));
        }
        Ok(())
    }

    /// Check every record against `table`: it must tile, and every row it
    /// patches must exist in the base table.
    fn validate(&self, table: &RouteTable) -> Result<(), String> {
        for (i, rec) in self.records.iter().enumerate() {
            Self::walk(rec, table.hub_count, |t, hub, k, _| {
                match table.hub_rows_len(t, hub) {
                    Some(n) if k < n => Ok(()),
                    n => Err(format!(
                        "plnp record {i}: hub {hub} type {t} row {k}, base has {n:?}"
                    )),
                }
            })?;
        }
        Ok(())
    }

    /// XOR record `mask` onto `table`. Only called on a validated set.
    fn xor(&self, table: &mut RouteTable, mask: u32) {
        let hub_count = table.hub_count;
        let _ = Self::walk(&self.records[mask as usize], hub_count, |t, hub, k, run| {
            if let Some(row) = table.hub_row_mut(t, hub, k) {
                for (d, s) in row.iter_mut().zip(run) {
                    *d ^= s;
                }
            }
            Ok(())
        });
    }
}

impl NavGraph {
    /// Attach a `plnp` patch set, validated against this graph's table. The
    /// mask starts at 0 (the PS2 constructor's), so the table is unchanged.
    /// Unblock an existing set before replacing it; refusal leaves it intact.
    pub fn attach_patches(&mut self, patches: RoutePatches) -> Result<(), String> {
        if self.blocked_groups() != 0 {
            return Err("unblock the current planning patches before replacing them".into());
        }
        patches.validate(&self.routes)?;
        self.patches = Some(RoutePatches { mask: 0, ..patches });
        Ok(())
    }

    /// The blocked-group mask in force (0 without patches).
    pub fn blocked_groups(&self) -> u32 {
        self.patches.as_ref().map_or(0, |p| p.mask)
    }

    /// `FUN_002ec5f8`: make `mask` the blocked-group set — restore the
    /// authored table from the current record, then apply the new one. A
    /// no-op without patches or when the (clamped) mask is unchanged.
    pub fn set_blocked_groups(&mut self, mask: u32) {
        let Some(mut p) = self.patches.take() else {
            return;
        };
        let mask = mask & ((1u32 << p.groups.len()) - 1);
        if mask != p.mask {
            if p.mask != 0 {
                p.xor(&mut self.routes, p.mask);
            }
            p.mask = mask;
            if mask != 0 {
                p.xor(&mut self.routes, mask);
            }
        }
        self.patches = Some(p);
    }

    /// `BlockPlanningGraphArcs` (`block`) / `UnblockPlanningGraphArcs` by arc
    /// name hash (fnv1a of the lower-cased name, as the patch set stores it).
    pub fn block_arcs(&mut self, hashes: &[u32], block: bool) {
        let (cur, bits) = match &self.patches {
            Some(p) => (p.mask, p.group_bits(hashes)),
            None => return,
        };
        self.set_blocked_groups(if block { cur | bits } else { cur & !bits });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::{Arc, Hub};
    use glam::Vec3;

    /// Two hubs, one arc; SOLDIER hub 0 has two rows, the others one each.
    fn graph() -> NavGraph {
        let hub = |x| Hub {
            pos: Vec3::new(x, 0.0, 0.0),
            radius: 1.0,
            arcs: vec![0],
        };
        let rows = (0..TYPE_COUNT * 2)
            .map(|i| {
                if i == 0 {
                    vec![0, 8, 0, 16]
                } else {
                    vec![0, 0]
                }
            })
            .collect();
        NavGraph::new(
            vec![hub(0.0), hub(5.0)],
            vec![Arc {
                a: 0,
                b: 1,
                type_mask: 1,
                flags: 0,
            }],
            RouteTable::from_rows(2, TYPE_COUNT, rows),
            Vec::new(),
            Vec::new(),
        )
        .expect("graph")
    }

    fn sidecar(records: &[Vec<u8>]) -> Vec<u8> {
        let mut b = (records.len() as u16).to_le_bytes().to_vec();
        for r in records {
            b.extend((r.len() as u32).to_le_bytes());
            b.extend(r);
        }
        b
    }

    /// Record 1 patches SOLDIER hub 0's row 1 (0,16 -> 0,24) and nothing else.
    fn patches() -> RoutePatches {
        let rec1 = vec![1, 2, 0, 0, 0, 8, 0, 0, 0, 0, 0];
        RoutePatches::parse(vec![vec![0xAB, 0xCD]], &sidecar(&[vec![0, 0], rec1])).expect("parse")
    }

    #[test]
    fn block_applies_the_record_and_unblock_restores_the_table() {
        let mut g = graph();
        let authored = g.routes.clone();
        g.attach_patches(patches()).expect("attach");
        assert_eq!(g.routes, authored, "attaching changes nothing");
        g.block_arcs(&[0xCD], true);
        assert_eq!(g.blocked_groups(), 1);
        assert_eq!(g.routes.entries(0, 0, 1).collect::<Vec<_>>(), vec![8, 24]);
        g.block_arcs(&[0xAB], true); // same group: no change
        assert_eq!(g.routes.entries(0, 0, 1).collect::<Vec<_>>(), vec![8, 24]);
        g.block_arcs(&[0x1234], false); // unknown name: ignored
        assert_eq!(g.blocked_groups(), 1);
        g.block_arcs(&[0xAB], false);
        assert_eq!(g.routes, authored, "unblocking restores the authored table");
        g.set_blocked_groups(0xFF); // clamped to the one group
        assert_eq!(g.blocked_groups(), 1);
    }

    #[test]
    fn replacing_active_patches_is_transactionally_refused() {
        let mut g = graph();
        let authored = g.routes.clone();
        g.attach_patches(patches()).unwrap();
        g.set_blocked_groups(1);
        let blocked = g.routes.clone();
        let short =
            RoutePatches::parse(vec![vec![1]], &sidecar(&[vec![0, 0], vec![1, 1]])).unwrap();
        for replacement in [patches(), short] {
            assert!(g.attach_patches(replacement).is_err());
            assert_eq!(g.blocked_groups(), 1);
            assert_eq!(g.routes, blocked);
        }
        g.set_blocked_groups(0);
        assert_eq!(g.routes, authored);
        g.attach_patches(patches()).unwrap();
        assert_eq!(g.routes, authored);
        g.set_blocked_groups(1);
        assert_eq!(g.routes, blocked);
        g.set_blocked_groups(0);
        assert_eq!(g.routes, authored);
    }

    #[test]
    fn a_patch_past_the_base_rows_or_short_is_refused() {
        let mut g = graph();
        // hub 1 has one SOLDIER row; a patch naming two must be refused.
        let bad = vec![0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0];
        let p = RoutePatches::parse(vec![vec![1]], &sidecar(&[vec![0, 0], bad])).unwrap();
        assert!(g.attach_patches(p).is_err());
        let short =
            RoutePatches::parse(vec![vec![1]], &sidecar(&[vec![0, 0], vec![1, 1]])).unwrap();
        assert!(g.attach_patches(short).is_err());
        assert!(RoutePatches::parse(vec![vec![1]], &sidecar(&[vec![0, 0]])).is_err());
        assert_eq!(g.blocked_groups(), 0);
    }
}
