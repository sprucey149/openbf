---
id: SBD
title: Super battle droid tri-shot, wrist rockets and HUD icons
status: archived
---

Historical technical record. Recorded state before archival was `in-progress`.

Issue: https://github.com/sprucey149/openbf/issues/27

BF2 SBD weapon mechanics and slot ammunition are shared by both map sets.
Tri-shot tilts, secondary rocket projectiles and the original HUD meshes are
implemented through the existing weapon, ordnance, roster and bake paths.
SBD audio follows the map's source game: BF1 on BF1 maps, BF2 on BF2 maps.
The canonical rocket's original particle trail loads on BF1 CIS battles and
replaces the generic box. Effects retain their own source era's particle rules.

Review removes duplicate soldier projectile spawning, reuses inherited-property
and effect readers, removes ignored effect-era arguments, and identifies the
weapon in reload events so wrist rockets use their own reload sound.

`analysis/sbd-weapons.md` records both discs' numbers, implementation decisions,
headless acceptance, sound provenance and local capture reproduction. Final
validation logs and packed-build evidence are under the ignored
`engine/artifacts/lead_tmp/sbd/review-*` paths. The local distribution is refreshed
before handoff while preserving profiles.

Live original-game firing comparison remains outstanding. Source/bake checks and
port screenshots are not claimed as a completed PS2 side-by-side match.
