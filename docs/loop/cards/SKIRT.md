---
id: SKIRT
title: BF2 Kamino hub 46: the PS2's soldier takes the pod's skirt (48 degrees, 1.0-1.5 m high) on momentum and three rules of its contact code the port lacks - the port's body makes 6 of its 14 climbs and the steering keeps the bots off the band (52 stalls per 100 passes, the PS2 5): build the flattest-normal rule, the dead zone and the overlap, the ledge rule's height and a steep probe that opens for a short band (RE done: re-soldier-ground 7; prototype 20 per 100, the maintainer's scene 3 -> 0)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Build result (2026-10-15, lane 1; branch `skirt` 2dccd5a on main 662e152; kit `lead_tmp/skirt/tools2`, README there)
Built: (a) the flattest normal, (b) the dead zone, (c) the overlap's lift-off in `Soldier::resolve_ground` (both games),
the ledge rule's kerb test inside the capsule's footprint (`walls::top_in_disc`; the prototype's second half, the ledge
asked at the floor under the axis, changed no walk and no bar: dropped), and `steer::short_band` - the steep face's OWN
slope reaches a holdable floor within 1.6 m, standing (n.y > 0.6) all the way (TAT1's ramp 0.57 / Geonosis's buttress
0.53 stay walls; a bot meets the skirt at a slant, so the march goes up the face, not along the bot's way).
1. Walk table: 14 of 14 (main 6), none of r182/obl/bot41/nhalf/footN, r179 61.83. Time ratio 0.5-1.25 on 11 of the 13
   run-ups; n122 0.49 and eastlong 0.46 are under - the PS2's own runs hung there (1.70 / 2.10 s; the 8-in-18 phase).
2. Tests: `game/tests/skirt_climb.rs` (3, baked Kamino), 3 soldier tests + the steering's short band; 11 mutants each
   fail one (`tools2/mut.py`, `mut/mut.log`; M10 after the slant case was moved, the lift-off mutant's code went).
3. Hub 46 (8 seeds x 3 min, `out/main662.txt`, `out/c3.txt`): main 662e152 209 / 427 passes = 49 per 100 -> 60 / 408 =
   15; steep slide 66 -> 0, open fwd at the foot 77 -> 20 (on 6dc59d4); Kamino P-stalls 448 -> 328. The maintainer's scene: main
   0 of 408, branch 1 of 425 (a fighter in a hold strafing on the skirt's west corner, #31 t651800).
4. All maps: fidelity 8 seeds PASS (recycles 48 -> 49, world deaths 1.475 -> 1.399 %, noprog 21 -> 14); 16 more seeds
   (`sw/ab_main`, `sw/ab_c3`): rounds 512/512 both, recycles 102 -> 111, floor 36 -> 33, lost 39 -> 36; tat1 + both
   Geonosis recycles over 24 seeds 77 -> 75; tat1 over 96 fresh seeds 260 -> 261, Mygeeto region deaths 72 seeds 27 -> 29.
   All-map stall table (`all_null.md`, `all_c2.md`, 6dc59d4): S 2518 -> 2435, P 8909 -> 8778; BF2 Hoth S 16 -> 45
   (unique events 11 -> 25; bunker1 at (-207, 191) 9 -> 14: the short band, a lead, not traced to its end).
5. Leak census new 0 on 32 maps; sites 13973 -> 14319, floor class 4714 -> 5321 (one traced: a 6 mm offset carries the
   run past a corner main wedges in, to a site main's own runs cross 12 times).

## Why (rewritten by the RE pass, 2026-10-12; the card as filed is kept below)
The maintainer, 2026-10-11, of BF2 Kamino: bots "miss ... go to the side of it, try running at the side for a few seconds, then
go back a few feet". Hub 46 (the pod, the spot of his own scene) holds 228 of the map's 485 stalls: 52 per 100
passes against the PS2's 5. **The RE is done** (`analysis/re-soldier-ground.md` 7, main 808406a) and it corrects the
card: the PS2's N on the band IS the face's (N.y 0.6665) and its x 0.1 binds. The PS2's soldier takes the band (1.03
to 1.50 m of 48 degrees) on momentum, helped by three rules of its contact code that the port lacks; from a stand at
the band's foot he does not get up either. The port's body is not a wall everywhere: with a run-up it makes 6 of the
PS2's 14 climbs (the band up to about 1.2 m high) and misses 8. And the bots are held by the steering first: 94 of
the 228 stalls push at the band from a stand at its foot, 60 slide along it because `steer::steep_ahead` calls it a
wall, 38 are bots on the dais walking at a point through its rail (card DAIS, not this card).

## Evidence (`engine/artifacts/lead_tmp/skirt/`, README.md)
- The 24 walks, PS2 beside port: `tools/table.py k0 k3 k8 k11 k11m32` - main makes 6 of the PS2's 14 climbs; rules
  (b)+(c) 12; rule (a) alone 14; all three 14 (and 4-5 climbs the PS2 does not make, all from a stand near the foot).
- Hub 46 per stack, 8 seeds x 3 min (`out/<tag>.txt`; per 100 passes / Kamino P-stalls / the maintainer's scene): main 52 /
  485 / 3; ledge 40 / 432 / 4; ledge + no probe 31 / 426 / 6; (b)+(c) + ledge + no probe 23 / 349 / 2; all three +
  ledge + no probe **20 / 378 / 0**; with `hold_above` instead of no probe 29-35. The body rules alone, probe on: 49-53.
- No probe at all: tat1 + bf1_geo1 + bf2_geo1, 8 seeds, recycles 9 -> 26 (BF1 Geonosis 0 -> 10); `hold_above`: 12.
- The riser line `r179` (x -17.9): main's feet stop at 60.65, with the ledge bit 61.80-61.83, the PS2 61.84.

## Notes
- The mid-climb loss of the floor (the sign of a 1e-3 m/s residue decides it, re-soldier-ground 7.2) and the
  sphere foot itself (feet 0.45 m over the band, the early touch) are NOT ported, on purpose.
- Leg `56>51` at (22.7, 51.2) is still open and is not this card.

---
# The card as filed by MOUTH2 (2026-10-12) - its premise ("the port's cannot", "a steady 3 m/s") is corrected above

## Why
The maintainer, 2026-10-11, of BF2 Kamino: bots "miss ... go to the side of it, try running at the side for a few seconds, then
go back a few feet". Card MOUTH2 (2026-10-12) traced what is left of that at hub 46 - the pod, the spot of his own
scene (-4.0, 60.3, 108.6): 228 of the map's 485 stalls (8 seeds x 3 min after MOUTH2's door rule; 52 per 100 passes
against the PS2's 5). It is not the route. The pod stands on a dais (floor 61.6) inside four rail walls; each wall has
a skirt outside it whose lowest 1.2-1.3 m is one planar face at 48 degrees (the port's mesh normal n.y 0.667), with
four diagonal ramps between. The corners of the four rail barriers lie ON the skirts. **The PS2's soldiers walk up
the skirt; the port's cannot** - bot or player. For the player it is an invisible wall on a showcase map.

## Evidence (`engine/artifacts/lead_tmp/mouth2/`, README.md; `analysis/re-soldier-ground.md` 6)
- **PS2** (`lead_tmp/mouth/ps2/bf2_kam1_m{2,3}_units.csv`, 10 Hz): 30 ascents from the ground to over 61.9 in 16
  min. `m2` k 27 from a STAND 1.7 m off the foot: y 60.46, 60.61, 60.84, 61.03, 61.32, 61.50, 61.86, 62.04 - a steady
  1.3-3.2 m/s over the ground, +1.4 to +2.9 m/s up, no slowing toward the band's top. About 3 m/s along the slope is
  BF2's uphill target at FULL hold for 48 degrees (7.0 x (1 - sin^2) = 3.1). Its corner nodes are on the skirts and
  its bots stand on them (y 61.0-62.5 within 1 m of (-17.3, 96.5)).
- **The code says no**: BF2 FUN_002480a0 gives control x 0.1 when N.y (S+0x4b4) < 0.7 (B2:163053) and its clamp
  window then takes at least 0.21 m/s off the uphill speed every update - no steady climb on a 0.667 face. So the
  PS2's N on that band is not 0.667, or something else is. [H], unread.
- **The port, three faults in a row** (scratch switches `MOUTH2=<bits>` in `mouth2/scratch_knobs.patch` on 5fbbbd4;
  binary `mouth2/bin/rto_k6`, null = main):
  1. the steering's steep probe (card STHOP, `steer::steep_ahead`) calls the band a wall: the bot slides along the
     skirt's foot and is left at its end pushing at it, `stop open, steer fwd, v 0.4` (20 stalls at (-7.0, 99.0)
     walking at the corner (-8.7, 98.2); `mouth2/tools/stallspots.py`).
  2. with the probe off (bit 2) a bot at 7.0 m/s is stopped in ONE tick: the wall pass asks its LEDGE rule with the
     full capsule at the height the feet had before this tick's floor is found, so the riser under the face being
     climbed (two vertical triangles, normal (0.896, 0, 0.445), (-18.36, 60.30, 109.76) .. (-17.44, 62.16, 107.90))
     pushes him 0.10 m sideways and takes 6 of his 7 m/s (`TRACE_PHYS=18,1056,1060`, seed 60).
  3. with the ledge asked at the floor under the axis (bit 32) he keeps the plane's projection (-2.90, 3.01, -3.28)
     and gets over the skirt's corner; head on he rises 0.94 of 1.3 m and slides back (x 0.1 hold).
- Hub 46's stalls by variant: main 256, door rule 216-228, + bit 32 193, + bits 32 and 2 142 (35 per 100). The maintainer's
  scene, trails over 6 reversals: 5 / 3-5 / 6 / 10 of about 410 - no variant cures it, the probe off makes it worse.

## Notes
- Depends on MOUTH2 only for the numbers' base. `files:` is a first guess (`sim/step.rs` and `ai/steer.rs` likely).
- The same ledge fault must be on every map with a riser beside a slope: after the build, `mouth/tools/table.py`
  over all 32 maps before / after.
- Leg `56>51` at (22.7, 51.2) (card MOUTH's note: the plan's corner lies at `kam_bldg_ramp1`'s wall; 17-34 stalls
  in 8 seeds x 3 min) is NOT this and is still open - untraced; a planner's note, no card.

## Note (planner 2026-10-12 23:40): the maintainer's scene today, by landing - read before using the scene as this card's bar
- The scene (`lead_tmp/doors/tools/scene_seeds.sh`, seeds 60-67, trails over 6 reversals / trails within 20 m), run by
  me on the sessions' kept binaries: main ac13ca6 (MOUTH2) **3 of 403**, worst 9; main 4dbb460 (+ DEFS) **4 of 397**,
  worst 9; main bc10b82 (+ CLSE4) **8 of 431**, worst 21; main 4aaa2a2 (+ DOORS) 8 of 431 (DOORS's own run).
  Mine: `lead_tmp/planner_1012f/scene_ac13ca6`, `scene_4dbb460`; DOORS's: `lead_tmp/doors/before/scene_base`.
- The rise came with CLSE4 and it is NOT route loops: the four worst trails on bc10b82 (t673600 #31 25 reversals in
  a 0.2 m box; t663000 #18 11 in 2.3 x 2.4 m; t653000 #20; t671800 #30) are all `does fight`, "holding a position"
  on leg 46->44 with the parked player (#0) as the enemy at 6-12 m, a hold goal re-drawn every 2 s
  (`doors/bin/bbread <snapshot> --unit <id>`, cwd `openbf-wt/doors-main-scratch/engine`). CLSE4's bursts and pauses
  make a fight at the idle human last longer; the scene tool counts a fighter's strafe steps as reversals.
- #31 is this card's business: for the whole 10 s it "walks at its hold goal" 2.3-2.6 m away ((-16.4,60.3,107.7)
  from (-16.1,60.3-60.5,110.2)), `steer fwd`, 0.6-0.8 m/s, and stays in a 0.2 m box with its height bobbing
  60.3 / 60.5, `stuck 0.0 s stall 3 s`: a FIGHT spot it cannot reach. Likely the pod's skirt [H, not traced] -
  check that the skirt rule also carries a fighter to a hold goal on the dais, and that a hold goal is not drawn
  where the body cannot go.
- So for this card: split the scene's trails by `does` (travel / recover against fight) before quoting it, and judge
  the skirt by the travel trails at hub 46 plus fighters like #31. The side-steppers are noted on card CLSE5.
