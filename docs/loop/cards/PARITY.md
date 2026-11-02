---
id: PARITY
title: A PS2 parity score: re-render the port at every stored PS2 pose and score each pair (16×16 block error, mean luma difference, p99 luma, HUD/minimap/own-body masked) → loop/parity/latest.csv + per-map summary + history; the scoreboard line; a gate for visual landings
milestone: M0
value: 5
phase: build
status: done
deps:
files: scripts/parity.py,scripts/gates.py,scripts/land.py,scripts/loopctl.py,engine/crates/game/examples/pose_render.rs,engine/CRATES.md,engine/artifacts/pcsx2/port.py
declares:
needs:
branch: parity
resume: landed 144b2f0 2026-09-30; baseline history.csv 12.000
created: 2026-09-30
---

## Why
The maintainer's INBOX note 2026-09-29 22:40 (approved loop improvements), item 2. The PS2 reference library has about 700 comparable pairs: the PXWA/PXWB walkabouts (187 sheets, 4
bearings each) and the earlier PXC sets. Every gate says "nothing broke"; nothing says "we got closer". The PXC census
metric (16×16 block error, 18.3 → 11.2 mean on 15 fogged maps) and p99 luma (which caught the droid shine) already
exist as scratch scripts.
