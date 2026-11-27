# PC Battlefront II environment-mapped materials (`MTRL` 0x100)

Session 2026-11-27. The sources and tools are `re-pc-bump.md`'s: the `SHDR` chunks of the PC `core.lvl`
(`fxc /dumpbin`) and `BattlefrontII.exe` (GOG build, unpacked, image base 0x400000, capstone).

The trigger case is a mod's Sith trooper armour, matte in the port and glossy on BF2 PC:

- `MTRL` flags 0x101 (0x100 | 0x01), no 0x80, no 0x20;
- second colour 0xff333333, power 10;
- `TNAM` 0 the diffuse, 1 and 2 empty, 3 a 64² cube map `tex_` (six `FACE`s, 7 mips).

Tags: `[R]` = read here from the bytecode or the exe. `[M]` = measured on the data.

[M] Every material with 0x100 in the measured mod (32 distinct) is `Normal`, has no 0x80 and no 0x20, and names
`TNAM` 3. Ten of them also carry 0x04.

## 1. The slot [R]

- The segment reader (`0x6e59e0`) reads `MTRL` as flags, diffuse colour, specular colour, power, two words and a
  string.
- A shader's material setup (`0x6f3e40`) stores the four `TNAM` textures as `[esi+0x24]` (slot 0), `+0x2c`
  (slot 1), `+0x28` (slot 2) and `+0x30` (slot 3), the flags at `+0x34`, the colour at `+0x48..0x54` and the power
  at `+0x58`.
- The specular pass sees that block as a material at `esi+0x24`: flags `+0x10`, colour `+0x24..0x30`, power
  `+0x34`, normal map `+8` (slot 1, as `re-pc-bump.md` uses it) and `+0xc` = **slot 3**.
- At `0x6f3ee3` a flags word with 0x100 gets 0x100 and does not also get 0x80: the env pass takes the specular
  pass's place.

## 2. The pass is queued and set up [R]

- `0x6f4646`: a segment with 0x100 is queued for the specular pass.
- `0x702720` (no normal map): `c0.rgb` = the material's colour × `byte[item + 0x29] / 255`; `c0.w` = 1 under 0x08.
- Directional lights take slots only under 0x80 (`0x70281c`).
- `0x702c06`: under 0x100 the cube takes one more slot. It is `[material + 0xc]`, or `[scene + 0x98]` when that is
  null. The slot's pixel colour is (1, 1, 1, 1) (`0x7fea54`), and its flag in `c42` is 1.
- `0x7020c0` picks the state by the slot count. With the env slot alone it is state 3 (`ps003`). With a normal map
  it is state 5 or 8 (`ps005`, `texm3x3vspec`), or 6 or 9 beside lights.

## 3. The shaders (`specularlighting_shader.xml`, state 3) [R]

Vertex shader (`vs040` and its format variants):

- R = I − 2 (I · N) N, I = vertex − camera, N the world normal;
- `mad oT1, R − half, c42.x, half`: with the env slot's `c42.x` = 1 the lookup vector is R itself;
- `mad oD0.xyz, atten, visibility, c42.x`: saturates to 1 in that slot.

`ps003`:

    r0.rgb = v0 * t1                 ; t1 = cube(R), v0 = 1
    r0.rgb *= c2                     ; (1, 1, 1)
    r1     = c0 * lrp(c0.w, t0.a, 1)
    out    = r0 * r1,  alpha = t0.a  ; t0 = the diffuse

So the pass writes `cube(R) × specular colour`.

## 4. Blend [R]

`0x7021a0` sets COLORWRITE 7, ALPHATEST on with ref 0x7f, ZWRITE off, blend ONE / ONE and a black fog colour. It
does not write `D3DRS_ALPHAFUNC`.

## 5. The port

- `bake::envmap` pairs a diffuse with its cube and writes `<diffuse>.env.ktx2` (six faces stacked, every mip).
- `bake::spec` gives the segment a `spec_pass` with the colour as bytes / 255.
- `shader.wgsl` `pc_env_spec`: `cube(R) × colour` in gamma over the base, nothing where the diffuse alpha is at
  or under 0x7f.
- PC geometry only. PS2 and Xbox disc assets never match.

PROVISIONAL / OURS:

- PROVISIONAL: the alpha compare is GREATER. trace: the last `D3DRS_ALPHAFUNC` write before the pass.
- OURS: the per-instance byte `[item + 0x29]` is read as 1.
- OURS: the diffuse's sampler reads the cube. trace: the stage's `SetSamplerState` calls.
- OURS: the bake keeps `texture_pc::Texture::best`'s surface. trace: which `FMT_` the exe's loader picks.
- OURS: one cube per diffuse sheet.
- OURS: world space is shared with the PC game, so R indexes the cube unchanged. measure: a PC still of a
  surface whose cube has distinct faces.

Not modelled:

- the map-wide cube for an empty slot 3 (`[scene + 0x98]`);
- the normal-mapped variant (`ps005`);
- the glossmap weight (0x08);
- the pass over a 0x04 base.

[M] The first three are on no env material measured.
