---
id: PAD
title: Scripted pad input on the PS2: drive PCSX2 with a pad script and screenshots through its PINE socket on top of engine/artifacts/pcsx2/, so a measurement can be a scripted action (a full-stick vehicle turn read from RAM is the first user) - headless, isolated, under the GPU lock
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
INBOX 2026-10-21 20:00 item 2. The maintainer's vehicle turn rate (VTRN) needs a full-stick turn per vehicle class with degrees /
s read from RAM; other measurements want the same. Some scripted-pad trials already exist (the roll strip kit under
`lead_tmp/rolls/ps2`, the walk and stand probes): read `engine/artifacts/pcsx2/README.md` and those first and build on
what works rather than beside it.

## Result (2026-10-21, lane 2) - DONE
`engine/artifacts/pcsx2/padscript.py` (README section "A scripted pad action with RAM rows every update"). Pad script
steps timed by the game clock (`KEY:2s|60u|150ms`, down/up, wait, shot, mark), one CSV row per game update (u from the
clock, keys held, body + camera yaw, the game's own stick words, feet, any `--read`), PINE on for the session only:
save the spawned moment to slot 9, load it before every repeat. PINE has no pad message: the pad stays XTEST keys.
Acceptance run (`padscript.py demo-turn bf2`, Coruscant no-AI, SW renderer):
```
rep 0: held 60 updates over 1.969 s, yaw -460.02 deg; rate -233.68 deg/s (steady half -286.19), largest step -9.549 deg, the game's turn stick [1.0]
rep 1: held 60 updates over 1.969 s, yaw -460.02 deg; rate -233.68 deg/s (steady half -286.19), largest step -9.549 deg, the game's turn stick [1.0]
rep 2: held 60 updates over 1.969 s, yaw -460.02 deg; rate -233.68 deg/s (steady half -286.19), largest step -9.549 deg, the game's turn stick [1.0]
turn rate over 3 reps: mean -233.675 deg/s, spread 0.000 deg/s (0.00 %)
```
Ramp: 2.164 deg/update +0.255 per update to a 9.549 deg/update cap (286 deg/s) after ~1 s. BF1 (Naboo units1):
247.36 deg/s, cap 10.327, spread 0.001. Rows `lead_tmp/pad/demo1/`, `demo_bf1/`; `--axis-scale 0.5` gave BF2's
forward stick word 0.3 (`demo2/`). Next user: VTRN (needs a vehicle entity rule).
