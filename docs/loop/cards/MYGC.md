---
id: MYGC
title: BF2 Mygeeto (showcase) is one-sided because the front never leaves cp3: the port's Alliance never reaches the Empire's forward post cp5_con (cp4, (-130, -31)) - 0 of 87 lives that aim at it in the first 2 min get within 10 m, ~50 deaths a side a round fall at the Alliance's own cp3 - while the PS2's Alliance flips cp4 inside the first minute; port 3-13 over 16 seeds, PS2 1-1 (margins 0 and 14 against the port's 61)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
Card WINS2 (2026-10-22, `reports/20261022_0400_card_WINS2.md`) sorted bf2_myg1 PORT: the port's Empire wins 13 of 16 seeds
(median winner's tickets 61 of 150), the PS2's two bots-only rounds split 1-1, both close (Empire by domination after
1280 s with tickets 2-2; Alliance 14-0 after 665 s). Kills do not decide it: on main the Alliance kills MORE (96.5 v 83.8 a
round, 8 seeds); it loses on posts (bleed). A showcase map that one side always wins would show in the maintainer's first battles.

## Evidence
- Port per post, 8 seeds (main, `lead_tmp/wins2/bq_main`, `lead_tmp/wins2/race.py`, `lead_tmp/wins/run1` cp rows): the Empire
  is first at the neutral cp0 (cp4_con, (10, -29)) at 24-26 s in 8 of 8 rounds (Alliance 42-58 s) and at the Alliance's cp3
  (cp7_con, (-64, 87)) at 20-26 s; the Alliance reaches cp4 (cp5_con) at 230-313 s in 4 rounds and never in 4; owned
  shares cp4 Empire 0.96, cp1 Empire 1.00, cp2 Alliance 0.97, cp0 / cp3 contested. Deaths a round by nearest post:
  cp3 48.6 Alliance + 57.9 Empire, cp0 15.6 + 16.9, cp4 15.8 + 13.8.
- PS2 (`lead_tmp/wins/ps2/bf2_myg1_r{1,2}_cps.jsonl`, CONT's member sampler `lead_tmp/cont/ps2/bf2_myg1_wins2_members.csv`,
  game clock 41.9 s at the first row): the Alliance holds cp0 and flips cp4 at 42 s with 3 members on it; member 17 spawns
  at cp2 (-54, 239) at 42 s and walks (-62, 146) (-86, 94) (-103, 72) (-121, 52) to cp4 at 115 s - the corridor where the
  port's Alliance dies. Captures a round: PS2 12 / 13 and 12 / 8 (rounds 665-1280 s), port 2.9 / 4.5 (~388 s).
- Traced port case (main f44cdcf, `lead_tmp/wins2/bb_t1280/bf2_myg1_s1.json`, `blackbox_read --unit 4`): #4
  all_inf_rocketeer, goal attack cp4, legs 43->44->45->47 (dest hub 91), meets the Empire at 30-56 m from z = 130, holds
  and takes Tactician positions #1408 / #1380 around (-70, 116) for ~8 s, walk-fights toward (-90.5, 80.8), shot at
  (-79, 100) at 42.5 s, 135 m short of cp4. Median closest approach to cp4 of the 87 lives that aimed at it: 135 m.
- NOT the opening wave: a scratch prototype of card OPNW (each opening bot draws a post; 13 of 17 Alliance open at cp3)
  still ends 3-13 over 16 seeds (`lead_tmp/wins2/ab/`), and 1 of 147 cp4-bound Alliance lives arrives.
- Caveat: the PS2 rounds ran with the player on the spawn map (memory 'PS2 samples without the player think slowly').

## Acceptance
- On main with the fix: the Alliance is within 10 m of cp4 by 120 s in at least half of 8 rounds (today 0 of 8), and
  bf2_myg1's win share over 16 seeds is no more one-sided than 10-6 (today 3-13), kills still within 15 %.
- First step (measure): why the Empire's forward push wins cp3 and the Alliance's never passes it - per class kills at
  the cp3 corridor, the Empire's spawn share at cp4 v the Alliance's at cp3 over time, the two teams' attack / defend goal
  shares against the PS2's (CONT's sampler gives the PS2's goal per member), and card MYGP's pit on the same route.
## Result (2026-10-22, lane 1, main f44cdcf) - MEASURED; the fix is two PS2 rules: OPNW + AITHK
Evidence `engine/artifacts/lead_tmp/mygc/` (README.md lists every script and output); report `reports/20261022_0434_card_MYGC.md`.
1. **The opening.** New PS2 samples from the LEVEL LOAD (`mygc/ps2/open_ps2.py`, 3 rounds o1-o3, SW, player off the field):
   the PS2 opens the Alliance 10 at its forward cp3 (cp7_con) + 6 at home cp2, the Empire 9 at its forward cp4 (cp5_con) + 7
   at home cp1, the same in 3 of 3; the Alliance neutralises cp4 at 26-30 s, the Empire cp3 at 29-34 s. Port main: Alliance
   17 at home, 0 forward (round-robin, `WalkSim::opening_wave`); Empire 10 home / 6 forward - so the Empire is first everywhere.
2. **The crossing.** The two forward waves cross head-on near (-100, -30) (the Empire's cp4 wave walks to cp0, 53-62 % of its
   targets). The PS2's runner keeps its capture order (type 8), passes Empire bots at 1-15 m and stands on cp4 at 17.5-24.5 s
   (o1 #2, o2 #2/#6/#13, o3 #2/#5/#6). The port's (OPNW prototype) see the Empire at 35-45 m, fight + hold, stand, die 40-45 m
   short: Alliance attack-cp4 lives from cp3 56, reached 2 %, median life 16 s. Attackers with an enemy at 20-40 m close at
   PS2 2.5-2.9 m/s, still 17-21 % (o1-o3) v port 1.4-1.7 m/s, still 28-33 % (`closing.py`).
3. **Why the PS2 walks through:** the PS2 rounds have no human, so every bot thinks at detail level 0 (re-soldier-ai 1.2-1.3
   [O][L]): a capture walk re-thinks every 8 s, a fight every 1.6 s. Scratch knock-in (`mygc/rto_knock`, env-switched;
   null = main's hashes on 16/16 seeds, OPNW = WINS2's prototype on 16/16):

| variant (16 seeds) | wins All-Emp | cp4 by 120 s (first 8 / 16) | kills All / Emp a round |
|---|---|---|---|
| main | 3-13 | 0/8 (1/16) | 92.9 / 80.1 (1.16) |
| OPNW prototype | 3-13 | 1/8 | 87.6 / 91.3 |
| think level 0 (walk 240 / fight 48 ticks) | 4-12 | 1/8 | 80.2 / 67.5 |
| think level 2 (120 / 24) | 0-16 | 0/8 | 86.1 / 74.8 |
| OPNW + level 2 | 1-15 | 3/8 | 97.3 / 91.8 |
| OPNW + level 1 (180 / 36) | 2-14 | 4/8 | 102.3 / 84.9 |
| **OPNW + level 0** | **9-7** | **8/8 (15/16)** | 94.8 / 86.2 (1.10) |

   All 32 maps x 16 seeds (`mygc/allmaps.txt`): maps at 13-3 or worse: main 16, OPNW 14, level 0 11, both 14; every round
   finishes; rounds ~35 % longer under level 0. Single maps move a lot (bf2_mus1 8-8 -> 16-0, bf2_kas2 11-5 -> 15-1, bf1_yav2
   4-12 -> 7-9, away from the PS2's DESIGN result) - the knock-in is crude (no per-agent factors beyond capture / fight, no
   mover / tracker rate, no view term), so AITHK's build must referee every map.
- So MYGC needs no code of its own: OPNW (the opening draw) + AITHK (the think interval by detail level; level 0 with no
  human, which is the sweep) together meet both bars. Either alone does not. Caveat: the maintainer's own battles run bots near him at
  levels 3-4 (fast); the bars are the PS2's far-from-the-player battle, the one the sweep and the PS2 columns measure.
