---
id: MUSS
title: BF2 MUS1: a hard seam between the lava backdrop and the sky, in a STILL frame
status: archived
issue: 17
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #17](https://github.com/sprucey149/openbf/issues/17).

## Why / Evidence / Acceptance
See the original entry below; the first session to work this card rewrites this section into short Why / Evidence / Acceptance lines and fills `files:` and `declares:`.

## Original QUEUE.md entry (verbatim, 2026-09-25)
MUSS. [TODO — ADDED 2026-09-17 (lane 1) by OCB's landing, from **The maintainer's flythrough report** item 2; [RE] first (what ARE the two layers?), then a build. The RE half is FREE, but the fix is odds-on to touch `game/src/viewer/` (since SPL1: the `scene` part's sky shells or the `look` part's far fade) and/or `render/`, which **L** (`lowr-sheet`) owns — check L's status before building.] **BF2 MUS1: a hard seam between the lava backdrop and the sky, in a STILL frame** [GOLDENS likely] — the maintainer: `engine/artifacts/walkabout/bf2_mus1/cp3_cp2.png` shows a sharp horizontal line where the lava-glow backdrop meets the smoke/sky above it, plus near-wall lava that reads flat and uniformly bright. **This is NOT the known flythrough-camera artefact** — it is in a static walkabout frame with a soldier in shot, so the 2026-08-31 "the flythrough is the wrong instrument for a short-fog map" explanation does not cover it. Already on record and NOT the same thing: the `mus1_sky_volcano` DOUBLE-BAND seam was root-caused in `analysis/re-sky-dome-placement.md` and its placement fix landed 2026-09-03 (`a937199` / `f01f45d` / `840a701`) — that fix was about the SKY SHELL's vertical placement hiding its seam below the ground horizon, a different boundary from the one in the maintainer's shot. Note `analysis/re-lowr.md:164` lists **BF2 MUS1 among the maps with no heightfield**, so whatever sits under the sky here is not a terrain LOWR sheet. Build, in this order: (1) [RE] §5 — identify what the two texture layers in that shot actually ARE (sky dome? a separate far-layer sheet? the lava backdrop plane?), whether the 2026-09-03 placement math covers their boundary, and whether the lava's flatness is an authored LUT/lighting choice (as the water's dark LUT turned out to be) or a missing animation/emissive pass — every citation's enclosing `FUN_...` named, absence claims searched, ONE opus verifier agent; (2) only then the port fix. GATE: a before/after capture pair at the exact `cp3_cp2` vantage showing the seam gone or explained; if a committed golden moves, quantify mean/max/changed-fraction BEFORE viewing, build the pair sheets and view them; ci +N for any new test; the sweep pair, the three replay hashes and `--sim-hash 100` UNMOVED (render-only) and no rebake unless the RE says the bake drops a layer. Files: `analysis/re-sky-dome-placement.md` (or a new `analysis/re-mus1-backdrop.md`), `analysis/RE_LOG.md`, scratch under `engine/artifacts/lead_tmp/re/mus1-seam/`; the build half's files are the RE's to name.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).

`acceptance: needed` (planner 2026-09-30): no measurable acceptance yet (a parity delta, a leak-census count, a sim replay hash or a pixel test). Write one before the card is picked.
