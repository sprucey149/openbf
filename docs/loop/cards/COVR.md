---
id: COVR
title: BF1 Yavin Temple takes BF2 Yavin Temple's Cover hints (same graph) - a bake donor step
milestone: M3
value: 4
phase: done
status: done
deps: HYST
files: engine/crates/bake/src/donor.rs; engine/crates/bake/src/lib.rs; engine/crates/bake/src/main.rs; engine/crates/game/tests/suite/all_maps_load.rs; scripts/rebake.sh; analysis/bot-design.md
declares: rebake=maps
needs:
effort:
branch: botup
resume:
created: 2026-10-28
---
## Why
`lead_tmp/botre/xbox_pc.md` finding 1: BF2's Yavin Temple authors 973 Cover hints on the same planning graph as BF1's
Yavin Temple, which has 51 (24 of them standable in the port's position table). Cover-hint density is the gap the bot
design measured on BF1 maps (`analysis/bot-design.md` M1, M6).

## Evidence
The baked manifests (`assets_baked/bf1_yav1`, `bf2_yav1`): 85 of 85 hubs at the same positions (0.0 m apart, no
offset); hints and hubs in the same stored frame (hint to nearest hub, median 21 m BF1 / 27 m BF2). Of BF2's 973
Cover hints 697 lie in a soldier barrier of BF1's map, 440 deeper than the position table can push one out
(`core::ai::tactic` moves a hint by at most 0.5 m to 0.35 m clear): they are authored against the props they hide
behind, as on Polis Massa.

`bake donor-hints` on a copy of `bf1_yav1` in the worktree: offered 973 -> added 409 (430 deep in a barrier, 130
off the graph, 4 on BF1's own). The table (`POSITIONS=1`): cover-hint positions standable 24 -> 400, with cover
7 -> 254; all positions standable 4,806 -> 5,182, covered 2,210 -> 2,457.

Sweep, BF1 Yavin 1 only, 24 seeds (42-65), `lead_tmp/botre/runs/COVR/seeds/` (`before` = the HYST commit on the
disc's manifest, its seed 42 row identical to HYST's full sweep; `after` = the donated manifest):

| | before | after |
|---|---|---|
| finished | 24/24 | 24/24 |
| stuck_max median / max (ticks) | 180 / 360 | 180 / 360 |
| recycles | 6 | 5 |
| fight time at an authored Cover hint's position (s) | 335 | 5,965 |
| B12: fight time at a position with cover from the target | 17.56 % | 17.70 % |
| covered picks (Tactician) | 9,480 | 10,791 |
| contact20 / fight share of foot time | 10.20 % / 31.9 % | 11.29 % / 34.1 % |
| kills per foot-minute; mean kill imbalance; rounds won by team 0 | 0.980; 0.076; 2 | 1.045; 0.059; 5 |
| team kills / world deaths | 45 / 294 | 65 / 313 |
| time in Recover | 0.90 % | 0.85 % |
| failed walks: all / body (going round a body) / given-up walks to a position | 3,172 / 290 / 1,562 | 3,837 / 861 / 1,926 |
| stand20 (unexplained standing) | 592 | 974 (board 172 -> 453: one seed 117) |

The body failures are the walks to hints against props: 36 a round, as BF2 Yavin 1 has with the same hints (36 a
round over HYST's six seeds against BF1's 14). A strict variant (`afterB`: drop every hint the table would have to
push, 79 added) kept body failures at 325 but gave little (Cover-hint fight time 1,157 s, B12 17.31 %).

## Change
`bake::donor` (D22): `bake donor-hints <assets_baked>`, run by `scripts/rebake.sh maps` after both games' maps
are baked. Per row of `DONORS` (`bf1_yav1` <- `bf2_yav1`, offset 0): the graphs must match (90 % of the target's
hubs within 0.5 m of a donor hub, moved), else it fails; the donor's Cover hints, moved, are added to the target's
nav block as `donor_<name>`, except one deeper than 0.15 m in a soldier barrier (the table pushes the rest out), one
with no hub within 40 m in XZ and 6 m in height, or one within 1 m of a hint the target authors. A second run
replaces its own. The position table then drops what has no floor or head room as for any hint. No disc data is
tracked; the runtime is unchanged.

Rebake: `scripts/rebake.sh maps` (or, on an already baked tree, `cargo run --release -p bake -- donor-hints
engine/assets_baked`).

## Deviation
D22 (`analysis/bot-design.md` 9): the PS2's BF1 Yavin Temple has 51 Cover hints; ours has BF2's on the same graph
as well.

## Open
- BF1 Bespin Platforms (BES2) and Rhen Var Harbor (RHN1): NOT imported - their BF2 versions are a fan conversion
  of the Xbox DLC (`ADDON_XBOX`). Before a `DONORS` row: in game, load each donated hint as a marker on the BF1 map
  and check, at every post, that the hints stand on the floor (not in props or under the ground), face real cover
  toward the approaches, and are reached by the bots' walks (the census's body / given-up counts against BF2's own
  map); BES2's graph matches 104 of 119 hubs and RHN1's 112 of 121, under this step's 90 % bar, so each also needs
  its own offset and the hubs that differ looked at.
- A soldier-capsule clearance test in the position table would drop hints pressed into props (the body failures)
  for every map, BF2's included.
