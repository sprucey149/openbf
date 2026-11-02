# OPUS RE (loop lane 1, 2026-09-25) - [O] UNAUDITED: no Fable pass
# RE: why the port runs brighter than the PS2 on static scenery — the PS2's model and terrain lighting (queue LITE)

**The question** (queue LITE; PCSX2 FINDINGS F15, F20, F27; HOLD 11(a)): the port renders most BF2 maps 1.3–2.6× brighter
than the PS2 (a lower-right-box census), both Hoths clip to white, BF1 Theed's shaded columns come out beige. Which term?
Method: the loop's RE discipline. The hops below, the five self-checks, then ONE Opus verifier given the claim list only
(`engine/artifacts/lead_tmp/re/lite/claims.md` → `verify/VERDICTS.md`). Two measurements back it:
- The PS2's draw taken apart in PCSX2 (save-state copies with one lighting input patched in EE RAM, §7).
- The port's draw taken apart at the same poses (a scratch instrument: the disc's per-vertex colour baked in, the PS2
  light rules as knobs, §8).

Scratch, scripts, frames and sheets are in `engine/artifacts/lead_tmp/re/lite/`. Conventions: B2 = `analysis/SWBFRNT2.ELF-decomp.c`,
B1 = `analysis/SWBFRONT.ELF-decomp.c`. VU1 is the main program in `.vutext`, decoded with
`scripts/tools/vu1dis.py`. "GS units" means 0x80 = 1.0 (MODULATE). Tags: [V] read or measured, [D] derived, [H] guess.
Load-bearing claims are also [O].

## 0. The answer

The port lights every static model with the map's hemisphere ambient plus its key sun, multiplied into the texture.
The PS2 lights them through a per-vertex colour the port throws away, and by a rule set the port does not have.

| term | PS2 (this RE) | port today | measured effect |
|---|---|---|---|
| **COL0**, the disc's per-vertex colour on static models (BF2 24–98 % of world indices, BF1 0–91 %, never above 0x80) | lit segment: colour = **COL0/128 × (hemisphere + lights)**, a multiply (VU1 0x521) | dropped by the bake | BF2 MYG1 floor: port 52 → **31**, PS2 28 |
| **prelit models** (BF2: any segment with MTRL 0x200, 0–95 % per map) | **no ambient; `Static()` lights skipped**; the 0x200 segments ADD mat × COL0/128 (VU1 0x537) | lit like any model | BF2 TAN1 frame: port 137 → **109**, PS2 103; its white-floor clipping 12–13 % → 0 % |
| **Light2**, `GlobalLights.Light2` (COR1 "Back", END1, MUS1, NAB2, UTA1) | a second directional light on non-prelit models | never rendered | BF2 COR1 floor: with it **43**, PS2 46 (port today 92) |
| **BF2 sun colour transfer** | `Light.Color` (0..1) is a gamma-space multiplier like every other colour | used raw as a linear value | small at these poses (−1 to −2 levels) |
| **terrain** (BF2) | no VU lighting on the base pass: the lightmap alone; non-`Static` point lights add in extra passes | LUMI × (ambient + sun·N·L) | BF2 HOT1 snow: port 242 → **229** (lightmap only), PS2 214; clipping 33 % → 11 %, PS2 0.2 % |
| **BF2 fog** | far-scene draws only (`re-fog.md` §2, already known) | every draw from `near` | BF2 MYG1 mid-distance band: port 82 with fog, 46 without, PS2 61 |
| **the sun direction** (queue SUND) | the PS2's own vector | rotated 90° (BF1) / z-mirrored (BF2) | BF1 NAB2 Rotunda columns (F15): port 97 → **59** with the PS2's vector, PS2 57 |
| **BF2 HDR** (`Effect("HDR")`) | scene × 0.5, then the composite doubles it and adds a bloom | none | neutral in level (§1); a bloom the port does not draw |

**The BF1 / BF2 split in F27 is explained.** BF1's light scale is right in the port (§6), and fewer BF1 models carry COL0
in the views sampled, so BF1 lands near 1.0 in both directions. On BF2, COL0, the prelit rule and the terrain rule all
darken, so BF2 lands above 1.

## 1. BF2's global colour scale DAT_007c5754 is `Effect("HDR")`, and it is neutral [V][O]

- FUN_00455f30 (B2 516693) parses the effect's keys: `Enable` 0xaf8bb8ce, `DownSizeFactor` 0x7e6c7187 (+0x430),
  `NumBloomPasses` 0xd4ef5803 (+0x434), `MaxTotalWeight` 0xeafd07ed (+0x438), `GlowThreshold` 0xb12c9b1f (+0x43c),
  `GlowFactor` 0xac9eee97 (+0x440). Every preimage is FNV-1a, recomputed.
- The object is registered as "HDR" (0xfeb6dbe1) by FUN_00455da8 (516627).
- `Enable` ≠ 0 → FUN_00456098 (516718 → 516764) → DAT_007c5754 = 0.5, else 1.0 (516761).
  - ELF words: 0x4560b4 = `lui at,0x3f00`, 0x4560cc = `lui at,0x3f80`.
  - The only other writer is the reset FUN_00580c80 (715862, 1.0).
  - Split-screen forces it off (516709–516711, verifier).
  - It is 0.5 in all five BF2 states read (MYG1, COR1, TAN1, HOT1, FEL1).
- **Consumers.**
  - The material colour of every model segment is scaled by it (FUN_0058ed70 723501–723520), except
    (F & 0x4000000) or segment flags & 0x14 == 0x14.
  - The ambient/light upload FUN_0057fd90 (715388–715400) is not scaled. The factor still reaches the lit colour through
    the material (VU `mat`, §2).
- **The render pass.** FUN_004665d0 (vtable 0x639a70 + 0x18, ELF word 0x004665d0) runs FUN_0058b188 (the GlowThreshold CLUT
  cut) and FUN_0058b9a8 (the blur passes).
  - Its final composite writes ALPHA = 0xff00000029 (live branch 722347; ELF 0x58c49c–0x58c4a4
    `ori 0xff00; dsll 24; ori 0x29`): **out = Cd·255/128 + Cs**.
  - So the half-scale frame is **doubled back**, and the blurred glow is added on top.
  - The pass is registered only while DAT_00601284 ≠ 0. That flag is set to 1 at level load (FUN_00450cf8, 512890) and
    was 1 in RAM (verifier).
- **Measured.** Patching the scale to 1.0 in a MYG1 state doubles the frame (band 61 → 136, §7).
- **Consequences.**
  - HDR is not a darkening, so queue LITE's lead (c) is closed.
  - Segments that skip the scale (flags 0x14) come out doubled; that is a lead for queue GLOW.
  - The bloom is a screen-space post pass; that is for queue SUNG and COMPARISON_RULES.

## 2. The per-vertex lighting (BF2 VU1) [V][O]

1. **Dispatch** (0x174–0x17f). A lit segment jumps through VU[84].w + 0x1d. Handler 0x427 stores (flags & 0x78) >> 2 there.
   - The EE sends flags 8 | 0x10 (directional lights present) | 0x20 (point lights present).
   - The table sends 8 → 0x1d5, 0x18 → 0x1e6, 0x28 → 0x210, 0x38 → 0x222.
2. **Uploads** (handler 0x56b).
   - Ambient: VU[92] = int(Top·128)/256 and VU[93] = int(Bottom·128)/256 (0x570–0x577), i.e. Top/2 and Bottom/2.
   - Directional colours: VU[60..63] = int/128 (0x58d–0x591), i.e. `Light.Color` 1.0 = GS 1.0. Directions go to VU[64..67].
   - The EE side is FUN_00497500 (ambient), FUN_004973b8 (directional, vector negated) and FUN_00497278 (point), all
     command 0x56b.
3. **Directional term** (0x1e6–0x20d). colour = Σᵢ (Cᵢ·mat)·max(0, N·Lᵢ). The normal's +65537 unpack offset is folded out
   at 0x1ee–0x1f1. The result is clamped at 255/128.
4. **Ambient pass** (0x1bb–0x1d2). colour += mat·(VU92 + VU93) + mat·(VU92 − VU93)·n′y. **That is mat × lerp(Bottom, Top,
   (1 + n′y)/2) in GS units, the port's hemisphere in form.** n′ = VU[86..88]·n. Clamped.
5. **COL0** (0x185–0x190, reached after the ambient pass). When the segment's prim has COL0 (slot bit 0x1000) and the batch
   carries the stream:
   - VU[7].z bit 0x1000 is written only at 0x4b1, from the stream-presence bits; VU[4].w is the stream.
   - Without MTRL 0x200 → **0x521: colour = COL0/128 × colour** (multiply).
   - With 0x200 ("vc-lit", prim 0x2000) → **0x537: colour = colour + mat·COL0/128** (add).
   - EE FUN_005916b0 (724816–724826) sets prim 0x4000 for flags & 1, 0x100000 for COL0, and 0x2000 for 0x200 & 1 (only
     with COL0).
   - FUN_00590c38 clears COL0's alpha for 0x200 segments (724632–724633, 724650–724651).
6. **Unlit segments** (MTRL flags & 1 = 0) take 0x2a3. They get the material colour constant, plus COL0 when present, and no
   lighting.
   - **Added by the LITEB build (2026-09-25), [V] VU1 read, not verifier-checked** (`vu1dis.py bf2 2a3 2f0`): 0x2a3 tests
     the same COL0 bit (VU[7].z & 0x1000, 0x2a7–0x2ab) and the same vc-lit bit 0x20 as the lit dispatch (0x2aa/0x2ac).
     No COL0 → 0x2b3: the constant `vf1` stored for every vertex. COL0 with the vc-lit bit → 0x2bf: `vf1 + COL0`
     (ADD, clamped by MINIi at 0x478000ff). COL0 without it → 0x2da: `(COL0 − 65536)·vf1 + 65536`, a multiply.
   - So unlit = mat × COL0, or mat + COL0 on a vc-lit segment. BF2 sets the vc-lit prim bit only with 0x01 (724822),
     so a BF2 unlit segment never adds; BF1 sets it on 0x40 alone (568929–568935). `vf1`'s scale in the unlit path is
     [H]; the port takes the constant as unity (`render::LightPath::Unlit` / `UnlitAdd`, PROVISIONAL).

## 3. The light set: Static lights, prelit models, Light2 [V][O]

- **`Static()` = light word 0, bit 8.** Set by FUN_00596978 (728642 → 728564; cleared at 728561).
- **GlobalLights** (FUN_00596380):
  - Top and Bottom go to DAT_0060c6e8 + 0x50 / + 0x60, divided by 255 (728273, 728276).
  - Light1 becomes DAT_0060c6e0 with fade … Light2 becomes DAT_0060c6e4 with fade
    DAT_0060c6f4 = 1.0 (728299–728300). Both fades are zeroed first (728288–728289) and have no other writer.
- **Prelit model = any segment with 0x200.** The `modl` loader FUN_0057de00 sets model + 0x48 |= 1 (713705–713718).
- **Per model and frame.** The draw FUN_0057cf60 builds the set with FUN_00597e98(…, model + 0x48 & 1, …) and uploads it
  with FUN_0057fd90 (712781–712783).
- **FUN_00597e98.**
  - It copies the global set and adds Light1 and Light2 (Light2 only while its fade > 0) as directional lights.
  - With the prelit argument it stamps byte + 0xa3 (729490).
- **FUN_0057fd90.** With + 0xa3 set:
  - It uploads a **black ambient** (715402).
  - It **skips every light whose Static bit is set** (715303–715305 point, 715354–715355 directional).
- **So a prelit model is lit by COL0 plus the non-Static lights only.** A normal model gets hemisphere + Light1 + Light2,
  all of them (Static included), then × COL0.
- **Added by FOLGL (2026-09-28, `re-foliage.md` §16, [O]):** `FUN_00597e98`'s third argument, the draw flag 0x80
  ("static": terrain foliage, ODF `Lighting="static"` classes), is not the prelit rule. It only skips the light-region
  lookup `FUN_0059b2f8`. BF1's twin is §12.
- **RAM, 5 BF2 states:**
  - COR1 / TAN1 / HOT1: the sun is Static (bit 8 set). MYG1 / FEL1: it is not.
  - DAT_0060c6f4 is 1.0 on COR1 only; MYG1's `Light2("light 2")` names no light.

## 4. The disc [V]

`lead_tmp/re/lite/` census (`lite_census.rs`, a scratch mungefs example; the verifier re-ran it over all 18 BF2 ground
maps):
- **Diffuse.** BF2 MTRL bytes 4..7 (the diffuse RGBA) are ff ff ff ff on every segment, so no diffuse term exists.
- **COL0** (index-weighted): BF2 COR1 95 %, DEA1 88 %, POL1 86 %, TAN1 98 %, HOT1 95 %, MUS1 83 %, UTA1 79 %, MYG1 78 %,
  KAM1 73 %, KAS2 48 %, GEO1 48 %, NAB2 37 %, TAT2 37 %, END1 32 %, FEL1 32 %, DAG1 25 %, YAV1 25 %.
  - Means 52–95 of 128. No component above 128 on COR1 / TAN1 / MYG1 / HOT1.
- **BF1 COL0:** HOT1 91 %, RHN2 83 %, KAS1 81 %, RHN1 59 %, KAM1 54 %, KAS2 51 %, GEO1 43 %, YAV2 33 %, TAT1 27 %, END1 26 %,
  YAV1 23 %, BES2 17 %, NAB2 17 %, BES1 13 %, NAB1 0 %.
- **Prelit share of BF2 indices:** TAN1 95 %, DEA1 88 %, COR1 86 %, POL1 73 %, KAM1 68 %, MUS1 60 %, GEO1 29 %, KAS2 24 %,
  HOT1 22 %, DAG1 15 %, NAB2 12 %, YAV1 9 %, UTA1 8 %; END1, FEL1, MYG1, TAT2 0 %.
- **The global sun is `Static`** on COR1, DAG1, DEA1, GEO1, HOT1, KAM1, MUS1, POL1, TAN1, UTA1, YAV1.
- No global light authors a projected `Texture`. No unknown `lght` key appears on any BF2 map. In particular 0x55b86d2c
  (verifier: word-0 bit 12 switches a light's colour packing ×128 → ×255) is never authored.

## 5. Terrain (BF2) [V code, verifier-corrected][V measured][H form]

> **Superseded by `analysis/re-terrain-lighting.md` (queue TERL, 2026-09-25, [O]).** The form and scale this section left
> [H] are read:
> - The base pass is VU1's UNLIT path, with the vertex colour from COLR.
> - BF1: c5 << 3. BF2: c5 << 2 × HDR (HDR-neutral).
> - The +0x94 light is a projector.
> - "Point lights" read "non-Static omni lights".
> - BF1 (unread here) has the same base pass and no light passes.

- **Code.** The terrain draw FUN_00571bd0 builds its set with FUN_00599040(box, 4, 1, 1, 0) (706665), prelit, so Static
  lights are filtered out.
  - Its patch draw FUN_0056f208 **never uploads the set or an ambient**. It reads only the point lights (+0x78, +0xa1)
    and the +0x94 light, for extra additive passes (verifier, B7).
  - So the terrain's base pass carries no VU lighting.
  - LUMI itself is read only by FUN_0055f068, a point lookup that tints soldiers (×1.2 + 0.3) and grass (×0.7 + 0.6).
    The terrain's per-vertex colour is PCHS `COLR` (`RE_LOG` "COLR cracked": the same lightmap in 5-bit colour).
- **Measured.** BF2 HOT1 (sun Static): the near snow does not move when the sun or the ambient is zeroed (217.8 / 207.8 in
  every variant), while every model darkens. The glow around the turret with the sun un-Static is the HDR bloom of the
  now sun-lit models.
- **Port side.** The port with the terrain lit by its lightmap alone (ambient 1, sun 0) gives 229 against the PS2's 214.
- **[H]** The remaining ~7 %: COLR's 5-bit value, the HDR scale on the terrain colour, or the detail layer. That needs the
  terrain's VU program, and BF1's terrain path is unread (queue TERL). FEL1's "ground" boxes are foliage-covered (F22)
  and respond to the non-Static sun and the ambient; the foliage path is FOLG's.

## 6. BF1 twin [V code][V measured]

- **COL0.** BF1 VU1 has the same dispatch (0x174–0x17d) and multiply (0x51b). Its add path (0x531) adds COL0 **without**
  the material factor.
- **Light sets.** The RedModel draw FUN_00496248 builds one set (FUN_00493cf0, 557187/557190) and publishes it twice:
  - DAT_006ea930 (557193), the normal set;
  - DAT_006ea948 (557198), with its third 8-byte word replaced by {DAT_0069c440, 0} (557194–557197).
  - Segments with BF1 flag 0x40 draw with the second set (FUN_004ac0d8 569068–569072, FUN_004cf6f0 594273–594277,
    FUN_004d0108).
  - DAT_0069c440 is (0,0,0,255) in the NAB2 state, so BF1's 0x40 segments are the prelit case [D]. 0x40 covers KAM1 49 %,
    HOT1 32 %, RHN1 30 %, BES2 12 %, GEO1 12 %, YAV1 8 %.
- **Colours.** BF1 `SkyInfo.AmbientColor` goes through FUN_00493298 (555595) into 0x69c570/578/580, as bytes (defaults
  0x80, FUN_00494d18). `SunInfo.Color` goes to 0x69c588 (`re-specular-pass.md` §2).
- **Measured, BF1 NAB2 Rotunda.**
  - Zeroing the sun leaves the floor at 20 of 79; zeroing the ambient leaves 52; zeroing both gives 0.
  - The port's /255 reading lands 8–18 % under the PS2 on the lit floor (67 vs 72, 65 vs 79). The GS-unit reading
    (/128) doubles it (135–138), so **BF1's light scale is the port's** [D]; the small remainder is [H] (the linear
    sum, or COL0-free vs. prelit floor segments).
  - **F15 is the sun direction.** Rendered with the PS2's own vector (RAM 0x69c590), the up-facing columns go
    104 / 83 / 97 → 66 / 36 / 59 against the PS2's 59 / 43 / 57 (v4_up), and the v3 wall 84 → 46 against 41.
  - **Added by SUND (2026-09-26, `analysis/re-sun-direction.md` §1, [O]):** BF1's directional term is
    ((1 + n·l)/2)⁴, not max(0, n·l). BF1's 0x40 set carries NO sun (factor 0), as well as the black ambient above, so
    §9's "BF1's 0x40 segments take the prelit rule" must drop the sun on BF1.

## 7. The PS2 taken apart (patched save states) [V measured]

Method: `ps2exp/mkstates.py`, `mkstates_bf1.py`, `run_ps2exp.py` (the WCOL recipe). The EE rebuilds every model's light set
from these globals per draw. Box means (luma, 640×480); sheets are `sheets/lite_*_ps2exp.png` and `ps2exp/sheet_*.png`.

| state (box) | control | sun 0 | ambient 0 | both 0 | other |
|---|---|---|---|---|---|
| BF2 MYG1 spawn (floor R) | 28.5 | 18.2 | 6.4 | 0.1 | HDR 1.0: 60.4 (band 61 → 136) |
| BF2 COR1 spawn (floor R, non-prelit model) | 45.7 | 20.1 | — (authored 0) | — | Static bit cleared: 45.7 |
| BF2 COR1 spawn (column, prelit) | 55.8 | 56.0 | — | — | Static bit cleared: **71.9**; HDR 1.0: 96.6 |
| BF2 HOT1 spawn (near snow, terrain) | 217.8 | 217.8 | 217.8 | — | Static bit cleared: 217.8 |
| BF1 NAB2 spawn (floor M) | 79.0 | 19.8 | 51.9 | 0.0 | |

## 8. The port taken apart at the same poses [V measured]

- **Instrument.** A scratch bake (`bake/src/glb.rs`) writes the disc's COL0 as COLOR_0. Its alpha codes the VU path:
  multiply, prelit-add, or prelit-other.
- **Knobs.** The sun / ambient / sun direction / Light2 / "Static sun" are scratch knobs in `look.rs` and `shader.wgsl`
  (`run_port.py`).
- **Instrument check.** The plain scratch bake is byte-identical to the shipped one, and the no-knob render equals the
  library's committed port frame (max diff 0) at every pose.
- "PS2 model" = COL0 + the prelit rule + the Static skip + the BF2 sun linearised.

| pose (box) | PS2 | port today | port + PS2 model | extra |
|---|---|---|---|---|
| BF2 MYG1 spawn (floor R / band) | 28.5 / 61.0 | 52.5 / 97.9 | 30.6 / 82.1 | band, no fog: 46.3 |
| BF2 MYG1 v2 (floor R / band) | 27.7 / 50.3 | 53.0 / 79.4 | 30.6 / 68.1 | band, no fog: 33.1 |
| BF2 COR1 spawn (floor R / L) | 45.7 / 47.2 | 91.5 / 90.6 | 31.7 / 33.2 | + Light2 and the PS2 vectors: **42.7 / 44.6** |
| BF2 COR1 v1 (floor R / L) | 41.1 / 46.0 | 88.3 / 87.3 | 34.2 / 31.4 | + Light2: **45.9 / 42.3** |
| BF2 TAN1 spawn (frame / floor R), clip ≥ 245 | 103.1 / 65.8, 0 % | 136.9 / 111.6, 11.9 % | **108.8 / 67.6, 0 %** | |
| BF2 TAN1 v3 (frame / floor R), clip | 103.8 / 110.7, 0 % | 143.2 / 225.4, 13.4 % | **109.5 / 123.1, 0 %** | |
| BF2 HOT1 spawn (near snow), clip | 214.1, 0.2 % | 242.3, 33.4 % | lightmap only: 229.5, 11.2 % | |
| BF1 NAB2 v4_up (floor R / M / wall) | 59.1 / 43.4 / 56.5 | 104.4 / 83.4 / 96.8 | PS2 sun vector: **65.6 / 35.7 / 58.6** | |

- **Pictures.** `sheets/lite_bf2_cor1_spawn.png`, `lite_bf2_myg1_spawn.png`, `lite_bf2_tan1_v3.png`,
  `lite_bf1_nab2_v4_up_sund.png`, `lite_bf2_hot1_v2.png`.
- **Residuals [H].**
  - COR1's prelit column is 43 against 56: bloom from the adjacent glow strip is the leading guess (the strip is an
    unlit 0x10 segment, §2.6).
  - MYG1's band sits between the fogged and unfogged port: the far-scene-only fog and its b..c blend.

## 9. What the port owes

- **Queue LITEB (build).**
  - Bake COL0 per vertex, plus per primitive whether the segment is lit, vc-lit (0x200 + COL0) or in a prelit model.
  - Bake Light2 and each light's Static bit into the manifest.
  - In the world shader:
    - normal models: tex × COL0 × (hemisphere + Light1 + Light2);
    - prelit models: tex × (COL0·[0x200] + non-Static lights), with no ambient;
    - unlit segments: unlit.
  - BF2 `Light.Color` goes through sRGB → linear like every other authored colour.
  - BF1's 0x40 segments take the prelit rule. PROVISIONAL until a BF1 patched-state pair checks one (§6 [D]).
  - Every lit map golden moves.
- **Queue TERL (RE → build).** The terrain base pass is its lightmap with no ambient and no sun (§5). Read the terrain
  VU path for the exact form and scale, and BF1's terrain; then drop the terrain's ambient and sun in the port. This is
  the Hoth clipping (HOLD 11(a)).
- **Queue FOGN.** BF2's fog on far-scene draws only (`re-fog.md` §2.5 / §6, "NOT reproduced" in `bake::lighting::fog_from`).
- **Notes for other queues.**
  - SUND: F15 is its case (§6).
  - GLOW: the unscaled 0x14 segments double under HDR (§1), and unlit 0x10 strips are drawn unlit (COR1).
  - SUNG: BF2's glare is the `Effect("HDR")` bloom, a post pass (§1).
  - BACKL: BF1 NAB2 matched without the back light at these poses [H].

## 10. What stays [H]

- The terrain's exact lightmap form and scale, and BF1's terrain path.
- BF1's 0x40 set-word [D].
- The COR1 column residual.
- The GS COLCLAMP state during the HDR composite.
- The name of the Light key 0x55b86d2c.
- FUN_003c9258: the reflection pass HDR is forced off around.

## 11. Verification

**Self-checks.** Every decomp citation was placed with `scripts/re_check.py fn` (all in the named functions).
- `writers`: DAT_007c5754 (2 writers), DAT_00601284 (2), DAT_0060c6f0 / f4 (FUN_00596380 only).
- `word`: 0x4560b4, 0x4560cc, 0x639a88, 0x601284, and the composite's `ori 0x29`.
- Absence searches: the port's bake writes no COLOR_0 and no MTRL diffuse reader exists (grep); `fill_light()` has no bake
  caller.
- The VU[7].z 0x1000 writers: all 10 `ISW.z …7(vi0)` read.
- BF1 twin: VU1 0x174 / 0x51b / 0x531 and the light-set lines.

**The verifier** (one Opus agent, claim list only, `lead_tmp/re/lite/verify/VERDICTS.md`): 29 SUPPORTED, 2 PARTLY, 0 NOT
SUPPORTED. Folded in:
- B7: the terrain gets no VU ambient at all, not a "black" one; its argument is a box.
- F3: `detail_factor`, and the per-layer unlit sheet / speckle / sky.
- The live composite branch is 722347 (722023 is dead).
- Line nits: 728561, 724816, 18 maps (not 17), index-weighted percentages.

Unchecked by it: the BF1 lighting routines outside D1, and the 0x55b86d2c name.

## 12. BF1's three ambients and the static bit — OPUS RE (2026-09-28, card FOLGL) - [O] UNAUDITED

Scratch: `engine/artifacts/lead_tmp/re/folgl/` (`claims.md` B2–B4, `verify/VERDICTS.md`, `ps2exp/`). The verifier
supported B2–B4. The measurement below is this session's.

- **The pick** [V][O]. BF1's light-set builder `FUN_00493cf0` (called only from the RedModel draw `FUN_00496248`,
  557187 / 557190) chooses its ambient word by the draw:
  - **Static draws** (draw flag 0x80): `DAT_0069c570`. There is no active-region walk, and `FUN_00496248` then forces the
    sun factor to 1.0 (556087–556088, 557190–557191). These are the terrain foliage (`re-foliage.md` §16) and the world
    props measured below.
  - **Non-static draws, no shadow volume**: `DAT_0069c578` (556042–556043).
  - **Non-static draws with a shadow volume** (model +0xb8 ≠ 0): `DAT_0069c580` (556046). The `modl` loader
    `FUN_00497150` stores the `shdw` chunk's object at +0xb8 (558126–558130).
  - Non-static draws also take the active regions' minimum sun factor and tint (556041–556083, `re-sun-direction.md` §5).
- **The sky keys** [V][O] (`FUN_002e44d8`):
  - `AmbientColor` (0x627ebb4a, 271058) → `FUN_00493298` writes all three slots. It skips `DAT_0069c578` / `…580` once
    the keys below have set their flags (555595–).
  - `CharacterAmbientColor` (0xcda12257, 271251) → `FUN_00493328` → `DAT_0069c578` (555626–555627).
  - `VehicleAmbientColor` (0x47c6806c, 271113) → `FUN_00493378` → `DAT_0069c580` (555642–555643).
  - The reset `FUN_00493670` clears both flags (555812–555813).
- **Measured** [V] (PS2 patched states, `ps2exp/sheet_geo1.png`). BF1 GEO1 spawn, one slot's RGB zeroed in EE RAM per
  copy; the two controls agree:
  - `DAT_0069c570` black: the rock spires and the wing on the right darken. The soldier does not.
  - `DAT_0069c578` black: the **soldier turns black**. The world does not.
  - `DAT_0069c580` black: no change in view (no vehicle nearby).
  - So BF1 soldiers take `CharacterAmbientColor`, and the world props in view are static.
- **The port.**
  - It bakes `character_ambient` (`bake::lighting`) but lights every draw, soldiers included, with `AmbientColor`. It
    parses `VehicleAmbientColor` (`mungefs::sky`) but does not bake it.
  - 11 of 14 BF1 maps author `CharacterAmbientColor`; on 9 it differs from `AmbientColor`. In RAM: GEO1 d19c49 vs
    78654c, HOT1 a6a9ad vs ffffff, END1 8c8c78 vs 646e64.
  - This is card **CAMB**. BF2 has no such split: `FUN_00597e98` builds one ambient for every kind (§3).
- **[H]:** which BF1 draws carry a `shdw` volume (vehicles, by the key's name); the region list's source (card LREG).
