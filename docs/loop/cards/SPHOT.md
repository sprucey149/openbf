---
id: SPHOT
title: A white specular hot spot on BF2 Kamino's lower landing pad seen from above, where the PS2's pad is mid grey
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why / Evidence / Acceptance
See the original entry below; the first session to work this card rewrites this section into short Why / Evidence / Acceptance lines and fills `files:` and `declares:`.

## Original QUEUE.md entry (verbatim, 2026-09-25)
SPHOT. [TODO — ADDED 2026-09-25 (lane 1) by PXC batch 2 (FINDINGS F19); **[RE] FIRST**; FREE; LOW-MEDIUM value] **A white specular hot spot on BF2 Kamino's lower landing pad seen from above, where the PS2's pad is mid grey** [RE] — evidence `engine/artifacts/pcsx2/ref/bf2_kam1/pairs/cp5_cp5_post5_{v4_sea,v5_sea}.png` (`_v5_sea` box (150,20,560,200): PS2 (102,107,111), 0 % of pixels ≥ 235 vs port (148,153,155), 9.4 %); the main floor at eye level matches ((55,60,66) vs (57,62,64), `_v1`), so it is view-dependent. The port's specular pass: `analysis/re-specular-pass.md` (SPEC, SPEC2). RE (§5): first the port-side split — the pose with the specular draws dropped (the scratch renderer's `PXC_DROP_LAYERS`; confirm which `DrawLayer` the specular pass uses); if the hot spot goes, read the PS2's specular term at a steep view; else find the term. GATE: the term named with its bytes (§5, [O], verifier), or the finding closed with the with/without pair. Files: `analysis/re-specular-pass.md`, `analysis/RE_LOG.md`, scratch `engine/artifacts/lead_tmp/re/sphot/`.

- Planner 2026-09-29: likely the same family as DSHN (the port's specular pass too hot vs the PS2). Let DSHN's RE of
  the specular colour/power go first, then re-render this pad before opening its own RE.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).

`acceptance: needed` (planner 2026-09-30): no measurable acceptance yet (a parity delta, a leak-census count, a sim replay hash or a pixel test). Write one before the card is picked.

## Acceptance (planner 2026-09-30 17:30)
The pad box at the FINDINGS F19 pose reads within ±10 levels per channel of the PS2's mid grey (no channel ≥ 240 in the box). KAMR (M1 5, alpha) lists every Kamino difference per post: if it already names this hot spot's cause, it takes it and closes this card MERGED.
