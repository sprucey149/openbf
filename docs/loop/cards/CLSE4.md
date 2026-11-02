---
id: CLSE4
title: After CLSE a BF2 bot dies FARTHER from its enemy, not nearer: deaths with an enemy inside 10 m 30 -> 25 % of all (Kamino 27 -> 18 against the PS2's 58), and beyond 30 m the port deals 2.2-3.2 x the PS2's damage per aimer-second (a rifle shot 13.3 hp against 9.0, fired 2.3 times a second against 1.5) - split the far fire against the PS2 by weapon, shots a second and damage per shot, name the cause, build it
milestone: M3
value: 5
phase: build
status: done
deps: CLSE2,DEFS
files: engine/crates/core/src/ai/gunner_foot.rs,engine/crates/core/src/weapon.rs,engine/crates/core/src/ai/gunner.rs
declares: sweep; arena; sim
needs:
effort:
branch: clse4
resume: landed a4905794 2026-10-12
created: 2026-10-12
alpha: yes
domain: ai
---

## Why
AUDIT CLSE (2026-10-12, PARTLY). Card CLSE's rule is the PS2's own (re-derived, seen on the running PS2) and every
number of its landing holds - but it was measured inside 30 m only. The 0.3 against a bot applies at every range, and
in the port the fire beyond 30 m was already above the PS2's. So the landing made a close fight deadlier (its
purpose) and a far one deadlier still: **bots now die farther from the enemy, the opposite of the PS2 and of what
the card set out to cure** (re-soldier-ai 15.5: "what differs is how near the enemy is when a bot is hurt").
The maintainer's rule of 2026-10-11: the fix was good and it brought up a bug that was hidden - this card is that bug.

## Evidence (`engine/artifacts/lead_tmp/audit-clse/README.md`; main 188bc17 unless said; 8 seeds, the first 540 s)
- **Where a bot dies**, 17 BF2 maps, parent b422c11 -> landing 77f4c70e (per-seed paired t): an enemy inside 10 m at
  death 30.3 -> 25.0 % (t -13.5; main with the rule off -> on 29.5 -> 25.6, t -5.3); none within 20 m 42.0 -> 47.6 %
  (t +8.3); the nearest enemy, median 16.5 -> 19 m; lives that get within 20 m of an enemy 76.7 -> 71.4 % (t -8.4).
  Kamino: inside 10 m 27.2 -> 18.3 % (**the PS2 42-70 %, 58 pooled** over three captures), none within 20 m 49.6 ->
  59.1 % (the PS2 12-26), median 19 -> 23 m (the PS2 7-12). Polis Massa: 59.0 -> 46.4 % (the PS2 68-73).
  `tools/contact_spells.py <sweep dir>` (port), `tools/ps2_deaths.py <stem>` (PS2), `sw/perseed/`, `ps2/ps2_deaths.txt`.
  A second reader's count from the kill events (`second/kill_dist.py`): 29.9 -> 24.9 % (t -13.7); kills from over
  30 m (killer to victim) 32.6 -> 35.8 % (t +4.7); per 100 round-minutes the kills with an enemy inside 10 m fell
  864 -> 783 and the others rose 2,025 -> 2,366; the two builds without the rule differ by 0.6 points.
- **Contact** (bar B5, a guard rail): contact20 12.63 -> 10.27 % (17 of 17 BF2 maps down; main 12.14 -> 9.66 over 24
  seeds, t -18); four fifths of it is shorter contact spells (4.64 -> 3.88 s: the close fight ends sooner, as it should),
  one fifth fewer bots getting close.
- **Damage per aimer-second beyond 30 m, port / PS2**: Kamino 1.74 -> 2.15 x (17.6 -> 21.7 hp against 10.1, the PS2's
  block bootstrap 6.8-13.7), main 2.30 x; Polis Massa 1.79 -> 2.32 x, main 3.17 x (the PS2 7.0, 2.0-15.5: 14 falls
  only). Inside 30 m the same landing moved every bin TOWARD the PS2 (`wf/K3/`). Total damage credited over the 17
  maps: inside 10 m -11 % (aimer time there -38 %), beyond 30 m +13 %. The port spends 63 % of Kamino's aimer time
  beyond 30 m, the PS2 38 %.
- **Rifles beyond 30 m on Kamino**: hp per shot the PS2 9.0 (201 shots), parent 10.3, landing 13.3; shots per
  aimer-second of rifle time the PS2 1.5-1.6, the port 2.26; firing 21 % of aimer time against 26.5 %. So about half
  of the 2.15 x is the shot and half the rate. On the PS2 53 % of the aimer time beyond 30 m is NOT in a fight
  order (the helper state column `hst` of `split_ps2.py`), in the port 16 %.
- **Tried, not the cause:** `AUDIT_AI_HOLD=0` (the PS2's bloom inside a bot's burst; card SPRD's tight hold for bots
  is OURS, `weapon.rs::TIGHT_HOLD_SECS_AI`) takes a tenth to a fifth off the far fire (Kamino 23.2 -> 20.8, Polis Massa
  22.3 -> 17.6 hp per aimer-second beyond 30 m) and moves neither the deaths inside 10 m (25.6 -> 25.5 %) nor contact
  (t +0.9). By part of the rule (`wf/K2/`): the 0.3 carries everything; the range factor and the clamp move nothing
  resolved between bots.

## Measured 2026-10-12 (lane 2; report `reports/20261012_1620_card_CLSE4.md`; `analysis/re-soldier-ai.md` 20, docs 5fbbbd4)
Evidence: `engine/artifacts/lead_tmp/clse4/` (README.md). Three PS2 captures a map (CLSE's + two new), 8 port seeds
on main 7d206bc with the observer (null = main byte for byte).
- **The table** (acceptance 1), beyond 30 m, PS2 | port: Kamino 14.4 (10.1-18.6) | 24.5 hp per aimer-second =
  **x 1.70** (not 2.3: the two new captures read 18.6 and 14.5); rifle 1.64 | 2.41 shots a second, 11.9 | 13.1 hp a
  shot, firing 34 | 48 %, in a fight 63 | 93 %; pistol 0.00 | 1.62 shots a second; shotgun 0.36 | 0.57 pulls, 16 |
  39 hp a pull; sniper rifle 0.9 | 19.9 % of the aimer time. Polis Massa 7.4 | 20.4 = x 2.75 (rifle 1.32 | 1.01 shots
  a second: NOT fire - the port holds a target 3-4 x as much of the time at 10-30 m and far aimers are credited with
  near aimers' falls; own hits 15.5). 20-30 m: Kamino x 1.04, Polis Massa x 0.75. Every weapon:
  `clse4/out/table_{kam1,pol1}.txt`, `budget_*.txt`.
- **The cause is four things, by share of Kamino's far damage excess** (hp per 100 AI foot-seconds 58.5 | 146.0):
  snipers 34 % (the PS2's stand at a snipe hint and hardly shoot; the port's short stand is design D13), rifles
  20 % (the burst pause), pistols and incinerators 22 % (fired beyond `MaxRange`), shotguns 19 % (damage per blast).
- **Two PS2 rules the port lacks, both games, verifier 16 / 5 / 0 of 21** (re-soldier-ai 20.2, 20.3): (1) the
  trigger fires a tracked target only when the eye-to-target distance is under the weapon in hand's `MaxRange`
  (pistol 24, commando pistol 32, shotgun and bowcaster 48, rifle 64, incinerator 8; the port: the bolt's reach,
  128 m); (2) the burst is the class keys `BarrageMin` / `BarrageMax` / `BarrageDelay`, defaults 3 / 6 / 2.0 s
  (BF2 n = trunc(3 + U[0,3) x s), pause (1.5 - 0.8 s) x 2.0 s from the burst's END; BF1 n = 3..6 uniform, pause
  2.0 s flat; flyer weapons and the defence turret write their own, 0.5 s) - the port's 2 / 6 / 0.5 are placeholders.
- **Both behind switches in one scratch binary** (`clse4/proto_sw2_7d206bc.patch`, `CLSE4=5`), 17 BF2 maps x 8 seeds,
  per-seed paired t: deaths with an enemy inside 10 m 26.3 -> 28.5 % (t +3.6), none within 20 m 45.9 -> 42.1 %
  (t -7.3), contact20 10.27 -> 11.29 % (t +2.4), kills a minute 31.4 -> 30.3, team kills 1.2 -> 1.3 %, 136 of 136
  rounds; Kamino beyond 30 m x 1.70 -> **x 1.24**, inside 10 m at death 19.0 -> 23.8 %; Polis Massa x 2.75 -> 2.44.
  14 BF1 maps with both: contact 10.75 -> 11.93 % (t +3.2), inside 10 m 22.7 -> 25.6 % (t +5.4), kills a minute
  33.6 -> 30.5, 112 of 112 (the range test alone: contact -0.50, t -2.0).
- **What the two rules do NOT reach** (card CLSE5): the 30 % bar (28.5), Polis Massa's ratio, and the bins inside
  20 m fall further under the PS2 (Kamino 0-10 m x 0.68 -> 0.55) because both rules cut fire at every range.

## Notes
- 2026-10-12 audit CLSE (lane 2; report `reports/20261012_1104_audit_CLSE.md`). Filed. `deps: CLSE2` because that
  build changes the radius at every range; step 2 (the PS2 captures) needs nothing and can run first. Planner: this is
  a BOTD build card's follow-up - add it to BOTV's deps if CLSE2 is there.

## Planner 2026-10-12 11:10
- **value 4 -> 5.** Why: it is the measured cause of a landed card moving the port AWAY from the PS2 on that card's own subject (where a bot dies), on the number where the port is furthest from the PS2 (Kamino: 18 % of deaths with an enemy inside 10 m against 42-70), and at 4 it would wait behind every five of the bot programme and the M1 / M2 fours. It stays after CLSE2 (`deps`). Not in BOTV's deps (bar B5 is "reported, not gated" until the maintainer answers question 32). Undo: `loopctl.py set CLSE4 value=4`.

- 2026-10-12 from CLSE2's landing (main d57ab7d; `lead_tmp/clse2/README.md`, `sw/split_b2_v{0,62}_*.txt`): with
  BF2's whole error block in the port the far fire is unchanged - hp per aimer-second beyond 30 m Kamino 22.1 ->
  24.5 (the PS2 10.1), Polis Massa 21.5 -> 20.4 (7.0); shots per aimer-second there 2.1 (the PS2 0.94) - so the gap
  is not the aim error. And BF1 has the same fault: with the PS2's exact BF1 aim (card CLSE2B, held back for it)
  Kashyyyk Docks deals 19.9 beyond 30 m against the PS2's 12-13 at 2.0-2.1 shots a second. The BF1 split of the
  PS2's far fire (shots a second, health per shot) is not measured yet: CLSE2B step 1 - one method for both games.
- 2026-10-12 17:05 planner: `deps += DEFS` for ORDER only (no code tie): DEFS is the maintainer's own playtest complaint of today
  and the picker took this card first by its ID's alphabet; only the maintainer sets `first:`. Lane 1 takes DEFS, then this
  card. Card SNPR (his sniper wish) depends on this card's build. Undo: `loopctl.py set CLSE4 deps=CLSE2`.

## Built 2026-10-12 18:49 (lane 1; LANDED main a490579, docs bc10b82; report `reports/20261012_1850_card_CLSE4.md`)
Both rules as the Build section says, both games; the three placeholders in `core/src/ai.rs` are gone (RLOOP's branch
does not touch those lines). Evidence: `engine/artifacts/lead_tmp/clse4/out2/` (`paired_t.txt`, `table4_*`,
`bursts_kam1.txt`, `contact_spells_*.txt`, `tat1_recycles.txt`), sweeps `sw/{m2_obs,br_obs,m2_b1,br_b1}`, binaries
`bin/rto_m2` (main 2e12961), `rto_m2_obs`, `rto_br_obs`; the mutation run `build/mut.py` / `mut.txt`.
- **Acceptance of the build, each run this session:** the tests (4 new or rewritten, 15 of 15 mutations fail one);
  Kamino beyond 30 m **x 1.67 -> x 1.23** of the PS2's (bar 0.67-1.5); deaths with an enemy inside 10 m on the 17 BF2
  maps **25.8 -> 29.1 %** (t +5.2, 8 of 8 seeds) and contact20 **11.14 -> 12.24 %** (t +5.2), neither under main's;
  **256 of 256** rounds (the gate's 32 maps x 8 seeds; 136 / 136 and 112 / 112 in the observer sweeps); Kamino's rifle
  bursts mean 2.93 shots (the PS2 2.99; main 6.17), median pause 1.67 s (1.97; main 1.30).
- **Costs:** team kills 1.24 -> 1.47 % of kills on BF2's maps (t +3.5) and 0.72 -> 0.91 % on BF1's (t +3.2); BF1 kills
  a minute 33.6 -> 30.9; Kamino's 20-30 m bin x 1.06 -> 0.78; Polis Massa's far bin x 1.85 -> 2.66 (thin; CLSE5);
  BF1 Tatooine recycles 211 -> 266 in 96 seeds (t +2.4), all but 10 at card TGAP's shut footpath - the fidelity gate
  passed it as unconfirmed on seeds 9-16 (20 / 24 against a baseline of 8 and main's own 14 / 17 on later blocks).
- **Not built (named in the code and re-soldier-ai 20.5):** the x 2 at a snipe stand, the meter gate of a burst's
  start, the PS2's "no pause owed when the want drops", a seat gun's MaxRange test.

## Audit 2026-10-13: PARTLY (lane 1; report `reports/20261013_0646_audit_CLSE4.md`; `analysis/re-soldier-ai.md` 23)
- **Holds:** both rules are the PS2's in both games (five verifiers, 25 claims) and are now read live from the
  PS2's own burst state in RAM (BF1's draw uniform over 3..6 with a 2.00 s pause - its first live reading; BF2's
  by team level; the range test: 1 of 2,380 rifle shots beyond 64 m where 41 are expected with none). Every number
  of the landing reproduces from rebuilt binaries, the sample files byte for byte; both rules off = the parent,
  hash for hash; the landing's 15 mutations reproduce.
- **Does not hold:** "a seat's gun takes its own class's" - the PS2 counts n in ROUNDS, a seat gun has unlimited
  rounds and fires until its heat is nearly full; the port's seat burst of 3-5 shots with CLSE4's 1.65-2.0 s pause
  cut mounted shots 13 % (t -4.7) and the BF1 maps' mounted kills 18 %. Card BRSTU (value 4).
- **Named not built, now sized:** the forgiven pause carries 42 % of the PS2's rifle bursts; the landing's rifle
  fires at 70-85 % of the PS2's rate in every range bin on Kamino. Note on card CLSE5 with the prototype.
- **Tests:** 6 of the audit's 8 mutations fail none (the seat's class, 2-D range, the bolt bound, a BF1 round's
  own draw at the point of use, a single-shot weapon's range test, the default MaxRange).
