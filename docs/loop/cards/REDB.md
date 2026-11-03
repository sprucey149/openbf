---
id: REDB
title: BF1 KAS2's red band is the port's fog arithmetic: fog by the PS2's rule (planar view depth, the disc's fogspread table, a lerp on the stored bytes) on every fogged draw — the ground panel and every KAS2 pair lose their red, the vantage keeps the thin dark-red line the PS2 itself draws there
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## RE result (2026-09-29, lane 2) — `analysis/re-fog-amount.md` [O]
**Why (rewritten).** The red is the port's fog ARITHMETIC. Past `near` (1000 m) the far land is near-black on both
the port and the PS2, and each fogs it red. The port makes it 3–10× redder than the PS2:
- it mixes the fog in LINEAR light, while the GS lerps the stored 8-bit values (≈20 of the 30 levels);
- it uses radial distance, while the PS2 uses planar view depth;
- it uses f itself, while the PS2 uses a lookup in the disc's `fogspread` texture.
Culling the heightfield is a separate, per-map change (card FARS).

**Evidence.**
- The PS2 was captured at the golden's own VANTAGE pose (chase camera held by RAM writes, `lead_tmp/re/redb/ps2exp/`).
  The PS2 draws a thin dark-red line there itself: up to ~(40,6,3), 887–1,146 red-dominant px. It turns blue under a blue
  fog word and black with fog off.
- On the natural frame's sheet pixels, the rule below errs +1.9 levels of R. Mixing in linear light errs +21.9, and the
  port today +30.3.
- Emulated, the rule leaves 0 red in the ground panel and 0 fog-made red at all 21 KAS2 PXC poses. The vantage panel
  keeps its thin line: 618 px, mean ≈ (45,1,0).
- The 32-golden sweep: only BF1 KAS2 (and 2 px of GEO1) has red on far heightfield.

**Build (this card).**
- The PS2 fog amount on every draw the port fogs:
  - `f` = (planar view depth − near) / (far − near), with the maintainer's 0.92 span kept;
  - A = the `fogspread` table's diagonal (the disc's 64×32 alpha at (f, f), bilinear, clamped; the 64 values are in
    `lead_tmp/re/redb/fogspread_bf1_64x32.rgba`, or bake the texture from CORE.LVL), and A = 0 at f ≤ 0;
  - blended as a lerp on sRGB-ENCODED values (encode the fragment and the fog colour, lerp, decode for the target).
- The water's fog gate and the speckle's fog fade take the same A. The sky stays unfogged.
- Keep the table swappable: the PS2 sits at 0.5–0.65× it in the fogged bins (card FOGC).

**Acceptance.**
- The ground panel of `bf1_kas2.png` has 0 red-dominant px.
- Every KAS2 PXC pair has 0 FOG-MADE red-dominant px (red with the fog, not red with it off).
- The vantage panel shows the PS2's thin dark-red line: compare it with `ps2exp/frames/vantage__nat_r1.png` (pose
  `vantage_pose.json`) and `vlevel__nat_r1.png`. It is NOT required to be 0: the PS2 draws red there (Question for the maintainer
  in the report).
- The white row (item 2 of the old acceptance) is the dome's haze sliver. The PS2 covers it with its mirrored `Reflect`
  shells, which is card REFL.
- Every other golden that moves is declared and viewed: the 16 fogged maps (`declares:`, confirm on the first gates run).
- Run the PXC census on the fogged maps (the SUND-style `lead_tmp/re/sund/` tools). The fog should come closer to the
  PS2 on most poses.
- Record the 0.92 question for the maintainer: his "a little too much fog" was judged against the stronger linear-light fog.

## Original card text (2026-09-28)

## Why
The maintainer (INBOX 2026-09-28): BF1 KAS2 still shows a red sliver on the horizon after CLRB, in the committed golden
`engine/goldens/maps/bf1_kas2.png`, **right (ground) panel, rows 119-124**, in the gap between the forest backdrop and the
water. The PS2 shows no red there. It must go: fix it for real or RE it to the real fix. Do NOT close it as "real fog, out
of scope".

## Evidence (Claude's probe, `engine/artifacts/lead_tmp/redband/`: `redband_probe.rs`, `kas2_ground_band_x3.png`,
`ground_asis.png`; at `gallery::ground_view` on main 73632e1 — reproduce before relying on it)
- The red pixels are TERRAIN (draw 32, `LightPath::Terrain`) ~1.4 km out. Fog off → (0,0,0); fog (255,0,0, 1000..3165 m)
  at t ≈ 0.18 → (45,0,0)/(63,0,0). Not the clear, not the sky. (Planner viewed the x3 crop: the red line is there.)
- The PS2 never draws that terrain: KAS2 authors `NearSceneRange(2, 80, 100)`; past 100 m the far scene draws the `LOWR`
  sheet (`lowres_kas2.glb`, to 2000 m), far models to `FarSceneRange` 1500, and on KAS the mirrored shells (REFL). The
  port's far cull is OFF by default (`far_cull_enabled`, `game/src/viewer/look.rs`, queue L 2026-09-17).
- `OPENBF_FAR_CULL=1`: red gone, black gap. `+ OPENBF_LOWRES=1`: the sheet fills the gap, (29,30,18) unfogged but (55,29,18)
  fogged — still a red line at the left. The PS2 band is dark, median (10,20,11)
  (`pcsx2/ref/bf1_kas2/pairs/cp0_cp2_beachhead_v1_sea.png`).
- Open: (a) how the PS2 fogs the LOWR sheet / far scene on BF1 (the `LowResTerrain` scope's own fog keys? planar depth?
  re-fog §9 "no pose shows it"; FOGB says BF1 near models take no fog); (b) the white line on row 119 = the dome's bottom
  row, present with zero draws, where the PS2 is dark.

## Notes
- Created 2026-09-28 by the planner from the maintainer's INBOX note. Related: FARC (value raised to 4), REFL, FOGB, KASF (done), CLRB (done).
- Likely shape (the card decides): far cull + LOWR default-on where the map has a sheet, with a PS2-faithful far-scene fog.
