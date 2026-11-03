---
id: UCALL
title: BF1's combat calls (the maintainer's INBOX 2026-10-12 18:55, ask 3; first slice of USND): a bot says its class's acquired line when its tracker takes a target, the first bot within 7.5 m of a live grenade says the flee line (once a second), a bot turned by a heard enemy says the heard line - through the voice queue, thinned as on the PS2 by the property's interval (20 +- 5 s; the Rebels and the Empire 1 to 10 s) and by the take's own lock, which the bake must start to carry; hide / approach / predamage are dead keys and stay silent; RE done 2026-10-13 (analysis/re-unit-chatter.md 1, 2); refereed per event against the PS2's NAB1 and TAT1 rows
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX 2026-10-12 18:55, ask 3 (umbrella card USND, slice 1 of his order): the calls units make in a fight.
On the PS2's BF1 the voice slot is never quiet - 32 lines a minute wherever the player stands; the port says none of
them (`analysis/re-unit-chatter.md` 1, 2, 5 `[O]`).

## Evidence (BF1's rules; they run on every BF1-rule map)
- **Three calls exist**, each the soldier class's own key, played 3-D at the unit on the battle's voice stream
  through the queue of `re-announcer.md` 1:
  - `AcquiredTargetSound`: whenever the AI's target tracker takes a target (first target, the old one invalid, a
    better candidate, the kept one timed out) - no "is it new" test;
  - `FleeSound`: per live grenade-class ordnance (grenade, thermal, haywire, beacon, sticky), after a reaction delay
    (uniform 0.6-1.3 s at the default difficulty) and at most once a second: every AI unit within 7.5 m is warned
    and the FIRST AI soldier among them says the line. A human's soldier never does;
  - `HeardEnemySound`: when the AI's hearing check (the fourth of its interrupt checks) turns the unit toward a
    heard enemy inside its range.
- **Three keys are dead**: `HidingSound`, `ApproachingTargetSound`, `PreparingForDamageSound` are parsed and never
  called (their takes are heard only because `flee` shares them). Do not play them.
- **What thins them** (the port's bank already applies the first, `game/src/sounds.rs` 1153-1162):
  - the PROPERTY's gate, one per battle and sub-level: `cis_` / `rep_` / Gungan 20 +- 5 s; `all_` / `imp_`
    acquired and approach 10 +- 5, flee 1 +- 0, heard 1 +- 5 (`imp_` 10 +- 5); the interval is re-armed BEFORE
    the probability is rolled;
  - the TAKE's own lock, which the bake does not carry: BF1 draws one take and a locked draw silences the request
    (`rep_` acquired 25 +- 5 s a take, `rep_` flee 5 s, heard 15 s; the Gungans' and the `ai_` lines 1 s; the
    stream path draws the take before the property gate). `lead_tmp/usnd/bf1_snd_all.tsv` has every property;
  - the voice queue (one line at a time, four pending, by 3-D level; 16-150 m).
- **PS2 rates** (`lead_tmp/usnd/live_bf1.txt`, per minute, the player parked at his post / beside a front-line
  team-mate): gate passes - acquired NAB1 7.75 / 7.5, TAT1 5.0 / 5.25; heard 5.5 / 5.5, 3.75 / 3.75; flee 5.25 /
  4.0, 3.25 / 2.5. Lines started: acquired 3.5-4.5, heard 2-3.75, flee 1.75-2.75.

## Notes
- The take lock is a bake change (`rebake` of the sound groups): the segment's third and fourth values
  (`lead_tmp/sqack/segs.py`, `lead_tmp/usnd/tools/sndcensus.py`). SQACK's hand-written 1 s / 2 s locks for the
  order's "yes" then come from the bake.
- `ChargeSound` (three AI soldiers within 7 m) is called and authored by no soldier class: nothing to play.
- BF2 maps use `VOSound` instead: card UVO2.
