---
id: PXWB
title: PCSX2 walkabout, BF2: capture the PS2 at every command post of every BF2 map at the port walkabout's stand, eye 1.7 m, level pitch, bearings 0/90/180/270, 0 AI, teleported by RAM; PS2|port pairs, parked-vehicle pairs, findings -> cards (holes first)
milestone: M1
value: 5
phase: capture
status: done
deps:
files:
declares:
needs:
branch:
resume:
created: 2026-09-28
---

## Why
The maintainer (INBOX 2026-09-28, value 5, M1): the walkabout (`engine/artifacts/walkabout/`) shows parts of maps the goldens never
frame, and some maps look like they have gaping holes (UTA1 first). Only a PS2 comparison at every post finds them;
eyeballing is not evidence.

## Notes
- Created 2026-09-28 by the planner, split per game from the maintainer's one card. Read `pcsx2/README.md` and
  `ref/COMPARISON_RULES.md` first.
- 2026-09-29 (UTAS, lane 2): **UTA1 con_cp2 is done** — `ref/bf2_uta1/cp4_con_cp2_b{000,090,180,270}` (+ port, pairs, states
  kept), flat names in `ref/bf2_uta1/` so `port.py` / `pairs.py` work unchanged. **The BF2 teleport exists:**
  `lead_tmp/re/uta1-sinkhole/ps2exp/capture_bf2.py EX EY EZ TAG [bearings]` holds the player soldier (position E+0x150,
  velocity E+0x4c0 zeroed, every 2 ms) so the chase camera's eye lands on the walkabout eye to < 5 cm, measures the stick
  directions/rates, and grabs a state per bearing (~1 min per post). E is the kept state's player (0x1c87180 in
  `bf2_uta1_cp3_con_cp1_postA_spawn.p2s`); for another map find it the same way (the ramcensus2 soldier nearest the
  camera axis, E+0x150 = feet). The battle kept running (no 0-AI setting used) and nobody shot him in 4 bearings.
- 2026-09-29 (PXWA, lane 2): **the BF1 twin is done and is the template** — `engine/artifacts/pcsx2/walk_bf1.py` (menus ->
  a patched no-AI battle -> the player held at each post's eye, the camera's air offset measured first so the post
  console's pull-in is recorded, not fought -> a savestate frame + pose per bearing, the mission name checked after the
  load), `port.py <map>/walk` (takes a subdirectory now), `pairs.py`, `walk_sheet.py`, `walk_index.py`, the detached
  driver `lead_tmp/pxwa/batch.sh` (ci.lock per map). For BF2: swap in capture_bf2.py's E+0x150 / E+0x4c0 hold and use
  BF2's real AI-count option (the playlist's OPTIONS); 6-11 min per map with menus, ~45-55 s per post (BF1, measured).
- 2026-09-29 (PXWB session 1, lane 2): **the BF2 tool is built and running** — `engine/artifacts/pcsx2/walk_bf2.py` (0 AI by
  the game's own Conquest Options "AI Units Per Team" = 0, menu path in `pcsx2/README.md` "The PCSX2 walkabout, BF2";
  the player's CapturePosts byte cleared so no post flips; velocity E+0x550, not E+0x4c0; feet clamped at the stand),
  driver `lead_tmp/pxwb/batch.sh <maps>`, vehicles `lead_tmp/pxwb/veh2.sh` + `veh_table2.py` + `veh_pairs.py`. Done and
  viewed: TAN1, UTA1, GEO1, TAT2, YAV1 (FINDINGS F46-F51; new card BPNT; notes on LEAF, ELFR, SKYO, SKIPC, FOGN, LSUM,
  GLOW, WADE). No holes.
- 2026-09-29 (PXWB session 1, end): **88 of 99 posts, 351 views** on 16 BF2 maps (TAN1 UTA1 GEO1 TAT2 YAV1 DAG1 DEA1 END1
  FEL1 HOT1 KAM1 KAS2 MUS1 MYG1 NAB2 complete; COR1 cp0_cp5 b000-b180 only; POL1 none), every sheet viewed, no holes
  anywhere (port <= 1.3 % clear-black at every PS2 pose). FINDINGS F46-F61; new cards BPNT (4), VPAD2 (3), MUSW (3); notes
  on LEAF ELFR SKYO SKIPC FOGN LSUM GLOW WADE VTUR2 FOLG TDTL SEGU KFIRE HWAT SUNG LREG WTHR LOD. Vehicle tables
  `lead_tmp/pxwb/veh/<map>_table.md` (GEO1 UTA1 YAV1 END1 FEL1 HOT1 KAS2 MYG1; the rest have no pads). Report
  `engine/artifacts/loop/reports/20260929_1835_card_PXWB.md`.
- 2026-09-29 (PXWB session 2, lane 2): **DONE — all 99 posts of the 17 BF2 maps, 396 views**, every sheet viewed, no
  holes (port ≤ 1.3 % clear-black past the explained TAT2 cantina / TAN1 space views). POL1 (24 views, 5 CHECK) and COR1
  (24, 0 CHECK, 0 deaths without the air hold) captured and viewed; vehicles NAB2 4 / POL1 3 matched (xz ≤ 2.1 cm).
  FINDINGS part 4 = F62 (POL1 / COR1 windows opaque: MTRL 0x04 the census bakes OPAQUE) -> new **ELFR2** (M1 4,
  build, deps ELFR); F63 (POL1's `dusteffect` mist, 10 port maps place such volumes, no port code) -> new **DUSTE**
  (M1 3, re); F64 notes on LEAF, GLOW, SUNG, SKIPC (+ KFIRE, ELFR pointers). `port.py` now re-renders a port frame
  older than its PS2 frame (a `--skip-done` rerun had left COR1 cp0_cp5's three stale). Report
  `engine/artifacts/loop/reports/20260929_1855_card_PXWB.md`.
