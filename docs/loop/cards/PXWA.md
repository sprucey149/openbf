---
id: PXWA
title: PCSX2 walkabout, BF1: capture the PS2 at every command post of every BF1 map at the port walkabout's stand, eye 1.7 m, level pitch, bearings 0/90/180/270, 0 AI, teleported by RAM; PS2|port pairs, parked-vehicle pairs (finishes VPXC1), findings -> cards
milestone: M1
value: 5
phase: capture
status: done
deps:
files:
declares:
needs:
branch:
resume: done 2026-09-29: all 15 BF1 maps / 88 posts / 352 views; F35-F45; ELFR INTL BESL KFIRE carded; VPXC1 closed
created: 2026-09-28
---

## Why
The maintainer (INBOX 2026-09-28, value 5, M1): the walkabout (`engine/artifacts/walkabout/`) shows parts of maps the goldens never
frame, and some maps look like they have gaping holes (UTA1 first). Only a PS2 comparison at every post finds them;
eyeballing is not evidence.

## Notes
- Created 2026-09-28 by the planner, split per game from the maintainer's one card. Read `pcsx2/README.md` and
  `ref/COMPARISON_RULES.md` first.

## Done 2026-09-29 (lane 2, max) — report `engine/artifacts/loop/reports/20260929_1210_capture_PXWA.md`
- **Coverage:** all 15 BF1 port maps, all 88 posts, 4 bearings = 352 views at the walkabout's own eye (`pcsx2/walk_bf1.py`),
  every one paired with the port at the PS2's exact pose (`port.py <map>/walk`) and every per-post sheet viewed
  (`pcsx2/ref/<map>/walk/pairs/<stem>__sheet.png`; `walk/index.md` per map). Eye error: 263 of 352 views < 5 cm, 35
  at 5-30 cm; where the post console or the ground holds the chase camera off, 0.3-1.0 m (16 posts) and 2.3-4.7 m (5
  views: YAV1 tflank b090/b270, GEO1 cp14 b000, RHN2 cp7 b180, YAV2 cp3 b270) — recorded per view; the port frame is
  always rendered at the PS2's actual eye, so every pair is like-for-like.
- **0 AI:** BF1 has NO single-player AI-count option (only `ifs_mp_gameopts` has `numbots`); `SetUnitCount`'s store
  patched to 1 per team in the IA menu state (0 crashes the load), the bots held 300 m up -> every pad parked.
  Recorded in `pcsx2/README.md`.
- **Findings** F35-F45 (`pcsx2/ref/FINDINGS.md`). Holes: none on BF1 beyond ELFR's alpha cases; KAM1's census holes
  were the pre-KOC sea, YAV1 temple is dark on the PS2 too. New cards: **ELFR** (5, build), **INTL** (4), **BESL** (4),
  **KFIRE** (3). Notes on SKIPC, LEAF, TMSK, FARC, SKYO, BFC, VREST, VTURD, LREG, SUNG, WTHR. **VPXC1 closed** (pairs +
  RAM tables for 213 vehicles on all 15 maps, `lead_tmp/pxwa/veh/`).
