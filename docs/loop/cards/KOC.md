---
id: KOC
title: BF1 Kamino's sea: dark teal under white foam crests on the PS2, a flat textureless light-slate plate in the port — the port's "sea" is the map's y = 0 terrain d
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why / Evidence / Acceptance (rewritten 2026-09-28 by the RE, lane 2)
- **Why.** BF1 Kamino's sea is a pale, textureless plate in the port where the PS2 shows dark teal water with white foam.
  The PS2 never draws that terrain: BF1 KAM1 (and BES1) author `DomeInfo { TerrainEnable(0) }`, which BF1 reads into
  `DAT_005229a8`. At 0 there is no terrain draw of any kind, and the height query answers −1000. The port reads no
  `TerrainEnable`, so it draws the map's y = 0 heightfield over its own ocean.
- **Evidence.** `analysis/re-terrain-enable.md` ([O], verifier 26/26).
  - PS2 patched states at F9's pose: with the terrain draw off nothing changes; with the water off the sea goes black
    (`lead_tmp/re/koc/ps2exp/`).
  - PS2 RAM: the flag is 0 on KAM1 and BES1 only.
  - `kam1_water` is flat grey (180) with the foam in alpha, which is why the opaque plate has no texture.
  - The port with the heightfield dropped: F9 `v4_down` box (20,33,29) against the PS2's (17,30,33); today
    (191,214,237) (`lead_tmp/re/koc/port/means_port_vs_ps2.txt`).
- **Build.**
  - `mungefs::sky::parse_dome` reads `TerrainEnable` (fnv 0xae291b6d, `DomeInfo` scope). The bake carries it into
    `ManifestSky` for BF1 sources only (BF2's ELF never reads the key).
  - `game::viewer::scene::upload_scene` draws no heightfield when it is 0: no base chunks, no TALPH layers, and no TDTL
    detail pass once that exists.
  - The sim's ground is NOT this card: that is TENS (M3), which moves the sweep.
- **Acceptance.**
  - `goldens/maps/bf1_kam1.png` shows the ocean where the plate was (viewed; the walkabout stats move with it).
  - F9's four pairs (`engine/artifacts/pcsx2/ref/bf1_kam1/pairs/cp0_cp2_beta1_{v2_water,v3_water,v4_down,v5_down}.png`)
    re-rendered with `port.py`: every clean box within ~7 levels of the PS2's mean (re-terrain-enable §7).
  - BES1's golden unchanged (its terrain is already not drawn); no other golden moves.
  - The foam's remaining contrast gap (spread 6 against 16) is FOAMT's, not this card's.

## Note (2026-09-26, TERL landing)
Since TERL (main 520c4cc) the y = 0 sea-terrain is drawn unlit with COLR (BF1 c5<<3): in `goldens/maps/bf1_kam1.png` it is now
a pale blue-white plate, further from the PS2's dark teal (17,30,33) than the old slate. Sheet:
`engine/artifacts/lead_tmp/terl/land/main/goldens/pair_04_bf1_kam1.png`. Raises KOC's visible cost.

## Original QUEUE.md entry (verbatim, 2026-09-25)
KOC. [TODO — ADDED 2026-09-25 (lane 1) by PXC batch 2 (FINDINGS F9); **[RE] FIRST**; FREE; its build touches the terrain draw — coordinate with KTEX's follow-on] **BF1 Kamino's sea: dark teal under white foam crests on the PS2, a flat textureless light-slate plate in the port — the port's "sea" is the map's y = 0 terrain drawn OPAQUE with the foam sheet** [RE] — evidence `engine/artifacts/pcsx2/ref/bf1_kam1/pairs/cp0_cp2_beta1_{v2_water,v3_water,v4_down,v5_down}.png` (`_v4_down` box (250,40,630,200): PS2 (17,30,33) spread 16.2 vs port (76,87,96) spread 1.0). `analysis/re-water-alpha.md` §5: the heightfield sits at y = 0 over all 1,618 wet cells, 8 m above the ocean plane at −8, textured with `kam1_water` (LTEX layer 0) and drawn opaque — "[H], unread: whether the PS2's terrain pass takes LTEX layer 0's texture alpha at all — if it did, BF1 Kamino's sea-terrain would show the −8 m ocean through it". The PS2 frames look like that case. RE (§5): the terrain pass's TEX0 TCC / ALPHA / TEST state for layer 0 (BF1 `FUN_00481608` and the patch packer, `re-terrain-heights.md` §9), the sheet's alpha (0..218, mean 62.2 per §5); then why the port's plate is textureless (tiling or mip — spread 1.0). GATE: a dated addendum to `re-water-alpha.md` §5 (§5 header, [O], verifier) naming the term with its bytes + a build entry with the four pairs to beat. Files: `analysis/re-water-alpha.md`, `analysis/RE_LOG.md`, scratch `engine/artifacts/lead_tmp/re/koc/`.

## TALPH's answer (2026-09-28, lane 2): NO — KOC's own RE stands; dep on TALPH dropped
- All 1024 of BF1 KAM1's PTCH masks are 0x1 (kam1_water only), and its tern `ALPH` chunk is empty (0 bytes).
- So every weight is the builder's 0xee (opaque), no triangle is skipped, and the PS2 draws the whole y = 0 sea terrain
  with kam1_water (`analysis/re-terrain-layers.md` §7; `lead_tmp/re/talph/census.txt`).
- Every other terrain layer texture on both discs is opaque. Only kam1_water carries alpha, 0..218
  (`lead_tmp/re/talph/texalpha.txt`). KOC's question is therefore exactly TEX0.TCC / the blend for this one layer.
- Note: BF1 BES1 has the same layout (kam1_water, masks 0x1, empty ALPH), but kam1_water is not in BES1.LVL.
