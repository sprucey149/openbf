---
id: GOLDX
title: More goldens, as groups: vehicles (each class in a fixed pose), a weapon / pickup sheet, the front-end menus beyond shell.png, a second vantage and a ground stand per map — each a goldens.sh group with its deps.toml table, so a change regenerates only what it can touch
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer's INBOX 2026-09-30 (speed-up round): expand the goldens beyond tat1 + the map / unit gallery, as GROUPS, so that a
change regenerates only the groups it can touch. Today: 6 groups (`tat1 hud shell spawn gallery walkabout`), 90 PNGs, 74 MB.

## Do (the maintainer's list)
- **Vehicles:** each class in a fixed pose (the gallery's vantage).
- **Weapons and items:** a pickup / weapon sheet.
- **Front-end menus** beyond `shell.png`: main menu, class select, options.
- **More map views** than the first command post: a second vantage and one ground stand per map.
Each new golden is a group: its test, a name in `scripts/goldens.sh` and `GOLDEN_GROUPS` in `scripts/gates.py`, a
`[groups.<name>]` table in `engine/goldens/deps.toml` with `not_by` prefixes ONLY for changes it provably cannot see,
and a README row.

## Acceptance
- `scripts/goldens.sh <group>` regenerates each new group alone; two runs byte-identical (paste the cmp).
- `scripts/gates_selftest.py`: for each new group one diff that runs it and one that its `not_by` skips; each `not_by`
  prefix backed by a mutation check written in the README row (a change under that prefix, the group unchanged).
- Coverage counts in the report: vehicle classes sheeted / classes baked; maps with a second vantage + a ground stand
  (32 of 32); menu screens (≥ 3).
- Total size under the README's ceiling (74 MB over 90 PNGs today), or the ceiling raised with the reason written there.
- Every gate PASS; the new goldens are viewed and committed by land.py (declared `goldens=<the new files>`).

## Notes
- Planner 2026-09-30 21:45: the maintainer's header — M0, value 3, build, alpha: no, defer: none.
- Do it after GSKIP (the skip rule this card's `not_by` lines extend is being corrected there).
- gpu_tests.sh edits are classifier-denied (memory `ci-after-rebake-and-gate-script-edits`): keep the change in
  goldens.sh / gates.py / deps.toml, or list the gpu_tests.sh line under For the maintainer.
- Larger than one session: vehicles + second vantages first, menus and the weapon sheet as `resume:`.
- 2026-10-07 planner, from WADE's report (`reports/20261006_2157_card_WADE.md`): **`gallery::ground_stand` takes a CUT terrain as the floor** at 3 posts (tat1 cp3 by 2.7 m - the walkabout sheet `tat1/cp3_cp2` lost its scale soldier, who now stands on the real floor under the frame; BF2 GEO1 cp8 1.89 m, BF2 HOT1 cp4 0.83 m). Fixing it moves the walkabout's eye and possibly `ground_view` (the map goldens' stand): do it with this card's 'ground stand per map', check the map goldens first.
