---
id: KASF
title: BF1 KAS2's far fog is `255,0,0` — is that authored art, or an unset-field default the table read as a colour?
milestone: M1
value: 5
phase: re
status: done
deps:
files:
declares:
needs:
branch:
resume: done — see analysis/re-fog.md §9; follow-ons CLRB, FOGB, REFL note
created: 2026-09-25 (from QUEUE.md)
---

## Why / Evidence / Acceptance
**Why:** BF1 KAS2's port shows a pure-red band the PS2 never shows. Is §6's fog row (255,0,0, 1000..2992) authored art or
a misread default?
**Evidence (ANSWERED 2026-09-28, lane 2, `analysis/re-fog.md` §9 [O]):**
- The row is exact and live: disc `LowResTerrain/FogColor(255,0,0,128)` at KAS2.LVL 0x18f368, and PS2 RAM 0xff0000ff,
  1000..2992.1875, in all 8 KAS2 states.
- It is unique on both discs (3 records; BES1's is dead SkyInfo).
- It never shows: nothing is deep enough at the captured poses (a fog-colour swap moves nothing; a patched 0..400 m ramp
  turns the far water red).
- (a) The port's blend is as specified (t = 0.2346 at 1500 m).
- The red band is the port's CLEAR colour (the fog colour), showing through a gap it draws nothing in. The PS2 clears to
  black.
**Acceptance:** met (§9 with the OPUS RE header, [O] tags, the §6 row CONFIRMED with its bytes, the RE_LOG pointer, one
Opus verifier: 12 SUPPORTED / 3 PARTLY folded / 0 NOT). Follow-ons: **CLRB** (build, the clear colour), **FOGB** (RE, BF1
near models took no fog), and a note on **REFL**.

## Original QUEUE.md entry (verbatim, 2026-09-25)
KASF. [TODO — **NOTE 2026-09-25 (PXC batch 3, FINDINGS F26): Beach Head (cp2), 5 poses: pure-red pixels PS2 0/0/0/0/0 vs port 0/5042/2245/861/3802, always rows 128–136 where the far water meets the forest backdrop; the PS2 shows light teal water there (`engine/artifacts/pcsx2/ref/bf1_kas2/pairs/cp0_cp2_beachhead_v1_sea.png`). 13 PS2 poses at four posts, none red; "never" stays [H] (no pose faces the terrain edge over open sea).** **NOTE 2026-09-25 (PXC batch 2, FINDINGS F23): BF1 KAS2's high ground (Republic, CITY HALL = cp3; 8 poses in the hall and in the bamboo outside): 0 pure-red pixels in every PS2 AND every port frame — but the bamboo hides the far horizon there, so "the PS2 never shows red" stays [H]; the next capture is a clear sea horizon from Beach Head (cp2).** **NOTE 2026-09-25 (PXC batch 1, `engine/artifacts/pcsx2/ref/FINDINGS.md` F2): PCSX2 ground truth at eye level from the docks shows NO red band — at the exact pose of `engine/artifacts/pcsx2/ref/bf1_kas2/pairs/cp5_cp10_dock_one_spawn.png` the port draws 4 rows of grey-white (210,211,215) over 4–5 rows of pure (255,0,0) under the forest backdrop, and the PS2 draws the forest down to the far walkway (0 red pixels; also `cp5_cp10_dock_one_v1`, `cp4_cp11_dock_two_{spawn,v2}`). So question (a) is answered for these poses: what the port shows there is wrong. Still open: whether the PS2 shows red ANYWHERE (the high ground, the terrain's far edge — PXC batch 2 captures a team-2 post), and (b), the bytes. The fix is a build entry of its own after (b).** ADDED 2026-09-17 (lane 1) by OCB's landing, which received **The maintainer's flythrough report** `engine/artifacts/loop/playtest_flythrough_review_20260917.md` item 3 mid-session; [RE] FIRST, then at most a small port fix. FREE (disc + `analysis/` only until the RE says otherwise). **Highest value of the maintainer's four**, because it can invalidate a SHIPPED fidelity claim rather than add a missing one.] **BF1 KAS2's far fog is `255,0,0` — is that authored art, or an unset-field default the table read as a colour?** [RE] — `analysis/re-fog.md:216` gives BF1 KAS2 fog ON, colour **255,0,0**, `1000..2992 (2000, a=128)`, `FarSceneRange 1500`, noted "<=25 % red at the 1500 m cull; far layer only for terrain" — i.e. cited as ACTIVE. The maintainer's flythrough reads it as a strong, close-to-opaque red wall rather than a <=25 % tint. Two questions, and the second is the one that matters: (a) **is the port applying the blend as spec'd** — 25 % at the cull, not 100 %? Measure the actual blend fraction the renderer produces at 1500 m on bf1_kas2 against the table's a=128 / 1000..2992 ramp. (b) **is the RE itself right?** `re-fog.md:209` lists BF1 BES1 with a SkyInfo entry of the EXACT same triplet `255,0,0` explicitly marked **dead** (unused — BF1's real fog source is LowResTerrain, not SkyInfo). The same literal in two BF1 maps, one a known-unused placeholder, is the signature of an unset-field convention (`0xFF,0x00,0x00`) rather than art direction — note the two are different SOURCES (BES1 SkyInfo vs KAS2 LowResTerrain), which is itself the pattern to test. Method: §5 verbatim — read **KAS2's raw LowResTerrain bytes off the disc** with `mungefs`, state the offset formula, and name the enclosing `FUN_...` of every citation; do NOT re-read the existing table and call that confirmation. Census the whole corpus for the `255,0,0` triplet in both fog sources across all 32 maps and paste the counts — a uniqueness claim needs a search, not a reading (§5 check 2). Then ONE `model: "opus"` verifier agent on the claim list and citations alone. GATE: `analysis/re-fog.md` gains a dated subsection whose first line is §5's `# OPUS RE` header, every load-bearing claim tagged `[O]` beside its [V]/[D]/[H], and the KAS2 row either CONFIRMED with its bytes quoted or corrected and marked dead; an `analysis/RE_LOG.md` pointer entry. If (a) turns up a real port bug it becomes its own build entry with a golden gate — do NOT fix the renderer inside this RE job. NO code change and no gate value moves here. Files: `analysis/re-fog.md`, `analysis/RE_LOG.md`, scratch under `engine/artifacts/lead_tmp/re/kas2-fog/`.
