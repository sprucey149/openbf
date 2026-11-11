# Bot baseline, November 2026 (bot rework stage 0) - measured 2026-11-10

What the bots and the difficulty tiers do on main today, measured before the bot rework changes anything
(`analysis/bot-rework-plan.md` §4 stage 0, issue #108). Every later stage is judged against these numbers.
All figures come from a proxy corpus: a Good stand-in human (`core::ai::proxy`) on three GCW maps, and a bot-only
sweep. They are not real-player win rates. The stage-1 noise bands of section 6 are OURS and PROVISIONAL, `measure:`
a second pair of seed blocks with this note's commands (twelve seeds estimate the per-seed spread only roughly); the tier
bands are `proxy::TARGETS`, unchanged.

## 1. Method

**Build.** Branch `bot/stage0` at 3811c061: main 39c6f1eb plus the round goldens (db7ecf41) and the census lines and
proxy freeze (75465cd4). Release `round_to_outcome`. No round hash differs from main's (section 1.1), so every
outcome below is main's. Main has since moved to 05e7c44d with no change under `engine/`; the branch now carries that
merge, and the checks of section 1.1 were re-run on it.

**Proxy gate** (`--difficulty-check`, unchanged since the issue-86 retune):

```
cd engine && OPENBF_BLACKBOX=0 OPENBF_MATCHLOG=0 SWEEP_THREADS=16 \
  target/release/examples/round_to_outcome --difficulty-check --strength good --seeds 32 --ticks 90000
OPENBF_BLACKBOX=0 OPENBF_MATCHLOG=0 SWEEP_THREADS=16 SWEEP_ERA=cw \
  target/release/examples/round_to_outcome --difficulty-check --strength good --seeds 16 --ticks 90000 bf2_cor1
scripts/bot_census.py dc <stderr> [--seeds 1-16]     # Wilson tables, D1, levers per foot-minute
```

- bf1_yav2, tat1 and bf1_nab2 (all BF1, GCW), 16 a side on the missions' tickets, seeds 1-32, the proxy on team 0
  and on team 1, all four tiers: 768 rounds, all finished. Tick cap 90,000 (50 min); no round reached it.
- Seeds 1-16 are the Nov-3 corpus (96 rounds a tier). Seeds 17-32 are new; 1-32 gives 192 rounds a tier.
- bf2_cor1 CW, seeds 1-16, 128 rounds: report only, the one BF2 map (D3 is judged per game).
- Wall time 262 s for the 768 rounds at 16 threads, peak 0.8 GB; bf2_cor1 37 s, 1.3 GB. The plan's estimate was
  10-15 minutes at 8 threads, so the full set ran; no fallback was needed.
- Wilson 95 % score intervals (z = 1.96) per tier, per side and per map, rounded once to 0.1 from the exact value.

**Census** (the stage-1 acceptance set):

```
cd engine && STUCK_CENSUS=1 SWEEP_DEATHS=1 HIT_CENSUS=1 BOT_CENSUS=1 SWEEP_THREADS=16 OPENBF_BLACKBOX=0 \
  OPENBF_MATCHLOG=0 target/release/examples/round_to_outcome 60000 <seed> > sweep_s<seed>.txt 2> sweep_s<seed>.err
scripts/bot_census.py census <block A dir> <block B dir>
```

- All 34 conquest maps (the shared bake's sweep set), Normal, bots only, the map's default team sizes.
- Block A seeds 42-47, block B seeds 48-53: 408 rounds, all finished. Round length median 12,933 ticks (7.2 min),
  range 2,528-43,145. 14-18 s wall per seed at 16 threads, 0.9 GB.
- Instrument check: seed 42's stdout is byte-identical to main's plain `round_to_outcome 60000 42` (34/34 rows), so
  the census variables only add stderr. `sweep_seeds.py --report-only` over the same folders gives the same recycles.

**Think cost.** Main and branch alternately, `round_to_outcome 60000 42` (34 maps) with `SWEEP_THREADS=4`, pinned
with `taskset -c 0,2,4,6` (one thread per physical performance core): two pairs at load average 5-6 from other work on
the machine, then six more pairs at load 4.0-4.5 (section 5).

**Attribution.** The same proxy gate (seeds 1-16) built from earlier main commits, on today's bake and missions, and
Hard at seeds 1-48 for the two builds either side of the bisected step (section 2.4).

### 1.1 Stage 0 changed no round hash

| check | main 39c6f1eb | bot/stage0 |
|---|---|---|
| `scripts/sim_gates.sh` TRACE=1 (walk, aim, aim targets, sim-hash 600) | c906917527627f71, 8a742201c8d1a7ec, 443de283b68845b0, 5c5458c8927f122b | identical; the three motion.csv files and the 600-tick trace byte-identical |
| `game --sim-hash 100` | 5573f2b603ed95f4 | identical |
| `round_to_outcome 60000 42` (34 maps, Normal, the sweep gate's command) | 137 lines | byte-identical |
| `arena 3000 42 flat all`, `arena 9000 42 tat2 all` | | byte-identical |
| `scripts/round_goldens.py` (2 space goldens, 12 ground proxy goldens, pinned from 39c6f1eb code) | | 14 PASS, 0 MOVED, 0 FAIL |
| 34-map seed-42 sweep at 4 threads, twice each, alternating | | stdout identical in both pairs |

`cargo test -p core --lib original_hash_pins` passes, as does the asset-free all-proxy pin
(`an_all_proxy_skirmish_keeps_its_hash`).

Re-run on the branch after merging main 05e7c44d (no engine change in that merge): the 14 round goldens PASS,
`game --sim-hash 100` gives 5573f2b603ed95f4, and the seed-42 34-map sweep's stdout is byte-identical to main's in the
six alternating pairs of section 5.

## 2. Proxy gate

Bands (`proxy::TARGETS`): Easy at least 90 %, Normal 77-90 %, Hard 45-60 %, Elite 25-40 %.

### 2.1 Per tier and side

Seeds 1-16 (the Nov-3 corpus):

| tier | wins | rate | Wilson 95 % | proxy on t0 | Wilson | proxy on t1 | Wilson | Nov 3 (landed) |
|---|---:|---:|---|---:|---|---:|---|---|
| Easy | 83/96 | 86.5 % | 78.2-91.9 | 37/48 | 63.5-86.7 | 46/48 | 86.0-98.8 | 88/96 (91.7 %) |
| Normal | 80/96 | 83.3 % | 74.6-89.5 | 35/48 | 59.0-83.4 | 45/48 | 83.2-97.9 | 79/96 (82.3 %) |
| Hard | 40/96 | 41.7 % | 32.3-51.7 | 15/48 | 19.9-45.3 | 25/48 | 38.3-65.5 | 54/96 (56.2 %) |
| Elite | 30/96 | 31.2 % | 22.9-41.1 | 15/48 | 19.9-45.3 | 15/48 | 19.9-45.3 | 13/48 seeds 1-8 (27.1 %) |

Seeds 1-32 (the baseline stage 4 is judged against):

| tier | wins | rate | Wilson 95 % | proxy on t0 | Wilson | proxy on t1 | Wilson | band |
|---|---:|---:|---|---:|---|---:|---|---|
| Easy | 172/192 | 89.6 % | 84.5-93.2 | 80/96 | 74.6-89.5 | 92/96 | 89.8-98.4 | under (point 0.4 below) |
| Normal | 152/192 | 79.2 % | 72.9-84.3 | 64/96 | 56.8-75.3 | 88/96 | 84.4-95.7 | in |
| Hard | 81/192 | 42.2 % | 35.4-49.3 | 29/96 | 21.9-40.0 | 52/96 | 44.2-63.8 | **under** |
| Elite | 63/192 | 32.8 % | 26.6-39.7 | 31/96 | 23.8-42.2 | 32/96 | 24.7-43.2 | in |

Seeds 17-32 alone: Easy 89/96, Normal 72/96, Hard 41/96, Elite 33/96. The two 16-seed blocks agree within their
intervals; Normal's 80 against 72 is the widest gap.

The tiers are ordered (Easy > Normal > Hard > Elite at the point estimates), but Hard and Elite overlap: their Wilson
intervals share 35.4-39.7 %. The gate prints FAIL for Easy (89.6 %, one round short) and Hard.

### 2.2 Per side: the faction split

The proxy's side matters far more than the tier step from Hard to Elite. On the three BF1 maps the proxy as the
Empire (team 0 on bf1_yav2 and tat1, team 1 on bf1_nab2) against the proxy as the Rebels, seeds 1-32:

| tier | proxy as Empire | Wilson | proxy as Rebels | Wilson |
|---|---:|---|---:|---|
| Easy | 77/96 (80.2 %) | 71.1-86.9 | 95/96 (99.0 %) | 94.3-99.8 |
| Normal | 60/96 (62.5 %) | 52.5-71.5 | 92/96 (95.8 %) | 89.8-98.4 |
| Hard | 19/96 (19.8 %) | 13.1-28.9 | 62/96 (64.6 %) | 54.6-73.4 |
| Elite | 10/96 (10.4 %) | 5.8-18.1 | 53/96 (55.2 %) | 45.3-64.8 |

bf2_cor1 CW, seeds 1-16 (team 0 CIS, team 1 Republic): Easy 29/32, Normal 27/32, Hard 18/32 (CIS 11/16, Republic
7/16), Elite 9/32 (CIS 9/16, **Republic 0/16**, Wilson 0-19.4 %). All four bf2_cor1 tiers sit inside their bands on
the two-side average.

This is the playtest's D3 (Hard felt easy as CIS on Coruscant and brutal as the Empire on BF1 maps), now measured.
The plan keeps map and side bias uncorrected and judges bands on the two-side average; the per-faction rows are the
tail stage 4 has to report.

### 2.3 Per map, seeds 1-32

| tier | map | proxy on t0 | proxy on t1 | both | Wilson 95 % |
|---|---|---:|---:|---:|---|
| Easy | bf1_nab2 | 32/32 | 29/32 | 61/64 (95.3 %) | 87.1-98.4 |
| Easy | bf1_yav2 | 24/32 | 32/32 | 56/64 (87.5 %) | 77.2-93.5 |
| Easy | tat1 | 24/32 | 31/32 | 55/64 (85.9 %) | 75.4-92.4 |
| Normal | bf1_nab2 | 30/32 | 26/32 | 56/64 (87.5 %) | 77.2-93.5 |
| Normal | bf1_yav2 | 16/32 | 30/32 | 46/64 (71.9 %) | 59.9-81.4 |
| Normal | tat1 | 18/32 | 32/32 | 50/64 (78.1 %) | 66.6-86.5 |
| Hard | bf1_nab2 | 17/32 | 7/32 | 24/64 (37.5 %) | 26.7-49.7 |
| Hard | bf1_yav2 | 8/32 | 26/32 | 34/64 (53.1 %) | 41.1-64.8 |
| Hard | tat1 | 4/32 | 19/32 | 23/64 (35.9 %) | 25.3-48.2 |
| Elite | bf1_nab2 | 23/32 | 2/32 | 25/64 (39.1 %) | 28.1-51.3 |
| Elite | bf1_yav2 | 6/32 | 12/32 | 18/64 (28.1 %) | 18.6-40.1 |
| Elite | tat1 | 2/32 | 18/32 | 20/64 (31.2 %) | 21.2-43.4 |

Worst tails: Elite tat1 proxy on t0 2/32 and bf1_nab2 proxy on t1 2/32 (both the Empire); Hard tat1 proxy on t0
4/32. Nov 3's worst tail was the same cell, Elite tat1 side 0 (1/16, then 1/24 on fresh seeds).

### 2.4 What moved since Nov 3

The proxy gate at seeds 1-16, built from four main commits and run on today's shared bake and missions:

| main commit | what it adds | Easy | Normal | Hard | Elite |
|---|---|---:|---:|---:|---:|
| 49414d7c (Nov 3, issue-86 retune landed, Elite cap 0.9 m) | | 89 | 81 | 54 | 30 |
| 2e57be54 (11-08) | the Nov 4-8 merges, among them the mend walk (5730b0d8), space-140 and the 11-08 build fixes | 87 | 80 | 31 | 26 |
| 170bb2c0 (11-10) | pre-difficulty: B1/B2 (no bot head or critical-prim multiplier on ground) and C1, C3, C4, C5 | 89 | 88 | 45 | 35 |
| 39c6f1eb (11-10, today) | one game: BF1 classes on BF2 health and infantry damage (and the rest of numbers-awards-lock) | 83 | 80 | 40 | 30 |

Wins out of 96. Instrument check: 49414d7c reproduces the landed Elite training figure exactly (13/48 on seeds 1-8)
and Hard 54/96; Easy and Normal differ from the card's 88 and 79 by one and two rounds (the card's Easy and Normal
rows were measured on the first retune binary and not re-run after the fix).

**Noise.** Each cell is one 96-round sample. A commit that moves the round hash re-rolls every round, so two cells in
a column differ by chance like two independent samples: 2 SD of the difference is about 14 rounds near 40 % (Hard,
Elite) and about 9 rounds near 85-90 % (Easy, Normal). The same code on two seed blocks shows that size: today's
seeds 1-16 against 17-32 differ by 6, 8, 1 and 3 rounds (section 2.1). Read against that:

| step | Easy | Normal | Hard | Elite |
|---|---|---|---|---|
| 49414d7c -> 2e57be54 | -2 | -1 | **-23 (3.3 SD)** | -4 (0.6 SD) |
| 2e57be54 -> 170bb2c0 | +2 | +8 (1.7 SD) | +14 (2.1 SD) | +9 (1.4 SD) |
| 170bb2c0 -> 39c6f1eb | -6 (1.4 SD) | -8 (1.7 SD) | -5 (0.7 SD) | -5 (0.8 SD) |

Only Hard's fall from 49414d7c to 2e57be54 is clear of the noise on one sample. The others are single-sample
observations: Hard's +14 at 170bb2c0 is about 2 SD, and every move at 39c6f1eb is inside 2 SD on its own (all four
fell, which is suggestive, not shown).

**Hard's fall, bisected.** Over the 100 first-parent commits from 49414d7c to 2e57be54, Hard alone at seeds 1-16:
0d5c4e06 48, 9a035ae6 32; inside that merge, 15db8522 (its parent) 48 and 5730b0d8 32. 5730b0d8 is "Walk mending bots
into the health droid service radius" (Nov 4, in merge 9a035ae6). The bisect steps repeat exactly 48 or 32 because
commits that leave the round hash alone replay the same rounds; the step itself is one sample, 16 rounds (2.3 SD).
Replicated at seeds 1-48 (the first 16 reproduce 48 and 32 exactly):

| build | seeds 1-16 | fresh seeds 17-48 | seeds 1-48 |
|---|---:|---:|---:|
| 15db8522 (before the mend walk) | 48/96 (50.0 %) | 87/192 (45.3 %) | 135/288 (46.9 %, Wilson 41.2-52.6) |
| 5730b0d8 (the mend walk) | 32/96 (33.3 %) | 74/192 (38.5 %) | 106/288 (36.8 %, Wilson 31.4-42.5) |
| difference | 16.7 points (2.3 SD) | 6.8 points (1.3 SD) | 10.1 points (2.5 SD) |

So the mend walk most likely lowers the proxy's Hard wins, but by less than seeds 1-16 showed: about 7 points on the
fresh seeds, about 10 on all 48. Seeds 1-16 were the seeds the bisect searched on, so their 17 points are the high
draw. The commit walks a bot sent to a health droid (`mend`: Hard and Elite, and the proxy's own rule) on into the
droid's service radius, so those bots now finish healing; `mend` holds on about 10 % of Hard and Elite thinks (18.9 a
foot-minute, section 3.2). It is a fix. The remaining 6 rounds of the 23 (54 to 48 before 0d5c4e06) are inside noise
and were not bisected, and the rest of the 23 may be noise as well.

What the bands were tuned with on Nov 3 has changed in these ways; the sizes are the single samples above:
- **Fixed:** Hard and Elite bots (and the proxy) now reach the health droid they go to (5730b0d8): Hard about 7-10
  points lower on 48 seeds.
- **Removed:** the bot head x3 and critical x4 multipliers from Hard up (B1/B2), landed together with C1-C5 in
  170bb2c0: Hard +14 and Elite +9 rounds on one sample each. The split between B1/B2 and C1-C5 was not measured.
- **Changed:** BF1 soldiers now have BF2 health and infantry damage (39c6f1eb). Every tier fell by 5-8 rounds, each
  inside 2 SD on its own; all three check maps are BF1.
- **Still in:** human pressure (x0.8 / 1.0 / 1.3 / 1.6), the Hard and Elite view (x1.25 / x1.5), DFOCUS focus and
  push, TOKN tokens (Elite 7), Elite's `picks` and `hunts`, `mend` at Hard and Elite, the Cover role at Hard and
  Elite, DTEAM ally offsets, Elite's 0.9 m aim-error cap and Elite's x1.25 bleed handicap on the human's team.
  Section 3 counts how often each one fires.

## 3. Tier levers and the far-human preference (D1)

From the same 768 proxy rounds. "Facing" = the bots on the proxy's enemy team, who play the host tier; "allies" =
the proxy's own team, which DTEAM puts on a lower row at Normal and up (Normal -> Easy row, Hard and Elite -> Normal
row).

### 3.1 D1: does a bot shoot the far human past a near bot?

On a think whose target is an enemy soldier in sight: the share where another engageable enemy bot (seen this think,
admitted, within fight reach) is at most half the target's distance. The human rate is the far-human preference; the
same bots' rate on bot targets is its comparison. Bots facing the proxy, seeds 1-32:

| tier | human-target thinks | far % | far % of new takes | bot-target thinks | far % | far % of new takes |
|---|---:|---:|---:|---:|---:|---:|
| Easy | 54,715 | 1.07 | 0.95 | 1,560,957 | 0.63 | 0.77 |
| Normal | 82,693 | 0.81 | 0.71 | 1,447,761 | 0.66 | 0.67 |
| Hard | 149,270 | **10.20** | 14.13 | 1,342,264 | 0.86 | 0.76 |
| Elite | 119,459 | 0.39 | 0.63 | 1,534,389 | 0.13 | 0.11 |

bf2_cor1: Hard 13.35 % against 1.42 %; the other tiers under 1 %.

Hard is the playtest's D1 complaint in numbers: one human-target think in ten has an engageable bot at half the
distance or less, twelve times the bot-target rate. Elite does not show it. Why is not measured: the census counts D1
and the levers on the same thinks but does not split D1 by which lever set the target. A think picks its target in
three steps (`ai/behave/brain.rs`): the pressure-scored choice, then DFOCUS (a token holder at Hard and Elite takes
its human), then at Elite only the threat pick (`proxy::pick`: the nearest enemy it sees, counted nearer when he aims
at it or is already hurt), which can replace either earlier choice. DFOCUS takes a human about as often at the two
tiers (9.1 and 8.7 a foot-minute, section 3.2), and the pick names the target on 44 % of Elite thinks, so the pick
overriding DFOCUS and pressure is a likely reason, not a shown one. Stage 3 should add the D1 split by lever before
acting on it. Either way, stage 3's acceptance ("far-human preference roughly equal to the bot-target rate") is a
Hard problem today: 10.2 % against 0.9 %.

### 3.2 Levers per foot-minute

Think-sampled levers count thinks where the lever held (a bot on foot thinks about 196 times a foot-minute, three a
second plus wakes); events count times it fired. Seeds 1-32, 3 maps:

| lever | kind | Easy facing | Normal facing | Hard facing | Elite facing | Normal row, allies at Hard |
|---|---|---:|---:|---:|---:|---:|
| reaction-wait (target seen, not yet fireable) | think | 2.85 | 2.98 | 2.82 | 4.27 | 2.94 |
| search (far-hit search still running, C1) | think | 0.99 | 1.07 | 1.00 | 0.46 | 0.82 |
| pressure-decisive (pressure decided the pick) | think | 2.72 | 0 | 5.21 | 3.46 | 0 |
| beyond-view (target past Normal's sight) | think | 3.13 | 6.70 | 16.29 | 27.47 | 6.47 |
| scope (target past this bot's own plain sight) | think | 3.13 | 6.70 | 6.40 | 7.35 | 6.47 |
| hunt (hunt rule overrides the commander) | think | 0 | 0 | 0 | 108.6 | 0 |
| dfocus-take (DFOCUS gives a human to focus on) | think | 0 | 0 | 9.09 | 8.69 | 0 |
| pick (threat pick names the target) | think | 0 | 0 | 0 | 86.10 | 0 |
| mend (going to a health droid) | think | 0 | 0 | 18.93 | 17.41 | 0 |
| dfocus-push (walk to the human's last place) | think | 0 | 0 | 0.63 | 0.25 | 0 |
| grenade-flee | think | 0.58 | 0.48 | 1.01 | 1.05 | 0.41 |
| grenade-ignore (in range, coin or delay said stay) | think | 1.98 | 1.61 | 0.67 | 0.49 | 1.38 |
| cover-role | think | 0 | 0 | 4.82 | 5.37 | 0 |
| flinch (C1) | event | 3.05 | 2.91 | 2.61 | 2.36 | 2.68 |
| react-strafe / roll / move / stay (REACT on a position) | event | 0.28 / 0.04 / 0.10 / 0.47 | 0.26 / 0.04 / 0.09 / 0.44 | 0.28 / 0.04 / 0.13 / 0.35 | 0.29 / 0.06 / 0.15 / 0.31 | 0.25 / 0.03 / 0.09 / 0.42 |
| position-hit | event | 1.39 | 1.32 | 1.12 | 1.01 | 1.21 |
| dodge (skill coin side-step on a leg) | event | 0.13 | 0.16 | 0.17 | 0.14 | 0.15 |
| foot-minutes | | 18,925 | 19,240 | 20,063 | 19,209 | 18,644 |

Reading it:
- **Easy's pressure 0.8 also decides picks** (2.72 a foot-minute): it turns Easy bots away from the human.
- **Hard and Elite differ in a fight mostly by Elite's `hunt` and `pick`** (55 % and 44 % of Elite thinks), both
  borrowed from the proxy's rules, plus view (beyond-view 16 against 27). Cover role, mend, DFOCUS and grenade flee are
  the same at the two tiers. This matches the plan's §1.4 ("Elite only differs from Hard by its extras").
- **Movement levers barely move with tier.** Flinch, REACT outcomes, position hits and dodges stay within a factor
  of 1.5 from Easy to Elite, in both directions (flinch falls 3.05 -> 2.36, a REACT move rises 0.10 -> 0.15). Only
  grenade flee doubles at Hard and Elite (the tier's coin). Stage 4's "lever census ordered by tier" starts from
  nearly flat.
- At Normal "beyond-view" and "scope" are the same count by construction (Normal's view is the reference).

The per-group lines (`d1 <map> <group>:`, `levers <map> <group>:`) are in the round's stderr; `bot_census.py dc`
prints the table for both facing and allied groups.

## 4. Census: the stage-1 acceptance set

34 maps, Normal, bots only. Ratios over the block; recycles, relocations and stand20 foot-s are the mean per seed
(over 34 maps). The band is the one stage 1 needs: a 6-seed candidate block against a 6-seed base block that only
re-rolled differs by up to 2 sd sqrt(1/6 + 1/6) about 95 % of the time, sd being the spread of the per-seed values over
both base blocks (`bot_census.py selftest` checks this on simulated re-rolls: 94 % held, against 67 % for the one-block
2 sd / sqrt(12) this note first used, which was half as wide). Noise = max(|A - B|, band).

| metric | A (42-47) | B (48-53) | both | abs(A - B) | band | noise | Oct HYST (42-47) |
|---|---:|---:|---:|---:|---:|---:|---:|
| recycles per seed | 0.33 | 0.50 | 0.42 | 0.17 | 0.60 | 0.60 | 3.2 (19 / 6) |
| relocations per seed | 4.17 | 4.67 | 4.42 | 0.50 | 1.67 | 1.67 | - |
| stand20 foot-s per seed | 802.5 | 766.8 | 784.7 | 35.7 | 113.3 | 113.3 | 1,398 (8,389 / 6) |
| stand20 % of foot time | 0.180 | 0.168 | 0.174 | 0.012 | 0.026 | 0.026 | - |
| Recover % of foot time | 1.74 | 1.82 | 1.78 | 0.07 | 0.21 | 0.21 | 1.36 |
| B12 cover % of fight | 17.18 | 17.00 | 17.09 | 0.18 | 0.39 | 0.39 | 15.61 |
| kills a round-minute (both teams) | 34.51 | 33.77 | 34.13 | 0.74 | 0.80 | 0.80 | - |
| kills a foot-minute | 1.14 | 1.14 | 1.14 | 0.002 | 0.011 | 0.011 | 1.196 |
| position flips a fight-minute | 1.81 | 1.81 | 1.81 | 0.005 | 0.039 | 0.039 | 1.71 |
| D1 bot-target far % (no human here) | 0.99 | 0.92 | 0.96 | 0.07 | 0.09 | 0.09 | - |

Totals: 408/408 rounds finished; recycles 2 (A) and 3 (B), no spot on two seeds; stuck_max median 180 on every seed
(240 on one).

The October column is card HYST's `final` on seeds 42-47 (different code, STUCK_CENSUS=200). It is context only:
stand20 and recycles have fallen since, Recover, B12 and flips have risen.

## 5. Think cost

`Sim::think_hist` (log buckets 25 % wide): one call = one unit's think entry on one tick; a think tick = a tick where
the think ran (scheduled or woken).

| run | per call p50 | per call p99 | per think tick p50 | per think tick p99 |
|---|---:|---:|---:|---:|
| pinned, 4 threads on 4 performance cores, 34 maps seed 42 (eight runs, identical medians) | 1.3 us | 49.2 us | 10.2 us | 131.1 us |
| census, 16 threads, 408 rounds | 1.8 us | 65.5 us | 14.3 us | 196.6 us |
| proxy gate, 16 threads, 768 rounds | 1.8 us | 57.3 us | 14.3 us | 196.6 us |

Medians over rounds of each round's quantile. The slowest single call was 35-37 ms in every pinned run, once in the
34-map sweep.

**The census's own cost.** The census work (`ai::census`, the D1 and lever reads) runs on every ground think whether
or not `BOT_CENSUS` is set; the variable only prints it. Main against the branch, the same pinned command alternating:

- First pair (load 5-6 from other work): main 125.4 / 130.0 s user, branch 127.9 / 128.5 s. Branch +0.4 %, but with
  two runs a side the band is +-7.7 % (`bot_census.py timing`, Student's t with 2 degrees of freedom): this pair could
  not tell +0.4 % from +5 %, and the first version of this note overstated it.
- Second set, six alternating pairs (load 4.0-4.5): main mean 129.10 s (sd 1.97; its first run, 132.9 s, is the
  slowest of all twelve), branch 128.94 s (sd 0.21). Branch -0.13 %, band +-1.40 %: within the +5 % budget, and the
  census costs at most about 1.3 %. Leaving out the first pair, +0.50 % +-0.60 %, so a real cost of about half a per
  cent is likely; it is not resolved from zero. All twelve runs printed byte-identical stdout (137 lines).

The histogram's 25 % buckets cannot resolve a 5 % budget. Stage 1 checks its think cost with
`bot_census.py timing <time file> --base <stage-0 build> --cand <candidate>`: user seconds of the 34-map seed-42
sweep, pinned as above, the two builds alternating, at least five runs each. The tool gives a verdict only when the
whole band lies on one side of the budget, and otherwise says how many runs would resolve it. Six pairs take seven
minutes.

## 6. What stage 1 must hold

Stage 1 (the carve into situation, selector and behaviours) accepts on:

1. **Hash identity, or a declared move inside noise.** Identity = every row of section 1.1, plus the 14 round goldens.
   If the carve cannot keep the hash (the plan's Q1, default: allowed), every census metric of section 4 must sit
   inside the noise column for a candidate block on the same seeds, using `scripts/bot_census.py compare <base
   block A> <candidate on seeds 42-47> --noise <base block B>` (and the same with the blocks swapped for seeds 48-53).
   compare takes the band for the base block's size against the candidate's, so a 12-seed candidate against both
   blocks gets the narrower band it has earned. On today's numbers that means, per 6-seed block: recycles within 0.6
   a seed, relocations 1.7, stand20 113 foot-s, Recover 0.21 points, B12 0.39 points, kills a round-minute 0.80, kills
   a foot-minute 0.011, flips a fight-minute 0.04, D1 bot far 0.09 points. Each band holds about 95 % of re-rolls, so over ten
   metrics a pure re-roll puts at least one outside up to 40 % of the time (less, as they are correlated): one metric
   just outside is a reason to run a second candidate block, not a verdict.
2. **The proxy is frozen.** `an_all_proxy_skirmish_keeps_its_hash` must not move. A declared move that changes proxy
   rounds re-runs the gate at seeds 1-32 and reports every tier, side and faction row against section 2 (bands are
   report-only until stage 4). The freeze covers the proxy's brain, not the round around it: the reads a proxy round
   still takes from the live tier rows are listed in `core::ai::proxy`'s header. One of them moves every Elite gate
   round: a proxy counts as a human, so at Elite its team bleeds tickets 1.25 times as fast
   (`Sim::apply_team_handicap` reads the live `Difficulty::tier().bleed_handicap`, not `FROZEN_TIERS`). A tier change
   to `bleed_handicap` therefore moves the Elite gate with no change to any proxy.
3. **Space is unchanged.** The two space goldens PASS (they run through the legacy think path; Q2 default).
4. **Think cost within +5 %**: `bot_census.py timing` on alternating pinned runs (section 5) says "within budget".

Stage 3 and 4 numbers to beat, from this note: D1 at Hard 10.2 % against 0.9 % (target: about equal); Hard 42.2 %
and Easy 89.6 % outside their bands; the faction split (Empire 10-20 % at Hard and Elite against Rebels 55-65 %);
Hard and Elite with overlapping intervals.

## 7. Not checked

- No live play; the proxy is a bot with a human's rules, not a person.
- Only one BF2 map (bf2_cor1, 16 seeds). No space round is part of this baseline (space keeps its goldens only).
- The census ran without `--botq` (bot_quality's reversal, pinball and spacing rows); they were not part of the
  stage-1 acceptance list.
- Only Hard was bisected, at seeds 1-16, and only its last step (5730b0d8) was replicated (seeds 1-48). The other
  moves of section 2.4 are single 96-round samples; Elite's 30 -> 26, Hard's 54 -> 48 before 5730b0d8, and the split
  of 170bb2c0 into B1/B2 against C1-C5 were not measured.
- Think time was measured with other jobs running (load 4-6); alternating pinned runs keep the comparison fair, not
  the absolute numbers.
