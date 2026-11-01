# PC import visual parity, 2026-11-01

Scope: improve PC geometry imports against the existing PS2 import, starting with
BF1 Bespin Cloud City.

PC geometry now retains its platform identity after parsing. Its prelit material
colours select a dedicated vertex-lighting path instead of the PS2 additive
paths. The new path uses the baked colour plus gathered dynamic point lights;
it does not add unity, ambient or directional lighting already present in the
baked colour. PS2 material dispatch retains its existing paths.

The HUD bake identifies PC input using the same geometry-format detector as
the model reader; the weapon-panel anchor limits adaptation to the stock
one-player layout. Its single-player layout is adapted to the existing PS2 coordinate system: weapon
panel, health/energy/hero/vehicle bars, target readout, reticle, minimap, team
icons and counters. Text uses smaller PC font assets with adjusted sizing.
The PC font artwork remains different. PS2 layouts and multiplayer layouts are
not normalized. This describes the low-level importer experiment; the supported
pack/rebake pipeline requires a console BF2 disc and takes menus, shell and HUD
from its console data, including fonts, rather than this PC approximation.
PC-only builds are refused.

## Evidence

Local artifacts are under `engine/artifacts/` and are not public deliverables:

- `pc-ps2-bespin-comparison.png`: before, corrected PC, and PS2-reference renders
  at camera `-95.328,-31.4,68,0,0`, size 1024x538.
- `pc-hud-before.png`, `pc-hud-after.png`, `ps2-hud-reference.png`: HUD comparison.
- Left-wall sample rectangle `(40,190)-(260,315)`: PC mean RGB brightness
  253.9 before, 155.9 after, PS2 139.5; near-white clipping 97.7% before,
  0% after and on PS2. The corrected PC sample is still 16.4 brightness units
  (11.8%) above the PS2 sample. Different source art affects the result; the
  VertexLit path is retained, but this is not full lighting calibration.
- PS2 Bespin render before/after: identical PNG bytes.
- Bakes using the saved previous bake executable versus the changed executable:
  BF1 PS2 BES2 197 files, BF2 PS2 COR1 182 files, BF2 PS2 INGAME/common 326
  files. All 705 files have identical SHA-256 hashes.
- Parser tests: 9 passed. Selected bake/lighting tests: 28 passed. PC HUD
  normalization tests: 3 passed. PS2 HUD layout tests against the PS2 asset
  tree: 11 passed, 1 pre-existing ignored census. GPU lighting-path test,
  including the new path and the existing PS2 paths: passed.

All 33 existing PC map bakes and BF2 common/HUD were refreshed. The release
game was rebuilt and packed with the current `assets_baked` and `missions`
trees into a temporary executable, then replaced `openbf-pc/dist/openbf.exe`
after packing succeeded. `dist/openbf_data` was preserved. The separate
`openbf` checkout and its PS2 distribution were not modified.

## PR review

The parity patch is prepared on `pc-asset-loading` without the first-person
feature commits or their staged asset fixes. HUD font updates share one helper,
and only nodes with child properties are visited. Model parsing and the HUD
source guard share `mungefs::model::is_pc_geometry`. Tests also cover PS2
layouts with PC-like coordinates, untouched multiplayer layouts, and dynamic
point lights versus already-baked static lights on the new rendering path.

Clippy with warnings denied passes for all four affected crates and all their
targets. The workspace-wide command encounters an existing `let_and_return`
warning in `core/src/sim/force.rs`; that unrelated file is left unchanged.

Final review checks: workspace tests passed (3425 passed, 87 ignored),
workspace doc tests passed, rustfmt passed, and the GPU lighting referee passed.
The reviewed bakes retain all 705 sampled PS2 files and all 326 PC common/HUD
files byte for byte. The combined local playtest checkout remains separate
from the PR commit.

## Integration limits

PC data is an optional visual source. Builds require a console BF2 source; the
menus, shell, HUD sheets, layouts and fonts come from that console data. The
PC HUD adaptation described above is historical importer evidence, not the
supported build pipeline. PC data alone cannot supply the console interface.

Keep the PC-only `VertexLit` dispatch. The reported wall sample remains brighter
than PS2: 155.9 versus 139.5 mean RGB (about 11.8%). The path excludes static
lighting already baked into vertex colours and adds dynamic point lights; it
does not establish parity for dynamic directional lights, all PC material flag
combinations, instance tints or all maps. No full calibration was attempted at
integration. The PS2 material dispatch and shader paths stay unchanged.
