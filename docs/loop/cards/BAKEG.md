---
id: BAKEG
title: Gate tooling: a scratch bake of the 'sounds' group without 'classes' recovers fewer sound names (114 vs 464 on TAT1) — scratch_bake.sh must link or bake classes/ whenever sounds is baked, or land.py's main bake check reports every */sounds/names.ron as not reproducible
milestone: M0
value: 5
phase: cleanup
status: done
deps:
files: scripts/scratch_bake.sh,scripts/gates.py
declares:
needs:
effort:
branch: bakeg
resume: landed ff13dbaa 2026-10-03
created: 2026-09-29
alpha: yes
---
## Why
VTUR's landing (2026-09-29) declared rebake=...,sounds,global-sound and land.py's main bake check reported DIFF=34 — every */sounds/names.ron — because the scratch bake's classes/ was empty (the sound-names step reads $OUT/classes). Main's own rebake was right (464 names on TAT1, matching the earlier full-group scratch); the landing was committed on that reading. The next card that declares sounds hits the same false alarm.

## Evidence
engine/artifacts/lead_tmp/vtur/land/main/cmp_bake.txt (34 DIFF, all names.ron); diff of tat1/sounds/names.ron: candidates 813983 / named 464 (main) vs 434090 / 114 (scratch). scripts/scratch_bake.sh:77 creates an empty classes/. Also: ffmpeg was upgraded to 8.1.3 on 2026-09-26, so every OGG re-encodes byte-differently from a pre-09-26 bake; VTUR's landing re-encoded all audio on main (sounds + global-sound), so main is consistent again.
