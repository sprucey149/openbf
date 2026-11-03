# Donor foliage placement limits

The foliage-count warnings on six maps describe different source artwork, not
missing random-number tables. The baker uses the same PS2 placement arithmetic
and checked random table for all inputs (`bake::foliage::grid`, `place`, `bake`).
PS2 placement is described in `analysis/re-foliage.md` §4: the per-block seed
(BF1 `FUN_00344a68`, line 331409; BF2 `FUN_0042add0`, line 485790) and
per-cell sampling (BF1 `FUN_00345c78`, lines 332107–332157; BF2
`FUN_0042bf00`, lines 486524–486573). No new original-game
comparison was made for this investigation.

The terrain's FOLG mask decides which cells participate. The `prp_` layers
supply density and weighted plant selection. Changing density also changes RNG
consumption and the positions of subsequent plants in a block. Matching only
the final total would not restore the PS2 placements.

| Map | PS2 instances | Xbox instances | PC1 profile instances | Xbox / PC mask cells differing from PS2 |
|---|---:|---:|---:|---:|
| BF1 Endor | 9,151 | 19,093 | 21,285 | 5 / 5 |
| BF1 Kashyyyk Islands | 2,436 | 1,419 | 1,419 | 365 / 365 |
| BF1 Kashyyyk Docks | 13,890 | 6,895 | 6,895 | 20 / 20 |
| BF1 Naboo Plains | 41,725 | 35,234 | 35,234 | 16,133 / 16,133 |
| BF1 Rhen Var Citadel | 840 | 685 | 655 | 17 / 19 |
| BF2 Endor | 6,391 | 12,367 | 12,367 | 3 / 3 |

The PC1 profile supplies BF2 from Xbox. All BF1 masks are 512×512; BF2 Endor is
128×128. Rhen Var Citadel's plant definitions match, isolating its discrepancy
to the mask. On the other five maps, density and/or weighted plant definitions
also differ. For example, Naboo Plains layer densities are PS2 0.1/0.2 versus
Xbox and PC 0.5/0.1. Xbox Kashyyyk Docks uses 0.2 instead of PS2's 0.1 on its
first layer. Endor's non-tree layers differ both in density and plant choices.

Exact restoration cannot be derived from those donor files alone. The original
mask and missing plant definitions are artwork; they must not be embedded as
baked assets or reconstructed placement lists in public source. Supported builds
with a PS2 copy already choose it as the base input and retain its placements.
For builds without that copy, retain these explicit warnings and counts. No
random thinning, density-only correction, invented placement or claim of matching
PS2 is made. Foliage drawing and collision are unchanged by this investigation.
