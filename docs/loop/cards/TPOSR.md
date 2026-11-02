---
id: TPOSR
title: The Tactician's reservation and hide / fire cycle run only on thinks with the target seen, where the PS2's cover state machine runs every update: 20 % of held time unreserved, two bots on positions within 2 m in 175 of 256 rounds (4,024 pair-ticks on the same position), a position hit twice kept 22 s, hide phases over 3 s late in 11 % of cover time - and tests for the 15 rules no test referees (AUDIT TPOS)
milestone: M3
value: 5
phase: build
status: done
deps: TPOS
files: engine/crates/core/src/ai/behave.rs, engine/crates/core/src/ai/tactic.rs, engine/crates/core/src/ai/tactic/tests.rs, engine/crates/core/src/ai/state.rs, engine/crates/core/src/perception.rs, engine/CRATES.md
declares: sim; sweep; arena
needs:
effort: high
branch: tposr
resume: landed 28d42d46 2026-10-20; audit INCOMPLETE/UNAUDITED 2026-10-25: restore held inputs and verified history; see private note
created: 2026-10-19
alpha: yes
domain: ai
---

## Why
AUDIT TPOS (2026-10-19, `engine/artifacts/loop/audits.md`; evidence `lead_tmp/audit-tpos/EVIDENCE.md`). The Tactician's
position is renewed, cycled and judged only inside `tactic::fight`, which `fight_position` calls only when the target is
SEEN on this think. A bot in COVER whose target is only remembered keeps its position through behave.rs's
`if st.fight.mode == FightState::COVER && memory.is_some() && free { h.until = ... }` and nothing else runs. Hearing
re-stamps the contact, so this can last as long as the enemy shoots within earshot. While it lasts:
- `TposState::until` is not renewed, so its reservation lapses 2 s later. Another bot's `choose` (`until > sim.tick`)
  then takes the same position or one within 2 m, and nothing re-checks spacing once both hold.
- `cycle` does not run, so the hide phase never ends: the bot stays crouched.
- The leave tests do not run either: hit twice, the target round its cover, out of band, the 5-12 s timer. bf2_tan1
  s4, unit 3 sat at pos #748 with "covered hidden, hits 2, phase 0.0 s" for 22 s with enemies 11-32 m away
  (`lead_tmp/audit-tpos/wf/C3/bb/`, hash = the sweep's).
The PS2 does none of this. Its cover state machine (`FUN_002d0c78`, vtable 0x623c08) re-stamps the hint's reservation on
every update in states 2-5 (B2:257399 / 257634 / 257667 / 257695 / 257740, `FUN_002ded48` -> `FUN_002adf10`), whether
or not the target is seen. Its hide / fire timers run in that update. The walk and hidden states set the target
tracker's keep flag (`FUN_002ce4e8`: tracker+0x38 = FLT_MAX), so a hidden bot never forgets its target. The fire
request (`FUN_002ce108`) does not read the cover state. Two verifiers agreed on all of this
(`lead_tmp/audit-tpos/wf/results.json`, V1 / V2).

## Evidence
Numbers on the landing's code: 32 maps x 8 seeds, a per-tick observer, a switch build proved equal to the landing on
256/256 rounds (`lead_tmp/audit-tpos/patches/landsw.patch`, `tools/obs.py`):
- Reservations: 20.1 % of held unit-ticks lapsed, 97.8 % of them in COVER. Two held positions within 2 m: 52,596
  pair-ticks in 175 of 256 rounds, 4,024 of them on the SAME position, 94 % teammates. Today's main b19cdf9 reads the
  same (20.7 %, 45,005).
- Cycle: 11.4 % of covered-COVER time is more than 3 s past its hide / fire swap. 28.7 % of hidden spells run past
  HIDE's 4 s, the longest 94 s. The firing phase is 20.8 % of covered-COVER time.
- Prototypes (switch bits in the same binary):
  - Renewal in the memory branch (`st.tpos.until = max(until, now + FREE_AFTER)`): lapsed 20.1 -> 1.44 %, pairs 52,596
    -> 494, kills a minute x0.996, 256/256.
  - `cycle` on memory thinks: over 3 s late 11.4 -> 0.04 %, firing phase 20.8 -> 34.7 %, BUT firing-and-seeing only
    10.6 -> 11.7 %, kills a minute x1.001. Standing up does not restore sight in 66 % of firing ticks.
  - Both: 255/256. bf1_nab2 s8 ended NO OUTCOME, 1 v 1 at tickets 1/1, not traced: trace it if the built version
    shows it.
- Mutations of the landed rules (`lead_tmp/audit-tpos/tools/mk_mut.py`, `mut/`): 15 of 34 survive every test.
  - M2 / M29: the 2 s reservation of a position just left is ignored.
  - M5: the Cover hint's side test is inverted. No test passes a `Source::Cover` position through `choose`.
  - M6: the tier's move share on a single hit.
  - M10: a CORNER position leans to one side only.
  - M12 / M13 / M14: the backward, friend's-line and grenade filters are off.
  - M16: the band's inner edge grows the wrong way.
  - M18: the confirming ray always passes. It rejects 106,358 candidates in the land sweep, and the crate test's "with a
    line to it" cannot fail.
  - M22: MOVE_MIN at half.
  - M26: `covered_from_target`'s radius (B12's instrument). No test calls it.
  - M28: CLIMB x3.
  - M30: the walk deadline's slack x4.
  - M33: a Tactician's bot steps out of a friend's line.
  The agents' smallest killing scenes are in `wf/results.json` (`missing_scene`).

## Notes
- The PS2 is not stricter everywhere. A normal end (cover state 100) or an invalid re-score frees the hint AT ONCE
  (`FUN_002adf50`, stamp -10). Only the 5-12 s swap and an outer abort leave it reserved 2 s. The port's FREE_AFTER on
  every leave is stricter: keep it, or match it and say which.
- Not this card's bar, for the planner: the port's cover time is not like the PS2's.
  - Share: 47.1 / 68.9 % of fight time on Kamino / Polis Massa, against the PS2's 22.1 / 48.4 % from clean rows.
  - Spell length: median 3.9 / 3.9 s against the PS2's 1.1 / 1.9 s, outside the PS2's 95 % CI.
  - Why the PS2's spells are short: they end mostly on outer restarts (target change, no target, beyond range + 10;
    `FUN_002cf458`, V7).
  - The 2026-10-16 17:00 ruling read the PS2's share as a floor.
- For `analysis/re-soldier-ai.md` 4.6, from the verifiers:
  - Spell counts: the strict spells are 52 / 36, not 46 / 28. The medians hold.
  - The cover draw also runs from modes 6 (x0.25) and 9.
  - |H - T| is 3D.
  - BF1 differs: its another-hint timer is randint(16, 25) s, its step-out is 3 m, it takes no coin flip
    (B1 `FUN_0024df78` / `FUN_0024e058`). The port uses BF2's 5-12 s on both games.
- Files overlap card TEAM (in progress on 2026-10-19). Build on top of it once it lands.
- planner 2026-10-20 05:20: value 4 -> 5, and TCMP now `deps += TPOSR`. TPOS's audit (PARTLY) found the reservation lapse;
  TCMP gives Elite the cover the Tactician runs, so the fix goes in before TCMP builds on it (both edit behave.rs).
  Undo: `loopctl.py set TPOSR value=4`; `loopctl.py set TCMP deps=BOTD,TIER,TPOS,GRAI,ARVS`.
