# PC asset inputs

PC assets are optional inputs to one self-contained game binary. A PC install
alone is insufficient: add a PS2 Battlefront II disc with `--iso2 path/to/disc.iso`
when running `scripts/pack.py --pc-dir path/to/installs`. The filename does not
identify the disc; its ISO9660 contents do. An already extracted PS2 BF2 tree
under `extracted/bf2/lvl/DATA/_LVL_PS2` also satisfies the requirement. Xbox BF2
will be an alternative console source after its reader and acceptance checks
are implemented; it is not accepted by this pipeline yet. There is no bypass.

For `scripts/rebake.sh --pc-dir path/to/installs`, extract the console BF2 disc
first. The bake always takes the shared menus, shell, HUD layouts, fonts and
sheets from console BF2. PC common/HUD art and PC texture alternatives are never
used for that shared UI. Console LVLs remain the source of missions, classes, geometry, collision and
navigation. PC map textures (both games) and BF2 side texture tiers can improve
their appearance without changing simulation numbers or shapes. Input files
are read only. The low-level importer still accepts PC geometry.

Until a PS2 reference table exists, PC BF1 input additionally needs the BF1 PS2
gameplay reference (`--iso1` or an extracted tree). Missing reference data is
refused explicitly; PC Lua is never installed as PS2 mission bytecode.

This supersedes PR #94's original claim that a PC-only installation produces a
supported build. The PC importer's VertexLit material path remains available;
its measured residual brightness gap is recorded in
[PC import visual parity](pc-import-visual-parity.md). Full calibration remains
open, and PS2 material dispatch remains unchanged.
