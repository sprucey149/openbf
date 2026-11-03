---
id: TPOSC
title: Positions at posts: Capture and Guard take a reserved position of TPOS's table in or beside the post's region with cover toward the approach (else card PARR's drawn point), and the PS2's at-post wander moves them to another 8-14 m off every 20-30 s (bot design 4.4; split from TPOS as its item 5)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card TPOS item 5, split off 2026-10-16 (lane 2): the Tactician's fight slice is complete without it, and it does not move the bars TPOS is judged on. `analysis/bot-design.md` 4.4: Capture asks for "a covered position in or beside the region facing the approach, else the drawn point; another within 8-14 m every 20-30 s [P 3; BF1: Patrol hints, 30-45 s]"; Guard for "a position of the post's set facing the nearest enemy post's approach". Today a capturer walks to card PARR's drawn point (`route::post_point`) and a defender to an authored Cover / Snipe hint (`commander::hint_spot`); neither is reserved, so two can stand on one spot.

## Evidence
- Close teammates (within 1.5 m) by behaviour, 32 maps x seeds 1-8, TPOS branch 8f620ce (`lead_tmp/tpos/fid4/fidelity/botq`): capture 2 % of all close samples (8.0 % of capture time), guard 0.5 % (2.6 % of guard time); travel 61 %, fight 31 %. Main (`lead_tmp/fidelity_rebase_20261016a`): capture 8.2 %, guard 1.4 % of their own time.
- BF1 Naboo seeds 9-16 on the branch: capture 20.9 % of its time with a teammate within 1.5 m (one block; seeds 1-8 3.4 %).

## Acceptance
- Capture / guard time with a teammate within 1.5 m at most half of main's (8 seeds x 32 maps, both seed blocks); B12-style share of capture + guard time at a position with cover toward the approach printed per map.
- Rounds 256 / 256; post captures a minute and finish ticks within the seed noise (24 seeds if a map moves).
- Unit tests, mutation-checked: two capturers at one post hold positions at least SPACING apart; a guard's position has cover toward the nearest enemy post; the wander moves 8-14 m after 20-30 s.
