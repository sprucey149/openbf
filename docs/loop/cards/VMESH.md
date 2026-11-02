---
id: VMESH
title: A vehicle gets INSIDE a closed prop mesh and its riders get out inside it - BF1 Endor's cp2 boulder (and two trees): a speeder bike crosses the boulder's wall at 23 m/s, an AT-ST walks its centre into its edge, Sim::exit_point puts the rider down inside (131 of 131 stays), nobody sees him, the vehicle is boarded again through the wall and when he is his team's last the whole other team heaps at the boulder with no shot (main seed 74; 500 s on the HVYW branch's seed 91) - close the way in, the exit and add a lost rule for a unit inside a body
milestone: M2
value: 5
phase: build
status: done
deps: VTURD
files: engine/crates/core/src/sim/seats.rs,engine/crates/core/src/sim/vehicles.rs,engine/crates/core/src/vehicle.rs,engine/crates/core/src/vehicle/hover.rs,engine/crates/core/src/ai/stall.rs,engine/crates/core/src/ai/ride.rs,engine/crates/core/src/ai/behave.rs,engine/crates/core/src/ai/state.rs,engine/crates/core/src/collision/rays.rs,engine/crates/core/src/collision/shape.rs,engine/crates/core/src/collision.rs
declares: sweep
needs:
effort:
branch: vmesh
resume: landed b8a634dc 2026-10-15
created: 2026-10-13
alpha: yes
---
## Why
Found while sorting card HVYW's Endor pile (2026-10-13; the planner's note of 05:13). It is on MAIN. BF1 Endor's boulder at
cp2's hub (`end_prop_boulder @ (-105.5,-1.4,253.3)`, black-box body 481) is a CLOSED collision mesh (146 triangles, every edge
shared by two; world box x -112.9..-101.7, y -4.9..3.4, z 245.3..258.6) with the terrain's floor inside it. Four faults in a row
make it a trap, each main's own:
1. **A vehicle gets inside.** A speeder bike crosses the wall at speed (main, seed 74: from (-113.2, 250.7) outside to
   (-104.8, 255.9) inside in one half second, 23 m/s) and then bounces about inside for as long as it is ridden; an AT-ST walks
   its centre into the mesh's low edge and stands wedged there (main seed 51: class 1, a walker, 5,985 hp, from (-111.1, 241.2)
   to (-110.5, 246.1) in 2 s; the HVYW branch's seed 91 at (-111.5, 245.6)).
2. **The rider gets out INSIDE.** `Sim::exit_point` (`core/src/sim/seats.rs`) tests each of its nine candidates with a ray from
   the vehicle's centre; with the centre inside a closed body every candidate inside passes, and the fallback is the centre.
   131 of 131 stays inside the boulder over 576 rounds began with a dismount (none by walking in, none by a spawn).
3. **Nobody sees him and he sees nobody.** Sight rays stop at the hull both ways: 0 of 1,922 unit rows near the boulder had a
   target in 60 s while 15 Rebels stood round one Imperial (main, seed 74). He contests the post from inside, so the other
   team's `last->2 dest 2` leg never ends: recover counts to 47, stalls to 106 s, recycles.
4. **The vehicle inside is boarded again through the wall**, by bots outside and by the ones already inside, before its 20 s
   idle decay starts (about 20 s empty, seen) - each ride ends in another exit inside. And the last resort does not always end
   it: a tick "at its post" closes the stall watch (`ai/stall.rs` `watch`'s `arrived`, from `ai/mover.rs`), and the boulder
   lies in cp2's capture region - unit 14's watch went 2,794 -> 0 on a `capture` tick (branch, seed 91, tick 29565) and unit 5
   stood inside for 806 s.
When the unit inside is its team's last, the whole other team heaps at the boulder with no shot: main seed 74 (tickets 42 / 1,
alive 16 / 1, the last kill 100.8 s before, ended by the survivor's recycle at 120 s); HVYW branch seed 91 (500 s).
Card ETREE's two trees at (-47, 155) and (-26, 145) are the same door (its note of 2026-10-13: a bike wedged at the tree, the
rider gets off inside the trunk) - this card takes them; ETREE keeps its own tree at (170, 0) until its way in is read.

## Evidence (`engine/artifacts/lead_tmp/hvyw/boulder/`, tools in `lead_tmp/hvyw/tools/`)
- `main_s74_entry.txt`, `main_s74_rides.txt`, `main_s74_vehicle.txt`, `main_s74_pile.txt`: MAIN's binary
  (`lead_tmp/hvyw/bin/rto_main_obs`, build fb4a2f1), seed 74 - the bike in at 353.5 s, 50 s of riding inside, the rider out
  inside at 404.8 s, the empty bike's decay (389 hp -> lost at 430.5 s), the pile at 524 s. Snapshots under `rto_main_obs/`.
- `inboulder_144.txt`, `inboulder_288.txt` (`tools/inboulder.py`: a stay = 5 s or more on foot inside the mesh, by ray parity
  1 m over the feet, 4 Hz rows of whole rounds; `tools/boulder_mesh.py` checked against 7 snapshot rows, 3 in and 4 out):

  | BF1 Endor, seeds 1-288 | rounds with a stay | stays | unit-seconds inside | longest | recycles at the boulder | rounds over 2 recycles |
  |---|---|---|---|---|---|---|
  | main fb4a2f1 | 12 | 20 | 1,859 | 221 s | 8 in 6 rounds | 0 |
  | branch hvyw 548ef04 | 16 | 111 | 9,159 | 806 s | 41 in 9 rounds | 4 (seeds 73, 91, 189, 236) |
- `inboulder_rules.txt`: build 4's switches in one binary, seeds 1-144 - every rule off 6 stays / 593 s (= main, the same six
  rounds); the vehicle rule off 10 / 642; all on 19 / 2,326. `doorfeed_288.txt`: why - in the 60 s after a first exit inside,
  enemies on foot within 40 m 1.74 a door on main and 0.28 on the branch, the riders' own side on foot within 30 m 0.79 and
  1.56: with HVYW the side that holds cp2 stands there unchallenged and keeps boarding (exits inside 1.7 -> 7.1 a door).
- `rto_br9/bf1_end1_s91_t6900.json`: the AT-ST case tick by tick (unit 14 boards at 5835, sits 17.5 s in a walker that does
  not move, is put out at (-110.6, -3.7, 247.8) inside, `STOP wall body 481` from then on).

## Notes
- 2026-10-13 07:09 planner (lane 2): **milestone M3 -> M2** (the way in, the exit and the boarding are vehicle code) and **`deps: VTURD` for
  ORDER only** (the maintainer's turret ask goes first on lane 1; by dry run lane 2 takes this card as its first build). Undo: `loopctl.py set
  VMESH milestone=M3 deps=`. The claim was judged by a verifier told to refute it (`lead_tmp/planner_1013b/verdict.md`): the fault
  is main's. **Read these before step 1 - they change what to build:**
  - **The two worst rounds are not the bike.** Branch seeds 91 and 236 (65 of 114 exits inside) are a WALKER standing still at the
    boulder's south-west corner (-111.5, 245.6) that is boarded by pair after pair, each put out inside after about 17 s: 31 and
    40 stationary ride spells. **Main has the same loop** - seed 227, vehicle 4, 19 spells, 11.4 boards per 100 s against 11.5 and
    14.9 - with the seat at (-108.9, 247.3), 2 m OUTSIDE the mesh, so the stay census did not count it
    (`lead_tmp/planner_1013b/verifier/loops2.py`). So there is a FIFTH fault and it needs no mesh: a bot boards a vehicle that
    cannot move, is put out, and boards it again. Step 4's "no boarding without a line" does not end it at seed 227 - add: a
    vehicle a bot was put out of as stuck is not boarded again by that team until it has moved (trace seed 227 on main first).
    Acceptance, added: boarding loops of 4 or more stationary spells within 8 m of the boulder's box 0 in Endor's seeds 1-288
    (main 1, the HVYW binary 5).
  - **Fault 4's "the stall watch does not end it" is shown on the HVYW binary only** (stays of 583-806 s, seed 91, `capture` rows
    inside); main's longest stay is 221 s. It is main's code by reading (stall.rs and mover.rs are not in HVYW's diff) - write the
    scene test first and see it fail on main.
  - **Do not build on "enemies on foot 1.74 -> 0.28"** (Evidence, above): three of main's 12 doors are team-1 riders whose "enemy"
    is the heap itself; over the whole time a door is open it is 0.63 against 0.71. And `inboulder_rules.txt` is build 4, not the
    measured build (its all-on run shares no round with `e_br9`): which HVYW rule feeds the door is not known.
  - Why a bot drives a walker left at (-112, 242..245) the last 1-4 m into the corner (3 rounds of the HVYW binary, none of
    main's 288 at that point; main's walker riders are put out inside in seeds 51 and 227) is measured, not traced.
  - The measured HVYW binary `lead_tmp/hvyw/bin/rto_br9` is stamped bea783c+dirty, built 6 minutes before 548ef04: rebuild from
    the branch head for the acceptance run. `bin/rto_main_obs` is main fb4a2f1 plus the read-only census observer.
- 2026-10-13 09:25 (card HJAM, lane 2): **a third door, and the size of it on main.** `end_prop_tree04 @ (-180.4, -0.1, 325.4)`
  at the Rebel spawn is a hollow trunk 8.7 m across (29 triangles, NOT closed: 15 edges used once, open below - a face
  overhead, none under; `lead_tmp/hjam/tools/tree_mesh.py`, `tr/tree04_parity.txt`). A bike enters it at 31 m/s (main +
  OBSV, seed 15, tick 540-570), the unstick manoeuvre does not move it, the driver bails at `stuck_run 180` and gets out
  inside; recycled 126 s later. So step 3's "back face overhead" test covers it, a "closed mesh" test would not.
  On MAIN 5f8f569, BF1 Endor seeds 9-80: **28 of 31 recycles are a driver recycled within 10 m of where he bailed out of a
  wedged bike, in 19 of 72 rounds** (`lead_tmp/hjam/tools/exdrv.py dth/c_off`; cells (168, 0) 14, then five trunks with 2).
  With HJAM's driver fix (branch `hjam` 7af3660, bikes no longer circle detour corners) 59 of 63, cells (-24, 144) 34 and
  (48, 24) 14: Endor's recycle count is this card's fault count, and any change to how bikes drive re-draws the trunk.
  The switch binary `lead_tmp/hjam/bin/sw4` prints every bail (`HJAM_CENSUS=1`: `HJO ... bail true`) - a ready census for step 1.

- 2026-10-15 note from card HJAM (lane 1): BF1 Endor seeds 17-112, main 1cf5c8b 40 recycles ("stalled"), HJAM's branch b7b2062
  82 - bikes that no longer circle a detour corner ride more and more of them end in a hollow trunk (on main 28 of 31 Endor
  recycles over seeds 9-80 were riders recycled within 10 m of where they bailed, `lead_tmp/hjam/tools/exdrv.py`, README).
  HJAM's Endor bar is this card's count: re-measure it with both on. Binaries `lead_tmp/hjam/bin/main2`, `bin/fix2`;
  `lead_tmp/hjam/fidsort.md`.
- 2026-10-15 11:07 lane 2 (land HVYW): HVYW is on main (3fba385) - this card's acceptance is now measured on main itself.
