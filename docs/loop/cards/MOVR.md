---
id: MOVR
title: One mover: a bot that arrives stands, a request that barely moves is ignored, the PS2's leg rule and stuck test, and a walk that fails climbs a recovery ladder instead of side-stepping (bot design 4.7, 6.3)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-02: bots "stuttering back and forth", and BF1 bots that "just stop". `analysis/bot-design.md` 4.7 (the Mover: the ONE writer of the stick), 5 (commitment), 6.3 (no walk without an outcome). After JITR a port bot that stays put still walks 8.8 m in 10 s on Kamino and 20.0 m on Kashyyyk Docks (PS2 1.3 m), with 2.96 / 6.47 reversals (PS2 0.36) - `lead_tmp/botm/post_jitr/seeds.md`.

## Notes
- Cut from `analysis/bot-design.md` (card BOTD, 2026-10-03). Waits for the maintainer's read (deps BOTD). In BOTV's deps.
- PS2 facts: `analysis/re-soldier-ai.md` 1.5, 2. The Halo / Detour references for "arrive", "result / blocked / done": bot-design.md 15 (X10).
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).** Decision 6: **bots do not sprint on long walks** (as the PS2). Decision 8: **one think rate everywhere** (phases by unit index only; no level of detail by distance from the player - PERF may ask for it later).
- 2026-10-05 planner: `effort: high` (the builds the maintainer's top concern rests on; a build runs at medium by default). Undo: `loopctl.py set MOVR effort=`.

- 2026-10-06 SPL11 landed (08e2907): `think` is `ai/behave.rs`; the mover is `ai/mover.rs::run` (its input is
  `MoveRequest`, today's chain as one value: narrow it here), the gunner `ai/gunner_foot.rs::run` (`AimRequest`,
  `GunReport`). The gunner runs between the two requests because the hold's strafe on a burst, its step out of a
  friend's line and the close hold read `GunReport`; once TPOS deletes them the MoveRequest can be built first.
  Keep the seam: `python3 engine/artifacts/lead_tmp/spl11/writers_census.py engine/crates/core/src/ai/{behave,mover,gunner_foot}.rs`
  must still read one function per output.

- 2026-10-06 **LANDED 6061eec, in part** (report `reports/20261006_2224_card_MOVR.md`). On main: the arrive latch and
  the 0.5 m deadband, the heading picked every 3rd tick and held, the think's phase by unit index, the walk's outcome
  (`Walk`) with the give-up after 6 failed windows (the commander draws again without the post), `Behaviour`, B9.
  **Not on main, by measurement** (9 maps x 8 seeds, each rule behind a switch): the PS2's stuck clock in place of
  the 2 s window (POL1 pinball 2.3 -> 5.0 %), arrival 1.0 m at a node (KAM1 1.47 -> 2.03 %) and 0.5 m at the last
  (KAM1 stay reversals 0.92 -> 1.99), the PS2's leg aim (neutral to worse) - card **MOVR2**, after OBST.
  8 seeds x 32 maps: 256 / 256, recycles 208 -> 105. B1 met on the four maps (KAM1 2.2 -> 1.5 %). B2 still missed on
  KAM1 (5.7 m / 1.46), DEA1 (reversals 1.17), KAS2 (14.0 m / 3.37): wall sliders (OBST) and holders (TPOS). B9 <= 0.19 %
  on the four maps. The maintainer's scene: 2 trails in ~480 over 6 reversals on main and on the branch alike.
  Tools: `lead_tmp/movr/tools/` (`mini.sh`, `bars.py`, `stayers.py`, `scene_seeds.sh`).
