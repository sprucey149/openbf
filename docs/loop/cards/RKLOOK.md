---
id: RKLOOK
title: Rockets look wrong: the CIS rocket trooper's rocket is drawn purple and looks incomplete; capture PCSX2 frames of every rocket (each team and class that carries one, both games) and match the port's draw (model, trail, glow, colour)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer 2026-10-28: "the rocket bullet at least from the droid ... the coloring is not what the original is. This one is purple, and doesn't seem complete ... get a screen capture of the pcsx2 rockets of different teams and classes".

## Evidence
- Read first: `re-bolt.md` (the ordnance draw), card FXB, RKEX (rockets explode).
- PCSX2 tools: `engine/artifacts/pcsx2/` (session tool `pcsx2_session.py`); run under gpu.lock.

## Acceptance
- A sheet of PCSX2 frames: each rocket in flight (BF1 and BF2, each side's rocket class, the SBD wrist rocket, the jet trooper's), beside the port's.
- The draw fixed per mismatch, the golden(s) viewed and quantified.
