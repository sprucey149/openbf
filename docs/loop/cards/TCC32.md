---
id: TCC32
title: Does a 32-bpp texture reach the GS with TCC=0 — i.e. is `re-material-flags.md` §2's "TEX0 TCC=1 for every texture in both games" too broad?
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why / Evidence / Acceptance
See the original entry below; the first session to work this card rewrites this section into short Why / Evidence / Acceptance lines and fills `files:` and `declares:`.

## Original QUEUE.md entry (verbatim, 2026-09-25)
TCC32. [TODO — ADDED 2026-09-17 (lane 1) by WA's RE, which turned it up; [RE], FREE (disc + `analysis/` only). LOW value: no shipped water sheet and no shipped model sheet known to be 32-bpp is affected today, so this is a doc-accuracy job that could become a render one.] **Does a 32-bpp texture reach the GS with TCC=0 — i.e. is `re-material-flags.md` §2's "TEX0 TCC=1 for every texture in both games" too broad?** [RE] — `analysis/re-water-alpha.md` §6 [V][O]: in the TEX0 assembly at BF1 `FUN_0049c5c8` 562037-562038 / BF2 `FUN_00582c90` 717184-717185 the 32-bpp branch ORs CLD but NOT `0x400000000`, while `re-material-flags.md` §2 cites 717400-717470 for "every TH case ORs 0x400000000" on the 16/32-bit path — two different sites in one builder, and only one of them can be the one a 32-bpp sheet takes. Method: `OPUS_LOOP_PROMPT.md` §5 — find which branch a 32-bpp texture actually reaches (the *(decompiler detail omitted)* path at 561737-561944 is a third), name the enclosing function of every line, read the branch bytes, and census which shipped textures are 32-bpp (`mungefs::texture` says BF1 COMMON.LVL effects are: lightning/boom/wing blur). Then either write the erratum into `re-material-flags.md` §2 or prove the doc right. GATE: the answer names the branch a 32-bpp sheet takes with its bytes, plus the count of shipped 32-bpp textures per bake; no code and no gate value moves in this entry. Files: `analysis/re-material-flags.md`, `analysis/RE_LOG.md`, `engine/artifacts/lead_tmp/re/tcc32/`.]

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 1 (kept).

`acceptance: needed` (planner 2026-09-30): no measurable acceptance yet (a parity delta, a leak-census count, a sim replay hash or a pixel test). Write one before the card is picked.
