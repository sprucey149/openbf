# PC Battlefront II bump materials (`MTRL` 0x20 / 0x08 / 0x80)

Session 2026-11-25. There is no Ghidra decomp of the PC exe, so this trace reads two sources:

- the shaders in the PC `core.lvl` (`SHDR` chunks, D3D9 bytecode, disassembled with the Windows SDK's
  `fxc /dumpbin`);
- `BattlefrontII.exe` (GOG build, unpacked, image base 0x400000), read with capstone.

The trigger case is the "SWBF2 Improved Sides mod 1.0" AT-RT, `rep_walk_oneman_atst` in `side/rep.lvl`:

- material `ATRT_diff` with `MTRL` flags 169 (0xa9 = 0x80 | 0x20 | 0x08 | 0x01);
- spec colour 0xffffffff and spec power 1;
- `TNAM` 0 `ATRT_diff`, 1 `ATRT_normal`.

Through the PS2 flat specular pass it drew washed out and flat. BF2 PC draws it darker and normal-mapped.

Tags: `[R]` = read here from the bytecode or the exe. `[M]` = measured on the data.

## 1. Sources

- `core.lvl`: `E:\Projects\bf1+bf2\star_wars_battlefront2\GameData\data\_lvl_pc\core.lvl`.
  - It holds 30 top-level `SHDR` chunks, each `RTYP` / `NAME` / `INFO` / `PIPE`.
  - `PIPE` = `INFO`, `VSP_` (a pool of `VS__`), `PSP_` (a pool of `PS__`) and `STAT` entries.
  - Each `STAT` = `INFO` (state id, pass count) and `PASS` children. A `PASS` = `INFO` flags, `PVS_` (vertex format,
    VS index) and `PPS_` (PS index).
  - A `VS__` / `PS__` body is a u32 size, then vs_1_1 / ps_1_1 bytecode without a CTAB.
- `bump` RTYP (`bump_shader.xml`): the `SHDR` at file offset 0x5ee514.
- `specularlighting` RTYP (`specularlighting_shader.xml`): the `SHDR` at 0x6ca86c.

## 2. The bump draw [R]

- **ps037** (state 13, the sun only):
  - N_b = `tex t1` `_bx2` (the normal map).
  - L = `v1_bx2`, the tangent-space sun, renormalised.
  - `light = sat(c0 + c1 * sat(N_b . L))`, then `out = t0 * v0 * light`.
  - vs416 writes `v0 = c39` and `v1 = -c22` in tangent space. It zeroes v1 where the vertex normal faces away from the
    light (N . L < 0).
- **ps041** (state 14) adds a second light per pixel, OUTSIDE the saturate: `+ c2 * sat(N_b . t2_bx2) * t2.w`.
- **ps045** (state 19) multiplies the sun term by `t2.w`, the shadow.
- States 31–37 add ps051's second pass.

The port, `shader.wgsl` `bump_lit`:

- light = `clamp(ambient + sun * sat(N_b . L) * front, 0, 1) + light2 * sat(N_b . L2) * front + points`;
- the result is texel × vertex colour × light, clamped at 1;
- the light terms are the map's authored colours, in the units the GS terms already use (`linear_to_srgb` of the
  globals);
- PC sat() clamps at 1, so the PS2's 2× headroom (`GS_MAX`) is gone. That is the darker look.

OURS, each with a note in `shader.wgsl`:

- The tangent frame comes from screen derivatives (T = dP/du, B = dP/dv), because every vertex attribute location is
  taken.
  - [M] The AT-RT's PC VBUF (flags 0x266, stride 60) holds pos, idx, normal, tangent, binormal and uv.
  - Its tangent is dP/du (mean cos 0.954) and its binormal dP/dv (mean cos 0.931), with CCW winding, in the uv openbf
    uses. So the derived frame is the authored one.
- The ambient stays the hemisphere of the vertex normal, and point lights stay the per-vertex `point_sum`.
- trace: what the bump vs uploads to c0, and the bump point-light states.

## 3. The specular pass [R]

`0x702480` is the spec pass entry:

- If the material has a normal map (`[material + 8]`) and the option byte `[0x85d08c]` is set, it binds the normal
  map to stage 0 and calls `0x702df0`.
- Otherwise it binds the diffuse and calls `0x702720`.

`0x7021a0` sets the pass states:

- COLORWRITE 7;
- ALPHATEST on with ref 0x7f;
- ZWRITE off;
- blend ONE / ONE.

`0x702449` binds `specularcubemap` (the global `0x1fed0c4`, set at `0x7880f0`) to stage 3 for every non-spot light.

`0x702df0` (bump spec):

- `c0` = the material's spec RGBA (`[material + 0x24..0x30]`) × `byte[item + 0x29] / 255`.
- `c0.w` = 1 when `[material flags] & 8` (glossmap), else 0.
- `c1` = `0x701ff0(power)`, where the power is `[material + 0x34]`.
- For each directional light (list +0x8c, count +0xa0, light word +4 bit 0x800):
  - VS `c42` = −(light dir), w = 0;
  - PS `c2` = light colour × a per-light float;
  - one draw per light.

`0x701ff0(power)` (constants 0x7dbd28 = 10, 0x7dbd6c = 1/90, 0x7dbd68, 0x7dbd60):

- `t = (power − 10) / 90`.
- If `t < 0.33333299`: `s = 3.00003·t` → (1−s, s, 0, 0).
- Else if `t < 0.66666597`: `s = (t − 0.33333001)·3.00003` → (0, 1−s, s, 0).
- Else: `s = (t − 0.66666001)·3.00003` → (0, 0, 1−s, s).
- There is no clamp in the code. ps_1_1 clamps constants to [−1, 1], so the channel anchors are powers 10 / 40 / 70 /
  100 for R / G / B / A.

`specularlighting` states 4–6 run ps004 / ps006:

- `t0` is the NORMAL map, and `texm3x3tex t3` = cube(N_b in vs044's frame).
- `lobe = dot(t3.rgb, c1.rgb) + t3.a·c1.a`.
- `out = lobe · c0 · c2 · v0.w · lrp(c0.w, t0.w, 1)`.

vs044:

- H = normalize(V − L), with L the light's travel direction.
- The frame is X = normalize(H × (T × H)), Y = H, Z = normalize(T × H).
- `v0` = (N_vertex . −L > 0) × attenuation.

`specularcubemap` [M]:

- A 128² cube, stored DXT3 with an A4R4G4B4 sibling.
- Only its +Y face (index 2) is non-zero: all 6 × 16384 texels were read.
- Its channels fit `floor(15·c^p)`, c = N . H, at p ≈ 10.1 / 30.5 / 54.4 / 101.6 for R / G / B / A. That fit is not
  analytically clean, so the bake keeps the real face (`bake pc-spec-cube`, `common_bf2/pc_specularcube.ktx2`).
- With no PC install, no face is baked and the pass adds nothing. No approximation is synthesised.

The port, `shader.wgsl` `pc_bump_spec`:

- per light (the sun and Light2), the +Y face sampled at `((d.x/d.y + 1)/2, (d.z/d.y + 1)/2)`;
- 0 when another face would be chosen;
- each add clamped to [0, 1] and summed in gamma over the base;
- no base-alpha weight.

The bake stores:

- the spec colour as bytes / 255 (`bake::spec`);
- the gloss lerp in the normal map's alpha (`bake::bump`: the map's alpha under 0x08, 255 without it).

OURS / trace:

- H is computed per pixel.
- The per-light float and `byte[item + 0x29]` are read as 1.
- The 0x800 light flag is not modelled.
- The alpha test (ref 0x7f) is not modelled.
- Which FMT the exe's loader picks for the cube is not traced; the bake takes the DXT3 face.

## 4. Which segments

`bake::bump::companion`:

- PC geometry (`Segment::pc_geometry`);
- a BF2 material;
- `MTRL` bit 0x20;
- non-empty `TNAM` slots 0 and 1.

PS2 and Xbox disc assets never match, so their draws are unchanged.

The companion is keyed per diffuse sheet. OURS: the port binds a material per texture.
