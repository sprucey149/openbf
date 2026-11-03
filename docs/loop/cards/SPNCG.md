---
id: SPNCG
title: Wire card SPNC's spawn-node census into the gates: a 'spawn' census line (spawn_node_census, 7 s for 32 maps) that fails on a NEW failing node not in spawn_node_census_accepted.txt, so a change to start_hub / level_hub / the floors / the bake that strands a spawn node is caught at the gate
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card SPNC's acceptance asked for a census line in the gates if the run is under a minute; it is 7 s for 32 maps. RULES:
a card that asks for a new gate lands the instrument and its accepted list and files the wiring as its own card.

## Evidence
- `engine/crates/game/examples/spawn_node_census.rs` (branch `spnc`, 9f688b1; lands with SPNC) and
  `spawn_node_census_accepted.txt` beside it (6 nodes, each with its lives on main). Last line:
  `spawn_node_census: 32 maps, 1455 nodes, failing before 9 after 6, accepted 6 new 0`.
- Control: deleting one accepted line prints `NEW failing node: bf1_end1 cp5 node 8` and `new 1`.

## Acceptance
- `gates.py --auto` runs the census when a file it reads can move (core/src/ai/route.rs, core/src/sim/{bounds,spawn}.rs,
  core/src/nav.rs, the bake) and FAILs on `new` > 0; a main run reads `new 0`.
- Mutation: one accepted line removed -> the gate fails naming the node.

## Notes
- The census is a screen, not a verdict (SPNC's report): 1 of its 3 "before" failures and 0 of its 6 "after" failures
  stranded bots on main. A NEW node gets its lives read (`lead_tmp/spnc/tools/nodelives.py`) before it is accepted.
