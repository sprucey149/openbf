---
id: LANE3
title: A third research lane: first time PCSX2's software renderer vs HW on one savestate (≤ 4 levels, CPU use), then LANE_PREF lane 3 (re, audit, capture), a gpu.lock / ci.lock / pcsx2.lock split, and a per-session XDG_CONFIG_HOME PCSX2 ini — the maintainer starts the lane himself
milestone: M0
value: 5
phase: build
status: done
deps: LDOC
files: scripts/loopctl.py,scripts/gates.py,engine/artifacts/pcsx2/pcsx2_session.py,engine/artifacts/pcsx2/pcsx2_capture.sh,engine/artifacts/pcsx2/README.md
declares:
needs:
branch: lane3
resume: LANDED d5df318 2026-09-30; artifacts side (pcsx2_session.py, pcsx2_capture.sh, README, ctl.sh) edited in place, backups in lead_tmp/lane3/backup
created: 2026-09-30
---

## Why
The maintainer's INBOX note 2026-09-29 22:40 (approved loop improvements), item 4. Research and capture are about 62% of spend and the queue's bottleneck. Three things stop a third lane
today:
1. `LANE_PREF` has only lanes 1 and 2.
2. Every PCSX2 run holds the one global `ci.lock`, so a RAM-only research session blocks the GPU gates.
3. `lead_tmp/re/pxc2/ctl.sh` edits the GLOBAL `~/.config/PCSX2/inis/PCSX2.ini` in place, so two sessions would fight
   over it.
Expected cost: about +(cost omitted)/day at current rates. The maintainer starts it with `LANE=3 scripts/autoloop.sh`. The loop never
launches autoloop.sh. deps LDOC only because both edit loopctl.py.
