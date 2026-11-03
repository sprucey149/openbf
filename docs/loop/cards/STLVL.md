---
id: STLVL
title: A unit on another floor has not arrived: level-aware hub arrival, locate and re-plan on the XZ planning graphs (KAM1 rims, POL1 hangar ramps, TAT2 raised post, MUS1 59.4 level, GEO1 cave)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
STALL C2 (`analysis/re-round-stalls.md` §2 C2). The BF2 graphs author every hub at one height, and arrival and locate
test XZ. `ai::route::hub_has_floor` accepts any floor under the hub at the unit's level. So a unit that reaches a hub's
XZ on another floor counts as arrived, and its next legs walk under or beside the floor they need.

## Evidence
- KAM1 platform rims: 69 % of no-progress episodes; the worst cluster is (−69.9, 50.5, 269.6), 1.02 M unit-ticks.
- POL1 hangar_04 ramps: (112–120, 13.1, −38), 1.1 M unit-ticks on 9/9 seeds; `maps/pol1_b.png`, sliced at y 20.
- TAT2 seed 8: six attackers under cp0's capture box (y 9–13) for the whole round.
- MUS1: the 59.4 level after a fall.
- GEO1: unit 27 in `geo_prop_cave1`.
- **PS2:** KAM1's alive samples never touch the rims (p1 51.4). POL1's never go near hangar_04's spots.

## Design (the port's own; the PS2's routing is XZ too, but its bots do not leave the level)
- Derive each hub's walkable floor(s) at load, from its arcs' walked line and the posts' capture boxes.
- Arrival needs the unit within ~2 m of that floor.
- Locate and re-plan pick hubs on the unit's floor.
- A unit whose floor has no hub routes to the nearest hub on its floor.
- The last resort is a PROVISIONAL recycle: a bot with no route and no progress for 3 min is despawned and respawned,
  with no ticket cost. Record it in FEEL_NOTES; reversible.

## Notes
AQD (BF1's 3-D locate, value 2) overlaps. Fold it in or close it.

## Note from SWPN (2026-09-30)
Judge the fix over seeds 1-8: `scripts/sweep_seeds.py --out <dir> --target <your target> --baseline lead_tmp/loop_baselines/20260930_1600_SWPN` (main: 27/32 maps >= 7/8, 227/256 rounds; ~2 min).

## Note from STFAL (2026-09-30)
With STFAL's floor check (de4ba72) KAM1 units no longer walk off where a wrong-floor leg crosses open air — they stay
alive and stranded. The new top KAM1 clusters are leg 19→34 walked from the 52.1 deck under hub 19's 63.3 bridge:
(47.2, 52.1, 195.6) and (47.8, 52.1, 186.4), 255 episodes over 8 seeds (`lead_tmp/stfal/v2/`, `eps.py`). Carried
acceptance: ≤ 1 % of KAM1 no-progress episodes open below the 51.4 deck (main 555/803; STFAL 214/925, `v4_32/`,
`summ.py`).

## Note from the SWPN audit (2026-09-30, `reports/20260930_1745_audit_SWPN.md`)
- **Judge baseline.** The SWPN note's `--baseline …/20260930_1600_SWPN` predates STFAL. Use
  `lead_tmp/audit-swpn/main_e09c854/`: sweep_seeds.py on main e09c854, seeds 1-8 plus 42. Its seeds 1-8 give 28/32 maps
  ≥ 7/8 and 228/256 rounds; KAM1 0/8, 96 % region; POL1 1/8. Or re-run main first if it has moved.
- **The seeds.md `noprog at end` column counts idle holders.** Over seeds 1-8 on 6c3c8ea, 48 of its 180 units are
  `steer idle`, vel 0, within ~1 m of their goal (POL1 22 of 46). This is CEN's rule, `NoProgress::stalled`, which has
  no idle filter. Judge by the finish rate and the episodes this card names, not by that column.
- **`--baseline` Δ columns** are raw count differences, so give both runs the same seed set. A baseline missing one
  of the run's seeds prints e.g. "+2 (was 3/4)" for 5/8.
- **C2's mechanism is confirmed.** `advance_route` hops on `xz_dist` alone. On POL1 seed 1, unit 26 stands on the
  terrain over the hangar (y 45.8), 4.8 m in XZ from hub 4, whose floor is 13.1; it has been stuck 38,640 ticks.

## Note from STHOP's build (2026-09-30, lane 1) - an acceptance line carried here
- **KAM1 environment deaths <= 5 % of deaths** (from STFAL, then STHOP) is now this card's to meet: after STHOP,
  8 seeds read 367 region deaths of 497 (74 %; main before: 1370 of 1427, 96 %), kills 57 -> 130, 0/8 finished,
  no-progress at the end 74 -> 124 (`lead_tmp/sthop/sweep8_final/seeds.md`). The soldiers now stay on the ramps; what
  is left is units on the wrong level.
- Baseline for the 8-seed table after STHOP: `lead_tmp/sthop/sweep8_final` (226/256 rounds, 28/32 maps >= 7/8) - but
  re-run main first, it goes stale.
- The AI steering has a new probe, `steer::steep_ahead` (a floor too steep to climb is a wall): POL1 1/8 -> 4/8 with it.

- Fable 2026-10-01: after GRND (3c7b00c) + KSTK (f293fd2) KAM1 reads 1/8 finished, region deaths 85-117 of ~480 (8 seeds), 6 of 30 stuck at 200 s on seed 1 - 4 of them over the rim rail on a hull or core. Baseline for this card's 8-seed judge: `lead_tmp/kamai-fable/sweep_nav_plus_classes_seeds.md` (main 023e3fd).

## Note from the maintainer's "rounds that do not end" pass (2026-10-01, main 4603e9e, `lead_tmp/endings/README.md`) — `first: 4`
- **BF1 YAV1, a new wrong-floor spot:** (107.4, -62.7, -136.1), on the arc at hub 17, 3.3-3.9 m UNDER the hub's floor,
  steer `slide`, objective attack cp3 (`samples/bf1_yav1_s8.csv`; 4,034 unit-seconds within 3 m over the round). On seeds
  2 and 8 the last team-0 unit sits there to tick 120,000 (3,424 s on seed 8) and the round never ends: 3 v 1 and 1 v 1.
  `level` reads 0.0 there, so BNDS's 45 s recycle does not fire. Added acceptance: BF1 YAV1 8/8 at 120,000 ticks with no
  unit-seconds at that spot past 60 s.
- The "last resort recycle" line in this card's Design is now card **RCYC** (`first: 2`). The maintainer: it should exist and never
  be needed - this card closes the gaps, and RCYC's census (recycles per sweep, by spot) is how that is shown. Added
  acceptance: 0 recycles at this card's spots.
- Since this card was written: BNDS (9440823) made hub arrival level-aware (`route::hub_on_level`, `brain::hub_reached`)
  and resolved hub floors (`Sim::nav_floors`). Start by measuring what is LEFT with RCYC's census, not from the 09-30
  cluster list.

## Note from RCYC (2026-10-01) - the recycle spots this card owns, and an acceptance line
RCYC's last resort (branch `rcyc`: a bot that wants to walk and makes no net progress for 120 s is recycled) was used 496 times over 8 seeds x 32 maps x 120,000 ticks; 79 spots were recycled on 2 or more seeds. `lead_tmp/rcyc/owners.py` gives this card 21 of them (96 recycles) by a HEURISTIC (on a hub / arc whose highest floor is 3 m or more from the bot's feet: another floor, or a roof over the hub) - look at each spot before trusting it. Re-list: `sweep_seeds.py --seeds 1-8 --ticks 120000`, then seeds.md's `## Recycles`; the run behind this table is `lead_tmp/rcyc/sweep120k_d`.

**Acceptance, added: 0 recycles at these spots (seeds.md `recycled` column and spot list) on this card's maps.**

| map | spot (x, y, z) | recycles | seeds | leg | locus | floor dy | steer | basis |
|---|---|---|---|---|---|---|---|---|
| bf1_bes1 | (0, 99, -156) | 14 | 6 | 75->74 dest 38 [table] x5 | H75 x9 | -31.0 | fwd x11 | floor dy >= 3 m on a hub / arc |
| bf1_bes2 | (-96, -51, 186) | 8 | 5 | last->97 dest 97 [table] x6 | H97 x8 | -47.7 | fwd x8 | floor dy >= 3 m on a hub / arc |
| bf1_bes2 | (-252, -51, 186) | 4 | 2 | 63->81 dest 81 [table] x2 | H81 x4 | -59.7 | fwd x4 | floor dy >= 3 m on a hub / arc |
| bf1_kas2 | (180, -6, 60) | 3 | 3 | last->48 dest 47 [table] x2 | A48 x2 | -4.9 | slide x3 | floor dy >= 3 m on a hub / arc |
| bf1_yav1 | (108, -63, -138) | 5 | 3 | last->17 dest 17 [table] x4 | A17 x5 | -3.8 | slide x5 | floor dy >= 3 m on a hub / arc |
| bf2_dag1 | (90, 0, -18) | 3 | 3 | 2->3 dest 4 [table] x1 | A2 x3 | -16.2 | slide x2 | floor dy >= 3 m on a hub / arc |
| bf2_geo1 | (-78, -60, 264) | 15 | 6 | last->9 dest 9 [table] x13 | H9 x15 | -10.7 | slide x15 | floor dy >= 3 m on a hub / arc |
| bf2_geo1 | (-78, -60, 270) | 5 | 4 | last->9 dest 9 [table] x4 | H9 x5 | -11.3 | slide x4 | floor dy >= 3 m on a hub / arc |
| bf2_geo1 | (-90, -63, 258) | 3 | 2 | last->9 dest 9 [table] x3 | H9 x3 | -13.3 | slide x3 | floor dy >= 3 m on a hub / arc |
| bf2_geo1 | (-84, -63, 264) | 2 | 2 | 9->12 dest 56 [table] x1 | H9 x2 | -13.2 | slide x2 | floor dy >= 3 m on a hub / arc |
| bf2_geo1 | (-84, -60, 258) | 2 | 2 | 9->12 dest 56 [table] x1 | H9 x2 | -10.7 | slide x2 | floor dy >= 3 m on a hub / arc |
| bf2_geo1 | (-78, -63, 276) | 2 | 2 | 9->0 dest 22 [table] x2 | H9 x2 | -13.2 | slide x2 | floor dy >= 3 m on a hub / arc |
| bf2_geo1 | (18, -63, -84) | 2 | 2 | 16->17 dest 56 [table] x1 | H16 x2 | -12.9 | slide x2 | floor dy >= 3 m on a hub / arc |
| bf2_hot1 | (-318, 0, -54) | 10 | 6 | 4->28 dest 29 [table] x5 | A4 x10 | +5.8 | slide x10 | floor dy >= 3 m on a hub / arc |
| bf2_kam1 | (54, 54, 144) | 3 | 2 | 123->121 dest 86 [table] x2 | A123 x3 | -11.3 | ledge x3 | floor dy >= 3 m on a hub / arc (ledge stand) |
| bf2_kam1 | (-186, 69, 30) | 2 | 2 | last->100 dest 100 [table] x2 | H100 x2 | -128.0 | slide x2 | floor dy >= 3 m on a hub / arc |
| bf2_kam1 | (-66, 57, 276) | 2 | 2 | 53->66 dest 100 [table] x1 | A53 x1 | -1.9 | fwd x2 | floor dy >= 3 m on a hub / arc |
| bf2_kam1 | (114, 51, 144) | 2 | 2 | 88->89 dest 47 [table] x2 | H88 x2 | -32.7 | slide x2 | floor dy >= 3 m on a hub / arc |
| bf2_mus1 | (-12, 66, 102) | 2 | 2 | 0->2 dest 11 [table] x1 | A0 x2 | -3.3 | fwd x2 | floor dy >= 3 m on a hub / arc |
| bf2_pol1 | (-48, 24, 84) | 2 | 2 | 0->61 dest 107 [table] x1 | A0 x1 | -4.0 | fwd x1 | floor dy >= 3 m on a hub / arc |
| tat1 | (-114, 15, 0) | 5 | 5 | last->104 dest 104 [table] x3 | H104 x5 | +6.9 | slide x5 | floor dy >= 3 m on a hub / arc |

BF2 KAM1 (54, 54, 144) is the ledge stand at arc 137 (leg 123 -> 121, steer `ledge`): card KA137 owns it.
BF1 YAV1 (108, -63, -138) is this card's hub-17 spot: with RCYC, YAV1 ends 8/8 by recycling it 5 times in 3 seeds.

- Planner 2026-10-01 22:00: `Sim::level_at` (BNDS) is not yet checked on terrain - main ce66fe2 kills 71 bots as `lost` on BF2 UTA1 and 33 on BF1 HOT1 over 8 seeds x 120,000 ticks (`lead_tmp/rcyc/sweep120k_main/seeds.md`); card **LVLF** (value 5, measure) classes them as really off or a false reading. If LVLF has landed, use its table; if not, do not make arrival or re-plan depend on `level_at().is_off()` without reporting the `lost` column for UTA1, HOT1 (both games) and KAM1 before and after. DTHR now also carries SLAND (landing damage).

## Note from LVLF (2026-10-02) - the level test, measured (`lead_tmp/lvlf/README.md`)
- No `lost` death in 8 seeds x 32 maps was a bot that walked off the level: 61 spawned under UTA1 (fixed), 34 stood in Hoth's trenches (fixed: a trench is on the level), 34 stand on ground a walk leads back from (card RETW).
- 21 of those 34 are this card's case on BF2 UTA1: bots go UNDER the landing (hubs 12 / 13 / 56 / 61, floor 14.4) on the ground floor (y 9.4), entering at (-135.8, 9.4, 123), and die at x -143..-134, z 92..111. Which hub they were routed at was not logged: check it first. MYG1 has one under the y 4.0 walkway at (-25, 0, -44). After this card lands, re-run `lvlf_probe classify` (RETW's acceptance) and expect those gone.
- `Sim::level_at` is safe to lean on for 'is this unit off its hub's floor' on decks; on terrain maps it now says ON in every open trench (both Hoths: 0 of 4,067 / 4,663 trench cells off).

## WAI build note (2026-10-02, `lead_tmp/wai/build/`)
- **Geonosis's raised posts are this trap too, and they now cost a round in 8 seeds.** BF1 GEO1 cp13 (pos y -59, its own
  floor at -58.7, terrain -62.3) and BF2 GEO1 cp6 (y -59 over -62.9): a unit on the terrain under the post and the units
  on the post's floor are 3.6 m apart, inside one capture cylinder, `BLOCKED world`, and neither side moves.
  With bots piloting the spiders (card WAI) the CIS reaches a 1-v-16 endgame there more often: BF1 GEO1 seed 3
  (`b1geo1_s3.txt`: tickets 1/73, the last clone at (-2.7, -62.1, -1.5) under ten droids at y -58.7) and seed 4 in the
  build before it; BF2 GEO1 seed 6 in the first build (`geo1_s6.txt`). Main's units stand on that floor as well
  (seed 3 at tick 8000: two at y -58.7), so the trap is the map's, not a hull's.

## Build note (2026-10-02, branch `stlvl`, `lead_tmp/stlvl/`, report `reports/20261002_0700_card_STLVL.md`)
Measured first (main fb216e6, 8 seeds x 120,000 ticks): 495 recycles; the RCYC table's `floor_dy` reads a ROOF over
the hub as its floor, so most of its 21 spots were not storeys. Against the routing's own floors (`recycle_lv`, new)
34 recycles were a bot AT its leg's hub and > 3 m off that hub's resolved floor. Four causes, four fixes:
1. the floor walk fell through a seam between two models (BF2 MYG1 hub 32 resolved under its balcony) - the foot's
   ring is asked where the one ray falls more than a step;
2. a post / spawn node pinned its NEAREST hub whatever its storey (MYG1 cp1, DAG1's cave mouth) - it pins only the hub
   whose disc it stands in;
3. the graph locates in XZ - `route::start_hub` and the hunted hub now take the nearest hub the floor walked from the
   feet reaches (`level_hub`: TAT2 rooftop hubs 25 / 26, BF1 BES2 hub 97, DAG1 cave);
4. a defence spot more than `LEDGE_DROP` under its post is not offered (BF2 DEA1 cp2).

| | main, seeds 1-8 | stlvl, 1-8 | main, 9-16 | stlvl, 9-16 |
|---|---|---|---|---|
| rounds finished | 253 / 256 | 255 / 256 | 256 / 256 | 255 / 256 |
| recycles | 495 | 332 | 356 | 432 (235 without BF2 GEO1; main 316) |
| on another storey at the leg's hub | 34 | 0 | not measured (old binary) | 10 (BF2 GEO1 hub 18) |
| `lost` deaths | 45 | 10 | 39 | 14 |
| BF2 POL1 / KAM1 / BF1 YAV1 finished | 8 / 8 / 7 | 8 / 8 / 8 | 8 / 8 / 8 | 8 / 8 / 8 |
| BF2 KAM1 world deaths | 1 % | 0 % | 0 % | 0 % |

Acceptance, line by line: no bot stalls off its leg hub's floor - met on seeds 1-8 (34 -> 0), 10 left on 9-16, all
BF2 GEO1's raised cp6; POL1 >= 7/8 and KAM1 >= 7/8 - met (8/8); KAM1 environment deaths <= 5 % - met (0-1 %);
BF1 YAV1 8/8 with nothing at hub 17's spot - met (0 recycles there in all four runs); "0 recycles at the 21 RCYC
spots" - NOT met: 94 -> 55 (seeds 1-8) and 55 -> 68 (9-16); without BF2 GEO1's seven it is 43 -> 15 and 32 -> 10
(`tools/spots.py`). What is left there is not a storey: BF2 GEO1 hubs 9 / 16 / 18 and TAT1 hub 104 are last legs
(card **LLEG**), DAG1 (90, 0, -18) and BES1 (0, 99, -156) are flat. Not measured: STFAL's "<= 1 % of KAM1 no-progress
episodes open below the 51.4 deck" (its `eps.py` reads a census this run did not write).
BF2 GEO1 still loses 1 round in 8 on both seed sets (main: 2 and 0): the hung rounds are at cp6 / cp7, LLEG's.
