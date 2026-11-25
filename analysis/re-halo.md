# OPUS RE (2026-10-29) - [O] UNAUDITED
# RE: BF2's LightBeam effects — the `halo` glows on props (issue #51)

**The question** (issue #51): Mustafar's ceiling lamps wear a warm cone (`halo`) and the collection platform a blue
one (`Blue_halo`) on the PS2; the port drew neither. Both names are `fx__` resources whose root node is
`LightBeam("<name>")` (hash 0xe8651328), not a `ParticleEmitter`, so the effect baker skipped them. BF1 has no
LightBeam anywhere on its disc.

## 1. The parser FUN_00456ff0 (vtable 0x639d88)

| Property | Hash | Field | Rule |
|---|---|---|---|
| Color | 0x3d7e6258 | +0x170 | each value clamped 0..255 to a byte; alpha 255 when fewer than 4 values |
| FlareIntensity | 0x633ed9d4 | +0x194 | default 1.0 |
| InitialWidth | 0x1414bf95 | +0x178 / +0x17c | halved; one value sets both |
| FinalWidth | 0xaa671039 | +0x180 / +0x184 | halved; both 0 unless every value is > 0 |
| FadeLength | 0x20c351fb | +0x188 | default 1.0 |
| FadeFactor | 0xa939a4bc | +0x18c | default 0.2 |
| DrawDistance | 0x1612c34c | +0x190 | squared; 0 = no limit of its own |
| Length | 0x83d03615 | +0x174 | default 10; stored max(v, 0) |
| Position | 0x934f4e0a | +0x150 | |
| Direction | 0xdf6dc76a | +0x160 | default (0, 1, 0) |
| BeamIntensity | 0x75096fab | — (PS2) / +0x190 (PC) | ignored on PS2; PC ctor `0x0049aee0` sets default 1.0, parser `0x00641d20` reads `0x75096fab` into `+0x190`, renderer `0x00653dc0` multiplies view alpha factor `f` by `+0x190` on both beam and glow paths and returns early when glow `centre_alpha == 0` |

With neither a Length > 0 nor a Direction authored, the length is 0 and the direction (0, 1, 0). Alpha 0 (+0x173)
draws nothing. vtable +0x40 is a per-view hide bit (+0x12c).

## 2. The draw FUN_00469628

Culled when +0x190 > 0 and the squared distance reaches it, and past the view's own far distance squared.
Additive, depth-tested, BF2 `white.tga` modulated (the GS RGB scale 1.0).

- **Length > 0 (a beam)**: the port's existing beam (`render::light_beam`, the CP hologram's) with the top row's alpha
  times +0x18c, the cap over +0x188, and elliptic radii seen along the view's side vector:
  r = sqrt(rx²·|side·X|² + rz²·|side·Z|²) when rx ≠ rz.
- **Length ≤ 0 (a glow)**: a camera-facing disc. The centre and an inner ring of 16 points at R = +0x180 sit R toward
  the camera; an outer ring at R + +0x188 sits at the base, at half-step angles; the points are (−sin θ, cos θ),
  θ = kπ/8. Alphas: centre round(A·f), inner trunc(centre·+0x18c), outer 0. Near fade: gap = depth − near; none
  when gap < R, (gap − R)/+0x188 up to R + +0x188. The distance fade uses the combined band below. Triangles: a fan
  (I_k, I_k+1, C) and the annulus.
- +0x120/+0x124 (from +0x194 and the colour) feed a separate lens-flare object. **Not ported**: the port has no lens
  flares anywhere.

## 3. Census (BF2 map LVLs)

- Beams (Length > 0): MUS1 `halo`, `Blue_halo` (length 1, InitialWidth 0.8/0.2, FinalWidth 1.3/0.8, FadeFactor 0.3,
  FadeLength 0.8, DrawDistance 300); TAN1 `all_halo`; NAB2 `fountain_halo`; SPA1–9 side halos (space is not baked).
- Glows (Length 0): DEA1 `dea1_halo_blue` / `dea1_halo_red`; COR1 and GEO1 `halo_yellow`; MYG1 `halo`;
  UTA1 `uta1_bldg_lizardpen`; GAL1 `halo`, `star`, `starblue`.

## 4. The port

`mungefs::lightbeam` parses the node; `bake::effects` writes `FxSet.beams` (z flipped on position and direction);
`game::fx::props::Glow` places each on its prop's hardpoint frame and submits it every frame as an additive
`ParticleDraw::Beam`; `render::light_beam::glow_vertices` builds the disc.

## 5. Distance-band follow-up (#77)

[V] `FUN_00469628` reads the view fade/far distances through `FUN_00565120` / `FUN_00565130`, squares both,
and tests the effect's positive distance and the view far distance separately (528793–528830).
Both glow and beam paths select the earlier fade start and shorter far end (528909–528916, 529211–529218).
[D] Thus `begin² = min(view_begin², 0.81 * effect_distance²)` and
`end² = min(view_end², effect_distance²)`, omitting the effect terms when its distance is zero.
Past begin, the single factor is `(end² - distance²) / (end² - begin²)`; the two fades are not multiplied.

Rechecked with `python3 scripts/re_check.py fn b2 528793 528795 528829 528909 528910 528913 528914 528916 529211 529212 529215 529216 529218`:
all thirteen lines resolve to `FUN_00469628` at `0x00469628` (header 528656), including
*(decompiler detail omitted)*, *(decompiler detail omitted)*, and the ratio above (the beam uses *(decompiler detail omitted)*).

[ours] The port supplies its existing world fade/cull policy through `game::fx::props::view_fade` to `Beam::view`.
Its relaxed prop reach and finite camera-distance fallback are recorded in `FEEL_NOTES.md` (#77 follow-up).
