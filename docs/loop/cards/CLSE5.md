---
id: CLSE5
title: The far fire that is left after CLSE4's two rules: snipers deal a third of Kamino's far damage excess (the PS2's stand at snipe hints and hardly shoot - design D13's cost), a shotgun blast deals 39 hp beyond 30 m against the PS2's 16, the port holds a target 3-4 x as long as the PS2 on Polis Massa, and inside 20 m the port deals 0.3-0.6 x the PS2's damage (a PS2 bot fires outside a fight order and on the move) - measure each, put D13 to the maintainer, build the PS2 rules
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card CLSE4's measurement (2026-10-12, `analysis/re-soldier-ai.md` 20) split the port's far fire against the PS2 by
weapon and found four causes; CLSE4 builds the two that are PS2 rules already read (the trigger's `MaxRange` test, the
Barrage keys). With both in a prototype the bars CLSE4 was filed with are still not met: deaths with an enemy inside
10 m 28.5 % of the 17 BF2 maps' (the bar 30; before CLSE 30.3), Polis Massa beyond 30 m x 2.44 the PS2's damage per
aimer-second (the bar 0.67-1.5), and the bins inside 20 m fall further UNDER the PS2 (Kamino 0-10 m x 0.68 -> 0.55,
Polis Massa x 0.36 -> 0.31). This card is the rest.

## Evidence (`engine/artifacts/lead_tmp/clse4/`: README.md, `out/table_*.txt`, `budget_*.txt`, `ps2_holder_kam1.txt`)
- **Snipers**: 34 % of Kamino's far damage excess. On the PS2 a sniper rifle is in hand 21.5 % of the AI foot time
  and its holder aims at a soldier 0.4 % of that (99 % in the snipe stand, standing still, 23 shots in 3,051
  unit-seconds); the port's is an aimer 10.2 % of its time and deals 29.6 of the 146 hp per 100 AI foot-seconds
  dealt beyond 30 m (the PS2 0 of 58.5). The port's short stand is bot design D13 (20-40 s against the PS2's
  60-120 s, the maintainer's annoyance 12) - a named deviation, so this part is a QUESTION for the maintainer, with the number.
- **Shotguns**: 19 %. A blast beyond 30 m is credited 39 hp in the port and 16 on the PS2 (20 blasts), at 20-30 m
  70 against 42 (27 blasts) - thin on the PS2 side; pulls a second 0.57 against 0.36. Candidates: the pellets' spread
  for a bot (card SPRD's tight hold is OURS; the CLSE audit's `AUDIT_AI_HOLD=0` took a tenth to a fifth off the far
  fire), the falloff, the cross-credit of the method.
- **Polis Massa's target time**: the port's aimer-seconds per 100 AI foot-seconds are 9.5 / 7.2 at 10-20 / 20-30 m
  against the PS2's 3.1 / 1.7, a quarter of it on a target not seen; the far aimer is then credited with a near
  aimer's falls (own hits beyond 30 m 15.5 hp per aimer-second against 20.4 credited; the PS2 7.4).
- **Inside 20 m the port deals less than the PS2**: per 100 AI foot-seconds Kamino 0-10 m 130 | 42, 10-20 m 108 | 64;
  Polis Massa 0-10 m 277 | 140. A PS2 rifle bot fires as much outside a fight order as in one (1.5-1.9 shots a
  second; 86 % of that firing is on the move), the port's fires only in Fight (0.01-0.04 shots a second outside it)
  and its fights are 85-93 % of the aimer time against the PS2's 56 %.

## What to do
1. After CLSE4's build: re-take `clse4/out/table_*` and `budget_*` on that main (tools in the README).
2. Two more PS2 captures a map for the shotgun's and the sniper's rows (`clse/tools/run_w.sh`, 4 minutes for both).
3. Each candidate behind its own switch in one binary (null = main byte for byte): fire on the move at a seen
   target outside Fight (re-soldier-ai 4.9: "firing and moving are independent"); the shotgun's spread / hold for a
   bot; dropping an unseen target sooner; the snipe stand at the PS2's 60-120 s (measure only - D13 is the maintainer's).
4. Build what is a PS2 rule in `analysis/`; put D13 to the maintainer under STATUS "Questions for the maintainer" with the two numbers.

## Acceptance
- A table per candidate (the 17 BF2 maps x 8 seeds, per-seed paired t): deaths with an enemy inside 10 m, contact20,
  kills a minute, team kills, and the damage per 100 AI foot-seconds in the four range bins on Kamino and Polis
  Massa beside the PS2's.
- Then a landed build with deaths with an enemy inside 10 m at 30 % or more on the 17 BF2 maps (Kamino 27 or more),
  Polis Massa beyond 30 m at 0.67-1.5 x the PS2's by the aimer's OWN hits, no bin inside 20 m farther from the PS2
  than on its parent, 256 of 256 rounds - or the numbers that say why not.

## Notes
- 2026-10-12 filed by card CLSE4's measure session (lane 2; report `reports/20261012_1620_card_CLSE4.md`). Planner:
  CLSE2B (`deps: CLSE2,CLSE4`) wants the far fire's fix; with CLSE4's two rules on the 14 BF1 maps contact rises
  10.75 -> 11.93 % (t +3.2), so CLSE2B can be re-measured right after CLSE4 lands, not after this card.
- 2026-10-12 17:05 planner: the sniper part's question is partly answered by the maintainer the same afternoon (INBOX 16:20, card
  SNPR): he wants a sniper that picks a spot for its lane, holds it while it pays and relocates after 20 quiet seconds
  - neither the PS2's 60-120 s statue nor today's rifleman with a scope. So this card MEASURES the PS2's stand (step 3,
  the switch) and does not build it; SNPR builds his rule. Re-take the far-fire budget after SNPR lands, not before.

- 2026-10-12 18:49 from CLSE4's landing (main a490579; `lead_tmp/clse4/out2/`, re-soldier-ai 20.5): the built rules read, on
  main 2e12961 -> a490579 over 8 seeds: Kamino hp per aimer-second against the PS2 at 0-10 / 10-20 / 20-30 / 30+ m
  x 0.62 / 1.04 / 1.06 / 1.67 -> x 0.60 / 0.98 / 0.78 / 1.23; Polis Massa x 0.33 / 0.36 / 0.68 / 1.85 -> x 0.33 /
  0.33 / 0.67 / 2.66 (its far bin's seed ranges 5.4-17.0 and 14.7-26.0 hp: take more seeds before reading it);
  deaths with an enemy inside 10 m 29.1 % on the 17 BF2 maps (the 30 % bar is 0.9 points off), Kamino 21.7 %.
  **New here:** team kills rose in both games (BF2 1.24 -> 1.47 % of kills, t +3.5; BF1 0.72 -> 0.91 %, t +3.2) -
  find where (a longer life near friends, or the first shot of a burst after a 1.7 s pause with a friend stepped in).
  Two PS2 rules of 20.3 are still NOT in the port and both ADD fire where the port is under the PS2 (inside 20 m):
  a bot that stops wanting to fire owes no pause (the next burst starts at once), and the burst's start asks the
  weapon's meter. Start with the first: it is one line in `gunner_foot::fire_discipline` behind a switch.

- 2026-10-12 20:40 from the audit of CLSE2 (`lead_tmp/audit-clse2/`, re-soldier-ai 21): three measured things for
  this card. (1) CLSE2's five rules ADD to the far fire: damage per aimer-second beyond 30 m over the 17 BF2-rule
  maps 14.67 -> 15.51 with the rules off -> on, on main bc10b82 (t +4.7, 8 of 8 seeds), and cost 0.5 points of
  contact within 20 m and 1.0 point of deaths inside 10 m (`sw/k1json/`, `tools/summ.py mainsw_0 mainsw_31`).
  (2) The skill level is NOT the far fire's cause: in a live PS2 Normal battle the error blocks run at level 14
  (the human's side as targets: floor 0.779, cap 1.516) and 11 (the other side: 0.968 / 1.674), read off 10,198
  thinks; the port runs every block at 17 (0.589 / 1.358). The port with the PS2's two levels in its blocks
  (`AUDIT_LVL=14,11`, `tools/lvl_patch.py`; 17,17 = main hash for hash) deals 15.51 -> 14.55 beyond 30 m (t -3.6)
  and 29.1 -> 30.1 % deaths inside 10 m (t +2.4): 6 % of a x 1.2-2.8 gap. (3) The PS2's bots on Kamino never
  sprint and never lie prone (0 of 86,652 unit-updates on foot; stand 87.6 %, crouch 11.3 %); most of their blocks
  think every 3rd update, not every 5th. Tools for the next PS2 read: `audit-clse2/ps2/block_live.py` (one row a
  game update per member: the block, the stance word, velocity, class) and `block_replay.py`.

## Note (planner 2026-10-12 23:40): a fighter that twitches in place (the maintainer's Kamino scene, after CLSE4)
- The maintainer's scene went 4 -> 8 trails over 6 reversals with CLSE4 (table on card SKIRT). The new ones are bots in
  `does fight` holding a position at hub 46 against the parked player 6-12 m off: #18 at t663000 (11 reversals in
  2.3 x 2.4 m), #20 at t653000 (11 in 3.4 x 2.5 m) and #30 at t671800 side-step between hold goals re-drawn every
  2 s; #31 at t673600 walks at a hold goal it cannot reach (25 reversals in a 0.2 m box - card SKIRT has it). On the
  PS2 a fighting bot STANDS 87.6 % of its foot time (audit of CLSE2).
- For this card's measure step: count, on all maps, the fight-hold seconds with over 6 reversals in a box under 1 m
  (4 Hz samples), main ac13ca6 against today's main, and how often a fighter's hold goal is re-drawn while the
  enemy has not moved. A fix belongs here or in its own card at value 4.

## Note (AUDIT CLSE4, 2026-10-13; `analysis/re-soldier-ai.md` 23.3, `lead_tmp/audit-clse4/`): candidates sized
- **After CLSE4 the port's rifle fires UNDER the PS2's rate in every range bin** (Kamino, shots per aimer-second at
  0-10 / 10-20 / 20-30 / over 30 m): PS2 1.78 / 1.65 / 1.91 / 1.64, parent 2.23 / 1.96 / 2.22 / 2.32, main 1.38 /
  1.19 / 1.32 / 1.39. Damage per aimer-second, port over PS2: x 0.60 / 0.98 / 0.78 / 1.23 on Kamino (two of four
  bins moved away, sum of |ln| 1.08 -> 0.97) and x 0.33 / 0.33 / 0.65 / 2.63 on Polis Massa (three away, 3.15 ->
  3.59). Polis Massa's rifle is at 0.49 against 1.66 at 10-20 m in EVERY build, the parent too (0.65): that gap
  is the aimer time, not the trigger (this card's third item).
- **Candidate: the forgiven pause** (a PS2 rule in both games, read by five verifiers; CLSE4 named it not built):
  a bot that stops wanting to fire owes no pause - 579 of 1,369 PS2 rifle burst starts on foot follow such a reset
  (42 %), 27 % of the PS2's rifle pauses are under 1.5 s against main's 11 %. Knocked in (`knob_main2.patch`,
  `AUDIT_CLSE4=16` in `audit-clse4/bin/rto_mainsw2`; the port's own on-target test, tighter in angle than the
  PS2's 22.5 degrees): Kamino's rifle 1.65 / 1.32 / 1.33 / 1.45, ratios x 0.61 / 0.96 / 0.88 / 1.19; deaths with
  an enemy inside 10 m +0.14 points (t +0.5), none within 20 m +1.08 (t +2.9), contact20 -0.40 (t -1.9), 256 of
  256. It closes the nearest bin's rate and nothing else: build it for fidelity, do not expect the bars from it.
- **Candidate: the PS2's skill levels** (re-ai-difficulty 2.1 / 6; read live: the human's team 11, the other 14
  and up to 17 by the rubber band; the port 17 for all): drawn at 14 / 11 the burst's shape fits the PS2's
  histogram better (sum of absolute differences 15.8-18.9 points against 29.3) and the rate gets WORSE (1.01 /
  0.98 / 1.04 / 1.16). Not a cure either; the tier design is the maintainer's.
- **Tests owed on CLSE4's foot rules** (5 of the audit's mutations fail no test; `audit-clse4/tools/mk_mut.py`):
  GF1 the range asked in 2-D; GF2 the bolt's reach dropped as a bound; GF4 a BF1 round drawing its burst by BF2's
  rule at the point of use (no test sees a BF1 bot fire a 6); GF7 a single-shot weapon exempt from the range test
  (the pistol is the card's own case and no test holds a TriggerSingle weapon at range); W1 the class default
  `MaxRange` 60. Add them with whatever this card builds in `gunner_foot.rs`.
- The range test alone (CLSE4's rule 1) moves none of the closeness numbers on BF2's maps (+0.09 points, t +0.1),
  lowers BF1's contact (-0.50, t -2.0) and carries Tatooine's recycles at card TGAP's footpath (8 -> 27 in 8
  seeds); the foot's Barrage rule carries everything else. The seat's part is card BRSTU.
