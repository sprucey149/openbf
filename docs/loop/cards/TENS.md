---
id: TENS
title: BF1 Kamino and Bespin Platforms have NO terrain floor on the PS2 (TerrainEnable 0: height query -1000, terrain rays never hit) — the port's sim stands soldiers on the y ~ 0 heightfield under the platforms
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
On BF1 KAM1 and BES1 a soldier knocked or walked off a platform falls on the PS2. In the port he lands on an invisible
floor at y ~ 0 (BES1: ~100 m below the decks) or on Kamino's sea, and walks about under the map. Falling off is a core
risk on both platform maps. The port's AI, projectiles and flyers use the same false floor.

## Evidence
- `analysis/re-terrain-enable.md` §2.3 ([O], verifier 26/26). `DomeInfo TerrainEnable(0)` sets BF1 `DAT_005229a8 = 0`
  (0 in the PS2's RAM on both maps). Then:
  - the height query `FUN_00473710` answers -1000.0 at every corner (B1 537066 ...);
  - the terrain segment test `FUN_00474680` returns 1.0, no hit (537629-537631);
  - `FUN_00474810` skips the terrain (537695);
  - all 28 calls of the sampler `FUN_00382338` sit behind the flag.
- The port: `Scene::heightfield` (`game/src/assets.rs`) builds the sim's ground from the baked terrain whenever one
  exists, and KOC's manifest field (`ManifestSky` TerrainEnable) is not consulted.
- Unread (the RE half of this card):
  - twelve more readers of the flag in the decomp (B1 `FUN_00163008`, `FUN_001b8080`, `FUN_001b80d0`, `FUN_001b8c60`,
    `FUN_001d8c30`, `FUN_00212530/668/7a0`, `FUN_00254a70/ed0`, `FUN_002552b8`, `FUN_00473e08`) and `FUN_00381408`
    through an alias;
  - what kills a faller: KAM1 has a death region below the sea (the port's manifest: a cylinder at y ~ -256); BES1 has
    none in the port's manifest; a fall to -1000 or a kill height is to be found (see DTHR's fall rule).

## Acceptance
- An [O] addendum to `analysis/re-terrain-enable.md`: what each reader does at flag 0, and what ends a fall on BES1 and
  KAM1 (a PS2 measurement with `ps2live.py`: walk a soldier off a BES1 / KAM1 edge and log height and health).
- Then a build: `Scene::heightfield` returns `None` when the manifest says TerrainEnable 0 (BF2 KAM1's stub already takes
  that path). Declare `sweep`: both maps are in the 32-map sweep.

## Notes
- Created 2026-09-28 by KOC's RE (lane 2). Deps KOC: the manifest field lands there.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).

## Note (KAMR RE, 2026-09-30): the size of it
The floor census (`lead_tmp/kamr/tools/floor_census.rs`, 0.5 m cells) on BF1 KAM1: 609,483 reachable points
(152,371 m2) of walkable y = 0 heightfield with nothing drawn within 0.3 m; BF1 BES1 at 1 m cells: 265,324 m2.
`analysis/re-kamino.md` §4.1. The maintainer's "invisible floors" on BF1 Kamino is this card.
