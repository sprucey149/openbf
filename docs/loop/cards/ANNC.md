---
id: ANNC
title: The commander's voice-overs: CP captured/lost, winning/losing, reinforcements low, won/lost, heroes, out of bounds — triggers, lines per faction and era, cooldown and priority
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/announcer.rs,engine/crates/core/src/lib.rs,engine/crates/game/src/announce.rs,engine/crates/game/src/lib.rs,engine/crates/game/src/layout.rs,engine/crates/game/src/mission.rs,engine/crates/game/src/sounds.rs,engine/crates/game/src/walk.rs,engine/crates/game/src/main/battle.rs,engine/crates/game/src/main/windowed.rs,engine/crates/game/tests/announcer_ps2.rs,scripts/gen_missions.py,engine/missions,engine/CRATES.md,analysis/re-announcer.md
declares:
needs:
effort:
branch: annc
resume: landed feb04a84 2026-10-01
created: 2026-09-30
alpha: yes
domain: behaviour
---
## Why
The maintainer's INBOX 2026-09-30 11:40: the team's commander speaks about what is going on — CP captures, winning or losing,
heroes and so on. The port plays none of it.

## What to find
- The trigger set and the lines per faction and era: CP captured or lost (ours or theirs), team winning or losing,
  reinforcements low, battle won or lost, hero events, the out-of-bounds warning. Cooldowns and priority (a later line
  cannot cut off an earlier one), and any subtitle / HUD text.
- Leads: the mission scripts name lines (`cis_off_com_report_enemy_losing_generic`,
  `cis_off_com_report_us_overwhelmed_generic` in `extracted/bf1/lua/mission/*.luac`; `SetOutOfBoundsVoiceOver`,
  `SetHeroClass`, `REP_bonus_*`), `analysis/re-sound-global.md`. MSND (mission data) and SNDC (the audio census) overlap:
  merge what overlaps (notes on both).

## RE result (2026-09-30, lane 2) — `analysis/re-announcer.md` (657ebe7), verifier 32 / 5 / 0 of 37
The table of every trigger is the doc's §0 (T1-T9), each [V] with lines, 14 of the events also measured live on the PS2.
In short: no announcer object — the post tick, the VoiceOverManager, the out-of-bounds timer and BF2's script ask the
sound handle for a 2-D stream line with the QUEUE flag. One voice slot, one line at a time, at most 4 pending, ordered
by key: a commander line goes ahead of pending chatter and never cuts a line off. "Cooldown" on the PS2 is: the 30 s
repeat of the bleeding line (own team only), the one-per-battle low-reinforcement latch, and the queue.

## Build result (2026-09-30 22:30, lane 1) — branch `annc` 703b1db, ci green, NOT gated (STOP was up)
Report `reports/20260930_2230_card_ANNC.md`. Acceptance 1-5: 5 unit tests pass. PS2 pair (UTA1 CW as CIS): 7 lines,
every name equal, onsets 0.00 s vs the PS2's 0.03 s, the queued line 2.80 s vs 2.84 s, repeat 30.03 s vs 30.06 s.
Data: 57 mission RONs, 0 differences from the bytecode census; 2,932 post VO properties over 32 maps, 0 maps differ.
`declares:` is empty — the bake always kept the posts' `VO_*` properties, so there is no rebake.
Not heard yet: no windowed run. Not built: T6 (BNDY), T7, T8, BF1's destroyable posts' lines (45 properties).

## Landed (2026-10-01 18:25, lane 1) — main feb04a8, report `reports/20261001_1830_resume_ANNC.md`
Every gate PASS on branch and main, 0 goldens moved, ci 1054 → 1063 (the new tests). Acceptance re-run on the rebased
branch: 5 rule tests, the PS2 pair (7 lines, names equal, onsets within 0.04 s), 57 RONs / 0 differ, 32 maps / 0 differ.
Open: the lines are not HEARD yet (no windowed run in the loop — the maintainer's check). `a_sim_battle_speaks_through_the_frame_hook`
now takes ~177 s in debug (6,473 ticks since HOLD) — shorten it when the file is next touched.

## Notes
- **AUDIT 2026-10-02: PARTLY** (`reports/20261002_0135_audit_ANNC.md`, evidence `engine/artifacts/lead_tmp/audit-annc/README.md`). Held: every rule against both ELFs, the 57 mission voice blocks (my own Lua 4.0 / 5.0 readers), the 2,932 post rows (my own raw disc scan), the tests and the PS2 pair. Not held: the feed — wrong commander when the two factions are swapped in the shell, and on BF2 maps no bleeding line in the last stand (77 missing in 23 of 31 battles; live PS2 run `lead_tmp/audit-annc/live1.csv`) -> card ANNC2. `common_objComplete` (T8) is not a conquest line; T7 and the destroyable posts' lines moved to ANNC2's notes.
