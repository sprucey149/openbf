---
id: SUND
title: The sun on models: the PS2's own light vector (BF1 SunInfo.Angle formula, BF2 lght quaternion row 2), BF1's ((1+n·l)/2)^4 term, BF1 prelit draws without the sun — land alone
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why / Evidence / Acceptance
- **Why.** The port lights every model from the wrong side: BF1's sun is rotated 35–76°, BF2's z-mirrored (up to
  105°). Surfaces the PS2 shows in shade come out sunlit (F15: Theed's columns; the Dune Sea rocks; Mos Eisley's walls).
- **Evidence.** `analysis/re-sun-direction.md` ([O], one Opus verifier: 27 SUPPORTED, 2 PARTLY folded, 0 NOT SUPPORTED).
  - §1/§2: the PS2's vectors, and BF1's ((1 + n·l)/2)⁴ term.
  - §1.10: BF1's prelit set has no sun.
  - §3: RAM matches the formulas to ≤ 0.004° on every map with a state.
  - §4: the census at 200 PCSX2 poses; BF1 68 % / BF2 72 % of the affected blocks move closer with the direction alone.
  - Sheets: `lead_tmp/re/sund/sheets/best_bf{1,2}.png`.
- **Acceptance (the build, §6).**
  - BF1 `sun_dir` = (−sin a0·cos a1, cos a0, −sin a0·sin a1) from `SunInfo.Angle` (deg).
  - BF2 Light1 and Light2 = row 2 of q = (−a3, a0, −a1, a2), each pinned by unit tests on the RAM vectors (§6.1/§6.2).
  - BF1's lit sun term is ((1 + n·l)/2)⁴; BF2 keeps max(0, n·l).
  - BF1 prelit draws take no sun.
  - `look::ps2_light_vector` is gone (the water reads `sun_dir`).
  - The old-convention tests and module docs are rewritten; ci green.
  - Rebake every map. View every moved golden: surfaces in shade on the PS2 frames go to shade.
  - Reproduce the census on the branch's own renders (`lead_tmp/re/sund/run_port.py` with `SUND_BIN` / `SUND_WT`
    pointing at the branch, then `census_pair.py base <variant>`). The scratch instrument's numbers for this exact
    change are BF1 64 % closer (22.1 → 11.7, `base → ps2d4s`) and BF2 72 % (22.5 → 13.8, `base → ps2`); land within a
    few points of them.
- **Not this card:** the GS-unit (gamma) light sum is card LSUM; d⁴ gains most with it (§4.3).

## Original QUEUE.md entry (verbatim, 2026-09-25)
SUND. [TODO — ADDED 2026-09-25 (lane 1) by WCOL's [RE] half (a side finding); **[RE] FIRST** (job D, at max); FREE; the build moves every lit golden and rebakes every manifest — land it alone, after WCOLB] **The port's sun comes from the wrong direction: BF1 bakes (−L.z, L.y, L.x) of the PS2's own `FUN_004931e0` vector (a 90° azimuth rotation), BF2 (L.x, L.y, −L.z) of its global light 1** [RE] [REBAKE] [GOLDENS] — `analysis/re-water-colour.md` §4 [V disc/RAM/code][O]: the disc's `SunInfo.Angle` through BF1 `FUN_004931e0` reproduces the RAM vector at 0x69c590 on 4 BF1 maps and the port's baked `sun_dir` is its rotation on all 14 BF1 maps with states; the port's BF2 sun is the z-mirror of `*(DAT_0060c6e0)+0x50` on 16 maps; the PS2's sun glare in `bf1_kas2/cp5_cp10_dock_one_v3_up` sits where the PS2's vector puts the sun, not the port's. `re-sky-lighting.md` §4 derived the port's two conventions from their agreement with each other on BF2 GEO1, not from the ELF. RE (§5): (1) that the PS2's DIFFUSE lighting of models (and terrain, if it is lit at runtime at all) uses this vector — the light setup `FUN_004b8e78`, the VU lighting routines behind the flags table, BF2's `lght` path; (2) BF2's quaternion convention (why z flips); (3) which faces the corrected sun lights at FINDINGS F1–F4 / F20 / F27 poses (a scratch render at the PCSX2 poses, as WCOL's instrument did). Build: correct `direction_to_sun` and the quaternion reading, pinned by tests against the RAM vectors; [REBAKE] every group that writes a manifest; every lit golden moves — quantify and view every pair. Files: `mungefs/src/sky.rs`, `mungefs/src/light.rs`, `bake/src/lighting.rs` and their tests.

## Notes
- 2026-09-25 (LITE RE, `analysis/re-model-lighting.md` §6/§8): F15 is this card's case, measured. BF1 NAB2 Rotunda rendered with the PS2's own sun vector (RAM 0x69c590 = (-0.4967, -0.8192, 0.2868), light travel) puts the columns at 66/36/59 against the PS2's 59/43/57 (port today 104/83/97), `lead_tmp/re/lite/sheets/lite_bf1_nab2_v4_up_sund.png`. BF1's light SCALE already matches, so only the direction is wrong. LITEB also relights every map: land the two one after the other, not together.

## Audit note (2026-09-26, lane 2, `reports/20260926_1300_audit_SUND.md`)
AUDIT OK: every load-bearing claim of `analysis/re-sun-direction.md` was re-derived (see the doc's banner).
- An independent census (the audit's own knobs, block metric and mask) gives for this card's exact change:
  - BF1 base → ps2d4s: 63.6 % closer, 25.3 → 15.1.
  - BF2 base → ps2: 68.1 % closer, 16.9 → 11.3.

  Expect SUND's own `census_pair.py` to land near the card's 64 % / 72 %; the gap is the instrument, not the rule.
- For §6.4: no BF1 world material takes the multiply `Prelit` path (all 96 BF1 0x40 lit materials are `prelit_add`
  with COL0), so dropping BF1's prelit sun blacks out nothing.
- The scratch knobs are `lead_tmp/audit-sund/audit_knobs.patch` (env `AUDIT_SUN=ps2`, `AUDIT_B1STATIC=1`, and a shader
  d⁴ edit) if a second instrument is wanted.
