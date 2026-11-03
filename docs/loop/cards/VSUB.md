---
id: VSUB
title: Vehicles meet each building's VEHICLE subset, not its soldier subset: the port's hover (and walker) world contacts query Layers::SOLDIER, but the PS2 pairs a mover's building subset with the building's vehicle subset (B1 +0x98 / B2 +0x110; re-hover §13.3) — 7,967 of 22,258 base-world bodies are soldier-only (PS2 vehicles pass them: FEL1's foliage stalks, KAM1, UTA1, POL1, MYG1, COR1) and 1,477 vehicle-only (PS2 vehicles stop: FEL1's clump cylinders, BF1 KAS1, MYG1 walls, UTA1, BF1 NAB1's tree cylinders)
status: archived
issue: 98
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #98](https://github.com/sprucey149/openbf/issues/98).

## Why
From HHUL's RE (2026-09-30, analysis/re-hover.md §13.3 [O]). When a hover meets a building, the PS2 tests the hover's
building subset against the building's **vehicle** subset:
- BF1: FUN_00168a60, B1:7397-7399; hover layer 4, building 8, so the building's +0x98.
- BF2: FUN_00169328, B2:7872-7874; hover 8, building 0x10, so the building's +0x110.

The port's `WorldContacts` and `WalkerWorld` (`core/src/sim/vehicles.rs`) query `Layers::SOLDIER` ("the port has no
vehicle subset yet"). Players see both errors:
- A hover snags on foliage stalks and props the PS2 drives through.
- It passes trees and vehicle walls the PS2 stops at.

## Evidence
- `engine/artifacts/lead_tmp/hhul/world_vehicle_subset.py` → `world_out.txt`. It applies game::collision_layers's rule
  to the 32 baked maps' base instances, with common merged. 22,258 instance-bodies:
  - **7,967 soldier-only:** FEL1 3,135; KAM1 1,052; UTA1 707; POL1 640; MYG1 638; COR1 618; TAN1 296; BF1 KAS1 211;
    BF1 GEO1 126 ….
  - **1,477 vehicle-only:** FEL1 485; BF1 KAS1 279; MYG1 217; UTA1 131; BF1 NAB1 106; BF1 KAS2 52 ….
  - 12,814 in both.
- Example: `fel1_bldg_whisker_clump01` has 12 `p_-so-_mutsuko*` stalks (MASK 0x11: soldier + ordnance) and one
  `p_-v-_mutsuko9` cylinder (MASK 2: vehicle only).
- Hull side (§13.4, RAM). On the four BF2 classes with no collision lines (`all_hover_combatspeeder`,
  `all_hover_bos_combatspeeder`, `cis_tread_snailtank`, `rep_hover_barcspeeder`), the MASK default leaves the SPRING
  SPHERES in the class's soldier and vehicle subsets. PS2 soldiers are stopped by those invisible spheres; VHULL's hull
  has none.

## Scope
1. `game::assets::body_layers` / `Scene::collision_world`: `Layer::Vehicle` → `Layers::VEHICLE` (reserved today). The
   world becomes the union of the soldier, ordnance and vehicle subsets, with a mask per body, so vehicle-only bodies
   enter the world. Soldier and ordnance queries keep their masks.
2. `WorldContacts` (hover springs and hull) and `WalkerWorld` (ground ray, building contacts) query `Layers::VEHICLE`.
   Walkers are [D]: they reach buildings through the same collide. Confirm the walker's layer id in RAM first (memory
   `ps2-collision-layers-in-ram`: B2 8 = vehicles).
3. Hull side: the four classes' spring spheres join their VHULL hull parts in the soldier (and vehicle) subsets, as the
   §13.2 replay gives them (`lead_tmp/hhul/census.py` prints the subsets; RAM `ram_out.txt`).

## Acceptance
- `layer_census` (or a test over the manifests) reproduces 22,258 / 7,967 / 1,477 on the base worlds.
- `hover_drive`:
  - A BF2 FEL1 hover driven into a `fel1_bldg_whisker_clump01` meets only its `p_-v-_mutsuko9` cylinder, not the
    `p_-so-` stalks.
  - A BF1 NAB1 hover stops at a `nab_prop_trees2` vehicle cylinder that the port drives through today.
- Soldier leak census and soldier-walk goldens unchanged (the masks). Declared: sweep (AI hover routes meet different
  bodies). Report the per-map sweep moves.

## Notes
- Do not start while HHUL is in progress: both edit `core/src/sim/vehicles.rs`.

## Note from HHUL (2026-10-03, landed 80a8edc)
The hover's building queries are now one function, `WorldContacts::buildings` in `core/src/sim/vehicles.rs` (spring
spheres through `sphere_contacts`, hull boxes through `CollisionWorld::shape_contacts`), both with `Layers::SOLDIER`:
that is the one place to switch to the vehicle subset. Re-run `audit_hover_ram` after (47 of 47 parked PS2 hovers
inside the bob band today; scorer in `reports/20261003_0625_card_HHUL.md`).

## Note (planner 2026-10-03 07:15, from HHUL's report)
HHUL (80a8edc) left buildings on the SOLDIER subset and names this card as the next piece of "vehicles meeting
walls": `WorldContacts::buildings` is the switch, and a hull body (`HullBody`, the fighter tanks' `p_cube1`) now
meets buildings by its own shape. `core/src/sim/vehicles.rs` is 1,526 lines (the split aim is 1,500): this card
edits it, so move `WorldContacts` out as the first, behaviour-neutral commit.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.
