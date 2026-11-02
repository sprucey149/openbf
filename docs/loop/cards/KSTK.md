---
id: KSTK
title: BF2 Kamino: bots stand next to / on the rail beside a walkway instead of walking it (kam1_bf2_2230_ai_stuck_on_rail.png)
milestone: M3
value: 5
phase: re
status: done
deps:
files:
declares:
needs:
effort:
branch:
resume: DONE by BNDS 9440823 (Fable, 2026-10-01) - see the note at the foot; close unless the maintainer's next play shows bots on the hulls
created: 2026-09-30
alpha: yes
---

## Why
The maintainer's BF2 Kamino play: bots (droids) do not use a path — they stand next to it. In the frame two droids are on top of / against
the dark rail strip beside a drawn walkway rather than on the walkway, and the player's droid stands on the rail too. This is the
AI-side half of Kamino's "ramps and paths" trouble; the render-side half is KRMP (undrawn ramp floors). GRND (the one-ray ground
query falling through slits) and STLVL (level-aware arrival) may be related; STHOP changed the walking rules today.

## Evidence
- <repo>/engine/artifacts/lead_tmp/playtest_0930/kam1_bf2_2230_ai_stuck_on_rail.png: two droids on the black rail at about x 720-1010, y 240-640, the walkway at the right
  (x 1100-1900) empty. The maintainer says they are "just getting stuck next to a path".
- RE (re-kamino.md §4.2): the rail IS walkable on the PS2 (feet 79.106 over the 0.57 m rail) and the port's soldier hops there;
  §4.1: the wall-box tops along bridges and ramps are a walkable level 0.44-0.77 m above the drawn rail. A bot steering along
  a hub edge whose line crosses that rail may be lifted onto it and then be unable to step down onto the walkway.
- Method: the stall census (STALL/SWPN tooling: `scripts/sweep_seeds.py`, STUCK_CENSUS) on BF2 KAM1 over 8 seeds, list the stuck
  units' positions and the planning-graph edge each is on; reproduce one with `ps2live`-style numbers: where the bot's feet y
  sit versus the walkway and the rail top.

## Notes
Filed 2026-09-30 22:50 from the maintainer's third BF2 Kamino play session (three frames, `lead_tmp/playtest_0930/kam1_bf2_2230_*.png`); a claim in Why/Evidence marked as reading of the image is Claude's, not measured.

- Fable 2026-10-01: the cause was the port's hub ARRIVAL (at the authored radius; the PS2 arrives at the waypoint or past its plane, RE_LOG 'Arrival and locate'), not the rail's collision. f293fd2: `brain::hub_reached`, `route::walk_point`, `route::approach_leg`. KAM1 0/8 -> 1/8, region deaths 367 -> 117 over 8 seeds. Evidence and what is left: `lead_tmp/kamai-fable/README.md`.

- Fable 2026-10-01 (second session): BNDS 9440823 - the rim is the disc's own BARR barriers (759 on KAM1, a ring round every platform); a barrier with the level's edge behind it is now a wall for the steering, the mission's DisableBarriers calls are replayed (its doors), and a bot off the level walks back or is recycled after 45 s. Four 10-minute rounds: off the level 10.9 % -> 0.9 % of unit-seconds, walking in place 16.2 % -> 5.1 %, nobody under y 51. KAM1 0/8 -> 4/8 at BNDS. Evidence: `lead_tmp/fable2/README.md`. What is left is not this card's: slow endgames (C7) and KAM1's low kill rate.
