# Hoth generator and turret map/target HUD, 2026-11-13

Maintainer request: restore the shield generator's red square map marker,
enemy reticle and named health bar, and neutral gray turret dots with occupied
team color and firing feedback. Scope is the existing Hoth objects and the
shared map/health-target paths. Base `19077869`, with concurrent vehicle pose
work excluded from this change's ownership. No issue created.

## Snapshot and original-game evidence

Read `dist/openbf_data/playtest/last.json` first and retained it locally as
`engine/artifacts/hoth-object-reported.json`: BF2 Hoth conquest, seed 404,
tick 2934 (97.8 s), clean build/checkout `19077869f8be`, human team 1 (Rebel),
position (-314.01, -7.48, 163.77), reticle and magnet both absent at exit.
The attached PS2 screenshot shows the square objective icon and gray turret
markers. The generator belongs to the Rebels: it is red/enemy for an Imperial,
blue/friendly for a Rebel.

The local class `hoth_bldg_shieldgenerator` derives from `commandpost`, authors
`MapTexture hud_dest_icon`, `MapScale 3`, `MaxHealth 120000`, a destroyed model
and an explosion. Its placed instance is `shield`, authored team 2 (port team
1), at (-423.489, -16.558, 204.95). The conquest mission's objective list contains
CP3–CP6; the generator is not one of those capturable/spawnable posts. Filtering
that list must not remove the generator's health entity or registered icon.

`re-hud-map.md` section 3 records the generator as a registered post drawn by
BF2 `FUN_003a6a48`, using its own `hud_dest_icon`; the square is authored art,
not a substitute for a command-post circle. Registration refreshes the owner
class's map information in `FUN_003a9098`.

Follow-up source reading corrects the abbreviated neutral-object rule in that
note. In the current local decomp export, `FUN_003a64b8` lines 389006–389035
returns visible for unmatched static types, friendly objects, and team-0
objects before distance/cone detection. Neutral turrets are not hidden just
because they are outside the soldier's detection cone. Ordinary map icons
still clip at the circle rather than clamping to its rim.

`FUN_003a5940` lines 388779–388817 blends inactive objects halfway toward
RGB (64,64,64), giving (128,128,128) from neutral (192,192,192). Occupied turret
colors use their live team. Firing feedback comes from `FUN_003a68d0`, lines
389118–389127: refire progress, rather than an invented periodic clock.
The highlight defaults to white (map constructor `FUN_003a2d60`, line 387417;
color initialization `FUN_0057ca38` at 700143–700169). The port uses its live seat occupancy
and gun refire state for those inputs.

`re-target-hud.md` sections 2–4: publisher `FUN_003b3398` uses the controller's
acquisition winner when there is no valid magnet (documented B2 lines
410036–410060); `FUN_002b0548` writes that winner. Any finite-health building
can supply a class name, health fraction and team relation. This includes
the Hoth generator before it has taken damage.

## Changes

The shared building-health loader now admits command-post classes with finite,
positive authored health. Ordinary indestructible command posts remain outside
that health list. The generator reuses existing health, collision binding,
damage effects, explosion and authored wreck support; no game data is added.

Health-bearing placed objects supply their class's map art. Command-post
objects are appended after the existing post/walker list, preserving spawn-ring
indices; the generator remains a marker, not an extra conquest spawn choice.
Destroyed objects lose their live markers.

Finite-health buildings now enter the infantry reticle/readout competition
without becoming infantry aim magnets. Armed ground seats use the same target
readout path as flyers. The relation is the object's authored team, not a
hardcoded enemy color. Neutral turrets bypass enemy detection and use muted
gray while empty; occupied guns retain team color with refire-driven highlight.

## Verification

Focused checks against the working source based on `19077869`:

- `cargo check --manifest-path engine/Cargo.toml -p game`: passed.
- `scripts/gpu_tests.sh --game-lib minimap::`: 20 passed, 997 filtered out.
- `cargo test --manifest-path engine/Cargo.toml -p core --lib sim::input::tests -- --test-threads=1`:
  32 passed, 2231 filtered out.
- `cargo test --manifest-path engine/Cargo.toml -p core --lib sim::lock_on::tests -- --test-threads=1`:
  12 passed, 2251 filtered out.
- `scripts/gpu_tests.sh --game hoth_object_hud`: 2 passed, 557 filtered out.
- `scripts/gpu_tests.sh --game command_walker_map`: 1 passed, 558 filtered out.

All GPU checks ran headless and serial. Logs are under
`engine/artifacts/hoth-{map-unit,target-core,target-lock,object-final,object-walker-regression}.log`.
The Hoth tests verify the generator's authored health/team, enemy and friendly
reticle acquisition, localized name, full and half-health readout, post icon,
no new spawn choice and removal of a destroyed marker. The neutral-turret test
uses a turret 30 m to the side of the player, outside the old detection cone.
The map unit check covers empty gray, occupied friendly/enemy color and changing
refire pulse without an arbitrary time period.

Visually inspected the headless captures
`engine/artifacts/hoth-object-hud/generator.png` (square/destroyable marker,
red reticle and named bar) and `neutral-turrets.png` (muted gray turret dots).
The final release is rebuilt and packed to `dist/openbf.exe`; local
`engine/artifacts/dist-update-receipt.json` records binary/archive verification
and profile/playtest preservation. No rebake is needed for this HUD change.

No complete alignment acceptance, new live PS2 pixel comparison, or generator
death-script/objective callback parity is claimed. Existing building destruction
is reused; Hoth's mission-script callbacks are outside this HUD request.
Per-map 60 fps bot populations and simulation timings remain unmeasured.
