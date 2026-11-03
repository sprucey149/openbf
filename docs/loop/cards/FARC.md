---
id: FARC
title: What the PS2 draws past the far cull on the 18 maps with no drawn `LOWR` sheet, and the far WATER pass through KAS1/KAS2's sheet holes — `OPENBF_FAR_CULL`'s default waits on it
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
`OPENBF_FAR_CULL` and `OPENBF_LOWRES` are OFF because on 18 maps (16 ship no `LOWR`; BF2 DAG1/NAB2 ship one with no texture)
the cull would show the clear colour. Since CLN5 the PS2 also shows its far water pass through the holes BF1 KAS1's and
KAS2's sheets cut, where the port shows the clear colour.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md §What next 2(b); `analysis/re-lowr.md` §2.3; CLN5's report `engine/artifacts/loop/reports/20260925_1200_build_land_CLN5.md`.
A PCSX2 frame from a high vantage (library `engine/artifacts/pcsx2/ref/`) settles the look.

## Acceptance
`analysis/re-lowr.md` dated section ([O]) naming what the PS2 draws past the far range per case, and a build card (or a
decision to keep the flags off, with evidence).

**Note (planner 2026-09-28):** raised 3 -> 4. The maintainer's 09-28 KAS2 note (card REDB, value 5) wants far cull + LOWR ruled MAP BY MAP with PCSX2 pairs, never one global switch. REDB fixes KAS2 (a map with a sheet) and sweeps the 32 goldens; this card owns the 18 maps with no drawn sheet. Coordinate: whichever runs second reads the other's addendum in `analysis/re-lowr.md`.

## Note (REDB, 2026-09-29, lane 2)
REDB's sweep (`analysis/re-fog-amount.md` §6, `lead_tmp/re/redb/sweep_table_t1.tsv`) lists the SHEETLESS maps with far heightfield
in the golden panels: BF1 END1 96,875/643, RHN1 106,881/4,131, RHN2 39,878/0, YAV1 225,940/22,262; tat2 58,028/0. The 13 sheet
maps moved to the new card FARS. Also: the sheet's live GS words carry DATE = DATM = 1 (drawn only where the sky pass left alpha),
and BF1 KAS1's sheet is ~95 % holes, so its far water and REFL's mirrored shells decide KAS1.

## Note (REFL RE, 2026-09-29, lane 2)
BF1 KAS1 authors `DisableLowRes`, so it draws no far water. On the PS2 the near scene fades into the far target over
`NearSceneRange` b..c = 150→180 m, through the Z→alpha LUT and the bucket-10 composite (`FUN_004a7098`, ALPHA 0x54).
- So past ~150–180 m the PS2 lake is the far target's mirrored sky, bare: band (52–78) = (23.4,47.1,80.2), equal to the
  PS2 far-only frame's (22.6,46.2,79.7) (`analysis/re-sky-reflect.md` §3–§4).
- The port keeps its near water to the horizon (`OPENBF_FAR_CULL` off), so that band stays water-tinted once REFL and
  TMSK land: prototype (55.9,74.5,90.5).
- Frames: `lead_tmp/re/refl/ps2exp/frames/refl_v3dn__{ctl,live_faronly,no9}.png`.

## Note (PXWA, 2026-09-29, lane 2): BF1 YAV1 (no far layer) at eye height
BF1 YAV1 (re-fog: fog (144,153,156) 90..350 m, NearSceneRange 50..100, FarSceneRange 0) — the port draws the far forest
and the Massassi temple as a pale fogged band where the PS2 shows dark haze and nothing past ~100 m:
`engine/artifacts/pcsx2/ref/bf1_yav1/walk/pairs/cp0_landingzone__sheet.png` b000 band (560..640,150..185) port
(129,137,140) vs PS2 (57,64,68); `cp3_cp1__sheet.png` b180 and `cp2_viaduct__sheet.png` b180 (the temple and a
waterfall in the port only), `cp4_tflank__sheet.png` b180, `cp6_reflectingpool__sheet.png` b180. HIGH.
- **BF1 RHN1 / RHN2 (no LOWR; NearSceneRange 100..150 / 90..110; FarSceneRange 100 / 0) (PXWA):** the PS2 shows nothing
  past the near scene — dark storm sky / haze — where the port draws the far fortress, ice cliffs and mountains crisp:
  `engine/artifacts/pcsx2/ref/bf1_rhn2/walk/pairs/cp2_cp1__sheet.png` b000, `cp4_cp2__sheet.png` b000/b270,
  `cp5_cp6__sheet.png` b000 (the Citadel faint on the PS2); `bf1_rhn1/walk/pairs/cp4_cp5__sheet.png` b000/b090,
  `cp3_cp3__sheet.png` b000/b180. World fog words are 0 in these states (no inherited fog). HIGH.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).

- Planner 2026-09-30 17:30: Raised 3 → 4 (the maintainer INBOX 11:10: "card FARC (the far WATER pass) … raise it"). BF2 KAM1 has NearSceneRange (0.8, 60, 90), FarSceneRange 800 and no LowResTerrain sheet; its sea shows a hard edge and a flat dark-green plane past the foam disc (lead_tmp/playtest_0930/kam1_port_overview_sea_boundary.png). KAMR (M1 5, alpha) researches Kamino; take KAM1's far water together with it.

## Note (KAMR RE, 2026-09-30): Kamino answered
`analysis/re-kamino.md` §2: on both Kaminos the PS2 draws the ocean in whole 160 m tiles reaching 0.8*FarSceneRange
(640 m), fogs the far part with the world fog colour and leaves the frame BLACK from the sea's edge to the horizon
(PS2 frames, lightning frozen, FarSceneRange and fog colour poked live). Build cards KSEA / KSEAL. This card keeps the
17 other sheetless maps.

## Note from BRGT (2026-10-03): BF1 RHN1's far snow
RHN1 (no fog, NearSceneRange 100..150, no LOWR sheet) draws its far terrain bright white where the PS2 shows the dark
sky's grey: terrain port / PS2 (filter undone) is 1.20-1.41 on the 5 poses that look over far ground (walk/cp2_cp7_b180
1.41, cp1_cp6_b180 1.20, cp4_cp5_b000 1.21, cp2_cp7_b000 / b090 1.17 / 1.21) against 1.03-1.08 on the other 20; the snow
carries +6.4 of RHN1's +5.2 dluma. Pair `lead_tmp/brgt/sheet_rhn1.png`; numbers `lead_tmp/brgt/tools/chan.py bf1_rhn1 terrain --poses`.

## Notes
- planner 2026-10-19 16:25: `defer: post-alpha` removed - the maintainer's 10-19 ask 1 (speed on an Iris Xe laptop): far culling on by default is one of ISPD's biggest candidate savings, and this card is what lets it default on faithfully.

## Note (Claude, 2026-10-27, from the maintainer's INBOX note)
Choosing how the near scene meets the far image: a per-pixel dither + discard fade may be expensive on a weak GPU, and a
hard seam may look wrong. Measure both (fps and a view sheet) on the Iris Xe proxy before choosing.
