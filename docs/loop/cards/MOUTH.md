---
id: MOUTH
title: Bots miss the mouth of a walkway, doorway or ramp, push at its side for seconds, back up and then cross (the maintainer's play, BF2 Kamino, 2026-10-11) - and the same stall is what puts bots over Bespin's rim: measure it per leg, take the PS2's rate, and make a bot enter a narrow leg through its mouth
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/ai/route.rs, engine/crates/core/src/ai/route/tests.rs, engine/crates/core/src/ai/state.rs, engine/crates/core/src/ai.rs
declares: sweep; arena; sim
needs:
effort: high
branch: mouth
resume: landed d096ae70 2026-10-12 (273a441 the two rules + d096ae7 CRATES.md)
created: 2026-10-11
alpha: yes
first: 1
---
## BUILT AND LANDED 2026-10-12 13:30 (lane 1; main 273a441 + d096ae7; report `loop/reports/20261012_1331_card_MOUTH.md`; outputs `lead_tmp/mouth/build/`)
Two rules in `ai/route.rs`, three tests (each fails with its rule taken out):
1. **The PS2's segment rule**: past a detour corner the leg point's line starts at that corner (`seg_from`).
2. **A hub hop opens a new progress window** - found by tracing BF1 Kamino's regress under rule 1 (acceptance f): the
   window's tick count was carried over the hop while its reference distance became the new leg's, so a hop in the
   window's last ticks "failed" it 0-3 ticks later: 23-40 of the 38-112 failed windows in 3 minutes of a round on
   BF1 / BF2 Kamino and Mygeeto (main), each a re-plan and a 1 s side-step for a bot at a run. At BF1 Kamino's hub 13
   (a ramp's foot) that side-step put unit 26 at the east rail's end; main's line pulled it 0.3 m further in and it
   went up the ramp, rule 1's line did not and it went round the outside into the corner at (149.6, 52, 155.1).

| acceptance (8 seeds unless said) | main | rule 1 | rule 2 | both (landed) |
|---|---|---|---|---|
| (b) BF2 Kamino's four walkway legs: card's stalls / P-stalls (bar 3) | 32 / 41 | 0 / 3 | 38 / 42 | **0 / 6** |
| BF2 Kamino, all legs | 258 / 711 | 238 / 660 | 273 / 704 | 239 / 616 |
| (c) BF1 Bespin deaths under the kill floor, 8 rounds of 5 min (bar 20) | 46 | 12 | 45 | **7** |
| (d) the maintainer's Kamino scene, trails over 6 reversals (worst) | 6 of 416 (18) | 5 of 398 (10) | 5 of 402 (13) | 6 of 404 (12) |
| (f) BF1 Kamino, 8 x 3 min: card's stalls / P-stalls | 17 / 195 | 51 / 252 | 4 / 184 | **7 / 182** |
| (e) 24 seeds x 32 maps: rounds finished | 768 | 768 | 768 | 768 |
| recycles | 163 | 144 | 177 | 147 (paired t -0.9) |
| deaths under the kill floor | 176 | 54 | 193 | 46 (t -9.4) |
| lost / falls / death regions | 56 / 8 / 49 | 56 / 7 / 24 | 61 / 3 / 45 | 64 / 3 / 66 (t +0.6 / -1.4 / +1.2) |
| kills | 176,810 | 177,728 | 176,255 | 174,562 (-1.3 %, t -1.5) |

(b) (c) (d) (f) and an 8-seed sweep were run again on today's main 4ca302f (after DAGS's landing): the same values on
those three maps; all maps recycles 61 -> 30 (t -3.1), floor 64 -> 13. Death regions: Mygeeto 7 -> 19 over 24 seeds is
noise - over 96 seeds 35 -> 37 (main's own 24-seed blocks run 4 to 15); they are hover speeders stuck on the slope of
the pit at (-69.7, -1.5, 178.6) whose riders get out and fall, on main too. Gates: fidelity 0 worse, 4 better
(recycled all maps 49 -> 30, stuck_max median 204 -> 169).
**Left open** (notes on the cards): acceptance 5 of the original, the stall rate as a standing number (card STLG);
BF2 Kamino leg `56>51` and the two edges below (MOUTH2); the corner pocket at BF1 Kamino's hub 13 (RLOOP); BESR's bars.
- Edge not built: when the continued corner search (`advance_route`, card OBST2) finds no further corner, the leg's
  corners are cleared and the line is `Route::from`'s again.

## MEASURED 2026-10-12 (lane 1; report `loop/reports/20261012_1216_card_MOUTH.md`; everything in `lead_tmp/mouth/`, README.md)
**The walkway miss is one port defect, the PS2 does not have it, and a 10-line change removes it. This card is now
that BUILD.** The other stalls on Kamino (the pod room's door, hub 46's pod) are different mechanisms: card MOUTH2.

1. **Still there on today's main** (the card's own detector, 8 seeds): Kamino 292 stalls in 739 bot-minutes on
   19708fa (one per 2.5; 289 on 4c238b1) and 258 on fe043b4 (OBST2 landed); Bespin 352 in 470 (one per 1.3).
2. **The PS2's number** (two 8-minute captures a map, player spawned; `stop` cannot be read, so the detector is
   `tools/pstall.py`: 1.5 s of forward stick on its route with under 1.5 m gained toward the bot's point of the next
   hub - on the port it finds 93 % of the card's stalls and 2.3 x as many again, detours count on both sides):

   | per 20 route bot-minutes | PS2 | port 19708fa |
   |---|---|---|
   | BF2 Kamino | 8.7 (67 in 153) | 22.5 (664 in 591) |
   | BF1 Bespin Platforms | 3.6 (33 in 186) | 32.2 (566 in 351; 12.8 without ONE looping bot a round, see 5) |

   By site, stalls per 100 passes, PS2 | port: Kamino's four walkway mouths 1 in 121 | 50 in 631 (hub 85: 2 | 11,
   hub 21: 0 | 37); the pod room's door 4 | 47; round hub 46 with its pod 5 | 50 (`out/kam1_sites.md`). The PS2 does
   stall - a third as often on Kamino, a ninth on Bespin - and almost never at a walkway. **The bar is the PS2's.**
3. **The walkway's cause, traced** (5 of 5 on `85->49`, `tools/trace.py`): the bot reaches ITS point of hub 85 (up
   to 4 m beside the walkway's line), walks at the corner the plan gives it at the mouth, (-91.4, 28.0) - and the
   moment that corner counts as passed, `route::leg_point` projects it back onto `Route::from` -> waypoint, the very
   line the corner walk was planned to avoid: the aim jumps to (-87, 24.9), beside the ramp, the bot runs diagonally
   into `kam_bldg_ramp1` / the rail barrier, slides 5 m along it, fails its window, walks back to the hub's centre
   and crosses from there. The maintainer's words exactly.
   **The PS2 at the same mouth** (`tools/ps2leg.py`, live): the follower holds `from h85 (-90.9, 25.9) at pt (-90.8,
   27.3) then h49`; half a second later `from pt (-90.8, 27.3) at h49 (-51.6, 29.2)` and the aim is (-82.3, 27.7), on
   the walkway. Its line starts at the corner. Read in the code and verified (`analysis/re-soldier-ai.md` 2, docs
   18e592e: `FUN_002f1470` -> `FUN_002f0e78`; one verifier, 6 of 6): the PS2's own rule, not a deviation.
4. **The rule, prototyped behind a switch** (`mouth_proto_m1.patch`, `MOUTH=1`: the leg point's line starts at the
   last corner passed; null = main frame for frame). 8 seeds each:

   | | main | line from the last corner | + the arrival plane (`MOUTH=3`) |
   |---|---|---|---|
   | Kamino, the four walkway legs: card's stalls / P-stalls (19708fa) | 28 / 45 | **0 / 6** | 2 / 5 |
   | the same on fe043b4 | 32 / 41 | **0 / 3** | - |
   | Kamino, all legs: card's stalls (19708fa; fe043b4) | 292; 258 | 278; 238 | 266 |
   | **BF1 Bespin, deaths under the kill floor, 8 rounds of 5 min** (19708fa; fe043b4) | 44; 46 | **8; 12** | 14 |
   | all 32 maps, 2 min: card's stalls / P-stalls (fe043b4) | 3,023 / 9,598 | 2,808 / 9,103 (paired t -1.3 / -2.5) | - |
   | all 32 maps: `kill_floor` events (19708fa; fe043b4) | 36; 39 | 3; 5 | 5 |
   | the maintainer's Kamino scene, trails over 6 reversals (19708fa; fe043b4) | 8 of 428; 6 of 416 | 9 of 418; 5 of 398 | 7 of 452 |

   Bespin's floor deaths fall in every one of the 16 paired rounds: its bots walk off a rim when the same snap pulls
   them off a walkway (legs 58->59, 51->61, 52->50 gone; 35->4 is left). That is the answer to "why are the bots
   falling off the map". The plane variant adds nothing: build bit 1 only.
   **Worse under it: BF1 Kamino** (7 -> 33 and 13 -> 39 stalls in 16 min x 8 seeds): 34 of the 39 are ONE bot,
   unit 26, in three of the eight seeds, at (150, 52, 155) on legs `-1->13` / `13->16` / `13->14` against
   `kam_bldg_mushplatform1` and `kam_bldg_ramp1` - a re-plan loop at a ramp mouth that main does not enter (not
   traced further; `proto_fe_all/v1/bf1_kam1/`). BF2 Tantive IV and BF1 Endor rise within their own noise.
   Not run: the 24-seed sweep, the gates, a test.
5. **Bespin's `122->7` is not a mouth**: 351 of the map's 566 stalls are unit 8, once a round - spawned at (-21.1,
   98.7, -187.6) against the round building's wall, it misses the door at hub 8 and walks the wall outside for the
   whole round with a route through the inside (recover count 14 in a minute, never ended). The PS2 and the port's
   other bots use that door alike. A note is on RLOOP and SPNC.
6. **All 32 maps** (`out/all_maps_table_19708fa.md`, 4 seeds x 2 min): 1,541 of the card's stalls in 4,399 route
   bot-minutes (7.0 per 20); 34 % begin at a mouth, 23 % on a first leg. Worst: BF2 Geonosis 24.4 (leg `9->13`: 97),
   BF2 Utapau 21.5, BF1 Bespin 21.4, BF1 Endor 11.7, BF2 Hoth 10.9 (one leg, 39), BF2 Tantive IV 9.5, BF2 Coruscant
   9.3, Mos Eisley (tat2) 9.0. NOT done: the free width per arc end against the hub's radius (acceptance 2's first half).

## Why (the original card, 2026-10-11)
The maintainer, 2026-10-11 (chat): "Lots of bots when crossing the walkways between adjacent platforms, they will miss the walkway
and go to the side of it, try running at the side for a few seconds, then go back a few feet then correctly go over the
walkway. That is a bug that shouldn't ever happen ... the bots somehow need better logic at knowing where to avoid
(ledges) and where to go (walkways)." And of Bespin: "I want to know why the bots are falling off the map. They shouldn't
do that."

No gate or scene number sees this. The scene's number is "trails with over 6 reversals in 10 s" (jitter); this stall is
one or two reversals. The sweep's `noprog` wants 3 s; these are 1.5 - 3 s. So cards have passed with it in place, and
HUBP's rejected variants (below) were judged by numbers that cannot see it.

## Evidence (measured 2026-10-11 by Claude in chat, main 4c238b1; tools and outputs in `engine/artifacts/lead_tmp/walk/`)
A STALL = a bot on foot walking its route leg (`walk` 5), something shutting its way within 1.5 m (`stop` wall / barrier /
steep / drop), for 1.5 s or more with under 1.5 m gained toward its aim point. BACKED UP = it then goes over 1.5 m further
from the aim point. `tools/snap.sh` (8 seeds, black-box snapshots) + `tools/stalls.py`.

| run | bot-minutes | stalls | one per | backed up | mean | longest |
|---|---|---|---|---|---|---|
| BF2 Kamino, main 4c238b1, 8 seeds x 3 min | 740 | 289 | 2.6 bot-min | 106 | 2.5 s | 11.5 s |
| BF2 Kamino, the maintainer's own play 10-07 (build 6f6160b, 4 snapshots) | 101 | 51 | 2.0 bot-min | 27 | 2.9 s | 7.5 s |
| BF1 Bespin Platforms, main 4c238b1, 8 seeds x 2 min | 466 | 394 | 1.2 bot-min | 189 | 2.4 s | 8.0 s |

So d9db73c, OBSR and 7dfb448 (the corner walk) did not cure it: 2.0 -> 2.6 bot-minutes a stall. With 32 bots that is
about 12 a minute on Kamino.

It sits on a few legs (`out/*.txt`):
- Kamino `85->49` (Hub117 r 4.2 -> Hub56 r 2.0, a 43.5 m walkway): moving bots are a median 0.5 m off the arc's line;
  the stalled ones 3.7 m off it, 2.5 m along it, 76 of 83 on one side, stopped by a `barrier` (the rail). `21->36`
  (r 2.0 -> r 2.0, 41.8 m): stalled 2.6 m off, 1.5 m along, a barrier. The bot starts the walkway leg from beside its
  mouth, with the rail between it and the line.
- Kamino `94->98` (83 stalls, 44 backed up; the pod room's doorway, `kam_bldg_interior_pod_room_a`): stalled bots a
  median 10.7 m off the line where moving ones are 4.4.
- Kamino round hub 46 (Hub53, r 15.0, the maintainer's spot; legs 46->44 / 42 / 45, 43->46): the arc's line runs through
  `kam_bldg_pod2` itself; bots slide round the pod and some stop dead for a second.
- Bespin `122->7` and `-1->122` at (-4, 99, -162): 316 of the 394, a wall of `bes1_bldg_main02`, 142 backed up.
- Bespin deaths off the floor in those snapshots: 17, on legs 58->59 (7), 51->61 (3), 35->4 (2, BESR's own leg) ...;
  all 17 units had a way shut within 1.5 m in their last 15 s (a weak test: stalls are common there - trace them).

The mechanism (card HUBP's own text + BESR's trace): a bot walks at its OWN point of the next hub's disc (the PS2's rule,
verified), then walks the next leg at its projection on the leg's line advanced by speed + 0.2 m (6f6160b, the PS2's
rule). From a point metres off the centre of a wide hub, that first step is a diagonal at the line - and where the leg is
narrower than the hub (a walkway, a door, a ramp between rims) the diagonal meets the rail, the door frame or the rim.
BESR's unit 14: "STOP Barrier at 1.00 m, then STOP Wall ... 9 s later it is LOST, walks over the rim and dies".

The PS2 does this less: bots inside a soldier barrier KAM1 0.80 % against the port's 2.19 % (OBST2's report). Nobody has
taken the PS2's rate of THIS stall.

## Notes
- `files:` is EMPTY on purpose while this card is `measure` (2026-10-11): listing route.rs / mover.rs / avoid.rs / nav.rs held it out of the picker behind OBST2's parked branch for a whole run. The build half (MOUTH2, or this card re-phased) lists them: `engine/crates/core/src/ai/{route,mover,avoid}.rs`, `engine/crates/core/src/nav.rs`.
- BESR is `defer: post-alpha` and Bespin is not a showcase map; Kamino is, and this is the same defect. BOTV should
  not re-take its table before this lands (planner: add MOUTH to BOTV's deps).
- OBST2's open items (barriers walked by their corners every step) touch the same code; its last form jittered in
  The maintainer's scene. Read `loop/reports/20261009_1413_card_OBST2.md` and `..._1725_...` first.
- Split when the measure is done: MOUTH2 (build) if the rule is more than a session.
