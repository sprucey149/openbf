---
id: TCMP
title: Tiers as competence: Easy decides slowly and fights plainly, Elite uses cover, flanks, priorities and grenades - one column per layer, never damage or sight through walls (bot design section 7)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer, 2026-10-02: "better difficulty levels"; the sketch: tiers scale chances, not competence - "Easy is slow and plain, Elite uses everything; never by cheating on damage or sight through walls". `analysis/bot-design.md` section 7 (D15) has the table; the PS2's difficulty is one skill number per team and three mode weights (`re-soldier-ai.md` 8, `re-ai-difficulty.md` 0).

## What to build (after card TIER has measured today's tiers)
The `skill::Tier` columns of bot-design.md section 7, each read by ONE layer: the decision interval (Behaviour, Tactician: 1.0 / 0.5 / 0.33 / 0.33 s); the trigger cone (6 / 3 / 2 / 2 degrees) and bursts beyond 40 m (Gunner); the cover chance per opportunity (25 / 50 / 75 / 90 %), fire from the hidden stance, leaving a compromised position (Tactician); the evasive step when aimed at and the roll when hit (Behaviour: Evade); grenades by situation at Hard and Elite (with card GRAI); target priorities (perception); memory of a lost target and the walk to its last point (Investigate); contacts passed on, the Flank role, cover for a capturer (Team board); fall back when hurt (D10); bots that may fire at one human at once (2 / 4 / no limit / no limit). The PS2's skill curves stay as they are.

## Acceptance (run and paste)
- Card TIER's table re-taken over 8 seeds: kills a minute and hit rate ordered Easy < Normal < Hard <= Elite with each neighbouring gap beyond the seed noise; `arena_tat2`'s `monotonic` line judged on the 8-seed mean (the gate-script change is its own card if needed).
- One unit test per column (the column changes the behaviour it names; mutation-checked).
- No tier reads a position its bot or its team has not perceived: a test with an enemy behind a wall at every tier.
- Sweep 8 seeds x 32 maps at Normal: 256 / 256, bars B1-B14 not worse. Declared: `sim; sweep; arena`.
- Elite uses cover (planner 2026-10-16, moved here from TPOS): strict B12 (fight time AT a position with cover from the current target) at Elite >= the lower of 20 % and half the map's ceiling on the six showcase maps, 8 seeds; TPOS's `fightsplit` line names where the rest goes (Mygeeto / Utapau at Normal: open positions 18-22 %, walks 16-17 %, own fight after a none 13-20 %).

## Notes
- Cut from `analysis/bot-design.md` (card BOTD, 2026-10-03). The maintainer agreed on 2026-10-03 (decision 2, "BOTD read, go"): **the damage bonus against a human (x1.15 Hard, x1.3 Elite, ours) is dropped**; PLAN's locked "per-HUMAN pressure" stays as the target preference and the fire-slot limit.
- Halo removed its reaction pause because players read it as a bug (bot-design.md 15, X1): keep the first-shot delay short; the feel is the maintainer's - one line per choice in `FEEL_NOTES.md`.
- 2026-10-05 (the maintainer, via Claude): build the tier as a PER-TEAM value, with the shooting columns separable from the behaviour columns - DIFB sets a team's tier from the difficulty and the humans on each side.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).** Decision 2 is settled: **Hard and Elite lose the damage bonus against a human (x1.15 / x1.3 go)** - tiers differ by competence only. Decision 8: one think rate for every bot, no slower thinking away from the player.
- 2026-10-05 planner: value 4 -> 5: every card BOTV waits on is ranked with the chain, so the bot programme runs in the maintainer's order ahead of the fours (his notes of 2026-10-02 16:30 / 17:40 and 2026-10-03 14:08). Undo: `loopctl.py set TCMP value=4`. `effort: high` (the builds the maintainer's top concern rests on; a build runs at medium by default). Undo: `loopctl.py set TCMP effort=`.

- 2026-10-07 04:34 (GRAI landed 79f1890): the grenade gate is `core/src/ai/arsenal.rs` `grenade_forced` (window / group / BF2's roll with `Skill::grenade_chance`), the pick `choose`, the claim `ArmState::claim`. The tiers' grenades by situation (a covered target, never at Easy) go in that gate. The score there is PROVISIONAL (no verifier pass; BF1's function not opened).
- 2026-10-07 08:50 (AUDIT FGHT): the evasive step's PS2 side differs by game, and card FGHT built BF2-Normal's stand /
  circle shares for both. **BF1's style draw has no open-ground gate** (`FUN_00238b38`, B1:152006-152117): by n % 6 and
  the enemy team's difficulty number (`DAT_005875f4` against 14 and 17, ELF 0x517dd0 / 0x517dd4) zig-zag + flank are 2 of
  6 bots below 14 and from 14 to 16, and 4 of 6 from 17 (zig-zag 3 of 6). BF2 gates them on the free space at
  controller +0x60 (over 10 m / over 5 m; it is 2.5 m on 88-99 % of combat rows), and on **Elite its weights are
  {1, 1, 5, 1}**: 5 of 8 bots zig-zag where there is room (`FUN_002aba38`, B2:231159-231176). FGHT's "2-5 % of attack
  time" is BF2 Kamino at Normal (flank 5.2 %, zig-zag 1.9 % of on-foot attack rows; Polis Massa 0); no BF1 capture has
  the pattern column - measure BF1's shares (the attack helper's pattern in a `bf1_kas2` / `bf1_nab1` capture) before
  setting the BF1 column of the evasive step. Evidence `engine/artifacts/lead_tmp/audit-fght/verify/VERDICT.md` D4, D7.
- 2026-10-07 (the maintainer, via Claude): same as TIER - card REACT's per-hit reaction is a competence column.
- AUDIT GRAI 2026-10-07, the PS2's grenade rate for the tier's grenades (`analysis/re-grenades.md` 7.5, `lead_tmp/audit-grai/ps2/`): BF2 GCW conquest at the default difficulty, a grenade episode (the AI's firing channel 0 -> 1) per 146 / 239 bot-seconds on Kamino and 347 / 363 on Polis Massa, median episode 2.5-5.0 s (the 3 s window); with an enemy soldier 8-24 m away the PS2's bots are on the grenade channel 7-10 % (Kamino) and 3-4 % (Polis Massa) of the time, the port's 4.1 % and 2.3 %. On main every one of the port's 8,392 throws in 256 rounds is a FORCED one (the rule alone throws 1).
- AUDIT FGHTA 2026-10-09 13:40: BF1's draw has ONE range flag - a drawn "stand" stands wherever the target is in range and rings only out of range (`FUN_00238b38`, B1:152006-152117; verifier C8 of `lead_tmp/audit-fghta/wf/RESULTS.md`); BF2's "2 becomes 3 outside the middle band" is BF2's alone. Card FGHTB step 3 builds it; BF1's assassin droid is `UnitType trooper` and takes this draw, not the scout rule (re-soldier-ai 16.1).

## Card TIER's measurement (2026-10-19, main 7752d00; TIER.md RESULT, 24 seeds, `lead_tmp/tier/`)
Today: easy < normal is clear; normal vs hard is inside the noise on tat2 (+0.57 kills/min, t +0.9); hard vs elite is REVERSED on the flat field (-16.7 kills/min, t -7.3). Hard and Elite share base skill 20, so every skill-curve field is equal; only `Tier` differs. Elite knock-outs show its REACT weights and view 1.5 / reaction 5 lower the same-tier fight's kills (Elite is harder to hit, and the arena puts Elite on BOTH teams). Fields that would separate the tiers as COMPETENCE: Hard vs Elite need a skill-curve difference (today none: give Hard a base below 20, e.g. the PS2's 17 for its top, or let Elite's err floor drop) and Elite's decision layers (cover, flank, priority); Normal vs Hard: base 17 -> 20 is too small a step (t 0.842 -> 1.0) - the PS2's own bases are 4 / 14 / 17. Before judging this card's "Easy < Normal < Hard <= Elite on kills a minute" acceptance: a same-tier arena reverses whenever a tier gets harder to hit, so judge it head-to-head (card ARVS, proposed as a dep - the planner's call).
- planner 2026-10-19 16:25: deps += ARVS (the head-to-head arena and per-team tier): judge each tier against the one below on the other team, never by a same-tier fight's kills (TIER's measurement, RULES).
- planner 2026-10-20 05:20: `deps += TPOSR` (the Tactician's reservation fix first; see TPOSR's note). Undo:
  `loopctl.py set TCMP deps=BOTD,TIER,TPOS,GRAI,ARVS`. ARVS (parked for land) gives the head-to-head referee:
  `arena 9000 <seed> tat2 vs all 24` / `flat` - judge TCMP's tiers by its `beats:` lines (ARVS report 20261020_0508).
- AUDIT ARVS 2026-10-20 (`lead_tmp/audit-arvs/EVIDENCE.md`): on today's main the head-to-head says Hard BEATS Elite on the flat field -
  elite>hard 0.466 t -1.72 (seeds 1-24), 0.475 t -2.22 (25-96), 0.484 t -1.42 (97-168), pooled 168 seeds 0.478 t -3.04; per-seed sd 0.096 against
  an effect of 0.022, so a 24-seed flat block cannot settle this pair (~146 seeds for 80 % power) - judge flat on 144+ seeds or on tat2 alone.
  On tat2 Elite wins on kill share only (0.517 t +8.7 over 96 seeds) and NOT on tickets (margin +1.3, t +0.5 over seeds 25-96). The elite>hard
  tat2 `yes` rests on three readers no test covers (the think tier, Elite's err cap, the sniper lane): card ARVSA puts tests under them first.
  With the think's tier on the round's (Hard) for both sides (mutation R1), flat elite>hard reads `yes` 0.534 t +2.39: the tier columns the
  think reads (REACT, view, reaction, grenade) are what lose Elite the open-field fight, its err cap alone wins it.
  (audit 2026-10-20: `deps += ARVSA` so the referee has tests before TCMP is judged on it; undo `loopctl.py set TCMP deps=BOTD,TIER,TPOS,GRAI,ARVS,TPOSR`.)

- planner 2026-10-20 14:20: TPOSR landed (28d42d4) and left one bar open for this card: the maintainer's Kamino scene
  (`lead_tmp/tpos/tools/scene_seeds.sh`) "0 pinballers over 6" was met by neither build (TPOSR branch 1, main 2 on seeds
  60-67; 0 / 1 on eight more), and the cover share is still above the PS2's (Kamino / Polis Massa 47.1 / 68.9 % of fight time
  against 22.1 / 48.4 %; `reports/20261020_1020_card_TPOSR.md`). Re-run the scene on TCMP's branch; ARVSA (the referee's fixes)
  lands first.
