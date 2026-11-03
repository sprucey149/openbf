---
id: REFL
title: BF1 Kashyyyk's mirrored sky: draw each Reflect shell mirrored under the eye (Y x -(0.85+0.006 sin), +0.1 m, ReflectionColor) before the shell - the sky under the lake and below the horizon on KAS1/KAS2 (REDB's white row)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Note (REDB, 2026-09-29, lane 2) — the visible half is read; REFL now carries REDB's white-row acceptance
`analysis/re-fog-amount.md` §7, [V code / D geometry]; the verifier's C8 is SUPPORTED down to the disassembly.
- BF1 sky tick `FUN_002f2308`: when a `DomeModel`'s `Reflect` (+0x34) is set (280242), the shell is drawn twice with a
  MIRROR matrix BEFORE its normal 0x20031 draw: first with 0x1000031 (280253), then with 0x20031 (280255).
- The mirror matrix: Y row = −(0.85 + 0.006·sin φ)·Y (280190–280218; `DAT_00518b24` = 0.006, φ `DAT_00518b1c` += 2.0/30
  per tick). Translation = the lerped shell position + (0, 0.1, 0) in model units (S = 1).
- The 0x20031 mirrored draw is the visible one. It lands in the normal no-Z sky pass, so on KAS2 the mirrored
  `kas2_mountains` covers the dome's hazy bottom sliver: the WHITE ROW of the port's KAS2 ground panel, row 119. It also
  paints the dark band the PS2 shows under the horizon (median (10,20,11)).
- The 0x1000031 draw goes down the reflection branch of `FUN_004cf6f0` (594262) and is still unread [H].
- Value 4 → 5: REDB's "white row explained and fixed" acceptance item moved here.

## Why / Evidence / Acceptance (RE done 2026-09-29, lane 2: `analysis/re-sky-reflect.md` [O])
**Why.** On BF1 KAS1/KAS2 the PS2 draws each `Reflect` shell twice more, mirrored under the eye:
Scale(1, −(0.85 + 0.006·sin φ), 1) about the shell's own position + 0.1 m, tinted by `ReflectionColor`, with φ at 2.0 rad/s.
It is the sky seen under the lake and below the horizon (§0–§3).
- The port reads neither key. Below the horizon it shows black (CLRB's clear) or the dome's white haze row where the
  PS2 shows the mirrored forest and sky.

**Evidence.** Code and disassembly [V], RAM [V], and PCSX2 with each draw removed on the PS2 itself (§4):
- the 0x1000031 copy is the near lake's reflection;
- the mirrored 0x20031 copy is everything beyond it.
A scratch prototype of the port rule (`lead_tmp/re/refl/prototype.patch`) registers to the PS2 far target to −1 px.
Squash and lift are both pinned: ±0.05 in s and ±0.1 m in lift each move it 4–19 px (§5).

**Build.**
1. `mungefs::sky` parses `DomeModel.Reflect` (0x92c778aa, no args) and `DomeInfo.ReflectionColor` (0x8a81de8b, 3
   bytes, default 255,255,255). Pin both preimages in `hash_preimages_are_confirmed`.
2. The manifest gets `ManifestSkyModel.reflect` and `ManifestSky.reflection_color`, both `#[serde(default)]`.
3. `bake::lighting::sky_from` copies them.
4. `game::viewer::scene::upload_sky` emits, per shell in authored order, the mirrored copy (all segments) and then
   the shell. The mirrored copy's colour is ReflectionColor as the GS takes it, ((c+1)>>1)/128, alpha 1. It keeps the
   shell's own `SkyMode`.
5. `render::sky` gets a mirror placement: T(eye + Y·((Offset − eye.y)(1 − MS) + 0.1)·k) · Scale(k, −s·k, k), with the
   same k as the shells. The wobble s = 0.85 + 0.006·sin(2.0·t) needs a time input to the sky matrix; t = 0 in
   headless renders, so s = 0.85 in every golden.
6. Update the `render/src/lib.rs` header and the `Placement` doc ("Reflect not built / no map checked"), CRATES.md,
   and tests: the matrix arithmetic, the parse, and one render test that the mirrored copy lands below the horizon.

**Acceptance.**
- `maps/bf1_kas2.png` ground panel: row 119 (the white haze row, REDB's old item 2) goes dark, and rows 120–121 lose
  their black. Prototype: row 119 (48.9,53.7,58.7) → (4.7,9.0,6.6); rows 120–121 → (19.0,28.8,18.6); the vantage is
  unchanged.
- `maps/bf1_kas1.png`: ~299 px (11 vantage, 288 ground: the black gap under the horizon → mirrored forest).
- KAS2 Beach Head `v1_sea` rows 128–136, median: PS2 (11,21,12), port today (0,0,0), prototype (15,27,16). Render at
  the PS2 pose with `pxc_pose` and view the pair.
- No other golden moves (no other map authors `Reflect`).
- KAS1's lake keeps its grey bed until TMSK. With the bed dropped, the prototype gives near lake (44,66,95) vs the
  PS2's (37,60,93). That pair is TMSK's to beat, not this card's.

## Original QUEUE.md entry (verbatim, 2026-09-25)
REFL. [TODO — ADDED 2026-09-25 (lane 1) by PXC batch 2 (`engine/artifacts/pcsx2/ref/FINDINGS.md` F16, part of F1); **[RE] FIRST** (job D, at max); FREE (disc + `analysis/`); the build it enables touches the bake's sky manifest, `render/src/sky.rs` and the water pass] **BF1 Kashyyyk's water mirrors the sky: every sky shell of KAS1 and KAS2 is authored `Reflect`, the PS2 draws it mirrored under the lake, the port reads no `Reflect`** [RE] — evidence `engine/artifacts/pcsx2/ref/bf1_kas1/pairs/cp2_cp2_eastharbor_v3.png` (x2 crop `engine/artifacts/lead_tmp/re/pxc2/kas1_v3_lake_x2.png`: the far mountains hang upside down in blue water; lake rows (100,150,600,235): PS2 (24,39,58) vs port (57,54,51)); the disc [V]: the `Reflect` key hash 0x92c778aa occurs, over every file of both discs, only in BF1 KAS1.LVL (3x — the DomeModels `kas1_sky_dome`, `kas1_mountains`, `kas_moon` of the `sky_` chunk at 0x1824c4), KAS2.LVL and KAS2_H.LVL (2x each: `kas2_sky_dome`, `kas2_mountains`); BF2 KAS2 has none. `analysis/re-sky-dome-placement.md` §3/§4 (BF2 425839–425855, BF1 280242–280255: two extra draws at +0.1 m, flags 0x2000031 / 0x1000031 then 0x20031, before the normal draw) and §8 311–312 ("ignore until a map needs it" — two do). The port: 0 hits for 0x92c778aa under `engine/crates`; `render/src/sky.rs` 125–126 and `render/src/lib.rs` 23–25 still say no map was checked. RE (§5): what the 0x1000000 (BF1) / 0x2000000 (BF2) draw flag does — a mirrored projection about the water plane? a clip to the water? its draw order against the water pass (`analysis/re-material-flags.md` §2's buckets) — and the mirror height. GATE: a dated § in `analysis/re-sky-dome-placement.md` (§5 header, [O] tags, verifier) naming the flag's effect with its bytes; a follow-on build entry [REBAKE maps] [GOLDENS the two Kashyyyk goldens] with the pair to beat. Files: `analysis/re-sky-dome-placement.md`, `analysis/RE_LOG.md`, scratch `engine/artifacts/lead_tmp/re/refl/`.

## Note (KASF, 2026-09-28, lane 2)
BF1 KAS2's band below the horizon, beyond the water and terrain edge at z = +1024 (rows 128–135 at Beach Head `v1_sea`),
is where REFL's mirrored shells would show on the PS2.
- The PS2 is dark there: median (10,20,11), dark forest-green texture.
- The port shows its clear colour, today the fog red. Card CLRB turns that black, the PS2's far-target clear.
- REFL is the remaining half of that band. The pair to beat is `engine/artifacts/pcsx2/ref/bf1_kas2/pairs/cp0_cp2_beachhead_v1_sea.png`
  (crop viewed: `engine/artifacts/lead_tmp/re/kas2-fog/v1_sea_band_ps2_port_green.png`). Source: `analysis/re-fog.md` §9.

## Note (TALPH RE, 2026-09-28, lane 2)
The PS2 draws NO lake bed under BF1 KAS1's and KAS2's water: those patches have layer mask 0, so no terrain pass is
issued (`analysis/re-terrain-layers.md` §2.5, §7). The mirrored shells are what shows through the water there. The port
draws the bed (F16's grey). New card TMSK drops it, with deps TALPH and REFL, so REFL's build should expect the bed to
vanish afterwards.
