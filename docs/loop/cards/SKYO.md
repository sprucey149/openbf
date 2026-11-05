---
id: SKYO
title: The sky's `SkyObject` bearing: count, altitude band and distance are authored, azimuth is not; `InDirectionFactor` and `0xb4c88558` are parsed and unused
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 10(e). The port's even azimuth spread is ours (PROVISIONAL).

## Acceptance
The PS2's placement rule for sky objects ([O]) and a build card, or closed with the port's spread confirmed by a PCSX2 frame.

## Note (PXWA, 2026-09-29, lane 2): BF1 BES1 at seven posts
The PCSX2 walkabout on BF1 Bespin Platforms: the port draws its sky objects (a saucer at b000, a Star Destroyer at
b090/b180, a cloud-car shape at b270) at the SAME screen place from every post; the PS2 shows none in all 28 frames
(`engine/artifacts/pcsx2/ref/bf1_bes1/walk/pairs/cp2_cp16__sheet.png`, `cp3_cp18__sheet.png`, `cp5_cp19__sheet.png`).
Either the PS2 places them elsewhere (the bearing is the unknown) or draws them only at some times.

## Notes
- 2026-09-29 (PXWB, lane 2; FINDINGS F50): BF2 too — GEO1's two Acclamators hang over the plain in the port and in none of its PS2 frames (`pcsx2/ref/bf2_geo1/walk/pairs/cp0_cp2__sheet.png` b180; through the bunker windows `cp1_cp7` b000, `cp2_cp1` b180); TAT2 has a Star Destroyer and a flying shuttle on the PS2 and none in the port (`pcsx2/ref/tat2/walk/pairs/cp3_cp1__sheet.png` b090, `cp0_cp7__sheet.png` b180).
- 2026-09-29 (PXWB part 3, FINDINGS F60): small ships in the port's BF2 KAS2 and MYG1 skies at most posts (`pcsx2/ref/bf2_kas2/walk/pairs/cp1_cp1con__sheet.png` b090/b180; `bf2_myg1/walk/pairs/cp3_cp7_con__sheet.png` b090-b270), none in any of those PS2 frames.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 1 (kept).

- 2026-11-02: BF2 placement and motion implemented on `map-atmosphere`; see `analysis/re-bf2-geonosis-atmosphere.md`. Original Geonosis RAM confirms world-origin centers, 2/20/20/0 live craft, authored velocities and recycling. BF1 placement remains unverified; this does not close the full card.

- 2026-11-02 player follow-up: all decorative fleet models now use material lighting and hull depth, with opaque parts before transparent parts. The original SkyObject path is 0x20001, distinct from dome 0x20031; the earlier claim that lit ships were a deviation is corrected. Shared presentation spawn RNG advances once per logical craft, independent of mesh segment count and simulation RNG. See the same analysis note for capital/gunship/fighter close-ups and tests.

## Source reconciliation — November 4, 2026

The #118 atmosphere source and the BF1 shared-runtime playtest trial are now
integrated at `4cda5ab4`. Earlier scope/hold statements above describe their dated
checkpoint. Current implementation boundaries and still-open native evidence
are in `analysis/re-bf2-geonosis-atmosphere.md` and
`analysis/re-hoth-atmosphere.md`; this note does not close original-game or
all-map acceptance.
