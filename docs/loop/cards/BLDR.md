---
id: BLDR
title: Bots build and repair buildings: engineers, pilots and Jawas (UnitType repair / support / special / pilot, with a cutter) look every 5 s within 75 m for an unbuilt, destroyed or damaged building, one bot per team per building, walk to 1.6 m and hold the cutter - the PS2's rule with two named deviations (no dead zone under 1 m, no building from afar)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
On the PS2 the build pads become turrets because bots build them: Jawas on Mos Eisley, engineers on Yavin, the pilots
on BF1's maps (129 gun turrets on 12 BF1 maps, 48 pads on 6 BF2 maps). With TBLD (the turret's states) and FCUT (the
cutter) in, only a human can build; this card gives the bots the PS2's rule. Showcase maps with buildable turrets:
BF1 NAB1 21, TAT1 12, YAV2 6 (they need BZONE1 after this card).

## Evidence (`analysis/re-build-zones.md` 3 and 5, [O]; verifier 33 / 4 / 0)
- Who: a bot whose class `UnitType` is repair / support / special or pilot and that carries a cutter with a positive
  amount for the target's type. BF2: the engineer classes (`UnitType pilot`), `tat_inf_jawa` (support). BF1: the
  pilot classes. A class with a cutter and no `UnitType` (BF2 `wok_inf_mechanic`, two Gungans) never builds.
- When: every 5 s, among friendly or neutral things within 75 m that lack health the tool can give - a building that is
  unbuilt or destroyed qualifies - at rest, nearest first; one claim per team per building (a claim lasts 10 s,
  renewed each think). BF1 skips the search for a team under its low-ticket gate (26 reinforcements).
- How: walk at the building and stop 1.6 m out; in position (inside the tool's reach) stop and hold the trigger;
  give up after 20 s out of position with the walk idle or failed, or after 8 s with no health gained. A fight takes
  the bot away (the order changes); it looks again 5 s later.
- PS2 curves (10 Hz live samples, the mission clock of a running battle): BF2 Mos Eisley CW built 3 of 11 at 55 s, 5
  at 130, 6 at 160, 7 at 205, 8 at 235, 9 at 250, 10 at 265; BF1 Naboo Plains 7 of 21 at 0, 12 at 45, 8 at 75, 13 at
  120, 17 at 165, 20 at 225; BF2 Yavin: fallen pads rebuilt 65-98 s later, three far pads never in 270 s. Builders
  stand 1.5-1.7 m from the pad's centre (BF1 1.1-4.5 m).

## Named deviations (FEEL_NOTES when built; the PS2 annoys here)
- **No dead zone under 1 m** (BF1's rule on both games): BF2's bots that reach the pad's centre stand 20 s and build
  nothing (22 of 79 spells on Mos Eisley; only 17 ended at a built pad).
- **No building from afar**: BF2's AI gives the weapon its target with no range test, so a bot still holding the
  trigger builds its next pad from up to 59 m (28 of 120 steps). The port's bot fires only in reach.
Both make the port's bots build sooner than BF2's; report the curve beside the PS2's.

## Acceptance
- `round_to_outcome` prints a `turrets built` line (built / buildable at 60, 120 and 240 s). Gate: at least 1 pad
  built within 60 s on BF2 `tat2` CW and `yav1` CW; the curve pasted beside the PS2's above. After BZONE1: BF1 `nab1`.
- Headless scene tests, each mutation-checked: an engineer bot 40 m from a pad walks up, stops 1.2-2.5 m out and the
  pad is built 7.5 s after its first shot; of two same-team engineers only one goes; a bot of `UnitType` trooper with
  a cutter does not go; a bot whose pad cannot gain health gives up after 8 s; no bot ever serves a soldier (this card
  serves buildings only).
- Sweep: rounds finished unchanged (32 of 32); kills a minute within the seed noise on the six BF2 pad maps.

## Notes
- **Planner: this is a bot behaviour ahead of the maintainer's read of BOTD.** Default taken by the RE session: not held - it is
  the PS2's own rule (bot-design row "engineers, pilots: MATCH"), one helper module `core/src/ai/serve.rs` with one
  hook in the think, no change to the think's structure; SPL11 moves it with the rest. To hold it: add BOTD to `deps`.
- Card ENGR keeps the rest of the serve behaviour (vehicles, the human, dispensers, mines); build `serve.rs` so ENGR
  only adds target kinds.
- BF2's bots near the player think every 0.25 s and stop in the 1-5 m ring; the far ones overshoot. The port's think
  rate is one for all, so the first deviation costs nothing.
- 2026-10-05 planner: `deps` gains SPL11: the builder's hook goes into `ai/brain.rs`, which SPL11 moves apart; BOTD is closed, so the report's 'not held behind BOTD' call is moot. Undo: `loopctl.py set BLDR deps=TBLD,FCUT`.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.
