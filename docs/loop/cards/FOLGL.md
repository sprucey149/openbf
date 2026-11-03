---
id: FOLGL
title: Foliage models' lighting: BF2 FEL1's leaves draw near-black in the port where the PS2 shows them pale pink and green — the prp_ entries are Lighting static (draw flag 0x80, meaning [H]) and the port lights them on the ordinary Lit path, back faces getting ambient only
status: archived
---

Historical technical record. Recorded state before archival was `done`.

- **Why:** FOLG's pairs (`engine/artifacts/lead_tmp/folg/pics/strip_bf2_fel1_cp3_cp3_1_postA_spawn.png`, `…_v1.png`: PS2 | before | after) put the right plants in the right places but many fel1_prop_foliage_b / _d leaves are dark brown-black where the PS2's are lit. The baked GLBs are light 'lit', MASK. re-foliage §6 leaves the draw flags 0x80 (static) / 0x800000 (BF1) / 0x1000000 (BF2) as [H].
- **RE:** what the model draw path (B1 FUN_003465c0 → the model render call with flags) does with 0x80 and the per-game flag: a prelit/unlit light set, two-sided lighting, or the static light set (re-model-lighting.md §3); then the build picks the LightPath for foliage draws in game::viewer::foliage.
- **Acceptance:** a FEL1 pair's leaves within a few levels of the PS2's mean colour.
**Planner note (2026-09-26):** raised 3 → 4. FOLG made these plants visible, so FEL1's black leaves are now in its golden
(`engine/goldens/maps/bf2_fel1.png`); a visible defect FOLG introduced outranks the internal gaps at 3.

## Outcome (2026-09-28, lane 2, RE) — DONE, no code owed (main 6cc802d)
`analysis/re-foliage.md` §16 and `analysis/re-model-lighting.md` §12 [O]; one Opus verifier: 17 SUPPORTED, 1 PARTLY
(folded), 0 NOT SUPPORTED. Report: `engine/artifacts/loop/reports/20260928_1520_re_FOLGL.md`.
- **Cause.** The black leaves were the port's pre-SUND sun. FOLG's strip (9b8d040, 11:29) predates SUND (e328300, 13:02).
- **Acceptance met.** On main, at the four FEL1 PCSX2 poses, the caladium leaves (`fel1_prop_foliage_d`) are +1.4 −0.2
  −1.1 from the PS2's mean sRGB (210,452 px). `fel1_prop_foliage_b` is −3.7 −2.9 −6.3 (134,383 px). Pairs:
  `lead_tmp/re/folgl/pair_*_main.png`.
- **Flags.**
  - 0x80 is the static light set. On BF2 it skips only the light-region lookup `FUN_0059b2f8`; on BF1 it gives
    `AmbientColor` and the full sun. Both equal the port's `Lit` path.
  - BF2's 0x1000000 has no reader. BF1's 0x800000 batches the segment by texture with the same light terms.
  - VU1 lighting is one-sided on the stored normal, as in the port.
- **New cards.** CAMB (M1, 4, build): BF1 soldiers take `CharacterAmbientColor`, measured by PS2 patched states.
  LREG (M1, 3, re): the light regions the port lacks. LSUM got the FEL1 data point (GS units lose there).
- 2026-10-05 (card LSUM landed, bf263f8): the light sum is now made in display units and the ambient floor is gone (report `reports/20261005_1505_card_LSUM.md`). bf2_fel1 parity 10.25 -> 10.56 (+0.31, passes the 0.5 gate): lit foliage overshoots a little under this sum, as FOLGL measured on the caladium leaves; not special-cased.
