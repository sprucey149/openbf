---
id: TURR
title: Turrets with a purpose: a bot mans a turret that has a target it can hit or that covers a front-line post, and gets out by five rules - no hittable target for 10 s, quiet, hit from outside its arcs, the endgame, the fight moved (the maintainer's annoyance 1; bot design 6.1)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's annoyance 1 (2026-10-02): "If there is an empty turret, they'll get in the turret and rarely leave unless they're being shot at or die ... the last few bots to kill are hiding somewhere sitting in a turret ... there should be more purpose." The PS2's rule and why it camps: `analysis/re-soldier-ai.md` 7, 10a (boarded with a target in an arc; left after 10 s with no remembered threat; an enemy turret 100-180 m away resets the clock; 15 of 32 seated spells on Kamino over 60 s). The port's bots never board a stand-alone turret (0 % seat time on Kamino and Polis Massa against 9.4 % and 18.0 %). Design: `analysis/bot-design.md` 6.1 (D11).

## Notes
- Cut from `analysis/bot-design.md` (card BOTD, 2026-10-03). Waits for the maintainer's read (deps BOTD). In BOTV's deps.
- Endor's scripts set `SetStayInTurrets` (SAIR 7.1): bots board with no target there. Not copied; the defence clause covers a turret beside an owned post.
- A vehicle card with no runnable check is parked (prompt, 2026-09-30): the scene tests above are the checks - write them first.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).** Decision 4: no garrison - a manned turret is the post's defence, so the five exits must not strip a front-line post of its last gunner while it has a target.
- 2026-10-05 planner: `effort: high` (the builds the maintainer's top concern rests on; a build runs at medium by default). Undo: `loopctl.py set TURR effort=`.

- 2026-10-07, from VTAT's RE (`analysis/re-turret-ai.md`, ab64c9c): (1) on the PS2 a bot that sits down gives the
  SEAT its own controller (UnitStationaryController: wait / combat / unboard only); a fight from a turret is always
  "stand and shoot" and is opened only on a target a weapon has inside MinRange..MaxRange, kept to MaxRange + 10 m.
  (2) A building gunner's idle look is a point within 30 deg of its base direction, re-drawn 1 think in 18
  (75 deg on a vehicle's turret). (3) The stick: BF2 the hover law (u·|u|, k = 4.5 - clamp(MaxTurnSpeed, 1, 3)) for
  a turret over 0.5 rad/s, BF1 always the deadbeat law the port uses. (4) The trigger is card VTAT's build (the dish
  turret's charge, taps): land VTAT before this card's B8 census, or Hoth's dish turret stays silent in a bot's
  hands. (5) The PS2's self-piloted turrets (card VSELF) are not something a bot boards: `PilotType self` seats stay
  unboardable.
- 2026-10-12 17:05 planner, from the maintainer's INBOX note of 16:00 ("bots do NOT man building turrets at all today ... The maintainer wants
  bots manning turrets with a purpose, so the planner should raise TURR's priority after DEFS and DOORS"): **`deps:`
  BOTD,TEAM -> BOTD,DEFS**, so this card no longer waits for TPOS and TEAM (two large builds). What changes in "What to
  build" 1: there is no Team board yet, so this card builds the offer itself - the purpose test is asked for each free
  turret every 30 ticks and the nearest free foot bot within 50 m takes it (capacity 1 a turret, 2 a post), in
  `ai/ride.rs` (`claim_seat` refuses a vehicle with no pilot seat today: that is the first line to change) and
  `ai/behave.rs`; the "enemy seen near this post in the last 30 s" test is DEFS's (same post-contact question as its
  rule 3 - use its function). TEAM's board takes the offer over when it lands. `files:` lost `ai/team.rs` (it does not
  exist). Undo: `loopctl.py set TURR deps=BOTD,TEAM`.

- Planner 2026-10-12 19:30: DEFS is on main (2e12961), so this card is free. Its defence clause calls
  `commander::post_threatened(sim, team, cp, threat_radius(sim))` (DEFS's report, Next step). The visual half
  (the turret turning, its gunner drawn) is card VTURD, raised to 5 by the maintainer's note of 18:55; neither waits for the other.

- 2026-10-16 06:21 build (lane 2, report `reports/20261016_0621_card_TURR.md`): BUILT on branch `turr` (33016bf + the census's
  hits/kills), gates `lead_tmp/turr/gates`: every gate PASS (goldens unchanged, sim hashes unchanged, arena identical,
  leak identical) but the behaviour gate (fidelity) refused three values - sorted below. Not landed.
  Results against the acceptance:
  - six scene tests asked (one per exit + the unreached seat): 13 scenes in `core/src/ai/man/tests.rs`, 28 mutations all
    killed (`lead_tmp/turr/mut/mut.py`, run1.txt + run2.txt).
  - B8 BF2 KAM1, 8 seeds (`lead_tmp/turr/proto/final`): seat share 8.37 9.79 7.72 8.57 9.63 8.71 8.98 9.59 % (mean 8.92,
    PS2 9.4, bar 3-10: 8/8); spells over 60 s with no shot in their last 20 s 0; seated in the endgame 0 ticks; 8/8 rounds.
  - BF2 COR1: the port's conquest places NO turret there (`turret_ride bf2_cor1`: 0 pads, 0 turrets): its 11
    `tur_bldg_laser` are build zones that start unbuilt (`vehicle_pads::add_turrets`, card VTUR2) and its three
    `tur_weap_built_gunturret` (near (0, 11, 7)) are not spawned by the con layer. Nothing to man, so the bar holds
    trivially; "the outside turret the maintainer named" is a buildable one if it is any.
  - Sweep 24 seeds x 32 maps: 768/768 rounds both builds. Kills a minute: the 12 maps where no turret was manned are
    identical seed for seed (the control); the 20 turret maps fall 7.6 % on every 8-seed block (main 246.3 / 256.5 /
    251.5, branch 236.7 / 223.6 / 236.6) - NOT within the noise. CAUSED by the card. Per map -0.8 % (BF1 BES1) to
    -20.8 % (BF1 GEO1), BF2 HOT1 +1.5 %. Hypothesis [H]: a seated gunner is rarely killed and BF2's turrets hit little
    (KAM1 seed 1: 16 hits, 2 kills a round; MYG1 / KAS2 never fire - card TAIM, filed today).
  Fidelity sort (rule 0): (1) `offgraph_pct@bf2_kam1` 4.41 -> 13.06 and `offgraph_pct` (all) 6.25 -> 8.21 - CAUSED, BY
  DESIGN: the shares sample counts riders at their seat and KAM1's turrets stand off the soldiers' graph. Same 8 seeds
  with a foot-only counter (`lead_tmp/turr/offg`, rto_foot2 = the branch + the counter): all 13.06 % = foot 5.78 % + seated
  7.28; of the foot share boarding 0.88, within 12 m of a turret 1.50, elsewhere 3.40 (main's whole value 4.41 / 4.35 on
  its two blocks). (2) `recycled` (all) 20 -> 33 - NOISE: three blocks main 20 / 36 / 34, branch 33 / 41 / 23
  (`lead_tmp/turr/fid`, gates/fidelity*); BF2 GEO1's 0 -> 8 at seeds 1-8 is 24 -> 20 over 48 seeds (`lead_tmp/turr/geo`).
  Decisions taken (reversible): the defence clause asks for an enemy the TEAM KNOWS within 60 m of the post (not card
  DEFS's 30 s threat record: with it 7 of 8 KAM1 rounds went over 10 % and 64 % of spells never fired - `proto/base`
  against `proto/seen`); a bot walking to its turret does not stand to fight within 25 m (it stood 0.4 m out of reach);
  exit 5 is implied by exit 1 at the card's numbers (a hittable target is a purpose) - kept, tested on set clocks.
- 2026-10-16 planner (1016b) - **TURR's blocked sort judged.** (1) offgraph@bf2_kam1 / offgraph agg: CAUSED by design, as the
  build said - 7.28 of the +8.65 points are gunners counted in their seats (`lead_tmp/turr/offg`). Not an approval: the
  metric is fixed instead (new card FOFG, value 5, M0: the share counts units on foot, the seated share printed beside it);
  this card waits for it (`deps`). (2) recycled agg 20 -> 33: NOISE (main 20 / 36 / 34, branch 33 / 41 / 23; GEO1 main 24
  / branch 20 over 48 seeds) - the same refusal as HJAM's, the maintainer's question 37 (FIDA). (3) kills a minute -7.6 % on every
  block: CAUSED - and visible: Mygeeto's (showcase) gunners sit and never fire, the maintainer's annoyance 1 in another form. So the
  card holds until they fire: card TAIM is merged in as step A (resume). The maintainer's question 56 offers him the other way.
  TPOS no longer waits for this card (the order dep had done its job; TPOS's table lists turret seats without this code).
  Undo: `set TURR status=blocked deps=BOTD,DEFS`, `set TAIM status=todo`, `set TPOS deps=BOTD,FGHT,OBST,TURR`.
- 2026-10-16 18:22 build (lane 1, high; report `reports/20261016_1825_card_TURR.md`): STEP A DONE, branch turr rebased on
  87ad3b9 + 79b5544; ci green (1633 / 0 / 46); gates NOT run (STOP at 18:19; the run started 18:21 was stopped before a
  row). Five faults, all fixed: the camera ray hit the turret's own building (the PS2 excludes the owner entity, B1:138064
  / B2:189959); the aimer laid the pivot's line, not the barrel's, on the point (PS2 RAM `myg1_lg`, 342 rows: 0.004 m off
  the barrels' line, 3.5 m off the pivot's; `lead_tmp/turr/aim/ps2/barrel_line.py`); a building turret's muzzle was turned
  by its bind heading; a bot's aimer took the camera ray (the PS2 hands an AI seat the AI's aim, B1:191963-191967); the
  gunner saw from the camera's eye (inside Geonosis' mesh, behind Kashyyyk's tower) - now from its muzzle, past its own
  building (ours). And it picks its target inside the arc (ours). Seeds 1-8 (`lead_tmp/turr/aim/c8_before` ->
  `c8_final3`): spells with a shot MYG1 3.7 -> 70.4 %, KAS2 2.4 -> 61.9, GEO1 14.2 -> 70.9, KAM1 57.5 -> 70.9; a hit in 8/8
  rounds on each. B8 KAM1 seat share 6.97 % (bar 3-10), 0 quiet long spells, 0 endgame ticks. Kills a minute, 21 moved
  maps, 3 blocks vs main: -2.0 / +1.1 / -2.1 % (main's own blocks 623.8 / 608.9 / 621.6); 11 control maps identical
  (`lead_tmp/turr/sw`). Scenes 6 new + 2 extended, 39 / 39 mutants killed (`lead_tmp/turr/mut/run4.txt`). The maintainer's Q56
  ("take the cost or hold") is moot: the cost is gone.
- 2026-10-19 13:45 land (lane 2; report `reports/20261019_1345_land_TURR.md`): LANDED b19cdf9 on top of TPOS (ce8975d). The
  branch was rebased by hand after TPOS merged (behave.rs: both sides' blocks kept; CRATES.md counts recommitted); ci 1659 / 0 / 46.
  Rebased tree, 8 seeds: KAM1 seat share 6.81 %, 0 quiet long spells, 0 endgame ticks; MYG1 shots in 75 % of spells
  (`lead_tmp/turr/land_c8`). Gates: only the declared sweep moved; fidelity PASS vs TPOS's landing.
