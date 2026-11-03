---
id: PARB
title: parity.py: the gate can score the tree and main over different shared bakes — main's cache is keyed by sha only and rendered in a separate ci.lock hold, so a landing's rebake between the two renders (or a main_scores run between a landing's ff-merge and its rebake) crosses bakes and caches the old bake's scores under the new sha
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Found by the CLNT cleanse (2026-09-30, inferred, a race): `cmd_gate` renders the tree, then `main_scores()` takes ci.lock again; `loop/parity/main/<sha>.csv` is keyed by main's sha alone.

## Evidence
scripts/parity.py `main_scores` (cache = PUB/main/<head>.csv) and `cmd_gate` (tree render, then main render in a second lock hold). land.py phase 1 ff-merges, then rebakes: a parity run in between scores the new sha over the old bake.

## Note (AUDIT VTUR2, 2026-09-30) — pose_render draws no common-bake prop
`game/examples/pose_render.rs` loads the map scene and applies the conquest layer, but never merges `common_<game>`
(`main/battle.rs:33` does). So every parity pair lacks what the game draws from common:
- BF2's 48 build-zone pads (`com_prop_buildzone`, from INGAME);
- BF1's gun turrets (`com_weap_gunturret`, BF1 COMMON; BZONE1's note);
- command posts and crates, if they come from common.
The parity score then counts those props as PS2-only pixels on every pose that sees one. The fix is one block, as in
battle.rs: `load_common(baked_root()/common_<game>)` then `scene.merge_common(c)`. It is proven in
`lead_tmp/audit-vtur2/audit_pose.rs`: the pads then draw where the PS2's do (COR1 walk/cp1_cp2_b090, TAT2
walk/cp1_cp2_b270). Expect parity to move on maps with common props: declare it as a parity re-baseline.

## Cloud source checkpoint (2026-10-24)
Implemented SHA + manifest-content cache identity and one CI hold across gate
builds/renders/main-cache/publication. Legacy SHA-only CSVs are ignored. Four
synthetic fixtures cover changed-manifest rescoring at fixed HEAD, inventory/path
identity, real OS lock exclusion and renderer lock nesting. Parity11 and the
full Python92 suite PASS (one corpus-dependent skip); constant-fingerprint and
no-lock mutants each fail two referees. See reports/20261024_1800_card_PARB.md.
The old common-props note above is historical: pose_render currently calls
`assets::load_battle_scene` (SKIPC), including COMMON by default. No scene edit
or parity rebaseline is part of this source checkpoint.
