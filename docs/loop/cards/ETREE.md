---
id: ETREE
title: BF1 Endor: bots end up INSIDE the hollow tree end_prop_tree01 at (170, 0) and are recycled there - 5 recycles in 32 rounds on main 17ae28e (10 of 37 in 72), a bot inside the trunk for 120 s with the steering's stop at 0.00 m on the tree: find how they get in (a gap in the trunk's collision, a spawn, a push), then close it
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Filed by card OBSV (2026-10-12). The Imperial bots that walk north from hub 0 to hub 54 pass this tree; some end up
inside its trunk (`end_prop_tree01`, body 516, a mesh, AABB x 168.1..171.5, z -0.7..2.7) and stay: `steer back`,
`STOP Wall at 0.00 m body 516`, recover 20-29, stall 120 s, recycled. It is on main. It is also what stops the cure of
BF1 Endor's turret orbit from landing (card OBSV2): the bots that cure frees walk on to this tree (5 -> 18 recycles here
in 32 rounds, the map's 37 -> 82 in 72 seeds).

## Evidence (engine/artifacts/lead_tmp/obsv/)
- `bb/a/bf1_end1_s9.json` + `bb/a/u21.txt`: MAIN's rules, seed 9, tick 10560, unit 21 at (169.9, -4.0, 0.0) - the tree's
  middle - for its last 36 s (`blackbox_read --unit 21`).
- `dth/e0_*.err` (main, seeds 9-80, every recycle with its spot): cell (168, 0) has 10 of the map's 37.
- `dth/e4_*.err` (the walk-point rule, seeds 9-40): 18 of 36 there.
- See also memory note doorway-stall-can-be-a-collision-bake-defect: dump the trunk's baked collision first.

## Acceptance
- How a bot gets in, traced on one life (the tick it crosses the trunk, from where, on what pick) and named: a gap in
  the baked mesh, a spawn or dismount point inside, or the wall pass.
- With the fix: 0 recycles within 6 m of (170, 0) over BF1 Endor seeds 9-80 (main: 10); the map's recycles not over 37.
- 2026-10-13 05:02 lane 1 (card HVYW, a trace met on the way; not this card's tree): TWO MORE trees of BF1 Endor hold bots, and here
  the way in is read: `end_prop_tree01 @ (-47.1,-2.9,155.0)` (black-box body 50) and a spot at (-25.8,-4.1,145.2). Seed 6, unit 5
  (branch hvyw 548ef04 binary `lead_tmp/hvyw/bin/rto_br9`, snapshot `lead_tmp/hvyw/bb/bf1_end1_s6.json`, 4 Hz rows
  `lead_tmp/hvyw/sw/clean9/bq/bf1_end1_s6.csv`): the bot walks to BOARD vehicle 6 (kind 0, a hover: the speeder bike) that stands at
  the tree, is seated at (-47.0,-4.2,154.6) at 241 s - inside the trunk - rides it 10 s without leaving a 1 m circle, gets out at
  250.5 s (the first `stuck` event) and stands in the trunk: `recover` with `stop wall body 50` for 122 s, recycled at tick 11173.
  So: a bike wedged at a tree, and a rider who gets off inside the tree's body. Counts over 6 blocks of 8 seeds (48 rounds each,
  `lead_tmp/hvyw/sw/main*` against `clean9` + `br9_b*`): recycles at these two spots 2 on main (seeds 10 and 14), 11 on the HVYW
  branch (why more is NOT traced; a guess: bikes are shot at less, a rifle is not fired at a vehicle past 10 m); the card's own tree (165,0) 9 on main, 7 on the branch.

- 2026-10-13 05:14 planner (found while judging HVYW's refusal; NOT a tree): a THIRD Endor spot holds bots - cp2's hub, against
  `end_prop_boulder @ (-105.5,-1.4,253.3)` (black-box body 481), the leg `last->2 dest 2` with its point 1-3 m off. On main
  fb4a2f1 it is 2 recycles in 2 of 144 rounds (seeds 74, 75); on branch hvyw 27 in 4 of 144, 20 of them one round (seed 91:
  both teams' living units there for 500 s; snapshot `lead_tmp/planner_1013a/bb/bf1_end1_s91.json`, count
  `lead_tmp/planner_1013a/boulder.py`). HVYW's next session sorts whose fault the pile is; if the point at the boulder is main's
  own it becomes a card of its own (or this card's third spot).

- 2026-10-13 05:46 lane 1 (card HVYW, the boulder sorted): the boulder is main's own and is now **card VMESH** (value 5) - a
  vehicle gets inside a closed prop mesh (a bike through the wall at 23 m/s, an AT-ST's centre into the edge) and
  `Sim::exit_point` puts its rider down inside: 131 of 131 stays inside the boulder over 576 rounds began with a dismount, none
  by walking in. The two trees of the note of 05:02 ((-47, 155) and (-26, 145): a bike at the tree, the rider off inside the
  trunk) are that door and go with VMESH. THIS card's tree at (170, 0) is not read yet: before any other trace, run its stays
  through the same question (`lead_tmp/hvyw/tools/inboulder.py` with the tree's mesh in place of `boulder_mesh.py`: is every
  stay a dismount?) - if yes, close this card into VMESH.

## Notes

- 2026-10-15 14:10 lane 2 (card VMESH LANDED b8a634d): the trunk at (170, 0) is `end_prop_tree01`, an OPEN shell (10 edges used once); `CollisionWorld::enclosing` reads a point at its centre as inside it, so since VMESH no rider is put down inside it (exits inside a body 0 on every map, 8 seeds) and a bike bailed out of there as stuck is not re-boarded until it moves. Its way IN is not closed (VMESH's keep-out is for watertight bodies only, `Sim::shut_in_solid`): that part stays this card's.

## Cloud preflight 2026-10-24 07:41 UTC
Blocked before measurement on main 3d487d9: baked maps/common/classes/vehicles, mission RONs, OBSV/HVYW/VMESH raw evidence and local iso/extracted source are absent. No mesh dump, life-entry trace, current-main baseline or seeds 9-80 acceptance ran. VMESH source guards remain, but how this tree is entered remains unmeasured. Restore inputs and follow resume before any collision fix. Report: `reports/20261024_0741_card_ETREE.md`.
