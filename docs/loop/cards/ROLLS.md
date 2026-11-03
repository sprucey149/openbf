---
id: ROLLS
title: The sideways roll: the PS2 has ONE roll clip (diveforward) and turns the body's root into the direction of travel while it plays - a left roll is a dive head first to the soldier's left, the camera still forward (RE done 2026-10-12, analysis/re-roll-picture.md); the port plays the right clips with NO turn, so the body somersaults forward while it slides sideways - build the root twist in game::animator, one input (the unit's velocity against its facing) for the player and the bots (the maintainer's INBOX 2026-10-12 18:55, ask 2)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX 2026-10-12 18:55, ask 2: the roll today is the FORWARD roll played while the soldier moves sideways.
The RE (2026-10-12, `analysis/re-roll-picture.md`) found what the PS2 does instead, in both games' code, in live RAM
rows and on video:
- one clip, `diveforward`, for every roll direction (no left / right / back clip in the name tables, the ELF
  strings or under 20-26 million names tried against the discs' clips);
- the skeleton's root bone (`DummyRoot`) is turned about the up axis toward the direction of TRAVEL, a quarter of
  the remaining angle per update: BF1 a fixed 90 degrees to the roll's side, BF2 atan2 of the soldier's own movement
  against his facing, every update; after the roll it turns back the same way;
- the same for the player and an AI soldier (seen on BF1).
**The PS2's picture is not "the body keeps facing forward"**: the camera and the aim do; the body turns about 90
degrees and dives head first to the side. The strips are in STATUS for the maintainer (Questions 42) - the faithful default is
built unless he says otherwise.

## Evidence
- PS2 strips: `engine/artifacts/lead_tmp/rolls/sheet_ps2_bf2.png` (forward, left, right, forward-left, and the refused
  back roll), `sheet_ps2_bf1.png` (left, right; forward + Cross is a jump), `sheet_ps2_bf1_bot.png` (an AI soldier).
- PS2 rows (`lead_tmp/rolls/ps2/analyse.py out bf1|bf2 --rows`): BF2 left roll - travel +90.0, twist 76.0 / 79.5 /
  82.1 / 85.6 / 88.1 / 89.4 / 89.9 on updates 1 / 2 / 3 / 5 / 8 / 12 / 26; forward 0.0; forward-left +37.9 against a
  twist of 36.5 -> 38.1. BF1 left roll - twist -31.2 / -0.9 / +21.8 / +51.6 / +73.8 / +84.9 / +89.9 (it starts from
  the strafe's own -71), every step 0.250 of the rest.
- The port (main c1ad907): `game::animator::body` -> `Special::Roll` -> the set's own `diveforward` pair by name
  (`Rig::body_slots`); the clips are baked already (52 of 52 set files name `human_rifle_diveforward`, 50
  `human_tool_diveforward`, 21 `human_sbdroid_diveforward`, 1 `sbdroid_rifle_diveforward`) - NO clip and NO bake
  change. No root twist exists: `soldier_vis.rs` `posed` draws at the soldier's position and aim yaw.
- The roll's direction is in the sim for every unit already: `Soldier::vel` against the facing. `soldier_vis.rs`
  (about line 598) has `horiz` and the aim frame for each unit, player and `ai` alike, where it fills `AnimInput`.

## Collisions
- ANIMM (4, todo) has the same two files and builds the strafe twist on the same field: whichever lands second
  rebases; the note is on its card. No `core/src/soldier.rs` edit here (BF1BK's file).
- The port has ONE roll for both games' maps (BF2's, any direction within 120 degrees of ahead - the maintainer's locked
  choice), so the BF2 rule (target = the travel angle) is used on BF1 maps too; for a pure side roll it equals BF1's
  90 degrees.

## Notes
- planner 2026-10-12 19:15: filed from the maintainer's note at value 5, alpha, M1 (a look). No deps. Lane 2 takes it by the dry run.
- lane 2, 2026-10-12 21:40 (RE session, report `reports/20261012_2145_card_ROLLS.md`): RE done, phase re -> build. The
  card's first question ("which clip per direction") has the answer "one clip"; the second ("is the body turned") "yes".
  Tools for the build's PS2 column: `lead_tmp/rolls/ps2/` (`roll_strip.py` records rows + video per direction in about
  5 minutes a game, `analyse.py`, `sheet.py`, `latency.py`).

## Original card text (planner 2026-10-12 19:15)
Why: the roll animation today is the FORWARD roll played while the soldier moves sideways. The PS2's sideways roll
keeps the body facing forward (the camera still points forward) and rolls sideways. Bots roll too (card REACT), so the
cure is on the animation side and one input for everybody - not a player-only patch.
What to do (RE): 1. the roll family in the animation banks per class set and which clip the builder puts in the roll
slot for which direction; 2. the body's yaw during a roll - read the code, then SEE it (a PCSX2 strip of the player
rolling forward, left, right and back in each game, and one bot's roll); 3. where `core` keeps the roll's direction.
If the PS2's picture is NOT what the maintainer remembers (the body does turn), put the strip in STATUS for him and build the
PS2's - the faithful default.
Acceptance (RE): the note in `analysis/` (the clips per class set and game, the slot rule, the yaw rule), the PS2 strip.
- lane 2, 2026-10-15 11:36 (build + land, report `reports/20261015_1136_card_ROLLS.md`): LANDED 662e152, no goldens, every gate PASS
  (ci +5 tests). `AnimInput::travel`, `Animator::twist` / `roll()` / `RollClock`, the turn on the drawn DummyRoot in
  `Animator::pose`, the roll lines in `ANIM_CENSUS`. Player strips `lead_tmp/rolls/sheet_port_bf1_tat1.png`,
  `sheet_port_bf2_kas2.png`; NO bot strip (the bot's input is pinned by `soldier_vis` test
  `a_side_roll_turns_the_drawn_body_onto_the_velocity` and the census lines). ANIMM: supply the strafe target in
  `Animator::step` where `target` is 0 outside a roll.
