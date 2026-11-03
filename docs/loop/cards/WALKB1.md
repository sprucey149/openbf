---
id: WALKB1
title: BF1-map walkers take BF2's handling with BF2's class values for the walker body (the kaadu: the tauntaun's handling at its own 19.54 m/s; e0c 0 on the spider, AT-AT and AT-TE) - today BF2's code reads BF1's data: a walking AT-ST turns 3.4 deg/s, the left stick cannot steer a tauntaun and the mounts run at half speed
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why (the build; rewritten by the RE session, 2026-10-07)
On BF1 maps the port runs BF2's walker code with BF1's class data. That is neither game (`analysis/re-walker.md` §13.8,
measured with `walker_drive` on main ab64c9c):
- a walking AT-ST, spider, AT-AT or AT-TE turns at most **0.060 rad/s** (3.4°/s): BF2 reads ForwardTurnSpeed, which BF1
  never read and left at its default 0.03. BF1's own is 1.0 / 0.2 / 0.1 / 0.2 rad/s, walking or standing;
- the left stick cannot steer a tauntaun, kaadu or AT-TE (YawLimits 0 / 0: 0.20, 0.34 and 0.00 rad in 4 s of stick), and
  the AT-TE's aim runs away from its hull (−8.28 rad after 5 s of right stick);
- the tauntaun and the kaadu run at half speed (10.2 and 10.5 m/s) with their feet sliding 8 m/s; BF1's own are 19.77
  and 19.54 m/s.

**The call (re-walker §13.9; FEEL_NOTES 2026-10-07):** PLAN locks BF2 handling, so on a BF1 map the walker BODY takes
BF2's class values.

| BF1-map class | body values | full stick after the build |
|---|---|---|
| imp_walk_atst, _snow, _jungle | BF2's class of the same name | 5.0 m/s, boost 10.0 for 4 s; camera-relative walk |
| all_walk_tauntaun | BF2's | 20, boost 30 |
| cis_walk_spider | BF2's, e0c = 0 | 3.0 |
| imp_walk_atat | BF2's, e0c = 0 | 2.0 |
| rep_walk_atte | BF2's, e0c = 0 | 2.5 |
| gun_walk_kaadu | its own + what BF2's tauntaun changed for handling; MaxSpeed = its BF1 gait z·30/N | 19.54, no boost |

- "Body values" = what `WalkerClass` hands the integrator: MaxSpeed, BoostSpeed, the three turn speeds, TurnThreshold,
  the six flags, e08 / e0c / e10 / e14, the body's YawLimits and PitchLimits, the energy props, MaxTurnSpeed,
  MaxYawSpeed, MaxPitchSpeed, LegRayHitLength, roll, WalkerWidth, StompThreshold. Per class and prop, BF1 against BF2:
  `lead_tmp/walkb1/body_diff_out.txt`. Everything else stays BF1's (health, weapons, seats, turret sections' limits,
  sounds, collision prims, the model, the clips — the clips are the same on both discs).
- e0c = 0 on the spider, AT-AT and AT-TE: it feeds only §4.5 C's factor, which a human never reaches; with BF2's value
  their bots crab 5° off the goal (BF2's own behaviour), BF1's bots converged.
- The kaadu: StoppedTurnSpeed 0.5, ForwardTurnSpeed 0.8, YawLimits ±90°, PitchLimits −35 / 40°, flags 0, 1, 3, 4, the
  angles 87 / 88 / 89°, StompThreshold 0.5; MaxSpeed from its own walk clip (z·30/N = 19.544); BoostSpeed 0.
- A table in `game/src/vehicle_class/walker.rs` keyed by class name, applied when the class's game is BF1 (and to BF2's
  own unfielded kaadu), is the smallest form; a data-gated test pins it to the baked BF2 class RONs so it cannot drift.
- One switch for the maintainer's other way (BF1's pace under BF2's handling): MaxSpeed = z·TEMP_AnimationSpeed·30/N per class
  (AT-ST 6.48, AT-AT 2.22, AT-TE 2.80) and no boost. Default off.

## Evidence
- `analysis/re-walker.md` §13 (BF1's own walker, replayed on the PS2: 29,241 of 29,241 steps), §13.8, §13.9; report
  `reports/20261007_0849_card_WALKB1.md`.
- Before: `lead_tmp/walkb1/port_bf1_<class>.txt` (8 classes). Targets of the same script on BF2's maps:
  `lead_tmp/walkb1/port_bf2_<class>.txt`.

## Original text (the RE phase, 2026-09-29; kept. Its premises corrected by the RE: the AT-AT and AT-TE walk 2.22 and 2.80 m/s on BF1, not 1.48 and 1.86)
### Why (RE)
The VWLK audit (2026-09-29, PARTLY) found two gaps in re-walker.md that WALK inherits on BF1 maps (HOT1/END1 AT-STs,
HOT1 tauntauns, GEO1 spiders, NAB1 kaadus, the AT-AT / AT-TE).
- §13's "BF1" speeds used BF2's (N−1)/30 clip duration. BF1 covers z·30/N: AT-ST 6.48, tauntaun 19.77, spider 2.97,
  AT-AT 1.48, AT-TE 1.86, kaadu 19.54 m/s.
- §4.6's control scheme holds only for the fielded BF2 classes. BF1 classes author none of BF2's flag or angle props.

### Evidence (RE)
- `analysis/re-walker.md` audit banner (3e4c742), `reports/20260929_0945_audit_VWLK.md`.
- BF1 duration and sampling: B1 FUN_00230be0 (0x230d40, frames × 1/30), FUN_0043d898 (phase += dt·30/N),
  FUN_0043da00 (frame ⌊(N−1)·phase⌋). The verifier supported all three.
- Live BF1 HOT1 (`lead_tmp/audit-vwlk/live_bf1hot1.csv`): AT-ST 6.474 m/s over 66 s; tauntaun 18.1 m/s over 40 s
  (the −8 % against 19.77 is [H]).
- Census: `lead_tmp/audit-vwlk/census_bf1.txt` / `census_bf2.txt` (the audit's own entc reader).
- Under BF2's code, a BF1 class keeps the ctor defaults (B2 FUN_00263160): K+0xe18 bits 1, 2, 3; e08 = e0c = e14 = 0;
  e10 = 5.
- BF1 YawLimits: AT-ST ±35°, spider ±45°, AT-AT ±35°; the tauntaun, kaadu and AT-TE author none (0/0).

### Acceptance (RE; met 2026-10-07 by re-walker §13 / §16 and FEEL_NOTES)
- re-walker §13: correct the table (BF1 = z·30/N, with a kaadu row). Add a subsection on what §4.1-§4.5 do with each
  BF1 class's data, for a human and for the AI:
  - bit 2 on: B falls to C;
  - with e08 = 0, B's first test reduces to the stick's side against the aim's;
  - YawLimits 0/0: A chases the aim at once.
  Compare each with BF1's own control: θ = turn·dt·StoppedTurnSpeed (2θ a tick), in place at |fwd|·5 ≤ |turn|.
- Decide per BF1 class and record it in FEEL_NOTES. Options:
  - BF2's code with BF1 data;
  - BF2's class values where both discs share the class (AT-ST, tauntaun, spider, AT-AT, AT-TE);
  - a BF1-gait clock for BF1-only classes (the kaadu: 19.54 m/s natively, 10.86 under BF2's clock).
- Correct WALK's FEEL_NOTES line: the BF1 tauntaun was ~19.8 m/s on the PS2 (live 18.1), not 21.75.

## KAAD audit note (2026-09-30, `reports/20260930_0645_audit_KAAD.md`)
- **On the PS2, unridden kaadus creep downhill. The port's stand still.** [measured]
  - RAM, all 9 NAB1 library states (CW and GCW): the kaadu on flat perma_spawn13 sits exactly on its spawn point in
    all 9. The two on sloped pads (the unnamed pad at (21.25, −75.11) and perma_spawn12) sit 4.8 m and 6.7 m downhill of
    theirs, unturned, in the unridden GCW side-select state. perma_spawn12's position there agrees to 5 cm with the CW
    load. In the 5 later GCW walk states they are 10-21 m off, growing along the sequence.
  - Live PCSX2 (SW, `bf1_nab1_units1_gcw_sideselect.p2s`), each kaadu written back onto its spawn point, unridden:
    - one moves at a constant (+0.0866, −0.0156, −0.0865) m/s for 30 s (0.1225 m/s along (1,−1)/√2);
    - the other at (−0.1901, −0.0386, 0) m/s (along −x), changing to (−0.228, −0.058, −0.041) further on;
    - neither turns, and perma_spawn13's kaadu moves 0.000.
  - Before the reset they ran 0.155 and 0.102 m/s.
  - The port (WALK, 5,400 ticks, both eras): the kaadus sink 0.03 / 0.10 m and stay.
  - Data: `lead_tmp/audit-kaad/slide1.csv`, `slide_reset_B.csv`, `slide_reset_C.csv`.
- **Mechanism [H].** The two directions are grid-aligned, which looks like a sphere pushed off the terrain grid's edges.
  - The kaadu's TerrainCollision lists p_collision (a 0.5 m sphere on dummyroot at y 0.5) beside the two feet.
  - A non-foot terrain contact takes re-walker §9 C5's else branch and then C8's generic push-out.
  - `core/src/vehicle/walker.rs::contacts` builds neither; it has only C7's building spheres.
  - Unread: BF1's own contact code, and whether BF2's one-pair walkers (the tauntaun) slide the same way. Decide here
    whether this is BF1-only or a WALK gap.
- **Live ridden kaadu.** A bot boarded one 10.6 s into a sample. It covered 41.9 m in 2.27 s: 18.5 m/s path average,
  21.5 m/s over a 0.33 s straight (keyframe-stepped). Compare this card's 19.54 m/s native and the port's 10.47
  (walker_drive, BF2's clock).

## RE session note (2026-10-07, `reports/20261007_0849_card_WALKB1.md`)
- The KAAD audit's creep is answered: it is the walker's own contact rule in BOTH games (a body prim in
  TerrainCollision: tauntaun and kaadu), with a cos²(slope) loss on climbs. Not this card: card **WCON**.
- The KAAD note's "live ridden kaadu 18.5 m/s" is an undrawn walker (whole keyframes, the clip's last interval lost at
  most wraps) on rolling ground; the player's own kaadu applies 19.544 m/s of root motion on every tick.
- The port's bots on BF1 maps: with BF2's values the AT-STs and the tauntaun gain flag bit 0 (the AI's strafe branch,
  `ai/walker_drive.rs`); the kaadu too (the tauntaun's flags). Watch the sweep's BF1 walker maps.

## Planner 2026-10-07 08:55 - value 4 -> 5
On BF1 maps a human cannot steer a walker today (the RE's own run: a walking AT-ST turns 0.060 rad/s, the left stick
turns an AT-TE 0.00 rad in 4 s, the kaadu on showcase Naboo Plains 0.34 rad). M2 has had no landing since 10-03; at 4
this build was about the 40th pick. Undo: `loopctl.py set WALKB1 value=4`.

## Build result (2026-10-09, landed eec32d1; `reports/20261009_1216_card_WALKB1.md`)
- Acceptance 2, 3, 4 met; 1 met except two bars. **Slip median < 1.5 m/s is not met and cannot be:** BF2's own
  tauntaun on bf2_hot1 reads 14.44 m/s by that measure (a running mount's median tick has no foot down); the planted
  ticks (10th percentile) read 0.46 (tauntaun) and 1.04 m/s (kaadu). **END1 AT-ST boost 5.83 m/s (bar 6.5)**: the
  ground on its new heading, not traced; the snow class with equal values reaches 7.23.
- Sweep: 8 maps move on all 8 seeds, 24 identical; all 8/8; tat1's wins 4/4 -> 0/8 (the AT-STs turn now).
