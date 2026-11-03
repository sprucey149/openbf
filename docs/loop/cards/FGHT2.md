---
id: FGHT2
title: The PS2's close contact, after FGHT: its attackers get into the defenders' home zone and the port's are stopped on the way (an enemy within 20 m: Kamino 3.7 % against 12.6-15.1, Death Star 5.7 against 18.6) - measure where the port's attackers die, verify the noise radius, then build it or put the look-rate decision to the maintainer; Investigate and Evade are built on a patch and measured
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
Card FGHT (69f8c84, 2026-10-07) built the PS2's fight styles (stand / circle, spaced out) and did NOT meet its own
contact bars: an enemy within 20 m, 8 seeds, BF2 Kamino 3.5 -> 3.7 % of alive unit-seconds (bar 12; the PS2 12.6 -
15.1), Polis Massa 25.3 -> 24.2 % (bar 28; the PS2 22.5 - 35.9), Death Star 2.7 -> 5.7 % (the PS2 18.6). The card's
premise - "go to the noise" brings the PS2's bots together - was built and measured false
(`analysis/re-soldier-ai.md` 15.2). This card owns what is left: find what does, and build it or put the decision
to the maintainer. It is in BOTV's deps (B5-B7 are checked there).

## Evidence (all in `engine/artifacts/lead_tmp/fght/`; `tools/tab.py`, `bars.py`, `near.py`, `mates.py`)
- `analysis/re-soldier-ai.md` 15: the noise radius [D, no verifier yet]; where the PS2's contact is; the look rate.
- **On the PS2's Kamino** (`lead_tmp/sair/ps2/bf2_kam1_v2_units.csv`): 38 % of the samples with an enemy within
  20 m are bots on their capture order that are NOT fighting, 24 % are in the attack (44 % of the attack's time is
  inside 20 m; the port's fight after FGHT: 18 %), 8 % go to the noise. The zone x -40..10, z 80..145 holds 21 % of
  all AI samples; the attacking team's samples there are 4.9 % of all - the port's 0.9 %. **The PS2's attackers
  get into the defenders' home zone; the port's are stopped on the way.**
- **The look rate** is the strongest lever found: a bot with no contact looking every 4 s (the PS2's capture-order
  think away from the player, re-soldier-ai 1.2) takes Kamino 3.5 -> 5.4 %, the Death Star 2.7 -> 7.7 %; 6 s with no
  hub hold Kamino 7.1 %, BF1 Kashyyyk 2 9.9 % (over the PS2's 5.3). `ab/k3_f32`, `ab/k4_f132`. It contradicts the
  bot design's "decides every 0.33 - 0.5 s" (section 7), which the maintainer approved: a DECISION, not a build detail.
- **Investigate** (the noise order, with the PS2's own radius): built - `knobs_k6.patch`, `FGHT=1`. Runs at the
  PS2's share of time; contact does not move; on Polis Massa a team mills over one sniper it cannot reach
  (`bb/bf2_pol1_s4.json`), and with a storey rule and a 10 s pause it still costs Polis Massa (deaths a minute 18.7
  -> 13.8, a teammate within 1.5 m 12.5 -> 21.7 %, recycles 10 -> 27; `ab/k5_f18` against `ab/k6_f19`).
- **Evade** (a passing shot): built in the same patch, `FGHT=8`. Too twitchy without the PS2's skill delay: pinball
  windows on the Death Star 1.5 -> 4.0 % (the PS2 1.5), reversals on BF1 Kashyyyk 2 1.10 -> 1.40 (the PS2 0.66).
- No hold at a hub for an attacker (`FGHT=4`): spreads Polis Massa's fighters (a teammate within 3 m in a fight
  33.7 -> 22.8 %) and loses contact and kills there (22.6 -> 20.4 %, 18.7 -> 16.8 a minute). Left in for TPOS.
- The knob patch applies to 55f055d; its switches are bits of the env word `FGHT` (1 noise + Investigate, 2 the
  fight, 4 no hub hold for an attacker, 8 Evade, 16 the stander's spacing, 32 / 64 / 128 a 4 / 2 / 6 s look rate
  for a bot with no contact, 256 "seen by an AI enemy = a threat entry", 512 the ring no wider than the distance).

## What to do
1. Measure, PS2 beside port, per life: how far from its target post a bot dies, in what behaviour, and how many
   lives reach the target's capture region (the PS2: 8-9 %, re-conquest-goal 5) - the PS2 from
   `lead_tmp/sair/ps2/*_units.csv` and `lead_tmp/cont/ps2/`, the port from `BOT_SAMPLES`. Name what stops the
   port's attackers: seen and shot at range (the look rate), the hub holds, the spawn waves, the route.
2. Run the ONE Opus verifier on re-soldier-ai 15.1 (the noise radius) and open BF1's twin (`FUN_002375e8`).
3. If the cause is the PS2's think / look rate: write the question for the maintainer (STATUS "Questions for the maintainer": a
   travelling bot looks every 2 s as beside the PS2's player, every 4 s, or as today; default today's) and set
   `needs: maintainer`. Else build it, one rule per switch, null variant first, all 32 maps x 8 seeds, the maintainer's Kamino
   scene (`lead_tmp/fght/tools/scene_seeds.sh`) before the report.
4. Investigate and Evade land only with a variant that does not move Polis Massa's bars away from the PS2; else
   they are closed here as "measured, not taken" with the numbers above, and bot-design 4.4's rows say so.

## Acceptance
- A table (PS2 | port, the four PS2 maps): deaths by distance to the target post, lives reaching the region, the
  attacking team's share of samples in the defenders' zone - in the report and in re-soldier-ai 15.
- Then either a landed build with: BF2 Kamino an enemy within 20 m >= 10.1 % (B5's floor), Polis Massa >= 24.1 %,
  the Death Star >= 12.5 %, BF1 Kashyyyk 2 3.6 - 8.0 %; deaths a minute inside x 0.67 - 1.5 of the PS2 where they
  are today (Kamino, Death Star); 256 of 256 rounds over 8 seeds; a teammate within 3 m in a fight no higher than
  after FGHT (Kamino 13.1, Polis Massa 39.9, Death Star 34.9, Kashyyyk 2 8.9 %) - or `needs: maintainer` with the
  question and the measured cost of each answer.
## Notes
- 2026-10-07 08:50 (AUDIT FGHT, PARTLY -> card FGHTA; evidence `engine/artifacts/lead_tmp/audit-fght/README.md`):
  - re-soldier-ai 15.2's PS2 split reproduces from `bf2_kam1_v2` with an own script (`tools/ps2_contact.py`): 12.6 % of
    living AI samples have an enemy within 20 m; 37.9 % of those are bots on their capture order outside the combat
    agent, 24.3 % attack, 8.1 % go to the noise; the zone x -40..10, z 80..145 holds 21 % of all samples.
  - Levers measured on main 6f6160b, 8 seeds x 32 maps (contact20, main 10.65 %; `AUDIT_FGHT` switch,
    `observer_main_6f6160b.patch`): every bot circles 12.16 % (Kamino 3.4 -> 4.7, Polis Massa 20.8 -> 24.7, Death Star
    5.1 -> 6.4, Kashyyyk 2 4.8 -> 6.2), no chance pause in the circle 13.12 % (Death Star 8.8; 255 of 256 rounds),
    every bot stands 9.55 % - card FGHT's contact gain is the circlers' alone. With the fight off, main leaves BF1
    Geonosis with NO OUTCOME in 3 of 16 rounds (main 0): keep that map in any A/B of the fight.
  - How long fighters stand, like for like (under 0.5 m in 1 s): the PS2's on-foot attack rows 24 % (Kamino and Polis
    Massa; the whole combat agent 36 / 24 %), the port's 49 / 56 % after FGHT (64 / 72 before). `tools/ps2_still.py`.
    The PS2 captures are 15 Hz in game time: a per-sample speed is NOT the port's measure.
  - **`lead_tmp/sair/ps2/bf2_kam1_playtest_units.csv` is not "the player at the maintainer's spot"**: the player (slot k 0) dies at
    t 59.3, 7 s into the capture, and from t 80.8 the sampler's hold pins a team-2 AI bot (the reused entity address
    0x1dd8770) at -4.0, 60.3, 108.6 with 400 hp to the end. No PS2 capture has enemies coming at a living held player:
    the PS2 column of the maintainer's scene (the port: 7.2 bots engaging him at once, a teammate within 3 m 33.5 %, still 51.4 %;
    `tools/scene.sh`, `scene_space.py`) does not exist. A hold must follow the player's SLOT, not the entity address,
    and the capture's first check is the slot's entity over time.
- 2026-10-07 (the maintainer's playtest, via Claude): zig-zag (while aimed at within 18 deg) and flank are PS2 engagement modes the port does not build; with REACT (reaction to a hit) they are what makes a fight look like a fight. Build them here or say why not.
- AUDIT GRAI 2026-10-07, two numbers for this card (`lead_tmp/audit-grai/README.md`, `ps2/band.py`): (a) an enemy SOLDIER 8-24 m away, share of live AI soldier samples - PS2 BF2 Kamino 14.4 / 14.7 % (two captures) against the port's 4.6 % (main, 8 seeds, the first 210 s), Polis Massa 24.3 / 22.4 % against 22.8 %: the Kamino gap is in grenade range too, and it is why the port's Kamino bots throw a seventh as often as the PS2's (one per 1,074 bot-seconds against 146 / 239) while Polis Massa matches; (b) GRAI's stand to throw costs contact20 0.54 points on main (11.21 -> 10.67 %, paired t -2.8 over 8 seeds; the whole card 11.04 -> 10.67, t -1.2).

## Measured 2026-10-09 (report `loop/reports/20261009_1025_card_FGHT2.md`; `analysis/re-soldier-ai.md` 15.1, 15.4, 15.5)
Evidence: `engine/artifacts/lead_tmp/fght2/` (`tools/{lives,rng,aware}.py`, `out/`, `sw/`, `verify/`).
- **Step 1, the table** (PS2 player spawned / parked | port main 853ecf0, 8 seeds): re-soldier-ai 15.5. The card's
  premise is corrected: **the port's attackers are not stopped on the way** - distance to the target post at death
  Kamino 156 / 150 | 134 m, Polis Massa 156 / 120 | 137, Death Star 92 / 73 | 106; lives that reach the region
  0.0 / 5.5 | 3.5 %, 4.1 / 8.3 | 6.4, 7.8 / 16.9 | 4.9. What differs: the nearest enemy at death 8 / 9 | 22 m,
  6 / 6 | 10, 9 / 8 | 21; died in a fight 42-67 % | 93-95 %; damage landing on a bot not in a fight order 45-70 %
  | 2-7 %. BF1 Kashyyyk Docks (no target post in its capture) already matches: contact 5.3 / 5.1 | 4.7 %.
- **The cause is BF2's slow think** (4-8 s on the capture order at levels 1-2, where its bots spend 93-98 % of
  their time). Knock-in, switch binary on 853ecf0 (`tools/knob_patch.py`, env `FGHT2` bits 1 look 4 s / 2 look
  6 s / 4 two candidates / 8 one / 16 hears and feels at the slow think only; `FGHT2_ADV=<m>` an attacker takes a
  fight position only inside m; null = main byte for byte on seed 42 x 32 maps), 8 seeds x 32 maps:

  | variant | all maps c20 % (paired t) | KAM1 | POL1 | DEA1 | KAS2 | kills/min | recycles | rounds |
  |---|---|---|---|---|---|---|---|---|
  | the PS2 | - | 12.6-17.0 | 22.5-35.9 | 18.6-24.4 | 5.3 | | | |
  | main | 10.67 | 3.5 | 22.4 | 5.9 | 4.9 | 30.8 | 67 | 255 |
  | look 4 s (1) | 13.97 (+13.0) | 6.0 | 25.5 | 7.6 | 8.0 | 29.1 | 70 | 256 |
  | look 6 s (2) | 15.96 (+28.7) | 7.4 | 25.0 | 10.1 | 9.6 | 28.3 | 95 | 256 |
  | 4 s, 2 candidates (5) | 15.71 (+7.4) | 6.3 | 25.6 | 9.6 | 8.0 | 28.9 | 115 | 254 |
  | 4 s, 2, slow hearing (21) | 15.71 (+20.0) | 7.6 | 25.3 | 11.0 | 8.6 | 29.4 | 74 | 256 |
  | 6 s, 1 candidate (10) | 17.94 (+33.1) | 8.3 | 26.3 | 12.1 | 11.3 | 27.1 | 85 | 256 |
  | **6 s, 1, slow hearing (26) = BF2's level 1** | 21.45 (+20.0) | **10.3** | **27.9** | **15.8** | 13.1 | 25.9 | 98 | 254 |
  | attackers fire and walk on beyond 25 m | 11.49 (+3.9) | 4.4 | 21.8 | 4.4 | 5.7 | 30.3 | 87 | 256 |
  | ... beyond 15 m | 12.81 (+8.9) | 4.9 | 24.8 | 5.6 | 7.5 | 30.6 | 80 | 256 |
  | look 4 s + beyond 15 m | 15.78 (+19.0) | 7.0 | 25.2 | 8.6 | 10.8 | 28.9 | 77 | 256 |

  With (26): died with an enemy inside 10 m 56 / 79 / 59 % (the PS2 56 / 71-77 / 56-61), a teammate within 3 m in a
  fight 9.3 / 27.1 / 29.1 / 5.4 % (no higher than after FGHT), the two unfinished rounds are BF1 Geonosis (the
  audit's stand-off map). The three BF2 bars are met only by (26); BF1 Kashyyyk Docks overshoots (BF1 has no such
  think), so the faithful rule is BF2-maps-only - against "universalised unit behaviour" - and it contradicts
  bot-design 7 (0.33-0.5 s) and card REACT's direction (a bot that is hit reacts). **Hence `needs: maintainer`.**
- **Step 2:** the verifier ran on 15.1 (23 claims: 19 / 3 partly / 1 not; corrections folded in) and BF1's twin is
  open (15.4: factor 3.0, no trigger byte, no floor).
- **Step 4:** Investigate and the passing-shot Evade are closed as measured, not taken (bot-design 4.4 says so).
- **The maintainer's zig-zag / flank note:** not built here - they are modes inside a fight (zig-zag 2 %, flank 5 % of the
  PS2's attack rows on Kamino) and do not bear on the contact gap; moved to card REACT's notes with the numbers.
- New card CLSE: inside 10 m a PS2 bot's aimed weapon deals 65-90 hp a second, the port's 21-33.
- Not measured: any variant with a human in the round (the "only away from a human" half of answer b); the maintainer's
  Kamino scene (no build to check).
- 2026-10-12 audit CLSE (lane 2; `lead_tmp/audit-clse/README.md`). **15.5's rows have moved since this card was
  measured, away from the PS2:** card CLSE's rule (77f4c70, the PS2's own 0.3 against a bot) cut the deaths with an
  enemy inside 10 m from 30.3 to 25.0 % on the 17 BF2 maps (Kamino 27.2 -> 18.3 against the PS2's 42-70; Polis Massa
  59.0 -> 46.4 against 68-73; Death Star 20.1 -> 16.3), raised the deaths with no enemy within 20 m from 42.0 to
  47.6 % and took contact20 from 12.6 to 10.3 % (Kamino 4.1 -> 2.5, Death Star 5.8 -> 4.5) - four fifths of that
  shorter contact spells, one fifth fewer bots getting close. Re-take this card's table before putting the look-rate
  decision to the maintainer: the port's far fire (2.2-3.2 x the PS2's beyond 30 m, card CLSE4) is a second cause of "shot
  before they get close", beside the slow think. `audit-clse/tools/contact_spells.py <sweep dir>` (port),
  `tools/ps2_deaths.py <stem>` (PS2; six captures in `audit-clse/ps2/ps2_deaths.txt`).
