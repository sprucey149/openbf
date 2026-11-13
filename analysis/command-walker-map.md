# Command walker map markers, 2026-11-13

Maintainer scope: BF2 Hoth's AT-AT uses its own map icon and can be selected
as a friendly spawn point, as shown in the two supplied PS2 screenshots.
No issue created. Implementation base: `0d391f75`; earlier gun/footstep work
was committed separately as `a424406b`.

Read `dist/openbf_data/playtest/last.json` first: schema 17, exit snapshot,
BF2 Hoth conquest, build `f14ee6d9644b+dirty`, seed 400, tick 401. The human
is Rebel (team 1). The last world row (tick 360) has two live Imperial
AT-ATs (team 0, vehicle IDs 26 and 30), both at 60000 health, at
(228.8, -6.1, 51.07) and (198.27, -7.36, 113.52). The human's last position
is (-41.79, 0.06, 39.48). These are enemy posts outside the minimap's
ordinary vehicle detection distance, and cannot be Rebel spawn choices.

## Evidence and cause

`analysis/re-command-walker-post.md` section 5: BF2 `FUN_003a6a48`
(including lines 401684-401685 for index visibility) draws registered posts
at their owners' live positions, using the owner class's `MapTexture`,
`MapScale`, team and heading. The command walker's activation removes the
ordinary vehicle marker (`FUN_001ad1c0`). The AT-AT authors `atat_icon`,
scale 3.5 and no HUD index. The supplied screenshots corroborate its icon
and the friendly spawn selection ring.

The original post activation is BF2 `FUN_00355648`; factory `FUN_00354920`,
owned by command walker `FUN_0035baa0` (354287-354310). Spawn eligibility is
`FUN_00356880`: own team, live owner and authored spawn points. The existing
port already implements friendly command-walker spawn nodes and selection.
The same note's section 4 cites `FUN_00356998` for the live spawn transform,
and section 5 cites `FUN_0039a058` for directional post selection.

The port's ordinary minimap instead classified AT-ATs as vehicles. Such
icons require detection and disappear beyond the map radius. The old spawn
view appended only friendly walkers, separately from the minimap's list.

## Change

`MapStatic` now puts both teams' live command walkers in the post list on
every view. It reads the authored texture/scale, follows the hull's heading
and live position, omits a number and uses team color. Post placement clamps
to the minimap rim. The ordinary vehicle loop excludes command walkers,
including while the human occupies one, to avoid duplicate markers.

The spawn view selects within that same displayed post list. Friendly
eligibility, pointer/directional selection and the six live AT-AT spawn
nodes retain the existing simulation behavior. Destroyed or dying walkers
lose their markers and spawn choices through `Sim::command_walkers`.

## Verification

`scripts/gpu_tests.sh --game-lib minimap::`: 19 passed, 0 failed,
996 filtered out (`engine/artifacts/command-map-unit.log`).

`scripts/gpu_tests.sh --game command_walker_map`: 1 passed, 0 failed,
556 filtered out (`engine/artifacts/command-map-gpu-final.log`). The test
loads the local Hoth battle, confirms exactly two AT-AT post markers and
no duplicate vehicle markers, selects one through `ScreenPick`, renders
both views with a headless GPU, resolves `atat_icon`, accepts map and class
through ordinary input, and verifies the soldier spawned near the selected
walker at (209.99886, -6.9091425, 127.1783). Unit coverage includes enemy
markers outside detection/range, friendly-only spawn choices, an occupied
command walker, movement, and death invalidation. Scoped `git diff --check`
passes.

The final release is rebuilt and packed to `dist/openbf.exe`; the local
`engine/artifacts/dist-update-receipt.json` records archive and executable
verification plus preservation of profile/playtest files. No rebake needed.
Headless captures: `engine/artifacts/command-walker-map/minimap.png` and
`spawn.png`. The spawn capture was visually inspected: both blue AT-AT
silhouettes appear, with the selection ring on the chosen walker.

Unchecked: full alignment acceptance, a new emulator measurement of icon
pixels, and other maps' command-walker screenshots. No baked inputs or
simulation rules changed. Per-map 60 fps bot population and simulation
timing remain unmeasured for this change.
